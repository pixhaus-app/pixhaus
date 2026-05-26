//! The shell's document model: a `core` [`Project`] plus the pixel-buffer
//! store, the monotonic id allocator, and the active-frame cursor. Single
//! owner, mutated through `&mut self` — no locks (matches the repo rule against
//! `Arc<Mutex<>>` away from the app boundary).

use std::collections::HashMap;

use pixhaus_core::canvas::{composite_layers, LayerInput, PixelBuffer};
use pixhaus_core::project::{
    ActiveTarget, AiMetadata, Animation, AnimationId, Cel, CelData, Entity, EntityContent,
    EntityDefaults, EntityId, EntityKind, Frame, FrameIndex, FrameRange, FrameTag, Layer, LayerId,
    LoopDirection, NamedSprite, PixelBufferId, Project, Size, Sprite, SpriteId, StateId, UserData,
};

/// Identifies a sprite by its containing entity and state, the address the
/// library panel selects and the canvas targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpriteRef {
    /// Library entity holding the sprite.
    pub entity_id: EntityId,
    /// State within the entity.
    pub state_id: StateId,
}

/// One row in the library panel.
pub struct SpriteListItem {
    /// Address used to select the sprite.
    pub sprite_ref: SpriteRef,
    /// Display name.
    pub name: String,
    /// Canvas size.
    pub canvas: Size,
    /// Whether this is the active sprite.
    pub selected: bool,
}

/// UI-thread document state. Single owner of the project.
pub struct DocumentStore {
    /// The core project: library entities, each holding sprite states.
    pub project: Project,
    /// Pixel buffers referenced by cels, keyed by id. The project model holds
    /// only [`PixelBufferId`] handles; the bytes live here.
    pub pixel_buffers: HashMap<PixelBufferId, PixelBuffer>,
    /// Monotonic id allocator shared across every id type, mirroring the `app`
    /// crate's `next_id`.
    next_id: u32,
    /// Active frame index within the active sprite.
    pub active_frame: FrameIndex,
    /// Approved reference sheet per sprite, as PNG bytes. The "canonical sheet"
    /// the animation pipeline reads as its character anchor (P5).
    anchors: HashMap<SpriteId, Vec<u8>>,
}

impl DocumentStore {
    /// A fresh, empty project.
    #[must_use]
    pub fn new() -> Self {
        Self {
            project: Project::new("untitled"),
            pixel_buffers: HashMap::new(),
            next_id: 1,
            active_frame: FrameIndex::new(0),
            anchors: HashMap::new(),
        }
    }

    /// The entity id of the active sprite's state, used as the reference-sheet
    /// verb's `entity_id` input.
    #[must_use]
    pub fn active_entity_id(&self) -> Option<EntityId> {
        match self.project.active {
            ActiveTarget::State { entity_id, .. } => Some(entity_id),
            _ => None,
        }
    }

    /// Stores `png` as the active sprite's approved reference sheet (anchor).
    pub fn set_active_anchor(&mut self, png: Vec<u8>) {
        if let Some(id) = self.project.active_sprite_id() {
            self.anchors.insert(id, png);
        }
    }

    /// The active sprite's approved reference sheet, if one was approved.
    #[must_use]
    pub fn active_anchor(&self) -> Option<&[u8]> {
        let id = self.project.active_sprite_id()?;
        self.anchors.get(&id).map(Vec::as_slice)
    }

    /// Allocates a fresh monotonic id.
    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Creates an empty sprite wrapped in a new `Custom` library entity and
    /// makes it active. Mirrors `app/src/commands/project.rs::sprite_add` and
    /// `install_sprite_as_new_entity`, without the IPC wrapper. Returns the
    /// address of the new sprite.
    pub fn create_sprite(&mut self, name: impl Into<String>, canvas: Size) -> SpriteRef {
        let name = name.into();
        let sprite_id = SpriteId::new(self.alloc_id());
        let entity_id = EntityId::new(self.alloc_id());
        let state_id = StateId::new(self.alloc_id());

        let sprite = Sprite::empty(sprite_id, name.clone(), canvas);
        self.project.library.entities.push(Entity {
            id: entity_id,
            kind: EntityKind::Custom("Sprite".into()),
            name,
            group_id: None,
            tags: Vec::new(),
            defaults: EntityDefaults::default(),
            content: EntityContent::Sprites {
                states: vec![NamedSprite {
                    id: state_id,
                    state_name: "primary".into(),
                    sprite,
                    engine_tags: Vec::new(),
                }],
                reference_sheet: None,
            },
            ai: AiMetadata::default(),
            user_data: UserData::default(),
            created_at: 0,
            updated_at: 0,
        });

        let sprite_ref = SpriteRef {
            entity_id,
            state_id,
        };
        self.select(sprite_ref);
        sprite_ref
    }

    /// Makes `sprite_ref` the active sprite and resets the frame cursor.
    pub fn select(&mut self, sprite_ref: SpriteRef) {
        self.project.active = ActiveTarget::State {
            entity_id: sprite_ref.entity_id,
            state_id: sprite_ref.state_id,
        };
        self.active_frame = FrameIndex::new(0);
    }

    /// The active sprite, if any.
    #[must_use]
    pub fn active_sprite(&self) -> Option<&Sprite> {
        let id = self.project.active_sprite_id()?;
        self.project.sprite(id)
    }

    /// The active sprite, mutably.
    pub fn active_sprite_mut(&mut self) -> Option<&mut Sprite> {
        let id = self.project.active_sprite_id()?;
        self.project.sprite_mut(id)
    }

    /// Rows for the library panel, in library order.
    #[must_use]
    pub fn sprite_list(&self) -> Vec<SpriteListItem> {
        let active = self.project.active_sprite_id();
        self.project
            .sprites_iter()
            .map(|(named, entity_id)| SpriteListItem {
                sprite_ref: SpriteRef {
                    entity_id,
                    state_id: named.id,
                },
                name: named.sprite.name.clone(),
                canvas: named.sprite.canvas,
                selected: active == Some(named.sprite.id),
            })
            .collect()
    }

    /// Composites the active sprite's frame at `frame` into a flat RGBA buffer
    /// the renderer can upload. Returns `None` when there is no active sprite.
    ///
    /// All slice cels are full-canvas (position 0,0, size = canvas), so each
    /// cel's buffer is passed directly as a [`LayerInput`]. An empty sprite (no
    /// layers/frames) composites to a transparent canvas — the checkerboard
    /// shows at the sprite's size.
    #[must_use]
    pub fn composite_frame(&self, frame: FrameIndex) -> Option<PixelBuffer> {
        let sprite = self.active_sprite()?;
        let width = sprite.canvas.width;
        let height = sprite.canvas.height;

        let mut inputs: Vec<LayerInput<'_>> = Vec::new();
        for layer in &sprite.layers {
            let source = sprite.resolve_source_frame(layer.id, frame);
            let Some(cel) = sprite.cel(layer.id, source) else {
                continue;
            };
            let CelData::Raster { buffer, .. } = &cel.data else {
                continue;
            };
            let Some(pixels) = self.pixel_buffers.get(buffer) else {
                continue;
            };
            inputs.push(LayerInput {
                buffer: pixels,
                mode: layer.blend_mode,
                opacity: layer.opacity,
                visible: layer.visible,
            });
        }

        match composite_layers(width, height, &inputs) {
            Ok(buffer) => Some(buffer),
            Err(err) => {
                tracing::error!(%err, "compositing active frame failed");
                None
            }
        }
    }

    /// Composites the current active frame.
    #[must_use]
    pub fn composite_active_frame(&self) -> Option<PixelBuffer> {
        self.composite_frame(self.active_frame)
    }

    /// Integrates a sequence of full-canvas frames into the active sprite as a
    /// new raster layer with one cel per frame, appends the frames to the
    /// timeline, and adds a [`FrameTag`] plus an [`Animation`] over the new
    /// range. Mirrors `app/src/commands/animation.rs::animation_integrate`
    /// without the IPC wrapper. Returns the tagged range.
    ///
    /// Every frame buffer must match the sprite's canvas size.
    #[allow(clippy::cast_possible_truncation)] // frame counts fit u32
    pub fn integrate_frames(
        &mut self,
        frames: Vec<PixelBuffer>,
        frame_duration_ms: u32,
        name: &str,
        loop_direction: LoopDirection,
    ) -> Option<FrameRange> {
        if frames.is_empty() {
            return None;
        }
        let canvas = self.active_sprite()?.canvas;

        // Allocate every id before borrowing the sprite mutably.
        let layer_id = LayerId::new(self.alloc_id());
        let animation_id = AnimationId::new(self.alloc_id());
        let buffer_ids: Vec<PixelBufferId> = (0..frames.len())
            .map(|_| PixelBufferId::new(self.alloc_id()))
            .collect();

        let range = {
            let sprite = self.active_sprite_mut()?;
            let start = sprite.frames.len() as u32;
            sprite.layers.push(Layer::raster(layer_id, name));
            for (i, buffer_id) in buffer_ids.iter().enumerate() {
                let frame_index = FrameIndex::new(start + i as u32);
                sprite.frames.push(Frame {
                    duration_ms: frame_duration_ms,
                    duration_mul: 1.0,
                    user_data: UserData::default(),
                });
                sprite
                    .cels
                    .push(Cel::raster(layer_id, frame_index, *buffer_id, canvas));
            }
            let end = start + buffer_ids.len() as u32 - 1;
            let range = FrameRange::new(FrameIndex::new(start), FrameIndex::new(end));
            sprite.frame_tags.push(FrameTag {
                name: name.to_owned(),
                range,
                loop_direction,
                repeat: 0,
                user_data: UserData::default(),
            });
            sprite.animations.push(Animation {
                id: animation_id,
                name: name.to_owned(),
                range,
                loop_direction,
                speed_multiplier: 1.0,
                user_data: UserData::default(),
            });
            range
        };

        // Store the pixel buffers now the sprite borrow has ended.
        for (buffer_id, buffer) in buffer_ids.into_iter().zip(frames) {
            self.pixel_buffers.insert(buffer_id, buffer);
        }
        // Show the first integrated frame.
        self.active_frame = range.start;
        Some(range)
    }

    /// The frame order playback should follow for the active sprite: the first
    /// frame tag's expanded loop order, or all frames forward when untagged.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // frame counts fit u32
    pub fn active_play_order(&self) -> Vec<FrameIndex> {
        let Some(sprite) = self.active_sprite() else {
            return Vec::new();
        };
        if let Some(tag) = sprite.frame_tags.first() {
            tag.loop_direction
                .play_order(tag.range.start.get(), tag.range.end.get())
                .into_iter()
                .map(FrameIndex::new)
                .collect()
        } else {
            (0..sprite.frames.len() as u32)
                .map(FrameIndex::new)
                .collect()
        }
    }

    /// Effective on-screen duration of `frame` in the active sprite, or 100ms.
    #[must_use]
    pub fn frame_duration_ms(&self, frame: FrameIndex) -> u32 {
        self.active_sprite()
            .and_then(|s| s.frames.get(frame.get() as usize))
            .map_or(100, Frame::effective_duration_ms)
    }

    /// Number of frames in the active sprite.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.active_sprite().map_or(0, |s| s.frames.len())
    }

    /// Dev helper: integrates a hand-built multi-frame animation (solid color
    /// frames that sweep hue) so playback can be demonstrated before the AI
    /// pipeline lands. Replaced by real generated frames in P5.
    pub fn add_demo_animation(&mut self) {
        const COUNT: u32 = 8;
        let Some(sprite) = self.active_sprite() else {
            return;
        };
        let (w, h) = (sprite.canvas.width, sprite.canvas.height);
        let mut frames = Vec::new();
        for i in 0..COUNT {
            let mut pixels = vec![0u8; (w * h * 4) as usize];
            let (red, green, blue) = hue_rgb(i, COUNT);
            for px in pixels.chunks_exact_mut(4) {
                px[0] = red;
                px[1] = green;
                px[2] = blue;
                px[3] = 255;
            }
            if let Ok(buf) = PixelBuffer::from_raw(w, h, w * 4, pixels) {
                frames.push(buf);
            }
        }
        self.integrate_frames(frames, 120, "demo", LoopDirection::Forward);
    }
}

/// Maps `i/total` around the hue wheel to an approximate RGB triple. Only used
/// by [`DocumentStore::add_demo_animation`].
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names
)]
fn hue_rgb(i: u32, total: u32) -> (u8, u8, u8) {
    let h = (f32::from(i as u16) / f32::from(total as u16)) * 6.0;
    let x = (1.0 - (h % 2.0 - 1.0).abs()) * 255.0;
    let x = x as u8;
    match h as u32 {
        0 => (255, x, 0),
        1 => (x, 255, 0),
        2 => (0, 255, x),
        3 => (0, x, 255),
        4 => (x, 0, 255),
        _ => (255, 0, x),
    }
}

impl Default for DocumentStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_sprite_makes_it_active() {
        let mut doc = DocumentStore::new();
        let r = doc.create_sprite("hero", Size::new(32, 32));
        let sprite = doc.active_sprite().expect("active sprite");
        assert_eq!(sprite.name, "hero");
        assert_eq!(sprite.canvas, Size::new(32, 32));
        assert_eq!(doc.sprite_list().len(), 1);
        assert!(doc.sprite_list()[0].selected);
        assert_eq!(doc.sprite_list()[0].sprite_ref, r);
    }

    #[test]
    fn ids_are_monotonic_and_unique() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("a", Size::new(8, 8));
        doc.create_sprite("b", Size::new(8, 8));
        assert_eq!(doc.sprite_list().len(), 2);
        let refs: Vec<_> = doc.sprite_list().iter().map(|i| i.sprite_ref).collect();
        assert_ne!(refs[0].entity_id, refs[1].entity_id);
        assert_ne!(refs[0].state_id, refs[1].state_id);
    }

    #[test]
    fn empty_sprite_composites_to_canvas_sized_transparent() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("blank", Size::new(16, 24));
        let frame = doc.composite_active_frame().expect("composite");
        assert_eq!(frame.width(), 16);
        assert_eq!(frame.height(), 24);
    }

    #[test]
    fn integrate_frames_builds_layer_cels_tag_and_animation() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let frames: Vec<PixelBuffer> = (0..4)
            .map(|_| PixelBuffer::filled(8, 8, pixhaus_core::project::Rgba::new(10, 20, 30, 255)).unwrap())
            .collect();
        let range = doc
            .integrate_frames(frames, 100, "walk", LoopDirection::Forward)
            .expect("integrated range");

        assert_eq!(range.start, FrameIndex::new(0));
        assert_eq!(range.end, FrameIndex::new(3));

        let sprite = doc.active_sprite().expect("sprite");
        assert_eq!(sprite.frames.len(), 4);
        assert_eq!(sprite.layers.len(), 1);
        assert_eq!(sprite.cels.len(), 4);
        assert_eq!(sprite.frame_tags.len(), 1);
        assert_eq!(sprite.animations.len(), 1);
        assert_eq!(doc.pixel_buffers.len(), 4);

        // Each integrated frame composites to the solid color.
        let composited = doc.composite_frame(FrameIndex::new(2)).expect("composite");
        assert_eq!(composited.width(), 8);

        // Forward play order over the tagged range.
        assert_eq!(
            doc.active_play_order(),
            vec![
                FrameIndex::new(0),
                FrameIndex::new(1),
                FrameIndex::new(2),
                FrameIndex::new(3)
            ]
        );
        assert_eq!(doc.frame_duration_ms(FrameIndex::new(0)), 100);
    }
}
