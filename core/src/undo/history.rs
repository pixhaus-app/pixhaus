//! The branching undo/redo history tree.
//!
//! # Tree model
//!
//! Every push creates a new node as a child of the current node. Undo
//! moves `current` to the parent without discarding the child, so the
//! redone branch survives if the user later makes a new edit instead
//! of redoing. Multiple children represent diverging edit branches;
//! redo always follows the most-recently-used child.
//!
//! Nodes are stored in a `HashMap` keyed by a monotonically increasing
//! `NodeId`. This lets deletion reduce `nodes.len()` by one per drop —
//! no tombstone slots accumulate — while keeping all other indices stable.
//!
//! # Memory cap
//!
//! The cap is enforced on push. When `nodes.len()` exceeds
//! `max_commands`, or the summed `estimated_size_bytes` across all live
//! nodes exceeds `max_bytes`, the oldest reachable ancestor of the root
//! branch is evicted. Off-path branches hanging off the evicted node
//! are dropped recursively. The surviving path-continuation node becomes
//! the new root (its parent is set to `None`).

use std::collections::HashMap;

use crate::project::Project;

use super::command::{CoalesceResult, Command};
use super::error::{Error, Result};

/// Opaque identifier for a history node. Monotonically increasing; never reused.
type NodeId = u32;

/// Tuning parameters for the history.
#[derive(Clone, Debug)]
pub struct HistoryConfig {
    /// Maximum number of nodes retained across all branches.
    ///
    /// Default: `500`.
    pub max_commands: usize,
    /// Maximum total heap bytes across all retained commands.
    ///
    /// Default: `500 * 1024 * 1024` (500 MiB).
    pub max_bytes: usize,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            max_commands: 500,
            max_bytes: 500 * 1024 * 1024,
        }
    }
}

/// A single node in the history tree.
struct HistoryNode {
    command: Box<dyn Command>,
    parent: Option<NodeId>,
    /// Children ordered oldest-first; the last entry is most-recently visited.
    children: Vec<NodeId>,
}

impl HistoryNode {
    fn new(command: Box<dyn Command>, parent: Option<NodeId>) -> Self {
        Self {
            command,
            parent,
            children: Vec::new(),
        }
    }
}

/// The branching undo/redo history.
///
/// `current` is `None` when the project is in its initial (unapplied)
/// state, i.e. every command has been undone. When `current` is `Some(id)`,
/// node `id` is the most recently applied command.
pub struct History {
    nodes: HashMap<NodeId, HistoryNode>,
    /// Next ID to assign. Monotonically increasing; never reused.
    next_node_id: NodeId,
    /// ID of the most recently applied command, or `None` at root.
    current: Option<NodeId>,
    /// Summed `estimated_size_bytes` across all live nodes.
    total_bytes: usize,
    config: HistoryConfig,
}

impl History {
    /// Creates an empty history with the given configuration.
    #[must_use]
    pub fn with_config(config: HistoryConfig) -> Self {
        Self {
            nodes: HashMap::new(),
            next_node_id: 0,
            current: None,
            total_bytes: 0,
            config,
        }
    }

    /// Creates an empty history with the default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(HistoryConfig::default())
    }

    /// Number of live nodes currently in the tree (across all branches).
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Label of the command that would be undone next, if any.
    #[must_use]
    pub fn undo_label(&self) -> Option<&str> {
        self.current
            .and_then(|id| self.nodes.get(&id))
            .map(|n| n.command.label())
    }

    /// Label of the command that would be redone next, if any.
    ///
    /// When `current` points to a node, follows the most-recently-visited
    /// child (last in the children list). At root (`current` is `None`),
    /// returns the label of the most-recently-created root-level node (the
    /// parentless node with the highest `NodeId`), which is the last root
    /// that was pushed while at the root state.
    #[must_use]
    pub fn redo_label(&self) -> Option<&str> {
        self.next_redo_id()
            .and_then(|id| self.nodes.get(&id))
            .map(|n| n.command.label())
    }

    /// Apply `command` against `project` and push it onto the history.
    ///
    /// If the top command coalesces with this one, the two are merged
    /// into a single history entry and `command` is dropped.
    ///
    /// # Errors
    ///
    /// Propagates any error from `command.apply`. On error the command
    /// is not recorded.
    pub fn push(&mut self, mut command: Box<dyn Command>, project: &mut Project) -> Result<()> {
        // Attempt coalescing with the current top command.
        if let Some(cur) = self.current {
            if let Some(node) = self.nodes.get_mut(&cur) {
                if let CoalesceResult::Merged = node.command.coalesce(command.as_ref()) {
                    return Ok(());
                }
            }
        }

        command
            .apply(project)
            .map_err(|source| Error::CommandFailed {
                label: command.label().to_owned(),
                source,
            })?;

        let parent = self.current;
        let node_id = self.next_node_id;
        self.next_node_id = self.next_node_id.saturating_add(1);
        let size = command.estimated_size_bytes();

        self.nodes
            .insert(node_id, HistoryNode::new(command, parent));
        self.total_bytes += size;

        if let Some(p) = parent {
            if let Some(parent_node) = self.nodes.get_mut(&p) {
                parent_node.children.push(node_id);
            }
        }

        self.current = Some(node_id);
        self.enforce_cap();
        Ok(())
    }

    /// Undo the current command, moving `current` to its parent.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NothingToUndo`] if already at root.
    /// Propagates any error from the command's `undo` implementation.
    pub fn undo(&mut self, project: &mut Project) -> Result<()> {
        let cur = self.current.ok_or(Error::NothingToUndo)?;
        let (label, parent) = self
            .nodes
            .get(&cur)
            .map(|n| (n.command.label().to_owned(), n.parent))
            .ok_or(Error::NothingToUndo)?;

        self.nodes
            .get_mut(&cur)
            .ok_or(Error::NothingToUndo)?
            .command
            .undo(project)
            .map_err(|source| Error::CommandFailed { label, source })?;

        self.current = parent;
        Ok(())
    }

    /// Redo the most-recently-visited child of `current`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NothingToRedo`] if no child exists.
    /// Propagates any error from the command's `apply` implementation.
    pub fn redo(&mut self, project: &mut Project) -> Result<()> {
        let next = self.next_redo_id().ok_or(Error::NothingToRedo)?;
        let label = self
            .nodes
            .get(&next)
            .map(|n| n.command.label().to_owned())
            .ok_or(Error::NothingToRedo)?;

        self.nodes
            .get_mut(&next)
            .ok_or(Error::NothingToRedo)?
            .command
            .apply(project)
            .map_err(|source| Error::CommandFailed { label, source })?;

        // Move the redone child to the end of its parent's child list so
        // subsequent redos continue down this branch.
        if let Some(p) = self.nodes.get(&next).and_then(|n| n.parent) {
            if let Some(parent_node) = self.nodes.get_mut(&p) {
                if let Some(pos) = parent_node.children.iter().position(|&c| c == next) {
                    parent_node.children.remove(pos);
                    parent_node.children.push(next);
                }
            }
        }
        self.current = Some(next);
        Ok(())
    }

    /// Returns the `NodeId` of the next node to redo, if any.
    fn next_redo_id(&self) -> Option<NodeId> {
        match self.current {
            Some(id) => self.nodes.get(&id)?.children.last().copied(),
            None => {
                // At root: redo the most-recently-created parentless node
                // (highest NodeId among nodes with parent == None).
                self.nodes
                    .iter()
                    .filter(|(_, n)| n.parent.is_none())
                    .map(|(&id, _)| id)
                    .max()
            }
        }
    }

    /// Evict the oldest ancestor on the current path until both caps are met.
    ///
    /// When evicting the oldest node:
    ///  - Off-path children (not on the current undo path) are dropped
    ///    recursively along with their entire subtrees.
    ///  - The path-continuation child becomes the new root (parent set to
    ///    `None`).
    ///  - The evicted node is removed from the map. `nodes.len()` decreases.
    fn enforce_cap(&mut self) {
        loop {
            if self.nodes.len() <= self.config.max_commands
                && self.total_bytes <= self.config.max_bytes
            {
                break;
            }

            let path = self.current_path();
            if path.len() < 2 {
                // Cannot evict the only/current node.
                break;
            }

            let oldest = path[0];
            let keep = path[1]; // path-continuation child; becomes new root

            // Drop off-path children of `oldest`.
            let off_path: Vec<NodeId> = self
                .nodes
                .get(&oldest)
                .map(|n| n.children.iter().filter(|&&c| c != keep).copied().collect())
                .unwrap_or_default();
            for child in off_path {
                self.drop_subtree(child);
            }

            // Detach `keep` from `oldest` so it becomes a new root.
            if let Some(node) = self.nodes.get_mut(&keep) {
                node.parent = None;
            }

            // Remove `oldest`.
            if let Some(node) = self.nodes.remove(&oldest) {
                self.total_bytes = self
                    .total_bytes
                    .saturating_sub(node.command.estimated_size_bytes());
            }
        }
    }

    /// Recursively drop an entire subtree rooted at `id`.
    ///
    /// Does not touch the parent's child list — the caller is responsible
    /// for unlinking `id` from its parent before calling this.
    fn drop_subtree(&mut self, id: NodeId) {
        let Some(node) = self.nodes.remove(&id) else {
            return;
        };
        self.total_bytes = self
            .total_bytes
            .saturating_sub(node.command.estimated_size_bytes());

        if self.current == Some(id) {
            self.current = node.parent;
        }

        for child in node.children {
            self.drop_subtree(child);
        }
    }

    /// Returns the path from the root ancestor of `current` down to
    /// `current`, as a `Vec` of node IDs oldest-first.
    fn current_path(&self) -> Vec<NodeId> {
        let mut path = Vec::new();
        let mut cur = self.current;
        while let Some(id) = cur {
            path.push(id);
            cur = self.nodes.get(&id).and_then(|n| n.parent);
        }
        path.reverse();
        path
    }

    /// Returns an ordered iterator over labels from oldest to newest
    /// along the current undo path.
    pub fn labels(&self) -> impl Iterator<Item = &str> {
        self.current_path()
            .into_iter()
            .filter_map(|id| self.nodes.get(&id))
            .map(|n| n.command.label())
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Project;

    type CmdResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

    // ---- helpers -----------------------------------------------------------

    /// A command that appends a char to `project.metadata.name` on apply
    /// and removes it on undo.
    struct AppendChar(char);

    impl Command for AppendChar {
        fn label(&self) -> &'static str {
            "append char"
        }

        fn apply(&mut self, project: &mut Project) -> CmdResult {
            project.metadata.name.push(self.0);
            Ok(())
        }

        fn undo(&mut self, project: &mut Project) -> CmdResult {
            project.metadata.name.pop();
            Ok(())
        }
    }

    /// A command that coalesces by appending the other command's char to
    /// its own char list, producing a single coalesced entry.
    struct CoalescingStroke {
        chars: Vec<char>,
    }

    impl CoalescingStroke {
        fn new(c: char) -> Self {
            Self { chars: vec![c] }
        }
    }

    impl Command for CoalescingStroke {
        fn label(&self) -> &'static str {
            "stroke"
        }

        fn apply(&mut self, project: &mut Project) -> CmdResult {
            for &c in &self.chars {
                project.metadata.name.push(c);
            }
            Ok(())
        }

        fn undo(&mut self, project: &mut Project) -> CmdResult {
            for _ in &self.chars {
                project.metadata.name.pop();
            }
            Ok(())
        }

        fn coalesce(&mut self, next: &dyn Command) -> CoalesceResult {
            if next.label() == "stroke" {
                self.chars.push('x');
                CoalesceResult::Merged
            } else {
                CoalesceResult::Keep
            }
        }
    }

    fn make_project() -> Project {
        let mut p = Project::new("");
        p.metadata.name = String::new();
        p
    }

    // ---- basic apply/undo/redo ---------------------------------------------

    #[test]
    fn push_applies_and_advances_current() {
        let mut h = History::new();
        let mut p = make_project();
        h.push(Box::new(AppendChar('a')), &mut p).unwrap();
        assert_eq!(p.metadata.name, "a");
        assert_eq!(h.current, Some(0));
    }

    #[test]
    fn undo_reverses_command() {
        let mut h = History::new();
        let mut p = make_project();
        h.push(Box::new(AppendChar('a')), &mut p).unwrap();
        h.undo(&mut p).unwrap();
        assert_eq!(p.metadata.name, "");
        assert_eq!(h.current, None);
    }

    #[test]
    fn redo_reapplies_command() {
        let mut h = History::new();
        let mut p = make_project();
        h.push(Box::new(AppendChar('a')), &mut p).unwrap();
        h.undo(&mut p).unwrap();
        h.redo(&mut p).unwrap();
        assert_eq!(p.metadata.name, "a");
        assert_eq!(h.current, Some(0));
    }

    #[test]
    fn undo_at_root_returns_error() {
        let mut h = History::new();
        let mut p = make_project();
        assert!(matches!(h.undo(&mut p), Err(Error::NothingToUndo)));
    }

    #[test]
    fn redo_at_leaf_returns_error() {
        let mut h = History::new();
        let mut p = make_project();
        h.push(Box::new(AppendChar('a')), &mut p).unwrap();
        assert!(matches!(h.redo(&mut p), Err(Error::NothingToRedo)));
    }

    // ---- round-trip invariance ---------------------------------------------

    #[test]
    fn apply_undo_apply_undo_invariant() {
        let mut h = History::new();
        let mut p = make_project();

        h.push(Box::new(AppendChar('a')), &mut p).unwrap();
        let after_apply = p.metadata.name.clone();

        h.undo(&mut p).unwrap();
        let after_undo = p.metadata.name.clone();

        h.redo(&mut p).unwrap();
        assert_eq!(
            p.metadata.name, after_apply,
            "second apply must match first"
        );

        h.undo(&mut p).unwrap();
        assert_eq!(p.metadata.name, after_undo, "second undo must match first");
    }

    // ---- branching (edit after undo) ---------------------------------------

    #[test]
    fn branching_preserves_redo_branch() {
        let mut h = History::new();
        let mut p = make_project();

        h.push(Box::new(AppendChar('a')), &mut p).unwrap();
        h.push(Box::new(AppendChar('b')), &mut p).unwrap();
        h.undo(&mut p).unwrap();
        assert_eq!(p.metadata.name, "a");

        h.push(Box::new(AppendChar('c')), &mut p).unwrap();
        assert_eq!(p.metadata.name, "ac");

        // Three live nodes: A, B (redo branch), C.
        assert_eq!(h.node_count(), 3);
    }

    #[test]
    fn redo_follows_most_recent_branch() {
        let mut h = History::new();
        let mut p = make_project();

        h.push(Box::new(AppendChar('a')), &mut p).unwrap();
        h.push(Box::new(AppendChar('b')), &mut p).unwrap();
        h.undo(&mut p).unwrap(); // back to A
        h.push(Box::new(AppendChar('c')), &mut p).unwrap(); // branch: AC
        h.undo(&mut p).unwrap(); // back to A
        h.undo(&mut p).unwrap(); // back to root

        h.redo(&mut p).unwrap();
        assert_eq!(p.metadata.name, "a");
        // A's most-recently visited child is C (pushed after B was visited).
        h.redo(&mut p).unwrap();
        assert_eq!(p.metadata.name, "ac");
    }

    // ---- coalescing --------------------------------------------------------

    #[test]
    fn coalescing_merges_consecutive_strokes() {
        let mut h = History::new();
        let mut p = make_project();

        h.push(Box::new(CoalescingStroke::new('a')), &mut p)
            .unwrap();
        h.push(Box::new(CoalescingStroke::new('b')), &mut p)
            .unwrap();
        // Node count stays at 1 (coalesced).
        assert_eq!(h.node_count(), 1);
        assert_eq!(p.metadata.name, "a");
    }

    #[test]
    fn non_coalescible_commands_produce_separate_nodes() {
        let mut h = History::new();
        let mut p = make_project();

        h.push(Box::new(AppendChar('a')), &mut p).unwrap();
        h.push(Box::new(AppendChar('b')), &mut p).unwrap();
        assert_eq!(h.node_count(), 2);
        assert_eq!(p.metadata.name, "ab");
    }

    // ---- memory eviction ---------------------------------------------------

    #[test]
    fn memory_cap_evicts_oldest_nodes() {
        let config = HistoryConfig {
            max_commands: 3,
            max_bytes: usize::MAX,
        };
        let mut h = History::with_config(config);
        let mut p = make_project();

        for c in ['a', 'b', 'c', 'd'] {
            h.push(Box::new(AppendChar(c)), &mut p).unwrap();
        }

        // Live node count must not exceed the cap.
        assert!(
            h.node_count() <= 3,
            "expected ≤3 live nodes, got {}",
            h.node_count()
        );
        // Current path length is bounded too.
        let labels: Vec<&str> = h.labels().collect();
        assert!(
            labels.len() <= 3,
            "expected ≤3 live commands on path, got {}: {:?}",
            labels.len(),
            labels
        );
        assert_eq!(p.metadata.name, "abcd");
    }

    #[test]
    fn eviction_past_cap_keeps_node_count_bounded() {
        // Push well past max_commands and verify nodes.len() stays bounded.
        let cap = 5_usize;
        let config = HistoryConfig {
            max_commands: cap,
            max_bytes: usize::MAX,
        };
        let mut h = History::with_config(config);
        let mut p = make_project();

        for i in 0..50_u8 {
            h.push(Box::new(AppendChar(char::from(b'a' + (i % 26)))), &mut p)
                .unwrap();
            assert!(
                h.node_count() <= cap,
                "after {} pushes, node_count={} exceeds cap={}",
                i + 1,
                h.node_count(),
                cap
            );
        }
    }

    #[test]
    fn oldest_node_unreachable_after_eviction() {
        let config = HistoryConfig {
            max_commands: 2,
            max_bytes: usize::MAX,
        };
        let mut h = History::with_config(config);
        let mut p = make_project();

        // Push three commands — the first should be evicted.
        h.push(Box::new(AppendChar('a')), &mut p).unwrap();
        let first_id = 0u32; // node 0 is the first push
        h.push(Box::new(AppendChar('b')), &mut p).unwrap();
        h.push(Box::new(AppendChar('c')), &mut p).unwrap();

        // Node 0 must have been evicted from the map.
        assert!(
            !h.nodes.contains_key(&first_id),
            "evicted node 0 must not be in the map"
        );
        // Only 2 live nodes remain.
        assert_eq!(h.node_count(), 2);
    }

    // ---- labels ------------------------------------------------------------

    #[test]
    fn labels_returns_oldest_to_newest() {
        let mut h = History::new();
        let mut p = make_project();

        h.push(Box::new(AppendChar('a')), &mut p).unwrap();
        h.push(Box::new(AppendChar('b')), &mut p).unwrap();
        h.push(Box::new(AppendChar('c')), &mut p).unwrap();

        let labels: Vec<&str> = h.labels().collect();
        assert_eq!(labels, vec!["append char", "append char", "append char"]);
    }

    #[test]
    fn undo_label_shows_what_would_be_undone() {
        let mut h = History::new();
        let mut p = make_project();

        h.push(Box::new(AppendChar('a')), &mut p).unwrap();
        assert_eq!(h.undo_label(), Some("append char"));
        h.undo(&mut p).unwrap();
        assert_eq!(h.undo_label(), None);
    }
}
