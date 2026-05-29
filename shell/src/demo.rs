//! The Bit demo project: the mascot seeded into a running document in code.
//!
//! There is no `io` crate yet, so the demo cannot be a `.pixhaus` file on disk.
//! It is constructed at runtime through the existing [`DocumentStore`] +
//! [`pixhaus_core::project::Project`] model: one `Custom("Character")` entity
//! named "Bit" holding a single sprite that carries every movement and action
//! as a frame tag, with placeholder pixel buffers allocated through the
//! document's id allocator. The first-run showcase the user can regenerate,
//! restyle, animate, and export — the capabilities the rest of the roadmap
//! builds.

use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::{
    AiMetadata, Cel, Entity, EntityContent, EntityDefaults, EntityId, EntityKind, Frame, FrameIndex, FrameRange, FrameTag, Layer, LayerId, LoopDirection,
    NamedSprite, Palette, PaletteId, PixelBufferId, Rgba, Size, Sprite, SpriteId, StateId, UserData,
};

use crate::document::{DocumentStore, SpriteRef};

/// How a Bit action plays back when tagged on the timeline.
///
/// A shell-local discriminant, not a core type: the core [`FrameTag`] expresses
/// playback through `loop_direction` + `repeat`, and the brief defers widening
/// `AnimationKind`. A movement loops forever; an action plays once.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum FrameTagKind {
    /// A looping movement — idle, walk, run, jump, fall.
    Loop,
    /// A one-shot action — attack, hurt.
    OneShot,
}

impl FrameTagKind {
    /// The `repeat` count this kind tags a frame range with. `0` is the
    /// "loop forever" convention; `1` plays the action exactly once.
    fn repeat(self) -> u16 {
        match self {
            FrameTagKind::Loop => 0,
            FrameTagKind::OneShot => 1,
        }
    }
}

/// The movement/action set the Bit demo ships. Movements loop; actions are
/// one-shot. Each entry seeds one frame, one cel, one buffer, and one frame tag.
pub(crate) const BIT_ACTIONS: &[(&str, FrameTagKind)] = &[
    ("idle", FrameTagKind::Loop),
    ("walk", FrameTagKind::Loop),
    ("run", FrameTagKind::Loop),
    ("jump", FrameTagKind::Loop),
    ("fall", FrameTagKind::Loop),
    ("attack", FrameTagKind::OneShot),
    ("hurt", FrameTagKind::OneShot),
];

/// Canvas size for the demo sprite. Small and square so the placeholder frames
/// are cheap to author and the timeline renders fast on first paint.
const BIT_CANVAS: Size = Size { width: 64, height: 64 };

/// Per-frame display duration for the demo's movement/action tags.
const BIT_FRAME_MS: u32 = 120;

/// Seeds the running document with the Bit mascot: one `Custom("Character")`
/// entity named "Bit", one sprite carrying every action as a frame tag, and
/// pixel buffers allocated through the document's id allocator.
///
/// Built in code — there is no `io` crate, so the demo is constructed, not
/// loaded. Re-seedable: it clears the existing library and pixel buffers first
/// but leaves the monotonic `next_id` allocator untouched, so calling it twice
/// (the File-menu "Open Bit demo" action) replaces cleanly with no id
/// collisions. Placeholder frames are tiny hand-authored colour fields so first
/// paint never waits on an AI call.
pub(crate) fn build_bit_demo(doc: &mut DocumentStore) {
    reset_document(doc);

    let sprite_id = SpriteId::new(doc.alloc_id());
    let entity_id = EntityId::new(doc.alloc_id());
    let state_id = StateId::new(doc.alloc_id());
    let layer_id = LayerId::new(doc.alloc_id());
    let palette_id = PaletteId::new(doc.alloc_id());

    let mut sprite = Sprite::empty(sprite_id, "Bit", BIT_CANVAS);
    sprite.layers.push(Layer::raster(layer_id, "Bit"));
    sprite.palettes.push(Palette::from_colors(palette_id, "Bit", bit_palette()));

    // One frame, one cel, one buffer, and one frame tag per action. A single
    // frame per tag is enough for the showcase: the timeline renders each tag,
    // and the user regenerates real frames from the cockpit.
    for (index, (name, kind)) in BIT_ACTIONS.iter().enumerate() {
        let frame_index = FrameIndex::new(u32::try_from(index).unwrap_or(0));
        let buffer_id = PixelBufferId::new(doc.alloc_id());
        if let Ok(buffer) = placeholder_frame(name) {
            doc.pixel_buffers.insert(buffer_id, buffer);
        }
        sprite.frames.push(Frame {
            duration_ms: BIT_FRAME_MS,
            duration_mul: 1.0,
            user_data: UserData::default(),
        });
        sprite.cels.push(Cel::raster(layer_id, frame_index, buffer_id, BIT_CANVAS));
        let range = FrameRange::new(frame_index, frame_index);
        sprite.frame_tags.push(FrameTag {
            name: (*name).to_owned(),
            range,
            loop_direction: LoopDirection::Forward,
            repeat: kind.repeat(),
            user_data: UserData::default(),
        });
    }

    doc.project.library.entities.push(Entity {
        id: entity_id,
        kind: EntityKind::Custom("Character".into()),
        name: "Bit".into(),
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

    doc.select(SpriteRef { entity_id, state_id });
}

/// Clears the document's library and pixel buffers so a re-seed replaces cleanly.
/// The `next_id` allocator is intentionally left untouched: reusing freed ids
/// would risk a collision with a buffer the renderer still references, so the
/// demo keeps allocating fresh ids forward.
fn reset_document(doc: &mut DocumentStore) {
    doc.project.library.entities.clear();
    doc.project.library.groups.clear();
    doc.pixel_buffers.clear();
    doc.active_layer = None;
}

/// A tiny placeholder frame for an action: a solid colour field derived from the
/// action name so each tag reads as visibly distinct before the user regenerates
/// real art. Never blocks on an AI call.
fn placeholder_frame(action: &str) -> pixhaus_core::canvas::Result<PixelBuffer> {
    PixelBuffer::filled(BIT_CANVAS.width, BIT_CANVAS.height, action_color(action))
}

/// Maps an action name to a stable, opaque placeholder colour. Pure: the same
/// name always yields the same colour so the demo is reproducible.
fn action_color(action: &str) -> Rgba {
    // A cheap deterministic hash over the bytes, spread across the colour wheel.
    let mut hash: u32 = 2_166_136_261;
    for byte in action.bytes() {
        hash = hash.wrapping_mul(16_777_619) ^ u32::from(byte);
    }
    let r = 80 + (hash & 0x7f) as u8;
    let g = 80 + ((hash >> 8) & 0x7f) as u8;
    let b = 80 + ((hash >> 16) & 0x7f) as u8;
    Rgba::opaque(r, g, b)
}

/// A compact starter palette for the demo sprite (transparent index 0, then a
/// small Bit-flavoured ramp). Keeps the palette panel non-empty.
fn bit_palette() -> Vec<Rgba> {
    vec![
        Rgba::transparent(),
        Rgba::opaque(24, 24, 32),
        Rgba::opaque(64, 200, 220),
        Rgba::opaque(240, 240, 245),
        Rgba::opaque(220, 90, 70),
        Rgba::opaque(120, 200, 90),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds the demo into a fresh document and returns it.
    fn seeded() -> DocumentStore {
        let mut doc = DocumentStore::new();
        build_bit_demo(&mut doc);
        doc
    }

    /// The seeded "Bit" sprite, by walking the library to the single state.
    fn bit_sprite(doc: &DocumentStore) -> &Sprite {
        doc.active_sprite().expect("the demo leaves the Bit sprite active")
    }

    #[test]
    fn build_bit_demo_seeds_one_bit_entity() {
        let doc = seeded();
        let custom: Vec<&Entity> = doc
            .project
            .library
            .entities
            .iter()
            .filter(|e| matches!(&e.kind, EntityKind::Custom(c) if c == "Character"))
            .collect();
        assert_eq!(custom.len(), 1, "exactly one Custom(\"Character\") entity");
        assert_eq!(custom[0].name, "Bit", "the entity is named Bit");
    }

    #[test]
    fn build_bit_demo_allocates_through_id_allocator() {
        let mut doc = seeded();
        // Collect every seeded id type: the sprite buffers plus the ids on the
        // sprite/entity/state/layer. The next allocation must exceed all of them
        // (the allocator advanced) and no two seeded ids may collide.
        let mut ids: Vec<u32> = doc.pixel_buffers.keys().map(|k| k.get()).collect();
        let sprite = bit_sprite(&doc);
        ids.push(sprite.id.get());
        for layer in &sprite.layers {
            ids.push(layer.id.get());
        }
        for palette in &sprite.palettes {
            ids.push(palette.id.get());
        }
        let entity = &doc.project.library.entities[0];
        ids.push(entity.id.get());
        if let EntityContent::Sprites { states, .. } = &entity.content {
            for state in states {
                ids.push(state.id.get());
            }
        }
        let max_seeded = *ids.iter().max().expect("seeded at least one id");

        // No collisions among the seeded ids.
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), ids.len(), "seeded ids must be unique: {ids:?}");

        // The allocator advanced past every seeded id.
        let next = doc.alloc_id();
        assert!(next > max_seeded, "next_id ({next}) must advance past the max seeded id ({max_seeded})");
    }

    #[test]
    fn build_bit_demo_tags_every_action() {
        let doc = seeded();
        let sprite = bit_sprite(&doc);
        assert_eq!(sprite.frame_tags.len(), BIT_ACTIONS.len(), "one tag per action");
        for (name, kind) in BIT_ACTIONS {
            let tag = sprite.frame_tags.iter().find(|t| t.name == *name).unwrap_or_else(|| panic!("tag {name} present"));
            let expected_repeat = match kind {
                FrameTagKind::Loop => 0,
                FrameTagKind::OneShot => 1,
            };
            assert_eq!(tag.repeat, expected_repeat, "tag {name} loop/one-shot repeat");
        }
    }

    #[test]
    fn build_bit_demo_pixel_buffers_present() {
        let doc = seeded();
        let sprite = bit_sprite(&doc);
        for tag in &sprite.frame_tags {
            let frame = tag.range.start;
            let source = sprite.resolve_source_frame(sprite.layers[0].id, frame);
            let cel = sprite.cel(sprite.layers[0].id, source).unwrap_or_else(|| panic!("cel for {} present", tag.name));
            let pixhaus_core::project::CelData::Raster { buffer, .. } = cel.data else {
                panic!("the demo cel is a raster cel");
            };
            let buf = doc.pixel_buffers.get(&buffer).unwrap_or_else(|| panic!("buffer for {} present", tag.name));
            assert!(buf.pixels().any(|p| p.a != 0), "frame {} must be a non-empty buffer", tag.name);
        }
    }

    #[test]
    fn reseeding_replaces_cleanly_without_id_collisions() {
        let mut doc = DocumentStore::new();
        build_bit_demo(&mut doc);
        let first_sprite_id = bit_sprite(&doc).id;
        build_bit_demo(&mut doc);

        // Still exactly one Bit entity after a second seed (no duplication).
        let custom = doc
            .project
            .library
            .entities
            .iter()
            .filter(|e| matches!(&e.kind, EntityKind::Custom(c) if c == "Character"))
            .count();
        assert_eq!(custom, 1, "a re-seed replaces, it does not duplicate");

        // The re-seed minted a fresh sprite id from the still-advancing allocator,
        // so no id from the first build collides with the second.
        let second_sprite_id = bit_sprite(&doc).id;
        assert_ne!(first_sprite_id, second_sprite_id, "re-seed mints fresh ids, never reusing the first build's");

        // The only buffers left are the second build's, one per action.
        assert_eq!(doc.pixel_buffers.len(), BIT_ACTIONS.len(), "stale buffers from the first build are dropped");
    }

    #[test]
    fn action_color_is_deterministic_and_opaque() {
        assert_eq!(action_color("idle"), action_color("idle"), "same name yields the same colour");
        assert_eq!(action_color("idle").a, 255, "placeholder colours are opaque");
    }
}
