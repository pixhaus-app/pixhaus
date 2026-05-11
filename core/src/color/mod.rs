//! Color space conversions, harmony tools, and palette operations.
//!
//! This module lives in `core` and builds on the
//! [`crate::project::color::Rgba`] type. It does no I/O; format readers
//! and writers live in `pixhaus-io`.
//!
//! # Modules
//!
//! - [`space`] — HSV / HSL / Oklab conversions and perceptual mixing.
//! - [`harmony`] — Complement, split-complement, triad, tetrad, analogous,
//!   monochromatic palette generators.
//! - [`ops`] — Palette swap, color cycle, color ramp, nearest-color index.
//! - [`extraction`] — Eyedropper palette extraction from raster images;
//!   used by the B10.3 sheet approval flow.

pub mod extraction;
pub mod harmony;
pub mod ops;
pub mod space;

// Re-export the most commonly used functions at the module level so callers
// don't have to reach into sub-modules for day-to-day use.
pub use extraction::{
    DEFAULT_MAX_COLORS, DEFAULT_QUANTIZE_BITS, ExtractionError, ExtractionOptions,
    extract_palette_from_png_bytes, extract_palette_from_rgba8,
};
pub use harmony::{
    analogous, analogous_standard, complement, monochromatic, split_complement, tetrad, triad,
};
pub use ops::{color_ramp, nearest_color_index, palette_cycle, palette_swap};
pub use space::{from_hsl, from_hsv, oklab_mix, rotate_hue, to_hsl, to_hsv};
