//! Aseprite round-trip integration tests.
//!
//! Exercises `document_to_archive` and `archive_to_document` against the
//! B9.1 library data model. These replace the B9.1–B9.5 stubs that
//! returned `LegacyImportUnsupported`; those stubs were kept in the git
//! history at `feat/stream-b9.1`'s parent commit.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::disallowed_methods
)]

use pixhaus_core::project::library::{
    AiMetadata, Entity, EntityContent, EntityDefaults, EntityKind, NamedSprite,
};
use pixhaus_core::project::{BlendMode, Cel, CelData, Frame, Sprite};
use pixhaus_core::project::{
    ColorMode, EntityId, FrameIndex, Layer, LayerKind, Size, SpriteId, StateId, UserData,
};
use pixhaus_io::Error;
use pixhaus_io::aseprite::{
    AsepriteDocument, DocumentFrame,
    archive::{ConversionWarning, archive_to_document, document_to_archive},
    chunk::{CelChunk, CelChunkData, Chunk, LayerChunk, LayerKindCode, TagEntry, TagsChunk},
};
use pixhaus_io::pixhaus::{PixelBufferEntry, PixhausArchive};

// ── Helpers ────────────────────────────────────────────────────────────────────

fn rgba_layer(name: &str) -> LayerChunk {
    LayerChunk {
        flags: 0x0001 | 0x0002, // visible | editable
        kind: LayerKindCode::Normal,
        child_level: 0,
        blend: BlendMode::Normal,
        unknown_blend_code: None,
        opacity: 255,
        name: name.into(),
        tileset_index: 0,
        uuid: None,
    }
}

fn compressed_cel(layer_index: u16, w: u16, h: u16) -> CelChunk {
    CelChunk {
        layer_index,
        x: 0,
        y: 0,
        opacity: 255,
        z_index: 0,
        data: CelChunkData::Compressed {
            width: w,
            height: h,
            pixels: vec![0u8; (w as usize) * (h as usize) * 4],
        },
    }
}

fn tag(name: &str, from: u16, to: u16) -> TagEntry {
    TagEntry {
        from_frame: from,
        to_frame: to,
        loop_direction: 0,
        repeat: 0,
        name: name.into(),
        deprecated_color: [0; 3],
    }
}

fn make_layer() -> Layer {
    Layer {
        id: pixhaus_core::project::id::LayerId::new(1),
        name: "bg".into(),
        kind: LayerKind::Raster,
        blend_mode: BlendMode::Normal,
        opacity: 255,
        visible: true,
        locked: false,
        parent: None,
        user_data: UserData::default(),
    }
}

fn make_sprite(sprite_id: u32, name: &str, layer: Layer, cel: Cel) -> Sprite {
    Sprite {
        id: SpriteId::new(sprite_id),
        name: name.into(),
        canvas: Size::new(8, 8),
        color_mode: ColorMode::Rgba,
        transparent_color_index: None,
        layers: vec![layer],
        frames: vec![Frame {
            duration_ms: 100,
            user_data: UserData::default(),
        }],
        cels: vec![cel],
        palettes: Vec::new(),
        palette_frame_overrides: Vec::new(),
        tilesets: Vec::new(),
        frame_tags: Vec::new(),
        animations: Vec::new(),
        slices: Vec::new(),
        user_data: UserData::default(),
    }
}

fn make_archive_with_states(state_names: &[&str]) -> PixhausArchive {
    use pixhaus_core::project::{PixelBufferId, id::LayerId};

    let layer = make_layer();
    let mut states: Vec<NamedSprite> = Vec::new();

    for (i, &name) in state_names.iter().enumerate() {
        let buf_id = u32::try_from(i).unwrap_or(u32::MAX).saturating_add(1);
        let cel = Cel {
            layer_id: LayerId::new(1),
            frame_index: FrameIndex::new(0),
            position: pixhaus_core::project::IVec2::new(0, 0),
            opacity: 255,
            data: CelData::Raster {
                buffer: PixelBufferId::new(buf_id),
                size: Size::new(8, 8),
            },
            user_data: UserData::default(),
        };

        states.push(NamedSprite {
            id: StateId::new(buf_id),
            state_name: name.into(),
            sprite: make_sprite(buf_id, &format!("hero / {name}"), layer.clone(), cel),
            engine_tags: Vec::new(),
        });
    }

    let entity = Entity {
        id: EntityId::new(1),
        kind: EntityKind::Custom("Character".into()),
        name: "hero".into(),
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Sprites { states },
        ai: AiMetadata::default(),
        anchor_reference_id: None,
        user_data: UserData::default(),
        created_at: 0,
        updated_at: 0,
    };

    let mut project = pixhaus_core::project::Project::new("test");
    project.library.entities.push(entity);

    let buffers: Vec<PixelBufferEntry> = state_names
        .iter()
        .enumerate()
        .map(|(i, _)| PixelBufferEntry {
            id: u32::try_from(i).unwrap_or(u32::MAX).saturating_add(1),
            width: 8,
            height: 8,
            stride: 32,
            pixels: vec![0u8; 8 * 8 * 4],
        })
        .collect();

    PixhausArchive { project, buffers }
}

// ── Import: document_to_archive ────────────────────────────────────────────────

#[test]
fn import_no_tags_creates_default_state() {
    let mut doc = AsepriteDocument::empty(8, 8);
    let mut f0 = DocumentFrame::new(100);
    f0.chunks.push(Chunk::Layer(rgba_layer("bg")));
    f0.chunks.push(Chunk::Cel(compressed_cel(0, 8, 8)));
    doc.frames.push(f0);

    let result = document_to_archive(&doc, "sprite").unwrap();
    let entities = &result.archive.project.library.entities;
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].name, "sprite");

    let EntityContent::Sprites { states } = &entities[0].content else {
        panic!()
    };
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].state_name, "default");
    assert_eq!(states[0].sprite.frames.len(), 1);
    assert_eq!(states[0].sprite.layers.len(), 1);
}

#[test]
fn import_tags_become_states() {
    let mut doc = AsepriteDocument::empty(8, 8);
    let mut f0 = DocumentFrame::new(100);
    f0.chunks.push(Chunk::Layer(rgba_layer("bg")));
    f0.chunks.push(Chunk::Tags(TagsChunk {
        tags: vec![tag("idle", 0, 1), tag("walk", 2, 3)],
    }));
    f0.chunks.push(Chunk::Cel(compressed_cel(0, 8, 8)));
    doc.frames.push(f0);
    for _ in 0..3 {
        let mut f = DocumentFrame::new(100);
        f.chunks.push(Chunk::Cel(CelChunk {
            layer_index: 0,
            x: 0,
            y: 0,
            opacity: 255,
            z_index: 0,
            data: CelChunkData::Linked { frame: 0 },
        }));
        doc.frames.push(f);
    }

    let result = document_to_archive(&doc, "hero").unwrap();
    assert!(result.warnings.is_empty());
    let EntityContent::Sprites { states } = &result.archive.project.library.entities[0].content
    else {
        panic!()
    };
    assert_eq!(states.len(), 2);
    assert_eq!(states[0].state_name, "idle");
    assert_eq!(states[1].state_name, "walk");
    assert_eq!(states[0].sprite.frames.len(), 2);
    assert_eq!(states[1].sprite.frames.len(), 2);
}

#[test]
fn import_empty_document_errors() {
    let doc = AsepriteDocument::empty(8, 8);
    assert!(matches!(
        document_to_archive(&doc, "x").unwrap_err(),
        Error::NoFrames
    ));
}

#[test]
fn import_linked_cel_remapped_within_state() {
    let mut doc = AsepriteDocument::empty(8, 8);
    let mut f0 = DocumentFrame::new(100);
    f0.chunks.push(Chunk::Layer(rgba_layer("bg")));
    f0.chunks.push(Chunk::Tags(TagsChunk {
        tags: vec![tag("idle", 0, 0), tag("walk", 1, 2)],
    }));
    f0.chunks.push(Chunk::Cel(compressed_cel(0, 8, 8)));
    doc.frames.push(f0);

    let mut f1 = DocumentFrame::new(100);
    f1.chunks.push(Chunk::Cel(compressed_cel(0, 8, 8)));
    doc.frames.push(f1);

    // walk frame 1 links to doc frame 1 (= walk frame 0)
    let mut f2 = DocumentFrame::new(100);
    f2.chunks.push(Chunk::Cel(CelChunk {
        layer_index: 0,
        x: 0,
        y: 0,
        opacity: 255,
        z_index: 0,
        data: CelChunkData::Linked { frame: 1 },
    }));
    doc.frames.push(f2);

    let result = document_to_archive(&doc, "hero").unwrap();
    let EntityContent::Sprites { states } = &result.archive.project.library.entities[0].content
    else {
        panic!()
    };
    let walk = &states[1];
    let linked = walk.sprite.cels.iter().find(|c| {
        matches!(c.data, CelData::Linked { source_frame } if source_frame == FrameIndex::new(0))
    });
    assert!(linked.is_some(), "linked cel should point at state frame 0");
}

#[test]
fn import_unknown_blend_mode_warns() {
    let mut doc = AsepriteDocument::empty(4, 4);
    let mut f0 = DocumentFrame::new(100);
    f0.chunks.push(Chunk::Layer(LayerChunk {
        flags: 0x0003,
        kind: LayerKindCode::Normal,
        child_level: 0,
        blend: BlendMode::Normal,
        unknown_blend_code: Some(99),
        opacity: 255,
        name: "weird".into(),
        tileset_index: 0,
        uuid: None,
    }));
    f0.chunks.push(Chunk::Cel(compressed_cel(0, 4, 4)));
    doc.frames.push(f0);

    let result = document_to_archive(&doc, "s").unwrap();
    assert_eq!(result.warnings.len(), 1);
    assert!(matches!(
        &result.warnings[0],
        ConversionWarning::UnknownBlendMode { code: 99, .. }
    ));
    let EntityContent::Sprites { states } = &result.archive.project.library.entities[0].content
    else {
        panic!()
    };
    assert_eq!(states[0].sprite.layers[0].blend_mode, BlendMode::Normal);
}

// ── Export: archive_to_document ────────────────────────────────────────────────

#[test]
fn export_two_states_produces_tags() {
    let archive = make_archive_with_states(&["idle", "walk"]);
    let doc = archive_to_document(&archive).unwrap();

    // Two states × one frame each → two total frames
    assert_eq!(doc.frames.len(), 2);

    let tags = doc.frames[0]
        .chunks
        .iter()
        .find_map(|c| {
            if let Chunk::Tags(t) = c {
                Some(t)
            } else {
                None
            }
        })
        .expect("frame 0 must have a Tags chunk");

    assert_eq!(tags.tags.len(), 2);
    assert_eq!(tags.tags[0].name, "idle");
    assert_eq!(tags.tags[0].from_frame, 0);
    assert_eq!(tags.tags[0].to_frame, 0);
    assert_eq!(tags.tags[1].name, "walk");
    assert_eq!(tags.tags[1].from_frame, 1);
    assert_eq!(tags.tags[1].to_frame, 1);
}

#[test]
fn export_empty_library_errors() {
    let project = pixhaus_core::project::Project::new("empty");
    let archive = PixhausArchive::new(project);
    assert!(matches!(
        archive_to_document(&archive).unwrap_err(),
        Error::NoSpriteEntityForExport
    ));
}

#[test]
fn export_layers_round_trip() {
    let archive = make_archive_with_states(&["idle"]);
    let doc = archive_to_document(&archive).unwrap();

    let layers: Vec<_> = doc.frames[0]
        .chunks
        .iter()
        .filter_map(|c| {
            if let Chunk::Layer(l) = c {
                Some(l)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].name, "bg");
    assert_eq!(layers[0].child_level, 0);
    assert!(layers[0].flags & 0x0001 != 0, "layer should be visible");
}

#[test]
fn export_three_states_frame_count() {
    let archive = make_archive_with_states(&["idle", "walk", "run"]);
    let doc = archive_to_document(&archive).unwrap();
    // Three states × one frame each = three total frames
    assert_eq!(doc.frames.len(), 3);
    let tags = doc.frames[0]
        .chunks
        .iter()
        .find_map(|c| {
            if let Chunk::Tags(t) = c {
                Some(t)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(tags.tags.len(), 3);
    assert_eq!(tags.tags[2].name, "run");
    assert_eq!(tags.tags[2].from_frame, 2);
    assert_eq!(tags.tags[2].to_frame, 2);
}

// ── Round-trip: import then export ────────────────────────────────────────────

#[test]
fn round_trip_preserves_state_names() {
    let mut doc = AsepriteDocument::empty(8, 8);
    let mut f0 = DocumentFrame::new(100);
    f0.chunks.push(Chunk::Layer(rgba_layer("fg")));
    f0.chunks.push(Chunk::Tags(TagsChunk {
        tags: vec![tag("idle", 0, 0), tag("walk", 1, 1)],
    }));
    f0.chunks.push(Chunk::Cel(compressed_cel(0, 8, 8)));
    doc.frames.push(f0);

    let mut f1 = DocumentFrame::new(150);
    f1.chunks.push(Chunk::Cel(compressed_cel(0, 8, 8)));
    doc.frames.push(f1);

    let imported = document_to_archive(&doc, "hero").unwrap();
    let exported = archive_to_document(&imported.archive).unwrap();

    let tags = exported.frames[0]
        .chunks
        .iter()
        .find_map(|c| {
            if let Chunk::Tags(t) = c {
                Some(t)
            } else {
                None
            }
        })
        .expect("exported doc must have a Tags chunk");

    let names: Vec<&str> = tags.tags.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["idle", "walk"]);
}
