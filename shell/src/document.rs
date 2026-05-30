//! The shell's document model: a `core` [`Project`] plus the pixel-buffer
//! store, the monotonic id allocator, and the active-frame cursor. Single
//! owner, mutated through `&mut self` — no locks (matches the repo rule against
//! `Arc<Mutex<>>` away from the app boundary).

use std::collections::{HashMap, HashSet};

use pixhaus_core::canvas::{LayerInput, PixelBuffer, composite_layers, composite_onto, composite_region};
use pixhaus_core::project::{
    ActiveTarget, AiMetadata, Animation, AnimationId, BlendMode, Cel, CelData, Entity, EntityContent, EntityDefaults, EntityGroup, EntityId, EntityKind, Frame,
    FrameIndex, FrameRange, FrameTag, GroupId, Layer, LayerId, LayerKind, LoopDirection, NamedSprite, Palette, PaletteId, PixelBufferId, Project, Rgba, Size,
    Sprite, SpriteId, StateId, UserData,
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

/// One rendered row of the library tree, flattened depth-first with folders
/// before sprites at each level. Built by [`DocumentStore::library_rows`].
pub enum LibraryRow {
    /// A folder header.
    Group {
        /// Folder id.
        id: GroupId,
        /// Display name.
        name: String,
        /// Indent depth (0 = top level).
        depth: u16,
        /// Whether the folder is collapsed (its subtree is omitted).
        collapsed: bool,
        /// Whether the folder contains any sprites or sub-folders.
        has_children: bool,
    },
    /// A sprite row.
    Sprite {
        /// Address used to select the sprite.
        sprite_ref: SpriteRef,
        /// Display name.
        name: String,
        /// Canvas size.
        canvas: Size,
        /// Whether this is the active sprite.
        selected: bool,
        /// Indent depth (0 = top level).
        depth: u16,
    },
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

    /// The active sprite's reference (entity + state), if a sprite is active.
    #[must_use]
    pub fn active_sprite_ref(&self) -> Option<SpriteRef> {
        match self.project.active {
            ActiveTarget::State { entity_id, state_id } => Some(SpriteRef { entity_id, state_id }),
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

    /// Creates a sprite sized to `buf` and fills its single cel with those
    /// pixels — the entry point for hand-editing a generated image in the
    /// drawing editor. Returns the new sprite and leaves it active.
    pub fn create_sprite_from_buffer(&mut self, name: impl Into<String>, buf: PixelBuffer) -> SpriteRef {
        let size = Size {
            width: buf.width(),
            height: buf.height(),
        };
        let sprite_ref = self.create_sprite(name, size);
        let buffer_id = self.active_sprite().and_then(|s| s.cels.first()).and_then(|c| match c.data {
            CelData::Raster { buffer, .. } => Some(buffer),
            _ => None,
        });
        if let Some(buffer_id) = buffer_id {
            self.pixel_buffers.insert(buffer_id, buf);
        }
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

    /// Renames the sprite at `sprite_ref`, updating both the library entity and
    /// the sprite. A whitespace-only name is rejected (no-op), matching the
    /// legacy `rename_entity_in_project` validation.
    pub fn rename_sprite(&mut self, sprite_ref: SpriteRef, new_name: &str) {
        let name = new_name.trim();
        if name.is_empty() {
            return;
        }
        let Some(entity) = self.project.library.entities.iter_mut().find(|e| e.id == sprite_ref.entity_id) else {
            return;
        };
        name.clone_into(&mut entity.name);
        if let EntityContent::Sprites { states, .. } = &mut entity.content {
            if let Some(state) = states.iter_mut().find(|s| s.id == sprite_ref.state_id) {
                name.clone_into(&mut state.sprite.name);
            }
        }
    }

    /// Deletes the sprite at `sprite_ref`: removes its state (and the containing
    /// entity when that empties it), drops the cel pixel buffers, and reassigns
    /// the active target when the deleted sprite was active — mirroring the
    /// legacy `delete_entity_clears_active_target` behavior so the canvas never
    /// points at freed data.
    pub fn delete_sprite(&mut self, sprite_ref: SpriteRef) {
        let Some(entity) = self.project.library.entities.iter_mut().find(|e| e.id == sprite_ref.entity_id) else {
            return;
        };
        let EntityContent::Sprites { states, .. } = &mut entity.content else {
            return;
        };
        let Some(pos) = states.iter().position(|s| s.id == sprite_ref.state_id) else {
            return;
        };
        let orphaned: Vec<PixelBufferId> = states[pos]
            .sprite
            .cels
            .iter()
            .filter_map(|c| match c.data {
                CelData::Raster { buffer, .. } => Some(buffer),
                _ => None,
            })
            .collect();
        states.remove(pos);
        let entity_emptied = states.is_empty();

        if entity_emptied {
            self.project.library.entities.retain(|e| e.id != sprite_ref.entity_id);
        }
        for buffer in orphaned {
            self.pixel_buffers.remove(&buffer);
        }

        let was_active = matches!(
            self.project.active,
            ActiveTarget::State { entity_id, state_id } if entity_id == sprite_ref.entity_id && state_id == sprite_ref.state_id
        );
        if was_active {
            let next = self
                .project
                .sprites_iter()
                .next()
                .map(|(named, entity_id)| SpriteRef { entity_id, state_id: named.id });
            if let Some(next) = next {
                self.select(next);
            } else {
                self.project.active = ActiveTarget::None;
                self.active_layer = None;
            }
        }
    }

    /// Deep-copies the sprite at `sprite_ref` into a new library entity, clones
    /// its pixel buffers under fresh ids, makes the copy active, and returns its
    /// address. Returns `None` if `sprite_ref` does not resolve.
    pub fn duplicate_sprite(&mut self, sprite_ref: SpriteRef) -> Option<SpriteRef> {
        let mut sprite = self
            .project
            .library
            .entities
            .iter()
            .find(|e| e.id == sprite_ref.entity_id)
            .and_then(|e| match &e.content {
                EntityContent::Sprites { states, .. } => states.iter().find(|s| s.id == sprite_ref.state_id).map(|s| s.sprite.clone()),
                _ => None,
            })?;

        let sprite_id = SpriteId::new(self.alloc_id());
        let entity_id = EntityId::new(self.alloc_id());
        let state_id = StateId::new(self.alloc_id());
        sprite.id = sprite_id;
        sprite.name = format!("{} copy", sprite.name);

        // Clone each raster buffer under a fresh id so the copy shares no pixel
        // data with the original.
        for cel in &mut sprite.cels {
            if let CelData::Raster { buffer, .. } = &mut cel.data {
                let new_buffer = PixelBufferId::new(self.alloc_id());
                if let Some(bytes) = self.pixel_buffers.get(buffer).cloned() {
                    self.pixel_buffers.insert(new_buffer, bytes);
                }
                *buffer = new_buffer;
            }
        }

        let name = sprite.name.clone();
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

        let new_ref = SpriteRef { entity_id, state_id };
        self.select(new_ref);
        Some(new_ref)
    }

    /// Moves the sprite one slot toward the front (`up`) or back **among its
    /// folder siblings** — entities sharing the same `group_id` — so ordering
    /// stays sane once sprites live in folders. A no-op at the ends.
    pub fn move_sprite(&mut self, sprite_ref: SpriteRef, up: bool) {
        let entities = &mut self.project.library.entities;
        let Some(idx) = entities.iter().position(|e| e.id == sprite_ref.entity_id) else {
            return;
        };
        let group = entities[idx].group_id;
        let target = if up {
            entities[..idx].iter().rposition(|e| e.group_id == group)
        } else {
            entities[idx + 1..].iter().position(|e| e.group_id == group).map(|off| idx + 1 + off)
        };
        if let Some(target) = target {
            entities.swap(idx, target);
        }
    }

    /// Creates a folder under `parent_id` (or top level when `None`) and returns
    /// its id. Rejects an empty name or a missing parent.
    pub fn create_group(&mut self, name: impl Into<String>, parent_id: Option<GroupId>) -> Option<GroupId> {
        let name = name.into();
        if name.trim().is_empty() {
            return None;
        }
        if let Some(pid) = parent_id {
            if !self.project.library.groups.iter().any(|g| g.id == pid) {
                return None;
            }
        }
        let id = GroupId::new(self.alloc_id());
        self.project.library.groups.push(EntityGroup {
            id,
            name,
            parent_id,
            user_data: UserData::default(),
        });
        Some(id)
    }

    /// Renames a folder. A whitespace-only name is rejected (no-op).
    pub fn rename_group(&mut self, group_id: GroupId, new_name: &str) {
        let name = new_name.trim();
        if name.is_empty() {
            return;
        }
        if let Some(group) = self.project.library.groups.iter_mut().find(|g| g.id == group_id) {
            name.clone_into(&mut group.name);
        }
    }

    /// Deletes a folder without losing its contents: sprites and sub-folders are
    /// re-parented to the deleted folder's parent (the legacy keep-entities
    /// behavior), then the folder is removed.
    pub fn delete_group(&mut self, group_id: GroupId) {
        let Some(parent) = self.project.library.groups.iter().find(|g| g.id == group_id).map(|g| g.parent_id) else {
            return;
        };
        for entity in &mut self.project.library.entities {
            if entity.group_id == Some(group_id) {
                entity.group_id = parent;
            }
        }
        for group in &mut self.project.library.groups {
            if group.parent_id == Some(group_id) {
                group.parent_id = parent;
            }
        }
        self.project.library.groups.retain(|g| g.id != group_id);
    }

    /// Re-parents a folder (drag-to-nest). Rejects a self-parent, a missing
    /// parent, or any move that would create a cycle. Returns whether it applied.
    pub fn set_group_parent(&mut self, group_id: GroupId, parent_id: Option<GroupId>) -> bool {
        if !self.project.library.groups.iter().any(|g| g.id == group_id) {
            return false;
        }
        if let Some(pid) = parent_id {
            if pid == group_id {
                return false;
            }
            // Walk the prospective parent's ancestry; hitting `group_id` is a cycle.
            let mut cursor = Some(pid);
            while let Some(cid) = cursor {
                if cid == group_id {
                    return false;
                }
                cursor = self.project.library.groups.iter().find(|g| g.id == cid).and_then(|g| g.parent_id);
            }
            if !self.project.library.groups.iter().any(|g| g.id == pid) {
                return false;
            }
        }
        if let Some(group) = self.project.library.groups.iter_mut().find(|g| g.id == group_id) {
            group.parent_id = parent_id;
        }
        true
    }

    /// Moves a sprite into a folder (drag-to-organize), or to top level when
    /// `group_id` is `None`. Rejects a missing target folder. Returns whether
    /// it applied.
    pub fn move_sprite_to_group(&mut self, sprite_ref: SpriteRef, group_id: Option<GroupId>) -> bool {
        if let Some(gid) = group_id {
            if !self.project.library.groups.iter().any(|g| g.id == gid) {
                return false;
            }
        }
        if let Some(entity) = self.project.library.entities.iter_mut().find(|e| e.id == sprite_ref.entity_id) {
            entity.group_id = group_id;
            true
        } else {
            false
        }
    }

    /// Moves a folder one slot toward the front (`up`) or back among its sibling
    /// folders (those sharing the same parent). A no-op at the ends.
    pub fn move_group(&mut self, group_id: GroupId, up: bool) {
        let groups = &mut self.project.library.groups;
        let Some(idx) = groups.iter().position(|g| g.id == group_id) else {
            return;
        };
        let parent = groups[idx].parent_id;
        let target = if up {
            groups[..idx].iter().rposition(|g| g.parent_id == parent)
        } else {
            groups[idx + 1..].iter().position(|g| g.parent_id == parent).map(|off| idx + 1 + off)
        };
        if let Some(target) = target {
            groups.swap(idx, target);
        }
    }

    /// The active sprite's first palette, if any.
    #[must_use]
    pub fn active_palette(&self) -> Option<&Palette> {
        self.active_sprite().and_then(|s| s.palettes.first())
    }

    /// The active sprite's palette selected by `id`, falling back to the first
    /// palette when `id` is `None` or names a palette the sprite no longer holds
    /// (e.g. after a delete or a sprite switch left a stale selection). `None`
    /// only when there is no active sprite or it has no palettes. This is the
    /// switcher-aware read every palette panel surface routes through, so the
    /// selection drives which palette the panel shows and edits.
    #[must_use]
    pub fn active_palette_by_id(&self, id: Option<PaletteId>) -> Option<&Palette> {
        let sprite = self.active_sprite()?;
        match id.and_then(|id| sprite.palette(id)) {
            Some(palette) => Some(palette),
            None => sprite.palettes.first(),
        }
    }

    /// The active sprite's palette selected by `id`, mutably, with the same
    /// fallback as [`Self::active_palette_by_id`]. The free-function
    /// [`palette_by_id_mut`] is the form `push_sprite_edit` closures use (they
    /// hand back `&mut Sprite`, not the store); this method is the store-level
    /// counterpart for the same selected-palette resolution.
    // The panel's edits run through `palette_by_id_mut` inside `push_sprite_edit`
    // closures (which hold `&mut Sprite`, not the store), so the binary never
    // calls this store-level form; the tests do.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn active_palette_by_id_mut(&mut self, id: Option<PaletteId>) -> Option<&mut Palette> {
        let sprite = self.active_sprite_mut()?;
        palette_by_id_mut(sprite, id)
    }

    /// Whether `layer_id` resolves to a locked layer in the active sprite.
    /// Painting refuses to touch a locked layer; the lock guards pixels, not
    /// the layer's properties.
    #[must_use]
    pub fn layer_is_locked(&self, layer_id: LayerId) -> bool {
        self.active_sprite()
            .and_then(|s| s.layers.iter().find(|ly| ly.id == layer_id))
            .is_some_and(|ly| ly.locked)
    }

    /// Ensures the active sprite has a frame, a raster layer, and a raster cel
    /// at the active `(layer, frame)`, creating each as needed, and returns the
    /// id of the buffer the active cel paints into. Linked cels resolve to
    /// their source frame's buffer (editing the shared drawing, Aseprite-style).
    /// Returns `None` when there is no active sprite, the resolved layer is not
    /// a raster layer, or the resolved layer is locked. This is the single gate
    /// every paint path funnels through, so the lock check here covers stroke,
    /// fill, shapes, and the move tool's lift target.
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

        // A locked layer refuses every paint path. Enforce once here, the shared
        // gate, rather than at each call site.
        if self.layer_is_locked(layer_id) {
            return None;
        }

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
    /// anything. `None` if there is no raster cel there yet, or the active layer
    /// is locked — the move/lift paths route through here, so a locked layer
    /// also refuses to be lifted from.
    #[must_use]
    pub fn active_buffer_id(&self) -> Option<PixelBufferId> {
        let sprite = self.active_sprite()?;
        let layer = self.active_layer?;
        if self.layer_is_locked(layer) {
            return None;
        }
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
                    clip_below: false,
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
                clip_below: false,
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

    /// The library flattened into renderable rows: folders before sprites at
    /// each level, depth-first, skipping the subtrees of folders in `collapsed`.
    /// Folder order follows the `groups` Vec; sprite order follows the
    /// `entities` Vec. A folder or sprite whose parent group no longer exists is
    /// shown at top level (defensive against a dangling `parent_id`/`group_id`).
    #[must_use]
    pub fn library_rows(&self, collapsed: &HashSet<GroupId>) -> Vec<LibraryRow> {
        let active = self.project.active_sprite_id();
        let mut rows = Vec::new();
        self.push_library_level(None, 0, collapsed, active, &mut rows);
        rows
    }

    /// Recursive helper for [`Self::library_rows`]: emit the folders and sprites
    /// whose effective parent is `parent`, recursing into expanded folders.
    fn push_library_level(&self, parent: Option<GroupId>, depth: u16, collapsed: &HashSet<GroupId>, active: Option<SpriteId>, rows: &mut Vec<LibraryRow>) {
        let lib = &self.project.library;
        for group in lib.groups.iter().filter(|g| self.effective_parent(g) == parent) {
            let is_collapsed = collapsed.contains(&group.id);
            let has_children = lib.groups.iter().any(|c| c.parent_id == Some(group.id)) || lib.entities.iter().any(|e| e.group_id == Some(group.id));
            rows.push(LibraryRow::Group {
                id: group.id,
                name: group.name.clone(),
                depth,
                collapsed: is_collapsed,
                has_children,
            });
            if !is_collapsed {
                self.push_library_level(Some(group.id), depth + 1, collapsed, active, rows);
            }
        }
        for (named, entity_id) in self.project.sprites_iter() {
            let group = lib
                .entities
                .iter()
                .find(|e| e.id == entity_id)
                .and_then(|e| e.group_id)
                .filter(|gid| lib.groups.iter().any(|g| g.id == *gid));
            if group == parent {
                rows.push(LibraryRow::Sprite {
                    sprite_ref: SpriteRef { entity_id, state_id: named.id },
                    name: named.sprite.name.clone(),
                    canvas: named.sprite.canvas,
                    selected: active == Some(named.sprite.id),
                    depth,
                });
            }
        }
    }

    /// A group's parent, treating a dangling `parent_id` (parent removed) as
    /// top level so no folder is ever orphaned out of the tree.
    fn effective_parent(&self, group: &EntityGroup) -> Option<GroupId> {
        match group.parent_id {
            Some(pid) if self.project.library.groups.iter().any(|g| g.id == pid) => Some(pid),
            _ => None,
        }
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
        for entry in sprite.composite_order() {
            let source = sprite.resolve_source_frame(entry.layer, frame);
            let Some(cel) = sprite.cel(entry.layer, source) else {
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
                mode: entry.mode,
                opacity: entry.opacity,
                visible: entry.visible,
                clip_below: entry.clip_below,
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

        // Build the layer inputs once (mirroring `composite_frame`), then let
        // `core` composite just the rect — fanning rows across rayon when the
        // rect is large enough to be worth it.
        let mut inputs: Vec<LayerInput<'_>> = Vec::new();
        for entry in sprite.composite_order() {
            let source = sprite.resolve_source_frame(entry.layer, frame);
            let Some(cel) = sprite.cel(entry.layer, source) else {
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
                mode: entry.mode,
                opacity: entry.opacity,
                visible: entry.visible,
                clip_below: entry.clip_below,
            });
        }
        composite_region(dst, &inputs, x, y, w, h);
    }

    /// Integrates a sequence of full-canvas frames into the active sprite as a
    /// new raster layer with one cel per frame, appends the frames to the
    /// timeline, and adds a [`FrameTag`] plus an [`Animation`] over the new
    /// range. Mirrors `app/src/commands/animation.rs::animation_integrate`
    /// without the IPC wrapper. Returns the tagged range.
    ///
    /// Every frame buffer must match the sprite's canvas size.
    ///
    /// `qc` is the per-frame QC and provenance the studio's normalize review
    /// produced; it rides onto the new [`Animation`] so a landed loop carries a
    /// machine-readable record of how it was processed. `None` for callers that
    /// did not run the review (the headless runner, the demo).
    ///
    /// `slice` is the resolved `SliceGrid` that cut the source grid sheet into
    /// these frames; it rides onto the new [`Animation`] so the cut is
    /// reproducible after save/load. `None` for frames that did not come from a
    /// sliced sheet (the i2v and video-import paths, the headless runner).
    #[allow(clippy::cast_possible_truncation)] // frame counts fit u32
    pub fn integrate_frames(
        &mut self,
        frames: Vec<PixelBuffer>,
        frame_duration_ms: u32,
        name: &str,
        loop_direction: LoopDirection,
        qc: Option<pixhaus_core::project::AnimationQc>,
        slice: Option<pixhaus_core::transforms::SliceGrid>,
    ) -> Option<FrameRange> {
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
                qc,
                slice,
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
    /// The keying itself is the pure [`chroma_key`];
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
        self.integrate_frames(frames, 120, "demo", LoopDirection::Forward, None, None);
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

/// The `sprite`'s palette selected by `id`, mutably, falling back to the first
/// palette when `id` is `None` or names a palette the sprite no longer holds.
/// The free-function form so a `push_sprite_edit` closure (handed `&mut Sprite`,
/// not the `DocumentStore`) can resolve the same selected palette the panel
/// reads, keeping every edit on the switcher's choice rather than always the
/// first palette.
pub(crate) fn palette_by_id_mut(sprite: &mut Sprite, id: Option<PaletteId>) -> Option<&mut Palette> {
    // Resolve to a concrete id up front so the mutable borrow is taken once.
    let target = id
        .filter(|id| sprite.palettes.iter().any(|p| p.id == *id))
        .or_else(|| sprite.palettes.first().map(|p| p.id))?;
    sprite.palette_mut(target)
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

    /// Flat `(sprite_ref, name, selected)` view of the visible sprite rows
    /// (all folders expanded), the test stand-in for the old `sprite_list`.
    fn sprites(doc: &DocumentStore) -> Vec<(SpriteRef, String, bool)> {
        doc.library_rows(&HashSet::new())
            .into_iter()
            .filter_map(|row| match row {
                LibraryRow::Sprite {
                    sprite_ref, name, selected, ..
                } => Some((sprite_ref, name, selected)),
                LibraryRow::Group { .. } => None,
            })
            .collect()
    }

    #[test]
    fn create_sprite_makes_it_active() {
        let mut doc = DocumentStore::new();
        let r = doc.create_sprite("hero", Size::new(32, 32));
        let sprite = doc.active_sprite().expect("active sprite");
        assert_eq!(sprite.name, "hero");
        assert_eq!(sprite.canvas, Size::new(32, 32));
        let rows = sprites(&doc);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].2, "new sprite is selected");
        assert_eq!(rows[0].0, r);
    }

    #[test]
    fn ids_are_monotonic_and_unique() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("a", Size::new(8, 8));
        doc.create_sprite("b", Size::new(8, 8));
        let refs: Vec<_> = sprites(&doc).into_iter().map(|s| s.0).collect();
        assert_eq!(refs.len(), 2);
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

    /// Adds a raster layer filled with `color` on frame 0 to the active sprite.
    /// Returns the new layer id. Layers added later stack on top.
    fn push_filled_layer(doc: &mut DocumentStore, name: &str, color: Rgba) -> LayerId {
        let canvas = doc.active_sprite().expect("sprite").canvas;
        let layer = LayerId::new(doc.alloc_id());
        let buffer = PixelBufferId::new(doc.alloc_id());
        doc.pixel_buffers
            .insert(buffer, PixelBuffer::filled(canvas.width, canvas.height, color).expect("layer buffer"));
        let sprite = doc.active_sprite_mut().expect("sprite");
        sprite.layers.push(Layer::raster(layer, name));
        sprite.cels.push(Cel::raster(layer, FrameIndex::new(0), buffer, canvas));
        layer
    }

    #[test]
    fn clip_layer_composites_only_over_its_opaque_base() {
        // Base layer: an opaque blob in the left half, transparent right half.
        // Clip layer on top: solid green over the whole canvas, marked clip.
        let mut doc = DocumentStore::new();
        doc.create_sprite("clip", Size::new(4, 4));
        // The seed layer is the base; paint its left half opaque blue.
        let base = doc.active_layer.expect("seed layer");
        let base_buf = doc.active_buffer_id().expect("seed buffer");
        {
            let buf = doc.pixel_buffers.get_mut(&base_buf).expect("base buffer");
            for y in 0..4 {
                for x in 0..2 {
                    buf.set_pixel(x, y, Rgba::opaque(0, 0, 255));
                }
            }
        }
        let clip = push_filled_layer(&mut doc, "shading", Rgba::opaque(0, 255, 0));
        // Mark the top layer to clip to the base.
        doc.active_sprite_mut()
            .expect("sprite")
            .layers
            .iter_mut()
            .find(|l| l.id == clip)
            .expect("clip layer")
            .clip_below = true;
        let _ = base;

        let frame = doc.composite_frame(FrameIndex::new(0)).expect("composite");
        for y in 0..4 {
            // Left half: base opaque, so the clip layer shows green.
            assert_eq!(frame.pixel(0, y), Some(Rgba::opaque(0, 255, 0)), "(0,{y})");
            assert_eq!(frame.pixel(1, y), Some(Rgba::opaque(0, 255, 0)), "(1,{y})");
            // Right half: base transparent, so the clip is masked away.
            assert_eq!(frame.pixel(2, y), Some(Rgba::transparent()), "(2,{y})");
            assert_eq!(frame.pixel(3, y), Some(Rgba::transparent()), "(3,{y})");
        }

        // The region path must agree with the full composite.
        let mut region = PixelBuffer::new(4, 4).expect("region buffer");
        doc.composite_region_into(&mut region, 0, 0, 4, 4);
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(region.pixel(x, y), frame.pixel(x, y), "region ({x},{y})");
            }
        }
    }

    #[test]
    fn integrate_frames_builds_layer_cels_tag_and_animation() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let frames: Vec<PixelBuffer> = (0..4)
            .map(|_| PixelBuffer::filled(8, 8, pixhaus_core::project::Rgba::new(10, 20, 30, 255)).unwrap())
            .collect();
        let range = doc
            .integrate_frames(frames, 100, "walk", LoopDirection::Forward, None, None)
            .expect("integrated range");

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
        let range = doc
            .integrate_frames(frames, 100, "walk", LoopDirection::Forward, None, None)
            .expect("integrated range");

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

    /// Locks the active layer of the active sprite. Returns the active layer id.
    fn lock_active_layer(doc: &mut DocumentStore, locked: bool) -> LayerId {
        let id = doc.active_layer.expect("active layer");
        let sprite = doc.active_sprite_mut().expect("active sprite");
        sprite.layers.iter_mut().find(|ly| ly.id == id).expect("layer present").locked = locked;
        id
    }

    #[test]
    fn layer_is_locked_reads_the_flag() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let id = doc.active_layer.expect("active layer");
        assert!(!doc.layer_is_locked(id), "fresh layer is unlocked");
        lock_active_layer(&mut doc, true);
        assert!(doc.layer_is_locked(id), "lock flag is read back");
    }

    #[test]
    fn ensure_drawable_denies_a_locked_layer() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        lock_active_layer(&mut doc, true);
        assert_eq!(doc.ensure_drawable(), None, "the paint gate refuses a locked layer");
    }

    #[test]
    fn active_buffer_id_denies_a_locked_layer() {
        // The move/lift paths route through active_buffer_id; a locked layer must
        // refuse to be lifted from.
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        doc.ensure_drawable().expect("seed a raster cel while unlocked");
        lock_active_layer(&mut doc, true);
        assert_eq!(doc.active_buffer_id(), None, "lift refuses a locked layer");
    }

    #[test]
    fn unlocking_restores_painting() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        lock_active_layer(&mut doc, true);
        assert_eq!(doc.ensure_drawable(), None);
        lock_active_layer(&mut doc, false);
        let id = doc.ensure_drawable().expect("unlocked layer paints again");
        assert_eq!(doc.active_buffer_id(), Some(id), "lift works once unlocked");
    }

    #[test]
    fn locked_layer_buffer_stays_byte_identical_across_denied_edits() {
        use crate::commands::extract_region;

        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        // Seed a cel and paint one pixel while unlocked, then lock.
        let id = doc.ensure_drawable().expect("seed cel");
        doc.pixel_buffers.get_mut(&id).expect("buffer").set_pixel(2, 2, Rgba::opaque(10, 20, 30));
        lock_active_layer(&mut doc, true);
        let before = extract_region(doc.pixel_buffers.get(&id).expect("buffer"), 0, 0, 8, 8);

        // Every paint path funnels through these two gates. Both deny, so a paint
        // method's `let Some(..) = gate() else { return }` early-returns without
        // touching the buffer — the stand-in for stroke/fill/shape/move.
        assert_eq!(doc.ensure_drawable(), None, "stroke/fill/shape gate denies");
        assert_eq!(doc.active_buffer_id(), None, "move/lift gate denies");

        let after = extract_region(doc.pixel_buffers.get(&id).expect("buffer"), 0, 0, 8, 8);
        assert_eq!(before, after, "a locked layer's pixels are unchanged");
    }

    #[test]
    fn structural_edits_apply_while_locked() {
        // Lock guards pixels, not properties: rename and opacity still apply.
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let id = lock_active_layer(&mut doc, true);

        let sprite = doc.active_sprite_mut().expect("sprite");
        let layer = sprite.layers.iter_mut().find(|ly| ly.id == id).expect("layer");
        layer.name = "renamed".into();
        layer.opacity = 128;

        let sprite = doc.active_sprite().expect("sprite");
        let layer = sprite.layers.iter().find(|ly| ly.id == id).expect("layer");
        assert!(layer.locked, "still locked");
        assert_eq!(layer.name, "renamed", "rename applied while locked");
        assert_eq!(layer.opacity, 128, "opacity applied while locked");

        // The sprite itself is unaffected by the lock.
        assert_eq!(doc.active_sprite().expect("sprite").name, "hero");
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

    #[test]
    fn rename_sprite_updates_entity_and_sprite() {
        let mut doc = DocumentStore::new();
        let r = doc.create_sprite("hero", Size::new(8, 8));
        doc.rename_sprite(r, "  villain  ");
        assert_eq!(doc.active_sprite().unwrap().name, "villain");
        assert_eq!(sprites(&doc)[0].1, "villain");
    }

    #[test]
    fn rename_sprite_rejects_whitespace_only() {
        let mut doc = DocumentStore::new();
        let r = doc.create_sprite("hero", Size::new(8, 8));
        doc.rename_sprite(r, "   ");
        assert_eq!(doc.active_sprite().unwrap().name, "hero");
    }

    #[test]
    fn delete_sprite_drops_buffers_and_reassigns_active() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("a", Size::new(8, 8));
        let b = doc.create_sprite("b", Size::new(8, 8));
        assert_eq!(doc.pixel_buffers.len(), 2);
        doc.delete_sprite(b);
        assert_eq!(sprites(&doc).len(), 1);
        assert_eq!(doc.pixel_buffers.len(), 1, "the deleted sprite's buffer is dropped");
        assert_eq!(doc.active_sprite().unwrap().name, "a", "active reassigns to the survivor");
    }

    #[test]
    fn delete_last_sprite_clears_active() {
        let mut doc = DocumentStore::new();
        let a = doc.create_sprite("only", Size::new(8, 8));
        doc.delete_sprite(a);
        assert!(sprites(&doc).is_empty());
        assert!(doc.active_sprite().is_none());
        assert!(doc.pixel_buffers.is_empty());
        assert!(doc.active_layer.is_none());
    }

    #[test]
    fn duplicate_sprite_clones_independent_buffers() {
        let mut doc = DocumentStore::new();
        let a = doc.create_sprite("hero", Size::new(8, 8));
        assert_eq!(doc.pixel_buffers.len(), 1);
        let dup = doc.duplicate_sprite(a).expect("duplicate");
        assert_ne!(dup.entity_id, a.entity_id);
        assert_eq!(sprites(&doc).len(), 2);
        assert_eq!(doc.pixel_buffers.len(), 2, "the copy owns a fresh buffer");
        assert_eq!(doc.active_sprite().unwrap().name, "hero copy");
    }

    #[test]
    fn move_sprite_reorders_library() {
        let mut doc = DocumentStore::new();
        let a = doc.create_sprite("a", Size::new(8, 8));
        doc.create_sprite("b", Size::new(8, 8));
        assert_eq!(sprites(&doc)[0].1, "a");
        doc.move_sprite(a, false);
        assert_eq!(sprites(&doc)[0].1, "b");
        assert_eq!(sprites(&doc)[1].1, "a");
        // Moving the front item up again is a no-op at the edge.
        let b_ref = sprites(&doc)[0].0;
        doc.move_sprite(b_ref, true);
        assert_eq!(sprites(&doc)[0].1, "b");
    }

    #[test]
    fn create_group_validates_name_and_parent() {
        let mut doc = DocumentStore::new();
        assert!(doc.create_group("   ", None).is_none(), "empty name rejected");
        let g = doc.create_group("Characters", None).expect("group created");
        assert!(doc.create_group("child", Some(g)).is_some(), "valid parent accepted");
        assert!(doc.create_group("orphan", Some(GroupId::new(9999))).is_none(), "missing parent rejected");
    }

    #[test]
    fn delete_group_reparents_contents_to_parent() {
        let mut doc = DocumentStore::new();
        let outer = doc.create_group("outer", None).unwrap();
        let inner = doc.create_group("inner", Some(outer)).unwrap();
        let sprite = doc.create_sprite("hero", Size::new(8, 8));
        assert!(doc.move_sprite_to_group(sprite, Some(inner)));
        // Deleting the inner folder moves its sprite and any child groups up to
        // `outer` — nothing is lost.
        doc.delete_group(inner);
        assert!(doc.project.library.groups.iter().all(|grp| grp.id != inner));
        let entity_group = doc.project.library.entities.iter().find(|e| e.id == sprite.entity_id).and_then(|e| e.group_id);
        assert_eq!(entity_group, Some(outer), "sprite re-parented to the deleted folder's parent");
    }

    #[test]
    fn set_group_parent_rejects_self_and_cycles() {
        let mut doc = DocumentStore::new();
        let a = doc.create_group("a", None).unwrap();
        let b = doc.create_group("b", Some(a)).unwrap();
        assert!(!doc.set_group_parent(a, Some(a)), "self-parent rejected");
        assert!(!doc.set_group_parent(a, Some(b)), "cycle (a under its own descendant b) rejected");
        let c = doc.create_group("c", None).unwrap();
        assert!(doc.set_group_parent(c, Some(a)), "valid nest accepted");
    }

    #[test]
    fn library_rows_orders_folders_then_sprites_and_honors_collapse() {
        let mut doc = DocumentStore::new();
        let folder = doc.create_group("folder", None).unwrap();
        let loose = doc.create_sprite("loose", Size::new(8, 8));
        let _ = loose;
        let inside = doc.create_sprite("inside", Size::new(8, 8));
        assert!(doc.move_sprite_to_group(inside, Some(folder)));

        // Expanded: folder header (depth 0), its sprite (depth 1), then the
        // top-level loose sprite.
        let rows = doc.library_rows(&HashSet::new());
        assert!(matches!(
            &rows[0],
            LibraryRow::Group {
                depth: 0,
                has_children: true,
                ..
            }
        ));
        assert!(matches!(&rows[1], LibraryRow::Sprite { depth: 1, name, .. } if name == "inside"));
        assert!(matches!(&rows[2], LibraryRow::Sprite { depth: 0, name, .. } if name == "loose"));

        // Collapsed: the folder's subtree is omitted.
        let collapsed: HashSet<GroupId> = [folder].into_iter().collect();
        let rows = doc.library_rows(&collapsed);
        assert!(matches!(&rows[0], LibraryRow::Group { collapsed: true, .. }));
        assert!(!rows.iter().any(|r| matches!(r, LibraryRow::Sprite { name, .. } if name == "inside")));
    }

    /// Appends a second, named palette to the active sprite and returns its id.
    fn push_palette(doc: &mut DocumentStore, name: &str) -> PaletteId {
        let id = PaletteId::new(doc.alloc_id());
        doc.active_sprite_mut().expect("sprite").add_palette(id, name);
        id
    }

    #[test]
    fn active_palette_by_id_selects_the_named_palette() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let second = push_palette(&mut doc, "second");

        // None falls back to the first (the seed "default" palette).
        assert_eq!(doc.active_palette_by_id(None).map(|p| p.name.as_str()), Some("default"));
        // A live id resolves to that palette.
        assert_eq!(doc.active_palette_by_id(Some(second)).map(|p| p.name.as_str()), Some("second"));
    }

    #[test]
    fn active_palette_by_id_falls_back_when_the_id_is_stale() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        // An id the sprite does not hold falls back to the first palette rather
        // than yielding None — the switcher never lands on an empty panel.
        let stale = PaletteId::new(999);
        assert_eq!(doc.active_palette_by_id(Some(stale)).map(|p| p.name.as_str()), Some("default"));
    }

    #[test]
    fn active_palette_by_id_is_none_without_a_sprite() {
        let doc = DocumentStore::new();
        assert!(doc.active_palette_by_id(None).is_none(), "no active sprite, no palette");
    }

    #[test]
    fn active_palette_by_id_mut_edits_the_selected_palette() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let second = push_palette(&mut doc, "second");

        // Editing through the mut accessor lands on the selected palette, not the
        // first.
        doc.active_palette_by_id_mut(Some(second))
            .expect("second palette")
            .colors
            .push(pixhaus_core::project::PaletteEntry::new(Rgba::opaque(1, 2, 3)));
        assert_eq!(doc.active_palette_by_id(Some(second)).map(|p| p.colors.len()), Some(1));
        // The first palette is untouched.
        assert_eq!(doc.active_palette_by_id(None).map(|p| p.colors.len()), Some(default_palette().len()));
    }

    #[test]
    fn palette_by_id_mut_falls_back_to_first_for_a_stale_id() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let first_id = doc.active_sprite().expect("sprite").palettes[0].id;
        let sprite = doc.active_sprite_mut().expect("sprite");
        // A None and a stale id both resolve to the first palette.
        assert_eq!(palette_by_id_mut(sprite, None).map(|p| p.id), Some(first_id));
        assert_eq!(palette_by_id_mut(sprite, Some(PaletteId::new(12345))).map(|p| p.id), Some(first_id));
    }
}
