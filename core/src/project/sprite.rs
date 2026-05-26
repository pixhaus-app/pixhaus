//! A sprite: one canvas with its layers, frames, palettes, and tilesets.
//!
//! A project may hold multiple sprites (think character sheets where
//! each character is its own sprite). Each sprite owns its layer
//! stack, its frame timeline, its palettes, its tilesets, and the
//! cels that bind a layer-frame pair to pixel data.

use serde::{Deserialize, Serialize};

use super::animation::Animation;
use super::cel::{Cel, CelData};
use super::color::ColorMode;
use super::frame::{Frame, FrameTag};
use super::geometry::Size;
use super::id::{FrameIndex, LayerId, SpriteId};
use super::layer::Layer;
use super::palette::{Palette, PaletteFrameOverride};
use super::slice::Slice;
use super::tileset::Tileset;
use super::user_data::UserData;

/// A sprite within a Pixhaus project.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

    /// Returns the cel at `(layer, frame)`, or `None` if the layer has no
    /// content on that frame.
    #[must_use]
    pub fn cel(&self, layer: LayerId, frame: FrameIndex) -> Option<&Cel> {
        self.cels.iter().find(|c| c.layer_id == layer && c.frame_index == frame)
    }

    /// Resolves a possibly-linked cel to the frame that actually owns its
    /// pixel data. A [`CelData::Linked`] cel points at its source frame on the
    /// same layer; this follows that one hop. Owning (raster/tilemap) cels
    /// resolve to themselves. Returns `frame` unchanged if no cel exists.
    #[must_use]
    pub fn resolve_source_frame(&self, layer: LayerId, frame: FrameIndex) -> FrameIndex {
        match self.cel(layer, frame).map(|c| &c.data) {
            Some(CelData::Linked { source_frame }) => *source_frame,
            _ => frame,
        }
    }

    /// Returns the link set a frame belongs to on `layer`: every frame whose
    /// cel shares the same owning source, including the source itself, sorted
    /// by frame index.
    ///
    /// The owning source frame is the stable link-set identity — editing the
    /// source updates the whole set. Idle animations that reuse one drawing
    /// across many frames form a link set without duplicating pixel data.
    /// Cel linking adopted from Pixelorama/Aseprite; see `THIRD_PARTY_NOTICES.md`.
    #[must_use]
    pub fn cel_link_set(&self, layer: LayerId, frame: FrameIndex) -> Vec<FrameIndex> {
        let source = self.resolve_source_frame(layer, frame);
        let mut members: Vec<FrameIndex> = self
            .cels
            .iter()
            .filter(|c| c.layer_id == layer)
            .filter(|c| match &c.data {
                CelData::Linked { source_frame } => *source_frame == source,
                _ => c.frame_index == source,
            })
            .map(|c| c.frame_index)
            .collect();
        members.sort_by_key(|f| f.get());
        members.dedup();
        members
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
        let json = serde_json::to_string(&s).unwrap();
        let back: Sprite = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    fn sprite_with_link_set() -> Sprite {
        use super::super::id::PixelBufferId;
        let mut s = Sprite::empty(SpriteId::new(1), "anim", Size::new(8, 8));
        let layer = LayerId::new(1);
        // Frame 0 owns the drawing; frames 1 and 3 link to it; frame 2 is its
        // own raster cel (not part of the set).
        s.cels.push(Cel::raster(layer, FrameIndex::new(0), PixelBufferId::new(10), Size::new(8, 8)));
        s.cels.push(Cel {
            layer_id: layer,
            frame_index: FrameIndex::new(1),
            position: super::super::geometry::IVec2::zero(),
            opacity: 255,
            data: CelData::Linked {
                source_frame: FrameIndex::new(0),
            },
            user_data: UserData::default(),
        });
        s.cels.push(Cel::raster(layer, FrameIndex::new(2), PixelBufferId::new(11), Size::new(8, 8)));
        s.cels.push(Cel {
            layer_id: layer,
            frame_index: FrameIndex::new(3),
            position: super::super::geometry::IVec2::zero(),
            opacity: 255,
            data: CelData::Linked {
                source_frame: FrameIndex::new(0),
            },
            user_data: UserData::default(),
        });
        s
    }

    #[test]
    fn resolve_source_frame_follows_link() {
        let s = sprite_with_link_set();
        let layer = LayerId::new(1);
        assert_eq!(s.resolve_source_frame(layer, FrameIndex::new(1)), FrameIndex::new(0));
        // An owning cel resolves to itself.
        assert_eq!(s.resolve_source_frame(layer, FrameIndex::new(2)), FrameIndex::new(2));
    }

    #[test]
    fn cel_link_set_groups_shared_source() {
        let s = sprite_with_link_set();
        let layer = LayerId::new(1);
        let from_link = s.cel_link_set(layer, FrameIndex::new(3));
        assert_eq!(from_link, vec![FrameIndex::new(0), FrameIndex::new(1), FrameIndex::new(3)]);
        assert_eq!(s.cel_link_set(layer, FrameIndex::new(2)), vec![FrameIndex::new(2)]);
    }
}
