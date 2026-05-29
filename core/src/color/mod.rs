//! Color operations: space conversions, harmony, palette ops, and the
//! eyedropper palette [`extraction`] used by the sheet-approval flow
//! ([`crate::project::approval`]).

pub mod extraction;
pub mod harmony;
pub mod ops;
pub mod palette_file;
pub mod space;

pub use extraction::{
    DEFAULT_MAX_COLORS, DEFAULT_QUANTIZE_BITS, ExtractionError, ExtractionOptions, extract_palette_from_image_bytes, extract_palette_from_rgba8,
};
pub use harmony::{analogous, analogous_standard, complement, monochromatic, split_complement, tetrad, triad};
pub use ops::{DEFAULT_SIMILARITY_TOLERANCE, color_ramp, nearest_color_index, palette_cycle, palette_swap, similar_colors};
pub use palette_file::{PaletteFileError, PaletteFileResult};
pub use space::{from_hsl, from_hsv, from_oklch, oklab_mix, rotate_hue, to_hsl, to_hsv, to_oklch};
