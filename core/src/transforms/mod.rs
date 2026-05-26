//! Transform operations for pixel buffers.
//!
//! Every transform takes a [`crate::canvas::buffer::PixelBuffer`] and
//! returns a new transformed buffer. No pixel data is mutated in place.
//!
//! This is the vertical-slice subset: only the scale and animation-sheet
//! normalization paths are ported. The full transform surface (translate,
//! flip, rotate, skew, perspective, antialias) and the selection-aware
//! `TransformSpec` dispatcher are editor-only and are not part of the slice.

pub mod error;
pub mod normalize;
pub mod scale;

pub use error::{Error, Result};
pub use normalize::{ChromaKey, FrameMetrics, NormalizeOptions, NormalizeReport, NormalizeResult, SeamMatch, chroma_key, measure, normalize_frames, repad};
pub use scale::{scale_integer, scale_integer_down, scale_nearest};
