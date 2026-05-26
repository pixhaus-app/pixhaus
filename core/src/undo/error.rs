//! Error types for the undo subsystem.

use thiserror::Error;

/// The error type returned by [`super::Command::apply`] and
/// [`super::Command::undo`].
///
/// Concrete by design: the project's CLAUDE.md prohibits
/// `Box<dyn std::error::Error>` in public APIs, so commands report
/// failures as a structured payload rather than an opaque trait object.
/// Most commands construct via [`CommandError::from_message`]; richer
/// variants land here as the editor needs them (e.g. `EntityNotFound`).
#[derive(Clone, Debug, Eq, PartialEq, Error)]
#[non_exhaustive]
pub enum CommandError {
    /// Free-form description of why the command could not run.
    ///
    /// Use this when the underlying failure has no structured representation
    /// the undo subsystem cares about — e.g. a domain-specific error from
    /// the app layer that the user just needs to see in a dialog.
    #[error("{0}")]
    Other(String),
}

impl CommandError {
    /// Construct a `CommandError::Other` from any displayable value.
    ///
    /// Convenient for `.map_err(CommandError::from_message)` at boundaries
    /// where domain errors get flattened into a string for the user.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "by-value lets `.map_err(CommandError::from_message)` work without a closure; `to_string` allocates anyway"
    )]
    #[must_use]
    pub fn from_message(message: impl ToString) -> Self {
        Self::Other(message.to_string())
    }
}

/// Convenience `Result` alias for [`super::Command`] implementations.
pub type CommandResult = std::result::Result<(), CommandError>;

/// Errors that can arise from undo/redo operations.
///
/// Marked `#[non_exhaustive]` so future variants can be added without
/// breaking external matches.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// No command is available to undo.
    #[error("nothing to undo")]
    NothingToUndo,
    /// No command is available to redo.
    #[error("nothing to redo")]
    NothingToRedo,
    /// A command's `apply` or `undo` implementation returned an error.
    #[error("command '{label}' failed: {source}")]
    CommandFailed {
        /// The label of the command that failed.
        label: String,
        /// The underlying error returned by the command.
        #[source]
        source: CommandError,
    },
    /// The history exhausted its 4-billion-edit `NodeId` space.
    #[error("undo history exhausted (4 billion edits in one session)")]
    HistoryFull,
    /// A prior `apply`/`undo` failed and the history is poisoned.
    ///
    /// The target state may be inconsistent. The caller should reload the
    /// document. All `push`/`undo`/`redo` calls return this until the
    /// `History` is rebuilt.
    #[error("undo history is poisoned by a prior failure: {detail}")]
    Poisoned {
        /// Description of the original failure that poisoned the history.
        detail: String,
    },
}

/// Convenience `Result` alias for the undo subsystem.
pub type Result<T> = std::result::Result<T, Error>;
