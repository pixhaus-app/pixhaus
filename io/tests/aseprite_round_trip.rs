//! Integration round-trip tests for `.aseprite` read/write.
//!
//! Three layers of fixtures, each driven through bytes → `AsepriteDocument`
//! → bytes and (where it makes sense) `AsepriteDocument` → `PixhausArchive`
//! → `AsepriteDocument`:
//!
//! 1. **Empty document** — header + no frames. The minimal valid file.
//! 2. **Structural document** — a single sprite with layers, palette,
//!    cels, tags, slices, and a tileset. Exercises every spec-mandated
//!    chunk Pixhaus claims to support.
//! 3. **Buffer-bearing document** — pixel cels with non-trivial RGBA
//!    content and a tileset whose pixels round-trip through ZLIB.
//!
//! The asserts care about wire round-trip first (every byte we write
//! reads back the same), then about archive translation (every archive
//! field we model survives a Document detour).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::disallowed_methods,
    clippy::cast_possible_truncation,
    clippy::too_many_lines
)]

use pixhaus_core::project::{
    BlendMode, Cel, CelData, ColorMode, FeatureFlags, Frame, FrameIndex, FrameRange, FrameTag,
    IVec2, Layer, LayerId, LayerKind, LoopDirection, NineSlice, Palette, PaletteEntry, PaletteId,
    Pivot, PixelBufferId, Project, ProjectMetadata, Rect, Rgba, SchemaVersion, Size, Slice,
    SliceId, SliceKey, Sprite, SpriteId, TileCell, TileFlags, TileIndex, TilemapData, Tileset,
    TilesetId, TilesetSource, UserData,
};
use pixhaus_io::aseprite::{
    AsepriteDocument, CelChunk, CelChunkData, Chunk, ConversionWarning, DocumentFrame,
    DocumentHeader, LayerChunk, LayerKindCode, PaletteChunk, PaletteEntryWire, SliceChunk,
    SliceKeyEntry, TagEntry, TagsChunk, TilesetChunk, TilesetSourceWire, UserDataChunk,
    archive_to_document, decode, document_to_archive, encode,
};
use pixhaus_io::pixhaus::{PixelBufferEntry, PixhausArchive};

// ── byte-level helpers ──────────────────────────────────────────────────────

fn rgba_solid(width: u16, height: u16, color: [u8; 4]) -> Vec<u8> {
    let mut out = Vec::with_capacity(usize::from(width) * usize::from(height) * 4);
    for _ in 0..(usize::from(width) * usize::from(height)) {
        out.extend_from_slice(&color);
    }
    out
}

fn rgba_checker(width: u16, height: u16) -> Vec<u8> {
    let mut out = Vec::with_capacity(usize::from(width) * usize::from(height) * 4);
    for y in 0..u32::from(height) {
        for x in 0..u32::from(width) {
            if (x + y) % 2 == 0 {
                out.extend_from_slice(&[255, 0, 0, 255]);
            } else {
                out.extend_from_slice(&[0, 0, 255, 255]);
            }
        }
    }
    out
}

// ── document-level fixtures ─────────────────────────────────────────────────

fn empty_document() -> AsepriteDocument {
    AsepriteDocument::empty(8, 8)
}

fn structural_document() -> AsepriteDocument {
    let mut doc = AsepriteDocument {
        header: DocumentHeader::rgba(16, 16),
        frames: vec![DocumentFrame::new(100), DocumentFrame::new(150)],
    };
    let frame0 = &mut doc.frames[0];
    // Two layers: a group, then a raster child.
    frame0.chunks.push(Chunk::Layer(LayerChunk {
        flags: 0b11, // visible + editable
        kind: LayerKindCode::Group,
        child_level: 0,
        blend: BlendMode::Normal,
        opacity: 255,
        name: "fx".into(),
        tileset_index: 0,
        uuid: None,
        unknown_blend_code: None,
    }));
    frame0.chunks.push(Chunk::UserData(UserDataChunk {
        text: Some("group note".into()),
        color: Some(Rgba::opaque(255, 200, 0)),
        had_properties: false,
    }));
    frame0.chunks.push(Chunk::Layer(LayerChunk {
        flags: 0b11,
        kind: LayerKindCode::Normal,
        child_level: 1,
        blend: BlendMode::Multiply,
        opacity: 200,
        name: "main".into(),
        tileset_index: 0,
        uuid: None,
        unknown_blend_code: None,
    }));
    // Raster cel on the child layer at frame 0.
    frame0.chunks.push(Chunk::Cel(CelChunk {
        layer_index: 1,
        x: 0,
        y: 0,
        opacity: 255,
        z_index: 0,
        data: CelChunkData::Compressed {
            width: 4,
            height: 4,
            pixels: rgba_solid(4, 4, [10, 20, 30, 255]),
        },
    }));
    // Tag covering both frames, ping-pong.
    frame0.chunks.push(Chunk::Tags(TagsChunk {
        tags: vec![TagEntry {
            from_frame: 0,
            to_frame: 1,
            loop_direction: 2,
            repeat: 0,
            name: "idle".into(),
            deprecated_color: [0, 0, 0],
        }],
    }));
    // Palette — two entries, second named.
    frame0.chunks.push(Chunk::Palette(PaletteChunk {
        palette_size: 2,
        first_index: 0,
        last_index: 1,
        entries: vec![
            PaletteEntryWire {
                color: Rgba::transparent(),
                name: None,
            },
            PaletteEntryWire {
                color: Rgba::opaque(255, 0, 0),
                name: Some("red".into()),
            },
        ],
    }));
    // Slice with nine-slice + pivot on a single key.
    frame0.chunks.push(Chunk::Slice(SliceChunk {
        name: "ui_button".into(),
        has_nine_slice: true,
        has_pivot: true,
        keys: vec![SliceKeyEntry {
            frame: 0,
            x: 0,
            y: 0,
            width: 16,
            height: 16,
            nine_slice: Some(pixhaus_io::aseprite::NineSliceWire {
                x: 2,
                y: 2,
                width: 12,
                height: 12,
            }),
            pivot: Some(pixhaus_io::aseprite::PivotWire { x: 8, y: 8 }),
        }],
    }));
    // Inline tileset. Three 4x4 tiles → 4 columns × 12 rows.
    frame0.chunks.push(Chunk::Tileset(TilesetChunk {
        tileset_id: 1,
        flags: 0,
        tile_count: 3,
        tile_width: 4,
        tile_height: 4,
        base_index: 1,
        name: "ground".into(),
        source: TilesetSourceWire::Inline {
            pixels: rgba_solid(4, 12, [50, 100, 150, 255]),
        },
    }));
    // Frame 1: linked cel pointing back at frame 0.
    let frame1 = &mut doc.frames[1];
    frame1.chunks.push(Chunk::Cel(CelChunk {
        layer_index: 1,
        x: 0,
        y: 0,
        opacity: 255,
        z_index: 0,
        data: CelChunkData::Linked { frame: 0 },
    }));
    doc
}

fn buffer_bearing_document() -> AsepriteDocument {
    let mut doc = AsepriteDocument {
        header: DocumentHeader::rgba(16, 16),
        frames: vec![DocumentFrame::new(100)],
    };
    let frame = &mut doc.frames[0];
    frame.chunks.push(Chunk::Layer(LayerChunk {
        flags: 0b11,
        kind: LayerKindCode::Normal,
        child_level: 0,
        blend: BlendMode::Normal,
        opacity: 255,
        name: "main".into(),
        tileset_index: 0,
        uuid: None,
        unknown_blend_code: None,
    }));
    frame.chunks.push(Chunk::Cel(CelChunk {
        layer_index: 0,
        x: 0,
        y: 0,
        opacity: 255,
        z_index: 0,
        data: CelChunkData::Compressed {
            width: 16,
            height: 16,
            pixels: rgba_checker(16, 16),
        },
    }));
    doc
}

// ── archive-level fixtures ──────────────────────────────────────────────────

fn structural_archive() -> PixhausArchive {
    let raster_layer = Layer {
        id: LayerId::new(1),
        name: "main".into(),
        kind: LayerKind::Raster,
        blend_mode: BlendMode::Multiply,
        opacity: 200,
        visible: true,
        locked: false,
        parent: None,
        user_data: UserData::default(),
    };
    let frames = vec![
        Frame {
            duration_ms: 100,
            user_data: UserData::default(),
        },
        Frame {
            duration_ms: 150,
            user_data: UserData::default(),
        },
    ];
    let cels = vec![
        Cel {
            layer_id: LayerId::new(1),
            frame_index: FrameIndex::new(0),
            position: IVec2::zero(),
            opacity: 255,
            data: CelData::Raster {
                buffer: PixelBufferId::new(10),
                size: Size::new(4, 4),
            },
            user_data: UserData::default(),
        },
        Cel {
            layer_id: LayerId::new(1),
            frame_index: FrameIndex::new(1),
            position: IVec2::zero(),
            opacity: 255,
            data: CelData::Linked {
                source_frame: FrameIndex::new(0),
            },
            user_data: UserData::default(),
        },
    ];
    let palette = Palette {
        id: PaletteId::new(1),
        name: "default".into(),
        colors: vec![
            PaletteEntry::new(Rgba::transparent()),
            PaletteEntry {
                color: Rgba::opaque(255, 0, 0),
                name: Some("red".into()),
            },
        ],
        user_data: UserData::default(),
    };
    let slices = vec![Slice {
        id: SliceId::new(1),
        name: "ui_button".into(),
        keys: vec![SliceKey {
            frame: FrameIndex::new(0),
            bounds: Rect::from_xywh(0, 0, 16, 16),
            nine_slice: Some(NineSlice {
                center: Rect::from_xywh(2, 2, 12, 12),
            }),
            pivot: Some(Pivot {
                offset: IVec2::new(8, 8),
            }),
        }],
        user_data: UserData::default(),
    }];
    let frame_tags = vec![FrameTag {
        name: "idle".into(),
        range: FrameRange::new(FrameIndex::new(0), FrameIndex::new(1)),
        loop_direction: LoopDirection::PingPong,
        repeat: 0,
        user_data: UserData::default(),
    }];
    let sprite = Sprite {
        id: SpriteId::new(1),
        name: "hero".into(),
        canvas: Size::new(16, 16),
        color_mode: ColorMode::Rgba,
        transparent_color_index: None,
        layers: vec![raster_layer],
        frames,
        cels,
        palettes: vec![palette],
        palette_frame_overrides: Vec::new(),
        tilesets: Vec::new(),
        frame_tags,
        animations: Vec::new(),
        slices,
        user_data: UserData::default(),
    };
    let buffers = vec![PixelBufferEntry {
        id: 10,
        width: 4,
        height: 4,
        stride: 16,
        pixels: rgba_solid(4, 4, [10, 20, 30, 255]),
    }];
    let project = Project {
        schema_version: SchemaVersion::current(),
        feature_flags: FeatureFlags::SLICES,
        metadata: ProjectMetadata {
            name: "hero".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: env!("CARGO_PKG_VERSION").into(),
        },
        sprites: vec![sprite],
        canvas: pixhaus_core::project::CanvasState::default(),
        brush: pixhaus_core::project::BrushState::default(),
        selection: pixhaus_core::project::SelectionState::default(),
    };
    PixhausArchive { project, buffers }
}

// ── tests ───────────────────────────────────────────────────────────────────

#[test]
fn empty_document_round_trips_through_bytes() {
    let doc = empty_document();
    let bytes = encode(&doc).unwrap();
    let back = decode(&bytes).unwrap();
    assert_eq!(back, doc);
}

#[test]
fn structural_document_round_trips_through_bytes() {
    let doc = structural_document();
    let bytes = encode(&doc).unwrap();
    let back = decode(&bytes).unwrap();
    assert_eq!(back.header, doc.header);
    assert_eq!(back.frames.len(), doc.frames.len());
    for (orig, parsed) in doc.frames.iter().zip(back.frames.iter()) {
        assert_eq!(orig.duration_ms, parsed.duration_ms);
        assert_eq!(orig.chunks.len(), parsed.chunks.len());
    }
    // Specific chunk asserts: pixel cels survive unchanged after the
    // ZLIB hop, palette names round-trip, the tag's loop direction
    // survives, and the inline tileset's pixels reach back as we
    // wrote them.
    let frame0 = &back.frames[0];
    let cel = frame0.chunks.iter().find_map(|c| match c {
        Chunk::Cel(cel) => Some(cel.clone()),
        _ => None,
    });
    assert!(cel.is_some(), "expected a cel chunk");
    let palette = frame0.chunks.iter().find_map(|c| match c {
        Chunk::Palette(p) => Some(p.clone()),
        _ => None,
    });
    assert_eq!(
        palette.as_ref().and_then(|p| p.entries[1].name.as_deref()),
        Some("red")
    );
    let tag = frame0.chunks.iter().find_map(|c| match c {
        Chunk::Tags(t) => t.tags.first().cloned(),
        _ => None,
    });
    assert_eq!(tag.as_ref().map(|t| t.loop_direction), Some(2));
    let tileset_pixels = frame0.chunks.iter().find_map(|c| match c {
        Chunk::Tileset(t) => match &t.source {
            TilesetSourceWire::Inline { pixels } => Some(pixels.clone()),
            TilesetSourceWire::External { .. } => None,
        },
        _ => None,
    });
    assert_eq!(tileset_pixels.map(|p| p.len()), Some(4 * 12 * 4));
}

#[test]
fn buffer_bearing_document_pixel_round_trips() {
    let doc = buffer_bearing_document();
    let bytes = encode(&doc).unwrap();
    let back = decode(&bytes).unwrap();
    let cel = back.frames[0].chunks.iter().find_map(|c| match c {
        Chunk::Cel(cel) => Some(cel.clone()),
        _ => None,
    });
    let cel = cel.expect("expected a cel chunk");
    match cel.data {
        CelChunkData::Compressed {
            width,
            height,
            pixels,
        } => {
            assert_eq!(width, 16);
            assert_eq!(height, 16);
            assert_eq!(pixels, rgba_checker(16, 16));
        }
        _ => panic!("expected compressed cel"),
    }
}

#[test]
fn empty_archive_round_trips_through_aseprite_document() {
    let archive = PixhausArchive::new(Project::new("empty"));
    let doc = archive_to_document(&archive);
    let bytes = encode(&doc).unwrap();
    let parsed = decode(&bytes).unwrap();
    let converted = document_to_archive(&parsed, "empty").unwrap();
    assert!(!converted.archive.project.sprites.is_empty());
    let sprite = converted.archive.project.sprites.first().unwrap();
    assert_eq!(sprite.name, "empty");
}

#[test]
fn structural_archive_round_trips_through_aseprite_document() {
    let archive = structural_archive();
    let doc = archive_to_document(&archive);
    let bytes = encode(&doc).unwrap();
    let parsed = decode(&bytes).unwrap();
    let converted = document_to_archive(&parsed, "hero").unwrap();
    let sprite = converted
        .archive
        .project
        .sprites
        .first()
        .expect("expected a sprite");

    // Layer count, kinds, and ordering match the source.
    assert_eq!(sprite.layers.len(), 1);
    assert_eq!(sprite.layers[0].name, "main");
    assert_eq!(sprite.layers[0].opacity, 200);
    assert_eq!(sprite.layers[0].blend_mode, BlendMode::Multiply);

    // Two frames + their durations.
    assert_eq!(sprite.frames.len(), 2);
    assert_eq!(sprite.frames[0].duration_ms, 100);
    assert_eq!(sprite.frames[1].duration_ms, 150);

    // Cels: one raster + one linked.
    assert_eq!(sprite.cels.len(), 2);
    let linked = sprite
        .cels
        .iter()
        .find(|c| matches!(c.data, CelData::Linked { .. }));
    assert!(linked.is_some(), "expected a linked cel");

    // Palette entries (with name preserved).
    let palette = sprite.palettes.first().expect("expected a palette");
    assert_eq!(palette.colors.len(), 2);
    assert_eq!(palette.colors[1].name.as_deref(), Some("red"));

    // Tag with loop direction and frame range.
    assert_eq!(sprite.frame_tags.len(), 1);
    let tag = &sprite.frame_tags[0];
    assert_eq!(tag.name, "idle");
    assert_eq!(tag.loop_direction, LoopDirection::PingPong);
    assert_eq!(tag.range.start.get(), 0);
    assert_eq!(tag.range.end.get(), 1);

    // Slice with nine-slice + pivot.
    assert_eq!(sprite.slices.len(), 1);
    let slice = &sprite.slices[0];
    assert!(slice.keys[0].nine_slice.is_some());
    assert_eq!(
        slice.keys[0].pivot.as_ref().map(|p| p.offset),
        Some(IVec2::new(8, 8))
    );

    // No spurious warnings — the structural archive uses only modelled
    // features.
    assert!(
        converted.warnings.is_empty(),
        "unexpected warnings: {:?}",
        converted.warnings
    );
}

#[test]
fn tilemap_archive_round_trips_through_aseprite_document() {
    // Tilemap-bearing archive: a tilemap layer pointing at an inline
    // tileset, with a tilemap cel whose cells exercise FLIP_X and
    // FLIP_DIAGONAL bits.
    let tileset = Tileset {
        id: TilesetId::new(1),
        name: "ground".into(),
        tile_size: Size::new(4, 4),
        tile_count: 3,
        base_index: 1,
        source: TilesetSource::Inline {
            buffer: PixelBufferId::new(20),
        },
        user_data: UserData::default(),
    };
    let mut tilemap = TilemapData::empty(2, 2);
    tilemap.cells[0] = TileCell {
        index: TileIndex::new(1),
        flags: TileFlags::empty(),
    };
    tilemap.cells[1] = TileCell {
        index: TileIndex::new(2),
        flags: TileFlags::FLIP_X.union(TileFlags::FLIP_DIAGONAL),
    };
    tilemap.cells[3] = TileCell {
        index: TileIndex::new(2),
        flags: TileFlags::FLIP_Y,
    };
    let layer = Layer {
        id: LayerId::new(1),
        name: "ground".into(),
        kind: LayerKind::Tilemap {
            tileset: TilesetId::new(1),
        },
        blend_mode: BlendMode::Normal,
        opacity: 255,
        visible: true,
        locked: false,
        parent: None,
        user_data: UserData::default(),
    };
    let cel = Cel {
        layer_id: LayerId::new(1),
        frame_index: FrameIndex::new(0),
        position: IVec2::zero(),
        opacity: 255,
        data: CelData::Tilemap { data: tilemap },
        user_data: UserData::default(),
    };
    let sprite = Sprite {
        id: SpriteId::new(1),
        name: "level".into(),
        canvas: Size::new(8, 8),
        color_mode: ColorMode::Rgba,
        transparent_color_index: None,
        layers: vec![layer],
        frames: vec![Frame::default()],
        cels: vec![cel],
        palettes: Vec::new(),
        palette_frame_overrides: Vec::new(),
        tilesets: vec![tileset],
        frame_tags: Vec::new(),
        animations: Vec::new(),
        slices: Vec::new(),
        user_data: UserData::default(),
    };
    let buffers = vec![PixelBufferEntry {
        id: 20,
        width: 4,
        height: 12,
        stride: 16,
        pixels: rgba_solid(4, 12, [50, 100, 150, 255]),
    }];
    let project = Project {
        schema_version: SchemaVersion::current(),
        feature_flags: FeatureFlags::TILEMAPS,
        metadata: ProjectMetadata {
            name: "level".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: env!("CARGO_PKG_VERSION").into(),
        },
        sprites: vec![sprite],
        canvas: pixhaus_core::project::CanvasState::default(),
        brush: pixhaus_core::project::BrushState::default(),
        selection: pixhaus_core::project::SelectionState::default(),
    };
    let archive = PixhausArchive { project, buffers };

    let doc = archive_to_document(&archive);
    let bytes = encode(&doc).unwrap();
    let parsed = decode(&bytes).unwrap();
    let converted = document_to_archive(&parsed, "level").unwrap();
    let sprite = converted
        .archive
        .project
        .sprites
        .first()
        .expect("expected a sprite");
    let cel = sprite.cels.first().expect("expected a tilemap cel");
    match &cel.data {
        CelData::Tilemap { data } => {
            assert_eq!(data.width, 2);
            assert_eq!(data.height, 2);
            assert_eq!(data.cells[0].index, TileIndex::new(1));
            assert_eq!(data.cells[1].index, TileIndex::new(2));
            assert!(data.cells[1].flags.contains(TileFlags::FLIP_X));
            assert!(data.cells[1].flags.contains(TileFlags::FLIP_DIAGONAL));
            assert!(data.cells[3].flags.contains(TileFlags::FLIP_Y));
        }
        _ => panic!("expected tilemap cel"),
    }
}

#[test]
fn rejects_truncated_file() {
    let result = decode(&[0u8; 16]);
    assert!(result.is_err());
}

#[test]
fn rejects_wrong_file_magic() {
    let mut bytes = vec![0u8; 128];
    // file size + bad magic, then valid color depth.
    bytes[0..4].copy_from_slice(&128u32.to_le_bytes());
    bytes[4..6].copy_from_slice(&0xDEADu16.to_le_bytes());
    let result = decode(&bytes);
    assert!(result.is_err());
}

#[test]
fn unknown_chunk_types_round_trip_verbatim() {
    // Build a document with one frame whose chunks include an unknown
    // chunk type code (0x2099, not in the spec). The reader preserves
    // it as Chunk::Unknown; the writer replays the bytes verbatim.
    let mut doc = AsepriteDocument::empty(8, 8);
    doc.frames.push(DocumentFrame::new(100));
    let payload = vec![0xAA, 0xBB, 0xCC, 0xDD];
    doc.frames[0].chunks.push(Chunk::Unknown {
        code: 0x2099,
        payload: payload.clone(),
    });
    let bytes = encode(&doc).unwrap();
    let back = decode(&bytes).unwrap();
    let chunk = back.frames[0].chunks.first().expect("at least one chunk");
    match chunk {
        Chunk::Unknown { code, payload: p } => {
            assert_eq!(*code, 0x2099);
            assert_eq!(p, &payload);
        }
        _ => panic!("expected unknown chunk"),
    }
}

#[test]
fn document_warns_on_external_tileset() {
    // External-tileset reference + empty external-files chunk so the
    // reader has somewhere to hang the lookup.
    let mut doc = AsepriteDocument::empty(8, 8);
    doc.frames.push(DocumentFrame::new(100));
    doc.frames[0].chunks.push(Chunk::Tileset(TilesetChunk {
        tileset_id: 1,
        flags: 0,
        tile_count: 1,
        tile_width: 4,
        tile_height: 4,
        base_index: 1,
        name: "external".into(),
        source: TilesetSourceWire::External {
            external_file_id: 0,
            external_tileset_id: 0,
        },
    }));
    let bytes = encode(&doc).unwrap();
    let back = decode(&bytes).unwrap();
    let converted = document_to_archive(&back, "x").unwrap();
    assert!(matches!(
        converted.warnings.first(),
        Some(ConversionWarning::ExternalTilesetInlined { .. })
    ));
}
