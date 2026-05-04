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
//! Nodes are stored in a flat `Vec`; each node carries its parent index
//! and a list of child indices. This lets the tree avoid pointer chasing
//! while keeping append O(1) amortised.
//!
//! # Memory cap
//!
//! The cap is enforced on push. When the total command count exceeds
//! `max_commands`, or the summed `estimated_size_bytes` across all live
//! nodes exceeds `max_bytes`, the oldest reachable ancestor of the root
//! branch is dropped. "Oldest" means the node with the smallest index
//! that is still on the undo path from `current` to the root.

use crate::project::Project;

use super::command::{CoalesceResult, Command};
use super::error::{Error, Result};

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
    parent: Option<usize>,
    /// Children ordered oldest-first; the last entry is most-recently visited.
    children: Vec<usize>,
}

impl HistoryNode {
    fn new(command: Box<dyn Command>, parent: Option<usize>) -> Self {
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
/// state, i.e. every command has been undone. When `current` is `Some(i)`,
/// node `i` is the most recently applied command.
pub struct History {
    nodes: Vec<HistoryNode>,
    /// Index of the most recently applied command, or `None` at root.
    current: Option<usize>,
    /// Summed `estimated_size_bytes` across all live nodes.
    total_bytes: usize,
    config: HistoryConfig,
}

impl History {
    /// Creates an empty history with the given configuration.
    #[must_use]
    pub fn with_config(config: HistoryConfig) -> Self {
        Self {
            nodes: Vec::new(),
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

    /// Number of nodes currently in the tree (across all branches).
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Label of the command that would be undone next, if any.
    #[must_use]
    pub fn undo_label(&self) -> Option<&str> {
        self.current.map(|i| self.nodes[i].command.label())
    }

    /// Label of the command that would be redone next, if any.
    ///
    /// Redo follows the most-recently-visited child of `current`.
    #[must_use]
    pub fn redo_label(&self) -> Option<&str> {
        let children = match self.current {
            Some(i) => &self.nodes[i].children,
            None => {
                // At root: the next redo target is the first root-level node
                // that has no parent. We track root nodes as children of a
                // virtual sentinel — here we scan for nodes with parent=None.
                return self
                    .nodes
                    .iter()
                    .rev()
                    .find(|n| n.parent.is_none())
                    .map(|n| n.command.label());
            }
        };
        children.last().map(|&ci| self.nodes[ci].command.label())
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
            if let CoalesceResult::Merged = self.nodes[cur].command.coalesce(command.as_ref()) {
                // Merged: the current command now represents both; nothing to push.
                return Ok(());
            }
        }

        command
            .apply(project)
            .map_err(|source| Error::CommandFailed {
                label: command.label().to_owned(),
                source,
            })?;

        let parent = self.current;
        let node_idx = self.nodes.len();
        let size = command.estimated_size_bytes();

        let node = HistoryNode::new(command, parent);
        self.nodes.push(node);
        self.total_bytes += size;

        // Register as a child of the current node.
        if let Some(p) = parent {
            self.nodes[p].children.push(node_idx);
        }

        self.current = Some(node_idx);
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
        self.nodes[cur]
            .command
            .undo(project)
            .map_err(|source| Error::CommandFailed {
                label: self.nodes[cur].command.label().to_owned(),
                source,
            })?;
        self.current = self.nodes[cur].parent;
        Ok(())
    }

    /// Redo the most-recently-visited child of `current`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NothingToRedo`] if no child exists.
    /// Propagates any error from the command's `apply` implementation.
    pub fn redo(&mut self, project: &mut Project) -> Result<()> {
        let next = self.next_redo_index().ok_or(Error::NothingToRedo)?;
        self.nodes[next]
            .command
            .apply(project)
            .map_err(|source| Error::CommandFailed {
                label: self.nodes[next].command.label().to_owned(),
                source,
            })?;
        // Move the redone child to the end of its parent's child list so
        // subsequent redos continue down this branch.
        if let Some(p) = self.nodes[next].parent {
            let children = &mut self.nodes[p].children;
            if let Some(pos) = children.iter().position(|&c| c == next) {
                children.remove(pos);
                children.push(next);
            }
        }
        self.current = Some(next);
        Ok(())
    }

    /// Returns the index of the next node to redo, if any.
    fn next_redo_index(&self) -> Option<usize> {
        match self.current {
            Some(i) => self.nodes[i].children.last().copied(),
            None => {
                // At root: find root nodes (parent == None) in insertion order.
                // The most-recently visited root node is what we want.
                // We look for root nodes and take the last one (most recent push).
                self.nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| n.parent.is_none())
                    .map(|(i, _)| i)
                    .next_back()
            }
        }
    }

    /// Drops the oldest nodes on the linear ancestor chain from the
    /// current position back to the root until both caps are satisfied.
    ///
    /// Nodes that are NOT on the current undo path are orphaned when
    /// their parent is dropped; we drop them recursively. This is a
    /// simple eager strategy — it keeps the tree bounded without a
    /// separate GC pass.
    fn enforce_cap(&mut self) {
        while self.nodes.len() > self.config.max_commands
            || self.total_bytes > self.config.max_bytes
        {
            // Collect the root-to-current path.
            let path = self.current_path();
            if path.len() < 2 {
                // Only one node exists; can't drop it while it's current.
                break;
            }
            // The oldest node on the path (index 0 is root).
            let oldest = path[0];
            self.drop_node(oldest);
            // After dropping the oldest, update `current` if needed — it
            // can't point to a dropped node, but the path guaranteed
            // `current` != `oldest`.
        }
    }

    /// Returns the path from the root ancestor of `current` down to
    /// `current`, as a Vec of node indices oldest-first.
    fn current_path(&self) -> Vec<usize> {
        let mut path = Vec::new();
        let mut idx = self.current;
        while let Some(i) = idx {
            path.push(i);
            idx = self.nodes[i].parent;
        }
        path.reverse();
        path
    }

    /// Drop node at `idx` and recursively orphan all its children.
    ///
    /// Also removes the node from its parent's child list.
    fn drop_node(&mut self, idx: usize) {
        // Collect children before mutating.
        let children: Vec<usize> = self.nodes[idx].children.clone();
        let parent = self.nodes[idx].parent;
        let size = self.nodes[idx].command.estimated_size_bytes();

        // Remove from parent's child list.
        if let Some(p) = parent {
            self.nodes[p].children.retain(|&c| c != idx);
        }

        // Subtract size; replace the node with a tombstone (we keep the
        // slot to avoid re-indexing — indices must stay stable).
        self.total_bytes = self.total_bytes.saturating_sub(size);

        // Recursively drop children.
        for child in children {
            self.drop_node(child);
        }

        // Update `current` if it pointed at this node (shouldn't happen
        // during enforce_cap, but defensive).
        if self.current == Some(idx) {
            self.current = parent;
        }

        // We leave the node in the Vec as a tombstone so indices stay
        // valid. Replace with a no-op slot that consumes no memory.
        // `HistoryNode` is not Clone, so we swap with a dummy.
        self.nodes[idx] = HistoryNode::new(Box::new(TombstoneCommand), None);
    }

    /// Returns an ordered iterator over labels from oldest to newest
    /// along the current undo path.
    pub fn labels(&self) -> impl Iterator<Item = &str> {
        self.current_path()
            .into_iter()
            .map(|i| self.nodes[i].command.label())
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal no-op command used as a tombstone when a node is evicted.
struct TombstoneCommand;

impl Command for TombstoneCommand {
    fn label(&self) -> &'static str {
        "<evicted>"
    }

    fn apply(
        &mut self,
        _project: &mut Project,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn undo(
        &mut self,
        _project: &mut Project,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Project;

    // Shadow the undo-specific `Result` so test command impls can use
    // the trait's expected return type without spelling out the full path.
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
            // Accept any other CoalescingStroke (identified by label).
            if next.label() == "stroke" {
                // We can't downcast through `dyn Command`, so we push a
                // synthetic char 'x' to represent the merged tick.
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

        // Push A, B.
        h.push(Box::new(AppendChar('a')), &mut p).unwrap();
        h.push(Box::new(AppendChar('b')), &mut p).unwrap();
        // Undo B — we're back at A.
        h.undo(&mut p).unwrap();
        assert_eq!(p.metadata.name, "a");

        // Push C — creates a new branch. B is now a redo branch.
        h.push(Box::new(AppendChar('c')), &mut p).unwrap();
        assert_eq!(p.metadata.name, "ac");

        // Node count: root A (0), B (1), C (2) — three nodes.
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

        // Redo takes the most-recently visited child of root, which is A.
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

        // Push first stroke — applies 'a'.
        h.push(Box::new(CoalescingStroke::new('a')), &mut p)
            .unwrap();
        // Push second stroke — should coalesce, not create a new node.
        h.push(Box::new(CoalescingStroke::new('b')), &mut p)
            .unwrap();
        // Node count stays at 1 (coalesced).
        assert_eq!(h.node_count(), 1);
        // The coalesced stroke applied 'x' (representative char) extra.
        // After apply of stroke 1: "a". After coalesce of stroke 2: no new apply.
        // (Coalescing only updates the receiver, not re-applies.)
        // The name should still be "a" because coalesce doesn't call apply again.
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

        // Push 4 commands — the cap is 3, so the first should be evicted.
        for c in ['a', 'b', 'c', 'd'] {
            h.push(Box::new(AppendChar(c)), &mut p).unwrap();
        }
        // Node count includes tombstone slots; current path should have ≤ 3
        // live commands on it.
        let labels: Vec<&str> = h.labels().collect();
        assert!(
            labels.len() <= 3,
            "expected ≤3 live commands on path, got {}: {:?}",
            labels.len(),
            labels
        );
        assert_eq!(p.metadata.name, "abcd");
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
