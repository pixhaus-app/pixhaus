//! Transform operations for pixel buffers.
//!
//! Every transform takes a [`crate::canvas::buffer::PixelBuffer`] and
//! returns a new transformed buffer. No pixel data is mutated in place.
//!
//! The surface here covers the canvas-level operations the shell drives —
//! scale (resample), resize (pad/crop), flip, 90°-multiple and arbitrary-angle
//! rotation — plus the animation-sheet normalization path. Arbitrary-angle
//! rotation ([`rotate::rotate`] with [`rotate::RotationAlgorithm`]) keeps the
//! source dimensions and runs on lifted pixels or the whole canvas. Skew
//! ([`skew::skew_x`], [`skew::skew_y`]) shifts rows or columns proportional to
//! position, keeping source dimensions. The remaining selection-aware,
//! mask-constrained variants (perspective, antialias) are editor-tool work and
//! not yet ported.

pub mod error;
pub mod flip;
pub mod normalize;
pub mod resize;
pub mod rotate;
pub mod scale;
pub mod skew;

pub use error::{Error, Result};
pub use flip::{flip_horizontal, flip_vertical};
pub use normalize::{ChromaKey, FrameMetrics, NormalizeOptions, NormalizeReport, NormalizeResult, SeamMatch, chroma_key, measure, normalize_frames, repad};
pub use resize::{CanvasAnchor, resize_canvas};
pub use rotate::{RotationAlgorithm, rotate, rotate_90_ccw, rotate_90_cw, rotate_180, rotate_bilinear, rotate_nearest, rotate_rotsprite};
pub use scale::{scale_integer, scale_integer_down, scale_nearest};
pub use skew::{skew_x, skew_y};
