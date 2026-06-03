//! Onion-skin overlay: ghost the neighbor frames around the current one.
//!
//! The current frame is blitted by the wgpu canvas callback; the neighbors are CPU-
//! composited, uploaded as egui textures, and painted under it at a low tint. The
//! textures are cached and rebuilt only when the document revision or the centered frame
//! changes — the same canvas-derived GPU cache the upload-dedup fields are — so a static
//! onion view uploads once and a held handle keeps each texture alive (egui frees it when
//! the handle drops on the next rebuild).

use egui::{Context, TextureHandle, TextureOptions};

use pixhaus_core::{Document, FrameId, SpriteId};

/// Which side of the current frame a ghost sits on, so the caller can tint past and
/// future distinctly.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Neighbor {
    /// An earlier frame.
    Past,
    /// A later frame.
    Future,
}

/// One cached neighbor-frame texture and the side it belongs to.
pub struct Ghost {
    /// The composited neighbor frame, NEAREST-sampled so the ghost stays crisp.
    pub texture: TextureHandle,
    /// Past or future, for tint selection at paint time.
    pub neighbor: Neighbor,
}

/// Canvas-derived cache of onion-skin neighbor textures, rebuilt when the document
/// revision or the centered frame changes. Empty when onion skin is off or the sprite
/// has a single frame.
#[derive(Default)]
pub struct OnionCache {
    /// `(revision, sprite, centered frame)` the cache was built for; `None` forces a
    /// rebuild. The `SpriteId` is part of the key because `FrameId`s are minted
    /// per-sprite, so they repeat across sprites — without it a sprite switch at the
    /// same revision could serve the previous sprite's ghosts.
    key: Option<(u64, SpriteId, FrameId)>,
    ghosts: Vec<Ghost>,
}

impl OnionCache {
    /// The neighbor ghosts to draw under `center`, rebuilding if the document or centered
    /// frame changed. The immediate previous and next frames (when they exist).
    pub fn ghosts(&mut self, ctx: &Context, doc: &Document, sprite: SpriteId, center: FrameId) -> &[Ghost] {
        let key = Some((doc.revision(), sprite, center));
        if self.key != key {
            self.key = key;
            self.ghosts = build_ghosts(ctx, doc, sprite, center);
        }
        &self.ghosts
    }

    /// Drop the cached textures (onion skin off, or no animated sprite to ghost).
    pub fn clear(&mut self) {
        if self.key.is_some() {
            self.key = None;
            self.ghosts.clear();
        }
    }
}

/// Composite and upload the immediate-neighbor frames of `center`.
fn build_ghosts(ctx: &Context, doc: &Document, sprite_id: SpriteId, center: FrameId) -> Vec<Ghost> {
    let Some(sprite) = doc.sprite(sprite_id) else {
        return Vec::new();
    };
    let frames = sprite.frames();
    let Some(index) = frames.iter().position(|f| f.id == center) else {
        return Vec::new();
    };

    let mut ghosts = Vec::new();
    if let Some(prev) = index.checked_sub(1).and_then(|i| frames.get(i))
        && let Some(ghost) = bake(ctx, doc, sprite_id, prev.id, Neighbor::Past)
    {
        ghosts.push(ghost);
    }
    if let Some(next) = frames.get(index + 1)
        && let Some(ghost) = bake(ctx, doc, sprite_id, next.id, Neighbor::Future)
    {
        ghosts.push(ghost);
    }
    ghosts
}

/// Composite one frame and upload it as a NEAREST egui texture, or `None` if it fails to
/// composite (a missing buffer or size mismatch — skip rather than surface mid-paint).
fn bake(ctx: &Context, doc: &Document, sprite: SpriteId, frame: FrameId, neighbor: Neighbor) -> Option<Ghost> {
    let buffer = pixhaus_core::composite_frame(doc, sprite, frame).ok()?;
    let size = [buffer.width() as usize, buffer.height() as usize];
    let image = egui::ColorImage::from_rgba_unmultiplied(size, buffer.as_bytes());
    let label = match neighbor {
        Neighbor::Past => "onion-past",
        Neighbor::Future => "onion-future",
    };
    let texture = ctx.load_texture(label, image, TextureOptions::NEAREST);
    Some(Ghost { texture, neighbor })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::commands::{ApplyGeneratedAnimation, GeneratedFrameData};
    use pixhaus_core::{Command, LoopMode};

    /// A document with one animated 2x2 sprite of `frames` opaque-black frames. The ui
    /// crate forbids `unwrap`/`expect` even in tests, so failures `panic!` explicitly.
    fn anim_doc(frames: usize) -> Document {
        let data: Vec<GeneratedFrameData> = (0..frames)
            .map(|_| GeneratedFrameData {
                stride: 8,
                rgba: vec![0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255],
            })
            .collect();
        let mut doc = Document::new();
        let mut cmd = ApplyGeneratedAnimation::new("s".to_owned(), "idle".to_owned(), 2, 2, 8, LoopMode::Loop, data);
        assert!(cmd.apply(&mut doc).is_ok(), "the test animation applies");
        doc
    }

    fn active(doc: &Document) -> SpriteId {
        let Some(sprite) = doc.active_sprite() else {
            panic!("the test document has an active sprite");
        };
        sprite
    }

    fn nth_frame(doc: &Document, n: usize) -> FrameId {
        let frame = doc.active_sprite().and_then(|s| doc.sprite(s)).and_then(|s| s.frames().get(n)).map(|f| f.id);
        let Some(frame) = frame else {
            panic!("the test sprite has a frame {n}");
        };
        frame
    }

    #[test]
    fn a_middle_frame_ghosts_both_neighbors() {
        let ctx = egui::Context::default();
        let doc = anim_doc(3);
        let mut cache = OnionCache::default();
        let ghosts = cache.ghosts(&ctx, &doc, active(&doc), nth_frame(&doc, 1));
        assert_eq!(ghosts.len(), 2, "a middle frame ghosts the previous and next frames");
        assert!(ghosts.iter().any(|g| g.neighbor == Neighbor::Past));
        assert!(ghosts.iter().any(|g| g.neighbor == Neighbor::Future));
    }

    #[test]
    fn the_first_frame_ghosts_only_the_future() {
        let ctx = egui::Context::default();
        let doc = anim_doc(3);
        let mut cache = OnionCache::default();
        let ghosts = cache.ghosts(&ctx, &doc, active(&doc), nth_frame(&doc, 0));
        assert_eq!(ghosts.len(), 1, "the first frame has no past neighbor");
        assert_eq!(ghosts.first().map(|g| g.neighbor), Some(Neighbor::Future));
    }

    #[test]
    fn clear_then_request_rebuilds() {
        let ctx = egui::Context::default();
        let doc = anim_doc(3);
        let sprite = active(&doc);
        let center = nth_frame(&doc, 1);
        let mut cache = OnionCache::default();
        assert_eq!(cache.ghosts(&ctx, &doc, sprite, center).len(), 2);
        cache.clear();
        assert_eq!(
            cache.ghosts(&ctx, &doc, sprite, center).len(),
            2,
            "a cleared cache rebuilds on the next request"
        );
    }
}
