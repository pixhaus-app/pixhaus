//! The document: the authoritative project state and the single [`Command`] target.
//!
//! A [`Document`] bundles structural data (sprites and their layers) with the
//! [`PixelBufferStore`] that owns their pixels, so one [`Command`](crate::Command)
//! type covers both structural and pixel edits. Every mutation flows through a
//! command and bumps [`Document::revision`], which the renderer watches to decide
//! when to recomposite.

use crate::animation::AnimationClip;
use crate::buffer_store::PixelBufferStore;
use crate::ids::{ClipId, FrameId, IdCounter, LayerId, PixelBufferId, SpriteId};
use crate::pixel::BlendMode;

/// Default canvas size `(width, height)` for a new sprite or the empty stage.
///
/// 512x512 is a comfortable size for testing the generation pipeline by eye while
/// staying cheap to composite and upload. The shell uses it both as the size it asks
/// a provider to generate and as the empty-document board fallback, so the two never
/// drift.
pub const DEFAULT_CANVAS_SIZE: (u32, u32) = (512, 512);

/// Default per-frame hold time, in milliseconds, for a frame whose timing has not
/// been set otherwise. A still sprite has one frame and the value is moot; animation
/// commands set each frame's duration from the clip's fps.
pub const DEFAULT_FRAME_DURATION_MS: u32 = 100;

/// One raster layer: metadata plus a handle to its pixels in the buffer store.
#[derive(Clone, Debug)]
pub struct Layer {
    /// Stable id within the owning sprite.
    pub id: LayerId,
    /// User-facing layer name (project content, not a localization key).
    pub name: String,
    /// Whether the layer contributes to the composite.
    pub visible: bool,
    /// Layer opacity, 0.0 transparent .. 1.0 opaque.
    pub opacity: f32,
    /// How the layer composites over the layers beneath it.
    pub blend: BlendMode,
    /// Handle into the document's [`PixelBufferStore`].
    pub buffer: PixelBufferId,
}

/// One animation frame: a stack of layers plus its hold time.
///
/// A frame owns the layer stack a sprite used to own directly. A still sprite is a
/// one-frame sprite; an animated sprite has many frames. The migration to true
/// sparse cels (one cel per layer-frame intersection, bible 9.6) stays internal to
/// this type when it lands.
#[derive(Clone, Debug)]
pub struct Frame {
    /// Stable id within the owning sprite.
    pub id: FrameId,
    /// Layers, bottom-first (index 0 is composited first).
    pub layers: Vec<Layer>,
    /// Hold time in milliseconds (per-frame timing; clip fps sets the playback rate).
    pub duration_ms: u32,
    pub(crate) layer_counter: IdCounter,
}

impl Frame {
    /// Mints a fresh layer id for this frame.
    pub(crate) fn mint_layer_id(&mut self) -> LayerId {
        LayerId(self.layer_counter.mint())
    }

    /// Adds a fully-opaque, visible, normal-blend layer wrapping `buffer` and returns
    /// its id. The common case for generated content, where one frame is one image.
    pub(crate) fn add_layer(&mut self, name: String, buffer: PixelBufferId) -> LayerId {
        let id = self.mint_layer_id();
        self.layers.push(Layer {
            id,
            name,
            visible: true,
            opacity: 1.0,
            blend: BlendMode::Normal,
            buffer,
        });
        id
    }
}

/// One sprite: a sized sequence of frames plus the clips that name ranges of them.
#[derive(Clone, Debug)]
pub struct Sprite {
    /// Stable id within the document.
    pub id: SpriteId,
    /// User-facing sprite name (project content, not a localization key).
    pub name: String,
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
    /// Frames in playback order. Always at least one; a still sprite has exactly one.
    pub frames: Vec<Frame>,
    /// Named frame-range clips over `frames` (idle, walk, ...).
    pub clips: Vec<AnimationClip>,
    /// The frame the editor and compositor currently show.
    pub active_frame: FrameId,
    pub(crate) frame_counter: IdCounter,
    pub(crate) clip_counter: IdCounter,
}

impl Sprite {
    /// Builds a sprite with a single empty frame, made active. Callers add layers to
    /// the frame; a still sprite stays a one-frame sprite.
    pub(crate) fn new_single_frame(id: SpriteId, name: String, width: u32, height: u32) -> Self {
        let mut frame_counter = IdCounter::default();
        let frame_id = FrameId(frame_counter.mint());
        let frame = Frame {
            id: frame_id,
            layers: Vec::new(),
            duration_ms: DEFAULT_FRAME_DURATION_MS,
            layer_counter: IdCounter::default(),
        };
        Self {
            id,
            name,
            width,
            height,
            frames: vec![frame],
            clips: Vec::new(),
            active_frame: frame_id,
            frame_counter,
            clip_counter: IdCounter::default(),
        }
    }

    /// Pushes a new empty frame with the given hold time and returns its id. Does not
    /// change the active frame.
    pub(crate) fn push_frame(&mut self, duration_ms: u32) -> FrameId {
        let id = FrameId(self.frame_counter.mint());
        self.frames.push(Frame {
            id,
            layers: Vec::new(),
            duration_ms,
            layer_counter: IdCounter::default(),
        });
        id
    }

    /// Adds a clip naming an inclusive frame-index range and returns its id.
    pub(crate) fn push_clip(&mut self, name: String, start: u32, end: u32, fps: u16, loop_mode: crate::animation::LoopMode) -> ClipId {
        let id = ClipId(self.clip_counter.mint());
        self.clips.push(AnimationClip {
            id,
            name,
            start,
            end,
            fps,
            loop_mode,
        });
        id
    }

    /// The frames in playback order.
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    /// The clips defined over this sprite's frames.
    pub fn clips(&self) -> &[AnimationClip] {
        &self.clips
    }

    /// Borrows the frame for `id`, or `None` if absent.
    pub fn frame(&self, id: FrameId) -> Option<&Frame> {
        self.frames.iter().find(|f| f.id == id)
    }

    /// The active frame, or `None` if the active id is somehow not present (a
    /// well-formed sprite always returns `Some`).
    pub fn active_frame(&self) -> Option<&Frame> {
        self.frame(self.active_frame)
    }

    /// Mutably borrows the active frame, or `None` if the active id is not present.
    pub fn active_frame_mut(&mut self) -> Option<&mut Frame> {
        let id = self.active_frame;
        self.frames.iter_mut().find(|f| f.id == id)
    }
}

/// The authoritative project state and the single [`Command`](crate::Command)
/// target. Holds the sprites, the pixel-buffer store, the active sprite, and a
/// revision counter bumped on every mutation.
#[derive(Clone, Debug, Default)]
pub struct Document {
    pub(crate) sprites: Vec<Sprite>,
    pub(crate) sprite_counter: IdCounter,
    pub(crate) buffers: PixelBufferStore,
    pub(crate) active_sprite: Option<SpriteId>,
    pub(crate) revision: u64,
}

impl Document {
    /// An empty document.
    pub fn new() -> Self {
        Self::default()
    }

    /// The sprites, in list order.
    pub fn sprites(&self) -> &[Sprite] {
        &self.sprites
    }

    /// Borrows the sprite for `id`, or `None` if absent.
    pub fn sprite(&self, id: SpriteId) -> Option<&Sprite> {
        self.sprites.iter().find(|s| s.id == id)
    }

    /// Read access to the pixel-buffer store (the renderer composites from here).
    pub fn buffers(&self) -> &PixelBufferStore {
        &self.buffers
    }

    /// The active sprite, if any.
    pub fn active_sprite(&self) -> Option<SpriteId> {
        self.active_sprite
    }

    /// The active sprite's `(width, height)`, or `None` if there is no active sprite.
    pub fn active_sprite_size(&self) -> Option<(u32, u32)> {
        self.active_sprite.and_then(|id| self.sprite(id)).map(|s| (s.width, s.height))
    }

    /// A counter bumped on every mutation; the renderer recomposites when it changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    // --- command-only mutators (in-crate; mutation flows through Command) ---

    pub(crate) fn mint_sprite_id(&mut self) -> SpriteId {
        SpriteId(self.sprite_counter.mint())
    }

    pub(crate) fn bump_revision(&mut self) {
        self.revision += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_document_has_no_active_sprite() {
        let doc = Document::new();
        assert!(doc.sprites().is_empty());
        assert_eq!(doc.active_sprite(), None);
        assert_eq!(doc.active_sprite_size(), None);
        assert_eq!(doc.revision(), 0);
    }
}
