//! Errors raised by the transforms subsystem.

use thiserror::Error;

/// Errors returned by transform operations.
#[derive(Debug, Error, PartialEq)]
pub enum Error {
    /// The source buffer has zero width or height.
    #[error("source buffer is empty (0×0)")]
    EmptyBuffer,

    /// A scale factor of zero was supplied.
    #[error("scale factor must be at least 1, got {0}")]
    InvalidScaleFactor(u32),

    /// The computed output dimensions overflowed `u32`.
    #[error("output dimensions overflow u32")]
    DimensionOverflow,

    /// Forwarded pixel-buffer allocation error.
    #[error("pixel buffer error: {0}")]
    Buffer(#[from] crate::canvas::error::Error),
}

/// `Result` alias scoped to transform errors.
pub type Result<T> = std::result::Result<T, Error>;
