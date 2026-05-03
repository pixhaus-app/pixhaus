//! Canvas viewport state.
//!
//! Where the user is in the document: which sprite is active, which
//! frame, which layer, where the viewport is scrolled, what zoom is
//! applied. Persists across save/load so the user returns to the same
//! viewport.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::id::{FrameIndex, LayerId, SpriteId};

/// Active viewport configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CanvasState {
    /// Sprite currently being edited. `None` means the project is
    /// open but no sprite is foregrounded yet.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub active_sprite: Option<SpriteId>,
    /// Layer currently selected for editing.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub active_layer: Option<LayerId>,
    /// Frame currently shown in the viewport.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub active_frame: Option<FrameIndex>,
    /// Viewport scroll offset in canvas pixels (float so the viewport
    /// can interpolate between integer canvas positions during pans).
    pub scroll_x: f32,
    /// Viewport scroll offset in canvas pixels (vertical).
    pub scroll_y: f32,
    /// Zoom factor. `1.0` is one canvas pixel per screen pixel.
    pub zoom: f32,
    /// Whether the editor is in onion-skin mode.
    pub onion_skin: bool,
    /// Whether tile grid lines render over tilemap layers.
    pub show_tile_grid: bool,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            active_sprite: None,
            active_layer: None,
            active_frame: None,
            scroll_x: 0.0,
            scroll_y: 0.0,
            zoom: 1.0,
            onion_skin: false,
            show_tile_grid: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_zoom_is_one() {
        assert!((CanvasState::default().zoom - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn canvas_round_trip() {
        let c = CanvasState {
            active_sprite: Some(SpriteId::new(1)),
            active_layer: Some(LayerId::new(2)),
            active_frame: Some(FrameIndex::new(3)),
            scroll_x: 12.5,
            scroll_y: -4.0,
            zoom: 8.0,
            onion_skin: true,
            show_tile_grid: true,
        };
        let bytes = rmp_serde::to_vec_named(&c).unwrap();
        let back: CanvasState = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(c, back);
    }
}
