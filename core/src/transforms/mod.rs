//! Transform operations for pixel buffers.
//!
//! Every transform takes a [`crate::canvas::buffer::PixelBuffer`] and
//! returns a new transformed buffer. No pixel data is mutated in place.
//!
//! The surface here covers the canvas-level operations the shell drives —
//! scale (resample), resize (pad/crop), flip, and 90°-multiple rotation — plus
//! the animation-sheet normalization path. The selection-aware,
//! arbitrary-angle, mask-constrained variants (skew, perspective, antialias,
//! the `TransformSpec` dispatcher) are editor-tool work and are not ported.

pub mod error;
pub mod flip;
pub mod normalize;
pub mod resize;
pub mod rotate;
pub mod scale;

pub use error::{Error, Result};
pub use flip::{flip_horizontal, flip_vertical};
pub use normalize::{ChromaKey, FrameMetrics, NormalizeOptions, NormalizeReport, NormalizeResult, SeamMatch, chroma_key, measure, normalize_frames, repad};
pub use resize::{CanvasAnchor, resize_canvas};
pub use rotate::{rotate_90_ccw, rotate_90_cw, rotate_180};
pub use scale::{scale_integer, scale_integer_down, scale_nearest};
