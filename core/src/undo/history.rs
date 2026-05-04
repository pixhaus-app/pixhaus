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
use std::num::NonZeroUsize;

use crate::project::Project;

use super::command::{CoalesceResult, Command};
use super::error::{Error, Result};

/// Opaque identifier for a history node.
///
/// Newtype around [`u32`] so it cannot be confused with any other count
/// or index in the codebase. IDs are monotonically increasing and never
/// reused; the underlying space is 2^32 ≈ 4 billion edits per session,
/// which `History::push` defends against via [`Error::HistoryFull`].
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
///
/// Both caps are `NonZeroUsize`: zero would mean "evict everything on
/// push", which the data structure can't honour (the just-pushed node
/// has nothing to evict to). Use [`HistoryConfig::new`] or
/// [`HistoryConfig::default`].
#[derive(Clone, Debug)]
pub struct HistoryConfig {
    /// Maximum number of nodes retained across all branches.
    pub max_commands: NonZeroUsize,
    /// Maximum total heap bytes across all retained commands.
    pub max_bytes: NonZeroUsize,
}

impl HistoryConfig {
    /// Constructs a config from the given caps.
    ///
    /// Both caps are typed as [`NonZeroUsize`] so a `0` cap is rejected at
    /// the type level — no silent disable. Use the literal constructors
    /// (`NonZeroUsize::new(n).expect("...")`) at call sites or
    /// [`HistoryConfig::default`] for sane defaults.
    #[must_use]
    pub const fn new(max_commands: NonZeroUsize, max_bytes: NonZeroUsize) -> Self {
        Self {
            max_commands,
            max_bytes,
        }
    }
}

impl Default for HistoryConfig {
    fn default() -> Self {
        // Compile-time `const` blocks force the `Option::expect` to be
        // evaluated by the compiler against literal non-zero values, so
        // a panic here would be a build error, not a runtime risk. The
        // local `#[allow]` is scoped to this single function for that
        // reason.
        #[allow(
            clippy::disallowed_methods,
            clippy::expect_used,
            reason = "const-evaluated literals; cannot panic at runtime"
        )]
        const DEFAULT_MAX_COMMANDS: NonZeroUsize = NonZeroUsize::new(500).expect("500 is non-zero");
        #[allow(
            clippy::disallowed_methods,
            clippy::expect_used,
            reason = "const-evaluated literals; cannot panic at runtime"
        )]
        const DEFAULT_MAX_BYTES: NonZeroUsize =
            NonZeroUsize::new(500 * 1024 * 1024).expect("500 MiB is non-zero");
        Self {
            max_commands: DEFAULT_MAX_COMMANDS,
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

/// A single node in the history tree.
struct HistoryNode {
    command: Box<dyn Command>,
    parent: Option<NodeId>,
    /// Children ordered oldest-first; the last entry is most-recently visited.
    children: Vec<NodeId>,
    /// `command.estimated_size_bytes()` captured at insertion time.
    ///
    /// Cached so eviction's `total_bytes` accounting can't drift if a
    /// command's size value changes between insertion and eviction (which
    /// would be a misbehaving `Command` impl, but harmless to defend against).
    size_bytes: usize,
}

impl HistoryNode {
    fn new(command: Box<dyn Command>, parent: Option<NodeId>) -> Self {
        let size_bytes = command.estimated_size_bytes();
        Self {
            command,
            parent,
            children: Vec::new(),
            size_bytes,
        }
    }
}

/// The branching undo/redo history.
///
/// `current` is `None` when the project is in its initial (unapplied)
/// state, i.e. every command has been undone. When `current` is `Some(id)`,
/// node `id` is the most recently applied command.
///
/// The history can become **poisoned** if a `Command::apply` or `undo`
/// implementation returns an error mid-mutation: at that point the project
/// state is unspecified and further history operations would compound the
/// damage. While poisoned every `push`/`undo`/`redo` returns
/// [`Error::Poisoned`]; the caller should reload the project to recover.
pub struct History {
    nodes: HashMap<NodeId, HistoryNode>,
    /// Next ID to assign. Monotonically increasing; never reused.
    next_node_id: NodeId,
    /// ID of the most recently applied command, or `None` at root.
    current: Option<NodeId>,
    /// Summed `size_bytes` across all live nodes.
    total_bytes: usize,
    config: HistoryConfig,
    /// Set when a command's `apply` or `undo` failed mid-mutation.
    ///
    /// While `Some(_)`, `push`/`undo`/`redo` reject with [`Error::Poisoned`].
    /// Cleared only by constructing a fresh `History`.
    poisoned: Option<String>,
}

impl History {
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
    /// **Protocol**: `apply` runs *first*, advancing the project. Then,
    /// if the previous top command's [`Command::coalesce`] returns
    /// [`CoalesceResult::Merged`], the just-applied command is dropped
    /// from the history (the merged-into command's undo state must
    /// already cover both effects after coalesce returns). If coalesce
    /// returns [`CoalesceResult::Keep`], the command is recorded as a
    /// new history node.
    ///
    /// # Errors
    ///
    /// - [`Error::Poisoned`] if the history was previously poisoned by
    ///   a failed `apply`/`undo`. Reload the project to recover.
    /// - [`Error::HistoryFull`] if 4 billion edits have accumulated in
    ///   this session.
    /// - [`Error::CommandFailed`] propagated from `command.apply`. On a
    ///   failed apply the project state is unspecified and the history
    ///   becomes [`Error::Poisoned`] for all subsequent operations.
    pub fn push(&mut self, mut command: Box<dyn Command>, project: &mut Project) -> Result<()> {
        self.check_not_poisoned()?;

        // Reserve a NodeId up front so we surface HistoryFull *before*
        // mutating the project. If 2^32-1 edits have already happened in
        // one session, fail loud rather than wrap and silently overwrite
        // node 0.
        let reserved_next = self.next_node_id.next().ok_or(Error::HistoryFull)?;

        // Apply first — the project state advances regardless of whether
        // the command coalesces with the current top. coalesce is then a
        // pure undo-state-merge operation and never silently skips work.
        if let Err(source) = command.apply(project) {
            let label = command.label().to_owned();
            self.poisoned = Some(format!("apply '{label}' failed: {source}"));
            return Err(Error::CommandFailed { label, source });
        }

        // Offer the freshly-applied command to the current top for
        // coalescing. If accepted, the merged-into command is responsible
        // for absorbing the new command's undo state; we drop the new
        // command without recording a new node.
        if let Some(cur) = self.current
            && let Some(node) = self.nodes.get_mut(&cur)
            && matches!(
                node.command.coalesce(command.as_ref()),
                CoalesceResult::Merged
            )
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

    /// Returns `Err(Error::Poisoned { detail })` if the history is
    /// poisoned, otherwise `Ok(())`.
    fn check_not_poisoned(&self) -> Result<()> {
        match &self.poisoned {
            Some(detail) => Err(Error::Poisoned {
                detail: detail.clone(),
            }),
            None => Ok(()),
        }
    }

    /// Undo the current command, moving `current` to its parent.
    ///
    /// # Errors
    ///
    /// - [`Error::Poisoned`] if a prior failure poisoned the history.
    /// - [`Error::NothingToUndo`] if already at root.
    /// - [`Error::CommandFailed`] propagated from `Command::undo`. On
    ///   failure the history becomes poisoned.
    pub fn undo(&mut self, project: &mut Project) -> Result<()> {
        self.check_not_poisoned()?;
        let cur = self.current.ok_or(Error::NothingToUndo)?;
        let (label, parent) = self
            .nodes
            .get(&cur)
            .map(|n| (n.command.label().to_owned(), n.parent))
            .ok_or(Error::NothingToUndo)?;

        if let Err(source) = self
            .nodes
            .get_mut(&cur)
            .ok_or(Error::NothingToUndo)?
            .command
            .undo(project)
        {
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
    /// - [`Error::CommandFailed`] propagated from `Command::apply`. On
    ///   failure the history becomes poisoned.
    pub fn redo(&mut self, project: &mut Project) -> Result<()> {
        self.check_not_poisoned()?;
        let next = self.next_redo_id().ok_or(Error::NothingToRedo)?;
        let label = self
            .nodes
            .get(&next)
            .map(|n| n.command.label().to_owned())
            .ok_or(Error::NothingToRedo)?;

        if let Err(source) = self
            .nodes
            .get_mut(&next)
            .ok_or(Error::NothingToRedo)?
            .command
            .apply(project)
        {
            self.poisoned = Some(format!("redo '{label}' failed: {source}"));
            return Err(Error::CommandFailed { label, source });
        }

        // Move the redone child to the end of its parent's child list so
        // subsequent redos continue down this branch.
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
            if self.nodes.len() <= self.config.max_commands.get()
                && self.total_bytes <= self.config.max_bytes.get()
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

            // Remove `oldest` using its cached size_bytes (set at insertion);
            // never re-call estimated_size_bytes on the trait object so a
            // misbehaving Command can't drift the total_bytes accounting.
            if let Some(node) = self.nodes.remove(&oldest) {
                self.total_bytes = self.total_bytes.saturating_sub(node.size_bytes);
            }
        }
    }

    /// Recursively drop an entire subtree rooted at `id`.
    ///
    /// Does not touch the parent's child list — the caller is responsible
    /// for unlinking `id` from its parent before calling this.
    ///
    /// `id` must not be on the current undo path. `enforce_cap` only ever
    /// passes off-path subtree roots here, so dropping `current` is a bug
    /// in the caller. The debug-assert turns silent state corruption into
    /// a loud test failure.
    fn drop_subtree(&mut self, id: NodeId) {
        debug_assert!(
            self.current != Some(id),
            "drop_subtree must not evict the current node (id={id:?})"
        );
        let Some(node) = self.nodes.remove(&id) else {
            return;
        };
        self.total_bytes = self.total_bytes.saturating_sub(node.size_bytes);

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

    use super::super::error::CommandError;
    type CmdResult = super::super::error::CommandResult;

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
        assert_eq!(h.current, Some(NodeId(0)));
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
        assert_eq!(h.current, Some(NodeId(0)));
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
        // Node count stays at 1 (the second push coalesced into the first).
        assert_eq!(h.node_count(), 1);
        // Project state has BOTH strokes applied: 'a' from the first push,
        // 'b' from the second (the apply runs before coalesce).
        assert_eq!(p.metadata.name, "ab");
    }

    #[test]
    fn undo_after_coalesced_pushes_reverses_all_applied_effects() {
        // Regression for the apply-then-coalesce contract: after N
        // coalesced pushes, undoing the merged node must restore the
        // pre-first-push state. If apply were skipped on coalesce, the
        // merged node's undo would over-pop, corrupting state.
        let mut h = History::new();
        let mut p = make_project();
        p.metadata.name = "base".into();

        h.push(Box::new(CoalescingStroke::new('a')), &mut p)
            .unwrap();
        h.push(Box::new(CoalescingStroke::new('b')), &mut p)
            .unwrap();
        h.push(Box::new(CoalescingStroke::new('c')), &mut p)
            .unwrap();
        assert_eq!(p.metadata.name, "baseabc");

        h.undo(&mut p).unwrap();
        assert_eq!(
            p.metadata.name, "base",
            "merged undo must reverse every applied effect"
        );
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

    fn nz(n: usize) -> NonZeroUsize {
        NonZeroUsize::new(n).unwrap()
    }

    #[test]
    fn memory_cap_evicts_oldest_nodes() {
        let config = HistoryConfig::new(nz(3), nz(usize::MAX));
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
        let cap = nz(5);
        let config = HistoryConfig::new(cap, nz(usize::MAX));
        let mut h = History::with_config(config);
        let mut p = make_project();

        for i in 0..50_u8 {
            h.push(Box::new(AppendChar(char::from(b'a' + (i % 26)))), &mut p)
                .unwrap();
            assert!(
                h.node_count() <= cap.get(),
                "after {} pushes, node_count={} exceeds cap={}",
                i + 1,
                h.node_count(),
                cap.get()
            );
        }
    }

    #[test]
    fn oldest_node_unreachable_after_eviction() {
        let config = HistoryConfig::new(nz(2), nz(usize::MAX));
        let mut h = History::with_config(config);
        let mut p = make_project();

        // Push three commands — the first should be evicted.
        h.push(Box::new(AppendChar('a')), &mut p).unwrap();
        let first_id = NodeId(0); // node 0 is the first push
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

    // ---- poisoning ---------------------------------------------------------

    /// A command whose `apply` always fails; used to trigger poisoning.
    struct FailingApply;
    impl Command for FailingApply {
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
        let mut h = History::new();
        let mut p = make_project();

        let result = h.push(Box::new(FailingApply), &mut p);
        assert!(matches!(result, Err(Error::CommandFailed { .. })));

        // All subsequent ops must report Poisoned, not silently succeed.
        assert!(matches!(
            h.push(Box::new(AppendChar('a')), &mut p),
            Err(Error::Poisoned { .. })
        ));
        assert!(matches!(h.undo(&mut p), Err(Error::Poisoned { .. })));
        assert!(matches!(h.redo(&mut p), Err(Error::Poisoned { .. })));
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
