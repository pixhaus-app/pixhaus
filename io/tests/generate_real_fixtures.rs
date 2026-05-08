//! Generates the binary `.aseprite` fixtures under
//! `examples/aseprite-roundtrip/`.
//!
//! Gated on the `PIXHAUS_REGEN_ASEPRITE_FIXTURES` environment variable so
//! it doesn't run in normal CI: the committed fixtures are the source of
//! truth, and this generator is the thing that produces them when the
//! writer's wire format intentionally changes.
//!
//! Usage:
//!
//! ```text
//! PIXHAUS_REGEN_ASEPRITE_FIXTURES=1 cargo nextest run -p pixhaus-io \
//!     --test generate_real_fixtures
//! ```
//!
//! Each generated file maps 1:1 to a case the brief calls out and is
//! consumed verbatim by `tests/aseprite_real_fixtures.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::disallowed_methods,
    clippy::cast_possible_truncation
)]

use std::path::PathBuf;

use pixhaus_core::project::{
    BlendMode, Cel, CelData, ColorMode, Frame, FrameIndex, FrameRange, FrameTag, IVec2, Layer,
    LayerId, LayerKind, LoopDirection, Palette, PaletteEntry, PaletteId, PixelBufferId, Project,
    Rgba, Size, Sprite, SpriteId, TileCell, TileFlags, TileIndex, TilemapData, Tileset, TilesetId,
    TilesetSource, UserData,
};
use pixhaus_io::aseprite::{archive_to_document, encode};
use pixhaus_io::pixhaus::{PixelBufferEntry, PixhausArchive};

const REGEN_ENV: &str = "PIXHAUS_REGEN_ASEPRITE_FIXTURES";

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("examples")
        .join("aseprite-roundtrip")
}

fn write_fixture(name: &str, archive: &PixhausArchive) {
    let dir = fixture_dir();
    std::fs::create_dir_all(&dir).expect("create fixture directory");
    let doc = archive_to_document(archive);
    let bytes = encode(&doc).expect("encode aseprite bytes");
    let path = dir.join(name);
    std::fs::write(&path, &bytes).expect("write fixture file");
}

fn rgba_solid(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for _ in 0..(width * height) {
        out.extend_from_slice(&color);
    }
    out
}

fn raster_layer(id: u32, name: &str, blend: BlendMode, parent: Option<LayerId>) -> Layer {
    Layer {
        id: LayerId::new(id),
        name: name.into(),
        kind: LayerKind::Raster,
        blend_mode: blend,
        opacity: 255,
        visible: true,
        locked: false,
        parent,
        user_data: UserData::default(),
    }
}

fn group_layer(id: u32, name: &str, blend: BlendMode) -> Layer {
    Layer {
        id: LayerId::new(id),
        name: name.into(),
        kind: LayerKind::Group { collapsed: false },
        blend_mode: blend,
        opacity: 255,
        visible: true,
        locked: false,
        parent: None,
        user_data: UserData::default(),
    }
}

fn raster_cel(layer: u32, frame: u32, buffer: u32, size: Size) -> Cel {
    Cel {
        layer_id: LayerId::new(layer),
        frame_index: FrameIndex::new(frame),
        position: IVec2::zero(),
        opacity: 255,
        data: CelData::Raster {
            buffer: PixelBufferId::new(buffer),
            size,
        },
        user_data: UserData::default(),
    }
}

fn buffer(id: u32, width: u32, height: u32, bpp: u32, pixels: Vec<u8>) -> PixelBufferEntry {
    PixelBufferEntry {
        id,
        width,
        height,
        stride: width * bpp,
        pixels,
    }
}

fn archive_with_sprite(
    name: &str,
    sprite: Sprite,
    buffers: Vec<PixelBufferEntry>,
) -> PixhausArchive {
    let mut project = Project::new(name);
    project.sprites = vec![sprite];
    PixhausArchive { project, buffers }
}

// ── case (a) single-frame single-layer raster ───────────────────────────────

fn single_frame_rgba() -> PixhausArchive {
    let layer = raster_layer(1, "main", BlendMode::Normal, None);
    let cel = raster_cel(1, 0, 10, Size::new(8, 8));
    let buf = buffer(10, 8, 8, 4, rgba_solid(8, 8, [200, 100, 50, 255]));

    let mut sprite = Sprite::empty(SpriteId::new(1), "single-frame-rgba", Size::new(8, 8));
    sprite.color_mode = ColorMode::Rgba;
    sprite.layers = vec![layer];
    sprite.frames = vec![Frame {
        duration_ms: 100,
        user_data: UserData::default(),
    }];
    sprite.cels = vec![cel];

    archive_with_sprite("single-frame-rgba", sprite, vec![buf])
}

// ── case (b) multi-frame animation with a frame tag ─────────────────────────

fn multi_frame_tags() -> PixhausArchive {
    let layer = raster_layer(1, "main", BlendMode::Normal, None);
    let frames = (0..3)
        .map(|_| Frame {
            duration_ms: 80,
            user_data: UserData::default(),
        })
        .collect();
    let cels = vec![
        raster_cel(1, 0, 10, Size::new(8, 8)),
        raster_cel(1, 1, 11, Size::new(8, 8)),
        raster_cel(1, 2, 12, Size::new(8, 8)),
    ];
    let buffers = vec![
        buffer(10, 8, 8, 4, rgba_solid(8, 8, [255, 0, 0, 255])),
        buffer(11, 8, 8, 4, rgba_solid(8, 8, [0, 255, 0, 255])),
        buffer(12, 8, 8, 4, rgba_solid(8, 8, [0, 0, 255, 255])),
    ];

    let mut sprite = Sprite::empty(SpriteId::new(1), "multi-frame-tags", Size::new(8, 8));
    sprite.color_mode = ColorMode::Rgba;
    sprite.layers = vec![layer];
    sprite.frames = frames;
    sprite.cels = cels;
    sprite.frame_tags = vec![FrameTag {
        name: "loop".into(),
        range: FrameRange::new(FrameIndex::new(0), FrameIndex::new(2)),
        loop_direction: LoopDirection::Forward,
        repeat: 0,
        user_data: UserData::default(),
    }];

    archive_with_sprite("multi-frame-tags", sprite, buffers)
}

// ── case (c) group with Multiply blend over a raster child ──────────────────

fn group_multiply() -> PixhausArchive {
    let group = group_layer(1, "fx", BlendMode::Multiply);
    let child = raster_layer(2, "main", BlendMode::Normal, Some(LayerId::new(1)));
    let cel = raster_cel(2, 0, 10, Size::new(8, 8));
    let buf = buffer(10, 8, 8, 4, rgba_solid(8, 8, [128, 128, 128, 255]));

    let mut sprite = Sprite::empty(SpriteId::new(1), "group-multiply", Size::new(8, 8));
    sprite.color_mode = ColorMode::Rgba;
    sprite.layers = vec![group, child];
    sprite.frames = vec![Frame {
        duration_ms: 100,
        user_data: UserData::default(),
    }];
    sprite.cels = vec![cel];

    archive_with_sprite("group-multiply", sprite, vec![buf])
}

// ── case (d) indexed sprite with palette + transparent index ────────────────

fn indexed_with_palette() -> PixhausArchive {
    let layer = raster_layer(1, "main", BlendMode::Normal, None);
    // Indexed pixel data: each byte is a palette index.
    let pixels: Vec<u8> = (0u8..(8 * 8)).map(|i| i % 4).collect();
    let cel = raster_cel(1, 0, 10, Size::new(8, 8));
    let buf = buffer(10, 8, 8, 1, pixels);
    let palette = Palette {
        id: PaletteId::new(1),
        name: "default".into(),
        colors: vec![
            PaletteEntry::new(Rgba::transparent()),
            PaletteEntry {
                color: Rgba::opaque(220, 60, 60),
                name: Some("red".into()),
            },
            PaletteEntry {
                color: Rgba::opaque(60, 220, 60),
                name: Some("green".into()),
            },
            PaletteEntry {
                color: Rgba::opaque(60, 60, 220),
                name: Some("blue".into()),
            },
        ],
        user_data: UserData::default(),
    };

    let mut sprite = Sprite::empty(SpriteId::new(1), "indexed-with-palette", Size::new(8, 8));
    sprite.color_mode = ColorMode::Indexed;
    sprite.transparent_color_index = Some(0);
    sprite.layers = vec![layer];
    sprite.frames = vec![Frame {
        duration_ms: 100,
        user_data: UserData::default(),
    }];
    sprite.cels = vec![cel];
    sprite.palettes = vec![palette];

    archive_with_sprite("indexed-with-palette", sprite, vec![buf])
}

// ── case (e) tilemap layer + tileset ────────────────────────────────────────

fn tilemap_with_tileset() -> PixhausArchive {
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
    // 2x2 tilemap (8x8 canvas at 4x4 tiles).
    let mut tilemap = TilemapData::empty(2, 2);
    tilemap.cells[0] = TileCell {
        index: TileIndex::new(1),
        flags: TileFlags::empty(),
    };
    tilemap.cells[1] = TileCell {
        index: TileIndex::new(2),
        flags: TileFlags::FLIP_X,
    };
    tilemap.cells[2] = TileCell {
        index: TileIndex::new(2),
        flags: TileFlags::FLIP_Y,
    };
    tilemap.cells[3] = TileCell {
        index: TileIndex::new(1),
        flags: TileFlags::FLIP_X.union(TileFlags::FLIP_Y),
    };
    let cel = Cel {
        layer_id: LayerId::new(1),
        frame_index: FrameIndex::new(0),
        position: IVec2::zero(),
        opacity: 255,
        data: CelData::Tilemap { data: tilemap },
        user_data: UserData::default(),
    };
    // Tileset: 3 tiles of 4x4, packed vertically (4 wide × 12 tall).
    let mut tile_pixels = Vec::with_capacity(4 * 12 * 4);
    tile_pixels.extend(rgba_solid(4, 4, [255, 255, 255, 0])); // empty tile
    tile_pixels.extend(rgba_solid(4, 4, [200, 50, 50, 255])); // tile 1
    tile_pixels.extend(rgba_solid(4, 4, [50, 200, 50, 255])); // tile 2
    let tileset = Tileset {
        id: TilesetId::new(1),
        name: "ground-tiles".into(),
        tile_size: Size::new(4, 4),
        tile_count: 3,
        base_index: 1,
        source: TilesetSource::Inline {
            buffer: PixelBufferId::new(20),
        },
        properties: Vec::new(),
        autotile: None,
        user_data: UserData::default(),
    };
    let buf = buffer(20, 4, 12, 4, tile_pixels);

    let mut sprite = Sprite::empty(SpriteId::new(1), "tilemap-with-tileset", Size::new(8, 8));
    sprite.color_mode = ColorMode::Rgba;
    sprite.layers = vec![layer];
    sprite.frames = vec![Frame {
        duration_ms: 100,
        user_data: UserData::default(),
    }];
    sprite.cels = vec![cel];
    sprite.tilesets = vec![tileset];

    archive_with_sprite("tilemap-with-tileset", sprite, vec![buf])
}

#[test]
fn generate_real_fixtures() {
    if std::env::var(REGEN_ENV).is_err() {
        // Default path: do nothing. The generator only runs when
        // explicitly opted into via the env var so CI doesn't churn the
        // fixture files on every test pass.
        return;
    }
    write_fixture("single-frame-rgba.aseprite", &single_frame_rgba());
    write_fixture("multi-frame-tags.aseprite", &multi_frame_tags());
    write_fixture("group-multiply.aseprite", &group_multiply());
    write_fixture("indexed-with-palette.aseprite", &indexed_with_palette());
    write_fixture("tilemap-with-tileset.aseprite", &tilemap_with_tileset());
}
