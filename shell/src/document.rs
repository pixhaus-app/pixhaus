//! The shell's document model: a `core` [`Project`] plus the pixel-buffer
//! store, the monotonic id allocator, and the active-frame cursor. Single
//! owner, mutated through `&mut self` — no locks (matches the repo rule against
//! `Arc<Mutex<>>` away from the app boundary).

use std::collections::HashMap;

use pixhaus_core::canvas::blend::blend;
use pixhaus_core::canvas::{LayerInput, PixelBuffer, composite_layers, composite_onto};
use pixhaus_core::project::{
    ActiveTarget, AiMetadata, Animation, AnimationId, BlendMode, Cel, CelData, Entity, EntityContent, EntityDefaults, EntityId, EntityKind, Frame, FrameIndex,
    FrameRange, FrameTag, Layer, LayerId, LayerKind, LoopDirection, NamedSprite, Palette, PaletteId, PixelBufferId, Project, Rgba, Size, Sprite, SpriteId,
    StateId, UserData,
};
use pixhaus_core::transforms::normalize::{ChromaKey, chroma_key};

use crate::editor::OnionConfig;

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
    /// Active layer within the active sprite — the target for drawing. `None`
    /// until a sprite is selected; set by [`Self::create_sprite`] / [`Self::select`].
    pub active_layer: Option<LayerId>,
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
            active_layer: None,
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

        // Populate a fresh sprite with one raster layer, one frame, an empty
        // cel bound to a transparent canvas-sized buffer, and a default
        // palette — so the canvas is editable and the layer/palette panels are
        // non-empty the moment the sprite is created.
        let layer_id = LayerId::new(self.alloc_id());
        let buffer_id = PixelBufferId::new(self.alloc_id());
        let palette_id = PaletteId::new(self.alloc_id());
        let mut sprite = Sprite::empty(sprite_id, name.clone(), canvas);
        sprite.layers.push(Layer::raster(layer_id, "Layer 1"));
        sprite.frames.push(Frame::default());
        sprite.cels.push(Cel::raster(layer_id, FrameIndex::new(0), buffer_id, canvas));
        sprite.palettes.push(Palette::from_colors(palette_id, "default", default_palette()));
        if let Ok(buf) = PixelBuffer::new(canvas.width, canvas.height) {
            self.pixel_buffers.insert(buffer_id, buf);
        }
        self.active_layer = Some(layer_id);

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

        let sprite_ref = SpriteRef { entity_id, state_id };
        self.select(sprite_ref);
        sprite_ref
    }

    /// Makes `sprite_ref` the active sprite and resets the frame cursor.
    /// Picks the topmost raster layer as the active drawing layer.
    pub fn select(&mut self, sprite_ref: SpriteRef) {
        self.project.active = ActiveTarget::State {
            entity_id: sprite_ref.entity_id,
            state_id: sprite_ref.state_id,
        };
        self.active_frame = FrameIndex::new(0);
        self.active_layer = self
            .active_sprite()
            .and_then(|s| s.layers.iter().rev().find(|l| matches!(l.kind, LayerKind::Raster)).map(|l| l.id));
    }

    /// The active sprite's first palette, if any.
    #[must_use]
    pub fn active_palette(&self) -> Option<&Palette> {
        self.active_sprite().and_then(|s| s.palettes.first())
    }

    /// Ensures the active sprite has a frame, a raster layer, and a raster cel
    /// at the active `(layer, frame)`, creating each as needed, and returns the
    /// id of the buffer the active cel paints into. Linked cels resolve to
    /// their source frame's buffer (editing the shared drawing, Aseprite-style).
    /// Returns `None` when there is no active sprite or the active layer is not
    /// a raster layer.
    pub fn ensure_drawable(&mut self) -> Option<PixelBufferId> {
        let sprite_id = self.project.active_sprite_id()?;
        let canvas = self.project.sprite(sprite_id)?.canvas;

        if self.project.sprite(sprite_id)?.frames.is_empty() {
            if let Some(s) = self.project.sprite_mut(sprite_id) {
                s.frames.push(Frame::default());
            }
            self.active_frame = FrameIndex::new(0);
        }

        let raster_active = self.active_layer.is_some_and(|l| {
            self.project
                .sprite(sprite_id)
                .and_then(|s| s.layers.iter().find(|ly| ly.id == l))
                .is_some_and(|ly| matches!(ly.kind, LayerKind::Raster))
        });
        let layer_id = if raster_active {
            self.active_layer?
        } else if let Some(existing) = self
            .project
            .sprite(sprite_id)?
            .layers
            .iter()
            .find(|ly| matches!(ly.kind, LayerKind::Raster))
            .map(|ly| ly.id)
        {
            existing
        } else {
            let new_id = LayerId::new(self.alloc_id());
            if let Some(s) = self.project.sprite_mut(sprite_id) {
                s.layers.push(Layer::raster(new_id, "Layer 1"));
            }
            new_id
        };
        self.active_layer = Some(layer_id);

        let frame = self.active_frame;
        let source = self.project.sprite(sprite_id)?.resolve_source_frame(layer_id, frame);

        if let Some(cel) = self.project.sprite(sprite_id)?.cel(layer_id, source) {
            return match cel.data {
                CelData::Raster { buffer, .. } => Some(buffer),
                _ => None,
            };
        }

        let buffer_id = PixelBufferId::new(self.alloc_id());
        let buf = PixelBuffer::new(canvas.width, canvas.height).ok()?;
        self.pixel_buffers.insert(buffer_id, buf);
        if let Some(s) = self.project.sprite_mut(sprite_id) {
            s.cels.push(Cel::raster(layer_id, source, buffer_id, canvas));
        }
        Some(buffer_id)
    }

    /// The buffer id the active `(layer, frame)` paints into, without creating
    /// anything. `None` if there is no raster cel there yet.
    #[must_use]
    pub fn active_buffer_id(&self) -> Option<PixelBufferId> {
        let sprite = self.active_sprite()?;
        let layer = self.active_layer?;
        let source = sprite.resolve_source_frame(layer, self.active_frame);
        match sprite.cel(layer, source)?.data {
            CelData::Raster { buffer, .. } => Some(buffer),
            _ => None,
        }
    }

    /// Composites the active frame including onion-skin ghosts of neighbouring
    /// frames when `onion.enabled`. Ghosts render behind the current frame,
    /// tinted and faded by distance. Returns `None` when there is no active
    /// sprite.
    #[must_use]
    pub fn composite_with_onion(&self, onion: &OnionConfig) -> Option<PixelBuffer> {
        let base = self.composite_active_frame()?;
        if !onion.enabled || (onion.prev == 0 && onion.next == 0) {
            return Some(base);
        }
        let sprite = self.active_sprite()?;
        let count = sprite.frames.len() as i64;
        if count <= 1 {
            return Some(base);
        }
        let here = i64::from(self.active_frame.get());
        let mut stack = PixelBuffer::new(base.width(), base.height()).ok()?;

        // Farthest ghosts first so nearer ones layer on top, current frame last.
        let mut ghosts: Vec<(i64, bool)> = Vec::new();
        for d in 1..=i64::from(onion.prev) {
            ghosts.push((here - d, true));
        }
        for d in 1..=i64::from(onion.next) {
            ghosts.push((here + d, false));
        }
        ghosts.sort_by_key(|(idx, _)| -(here - idx).abs());

        for (idx, is_prev) in ghosts {
            if idx < 0 || idx >= count {
                continue;
            }
            let Some(ghost) = self.composite_frame(FrameIndex::new(idx as u32)) else {
                continue;
            };
            let dist = (here - idx).unsigned_abs().max(1) as f32;
            let factor = (onion.opacity / dist).clamp(0.0, 1.0);
            let tint = if is_prev { onion.prev_tint } else { onion.next_tint };
            let tinted = tint_ghost(&ghost, tint, factor);
            let _ = composite_onto(
                &mut stack,
                &LayerInput {
                    buffer: &tinted,
                    mode: BlendMode::Normal,
                    opacity: 255,
                    visible: true,
                },
            );
        }
        let _ = composite_onto(
            &mut stack,
            &LayerInput {
                buffer: &base,
                mode: BlendMode::Normal,
                opacity: 255,
                visible: true,
            },
        );
        Some(stack)
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
                sprite_ref: SpriteRef { entity_id, state_id: named.id },
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

    /// Recomposites only the rectangle `(x, y, w, h)` of the active frame into
    /// `dst` (a full-canvas buffer), blending every visible layer for those
    /// pixels. This is the drawing hot path: after a brush dab edits a cel, the
    /// shell recomposites just the dirty rect across layers and uploads that
    /// rect — work bounded by the region, not the canvas (the 8K constraint).
    pub fn composite_region_into(&self, dst: &mut PixelBuffer, x: u32, y: u32, w: u32, h: u32) {
        let Some(sprite) = self.active_sprite() else {
            return;
        };
        let frame = self.active_frame;
        let x1 = (x + w).min(dst.width());
        let y1 = (y + h).min(dst.height());
        for py in y..y1 {
            for px in x..x1 {
                let mut acc = Rgba::transparent();
                for layer in &sprite.layers {
                    if !layer.visible || layer.opacity == 0 {
                        continue;
                    }
                    let source = sprite.resolve_source_frame(layer.id, frame);
                    let Some(cel) = sprite.cel(layer.id, source) else {
                        continue;
                    };
                    let CelData::Raster { buffer, .. } = &cel.data else {
                        continue;
                    };
                    let Some(buf) = self.pixel_buffers.get(buffer) else {
                        continue;
                    };
                    let Some(src) = buf.pixel(px, py) else { continue };
                    if src.a == 0 {
                        continue;
                    }
                    acc = blend(layer.blend_mode, src, acc, layer.opacity);
                }
                dst.set_pixel(px, py, acc);
            }
        }
    }

    /// Integrates a sequence of full-canvas frames into the active sprite as a
    /// new raster layer with one cel per frame, appends the frames to the
    /// timeline, and adds a [`FrameTag`] plus an [`Animation`] over the new
    /// range. Mirrors `app/src/commands/animation.rs::animation_integrate`
    /// without the IPC wrapper. Returns the tagged range.
    ///
    /// Every frame buffer must match the sprite's canvas size.
    #[allow(clippy::cast_possible_truncation)] // frame counts fit u32
    pub fn integrate_frames(&mut self, frames: Vec<PixelBuffer>, frame_duration_ms: u32, name: &str, loop_direction: LoopDirection) -> Option<FrameRange> {
        if frames.is_empty() {
            return None;
        }
        let canvas = self.active_sprite()?.canvas;

        // A pristine sprite (one frame, nothing drawn) hosts the animation from
        // frame 0 so it carries no leading blank frame; a sprite the user has
        // touched keeps the append behavior.
        let replace_seed = self.active_sprite_is_pristine();

        // Allocate every id before borrowing the sprite mutably.
        let layer_id = LayerId::new(self.alloc_id());
        let animation_id = AnimationId::new(self.alloc_id());
        let buffer_ids: Vec<PixelBufferId> = (0..frames.len()).map(|_| PixelBufferId::new(self.alloc_id())).collect();

        // Buffers of the discarded seed cel(s), removed after the borrow ends.
        let mut orphaned: Vec<PixelBufferId> = Vec::new();

        let range = {
            let sprite = self.active_sprite_mut()?;
            let start = if replace_seed {
                orphaned = sprite
                    .cels
                    .iter()
                    .filter_map(|cel| match cel.data {
                        CelData::Raster { buffer, .. } => Some(buffer),
                        _ => None,
                    })
                    .collect();
                sprite.frames.clear();
                sprite.cels.clear();
                sprite.layers.clear();
                sprite.frame_tags.clear();
                sprite.animations.clear();
                0
            } else {
                sprite.frames.len() as u32
            };
            sprite.layers.push(Layer::raster(layer_id, name));
            for (i, buffer_id) in buffer_ids.iter().enumerate() {
                let frame_index = FrameIndex::new(start + i as u32);
                sprite.frames.push(Frame {
                    duration_ms: frame_duration_ms,
                    duration_mul: 1.0,
                    user_data: UserData::default(),
                });
                sprite.cels.push(Cel::raster(layer_id, frame_index, *buffer_id, canvas));
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
        // Drop the discarded seed buffers so they don't leak.
        for buffer_id in orphaned {
            self.pixel_buffers.remove(&buffer_id);
        }
        // Show the first integrated frame.
        self.active_frame = range.start;
        Some(range)
    }

    /// Keys `key` out of `buffer_id` in place, returning the pre-key snapshot so
    /// the caller can record an undo entry. `None` when the buffer is missing.
    /// The keying itself is the pure [`chroma_key`](pixhaus_core::transforms::normalize::chroma_key);
    /// this just swaps the stored buffer and hands back the original.
    pub fn chroma_key_buffer(&mut self, buffer_id: PixelBufferId, key: ChromaKey) -> Option<PixelBuffer> {
        let before = self.pixel_buffers.get(&buffer_id)?.clone();
        let keyed = chroma_key(&before, key);
        self.pixel_buffers.insert(buffer_id, keyed);
        Some(before)
    }

    /// Replaces `buffer_id` with `new` (which must match the existing size),
    /// returning the pre-replace snapshot for undo. `None` on a missing buffer
    /// or a size mismatch. Used to land an AI background-removal result on a cel.
    pub fn replace_buffer(&mut self, buffer_id: PixelBufferId, new: PixelBuffer) -> Option<PixelBuffer> {
        let before = self.pixel_buffers.get(&buffer_id)?.clone();
        if new.width() != before.width() || new.height() != before.height() {
            return None;
        }
        self.pixel_buffers.insert(buffer_id, new);
        Some(before)
    }

    /// The active-layer cel buffers across every frame, for a whole-animation
    /// background-removal pass. Falls back to the single active cel when no
    /// layer is active.
    #[must_use]
    pub fn active_layer_frame_buffers(&self) -> Vec<PixelBufferId> {
        let Some(sprite) = self.active_sprite() else {
            return Vec::new();
        };
        let Some(layer) = self.active_layer else {
            return self.active_buffer_id().into_iter().collect();
        };
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for frame in 0..sprite.frames.len() as u32 {
            let source = sprite.resolve_source_frame(layer, FrameIndex::new(frame));
            if let Some(cel) = sprite.cel(layer, source) {
                if let CelData::Raster { buffer, .. } = cel.data {
                    if seen.insert(buffer) {
                        out.push(buffer);
                    }
                }
            }
        }
        out
    }

    /// Whether the active sprite is untouched: a single frame whose raster cels
    /// are all fully transparent. Such a sprite is safe to rebuild around an
    /// integrated animation rather than appending behind its seed frame.
    fn active_sprite_is_pristine(&self) -> bool {
        let Some(sprite) = self.active_sprite() else {
            return false;
        };
        if sprite.frames.len() != 1 {
            return false;
        }
        sprite.cels.iter().all(|cel| match cel.data {
            CelData::Raster { buffer, .. } => self.pixel_buffers.get(&buffer).is_none_or(|b| b.pixels().all(|p| p.a == 0)),
            _ => true,
        })
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
            (0..sprite.frames.len() as u32).map(FrameIndex::new).collect()
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

/// Builds an onion ghost: every opaque pixel takes `tint`'s colour with its
/// alpha scaled by `factor`. Transparent pixels stay transparent.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn tint_ghost(frame: &PixelBuffer, tint: Rgba, factor: f32) -> PixelBuffer {
    let mut out = frame.clone();
    for y in 0..out.height() {
        for x in 0..out.width() {
            if let Some(p) = out.pixel(x, y) {
                if p.a == 0 {
                    continue;
                }
                let a = (f32::from(p.a) * factor).clamp(0.0, 255.0) as u8;
                out.set_pixel(x, y, Rgba::new(tint.r, tint.g, tint.b, a));
            }
        }
    }
    out
}

/// A compact 16-colour starter palette (transparent index 0, then a balanced
/// pixel-art ramp). New sprites get this so the palette panel and the
/// foreground swatch have something to work with immediately.
fn default_palette() -> Vec<Rgba> {
    vec![
        Rgba::transparent(),
        Rgba::opaque(20, 20, 28),
        Rgba::opaque(48, 52, 70),
        Rgba::opaque(90, 100, 120),
        Rgba::opaque(160, 170, 185),
        Rgba::opaque(235, 240, 245),
        Rgba::opaque(180, 60, 60),
        Rgba::opaque(230, 110, 70),
        Rgba::opaque(240, 190, 90),
        Rgba::opaque(120, 200, 90),
        Rgba::opaque(70, 160, 110),
        Rgba::opaque(70, 130, 200),
        Rgba::opaque(60, 80, 170),
        Rgba::opaque(140, 90, 200),
        Rgba::opaque(210, 110, 180),
        Rgba::opaque(120, 80, 60),
    ]
}

/// Maps `i/total` around the hue wheel to an approximate RGB triple. Only used
/// by [`DocumentStore::add_demo_animation`].
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::many_single_char_names)]
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
        let range = doc.integrate_frames(frames, 100, "walk", LoopDirection::Forward).expect("integrated range");

        // The fresh sprite is pristine, so the animation replaces its seed frame
        // and occupies frames 0..=3 on a single layer — no leading blank.
        assert_eq!(range.start, FrameIndex::new(0));
        assert_eq!(range.end, FrameIndex::new(3));

        let sprite = doc.active_sprite().expect("sprite");
        assert_eq!(sprite.frames.len(), 4);
        assert_eq!(sprite.layers.len(), 1);
        assert_eq!(sprite.cels.len(), 4);
        assert_eq!(sprite.frame_tags.len(), 1);
        assert_eq!(sprite.animations.len(), 1);
        // The seed buffer was dropped; only the four integrated frames remain.
        assert_eq!(doc.pixel_buffers.len(), 4);

        // Each integrated frame composites to the solid color.
        let composited = doc.composite_frame(FrameIndex::new(2)).expect("composite");
        assert_eq!(composited.width(), 8);

        // Forward play order over the tagged range.
        assert_eq!(
            doc.active_play_order(),
            vec![FrameIndex::new(0), FrameIndex::new(1), FrameIndex::new(2), FrameIndex::new(3)]
        );
        assert_eq!(doc.frame_duration_ms(FrameIndex::new(0)), 100);
    }

    #[test]
    fn integrate_frames_appends_when_the_sprite_has_been_drawn_on() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        // Drawing on the seed frame makes the sprite non-pristine, so the
        // animation appends after it instead of replacing it.
        let seed = doc.active_buffer_id().expect("seed buffer");
        doc.pixel_buffers
            .get_mut(&seed)
            .expect("seed buffer present")
            .set_pixel(0, 0, pixhaus_core::project::Rgba::new(1, 2, 3, 255));

        let frames: Vec<PixelBuffer> = (0..4)
            .map(|_| PixelBuffer::filled(8, 8, pixhaus_core::project::Rgba::new(10, 20, 30, 255)).unwrap())
            .collect();
        let range = doc.integrate_frames(frames, 100, "walk", LoopDirection::Forward).expect("integrated range");

        assert_eq!(range.start, FrameIndex::new(1));
        assert_eq!(range.end, FrameIndex::new(4));
        let sprite = doc.active_sprite().expect("sprite");
        assert_eq!(sprite.frames.len(), 5);
        assert_eq!(sprite.layers.len(), 2);
        assert_eq!(doc.pixel_buffers.len(), 5);
    }

    #[test]
    fn ensure_drawable_returns_the_default_cel_buffer() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let id = doc.ensure_drawable().expect("drawable buffer");
        assert!(doc.pixel_buffers.contains_key(&id));
        assert_eq!(doc.active_buffer_id(), Some(id));
    }

    #[test]
    fn pixel_region_edit_undo_redo_round_trips() {
        use crate::commands::{PixelRegionEdit, extract_region};
        use pixhaus_core::undo::History;

        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let id = doc.ensure_drawable().unwrap();
        let before = doc.pixel_buffers.get(&id).unwrap().clone();
        let red = Rgba::opaque(255, 0, 0);
        doc.pixel_buffers.get_mut(&id).unwrap().set_pixel(2, 3, red);

        let cmd = PixelRegionEdit {
            buffer_id: id,
            x: 0,
            y: 0,
            w: 8,
            h: 8,
            before: extract_region(&before, 0, 0, 8, 8),
            after: extract_region(doc.pixel_buffers.get(&id).unwrap(), 0, 0, 8, 8),
            label: "test".into(),
        };
        let mut history: History<DocumentStore> = History::new();
        history.push(Box::new(cmd), &mut doc).unwrap();
        assert_eq!(doc.pixel_buffers.get(&id).unwrap().pixel(2, 3), Some(red));
        history.undo(&mut doc).unwrap();
        assert_eq!(doc.pixel_buffers.get(&id).unwrap().pixel(2, 3), Some(Rgba::transparent()));
        history.redo(&mut doc).unwrap();
        assert_eq!(doc.pixel_buffers.get(&id).unwrap().pixel(2, 3), Some(red));
    }

    #[test]
    fn remove_background_keys_flat_buffer_and_undo_restores() {
        use crate::commands::{PixelRegionEdit, extract_region};
        use pixhaus_core::transforms::normalize::ChromaKey;
        use pixhaus_core::undo::History;

        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let id = doc.ensure_drawable().unwrap();
        // Flat magenta background with one opaque subject pixel.
        {
            let buf = doc.pixel_buffers.get_mut(&id).unwrap();
            for y in 0..8 {
                for x in 0..8 {
                    buf.set_pixel(x, y, Rgba::opaque(255, 0, 255));
                }
            }
            buf.set_pixel(3, 3, Rgba::opaque(10, 20, 30));
        }
        let original = doc.pixel_buffers.get(&id).unwrap().clone();

        // Key the magenta out, recording the pre-key snapshot.
        let before = doc.chroma_key_buffer(id, ChromaKey::magenta()).expect("keyed buffer");
        assert_eq!(doc.pixel_buffers.get(&id).unwrap().pixel(0, 0).unwrap().a, 0, "background cleared");
        assert_eq!(doc.pixel_buffers.get(&id).unwrap().pixel(3, 3), Some(Rgba::opaque(10, 20, 30)), "subject kept");

        // Wrap the change as an undo entry and confirm undo restores the buffer.
        let cmd = PixelRegionEdit {
            buffer_id: id,
            x: 0,
            y: 0,
            w: 8,
            h: 8,
            before: extract_region(&before, 0, 0, 8, 8),
            after: extract_region(doc.pixel_buffers.get(&id).unwrap(), 0, 0, 8, 8),
            label: "Remove background".into(),
        };
        let mut history: History<DocumentStore> = History::new();
        history.push(Box::new(cmd), &mut doc).unwrap();
        history.undo(&mut doc).unwrap();
        assert_eq!(
            doc.pixel_buffers.get(&id).unwrap().as_bytes(),
            original.as_bytes(),
            "undo restores the background"
        );
    }

    #[test]
    fn composite_region_matches_full_composite() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let id = doc.ensure_drawable().unwrap();
        doc.pixel_buffers.get_mut(&id).unwrap().set_pixel(4, 4, Rgba::opaque(10, 20, 30));
        let full = doc.composite_active_frame().unwrap();
        let mut region = PixelBuffer::new(8, 8).unwrap();
        doc.composite_region_into(&mut region, 3, 3, 3, 3);
        for y in 3..6 {
            for x in 3..6 {
                assert_eq!(region.pixel(x, y), full.pixel(x, y), "({x},{y})");
            }
        }
    }

    #[test]
    fn onion_disabled_matches_base_composite() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let base = doc.composite_active_frame().unwrap();
        let onion = crate::editor::OnionConfig::default();
        let with = doc.composite_with_onion(&onion).unwrap();
        assert_eq!(base.as_bytes(), with.as_bytes());
    }
}
