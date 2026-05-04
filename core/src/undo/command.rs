//! The `Command` trait and coalescing protocol.

use crate::project::Project;

use super::error::CommandResult;

/// Outcome of attempting to merge two consecutive commands.
///
/// Marked `#[non_exhaustive]` so future variants (e.g. a `Failed` arm
/// for incompatible merges) can be added without breaking external
/// matches.
#[non_exhaustive]
pub enum CoalesceResult {
    /// The commands were merged. The receiver now embodies both.
    Merged,
    /// The commands could not be merged; keep both on the stack.
    Keep,
}

/// A reversible editor mutation.
///
/// Every change to a [`Project`] must be expressed as a `Command` so
/// that the undo stack can reproduce or reverse it. The command owns
/// whatever state it needs to do that — typically a "before" snapshot
/// for undo and the intended "after" for apply, or enough parameters
/// to recompute both.
///
/// # Memory accounting
///
/// `estimated_size_bytes` feeds the stack's memory cap. Return a
/// realistic estimate; an overly-small value delays eviction, an
/// overly-large one evicts too aggressively. Zero (the default) is
/// fine for parameter-only commands whose heap cost is negligible.
pub trait Command: Send + Sync + 'static {
    /// Short human-readable label shown in the history panel.
    ///
    /// Examples: `"Brush stroke"`, `"Add layer 'Foreground'"`,
    /// `"Flip horizontal"`.
    fn label(&self) -> &str;

    /// Apply the command to `project`, advancing it to the new state.
    ///
    /// # Errors
    ///
    /// Returns [`super::CommandError`] if the mutation cannot complete.
    /// On failure the project state is unspecified — the [`super::History`]
    /// becomes poisoned and rejects all subsequent operations until the
    /// caller reloads the document.
    fn apply(&mut self, project: &mut Project) -> CommandResult;

    /// Reverse the effect of the most recent `apply`, restoring
    /// `project` to the state it was in before.
    ///
    /// # Errors
    ///
    /// Same corruption caveat as `apply`: a failure here poisons the
    /// [`super::History`].
    fn undo(&mut self, project: &mut Project) -> CommandResult;

    /// Attempt to merge `next` into `self`.
    ///
    /// **Contract**: the caller has already invoked `next.apply(project)`
    /// before this method runs. The implementer's job is to update `self`'s
    /// undo state so that a future `self.undo(project)` reverses the
    /// effects of *both* the original apply (already on `self`) and the
    /// just-run apply of `next`. Return [`CoalesceResult::Merged`] if
    /// the merge happened — the stack discards `next` in that case.
    /// Return [`CoalesceResult::Keep`] to leave `next` as a separate
    /// history entry.
    ///
    /// The default never coalesces.
    #[allow(unused_variables)]
    fn coalesce(&mut self, next: &dyn Command) -> CoalesceResult {
        CoalesceResult::Keep
    }

    /// Estimated heap bytes consumed by this command.
    ///
    /// Used by the history's memory cap. The default returns `0`,
    /// which is appropriate for commands that carry only scalars.
    fn estimated_size_bytes(&self) -> usize {
        0
    }
}
