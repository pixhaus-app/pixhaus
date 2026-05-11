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
    clippy::disallowed_methods,
    clippy::too_many_lines
)]

use pixhaus_core::project::library::{
    AiMetadata, Entity, EntityContent, EntityDefaults, EntityKind, NamedSprite,
};
use pixhaus_core::project::{BlendMode, Cel, CelData, Frame, Sprite};
use pixhaus_core::project::{
    ColorMode, EntityId, FrameIndex, Layer, LayerKind, Size, SpriteId, StateId, UserData,
};
use pixhaus_core::project::{PaletteEntry, Rgba, Slice, SliceId, SliceKey};
use pixhaus_io::Error;
use pixhaus_io::aseprite::{
    AsepriteDocument, ColorDepth, DocumentFrame,
    archive::{
        ConversionWarning, archive_to_document, archive_to_per_state_documents, document_to_archive,
    },
    chunk::{
        CelChunk, CelChunkData, Chunk, ExternalFileEntry, ExternalFilesChunk, LayerChunk,
        LayerKindCode, TagEntry, TagsChunk, TilesetChunk, TilesetSourceWire,
    },
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

    // idle frame 1: link within idle's range [0, 1].
    let mut f1 = DocumentFrame::new(100);
    f1.chunks.push(Chunk::Cel(CelChunk {
        layer_index: 0,
        x: 0,
        y: 0,
        opacity: 255,
        z_index: 0,
        data: CelChunkData::Linked { frame: 0 },
    }));
    doc.frames.push(f1);

    // walk frame 0: raster base.
    let mut f2 = DocumentFrame::new(100);
    f2.chunks.push(Chunk::Cel(compressed_cel(0, 8, 8)));
    doc.frames.push(f2);

    // walk frame 1: link within walk's range [2, 3].
    let mut f3 = DocumentFrame::new(100);
    f3.chunks.push(Chunk::Cel(CelChunk {
        layer_index: 0,
        x: 0,
        y: 0,
        opacity: 255,
        z_index: 0,
        data: CelChunkData::Linked { frame: 2 },
    }));
    doc.frames.push(f3);

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

// ── Per-state export: archive_to_per_state_documents ──────────────────────────

#[test]
fn per_state_export_produces_one_document_per_state() {
    let archive = make_archive_with_states(&["idle", "walk", "run"]);
    let docs = archive_to_per_state_documents(&archive).unwrap();
    assert_eq!(docs.len(), 3);
    assert_eq!(docs[0].state_name, "idle");
    assert_eq!(docs[1].state_name, "walk");
    assert_eq!(docs[2].state_name, "run");
}

#[test]
fn per_state_export_omits_frame_tags() {
    let archive = make_archive_with_states(&["idle", "walk"]);
    let docs = archive_to_per_state_documents(&archive).unwrap();
    for doc in &docs {
        let has_tags = doc.document.frames[0]
            .chunks
            .iter()
            .any(|c| matches!(c, Chunk::Tags(_)));
        assert!(!has_tags, "per-state doc must have no Tags chunk");
    }
}

#[test]
fn per_state_export_preserves_state_names_via_filename_stem() {
    let archive = make_archive_with_states(&["Idle Walk", "run.fast", "jump!"]);
    let docs = archive_to_per_state_documents(&archive).unwrap();
    assert_eq!(docs[0].filename_stem, "idle-walk");
    assert_eq!(docs[1].filename_stem, "run-fast");
    assert_eq!(docs[2].filename_stem, "jump");
}

#[test]
fn per_state_export_round_trips_each_state() {
    use pixhaus_io::aseprite::{decode, encode};

    let archive = make_archive_with_states(&["idle", "walk"]);
    let original_states = match &archive.project.library.entities[0].content {
        EntityContent::Sprites { states } => states.clone(),
        _ => panic!(),
    };

    let docs = archive_to_per_state_documents(&archive).unwrap();

    for (i, per_state) in docs.iter().enumerate() {
        let bytes = encode(&per_state.document).expect("encode must succeed");
        let decoded = decode(&bytes).expect("decode must succeed");
        let re_imported =
            document_to_archive(&decoded, &per_state.state_name).expect("import must succeed");

        let EntityContent::Sprites { states } =
            &re_imported.archive.project.library.entities[0].content
        else {
            panic!("expected Sprites content");
        };

        assert_eq!(
            states.len(),
            1,
            "re-imported per-state doc should have one state"
        );
        assert_eq!(
            states[0].sprite.frames.len(),
            original_states[i].sprite.frames.len()
        );
    }
}

#[test]
fn per_state_export_empty_library_errors() {
    let project = pixhaus_core::project::Project::new("empty");
    let archive = PixhausArchive::new(project);
    assert!(matches!(
        archive_to_per_state_documents(&archive).unwrap_err(),
        Error::NoSpriteEntityForExport
    ));
}

#[test]
fn per_state_export_picks_active_entity() {
    use pixhaus_core::project::{ActiveTarget, IVec2, PixelBufferId};

    let mut archive = make_archive_with_states(&["idle"]);

    // Push a second entity (id=2) with a distinct state.
    let second = Entity {
        id: EntityId::new(2),
        kind: EntityKind::Custom("Character".into()),
        name: "second".into(),
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Sprites {
            states: vec![NamedSprite {
                id: StateId::new(10),
                state_name: "attack".into(),
                sprite: Sprite {
                    id: SpriteId::new(10),
                    name: "second / attack".into(),
                    canvas: Size::new(8, 8),
                    color_mode: ColorMode::Rgba,
                    transparent_color_index: None,
                    layers: vec![make_layer()],
                    frames: vec![Frame {
                        duration_ms: 100,
                        user_data: UserData::default(),
                    }],
                    cels: vec![Cel {
                        layer_id: pixhaus_core::project::id::LayerId::new(1),
                        frame_index: FrameIndex::new(0),
                        position: IVec2::new(0, 0),
                        opacity: 255,
                        data: CelData::Raster {
                            buffer: PixelBufferId::new(99),
                            size: Size::new(8, 8),
                        },
                        user_data: UserData::default(),
                    }],
                    palettes: Vec::new(),
                    palette_frame_overrides: Vec::new(),
                    tilesets: Vec::new(),
                    frame_tags: Vec::new(),
                    animations: Vec::new(),
                    slices: Vec::new(),
                    user_data: UserData::default(),
                },
                engine_tags: Vec::new(),
            }],
        },
        ai: AiMetadata::default(),
        anchor_reference_id: None,
        user_data: UserData::default(),
        created_at: 0,
        updated_at: 0,
    };

    archive.buffers.push(PixelBufferEntry {
        id: 99,
        width: 8,
        height: 8,
        stride: 32,
        pixels: vec![0u8; 256],
    });
    archive.project.library.entities.push(second);
    archive.project.active = ActiveTarget::State {
        entity_id: EntityId::new(2),
        state_id: StateId::new(10),
    };

    let docs = archive_to_per_state_documents(&archive).unwrap();
    assert_eq!(docs.len(), 1, "active entity wins over first entity");
    assert_eq!(docs[0].state_name, "attack");
}

// ── M6: cross-state linked-cel validation ─────────────────────────────────────

#[test]
fn per_state_export_cross_state_link_warns_and_drops_cel() {
    use pixhaus_core::project::{IVec2, PixelBufferId};

    // Two states, each with 1 frame. "idle" has a valid raster cel.
    // "walk" has a linked cel with source_frame=5, which is out of bounds
    // for a 1-frame state and should trigger a CrossStateLinkedCel warning.
    let layer = make_layer();

    let idle_sprite = make_sprite(
        1,
        "hero / idle",
        layer.clone(),
        Cel {
            layer_id: layer.id,
            frame_index: FrameIndex::new(0),
            position: IVec2::new(0, 0),
            opacity: 255,
            data: CelData::Raster {
                buffer: PixelBufferId::new(1),
                size: Size::new(8, 8),
            },
            user_data: UserData::default(),
        },
    );

    let mut walk_sprite = make_sprite(
        2,
        "hero / walk",
        layer.clone(),
        // placeholder; will be replaced below
        Cel {
            layer_id: layer.id,
            frame_index: FrameIndex::new(0),
            position: IVec2::new(0, 0),
            opacity: 255,
            data: CelData::Raster {
                buffer: PixelBufferId::new(1),
                size: Size::new(8, 8),
            },
            user_data: UserData::default(),
        },
    );
    // Replace with the out-of-bounds linked cel.
    walk_sprite.cels = vec![Cel {
        layer_id: layer.id,
        frame_index: FrameIndex::new(0),
        position: IVec2::new(0, 0),
        opacity: 255,
        data: CelData::Linked {
            source_frame: FrameIndex::new(5), // >= walk's frame count of 1
        },
        user_data: UserData::default(),
    }];

    let entity = Entity {
        id: EntityId::new(1),
        kind: EntityKind::Custom("Sprite".into()),
        name: "hero".into(),
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Sprites {
            states: vec![
                NamedSprite {
                    id: StateId::new(1),
                    state_name: "idle".into(),
                    sprite: idle_sprite,
                    engine_tags: Vec::new(),
                },
                NamedSprite {
                    id: StateId::new(2),
                    state_name: "walk".into(),
                    sprite: walk_sprite,
                    engine_tags: Vec::new(),
                },
            ],
        },
        ai: AiMetadata::default(),
        anchor_reference_id: None,
        user_data: UserData::default(),
        created_at: 0,
        updated_at: 0,
    };

    let mut project = pixhaus_core::project::Project::new("test");
    project.library.entities.push(entity);
    let archive = PixhausArchive {
        project,
        buffers: vec![PixelBufferEntry {
            id: 1,
            width: 8,
            height: 8,
            stride: 32,
            pixels: vec![0u8; 256],
        }],
    };

    let docs = archive_to_per_state_documents(&archive).unwrap();
    assert_eq!(docs.len(), 2);

    assert!(docs[0].warnings.is_empty(), "idle should have no warnings");

    assert_eq!(docs[1].warnings.len(), 1);
    assert!(
        matches!(
            &docs[1].warnings[0],
            ConversionWarning::CrossStateLinkedCel { .. }
        ),
        "expected CrossStateLinkedCel warning for walk"
    );

    // The corrupt link must be dropped — no Cel chunk in walk's frame 0.
    let cel_count = docs[1].document.frames[0]
        .chunks
        .iter()
        .filter(|c| matches!(c, Chunk::Cel(_)))
        .count();
    assert_eq!(cel_count, 0, "dropped linked cel must not appear in output");
}

// ── Phase 1: panic + overflow + tileset-layout safety ─────────────────────────

#[test]
fn tag_with_from_frame_out_of_bounds_is_skipped_with_warning() {
    // 5-frame doc with one valid tag and one tag whose from_frame exceeds the
    // frame count. The invalid tag must produce an InvalidTagRange warning and
    // must NOT produce a state; only the valid tag's state is created.
    let mut doc = AsepriteDocument::empty(8, 8);
    let mut f0 = DocumentFrame::new(100);
    f0.chunks.push(Chunk::Layer(rgba_layer("bg")));
    f0.chunks.push(Chunk::Tags(TagsChunk {
        tags: vec![tag("idle", 0, 1), tag("oob", 10, 12)],
    }));
    f0.chunks.push(Chunk::Cel(compressed_cel(0, 8, 8)));
    doc.frames.push(f0);
    for _ in 1..5 {
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

    let result = document_to_archive(&doc, "sprite").unwrap();

    assert!(
        result.warnings.iter().any(|w| matches!(
            w,
            ConversionWarning::InvalidTagRange { tag_name, .. } if tag_name == "oob"
        )),
        "expected InvalidTagRange warning for 'oob'"
    );

    let EntityContent::Sprites { states } =
        &result.archive.project.library.entities[0].content
    else {
        panic!("expected Sprites content");
    };
    assert_eq!(states.len(), 1, "only the valid tag's state is created");
    assert_eq!(states[0].state_name, "idle");
}

#[test]
fn merged_export_overflowing_total_frames_returns_typed_error() {
    use pixhaus_core::project::{IVec2, PixelBufferId};

    let layer = make_layer();

    let make_state = |id: u32, frame_count: usize| -> NamedSprite {
        let frames: Vec<Frame> = (0..frame_count)
            .map(|_| Frame {
                duration_ms: 100,
                user_data: UserData::default(),
            })
            .collect();
        let cels = vec![Cel {
            layer_id: layer.id,
            frame_index: FrameIndex::new(0),
            position: IVec2::new(0, 0),
            opacity: 255,
            data: CelData::Raster {
                buffer: PixelBufferId::new(id),
                size: Size::new(8, 8),
            },
            user_data: UserData::default(),
        }];
        NamedSprite {
            id: StateId::new(id),
            state_name: format!("s{id}"),
            sprite: Sprite {
                id: SpriteId::new(id),
                name: format!("hero / s{id}"),
                canvas: Size::new(8, 8),
                color_mode: ColorMode::Rgba,
                transparent_color_index: None,
                layers: vec![layer.clone()],
                frames,
                cels,
                palettes: Vec::new(),
                palette_frame_overrides: Vec::new(),
                tilesets: Vec::new(),
                frame_tags: Vec::new(),
                animations: Vec::new(),
                slices: Vec::new(),
                user_data: UserData::default(),
            },
            engine_tags: Vec::new(),
        }
    };

    let entity = Entity {
        id: EntityId::new(1),
        kind: EntityKind::Custom("Sprite".into()),
        name: "hero".into(),
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Sprites {
            states: vec![make_state(1, 40_000), make_state(2, 40_000)],
        },
        ai: AiMetadata::default(),
        anchor_reference_id: None,
        user_data: UserData::default(),
        created_at: 0,
        updated_at: 0,
    };

    let mut project = pixhaus_core::project::Project::new("test");
    project.library.entities.push(entity);
    let archive = PixhausArchive {
        project,
        buffers: vec![
            PixelBufferEntry {
                id: 1,
                width: 8,
                height: 8,
                stride: 32,
                pixels: vec![0u8; 256],
            },
            PixelBufferEntry {
                id: 2,
                width: 8,
                height: 8,
                stride: 32,
                pixels: vec![0u8; 256],
            },
        ],
    };

    let err = archive_to_document(&archive).unwrap_err();
    assert!(
        matches!(err, Error::FrameCountOverflow { .. }),
        "expected FrameCountOverflow, got {err:?}"
    );
}

#[test]
fn inline_tileset_import_repacks_to_horizontal_layout() {
    // 2 RGBA 4×4 tiles in Aseprite's vertical-strip layout.
    // Tile 0 filled with 0xAA; tile 1 filled with 0xBB.
    // After import the buffer must use Pixhaus's horizontal-strip layout.
    let tile_w: u16 = 4;
    let tile_h: u16 = 4;
    let tile_count: u32 = 2;
    let bpp: usize = 4;
    let row_bytes = tile_w as usize * bpp;

    let mut pixels = vec![0xAAu8; row_bytes * tile_h as usize];
    pixels.extend(vec![0xBBu8; row_bytes * tile_h as usize]);

    let mut doc = AsepriteDocument::empty(tile_w, tile_h);
    let mut f0 = DocumentFrame::new(100);
    f0.chunks.push(Chunk::Layer(rgba_layer("bg")));
    f0.chunks.push(Chunk::Tileset(TilesetChunk {
        tileset_id: 0,
        flags: 0,
        tile_count,
        tile_width: tile_w,
        tile_height: tile_h,
        base_index: 1,
        name: "tiles".into(),
        source: TilesetSourceWire::Inline { pixels },
    }));
    f0.chunks.push(Chunk::Cel(compressed_cel(0, tile_w, tile_h)));
    doc.frames.push(f0);

    let result = document_to_archive(&doc, "sprite").unwrap();

    assert_eq!(result.archive.buffers.len(), 2, "one buffer for the cel, one for the tileset");

    // Tilesets are built before cels, so the tileset buffer has the smaller id.
    let ts_buf = result
        .archive
        .buffers
        .iter()
        .min_by_key(|b| b.id)
        .expect("buffers must be non-empty");

    assert_eq!(ts_buf.width, u32::from(tile_w) * tile_count, "horizontal width");
    assert_eq!(ts_buf.height, u32::from(tile_h), "single-row height");

    // Row 0 of the horizontal strip: first tile_w*bpp bytes = tile 0 (0xAA),
    // next tile_w*bpp bytes = tile 1 (0xBB).
    assert_eq!(&ts_buf.pixels[0..row_bytes], vec![0xAAu8; row_bytes].as_slice());
    assert_eq!(
        &ts_buf.pixels[row_bytes..row_bytes * 2],
        vec![0xBBu8; row_bytes].as_slice()
    );
}

// ── Phase 2: round-trip completeness ──────────────────────────────────────────

#[test]
fn merged_export_emits_tileset_chunks_for_sprite_tilesets() {
    use pixhaus_core::project::{PixelBufferId, Tileset, TilesetId, TilesetSource};

    let mut archive = make_archive_with_states(&["idle"]);

    let buf_id = 99u32;
    archive.buffers.push(PixelBufferEntry {
        id: buf_id,
        width: 8,
        height: 4,
        stride: 32,
        pixels: vec![0xCCu8; 8 * 4 * 4],
    });

    let EntityContent::Sprites { states } =
        &mut archive.project.library.entities[0].content
    else {
        panic!()
    };
    states[0].sprite.tilesets.push(Tileset {
        id: TilesetId::new(0),
        name: "terrain".into(),
        tile_size: Size::new(4, 4),
        tile_count: 2,
        base_index: 1,
        source: TilesetSource::Inline {
            buffer: PixelBufferId::new(buf_id),
        },
        properties: Vec::new(),
        autotile: None,
        user_data: UserData::default(),
    });

    let doc = archive_to_document(&archive).unwrap();

    let has_tileset = doc.frames[0]
        .chunks
        .iter()
        .any(|c| matches!(c, Chunk::Tileset(_)));

    assert!(has_tileset, "frame 0 must contain a Tileset chunk");
}

#[test]
fn merged_export_preserves_indexed_transparent_index() {
    use pixhaus_core::project::{Palette, PaletteId, PixelBufferId};

    let layer = make_layer();
    let cel = Cel {
        layer_id: layer.id,
        frame_index: FrameIndex::new(0),
        position: pixhaus_core::project::IVec2::new(0, 0),
        opacity: 255,
        data: CelData::Raster {
            buffer: PixelBufferId::new(1),
            size: Size::new(8, 8),
        },
        user_data: UserData::default(),
    };

    let mut sprite = make_sprite(1, "hero / idle", layer, cel);
    sprite.color_mode = ColorMode::Indexed;
    sprite.transparent_color_index = Some(3);
    sprite.palettes = vec![Palette {
        id: PaletteId::new(1),
        name: "pal".into(),
        colors: vec![PaletteEntry::new(Rgba::transparent()); 4],
        user_data: UserData::default(),
    }];

    let entity = Entity {
        id: EntityId::new(1),
        kind: EntityKind::Custom("Sprite".into()),
        name: "hero".into(),
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Sprites {
            states: vec![NamedSprite {
                id: StateId::new(1),
                state_name: "idle".into(),
                sprite,
                engine_tags: Vec::new(),
            }],
        },
        ai: AiMetadata::default(),
        anchor_reference_id: None,
        user_data: UserData::default(),
        created_at: 0,
        updated_at: 0,
    };

    let mut project = pixhaus_core::project::Project::new("test");
    project.library.entities.push(entity);
    let archive = PixhausArchive {
        project,
        buffers: vec![PixelBufferEntry {
            id: 1,
            width: 8,
            height: 8,
            stride: 8,
            pixels: vec![0u8; 64],
        }],
    };

    let doc = archive_to_document(&archive).unwrap();

    assert_eq!(doc.header.color_depth, ColorDepth::Indexed);
    assert_eq!(doc.header.transparent_index, 3);
}

#[test]
fn merged_export_merges_slices_across_states_with_offset() {
    use pixhaus_core::project::{IVec2, PixelBufferId, Rect};

    let key_at_0 = SliceKey {
        frame: FrameIndex::new(0),
        bounds: Rect::from_xywh(0, 0, 8, 8),
        nine_slice: None,
        pivot: None,
    };

    let make_state = |id: u32| -> NamedSprite {
        let layer = make_layer();
        let cel = Cel {
            layer_id: layer.id,
            frame_index: FrameIndex::new(0),
            position: IVec2::new(0, 0),
            opacity: 255,
            data: CelData::Raster {
                buffer: PixelBufferId::new(id),
                size: Size::new(8, 8),
            },
            user_data: UserData::default(),
        };
        NamedSprite {
            id: StateId::new(id),
            state_name: format!("s{id}"),
            sprite: Sprite {
                id: SpriteId::new(id),
                name: format!("hero / s{id}"),
                canvas: Size::new(8, 8),
                color_mode: ColorMode::Rgba,
                transparent_color_index: None,
                layers: vec![layer],
                frames: vec![
                    Frame {
                        duration_ms: 100,
                        user_data: UserData::default(),
                    },
                    Frame {
                        duration_ms: 100,
                        user_data: UserData::default(),
                    },
                ],
                cels: vec![cel],
                palettes: Vec::new(),
                palette_frame_overrides: Vec::new(),
                tilesets: Vec::new(),
                frame_tags: Vec::new(),
                animations: Vec::new(),
                slices: vec![Slice {
                    id: SliceId::new(1),
                    name: "hitbox".into(),
                    keys: vec![key_at_0.clone()],
                    user_data: UserData::default(),
                }],
                user_data: UserData::default(),
            },
            engine_tags: Vec::new(),
        }
    };

    let entity = Entity {
        id: EntityId::new(1),
        kind: EntityKind::Custom("Sprite".into()),
        name: "hero".into(),
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Sprites {
            states: vec![make_state(1), make_state(2)],
        },
        ai: AiMetadata::default(),
        anchor_reference_id: None,
        user_data: UserData::default(),
        created_at: 0,
        updated_at: 0,
    };

    let mut project = pixhaus_core::project::Project::new("test");
    project.library.entities.push(entity);
    let archive = PixhausArchive {
        project,
        buffers: vec![
            PixelBufferEntry {
                id: 1,
                width: 8,
                height: 8,
                stride: 32,
                pixels: vec![0u8; 256],
            },
            PixelBufferEntry {
                id: 2,
                width: 8,
                height: 8,
                stride: 32,
                pixels: vec![0u8; 256],
            },
        ],
    };

    let doc = archive_to_document(&archive).unwrap();

    let sc = doc.frames[0]
        .chunks
        .iter()
        .find_map(|c| {
            if let Chunk::Slice(s) = c {
                Some(s)
            } else {
                None
            }
        })
        .expect("frame 0 must have a Slice chunk");

    assert_eq!(sc.name, "hitbox");
    assert_eq!(sc.keys.len(), 2, "one key per state");
    assert_eq!(sc.keys[0].frame, 0, "state 1 key at abs frame 0");
    assert_eq!(sc.keys[1].frame, 2, "state 2 key at abs frame 2 (offset 2)");
}

#[test]
fn external_tileset_import_preserves_path() {
    use pixhaus_core::project::TilesetSource as CoreTilesetSource;

    let mut doc = AsepriteDocument::empty(8, 8);
    let mut f0 = DocumentFrame::new(100);
    f0.chunks.push(Chunk::Layer(rgba_layer("bg")));
    f0.chunks.push(Chunk::ExternalFiles(ExternalFilesChunk {
        entries: vec![ExternalFileEntry {
            id: 0,
            kind: 1,
            name: "tiles.png".into(),
        }],
    }));
    f0.chunks.push(Chunk::Tileset(TilesetChunk {
        tileset_id: 0,
        flags: 0,
        tile_count: 4,
        tile_width: 8,
        tile_height: 8,
        base_index: 1,
        name: "terrain".into(),
        source: TilesetSourceWire::External {
            external_file_id: 0,
            external_tileset_id: 0,
        },
    }));
    f0.chunks.push(Chunk::Cel(compressed_cel(0, 8, 8)));
    doc.frames.push(f0);

    let result = document_to_archive(&doc, "sprite").unwrap();

    assert!(result.warnings.is_empty(), "no warnings expected for valid external tileset");

    let EntityContent::Sprites { states } =
        &result.archive.project.library.entities[0].content
    else {
        panic!()
    };
    assert_eq!(states[0].sprite.tilesets.len(), 1);
    assert!(
        matches!(
            &states[0].sprite.tilesets[0].source,
            CoreTilesetSource::External { path } if path == "tiles.png"
        ),
        "external tileset source must preserve the path"
    );
}

// ── Phase 3: palette overrides + linked-cel out-of-range ─────────────────────

#[test]
fn cross_state_linked_cel_is_inlined_with_warning() {
    // Two single-frame states. "walk" frame 0 has a linked cel pointing to
    // doc frame 0 (in "idle"), which is outside walk's range [1, 1]. The
    // import must fire CrossStateLinkedCel and inline the raster pixels
    // instead of creating a corrupt linked cel.
    let mut doc = AsepriteDocument::empty(8, 8);
    let mut f0 = DocumentFrame::new(100);
    f0.chunks.push(Chunk::Layer(rgba_layer("bg")));
    f0.chunks.push(Chunk::Tags(TagsChunk {
        tags: vec![tag("idle", 0, 0), tag("walk", 1, 1)],
    }));
    f0.chunks.push(Chunk::Cel(compressed_cel(0, 8, 8)));
    doc.frames.push(f0);

    let mut f1 = DocumentFrame::new(100);
    f1.chunks.push(Chunk::Cel(CelChunk {
        layer_index: 0,
        x: 0,
        y: 0,
        opacity: 255,
        z_index: 0,
        data: CelChunkData::Linked { frame: 0 },
    }));
    doc.frames.push(f1);

    let result = document_to_archive(&doc, "sprite").unwrap();

    assert!(
        result
            .warnings
            .iter()
            .any(|w| matches!(w, ConversionWarning::CrossStateLinkedCel { .. })),
        "expected CrossStateLinkedCel warning on import"
    );

    let EntityContent::Sprites { states } =
        &result.archive.project.library.entities[0].content
    else {
        panic!()
    };

    let walk = &states[1];
    assert_eq!(walk.state_name, "walk");
    let cel = walk
        .sprite
        .cels
        .iter()
        .find(|c| c.frame_index.get() == 0)
        .expect("walk state must have a cel at frame 0");
    assert!(
        matches!(cel.data, CelData::Raster { .. }),
        "inlined cross-state linked cel must be raster"
    );
}

#[test]
fn merged_export_emits_per_frame_palette_overrides_with_offsets() {
    use pixhaus_core::project::{IVec2, PaletteFrameOverride, PixelBufferId};

    let layer = make_layer();

    let make_frame = || Frame {
        duration_ms: 100,
        user_data: UserData::default(),
    };
    let make_cel = |buf_id: u32| Cel {
        layer_id: layer.id,
        frame_index: FrameIndex::new(0),
        position: IVec2::new(0, 0),
        opacity: 255,
        data: CelData::Raster {
            buffer: PixelBufferId::new(buf_id),
            size: Size::new(8, 8),
        },
        user_data: UserData::default(),
    };

    let override_colors = vec![
        PaletteEntry::new(Rgba::new(0xFF, 0x00, 0x00, 0xFF)),
        PaletteEntry::new(Rgba::new(0x00, 0xFF, 0x00, 0xFF)),
    ];

    // State 1: 1 frame, no palette overrides.
    let state1 = NamedSprite {
        id: StateId::new(1),
        state_name: "idle".into(),
        sprite: Sprite {
            id: SpriteId::new(1),
            name: "hero / idle".into(),
            canvas: Size::new(8, 8),
            color_mode: ColorMode::Rgba,
            transparent_color_index: None,
            layers: vec![layer.clone()],
            frames: vec![make_frame()],
            cels: vec![make_cel(1)],
            palettes: Vec::new(),
            palette_frame_overrides: Vec::new(),
            tilesets: Vec::new(),
            frame_tags: Vec::new(),
            animations: Vec::new(),
            slices: Vec::new(),
            user_data: UserData::default(),
        },
        engine_tags: Vec::new(),
    };

    // State 2: 2 frames, palette override at local frame 1 (abs frame 2).
    let state2 = NamedSprite {
        id: StateId::new(2),
        state_name: "walk".into(),
        sprite: Sprite {
            id: SpriteId::new(2),
            name: "hero / walk".into(),
            canvas: Size::new(8, 8),
            color_mode: ColorMode::Rgba,
            transparent_color_index: None,
            layers: vec![layer.clone()],
            frames: vec![make_frame(), make_frame()],
            cels: vec![make_cel(2)],
            palettes: Vec::new(),
            palette_frame_overrides: vec![PaletteFrameOverride {
                frame: 1,
                colors: override_colors,
            }],
            tilesets: Vec::new(),
            frame_tags: Vec::new(),
            animations: Vec::new(),
            slices: Vec::new(),
            user_data: UserData::default(),
        },
        engine_tags: Vec::new(),
    };

    let entity = Entity {
        id: EntityId::new(1),
        kind: EntityKind::Custom("Sprite".into()),
        name: "hero".into(),
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Sprites {
            states: vec![state1, state2],
        },
        ai: AiMetadata::default(),
        anchor_reference_id: None,
        user_data: UserData::default(),
        created_at: 0,
        updated_at: 0,
    };

    let mut project = pixhaus_core::project::Project::new("test");
    project.library.entities.push(entity);
    let archive = PixhausArchive {
        project,
        buffers: vec![
            PixelBufferEntry {
                id: 1,
                width: 8,
                height: 8,
                stride: 32,
                pixels: vec![0u8; 256],
            },
            PixelBufferEntry {
                id: 2,
                width: 8,
                height: 8,
                stride: 32,
                pixels: vec![0u8; 256],
            },
        ],
    };

    let doc = archive_to_document(&archive).unwrap();

    // Merged layout: frame 0 = state1 f0, frame 1 = state2 f0, frame 2 = state2 f1.
    assert_eq!(doc.frames.len(), 3, "merged doc must have 3 frames");

    // Frame 2 carries the palette override.
    let frame2_has_palette = doc.frames[2]
        .chunks
        .iter()
        .any(|c| matches!(c, Chunk::Palette(_)));
    assert!(frame2_has_palette, "frame 2 must carry the palette override");

    // Frames 0 and 1 must not carry palette chunks (no base palette, no override there).
    for (i, frame) in doc.frames[0..2].iter().enumerate() {
        let has = frame.chunks.iter().any(|c| matches!(c, Chunk::Palette(_)));
        assert!(!has, "frame {i} must not carry a palette chunk");
    }
}
