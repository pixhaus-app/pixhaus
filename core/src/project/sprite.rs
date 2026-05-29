//! A sprite: one canvas with its layers, frames, palettes, and tilesets.
//!
//! A project may hold multiple sprites (think character sheets where
//! each character is its own sprite). Each sprite owns its layer
//! stack, its frame timeline, its palettes, its tilesets, and the
//! cels that bind a layer-frame pair to pixel data.

use serde::{Deserialize, Serialize};

use super::animation::Animation;
use super::blend::BlendMode;
use super::cel::{Cel, CelData};
use super::color::ColorMode;
use super::frame::{Frame, FrameTag};
use super::geometry::Size;
use super::id::{FrameIndex, LayerId, PaletteId, SpriteId};
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

/// One non-group layer resolved for compositing: its blend mode plus the
/// visibility and opacity it has after ancestor groups are folded in. Produced
/// by [`Sprite::composite_order`] in back-to-front order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerComposite {
    /// The layer that contributes pixels.
    pub layer: LayerId,
    /// Blend mode to composite it with.
    pub mode: BlendMode,
    /// Effective visibility (false if the layer or any ancestor group is hidden).
    pub visible: bool,
    /// Effective opacity, the layer's opacity scaled by ancestor group opacities.
    pub opacity: u8,
    /// Whether this layer clips to the layer below it — its visible pixels are
    /// masked by the alpha of everything composited at-and-below its clip base.
    /// In-memory only; never serialized (it mirrors [`Layer::clip_below`]).
    pub clip_below: bool,
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

    /// Returns the layer with `id`, if present.
    #[must_use]
    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    /// Returns the `layers` index of `id`, if present.
    #[must_use]
    pub fn layer_index(&self, id: LayerId) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    /// A layer's effective parent: its `parent` if that still names an existing
    /// group, otherwise top level. Guards against a dangling `parent` pointer so
    /// no layer is ever orphaned out of the tree (mirrors the library's folder
    /// handling).
    #[must_use]
    fn effective_layer_parent(&self, layer: &Layer) -> Option<LayerId> {
        match layer.parent {
            Some(pid) if self.layers.iter().any(|l| l.id == pid && l.is_group()) => Some(pid),
            _ => None,
        }
    }

    /// Whether `ancestor` is `id` itself or one of the groups above it in the
    /// parent chain. Used to reject group moves that would form a cycle.
    #[must_use]
    pub fn layer_is_ancestor(&self, ancestor: LayerId, id: LayerId) -> bool {
        let mut cursor = Some(id);
        while let Some(cid) = cursor {
            if cid == ancestor {
                return true;
            }
            cursor = self.layer(cid).and_then(|l| l.parent);
        }
        false
    }

    /// Reparents and repositions `id`. Its parent becomes `new_parent` (which
    /// must be `None` or an existing group), and it lands directly above
    /// `anchor` in stacking order when `anchor` is a sibling, or at the top of
    /// its new sibling group when `anchor` is `None`. Rejects a missing layer, a
    /// non-group parent, or a move that would nest a group inside its own
    /// subtree. Returns whether it applied.
    pub fn move_layer(&mut self, id: LayerId, new_parent: Option<LayerId>, anchor: Option<LayerId>) -> bool {
        if self.layer_index(id).is_none() {
            return false;
        }
        if let Some(pid) = new_parent {
            let is_group = self.layer(pid).is_some_and(Layer::is_group);
            // Reject self-parent, a non-group parent, or nesting into a descendant.
            if pid == id || !is_group || self.layer_is_ancestor(id, pid) {
                return false;
            }
        }
        let Some(pos) = self.layer_index(id) else {
            return false;
        };
        let mut layer = self.layers.remove(pos);
        layer.parent = new_parent;
        // Indices below are computed after the removal above.
        let insert_at = match anchor.and_then(|a| self.layer_index(a)) {
            // Just above the anchor in display order (one slot higher in the Vec).
            Some(anchor_idx) => anchor_idx + 1,
            // Top of the destination sibling group: one past its highest member.
            None => self
                .layers
                .iter()
                .rposition(|l| self.effective_layer_parent(l) == new_parent)
                .map_or(self.layers.len(), |i| i + 1),
        };
        self.layers.insert(insert_at.min(self.layers.len()), layer);
        true
    }

    /// Moves `id` one slot toward the top (`up`) or bottom of the stack among
    /// its siblings — layers sharing the same parent. A no-op at the ends.
    pub fn move_layer_in_stack(&mut self, id: LayerId, up: bool) {
        let Some(idx) = self.layer_index(id) else {
            return;
        };
        let parent = self.effective_layer_parent(&self.layers[idx]);
        // Display is top-first while the Vec is bottom-first, so "up" is a higher index.
        let target = if up {
            self.layers[idx + 1..]
                .iter()
                .position(|l| self.effective_layer_parent(l) == parent)
                .map(|off| idx + 1 + off)
        } else {
            self.layers[..idx].iter().rposition(|l| self.effective_layer_parent(l) == parent)
        };
        if let Some(target) = target {
            self.layers.swap(idx, target);
        }
    }

    /// Removes `id`, re-parenting its direct children to its own parent so they
    /// stay in the tree (mirrors the library's folder delete), then drops the
    /// layer's cels.
    pub fn remove_layer(&mut self, id: LayerId) {
        let Some(parent) = self.layer(id).map(|l| l.parent) else {
            return;
        };
        for layer in &mut self.layers {
            if layer.parent == Some(id) {
                layer.parent = parent;
            }
        }
        self.layers.retain(|l| l.id != id);
        self.cels.retain(|c| c.layer_id != id);
    }

    /// The palette with `id`, if present.
    #[must_use]
    pub fn palette(&self, id: PaletteId) -> Option<&Palette> {
        self.palettes.iter().find(|p| p.id == id)
    }

    /// The palette with `id`, mutably, if present.
    pub fn palette_mut(&mut self, id: PaletteId) -> Option<&mut Palette> {
        self.palettes.iter_mut().find(|p| p.id == id)
    }

    /// Appends an empty palette named `name` under `id`. The caller allocates
    /// `id` (a fresh [`PaletteId`]) before calling. A blank name is kept as-is —
    /// the panel guards against empty names at the UI edge, not here.
    pub fn add_palette(&mut self, id: PaletteId, name: impl Into<String>) {
        self.palettes.push(Palette::from_colors(id, name, Vec::new()));
    }

    /// Removes the palette with `id`, returning whether it removed one. Refuses
    /// to drop the sprite's last palette: a sprite always keeps at least one, so
    /// the panel never lands on an empty switcher. An absent `id` is a no-op.
    pub fn delete_palette(&mut self, id: PaletteId) -> bool {
        if self.palettes.len() <= 1 {
            return false;
        }
        let before = self.palettes.len();
        self.palettes.retain(|p| p.id != id);
        self.palettes.len() != before
    }

    /// Renames the palette with `id`, returning whether it renamed one. A
    /// whitespace-only name is rejected (no-op), matching the sprite/group rename
    /// validation. An absent `id` is a no-op.
    pub fn rename_palette(&mut self, id: PaletteId, new_name: &str) -> bool {
        let name = new_name.trim();
        if name.is_empty() {
            return false;
        }
        match self.palette_mut(id) {
            Some(palette) => {
                name.clone_into(&mut palette.name);
                true
            }
            None => false,
        }
    }

    /// The non-group layers in back-to-front composite order, each carrying the
    /// blend mode plus the visibility and opacity it inherits once ancestor
    /// groups are folded in. A group hidden (or dimmed) hides (or dims) all of
    /// its descendants; the group itself contributes no entry.
    #[must_use]
    pub fn composite_order(&self) -> Vec<LayerComposite> {
        let mut out = Vec::new();
        self.push_composite_level(None, true, 255, &mut out);
        out
    }

    /// Recursive helper for [`Self::composite_order`].
    fn push_composite_level(&self, parent: Option<LayerId>, parent_visible: bool, parent_opacity: u8, out: &mut Vec<LayerComposite>) {
        for layer in self.layers.iter().filter(|l| self.effective_layer_parent(l) == parent) {
            let visible = parent_visible && layer.visible;
            // Product of two u8 / 255 is always <= 255, so this never truncates.
            let opacity = u8::try_from(u16::from(parent_opacity) * u16::from(layer.opacity) / 255).unwrap_or(255);
            if layer.is_group() {
                self.push_composite_level(Some(layer.id), visible, opacity, out);
            } else {
                out.push(LayerComposite {
                    layer: layer.id,
                    mode: layer.blend_mode,
                    visible,
                    opacity,
                    clip_below: layer.clip_below,
                });
            }
        }
    }

    /// The layers in top-first display order with their nesting depth, skipping
    /// the contents of collapsed groups. Drives the layer panel tree.
    #[must_use]
    pub fn layer_display_order(&self) -> Vec<(LayerId, u16)> {
        let mut rows = Vec::new();
        self.push_display_level(None, 0, &mut rows);
        rows
    }

    /// Recursive helper for [`Self::layer_display_order`]: emit a level's layers
    /// top-first, descending into expanded groups.
    fn push_display_level(&self, parent: Option<LayerId>, depth: u16, rows: &mut Vec<(LayerId, u16)>) {
        for layer in self.layers.iter().rev().filter(|l| self.effective_layer_parent(l) == parent) {
            rows.push((layer.id, depth));
            if layer.is_group() && !layer.collapsed() {
                self.push_display_level(Some(layer.id), depth + 1, rows);
            }
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

    use super::super::layer::LayerKind;

    /// A group layer with the given id and child collapse state.
    fn group(id: u32, collapsed: bool) -> Layer {
        let mut l = Layer::raster(LayerId::new(id), "group");
        l.kind = LayerKind::Group { collapsed };
        l
    }

    fn raster(id: u32, parent: Option<u32>) -> Layer {
        let mut l = Layer::raster(LayerId::new(id), "layer");
        l.parent = parent.map(LayerId::new);
        l
    }

    /// Sprite with a bottom raster (1), a group (2) holding one child raster
    /// (3), and a top raster (4). Vec order is bottom-first.
    fn sprite_with_group() -> Sprite {
        let mut s = Sprite::empty(SpriteId::new(1), "s", Size::new(8, 8));
        s.layers = vec![raster(1, None), group(2, false), raster(3, Some(2)), raster(4, None)];
        s
    }

    #[test]
    fn move_layer_nests_into_group() {
        let mut s = sprite_with_group();
        assert!(s.move_layer(LayerId::new(4), Some(LayerId::new(2)), None));
        assert_eq!(s.layer(LayerId::new(4)).and_then(|l| l.parent), Some(LayerId::new(2)));
    }

    #[test]
    fn move_layer_rejects_group_into_own_subtree() {
        let mut s = sprite_with_group();
        // Group 2 cannot become a child of its own descendant... here 3 is not a
        // group, so nesting into a non-group is also rejected.
        assert!(!s.move_layer(LayerId::new(2), Some(LayerId::new(3)), None));
        // And a group may not nest under itself.
        assert!(!s.move_layer(LayerId::new(2), Some(LayerId::new(2)), None));
    }

    #[test]
    fn move_layer_reorders_above_anchor() {
        let mut s = sprite_with_group();
        // Move bottom raster 1 to sit directly above top raster 4.
        assert!(s.move_layer(LayerId::new(1), None, Some(LayerId::new(4))));
        let order = s.layer_display_order();
        // Top-first: 1 now sits above 4.
        let ids: Vec<u32> = order.iter().map(|(id, _)| id.get()).collect();
        let pos1 = ids.iter().position(|&i| i == 1).unwrap();
        let pos4 = ids.iter().position(|&i| i == 4).unwrap();
        assert!(pos1 < pos4, "layer 1 should display above layer 4: {ids:?}");
    }

    #[test]
    fn remove_group_reparents_children() {
        let mut s = sprite_with_group();
        s.remove_layer(LayerId::new(2));
        assert!(s.layer(LayerId::new(2)).is_none());
        // Child 3 is promoted to the deleted group's parent (top level).
        assert_eq!(s.layer(LayerId::new(3)).and_then(|l| l.parent), None);
    }

    #[test]
    fn composite_order_skips_groups_and_cascades_visibility() {
        let mut s = sprite_with_group();
        let order = s.composite_order();
        // Groups never appear; the three rasters do, bottom-first: 1, 3, 4.
        let ids: Vec<u32> = order.iter().map(|c| c.layer.get()).collect();
        assert_eq!(ids, vec![1, 3, 4]);

        // Hiding the group hides its child but not the top-level rasters.
        if let Some(g) = s.layers.iter_mut().find(|l| l.id == LayerId::new(2)) {
            g.visible = false;
        }
        let order = s.composite_order();
        let child = order.iter().find(|c| c.layer == LayerId::new(3)).unwrap();
        assert!(!child.visible);
        assert!(order.iter().find(|c| c.layer == LayerId::new(4)).unwrap().visible);
    }

    #[test]
    fn composite_order_scales_child_opacity_by_group() {
        let mut s = sprite_with_group();
        if let Some(g) = s.layers.iter_mut().find(|l| l.id == LayerId::new(2)) {
            g.opacity = 128;
        }
        let order = s.composite_order();
        let child = order.iter().find(|c| c.layer == LayerId::new(3)).unwrap();
        // 255 * 128 / 255 == 128.
        assert_eq!(child.opacity, 128);
    }

    #[test]
    fn composite_order_carries_clip_below_flag() {
        let mut s = sprite_with_group();
        // Mark the top raster (4) as clipping to the layer below it.
        if let Some(l) = s.layers.iter_mut().find(|l| l.id == LayerId::new(4)) {
            l.clip_below = true;
        }
        let order = s.composite_order();
        let clipped = order.iter().find(|c| c.layer == LayerId::new(4)).unwrap();
        assert!(clipped.clip_below);
        // Unmarked layers report the flag clear.
        let plain = order.iter().find(|c| c.layer == LayerId::new(1)).unwrap();
        assert!(!plain.clip_below);
    }

    #[test]
    fn layer_display_order_hides_collapsed_group_children() {
        let mut s = sprite_with_group();
        if let Some(g) = s.layers.iter_mut().find(|l| l.id == LayerId::new(2)) {
            g.set_collapsed(true);
        }
        let ids: Vec<u32> = s.layer_display_order().iter().map(|(id, _)| id.get()).collect();
        // Child 3 is hidden while the group is collapsed.
        assert!(!ids.contains(&3), "{ids:?}");
        assert!(ids.contains(&2));
    }

    // ── palette CRUD ──────────────────────────────────────────────────────

    use rstest::rstest;

    /// A sprite carrying `count` palettes named `p0`..`p{count-1}`, ids `1..=count`.
    fn sprite_with_palettes(count: u32) -> Sprite {
        let mut s = Sprite::empty(SpriteId::new(1), "s", Size::new(8, 8));
        for n in 1..=count {
            s.palettes.push(Palette::from_colors(PaletteId::new(n), format!("p{}", n - 1), Vec::new()));
        }
        s
    }

    #[test]
    fn add_palette_appends_an_empty_palette() {
        let mut s = sprite_with_palettes(1);
        s.add_palette(PaletteId::new(7), "extra");
        assert_eq!(s.palettes.len(), 2);
        let added = s.palette(PaletteId::new(7)).expect("added palette present");
        assert_eq!(added.name, "extra");
        assert!(added.colors.is_empty(), "a fresh palette has no swatches");
    }

    #[test]
    fn delete_palette_drops_the_named_palette() {
        let mut s = sprite_with_palettes(3);
        assert!(s.delete_palette(PaletteId::new(2)), "deleting a non-last palette succeeds");
        assert_eq!(s.palettes.len(), 2);
        assert!(s.palette(PaletteId::new(2)).is_none());
        // The survivors keep their order.
        let ids: Vec<u32> = s.palettes.iter().map(|p| p.id.get()).collect();
        assert_eq!(ids, vec![1, 3]);
    }

    #[test]
    fn delete_palette_refuses_the_last_palette() {
        let mut s = sprite_with_palettes(1);
        assert!(!s.delete_palette(PaletteId::new(1)), "the only palette cannot be deleted");
        assert_eq!(s.palettes.len(), 1, "the sprite always keeps at least one palette");
    }

    #[test]
    fn delete_palette_absent_id_is_a_noop() {
        let mut s = sprite_with_palettes(2);
        assert!(!s.delete_palette(PaletteId::new(99)));
        assert_eq!(s.palettes.len(), 2);
    }

    #[rstest]
    #[case("Skin tones", true, "Skin tones")]
    #[case("  trimmed  ", true, "trimmed")]
    #[case("", false, "p0")] // empty rejected, name unchanged
    #[case("   ", false, "p0")] // whitespace-only rejected
    fn rename_palette_trims_and_rejects_blank(#[case] input: &str, #[case] applied: bool, #[case] expect: &str) {
        let mut s = sprite_with_palettes(1);
        assert_eq!(s.rename_palette(PaletteId::new(1), input), applied);
        assert_eq!(s.palette(PaletteId::new(1)).expect("palette present").name, expect);
    }

    #[test]
    fn rename_palette_absent_id_is_a_noop() {
        let mut s = sprite_with_palettes(1);
        assert!(!s.rename_palette(PaletteId::new(42), "nope"));
    }
}
