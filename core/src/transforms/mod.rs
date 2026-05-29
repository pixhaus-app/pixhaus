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
//! position, keeping source dimensions. Perspective
//! ([`perspective::perspective`]) warps the four source corners onto
//! caller-supplied destination corners via a projective homography, keeping
//! source dimensions and sampling bilinearly. Morphological antialias
//! ([`antialias::morphological_antialias`] with [`antialias::MlaaConfig`])
//! softens stair-stepped edges in two passes (rows then columns), keeping
//! source dimensions; `softness == 0` is identity. Procedural inbetweening
//! ([`inbetween::tween`], built on [`inbetween::interpolate_frames`])
//! generates evenly spaced intermediate frames between two cels via
//! variance-rejected 3×3 weighted averaging; the endpoints are reproduced
//! byte-for-byte and the byte-level core is infallible. Beat-driven timing
//! ([`audio_timing::detect_onsets_wav`], [`audio_timing::onset_frame_durations`])
//! runs an energy-envelope onset detector over an inline-parsed PCM WAV and
//! turns the detected beats into per-frame durations at a target fps; it reads
//! audio for timing only, never embedding it.
//!
//! Selection-awareness boundary: transforms here operate on whole buffers.
//! The shell lifts a selection into a canvas-sized buffer, transforms that,
//! and stamps the result back, so these functions stay selection-agnostic.

pub mod antialias;
pub mod audio_timing;
pub mod error;
pub mod finisher;
pub mod flip;
pub mod inbetween;
pub mod normalize;
pub mod perspective;
pub mod resize;
pub mod rotate;
pub mod scale;
pub mod sheet;
pub mod skew;

pub use antialias::{MlaaConfig, morphological_antialias};
pub use audio_timing::{AudioError, OnsetMs, detect_onsets_wav, onset_frame_durations};
pub use error::{Error, Result};
pub use finisher::{FinishOptions, FinishedFrame, PaletteSource, finish_frame, finish_frames, snap_to_palette};
pub use flip::{flip_horizontal, flip_vertical};
pub use inbetween::{DEFAULT_VARIANCE_RANGE, InbetweenError, interpolate_frames, tween, tween_with_variance};
pub use normalize::{
    ChromaKey, Component, ComponentMode, ComponentStats, FrameMetrics, NormalizeOptions, NormalizeReport, NormalizeResult, SeamMatch, chroma_key,
    chroma_key_two_pass, connected_components, label_components, measure, measure_components, measure_qc, normalize_frames, repad,
};
pub use perspective::perspective;
pub use resize::{CanvasAnchor, crop, resize_canvas};
pub use rotate::{RotationAlgorithm, rotate, rotate_90_ccw, rotate_90_cw, rotate_180, rotate_bilinear, rotate_nearest, rotate_rotsprite};
pub use scale::{scale_integer, scale_integer_down, scale_nearest};
pub use sheet::{SliceGrid, slice_grid, slice_grid_spec, slice_rects};
pub use skew::{skew_x, skew_y};
