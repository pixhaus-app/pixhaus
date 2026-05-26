//! Layer model: the heterogeneous units that compose a sprite frame.
//!
//! A sprite owns a flat `Vec<Layer>`. Group hierarchy is expressed by
//! the optional `parent` field rather than by nesting; this matches
//! Aseprite's storage and lets the timeline render the layer column
//! linearly without recursive walks.

use serde::{Deserialize, Serialize};

use super::blend::BlendMode;
use super::effect::LayerEffect;
use super::geometry::IVec2;
use super::id::{LayerId, PixelBufferId, TilesetId};
use super::user_data::UserData;

/// A single layer in a sprite.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    /// Stable identifier for this layer.
    pub id: LayerId,
    /// Display name shown in the layer panel.
    pub name: String,
    /// Variant-specific payload (raster, group, tilemap, reference).
    pub kind: LayerKind,
    /// Composition blend mode.
    pub blend_mode: BlendMode,
    /// Opacity, `0` (invisible) through `255` (fully opaque).
    pub opacity: u8,
    /// Whether the layer renders. Hidden layers are still part of the
    /// document; they just don't contribute to the composite.
    pub visible: bool,
    /// Whether the layer is locked against edits.
    pub locked: bool,
    /// Parent group, if any. `None` means a top-level layer.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub parent: Option<LayerId>,
    /// Non-destructive effects applied to this layer's composited pixels
    /// in order at render time. Empty by default; files written before
    /// effects existed load with an empty stack and re-save with the field
    /// omitted from the wire form.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<LayerEffect>,
    /// Free-form user metadata.
    #[serde(skip_serializing_if = "UserData::is_empty", default)]
    pub user_data: UserData,
}

impl Layer {
    /// Constructs a raster layer with sensible defaults.
    #[must_use]
    pub fn raster(id: LayerId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            kind: LayerKind::Raster,
            blend_mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            locked: false,
            parent: None,
            effects: Vec::new(),
            user_data: UserData::default(),
        }
    }
}

/// Variant payload for a [`Layer`].
///
/// The payload distinguishes the four supported layer kinds. Group
/// layers carry only display state because their pixel content lives
/// on their children; raster, tilemap, and reference layers carry the
/// pointers their cels need.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LayerKind {
    /// Pixel-buffer layer. Cels on this layer carry raster pixel data.
    Raster,
    /// Group layer. Holds no pixels of its own; child layers reference
    /// it via `parent`.
    Group {
        /// Whether the group is rendered collapsed in the layer panel.
        collapsed: bool,
    },
    /// Tilemap layer. Cels on this layer carry tile-grid data resolved
    /// against the named tileset.
    Tilemap {
        /// Tileset whose tiles this layer's cels reference.
        tileset: TilesetId,
    },
    /// Reference layer. Holds a single image used as visual reference
    /// during editing; the layer is excluded from exports unless the
    /// user opts in.
    Reference {
        /// Pixel-buffer handle for the reference image.
        image: PixelBufferId,
        /// Top-left placement of the image in canvas coordinates.
        origin: IVec2,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raster_constructor_sets_defaults() {
        let l = Layer::raster(LayerId::new(1), "background");
        assert_eq!(l.opacity, 255);
        assert!(l.visible);
        assert!(!l.locked);
        assert_eq!(l.blend_mode, BlendMode::Normal);
        assert!(matches!(l.kind, LayerKind::Raster));
    }

    #[test]
    fn tilemap_layer_round_trip() {
        let l = Layer {
            id: LayerId::new(3),
            name: "ground".into(),
            kind: LayerKind::Tilemap {
                tileset: TilesetId::new(7),
            },
            blend_mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            locked: false,
            parent: Some(LayerId::new(2)),
            effects: Vec::new(),
            user_data: UserData::default(),
        };
        let json = serde_json::to_string(&l).unwrap();
        let back: Layer = serde_json::from_str(&json).unwrap();
        assert_eq!(l, back);
    }

    #[test]
    fn layer_kind_uses_kind_tag_in_json() {
        let l = Layer::raster(LayerId::new(1), "main");
        let json = serde_json::to_string(&l.kind).unwrap();
        assert_eq!(json, "{\"kind\":\"raster\"}");
    }
}
