//! Generates sample `.pixhaus` project files for S45.
//!
//! Gated on the `PIXHAUS_REGEN_SAMPLES` environment variable so it does
//! not run in normal CI — the committed binaries are the source of truth
//! and this generator only runs when the wire format intentionally changes
//! or new samples are added.
//!
//! Usage:
//!
//! ```text
//! PIXHAUS_REGEN_SAMPLES=1 cargo nextest run -p pixhaus-io \
//!     --test generate_sample_projects
//! ```
//!
//! Output: `examples/samples/` relative to the workspace root.
//!
//! Pixel data is procedurally generated placeholder art. Palette
//! discipline (limited colors, named entries) is the deliverable, not
//! pixel fidelity. The layer/frame/animation structure matches the S45
//! brief exactly.

#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::collapsible_match,
    clippy::disallowed_methods,
    clippy::drop_non_drop,
    clippy::expect_used,
    clippy::match_same_arms,
    clippy::missing_panics_doc,
    clippy::panic,
    clippy::print_stdout,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    clippy::unwrap_used
)]

use std::path::PathBuf;

use pixhaus_core::project::{
    ActiveTarget, AiMetadata, AnimLoopMode, Animation, AnimationId, BlendMode, BrushShape,
    BrushState, CanvasState, Cel, CelData, CollisionShape, ColorMode, Entity, EntityContent,
    EntityDefaults, EntityId, EntityKind, FeatureFlags, Frame, FrameIndex, FrameRange, FrameTag,
    IVec2, Layer, LayerId, LayerKind, LoopDirection, NamedSprite, NineSlice, Palette, PaletteEntry,
    PaletteId, Pivot, PixelBufferId, Project, Rect, Rgba, Size, Slice, SliceId, SliceKey, Sprite,
    SpriteId, StateId, TileAnimation, TileAnimationFrame, TileCell, TileFlags, TileIndex,
    TileProperties, TilemapData, Tileset, TilesetId, TilesetSource, UserData,
};
use pixhaus_io::pixhaus::{PixelBufferEntry, PixhausArchive, encode_to_file};

/// Wraps a single sprite into the library as a `Custom`-kind entity
/// with one primary state. Replacement for the removed
/// `project.sprites = vec![sprite]` shorthand used throughout these
/// generators. Each project still has at most one sprite, so the
/// fixed `EntityId::new(1)` / `StateId::new(1)` is fine; the helper
/// also sets `project.active` so consumers that ask "what's the
/// editor on" continue to resolve.
fn install_sprite_as_entity(project: &mut Project, sprite: Sprite, entity_name: &str) {
    let entity = Entity {
        id: EntityId::new(1),
        kind: EntityKind::Custom("Sample".into()),
        name: entity_name.into(),
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Sprites {
            states: vec![NamedSprite {
                id: StateId::new(1),
                state_name: "primary".into(),
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
    project.library.entities.push(entity);
    project.active = ActiveTarget::State {
        entity_id: EntityId::new(1),
        state_id: StateId::new(1),
    };
}

const REGEN_ENV: &str = "PIXHAUS_REGEN_SAMPLES";

fn sample_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("examples")
        .join("samples")
}

fn write_sample(name: &str, archive: &PixhausArchive) {
    let dir = sample_dir();
    std::fs::create_dir_all(&dir).expect("create samples dir");
    let path = dir.join(name);
    encode_to_file(archive, &path).expect("encode sample");
    let size = std::fs::metadata(&path).map_or(0, |m| m.len());
    println!("wrote {} ({size} bytes)", path.display());
}

// ── pixel data helpers ───────────────────────────────────────────────────────

/// Indexed 1 byte/pixel frame: solid `fill` inside a 1px `border`, with a
/// progress bar on the bottom-inside row showing `frame / total`.
fn indexed_frame(w: u32, h: u32, fill: u8, border: u8, frame: u32, total: u32) -> Vec<u8> {
    let w = w as usize;
    let h = h as usize;
    let mut pixels = vec![fill; w * h];
    for x in 0..w {
        pixels[x] = border;
        pixels[(h - 1) * w + x] = border;
    }
    for y in 0..h {
        pixels[y * w] = border;
        pixels[y * w + w - 1] = border;
    }
    if h >= 4 && total > 0 {
        let filled = ((frame as usize * (w - 2)) / total as usize).min(w - 2);
        for x in 1..=filled {
            pixels[(h - 2) * w + x] = 13;
        }
    }
    pixels
}

/// Like `indexed_frame` but adds a 1px corner pixel for the directional
/// indicator. `corner` is 0-7 clockwise from south.
fn indexed_dir_frame(
    w: u32,
    h: u32,
    fill: u8,
    border: u8,
    frame: u32,
    total: u32,
    corner: u8,
    dot: u8,
) -> Vec<u8> {
    let mut pixels = indexed_frame(w, h, fill, border, frame, total);
    let w = w as usize;
    let h = h as usize;
    let (cx, cy): (usize, usize) = match corner {
        0 => (w / 2, h - 2), // S — bottom-centre
        1 => (1, h - 2),     // SW — bottom-left inside
        2 => (1, h / 2),     // W — mid-left inside
        3 => (1, 1),         // NW — top-left inside
        4 => (w / 2, 1),     // N — top-centre
        5 => (w - 2, 1),     // NE — top-right inside
        6 => (w - 2, h / 2), // E — mid-right inside
        7 => (w - 2, h - 2), // SE — bottom-right inside
        _ => (w / 2, h / 2),
    };
    pixels[cy * w + cx] = dot;
    pixels
}

/// RGBA 4 bytes/pixel: `fill` everywhere, `border` color for a 2px margin.
fn rgba_bordered(w: u32, h: u32, border: [u8; 4], fill: [u8; 4]) -> Vec<u8> {
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let is_border = x < 2 || x >= w - 2 || y < 2 || y >= h - 2;
            let color = if is_border { border } else { fill };
            let off = ((y * w + x) * 4) as usize;
            pixels[off..off + 4].copy_from_slice(&color);
        }
    }
    pixels
}

/// RGBA vertical tileset strip: `tile_colors.len()` tiles of `(tw, th)`
/// stacked top-to-bottom, each with a 1px darker border.
fn tileset_strip(tw: u32, th: u32, tile_colors: &[[u8; 4]]) -> Vec<u8> {
    let n = tile_colors.len() as u32;
    let total_h = th * n;
    let mut pixels = vec![0u8; (tw * total_h * 4) as usize];
    for (i, &c) in tile_colors.iter().enumerate() {
        let base_y = i as u32 * th;
        let dark = [c[0] / 2, c[1] / 2, c[2] / 2, c[3]];
        for y in base_y..base_y + th {
            for x in 0..tw {
                let is_border = x == 0 || x == tw - 1 || y == base_y || y == base_y + th - 1;
                let color = if is_border { dark } else { c };
                let off = ((y * tw + x) * 4) as usize;
                pixels[off..off + 4].copy_from_slice(&color);
            }
        }
    }
    pixels
}

// ── palette helpers ──────────────────────────────────────────────────────────

fn make_palette(id: u32, name: &str, entries: Vec<(&str, Rgba)>) -> Palette {
    Palette {
        id: PaletteId::new(id),
        name: name.into(),
        colors: entries
            .into_iter()
            .map(|(label, color)| {
                let n = if label.is_empty() {
                    None
                } else {
                    Some(label.into())
                };
                PaletteEntry { color, name: n }
            })
            .collect(),
        user_data: UserData::default(),
    }
}

fn knight_palette() -> Palette {
    make_palette(
        1,
        "knight",
        vec![
            ("transparent", Rgba::transparent()),
            ("outline", Rgba::opaque(10, 10, 15)),
            ("shadow", Rgba::opaque(25, 25, 45)),
            ("armor-dark", Rgba::opaque(45, 55, 105)),
            ("armor", Rgba::opaque(65, 85, 165)),
            ("armor-light", Rgba::opaque(100, 130, 210)),
            ("skin-dark", Rgba::opaque(110, 65, 50)),
            ("skin", Rgba::opaque(185, 125, 90)),
            ("skin-light", Rgba::opaque(230, 185, 155)),
            ("gold-dark", Rgba::opaque(105, 75, 20)),
            ("gold", Rgba::opaque(195, 155, 50)),
            ("gold-light", Rgba::opaque(255, 215, 0)),
            ("hit", Rgba::opaque(200, 40, 50)),
            ("white", Rgba::opaque(255, 255, 255)),
            ("steel-dark", Rgba::opaque(55, 55, 65)),
            ("steel", Rgba::opaque(140, 150, 165)),
        ],
    )
}

fn slime_palette() -> Palette {
    make_palette(
        1,
        "slime",
        vec![
            ("transparent", Rgba::transparent()),
            ("outline", Rgba::opaque(10, 10, 15)),
            ("body-dark", Rgba::opaque(20, 80, 20)),
            ("body", Rgba::opaque(40, 160, 40)),
            ("body-light", Rgba::opaque(80, 220, 80)),
            ("highlight", Rgba::opaque(150, 255, 150)),
            ("hit", Rgba::opaque(200, 40, 40)),
            ("white", Rgba::opaque(255, 255, 255)),
            ("body-shadow", Rgba::opaque(20, 120, 20)),
            ("eye", Rgba::opaque(255, 220, 50)),
        ],
    )
}

// ── layer helpers ────────────────────────────────────────────────────────────

fn raster(id: u32, name: &str, blend: BlendMode, opacity: u8, parent: Option<u32>) -> Layer {
    Layer {
        id: LayerId::new(id),
        name: name.into(),
        kind: LayerKind::Raster,
        blend_mode: blend,
        opacity,
        visible: true,
        locked: false,
        parent: parent.map(LayerId::new),
        user_data: UserData::default(),
    }
}

fn group(id: u32, name: &str) -> Layer {
    Layer {
        id: LayerId::new(id),
        name: name.into(),
        kind: LayerKind::Group { collapsed: false },
        blend_mode: BlendMode::Normal,
        opacity: 255,
        visible: true,
        locked: false,
        parent: None,
        user_data: UserData::default(),
    }
}

fn tilemap(id: u32, name: &str, tileset_id: u32) -> Layer {
    Layer {
        id: LayerId::new(id),
        name: name.into(),
        kind: LayerKind::Tilemap {
            tileset: TilesetId::new(tileset_id),
        },
        blend_mode: BlendMode::Normal,
        opacity: 255,
        visible: true,
        locked: false,
        parent: None,
        user_data: UserData::default(),
    }
}

// ── buffer + cel helpers ─────────────────────────────────────────────────────

fn idx_buf(id: u32, w: u32, h: u32, pixels: Vec<u8>) -> PixelBufferEntry {
    PixelBufferEntry {
        id,
        width: w,
        height: h,
        stride: w,
        pixels,
    }
}

fn rgba_buf(id: u32, w: u32, h: u32, pixels: Vec<u8>) -> PixelBufferEntry {
    PixelBufferEntry {
        id,
        width: w,
        height: h,
        stride: w * 4,
        pixels,
    }
}

fn rcel(layer: u32, frame: u32, buf: u32, w: u32, h: u32) -> Cel {
    Cel::raster(
        LayerId::new(layer),
        FrameIndex::new(frame),
        PixelBufferId::new(buf),
        Size::new(w, h),
    )
}

fn linked_cel(layer: u32, frame: u32, source: u32) -> Cel {
    Cel {
        layer_id: LayerId::new(layer),
        frame_index: FrameIndex::new(frame),
        position: IVec2::zero(),
        opacity: 255,
        data: CelData::Linked {
            source_frame: FrameIndex::new(source),
        },
        user_data: UserData::default(),
    }
}

fn ms(duration_ms: u32) -> Frame {
    Frame {
        duration_ms,
        user_data: UserData::default(),
    }
}

fn tag(name: &str, start: u32, end: u32, dir: LoopDirection) -> FrameTag {
    FrameTag {
        name: name.into(),
        range: FrameRange::new(FrameIndex::new(start), FrameIndex::new(end)),
        loop_direction: dir,
        repeat: 0,
        user_data: UserData::default(),
    }
}

fn anim(id: u32, name: &str, start: u32, end: u32, dir: LoopDirection, speed: f32) -> Animation {
    Animation {
        id: AnimationId::new(id),
        name: name.into(),
        range: FrameRange::new(FrameIndex::new(start), FrameIndex::new(end)),
        loop_direction: dir,
        speed_multiplier: speed,
        user_data: UserData::default(),
    }
}

/// Maps a compass-direction string to the 0-7 corner offset used by
/// `indexed_dir_frame()` so directional animations differ visually.
fn direction_to_corner(dir: &str) -> u8 {
    match dir {
        "s" => 0,
        "sw" => 1,
        "w" => 2,
        "nw" => 3,
        "n" => 4,
        "ne" => 5,
        "e" => 6,
        "se" => 7,
        _ => 0,
    }
}

// ── S45(a): character-knight.pixhaus ────────────────────────────────────────
//
// 32×32 indexed sprite, 167 frames. Layer 1 ("armor") carries all cel pixel
// data; the other layers are defined for structure but have no cels in the
// placeholder. Tags and Animations cover every S45-specified state.

fn build_knight() -> PixhausArchive {
    const W: u32 = 32;
    const H: u32 = 32;

    // Accumulation targets for the closure below.
    let mut all_frames: Vec<Frame> = Vec::new();
    let mut all_cels: Vec<Cel> = Vec::new();
    let mut all_buffers: Vec<PixelBufferEntry> = Vec::new();
    let mut all_tags: Vec<FrameTag> = Vec::new();
    let mut all_anims: Vec<Animation> = Vec::new();
    let mut fc: u32 = 0; // frame cursor
    let mut bc: u32 = 1000; // buffer id counter
    let mut ac: u32 = 1; // animation id counter

    // Adds `count` indexed frames for layer 1 with a directional dot, then
    // pushes a FrameTag and (if `add_anim`) an Animation.
    let mut push = |name: &str,
                    count: u32,
                    duration_ms: u32,
                    fill: u8,
                    dir: LoopDirection,
                    add_anim: bool,
                    dot_corner: Option<u8>| {
        let start = fc;
        for i in 0..count {
            all_frames.push(ms(duration_ms));
            let pixels = match dot_corner {
                Some(corner) => indexed_dir_frame(W, H, fill, 1, i, count, corner, 13),
                None => indexed_frame(W, H, fill, 1, i, count),
            };
            all_buffers.push(idx_buf(bc, W, H, pixels));
            all_cels.push(rcel(1, fc, bc, W, H));
            bc += 1;
            fc += 1;
        }
        let end = fc - 1;
        all_tags.push(tag(name, start, end, dir));
        if add_anim {
            all_anims.push(anim(ac, name, start, end, dir, 1.0));
            ac += 1;
        }
    };

    // idle (4 frames, PingPong)
    push("idle", 4, 120, 4, LoopDirection::PingPong, true, None);

    // walk-8dir (8 frames × 8 directions). Each direction is a real
    // engine-side animation entry, not just a frame tag — the showcase
    // sample's whole point is to exercise all eight handoff paths.
    for dir in ["s", "sw", "w", "nw", "n", "ne", "e", "se"] {
        push(
            &format!("walk-{dir}"),
            8,
            100,
            4,
            LoopDirection::Forward,
            true,
            Some(direction_to_corner(dir)),
        );
    }

    // run-8dir (8 frames × 8 directions)
    for dir in ["s", "sw", "w", "nw", "n", "ne", "e", "se"] {
        push(
            &format!("run-{dir}"),
            8,
            60,
            5, // armor-light: slightly brighter to hint at speed
            LoopDirection::Forward,
            true,
            Some(direction_to_corner(dir)),
        );
    }

    // attack-4dir (6 frames × 4 directions)
    for (i, &dir) in ["s", "w", "n", "e"].iter().enumerate() {
        push(
            &format!("attack-{dir}"),
            6,
            80,
            10, // gold: weapon swing
            LoopDirection::Forward,
            true,
            Some((i * 2) as u8), // S→0, W→2, N→4, E→6
        );
    }

    // hurt (3 frames)
    push("hurt", 3, 80, 12, LoopDirection::Forward, true, None);

    // death (8 frames)
    push("death", 8, 120, 14, LoopDirection::Forward, true, None);

    // Release mutable borrows so the accumulated vecs are accessible below.
    drop(push);

    // Layer stack (bottom to top): shadow, knight group, armor (in group),
    // skin (in group), weapon, outline.
    let layers = vec![
        // Pixhaus stores layers bottom-to-top (index 0 = bottommost).
        // Order so the shadow renders first (behind everything), then
        // the knight group with armor → skin → outline stacking
        // inside, then the weapon on top of the character. The R1
        // version had this stack inverted, putting the shadow at the
        // top.
        raster(6, "shadow", BlendMode::Multiply, 100, None),
        group(2, "knight"),
        Layer {
            id: LayerId::new(1),
            name: "armor".into(),
            kind: LayerKind::Raster,
            blend_mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            locked: false,
            parent: Some(LayerId::new(2)),
            user_data: UserData::default(),
        },
        raster(3, "skin", BlendMode::Normal, 255, Some(2)),
        raster(4, "outline", BlendMode::Normal, 255, Some(2)),
        raster(5, "weapon", BlendMode::Normal, 255, None),
    ];

    let mut sprite = Sprite::empty(SpriteId::new(1), "knight-32x32", Size::new(W, H));
    sprite.color_mode = ColorMode::Indexed;
    sprite.transparent_color_index = Some(0);
    sprite.layers = layers;
    sprite.frames = all_frames;
    sprite.cels = all_cels;
    sprite.palettes = vec![knight_palette()];
    sprite.frame_tags = all_tags;
    sprite.animations = all_anims;

    let mut project = Project::new("character-knight");
    project.metadata.description =
        Some("32×32 knight character. Indexed, 16-color palette. S45 sample.".into());
    project.metadata.author = Some("pixhaus".into());
    project.feature_flags = FeatureFlags::ANIMATIONS;
    install_sprite_as_entity(&mut project, sprite, "Knight");
    project.canvas = CanvasState {
        active_layer: Some(LayerId::new(1)),
        active_frame: Some(FrameIndex::new(0)),
        scroll_x: 0.0,
        scroll_y: 0.0,
        zoom: 8.0,
        onion_skin: true,
        show_tile_grid: false,
    };
    project.brush = BrushState {
        shape: BrushShape::Square,
        size: 1,
        foreground_index: 4,
        background_index: 0,
        active_palette: Some(PaletteId::new(1)),
    };

    PixhausArchive {
        project,
        buffers: all_buffers,
    }
}

// ── S45(b): tileset-forest.pixhaus ──────────────────────────────────────────
//
// 16×16 RGBA tileset with 17 tiles (0=empty, 1-4=grass, 5-8=dirt,
// 9-12=stone, 13-15=water animation, 16=flower). Three animation frames for
// water tiles via TileAnimation + TileProperties. Canvas matches the tile
// strip so the sprite preview is the tileset atlas itself.

const FOREST_TILE_SIZE: u32 = 16;
const FOREST_TILE_COUNT: u32 = 17; // 0..=16

fn forest_tile_colors() -> Vec<[u8; 4]> {
    vec![
        [0, 0, 0, 0],         // 0: empty (transparent)
        [55, 100, 35, 255],   // 1: grass-a
        [70, 120, 45, 255],   // 2: grass-b
        [45, 85, 25, 255],    // 3: grass-dark
        [90, 140, 60, 255],   // 4: grass-light
        [140, 90, 45, 255],   // 5: dirt-a
        [160, 110, 55, 255],  // 6: dirt-b
        [120, 75, 35, 255],   // 7: dirt-dark
        [175, 130, 75, 255],  // 8: dirt-path
        [110, 100, 90, 255],  // 9: stone-a
        [130, 120, 110, 255], // 10: stone-b
        [95, 85, 75, 255],    // 11: stone-dark
        [80, 75, 70, 255],    // 12: stone-wall
        [50, 100, 185, 255],  // 13: water-a (frame 0)
        [65, 115, 200, 255],  // 14: water-b (frame 1)
        [40, 90, 170, 255],   // 15: water-c (frame 2)
        [200, 120, 150, 255], // 16: flower decoration
    ]
}

/// Returns the pixel buffer for the forest tileset atlas (17 tiles stacked).
fn forest_tileset_pixels() -> Vec<u8> {
    tileset_strip(FOREST_TILE_SIZE, FOREST_TILE_SIZE, &forest_tile_colors())
}

/// Per-tile metadata for the 17-tile forest tileset. Stone tiles
/// (indices 9-12) carry full collision; tile 13 (water) has the
/// `water_anim` animation. Shared between the standalone tileset
/// sample and the level sample so the level's inlined tileset keeps
/// the same collision / animation contract.
fn forest_tile_properties(n: u32, water_anim: TileAnimation) -> Vec<TileProperties> {
    let mut props: Vec<TileProperties> = (0..n).map(|_| TileProperties::default()).collect();
    for i in 9..=12 {
        if let Some(p) = props.get_mut(i as usize) {
            p.collision = CollisionShape::Full;
        }
    }
    if let Some(p) = props.get_mut(13) {
        p.animation = Some(water_anim);
    }
    props
}

/// Build the water animation referenced by tile 13 in the forest
/// tileset. Hoisted so the level sample can reproduce it without
/// duplicating frame literals.
fn forest_water_anim() -> TileAnimation {
    TileAnimation {
        frames: vec![
            TileAnimationFrame {
                tile_index: TileIndex::new(13),
                duration_ms: 150,
            },
            TileAnimationFrame {
                tile_index: TileIndex::new(14),
                duration_ms: 150,
            },
            TileAnimationFrame {
                tile_index: TileIndex::new(15),
                duration_ms: 150,
            },
        ],
        loop_mode: AnimLoopMode::Loop,
    }
}

fn build_forest_tileset() -> PixhausArchive {
    let tw = FOREST_TILE_SIZE;
    let th = FOREST_TILE_SIZE;
    let n = FOREST_TILE_COUNT;

    // Atlas buffer: all 17 tiles stacked vertically.
    let atlas_pixels = forest_tileset_pixels();
    let atlas = rgba_buf(100, tw, th * n, atlas_pixels);

    // The sprite canvas is the atlas strip itself (16 × 272).
    // Layer 1 holds the tile atlas as a raster layer so the editor shows
    // the full tileset in the preview.
    let atlas_layer = raster(1, "tileset-atlas", BlendMode::Normal, 255, None);

    // Three frames: one per water animation cycle step. The atlas itself
    // doesn't change between frames (static tiles are the same); water
    // tiles are distinguished by TileAnimation at runtime. Frame 0 shows
    // the base atlas.
    let frames = vec![ms(150), ms(150), ms(150)];

    // Three cels — all link to the same atlas (frame 0), because the atlas
    // image is static; the runtime swaps tile_index per TileAnimation.
    let cels = vec![
        rcel(1, 0, 100, tw, th * n),
        linked_cel(1, 1, 0),
        linked_cel(1, 2, 0),
    ];

    // Water animation: tile 13 cycles through tiles 13→14→15 at 150 ms each.
    let properties = forest_tile_properties(n, forest_water_anim());

    let tileset = Tileset {
        id: TilesetId::new(1),
        name: "forest".into(),
        tile_size: Size::new(tw, th),
        tile_count: n,
        base_index: 1,
        source: TilesetSource::Inline {
            buffer: PixelBufferId::new(100),
        },
        properties,
        autotile: None,
        user_data: UserData::default(),
    };

    let mut sprite = Sprite::empty(SpriteId::new(1), "forest-tileset", Size::new(tw, th * n));
    sprite.color_mode = ColorMode::Rgba;
    sprite.layers = vec![atlas_layer];
    sprite.frames = frames;
    sprite.cels = cels;
    sprite.tilesets = vec![tileset];
    sprite.frame_tags = vec![
        tag("water-a", 0, 0, LoopDirection::Forward),
        tag("water-b", 1, 1, LoopDirection::Forward),
        tag("water-c", 2, 2, LoopDirection::Forward),
    ];
    sprite.animations = vec![anim(1, "Water", 0, 2, LoopDirection::Forward, 1.0)];

    let mut project = Project::new("tileset-forest");
    project.metadata.description = Some(
        "16×16 forest tileset. Grass/dirt/stone/water tiles with animated water. S45 sample."
            .into(),
    );
    project.metadata.author = Some("pixhaus".into());
    project.feature_flags = FeatureFlags::TILEMAPS.union(FeatureFlags::ANIMATIONS);
    install_sprite_as_entity(&mut project, sprite, "Forest Tileset");
    project.canvas = CanvasState {
        active_layer: Some(LayerId::new(1)),
        active_frame: Some(FrameIndex::new(0)),
        scroll_x: 0.0,
        scroll_y: 0.0,
        zoom: 4.0,
        onion_skin: false,
        show_tile_grid: true,
    };

    PixhausArchive {
        project,
        buffers: vec![atlas],
    }
}

// ── S45(c): enemy-slime.pixhaus ─────────────────────────────────────────────
//
// 16×16 indexed sprite, 21 frames (idle 4, hop 6, hit 3, split 8).

fn build_slime() -> PixhausArchive {
    const W: u32 = 16;
    const H: u32 = 16;

    let mut all_frames: Vec<Frame> = Vec::new();
    let mut all_cels: Vec<Cel> = Vec::new();
    let mut all_buffers: Vec<PixelBufferEntry> = Vec::new();
    let mut all_tags: Vec<FrameTag> = Vec::new();
    let mut all_anims: Vec<Animation> = Vec::new();
    let mut fc: u32 = 0;
    let mut bc: u32 = 2000;
    let mut ac: u32 = 1;

    let mut push = |name: &str, count: u32, duration_ms: u32, fill: u8, dir: LoopDirection| {
        let start = fc;
        for i in 0..count {
            all_frames.push(ms(duration_ms));
            let pixels = indexed_frame(W, H, fill, 1, i, count);
            all_buffers.push(idx_buf(bc, W, H, pixels));
            all_cels.push(rcel(1, fc, bc, W, H));
            bc += 1;
            fc += 1;
        }
        let end = fc - 1;
        all_tags.push(tag(name, start, end, dir));
        all_anims.push(anim(ac, name, start, end, dir, 1.0));
        ac += 1;
    };

    push("idle", 4, 150, 3, LoopDirection::PingPong); // body
    push("hop", 6, 80, 4, LoopDirection::Forward); // body-light: squash/stretch
    push("hit", 3, 60, 6, LoopDirection::Forward); // hit flash
    push("split", 8, 80, 8, LoopDirection::Forward); // body-shadow: splitting

    drop(push);

    let layers = vec![
        raster(1, "body", BlendMode::Normal, 255, None),
        raster(2, "outline", BlendMode::Normal, 255, None),
        raster(3, "eyes", BlendMode::Normal, 255, None),
    ];

    let mut sprite = Sprite::empty(SpriteId::new(1), "slime-16x16", Size::new(W, H));
    sprite.color_mode = ColorMode::Indexed;
    sprite.transparent_color_index = Some(0);
    sprite.layers = layers;
    sprite.frames = all_frames;
    sprite.cels = all_cels;
    sprite.palettes = vec![slime_palette()];
    sprite.frame_tags = all_tags;
    sprite.animations = all_anims;

    let mut project = Project::new("enemy-slime");
    project.metadata.description =
        Some("16×16 slime enemy. Indexed, 10-color palette. S45 sample.".into());
    project.metadata.author = Some("pixhaus".into());
    project.feature_flags = FeatureFlags::ANIMATIONS;
    install_sprite_as_entity(&mut project, sprite, "Slime");
    project.canvas = CanvasState {
        active_layer: Some(LayerId::new(1)),
        active_frame: Some(FrameIndex::new(0)),
        scroll_x: 0.0,
        scroll_y: 0.0,
        zoom: 12.0,
        onion_skin: false,
        show_tile_grid: false,
    };

    PixhausArchive {
        project,
        buffers: all_buffers,
    }
}

// ── S45(d): ui-sprites.pixhaus ───────────────────────────────────────────────
//
// RGBA UI element sheet, 1 frame. Slice data for health bar, mana bar,
// button states, and dialogue box.

fn build_ui_sprites() -> PixhausArchive {
    // Canvas layout (96 × 72):
    //   (0,0)   64×8   health bar
    //   (0,12)  64×8   mana bar
    //   (0,24)  32×14  button-normal
    //   (36,24) 32×14  button-hover
    //   (0,42)  96×30  dialogue box

    const W: u32 = 96;
    const H: u32 = 72;

    let mut canvas_pixels = vec![0u8; (W * H * 4) as usize];

    // Helper: blit a bordered rectangle onto the canvas at (ox, oy).
    let mut blit = |ox: u32, oy: u32, w: u32, h: u32, border: [u8; 4], fill: [u8; 4]| {
        let patch = rgba_bordered(w, h, border, fill);
        for y in 0..h {
            for x in 0..w {
                let src = ((y * w + x) * 4) as usize;
                let dst = (((oy + y) * W + ox + x) * 4) as usize;
                canvas_pixels[dst..dst + 4].copy_from_slice(&patch[src..src + 4]);
            }
        }
    };

    // Health bar (red → bright red fill)
    blit(0, 0, 64, 8, [100, 10, 10, 255], [220, 50, 50, 255]);
    // Mana bar (dark blue → bright blue fill)
    blit(0, 12, 64, 8, [10, 20, 100, 255], [50, 100, 220, 255]);
    // Button normal (gray border, mid-gray fill)
    blit(0, 24, 32, 14, [60, 60, 60, 255], [140, 140, 140, 255]);
    // Button hover (lighter)
    blit(36, 24, 32, 14, [80, 80, 80, 255], [180, 180, 180, 255]);
    // Dialogue box (dark brown border, parchment fill)
    blit(0, 42, 96, 30, [60, 40, 20, 255], [220, 200, 160, 255]);

    let base_buf = rgba_buf(3000, W, H, canvas_pixels);

    // The canvas has one raster layer.
    let layer = raster(1, "ui-elements", BlendMode::Normal, 255, None);
    let cel = rcel(1, 0, 3000, W, H);

    // Slices define the nine-slice regions and pivots.
    let slices = vec![
        Slice {
            id: SliceId::new(1),
            name: "health-bar".into(),
            keys: vec![SliceKey {
                frame: FrameIndex::new(0),
                bounds: Rect::from_xywh(0, 0, 64, 8),
                nine_slice: Some(NineSlice {
                    center: Rect {
                        origin: IVec2::new(2, 2),
                        size: Size::new(60, 4),
                    },
                }),
                pivot: None,
            }],
            user_data: UserData::default(),
        },
        // NineSlice.center origins are RELATIVE to the slice's bounds.origin
        // (see NineSlice doc in core/src/project/slice.rs). Importers add
        // bounds.origin themselves at render time. Earlier samples baked
        // absolute canvas coords, which made every importer offset twice.
        Slice {
            id: SliceId::new(2),
            name: "mana-bar".into(),
            keys: vec![SliceKey {
                frame: FrameIndex::new(0),
                bounds: Rect::from_xywh(0, 12, 64, 8),
                nine_slice: Some(NineSlice {
                    center: Rect {
                        origin: IVec2::new(2, 2),
                        size: Size::new(60, 4),
                    },
                }),
                pivot: None,
            }],
            user_data: UserData::default(),
        },
        Slice {
            id: SliceId::new(3),
            name: "button-normal".into(),
            keys: vec![SliceKey {
                frame: FrameIndex::new(0),
                bounds: Rect::from_xywh(0, 24, 32, 14),
                nine_slice: Some(NineSlice {
                    center: Rect {
                        origin: IVec2::new(2, 2),
                        size: Size::new(28, 10),
                    },
                }),
                pivot: Some(Pivot {
                    offset: IVec2::new(16, 7),
                }),
            }],
            user_data: UserData::default(),
        },
        Slice {
            id: SliceId::new(4),
            name: "button-hover".into(),
            keys: vec![SliceKey {
                frame: FrameIndex::new(0),
                bounds: Rect::from_xywh(36, 24, 32, 14),
                nine_slice: Some(NineSlice {
                    center: Rect {
                        origin: IVec2::new(2, 2),
                        size: Size::new(28, 10),
                    },
                }),
                pivot: Some(Pivot {
                    offset: IVec2::new(16, 7),
                }),
            }],
            user_data: UserData::default(),
        },
        Slice {
            id: SliceId::new(5),
            name: "dialogue-box".into(),
            keys: vec![SliceKey {
                frame: FrameIndex::new(0),
                bounds: Rect::from_xywh(0, 42, 96, 30),
                nine_slice: Some(NineSlice {
                    center: Rect {
                        origin: IVec2::new(4, 4),
                        size: Size::new(88, 22),
                    },
                }),
                pivot: None,
            }],
            user_data: UserData::default(),
        },
    ];

    let mut sprite = Sprite::empty(SpriteId::new(1), "ui-sprites-96x72", Size::new(W, H));
    sprite.color_mode = ColorMode::Rgba;
    sprite.layers = vec![layer];
    sprite.frames = vec![ms(100)];
    sprite.cels = vec![cel];
    sprite.slices = slices;

    let mut project = Project::new("ui-sprites");
    project.metadata.description = Some(
        "UI sprite sheet: health bar, mana bar, buttons, dialogue box. RGBA. S45 sample.".into(),
    );
    project.metadata.author = Some("pixhaus".into());
    project.feature_flags = FeatureFlags::SLICES;
    install_sprite_as_entity(&mut project, sprite, "UI Sprites");
    project.canvas = CanvasState {
        active_layer: Some(LayerId::new(1)),
        active_frame: Some(FrameIndex::new(0)),
        scroll_x: 0.0,
        scroll_y: 0.0,
        zoom: 4.0,
        onion_skin: false,
        show_tile_grid: false,
    };

    PixhausArchive {
        project,
        buffers: vec![base_buf],
    }
}

// ── S45(e): level-forest.pixhaus ────────────────────────────────────────────
//
// 32×16 tilemap level using the forest tileset. Canvas 512×256 (32×16 tiles
// at 16px). Tileset is the same 17-tile forest set, included inline.

fn build_forest_level() -> PixhausArchive {
    const COLS: u32 = 32;
    const ROWS: u32 = 16;
    const TW: u32 = FOREST_TILE_SIZE;
    const TH: u32 = FOREST_TILE_SIZE;

    // Level layout (indices into the forest tileset, 1-based for grass):
    //   rows 0-6:  grass (alternating 1/2 for visual variety)
    //   row  7:    grass with scattered dirt (5/6)
    //   rows 8-9:  dirt path
    //   rows 10-11: stone patches
    //   rows 12-13: grass again
    //   rows 14-15: water
    let mut cells = vec![
        TileCell {
            index: TileIndex::new(0),
            flags: TileFlags::empty()
        };
        (COLS * ROWS) as usize
    ];

    for row in 0..ROWS {
        for col in 0..COLS {
            let idx = (row * COLS + col) as usize;
            let tile_index: u32 = match row {
                0..=5 => {
                    if (col + row) % 4 == 0 { 2 } else { 1 } // grass-a / grass-b
                }
                6 => {
                    if col % 8 == 3 || col % 8 == 4 { 5 } else { 1 } // some dirt
                }
                7 => {
                    if col % 8 < 3 || col % 8 > 5 { 5 } else { 8 } // dirt path
                }
                8 => {
                    if col % 6 < 2 { 9 } else { 5 } // stone + dirt
                }
                9 => {
                    if col % 6 < 3 { 9 } else { 10 } // stone variants
                }
                10 | 11 => {
                    if col % 4 < 2 { 1 } else { 9 } // grass + stone patches
                }
                12 | 13 => 1, // grass
                14 | 15 => {
                    if (col + row) % 3 == 0 { 14 } else { 13 } // water-a / water-b
                }
                _ => 1,
            };
            cells[idx] = TileCell {
                index: TileIndex::new(tile_index),
                flags: TileFlags::empty(),
            };
        }
    }

    let tilemap_data = TilemapData {
        width: COLS,
        height: ROWS,
        cells,
    };

    let tilemap_cel = Cel {
        layer_id: LayerId::new(1),
        frame_index: FrameIndex::new(0),
        position: IVec2::zero(),
        opacity: 255,
        data: CelData::Tilemap { data: tilemap_data },
        user_data: UserData::default(),
    };

    // Inline the forest tileset (same 17-tile definition).
    let atlas_pixels = forest_tileset_pixels();
    let atlas_buf = rgba_buf(4000, TW, TH * FOREST_TILE_COUNT, atlas_pixels);

    let tileset = Tileset {
        id: TilesetId::new(1),
        name: "forest".into(),
        tile_size: Size::new(TW, TH),
        tile_count: FOREST_TILE_COUNT,
        base_index: 1,
        source: TilesetSource::Inline {
            buffer: PixelBufferId::new(4000),
        },
        // Inline the same per-tile metadata the standalone tileset
        // sample carries — collision shapes (stone tiles 9-12) plus the
        // water-tile animation. Importers that care about collision or
        // animated tiles need this to match the tileset-forest sample.
        properties: forest_tile_properties(FOREST_TILE_COUNT, forest_water_anim()),
        autotile: None,
        user_data: UserData::default(),
    };

    let layer = tilemap(1, "terrain", 1);

    let mut sprite = Sprite::empty(
        SpriteId::new(1),
        "forest-level-32x16",
        Size::new(COLS * TW, ROWS * TH),
    );
    sprite.color_mode = ColorMode::Rgba;
    sprite.layers = vec![layer];
    sprite.frames = vec![ms(100)];
    sprite.cels = vec![tilemap_cel];
    sprite.tilesets = vec![tileset];

    let mut project = Project::new("level-forest");
    project.metadata.description =
        Some("32×16 forest level. Tilemap using inline forest tileset. S45 sample.".into());
    project.metadata.author = Some("pixhaus".into());
    project.feature_flags = FeatureFlags::TILEMAPS;
    install_sprite_as_entity(&mut project, sprite, "Forest Level");
    project.canvas = CanvasState {
        active_layer: Some(LayerId::new(1)),
        active_frame: Some(FrameIndex::new(0)),
        scroll_x: 0.0,
        scroll_y: 0.0,
        zoom: 2.0,
        onion_skin: false,
        show_tile_grid: true,
    };

    PixhausArchive {
        project,
        buffers: vec![atlas_buf],
    }
}

// ── generator test ───────────────────────────────────────────────────────────

#[test]
fn generate_sample_projects() {
    if std::env::var(REGEN_ENV).is_err() {
        return;
    }
    let samples: &[(&str, fn() -> PixhausArchive)] = &[
        ("character-knight.pixhaus", build_knight),
        ("tileset-forest.pixhaus", build_forest_tileset),
        ("enemy-slime.pixhaus", build_slime),
        ("ui-sprites.pixhaus", build_ui_sprites),
        ("level-forest.pixhaus", build_forest_level),
    ];
    for (name, build) in samples {
        let archive = build();
        write_sample(name, &archive);
    }
    println!("all samples written to {}", sample_dir().display());
}

// ── verification tests (always run) ─────────────────────────────────────────
//
// These don't touch the filesystem but prove every builder round-trips
// cleanly through the codec. They run in normal CI.

#[test]
fn knight_round_trips() {
    let archive = build_knight();
    let bytes = pixhaus_io::pixhaus::encode(&archive).expect("encode");
    let back = pixhaus_io::pixhaus::decode(&bytes).expect("decode");
    assert_eq!(back.project, archive.project);
    assert_eq!(back.buffers.len(), archive.buffers.len());
}

#[test]
fn forest_tileset_round_trips() {
    let archive = build_forest_tileset();
    let bytes = pixhaus_io::pixhaus::encode(&archive).expect("encode");
    let back = pixhaus_io::pixhaus::decode(&bytes).expect("decode");
    assert_eq!(back.project, archive.project);
    assert_eq!(back.buffers.len(), archive.buffers.len());
}

#[test]
fn slime_round_trips() {
    let archive = build_slime();
    let bytes = pixhaus_io::pixhaus::encode(&archive).expect("encode");
    let back = pixhaus_io::pixhaus::decode(&bytes).expect("decode");
    assert_eq!(back.project, archive.project);
    assert_eq!(back.buffers.len(), archive.buffers.len());
}

#[test]
fn ui_sprites_round_trips() {
    let archive = build_ui_sprites();
    let bytes = pixhaus_io::pixhaus::encode(&archive).expect("encode");
    let back = pixhaus_io::pixhaus::decode(&bytes).expect("decode");
    assert_eq!(back.project, archive.project);
    assert_eq!(back.buffers.len(), archive.buffers.len());
}

#[test]
fn forest_level_round_trips() {
    let archive = build_forest_level();
    let bytes = pixhaus_io::pixhaus::encode(&archive).expect("encode");
    let back = pixhaus_io::pixhaus::decode(&bytes).expect("decode");
    assert_eq!(back.project, archive.project);
    assert_eq!(back.buffers.len(), archive.buffers.len());
}

#[test]
fn knight_has_correct_frame_count() {
    let archive = build_knight();
    let sprite = archive
        .project
        .sprite(SpriteId::new(1))
        .expect("primary sprite present");
    // idle=4, walk=8×8=64, run=8×8=64, attack=6×4=24, hurt=3, death=8 → 167
    assert_eq!(sprite.frames.len(), 167, "expected 167 frames");
}

#[test]
fn knight_has_all_directional_tags() {
    let archive = build_knight();
    let sprite = archive
        .project
        .sprite(SpriteId::new(1))
        .expect("knight sprite");
    let has_tag = |name: &str| sprite.frame_tags.iter().any(|t| t.name == name);
    for dir in ["s", "sw", "w", "nw", "n", "ne", "e", "se"] {
        assert!(has_tag(&format!("walk-{dir}")), "missing walk-{dir}");
        assert!(has_tag(&format!("run-{dir}")), "missing run-{dir}");
    }
    for dir in ["s", "w", "n", "e"] {
        assert!(has_tag(&format!("attack-{dir}")), "missing attack-{dir}");
    }
    assert!(has_tag("hurt"), "missing hurt");
    assert!(has_tag("death"), "missing death");
}

#[test]
fn forest_tileset_has_water_animation() {
    let archive = build_forest_tileset();
    let sprite = archive
        .project
        .sprite(SpriteId::new(1))
        .expect("forest tileset sprite");
    let tileset = &sprite.tilesets[0];
    assert_eq!(tileset.tile_count, FOREST_TILE_COUNT);
    let water_props = &tileset.properties[13];
    let anim = water_props
        .animation
        .as_ref()
        .expect("tile 13 should have animation");
    assert_eq!(anim.frames.len(), 3, "water animation must have 3 frames");
}

#[test]
fn forest_level_tilemap_dimensions() {
    let archive = build_forest_level();
    let cel = &archive
        .project
        .sprite(SpriteId::new(1))
        .expect("forest level sprite")
        .cels[0];
    let CelData::Tilemap { data } = &cel.data else {
        panic!("expected tilemap cel");
    };
    assert_eq!(data.width, 32);
    assert_eq!(data.height, 16);
    assert_eq!(data.cells.len(), 512);
}

#[test]
fn ui_sprites_has_all_slices() {
    let archive = build_ui_sprites();
    let sprite = archive
        .project
        .sprite(SpriteId::new(1))
        .expect("ui sprites sprite");
    let names: Vec<&str> = sprite.slices.iter().map(|s| s.name.as_str()).collect();
    for expected in [
        "health-bar",
        "mana-bar",
        "button-normal",
        "button-hover",
        "dialogue-box",
    ] {
        assert!(names.contains(&expected), "missing slice: {expected}");
    }
    // All button slices must have nine-slice data.
    for slice in &sprite.slices {
        if slice.name.starts_with("button") {
            assert!(
                slice.keys[0].nine_slice.is_some(),
                "button slice '{}' must have nine-slice",
                slice.name
            );
        }
    }
}

/// Verifies the level's inlined forest tileset carries the same per-tile
/// metadata as the standalone tileset sample: collision shapes on stone
/// tiles 9-12 and the 3-frame water animation on tile 13.
///
/// This is the regression guard for S45-followup. The builder already
/// calls `forest_tile_properties()` at construction time; this test
/// makes that contract explicit so a future edit that drops the call
/// cannot merge without a visible failure.
#[test]
fn forest_level_tileset_preserves_metadata() {
    let archive = build_forest_level();
    let tileset = &archive
        .project
        .sprite(SpriteId::new(1))
        .expect("forest level sprite")
        .tilesets[0];
    assert_eq!(
        tileset.tile_count, FOREST_TILE_COUNT,
        "level tileset tile_count must match forest definition"
    );

    // Stone tiles 9-12 must have full collision.
    for i in 9u32..=12 {
        let props = tileset.tile_properties(TileIndex::new(i));
        assert_eq!(
            props.collision,
            CollisionShape::Full,
            "tile {i} must have CollisionShape::Full in the inlined level tileset"
        );
    }

    // Tile 13 must carry the 3-frame water animation.
    let water = tileset
        .tile_properties(TileIndex::new(13))
        .animation
        .as_ref()
        .expect("tile 13 must have an animation in the inlined level tileset");
    assert_eq!(
        water.frames.len(),
        3,
        "water animation must have 3 frames in the level tileset"
    );
    assert_eq!(
        water.loop_mode,
        AnimLoopMode::Loop,
        "water animation loop mode must be Loop"
    );
    for (i, frame) in water.frames.iter().enumerate() {
        assert_eq!(
            frame.duration_ms, 150,
            "water frame {i} must be 150 ms in the level tileset"
        );
    }
}

/// Asserts the level's inlined tileset properties are identical to the
/// standalone tileset's properties. If someone updates one without the
/// other the gap shows up here before it reaches CI consumers (Unity
/// importer, TMX exporter) that assume the two are equivalent.
#[test]
fn forest_level_tileset_properties_match_standalone() {
    let level = build_forest_level();
    let standalone = build_forest_tileset();

    let level_props = &level
        .project
        .sprite(SpriteId::new(1))
        .expect("level sprite")
        .tilesets[0]
        .properties;
    let standalone_props = &standalone
        .project
        .sprite(SpriteId::new(1))
        .expect("standalone sprite")
        .tilesets[0]
        .properties;

    assert_eq!(
        level_props, standalone_props,
        "level's inlined tileset properties must match the standalone tileset's properties"
    );
}

/// Walks every sample under `examples/samples/` and re-decodes it, then
/// confirms it round-trips back to the same project. Without this, a
/// stale fixture (left over after a builder change but not regenerated
/// via `PIXHAUS_REGEN_SAMPLES`) would merge silently — every other test
/// in this file builds the archive in memory and never touches the
/// committed bytes. See `examples/samples/README.md`.
#[test]
fn committed_samples_round_trip_against_current_builders() {
    let samples_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("io crate has parent")
        .join("examples")
        .join("samples");
    assert!(
        samples_dir.exists(),
        "examples/samples/ not found at {} — has the directory moved?",
        samples_dir.display()
    );

    // Each (filename, builder) pair: read the committed file, decode,
    // and compare against what the current builder would produce. A
    // mismatch means the committed sample is stale.
    let cases: &[(&str, fn() -> PixhausArchive)] = &[
        ("character-knight.pixhaus", build_knight),
        ("tileset-forest.pixhaus", build_forest_tileset),
        ("enemy-slime.pixhaus", build_slime),
        ("ui-sprites.pixhaus", build_ui_sprites),
        ("level-forest.pixhaus", build_forest_level),
    ];

    for (filename, builder) in cases {
        let path = samples_dir.join(filename);
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let on_disk = pixhaus_io::pixhaus::decode(&bytes)
            .unwrap_or_else(|e| panic!("decode {}: {e:?}", path.display()));
        let in_memory = builder();
        assert_eq!(
            on_disk.project, in_memory.project,
            "{filename} is stale — re-run PIXHAUS_REGEN_SAMPLES=1 cargo nextest run -p pixhaus-io --test generate_sample_projects"
        );
    }
}
