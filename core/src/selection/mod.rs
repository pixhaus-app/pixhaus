//! Selection algorithms and mask operations (stream S03).
//!
//! The selection subsystem has three layers:
//!
//! - **[`SelectionMask`]** — the coverage-map type. One `u8` per canvas
//!   pixel; `0` = unselected, `255` = fully selected, intermediate
//!   values arise from feathering.
//!
//! - **[`algorithms`]** — algorithms that *produce* masks from
//!   geometric shapes or pixel content:
//!   - [`algorithms::select_rect`] — axis-aligned rectangle
//!   - [`algorithms::select_ellipse`] — filled ellipse
//!   - [`algorithms::select_polygon`] — scanline-filled polygon
//!   - [`algorithms::magic_wand`] — flood-fill within a tolerance
//!   - [`algorithms::color_range`] — global colour-range selection
//!
//! - **[`morphology`]** — operations that *modify* an existing mask:
//!   - [`morphology::expand`] — dilation (grow selection outward)
//!   - [`morphology::contract`] — erosion (shrink selection inward)
//!   - [`morphology::feather`] — blur coverage for soft edges
//!
//! Boolean set operations (union, intersect, subtract, xor, invert)
//! are methods on [`SelectionMask`] itself.
//!
//! # Pixel-art notes
//!
//! All algorithms snap to integer pixel boundaries. Feathering is
//! available but off by default at the editor layer. Magic wand and
//! colour range use a per-channel absolute-difference tolerance so
//! `tolerance = 0` selects only exact matches and `tolerance = 255`
//! selects everything.

pub mod algorithms;
pub mod autoclose;
pub mod error;
pub mod mask;
pub mod morphology;
mod skeleton_lut;

pub use algorithms::{
    Connectivity, color_range, magic_wand, magic_wand_with_gap_close, select_ellipse,
    select_polygon, select_rect,
};
pub use autoclose::{GapCloseConfig, close_gaps};
pub use error::{Error, Result};
pub use mask::SelectionMask;
pub use morphology::{contract, expand, feather};
