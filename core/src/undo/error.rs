//! Error types for the undo subsystem.

use thiserror::Error;

/// Errors that can arise from undo/redo operations.
#[derive(Debug, Error)]
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
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

/// Convenience `Result` alias for the undo subsystem.
pub type Result<T> = std::result::Result<T, Error>;
