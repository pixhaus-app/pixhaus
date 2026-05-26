//! Errors raised by the canvas crate's pixel-buffer and compositor.

use thiserror::Error;

/// Errors returned by the canvas subsystem.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    /// A buffer constructor was given a `pixels` slice whose length
    /// does not match `stride * height`.
    #[error("byte length mismatch: expected stride * height = {expected} bytes, got {actual} bytes")]
    ByteLengthMismatch {
        /// `stride * height` derived from the requested dimensions.
        expected: usize,
        /// Length of the `pixels` slice that was passed in.
        actual: usize,
    },

    /// A buffer constructor was given a `stride` smaller than
    /// `width * bytes_per_pixel`. Stride may *exceed* the row width
    /// for SIMD padding, but it must always cover one full row.
    #[error("stride {stride} is shorter than minimum row width {min_stride}")]
    StrideTooSmall {
        /// Stride supplied by the caller.
        stride: u32,
        /// Required row width in bytes (`width * bytes_per_pixel`).
        min_stride: u32,
    },

    /// A composite operation was asked to merge buffers whose
    /// dimensions disagree. The compositor never grows or crops
    /// implicitly; the caller is expected to align layers up front.
    #[error("dimension mismatch: backdrop is {bw}x{bh}, source is {sw}x{sh}")]
    DimensionMismatch {
        /// Backdrop width.
        bw: u32,
        /// Backdrop height.
        bh: u32,
        /// Source width.
        sw: u32,
        /// Source height.
        sh: u32,
    },

    /// Resolving an indexed buffer hit an index outside the palette.
    /// Indexed sprites that do not match their palette are corrupt;
    /// the editor surfaces this rather than silently blanking pixels.
    #[error("palette index {index} out of bounds (palette has {palette_len} entries)")]
    PaletteIndexOutOfRange {
        /// The offending index.
        index: u8,
        /// Palette length the index was looked up against.
        palette_len: usize,
    },
}

/// `Result` alias scoped to canvas errors.
pub type Result<T> = core::result::Result<T, Error>;
