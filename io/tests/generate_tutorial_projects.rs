//! Generates tutorial starter and finished `.pixhaus` files for S44.
//!
//! Gated on `PIXHAUS_REGEN_TUTORIALS` so it does not run in normal CI —
//! the committed binaries are the source of truth and this generator only
//! runs when the wire format changes or new tutorial files are needed.
//!
//! Usage:
//!
//! ```text
//! PIXHAUS_REGEN_TUTORIALS=1 cargo nextest run -p pixhaus-io \
//!     --test generate_tutorial_projects
//! ```
//!
//! Output: `examples/tutorials/` relative to the workspace root.

#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::drop_non_drop,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::panic,
    clippy::print_stdout,
    clippy::too_many_lines,
    clippy::unwrap_used
)]

use std::path::PathBuf;

use pixhaus_core::project::{
    ActiveTarget, AiMetadata, AnimationId, BlendMode, BrushShape, BrushState, CanvasState, Cel,
    ColorMode, Entity, EntityContent, EntityDefaults, EntityId, EntityKind, FeatureFlags, Frame,
    FrameIndex, FrameRange, FrameTag, Layer, LayerId, LayerKind, LoopDirection, NamedSprite,
    Palette, PaletteEntry, PaletteId, PixelBufferId, Project, Rgba, Size, Sprite, SpriteId,
    StateId, UserData,
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

const REGEN_ENV: &str = "PIXHAUS_REGEN_TUTORIALS";

fn tutorial_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("examples")
        .join("tutorials")
}

fn write_tutorial(name: &str, archive: &PixhausArchive) {
    let dir = tutorial_dir();
    std::fs::create_dir_all(&dir).expect("create tutorials dir");
    let path = dir.join(name);
    encode_to_file(archive, &path).expect("encode tutorial");
    let size = std::fs::metadata(&path).map_or(0, |m| m.len());
    println!("wrote {} ({size} bytes)", path.display());
}

// ── pixel data ───────────────────────────────────────────────────────────────

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

// ── palette helpers ──────────────────────────────────────────────────────────

fn knight_palette() -> Palette {
    Palette {
        id: PaletteId::new(1),
        name: "knight".into(),
        colors: vec![
            pe("transparent", Rgba::transparent()),
            pe("outline", Rgba::opaque(10, 10, 15)),
            pe("shadow", Rgba::opaque(25, 25, 45)),
            pe("armor-dark", Rgba::opaque(45, 55, 105)),
            pe("armor", Rgba::opaque(65, 85, 165)),
            pe("armor-light", Rgba::opaque(100, 130, 210)),
            pe("skin-dark", Rgba::opaque(110, 65, 50)),
            pe("skin", Rgba::opaque(185, 125, 90)),
            pe("skin-light", Rgba::opaque(230, 185, 155)),
            pe("gold-dark", Rgba::opaque(105, 75, 20)),
            pe("gold", Rgba::opaque(195, 155, 50)),
            pe("gold-light", Rgba::opaque(255, 215, 0)),
            pe("hit", Rgba::opaque(200, 40, 50)),
            pe("white", Rgba::opaque(255, 255, 255)),
            pe("steel-dark", Rgba::opaque(55, 55, 65)),
            pe("steel", Rgba::opaque(140, 150, 165)),
        ],
        user_data: UserData::default(),
    }
}

fn pe(name: &str, color: Rgba) -> PaletteEntry {
    PaletteEntry {
        color,
        name: if name.is_empty() {
            None
        } else {
            Some(name.into())
        },
    }
}

/// Palette with 16 entries in arbitrary (unsorted by luminance) order.
/// Index 0 is transparent; indices 1–15 are colors mixed so luminance
/// is not monotone. The Lua tutorial's sort script should reorder 1–15
/// to produce `luminance_sorted_palette()`.
fn unsorted_palette() -> Palette {
    Palette {
        id: PaletteId::new(1),
        name: "tutorial".into(),
        colors: vec![
            pe("transparent", Rgba::transparent()),
            // Deliberately mixed: some bright colors early, dark ones later.
            pe("bright-green", Rgba::opaque(100, 200, 80)),
            pe("dark-blue", Rgba::opaque(20, 30, 90)),
            pe("light-yellow", Rgba::opaque(240, 230, 100)),
            pe("mid-red", Rgba::opaque(160, 40, 40)),
            pe("white", Rgba::opaque(255, 255, 255)),
            pe("dark-green", Rgba::opaque(15, 60, 20)),
            pe("light-blue", Rgba::opaque(120, 180, 230)),
            pe("black", Rgba::opaque(5, 5, 10)),
            pe("orange", Rgba::opaque(210, 110, 20)),
            pe("light-gray", Rgba::opaque(190, 190, 190)),
            pe("dark-gray", Rgba::opaque(50, 50, 55)),
            pe("purple", Rgba::opaque(90, 30, 130)),
            pe("teal", Rgba::opaque(40, 160, 140)),
            pe("pink", Rgba::opaque(220, 140, 170)),
            pe("mid-gray", Rgba::opaque(115, 115, 115)),
        ],
        user_data: UserData::default(),
    }
}

/// The same 16 colors with index 0 (transparent) fixed and indices 1–15
/// sorted by BT.601 luminance, ascending (dark to light).
fn sorted_palette() -> Palette {
    let mut entries: Vec<PaletteEntry> = unsorted_palette()
        .colors
        .into_iter()
        .enumerate()
        .filter_map(|(i, e)| if i == 0 { None } else { Some(e) })
        .collect();

    entries.sort_by(|a, b| {
        let lum = |e: &PaletteEntry| {
            let c = &e.color;
            0.299 * f64::from(c.r) + 0.587 * f64::from(c.g) + 0.114 * f64::from(c.b)
        };
        lum(a)
            .partial_cmp(&lum(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut colors = vec![pe("transparent", Rgba::transparent())];
    colors.extend(entries);

    Palette {
        id: PaletteId::new(1),
        name: "tutorial-sorted".into(),
        colors,
        user_data: UserData::default(),
    }
}

// ── shared layer / cel / frame helpers ──────────────────────────────────────

fn base_layer() -> Layer {
    Layer {
        id: LayerId::new(1),
        name: "base".into(),
        kind: LayerKind::Raster,
        blend_mode: BlendMode::Normal,
        opacity: 255,
        visible: true,
        locked: false,
        parent: None,
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

fn idx_buf(id: u32, w: u32, h: u32, pixels: Vec<u8>) -> PixelBufferEntry {
    PixelBufferEntry {
        id,
        width: w,
        height: h,
        stride: w,
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

fn default_canvas(active_frame: u32) -> CanvasState {
    CanvasState {
        active_layer: Some(LayerId::new(1)),
        active_frame: Some(FrameIndex::new(active_frame)),
        scroll_x: 0.0,
        scroll_y: 0.0,
        zoom: 8.0,
        onion_skin: false,
        show_tile_grid: false,
    }
}

fn default_brush(fg: u32) -> BrushState {
    BrushState {
        shape: BrushShape::Square,
        size: 1,
        foreground_index: fg,
        background_index: 0,
        active_palette: Some(PaletteId::new(1)),
    }
}

// ── walk-cycle-start.pixhaus ─────────────────────────────────────────────────
//
// 2 frames, tagged "walk-key". Frame 0 = contact pose, frame 1 = passing pose.
// Starter for the AI inbetween tutorial.

fn build_walk_cycle_start() -> PixhausArchive {
    const W: u32 = 32;
    const H: u32 = 32;

    let frames = vec![ms(150), ms(150)];
    let buffers = vec![
        idx_buf(1, W, H, indexed_frame(W, H, 4, 1, 0, 2)), // contact pose: armor color
        idx_buf(2, W, H, indexed_frame(W, H, 5, 1, 1, 2)), // passing pose: armor-light color
    ];
    let cels = vec![rcel(1, 0, 1, W, H), rcel(1, 1, 2, W, H)];
    let tags = vec![tag("walk-key", 0, 1, LoopDirection::Forward)];

    let mut sprite = Sprite::empty(SpriteId::new(1), "knight-32x32", Size::new(W, H));
    sprite.color_mode = ColorMode::Indexed;
    sprite.transparent_color_index = Some(0);
    sprite.layers = vec![base_layer()];
    sprite.frames = frames;
    sprite.cels = cels;
    sprite.palettes = vec![knight_palette()];
    sprite.frame_tags = tags;

    let mut project = Project::new("walk-cycle-start");
    project.metadata.description =
        Some("Walk cycle starter: 2 key frames for the AI inbetween tutorial (S44).".into());
    project.feature_flags = FeatureFlags::ANIMATIONS;
    install_sprite_as_entity(&mut project, sprite, "Walk Cycle Start");
    project.canvas = CanvasState {
        onion_skin: true,
        ..default_canvas(0)
    };
    project.brush = default_brush(4);

    PixhausArchive { project, buffers }
}

// ── walk-cycle-finished.pixhaus ──────────────────────────────────────────────
//
// 4 frames, tagged "walk". Frames 0 and 2 are the original key frames;
// frames 1 and 3 are the simulated inbetweens (different fill color).

fn build_walk_cycle_finished() -> PixhausArchive {
    const W: u32 = 32;
    const H: u32 = 32;

    let frames = vec![ms(100), ms(100), ms(100), ms(100)];
    let buffers = vec![
        idx_buf(1, W, H, indexed_frame(W, H, 4, 1, 0, 4)), // key: contact
        idx_buf(2, W, H, indexed_frame(W, H, 3, 1, 1, 4)), // inbetween (darker, midpoint)
        idx_buf(3, W, H, indexed_frame(W, H, 5, 1, 2, 4)), // key: passing
        idx_buf(4, W, H, indexed_frame(W, H, 3, 1, 3, 4)), // inbetween (return)
    ];
    let cels = vec![
        rcel(1, 0, 1, W, H),
        rcel(1, 1, 2, W, H),
        rcel(1, 2, 3, W, H),
        rcel(1, 3, 4, W, H),
    ];
    let tags = vec![tag("walk", 0, 3, LoopDirection::Forward)];

    let mut sprite = Sprite::empty(SpriteId::new(1), "knight-32x32", Size::new(W, H));
    sprite.color_mode = ColorMode::Indexed;
    sprite.transparent_color_index = Some(0);
    sprite.layers = vec![base_layer()];
    sprite.frames = frames;
    sprite.cels = cels;
    sprite.palettes = vec![knight_palette()];
    sprite.frame_tags = tags;
    sprite.animations = vec![pixhaus_core::project::Animation {
        id: AnimationId::new(1),
        name: "walk".into(),
        range: FrameRange::new(FrameIndex::new(0), FrameIndex::new(3)),
        loop_direction: LoopDirection::Forward,
        speed_multiplier: 1.0,
        user_data: UserData::default(),
    }];

    let mut project = Project::new("walk-cycle-finished");
    project.metadata.description = Some(
        "Walk cycle finished: 4 frames (2 key + 2 inbetween) for the AI inbetween tutorial (S44)."
            .into(),
    );
    project.feature_flags = FeatureFlags::ANIMATIONS;
    install_sprite_as_entity(&mut project, sprite, "Walk Cycle Finished");
    project.canvas = default_canvas(0);
    project.brush = default_brush(4);

    PixhausArchive { project, buffers }
}

// ── export-unity-start.pixhaus ───────────────────────────────────────────────
//
// 18 frames total: idle (4 frames, PingPong), walk (8 frames, Forward),
// attack (6 frames, Forward). Starter for the Unity export tutorial.

fn build_export_unity_start() -> PixhausArchive {
    const W: u32 = 32;
    const H: u32 = 32;

    let mut frames: Vec<Frame> = Vec::new();
    let mut buffers: Vec<PixelBufferEntry> = Vec::new();
    let mut cels: Vec<Cel> = Vec::new();
    let mut tags: Vec<FrameTag> = Vec::new();
    let mut animations: Vec<pixhaus_core::project::Animation> = Vec::new();
    let mut fc: u32 = 0;
    let mut bc: u32 = 1;
    let mut ac: u32 = 1;

    let mut push = |name: &str, count: u32, dur_ms: u32, fill: u8, dir: LoopDirection| {
        let start = fc;
        for i in 0..count {
            frames.push(ms(dur_ms));
            buffers.push(idx_buf(bc, W, H, indexed_frame(W, H, fill, 1, i, count)));
            cels.push(rcel(1, fc, bc, W, H));
            bc += 1;
            fc += 1;
        }
        let end = fc - 1;
        tags.push(tag(name, start, end, dir));
        animations.push(pixhaus_core::project::Animation {
            id: AnimationId::new(ac),
            name: name.into(),
            range: FrameRange::new(FrameIndex::new(start), FrameIndex::new(end)),
            loop_direction: dir,
            speed_multiplier: 1.0,
            user_data: UserData::default(),
        });
        ac += 1;
    };

    push("idle", 4, 120, 4, LoopDirection::PingPong);
    push("walk", 8, 100, 5, LoopDirection::Forward);
    push("attack", 6, 80, 10, LoopDirection::Forward);
    drop(push);

    let mut sprite = Sprite::empty(SpriteId::new(1), "knight-32x32", Size::new(W, H));
    sprite.color_mode = ColorMode::Indexed;
    sprite.transparent_color_index = Some(0);
    sprite.layers = vec![base_layer()];
    sprite.frames = frames;
    sprite.cels = cels;
    sprite.palettes = vec![knight_palette()];
    sprite.frame_tags = tags;
    sprite.animations = animations;

    let mut project = Project::new("export-unity-start");
    project.metadata.description = Some(
        "Unity export starter: idle/walk/attack animations for the export tutorial (S44).".into(),
    );
    project.feature_flags = FeatureFlags::ANIMATIONS;
    install_sprite_as_entity(&mut project, sprite, "Export Unity Start");
    project.canvas = default_canvas(0);
    project.brush = default_brush(4);

    PixhausArchive { project, buffers }
}

// ── lua-palette-start.pixhaus ────────────────────────────────────────────────
//
// Single frame with a 16-color palette in non-luminance order.
// Starter for the Lua scripting tutorial.

fn build_lua_palette_start() -> PixhausArchive {
    const W: u32 = 32;
    const H: u32 = 32;

    let frames = vec![ms(100)];
    let pixels = indexed_frame(W, H, 4, 1, 0, 0);
    let buffers = vec![idx_buf(1, W, H, pixels)];
    let cels = vec![rcel(1, 0, 1, W, H)];

    let mut sprite = Sprite::empty(SpriteId::new(1), "lua-palette", Size::new(W, H));
    sprite.color_mode = ColorMode::Indexed;
    sprite.transparent_color_index = Some(0);
    sprite.layers = vec![base_layer()];
    sprite.frames = frames;
    sprite.cels = cels;
    sprite.palettes = vec![unsorted_palette()];

    let mut project = Project::new("lua-palette-start");
    project.metadata.description =
        Some("Lua tutorial starter: 16-color palette in arbitrary order (S44).".into());
    install_sprite_as_entity(&mut project, sprite, "Lua Palette Start");
    project.canvas = default_canvas(0);
    project.brush = default_brush(4);

    PixhausArchive { project, buffers }
}

// ── lua-palette-finished.pixhaus ─────────────────────────────────────────────
//
// Same sprite but with the palette sorted by luminance (index 0 preserved).
// Shows the result of running the sort-palette-by-luminance Lua script.

fn build_lua_palette_finished() -> PixhausArchive {
    const W: u32 = 32;
    const H: u32 = 32;

    let frames = vec![ms(100)];
    let pixels = indexed_frame(W, H, 4, 1, 0, 0);
    let buffers = vec![idx_buf(1, W, H, pixels)];
    let cels = vec![rcel(1, 0, 1, W, H)];

    let mut sprite = Sprite::empty(SpriteId::new(1), "lua-palette", Size::new(W, H));
    sprite.color_mode = ColorMode::Indexed;
    sprite.transparent_color_index = Some(0);
    sprite.layers = vec![base_layer()];
    sprite.frames = frames;
    sprite.cels = cels;
    sprite.palettes = vec![sorted_palette()];

    let mut project = Project::new("lua-palette-finished");
    project.metadata.description = Some(
        "Lua tutorial finished: palette sorted by luminance via the tutorial script (S44).".into(),
    );
    install_sprite_as_entity(&mut project, sprite, "Lua Palette Finished");
    project.canvas = default_canvas(0);
    project.brush = default_brush(4);

    PixhausArchive { project, buffers }
}

// ── test entry point ─────────────────────────────────────────────────────────

#[test]
fn generate_tutorial_projects() {
    if std::env::var(REGEN_ENV).is_err() {
        println!("skipped — set {REGEN_ENV}=1 to regenerate");
        return;
    }

    write_tutorial("walk-cycle-start.pixhaus", &build_walk_cycle_start());
    write_tutorial("walk-cycle-finished.pixhaus", &build_walk_cycle_finished());
    write_tutorial("export-unity-start.pixhaus", &build_export_unity_start());
    write_tutorial("lua-palette-start.pixhaus", &build_lua_palette_start());
    write_tutorial(
        "lua-palette-finished.pixhaus",
        &build_lua_palette_finished(),
    );

    println!("done — 5 tutorial files written to examples/tutorials/");
}
