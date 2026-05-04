//! A sprite: one canvas with its layers, frames, palettes, and tilesets.
//!
//! A project may hold multiple sprites (think character sheets where
//! each character is its own sprite). Each sprite owns its layer
//! stack, its frame timeline, its palettes, its tilesets, and the
//! cels that bind a layer-frame pair to pixel data.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::animation::Animation;
use super::cel::Cel;
use super::color::ColorMode;
use super::frame::{Frame, FrameTag};
use super::geometry::Size;
use super::id::SpriteId;
use super::layer::Layer;
use super::palette::{Palette, PaletteFrameOverride};
use super::slice::Slice;
use super::tileset::Tileset;
use super::user_data::UserData;

/// A sprite within a Pixhaus project.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Sprite {
    /// Stable identifier within the parent project.
    pub id: SpriteId,
    /// Display name in the project tree.
    pub name: String,
    /// Canvas dimensions in pixels.
    pub canvas: Size,
    /// Authoring color mode.
    pub color_mode: ColorMode,
    /// Palette index treated as transparent in indexed-mode sprites.
    /// `None` for RGBA / grayscale sprites or when no transparent index
    /// is declared; defaults to `Some(0)` after a freshly-loaded indexed
    /// sprite per Aseprite convention.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub transparent_color_index: Option<u8>,
    /// Layer stack. Order matters: index `0` is the bottom layer.
    pub layers: Vec<Layer>,
    /// Frame timeline. Index is the frame number; `frames.len()` is
    /// the total animation length.
    pub frames: Vec<Frame>,
    /// Cels: per-(layer, frame) drawn content. Sparse — a layer with
    /// no content on a frame simply has no entry.
    pub cels: Vec<Cel>,
    /// Palettes available within this sprite.
    pub palettes: Vec<Palette>,
    /// Per-frame palette overrides for animated palette-cycling. Keyed
    /// on frame index; each entry replaces the active palette wholesale
    /// at that frame boundary. Sparse — frames not present inherit the
    /// previous frame's palette state. Empty for the common case where
    /// the palette is constant for the sprite's lifetime.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub palette_frame_overrides: Vec<PaletteFrameOverride>,
    /// Tilesets available within this sprite.
    pub tilesets: Vec<Tileset>,
    /// Editor-side frame tags (named ranges).
    pub frame_tags: Vec<FrameTag>,
    /// Engine-side animation entries (frame ranges + handoff metadata).
    pub animations: Vec<Animation>,
    /// Named rectangular regions used for nine-slice and pivots.
    pub slices: Vec<Slice>,
    /// Free-form user metadata.
    #[serde(skip_serializing_if = "UserData::is_empty", default)]
    pub user_data: UserData,
}

impl Sprite {
    /// Constructs an empty RGBA sprite of the given canvas size with
    /// no layers, frames, or palettes.
    #[must_use]
    pub fn empty(id: SpriteId, name: impl Into<String>, canvas: Size) -> Self {
        Self {
            id,
            name: name.into(),
            canvas,
            color_mode: ColorMode::Rgba,
            transparent_color_index: None,
            layers: Vec::new(),
            frames: Vec::new(),
            cels: Vec::new(),
            palettes: Vec::new(),
            palette_frame_overrides: Vec::new(),
            tilesets: Vec::new(),
            frame_tags: Vec::new(),
            animations: Vec::new(),
            slices: Vec::new(),
            user_data: UserData::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_constructor_has_no_content() {
        let s = Sprite::empty(SpriteId::new(1), "hero", Size::new(32, 32));
        assert_eq!(s.canvas, Size::new(32, 32));
        assert!(s.layers.is_empty());
        assert!(s.frames.is_empty());
    }

    #[test]
    fn empty_sprite_round_trip() {
        let s = Sprite::empty(SpriteId::new(1), "blank", Size::new(8, 8));
        let bytes = rmp_serde::to_vec_named(&s).unwrap();
        let back: Sprite = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(s, back);
    }
}
