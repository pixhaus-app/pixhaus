//! The branching undo/redo history tree, generic over the target `T`.
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
use std::num::NonZeroUsize;

use super::command::{CoalesceResult, Command};
use super::error::{Error, Result};

/// Opaque identifier for a history node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct NodeId(u32);

impl NodeId {
    /// First ID issued by a fresh history.
    const ZERO: Self = Self(0);

    /// Returns the next ID after `self`, or `None` at saturation.
    fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

/// Tuning parameters for the history.
#[derive(Clone, Debug)]
pub struct HistoryConfig {
    /// Maximum number of nodes retained across all branches.
    pub max_commands: NonZeroUsize,
    /// Maximum total heap bytes across all retained commands.
    pub max_bytes: NonZeroUsize,
}

impl HistoryConfig {
    /// Constructs a config from the given caps.
    #[must_use]
    pub const fn new(max_commands: NonZeroUsize, max_bytes: NonZeroUsize) -> Self {
        Self { max_commands, max_bytes }
    }
}

impl Default for HistoryConfig {
    fn default() -> Self {
        #[allow(clippy::disallowed_methods, clippy::expect_used, reason = "const-evaluated literals; cannot panic at runtime")]
        const DEFAULT_MAX_COMMANDS: NonZeroUsize = NonZeroUsize::new(500).expect("500 is non-zero");
        #[allow(clippy::disallowed_methods, clippy::expect_used, reason = "const-evaluated literals; cannot panic at runtime")]
        const DEFAULT_MAX_BYTES: NonZeroUsize = NonZeroUsize::new(500 * 1024 * 1024).expect("500 MiB is non-zero");
        Self {
            max_commands: DEFAULT_MAX_COMMANDS,
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

/// A single node in the history tree.
struct HistoryNode<T> {
    command: Box<dyn Command<T>>,
    parent: Option<NodeId>,
    /// Children ordered oldest-first; the last entry is most-recently visited.
    children: Vec<NodeId>,
    /// `command.estimated_size_bytes()` captured at insertion time.
    size_bytes: usize,
}

impl<T: 'static> HistoryNode<T> {
    fn new(command: Box<dyn Command<T>>, parent: Option<NodeId>) -> Self {
        let size_bytes = command.estimated_size_bytes();
        Self {
            command,
            parent,
            children: Vec::new(),
            size_bytes,
        }
    }
}

/// The branching undo/redo history over a target `T`.
///
/// `current` is `None` when the target is in its initial (unapplied)
/// state, i.e. every command has been undone. When `current` is `Some(id)`,
/// node `id` is the most recently applied command.
pub struct History<T> {
    nodes: HashMap<NodeId, HistoryNode<T>>,
    next_node_id: NodeId,
    current: Option<NodeId>,
    total_bytes: usize,
    config: HistoryConfig,
    poisoned: Option<String>,
}

impl<T: 'static> History<T> {
    /// Creates an empty history with the given configuration.
    #[must_use]
    pub fn with_config(config: HistoryConfig) -> Self {
        Self {
            nodes: HashMap::new(),
            next_node_id: NodeId::ZERO,
            current: None,
            total_bytes: 0,
            config,
            poisoned: None,
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

    /// Whether an undo is available.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.current.is_some() && self.poisoned.is_none()
    }

    /// Whether a redo is available.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.poisoned.is_none() && self.next_redo_id().is_some()
    }

    /// Label of the command that would be undone next, if any.
    #[must_use]
    pub fn undo_label(&self) -> Option<&str> {
        self.current.and_then(|id| self.nodes.get(&id)).map(|n| n.command.label())
    }

    /// Label of the command that would be redone next, if any.
    #[must_use]
    pub fn redo_label(&self) -> Option<&str> {
        self.next_redo_id().and_then(|id| self.nodes.get(&id)).map(|n| n.command.label())
    }

    /// Apply `command` against `target` and push it onto the history.
    ///
    /// # Errors
    ///
    /// - [`Error::Poisoned`] if the history was previously poisoned.
    /// - [`Error::HistoryFull`] if 4 billion edits have accumulated.
    /// - [`Error::CommandFailed`] propagated from `command.apply`.
    pub fn push(&mut self, mut command: Box<dyn Command<T>>, target: &mut T) -> Result<()> {
        self.check_not_poisoned()?;

        let reserved_next = self.next_node_id.next().ok_or(Error::HistoryFull)?;

        if let Err(source) = command.apply(target) {
            let label = command.label().to_owned();
            self.poisoned = Some(format!("apply '{label}' failed: {source}"));
            return Err(Error::CommandFailed { label, source });
        }

        if let Some(cur) = self.current
            && let Some(node) = self.nodes.get_mut(&cur)
            && matches!(node.command.coalesce(command.as_ref()), CoalesceResult::Merged)
        {
            return Ok(());
        }

        let parent = self.current;
        let node_id = self.next_node_id;
        self.next_node_id = reserved_next;
        let node = HistoryNode::new(command, parent);
        self.total_bytes = self.total_bytes.saturating_add(node.size_bytes);
        self.nodes.insert(node_id, node);

        if let Some(p) = parent
            && let Some(parent_node) = self.nodes.get_mut(&p)
        {
            parent_node.children.push(node_id);
        }

        self.current = Some(node_id);
        self.enforce_cap();
        Ok(())
    }

    fn check_not_poisoned(&self) -> Result<()> {
        match &self.poisoned {
            Some(detail) => Err(Error::Poisoned { detail: detail.clone() }),
            None => Ok(()),
        }
    }

    /// Undo the current command, moving `current` to its parent.
    ///
    /// # Errors
    ///
    /// - [`Error::Poisoned`] if a prior failure poisoned the history.
    /// - [`Error::NothingToUndo`] if already at root.
    /// - [`Error::CommandFailed`] propagated from `Command::undo`.
    pub fn undo(&mut self, target: &mut T) -> Result<()> {
        self.check_not_poisoned()?;
        let cur = self.current.ok_or(Error::NothingToUndo)?;
        let (label, parent) = self
            .nodes
            .get(&cur)
            .map(|n| (n.command.label().to_owned(), n.parent))
            .ok_or(Error::NothingToUndo)?;

        if let Err(source) = self.nodes.get_mut(&cur).ok_or(Error::NothingToUndo)?.command.undo(target) {
            self.poisoned = Some(format!("undo '{label}' failed: {source}"));
            return Err(Error::CommandFailed { label, source });
        }

        self.current = parent;
        Ok(())
    }

    /// Redo the most-recently-visited child of `current`.
    ///
    /// # Errors
    ///
    /// - [`Error::Poisoned`] if a prior failure poisoned the history.
    /// - [`Error::NothingToRedo`] if no child exists.
    /// - [`Error::CommandFailed`] propagated from `Command::apply`.
    pub fn redo(&mut self, target: &mut T) -> Result<()> {
        self.check_not_poisoned()?;
        let next = self.next_redo_id().ok_or(Error::NothingToRedo)?;
        let label = self.nodes.get(&next).map(|n| n.command.label().to_owned()).ok_or(Error::NothingToRedo)?;

        if let Err(source) = self.nodes.get_mut(&next).ok_or(Error::NothingToRedo)?.command.apply(target) {
            self.poisoned = Some(format!("redo '{label}' failed: {source}"));
            return Err(Error::CommandFailed { label, source });
        }

        if let Some(p) = self.nodes.get(&next).and_then(|n| n.parent)
            && let Some(parent_node) = self.nodes.get_mut(&p)
            && let Some(pos) = parent_node.children.iter().position(|&c| c == next)
        {
            parent_node.children.remove(pos);
            parent_node.children.push(next);
        }
        self.current = Some(next);
        Ok(())
    }

    fn next_redo_id(&self) -> Option<NodeId> {
        match self.current {
            Some(id) => self.nodes.get(&id)?.children.last().copied(),
            None => self.nodes.iter().filter(|(_, n)| n.parent.is_none()).map(|(&id, _)| id).max(),
        }
    }

    fn enforce_cap(&mut self) {
        loop {
            if self.nodes.len() <= self.config.max_commands.get() && self.total_bytes <= self.config.max_bytes.get() {
                break;
            }

            let path = self.current_path();
            if path.len() < 2 {
                break;
            }

            let oldest = path[0];
            let keep = path[1];

            let off_path: Vec<NodeId> = self
                .nodes
                .get(&oldest)
                .map(|n| n.children.iter().filter(|&&c| c != keep).copied().collect())
                .unwrap_or_default();
            for child in off_path {
                self.drop_subtree(child);
            }

            if let Some(node) = self.nodes.get_mut(&keep) {
                node.parent = None;
            }

            if let Some(node) = self.nodes.remove(&oldest) {
                self.total_bytes = self.total_bytes.saturating_sub(node.size_bytes);
            }
        }
    }

    fn drop_subtree(&mut self, id: NodeId) {
        debug_assert!(self.current != Some(id), "drop_subtree must not evict the current node (id={id:?})");
        let Some(node) = self.nodes.remove(&id) else {
            return;
        };
        self.total_bytes = self.total_bytes.saturating_sub(node.size_bytes);

        for child in node.children {
            self.drop_subtree(child);
        }
    }

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
        self.current_path().into_iter().filter_map(|id| self.nodes.get(&id)).map(|n| n.command.label())
    }
}

impl<T: 'static> Default for History<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Project;

    use super::super::error::CommandError;
    type CmdResult = super::super::error::CommandResult;

    /// A command that appends a char to `project.metadata.name` on apply
    /// and removes it on undo.
    struct AppendChar(char);

    impl Command<Project> for AppendChar {
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

    struct CoalescingStroke {
        chars: Vec<char>,
    }

    impl CoalescingStroke {
        fn new(c: char) -> Self {
            Self { chars: vec![c] }
        }
    }

    impl Command<Project> for CoalescingStroke {
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

        fn coalesce(&mut self, next: &dyn Command<Project>) -> CoalesceResult {
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

    #[test]
    fn push_applies_and_advances_current() {
        let mut h = History::<Project>::new();
        let mut p = make_project();
        h.push(Box::new(AppendChar('a')), &mut p).unwrap();
        assert_eq!(p.metadata.name, "a");
        assert_eq!(h.current, Some(NodeId(0)));
    }

    #[test]
    fn undo_reverses_command() {
        let mut h = History::<Project>::new();
        let mut p = make_project();
        h.push(Box::new(AppendChar('a')), &mut p).unwrap();
        h.undo(&mut p).unwrap();
        assert_eq!(p.metadata.name, "");
        assert_eq!(h.current, None);
    }

    #[test]
    fn redo_reapplies_command() {
        let mut h = History::<Project>::new();
        let mut p = make_project();
        h.push(Box::new(AppendChar('a')), &mut p).unwrap();
        h.undo(&mut p).unwrap();
        h.redo(&mut p).unwrap();
        assert_eq!(p.metadata.name, "a");
        assert_eq!(h.current, Some(NodeId(0)));
    }

    #[test]
    fn undo_at_root_returns_error() {
        let mut h = History::<Project>::new();
        let mut p = make_project();
        assert!(matches!(h.undo(&mut p), Err(Error::NothingToUndo)));
    }

    #[test]
    fn redo_at_leaf_returns_error() {
        let mut h = History::<Project>::new();
        let mut p = make_project();
        h.push(Box::new(AppendChar('a')), &mut p).unwrap();
        assert!(matches!(h.redo(&mut p), Err(Error::NothingToRedo)));
    }

    #[test]
    fn branching_preserves_redo_branch() {
        let mut h = History::<Project>::new();
        let mut p = make_project();

        h.push(Box::new(AppendChar('a')), &mut p).unwrap();
        h.push(Box::new(AppendChar('b')), &mut p).unwrap();
        h.undo(&mut p).unwrap();
        assert_eq!(p.metadata.name, "a");

        h.push(Box::new(AppendChar('c')), &mut p).unwrap();
        assert_eq!(p.metadata.name, "ac");

        assert_eq!(h.node_count(), 3);
    }

    #[test]
    fn coalescing_merges_consecutive_strokes() {
        let mut h = History::<Project>::new();
        let mut p = make_project();

        h.push(Box::new(CoalescingStroke::new('a')), &mut p).unwrap();
        h.push(Box::new(CoalescingStroke::new('b')), &mut p).unwrap();
        assert_eq!(h.node_count(), 1);
        assert_eq!(p.metadata.name, "ab");
    }

    #[test]
    fn undo_after_coalesced_pushes_reverses_all_applied_effects() {
        let mut h = History::<Project>::new();
        let mut p = make_project();
        p.metadata.name = "base".into();

        h.push(Box::new(CoalescingStroke::new('a')), &mut p).unwrap();
        h.push(Box::new(CoalescingStroke::new('b')), &mut p).unwrap();
        h.push(Box::new(CoalescingStroke::new('c')), &mut p).unwrap();
        assert_eq!(p.metadata.name, "baseabc");

        h.undo(&mut p).unwrap();
        assert_eq!(p.metadata.name, "base");
    }

    fn nz(n: usize) -> NonZeroUsize {
        NonZeroUsize::new(n).unwrap()
    }

    #[test]
    fn memory_cap_evicts_oldest_nodes() {
        let config = HistoryConfig::new(nz(3), nz(usize::MAX));
        let mut h = History::<Project>::with_config(config);
        let mut p = make_project();

        for c in ['a', 'b', 'c', 'd'] {
            h.push(Box::new(AppendChar(c)), &mut p).unwrap();
        }

        assert!(h.node_count() <= 3);
        assert_eq!(p.metadata.name, "abcd");
    }

    /// A command whose `apply` always fails; used to trigger poisoning.
    struct FailingApply;
    impl Command<Project> for FailingApply {
        fn label(&self) -> &'static str {
            "failing apply"
        }
        fn apply(&mut self, _project: &mut Project) -> CmdResult {
            Err(CommandError::Other("simulated failure".into()))
        }
        fn undo(&mut self, _project: &mut Project) -> CmdResult {
            Ok(())
        }
    }

    #[test]
    fn failed_push_poisons_history_and_blocks_subsequent_ops() {
        let mut h = History::<Project>::new();
        let mut p = make_project();

        let result = h.push(Box::new(FailingApply), &mut p);
        assert!(matches!(result, Err(Error::CommandFailed { .. })));

        assert!(matches!(h.push(Box::new(AppendChar('a')), &mut p), Err(Error::Poisoned { .. })));
        assert!(matches!(h.undo(&mut p), Err(Error::Poisoned { .. })));
        assert!(matches!(h.redo(&mut p), Err(Error::Poisoned { .. })));
    }

    #[test]
    fn undo_label_shows_what_would_be_undone() {
        let mut h = History::<Project>::new();
        let mut p = make_project();

        h.push(Box::new(AppendChar('a')), &mut p).unwrap();
        assert_eq!(h.undo_label(), Some("append char"));
        assert!(h.can_undo());
        h.undo(&mut p).unwrap();
        assert_eq!(h.undo_label(), None);
        assert!(!h.can_undo());
        assert!(h.can_redo());
    }
}
