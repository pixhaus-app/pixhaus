//! Drawing tools: stroke rasterization, flood fill, and shape drawing.
//!
//! Every function in this module operates on a [`crate::canvas::PixelBuffer`] in place.
//! No I/O, no undo — those concerns live in the `app` crate. The
//! algorithms here are deterministic pure functions so they can be
//! property-tested without standing up a full project.

pub mod dither;
pub mod fill;
pub mod shapes;
pub mod stroke;

pub use dither::{DitherPattern, dither_allows};
pub use fill::{flood_fill, flood_fill_blended};
pub use shapes::{draw_ellipse, draw_filled_ellipse, draw_filled_rect, draw_rect};
pub use stroke::{
    BrushShape, brush_covers, draw_line, draw_line_masked, draw_stroke, paint_brush, paint_brush_blended, paint_brush_masked, stamp_segment,
    stamp_segment_masked,
};
