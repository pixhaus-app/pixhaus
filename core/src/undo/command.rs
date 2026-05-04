//! The `Command` trait and coalescing protocol.

use crate::project::Project;

/// Outcome of attempting to merge two consecutive commands.
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
    /// Returns an error if the mutation cannot complete. The project
    /// state after a failed `apply` is unspecified; callers should
    /// treat it as corrupted and surface the error to the user.
    fn apply(
        &mut self,
        project: &mut Project,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Reverse the effect of the most recent `apply`, restoring
    /// `project` to the state it was in before.
    ///
    /// # Errors
    ///
    /// Returns an error if the reversal cannot complete. Same
    /// corruption caveat as `apply`.
    fn undo(
        &mut self,
        project: &mut Project,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Attempt to merge `next` into `self`.
    ///
    /// Called when `next` is pushed onto the stack and the top command
    /// is `self`. If the two commands represent the same logical action
    /// (e.g. two consecutive brush ticks in the same stroke), merging
    /// them produces a single history entry. Return [`CoalesceResult::Merged`]
    /// if the merge happened; the stack discards `next` in that case.
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
