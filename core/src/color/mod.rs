//! Color operations used by the vertical slice.
//!
//! Only the eyedropper palette [`extraction`] is ported — the broader
//! color subsystem (space conversions, harmony, palette ops) is editor
//! tooling and is not part of the slice. `extraction` is kept because the
//! sheet-approval flow ([`crate::project::approval`]) depends on it.

pub mod extraction;

pub use extraction::{
    DEFAULT_MAX_COLORS, DEFAULT_QUANTIZE_BITS, ExtractionError, ExtractionOptions, extract_palette_from_image_bytes, extract_palette_from_rgba8,
};
