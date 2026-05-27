//! Drawing tools: stroke rasterization, flood fill, and shape drawing.
//!
//! Every function in this module operates on a [`crate::canvas::PixelBuffer`] in place.
//! No I/O, no undo — those concerns live in the `app` crate. The
//! algorithms here are deterministic pure functions so they can be
//! property-tested without standing up a full project.

pub mod fill;
pub mod shapes;
pub mod stroke;

pub use fill::flood_fill;
pub use shapes::{draw_ellipse, draw_filled_ellipse, draw_filled_rect, draw_rect};
pub use stroke::{BrushShape, brush_covers, draw_line, draw_stroke, paint_brush, stamp_segment};
