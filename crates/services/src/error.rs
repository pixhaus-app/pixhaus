//! The service-layer error type.

use thiserror::Error;

use pixhaus_core::CommandError;

/// Why a service operation failed.
#[derive(Debug, Error)]
pub enum ServiceError {
    /// `undo` was called with an empty undo stack.
    #[error("nothing to undo")]
    NothingToUndo,
    /// `redo` was called with an empty redo stack.
    #[error("nothing to redo")]
    NothingToRedo,
    /// A command failed to apply or undo.
    #[error(transparent)]
    Command(#[from] CommandError),
}
