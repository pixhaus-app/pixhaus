//! Brush state: the active drawing tool and its parameters.
//!
//! Brushes are editor state. They persist across save/load so the
//! user's working tool comes back on reopen. The render layer owns
//! actual stroke geometry; this module tracks shape, size, and
//! current-color references.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::id::PaletteId;

/// The shape of the active brush.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum BrushShape {
    /// Circular brush; `size` is the diameter.
    Circle,
    /// Square brush; `size` is the side length.
    Square,
    /// Single-pixel brush; `size` is ignored.
    #[default]
    Pixel,
}

/// The active brush configuration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BrushState {
    /// Brush shape.
    pub shape: BrushShape,
    /// Brush size in pixels (diameter for circles, side for squares).
    pub size: u32,
    /// Index into the active palette for the foreground color.
    pub foreground_index: u32,
    /// Index into the active palette for the background color.
    pub background_index: u32,
    /// Active palette providing colors for the brush. `None` means
    /// fall back to the sprite's first palette.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub active_palette: Option<PaletteId>,
}

impl Default for BrushState {
    fn default() -> Self {
        Self {
            shape: BrushShape::Pixel,
            size: 1,
            foreground_index: 0,
            background_index: 1,
            active_palette: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_pixel_size_one() {
        let b = BrushState::default();
        assert_eq!(b.shape, BrushShape::Pixel);
        assert_eq!(b.size, 1);
    }

    #[test]
    fn brush_round_trip() {
        let b = BrushState {
            shape: BrushShape::Circle,
            size: 8,
            foreground_index: 3,
            background_index: 4,
            active_palette: Some(PaletteId::new(2)),
        };
        let bytes = rmp_serde::to_vec_named(&b).unwrap();
        let back: BrushState = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(b, back);
    }

    #[test]
    fn brush_shape_serializes_snake_case() {
        let json = serde_json::to_string(&BrushShape::Square).unwrap();
        assert_eq!(json, "\"square\"");
    }
}
