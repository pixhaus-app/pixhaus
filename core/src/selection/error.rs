//! Error types for the selection module.

use thiserror::Error;

/// Errors produced by selection operations.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum Error {
    /// Two masks had incompatible dimensions.
    #[error("mask size mismatch: expected {expected_w}x{expected_h}, got {actual_w}x{actual_h}")]
    SizeMismatch {
        /// Expected width.
        expected_w: u32,
        /// Expected height.
        expected_h: u32,
        /// Actual width.
        actual_w: u32,
        /// Actual height.
        actual_h: u32,
    },

    /// Seed point for a flood-fill was outside the buffer bounds.
    #[error("seed ({x}, {y}) is outside buffer bounds {width}x{height}")]
    SeedOutOfBounds {
        /// Seed x coordinate.
        x: u32,
        /// Seed y coordinate.
        y: u32,
        /// Buffer width.
        width: u32,
        /// Buffer height.
        height: u32,
    },

    /// `width × height` overflowed `usize`. On 64-bit hosts this only
    /// triggers near `u32::MAX` per side; on 32-bit hosts it can fire
    /// for `46_341 × 46_341` and above. Reject early so we don't panic
    /// inside `vec![0u8; …]`.
    #[error("selection dimensions {width}x{height} overflow usize")]
    DimensionOverflow {
        /// Requested width.
        width: u32,
        /// Requested height.
        height: u32,
    },
}

/// Convenience alias for results in this module.
pub type Result<T> = std::result::Result<T, Error>;
