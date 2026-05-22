//! Integration tests for the PNG sprite sheet exporter (S10).
//!
//! These tests operate on the public API of `pixhaus-io` exactly as an
//! external caller would. Test coverage mirrors the brief:
//!
//! - Small grid (4 × 4 frames)
//! - Large packed sheet (100+ frames)
//! - Animated sprite with frame tags
//! - Reference layer exclusion
//! - Round-trip: JSON + PNG can be decoded back into a frame sequence

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::disallowed_methods,
    clippy::cast_possible_truncation
)]

use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::geometry::{IVec2, Rect, Size};
use pixhaus_core::project::id::{FrameIndex, LayerId, PixelBufferId, SliceId, SpriteId};
use pixhaus_core::project::layer::{Layer, LayerKind};
use pixhaus_core::project::slice::{NineSlice, Pivot, Slice, SliceKey};
use pixhaus_core::project::{
    BlendMode, Frame, FrameRange, FrameTag, LoopDirection, Rgba, Sprite, UserData,
};
use pixhaus_io::png::{ExportOptions, LayoutStrategy, export_sprite_sheet};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn solid_buf(w: u32, h: u32, color: Rgba) -> PixelBuffer {
    PixelBuffer::filled(w, h, color).unwrap()
}

fn transparent_buf(w: u32, h: u32) -> PixelBuffer {
    PixelBuffer::new(w, h).unwrap()
}

fn sprite_with_n_frames(name: &str, canvas: Size, n: usize) -> Sprite {
    let mut s = Sprite::empty(SpriteId::new(1), name, canvas);
    for _ in 0..n {
        s.frames.push(Frame::default());
    }
    s
}

fn decode_sheet_json(json_bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(json_bytes).expect("JSON must parse")
}

fn decode_sheet_png(png_bytes: &[u8]) -> image::RgbaImage {
    image::load_from_memory(png_bytes)
        .expect("PNG must decode")
        .into_rgba8()
}

// ── Small grid (4 × 4) ───────────────────────────────────────────────────────

#[test]
fn small_grid_sheet_dimensions_and_frame_count() {
    let canvas = Size::new(16, 16);
    let sprite = sprite_with_n_frames("hero", canvas, 4);
    // Fully opaque so no trimming occurs; cell size equals the full canvas.
    let frames: Vec<PixelBuffer> = (0..4)
        .map(|_| solid_buf(16, 16, Rgba::opaque(10, 20, 30)))
        .collect();

    let output = export_sprite_sheet(
        &sprite,
        &frames,
        &ExportOptions {
            layout: LayoutStrategy::Grid { cols: 4 },
            sprite_name: "hero".to_owned(),
        },
    )
    .unwrap();

    let json = decode_sheet_json(&output.json_bytes);
    assert_eq!(
        json["meta"]["size"]["w"], 64,
        "single row of 4 × 16px frames"
    );
    assert_eq!(json["meta"]["size"]["h"], 16);
    assert_eq!(json["frames"].as_array().unwrap().len(), 4);
}

#[test]
fn small_grid_pixels_placed_correctly() {
    // 4 frames in a 2 × 2 grid. Each frame is a distinct solid colour so
    // we can verify the blit landed at the right position.
    let canvas = Size::new(8, 8);
    let sprite = sprite_with_n_frames("colours", canvas, 4);
    let colors = [
        Rgba::opaque(255, 0, 0),   // red   → top-left
        Rgba::opaque(0, 255, 0),   // green → top-right
        Rgba::opaque(0, 0, 255),   // blue  → bottom-left
        Rgba::opaque(255, 255, 0), // yellow → bottom-right
    ];
    let frames: Vec<PixelBuffer> = colors.iter().map(|&c| solid_buf(8, 8, c)).collect();

    let output = export_sprite_sheet(
        &sprite,
        &frames,
        &ExportOptions {
            layout: LayoutStrategy::Grid { cols: 2 },
            sprite_name: "colours".to_owned(),
        },
    )
    .unwrap();

    let img = decode_sheet_png(&output.png_bytes);

    // Top-left cell (frame 0) — red
    assert_eq!(img.get_pixel(0, 0).0, [255, 0, 0, 255]);
    // Top-right cell (frame 1) — green
    assert_eq!(img.get_pixel(8, 0).0, [0, 255, 0, 255]);
    // Bottom-left cell (frame 2) — blue
    assert_eq!(img.get_pixel(0, 8).0, [0, 0, 255, 255]);
    // Bottom-right cell (frame 3) — yellow
    assert_eq!(img.get_pixel(8, 8).0, [255, 255, 0, 255]);
}

#[test]
fn small_grid_json_frame_rects_match_layout() {
    let canvas = Size::new(16, 16);
    let sprite = sprite_with_n_frames("sprite", canvas, 4);
    // Fully opaque so no trimming occurs; frame rects equal the full canvas size.
    let frames: Vec<PixelBuffer> = (0..4)
        .map(|_| solid_buf(16, 16, Rgba::opaque(10, 20, 30)))
        .collect();

    let output = export_sprite_sheet(
        &sprite,
        &frames,
        &ExportOptions {
            layout: LayoutStrategy::Grid { cols: 2 },
            sprite_name: "sprite".to_owned(),
        },
    )
    .unwrap();

    let json = decode_sheet_json(&output.json_bytes);
    let frames_json = json["frames"].as_array().unwrap();

    // Frame 0 at (0, 0)
    assert_eq!(frames_json[0]["frame"]["x"], 0);
    assert_eq!(frames_json[0]["frame"]["y"], 0);
    assert_eq!(frames_json[0]["frame"]["w"], 16);
    assert_eq!(frames_json[0]["frame"]["h"], 16);

    // Frame 1 at (16, 0)
    assert_eq!(frames_json[1]["frame"]["x"], 16);
    assert_eq!(frames_json[1]["frame"]["y"], 0);

    // Frame 2 at (0, 16)
    assert_eq!(frames_json[2]["frame"]["x"], 0);
    assert_eq!(frames_json[2]["frame"]["y"], 16);

    // Frame 3 at (16, 16)
    assert_eq!(frames_json[3]["frame"]["x"], 16);
    assert_eq!(frames_json[3]["frame"]["y"], 16);
}

// ── Large packed sheet (100+ frames) ─────────────────────────────────────────

#[test]
fn large_packed_sheet_contains_all_frames() {
    let canvas = Size::new(8, 8);
    let sprite = sprite_with_n_frames("big", canvas, 120);
    let frames: Vec<PixelBuffer> = (0..120).map(|_| transparent_buf(8, 8)).collect();

    let output = export_sprite_sheet(
        &sprite,
        &frames,
        &ExportOptions {
            layout: LayoutStrategy::Packed,
            sprite_name: "big".to_owned(),
        },
    )
    .unwrap();

    let json = decode_sheet_json(&output.json_bytes);
    assert_eq!(json["frames"].as_array().unwrap().len(), 120);
}

#[test]
fn large_packed_sheet_no_frame_overlaps_in_json() {
    let canvas = Size::new(4, 4);
    let sprite = sprite_with_n_frames("big", canvas, 100);
    // Fully opaque — no trimming; each frame occupies a 4 × 4 cell.
    let frames: Vec<PixelBuffer> = (0..100)
        .map(|_| solid_buf(4, 4, Rgba::opaque(1, 2, 3)))
        .collect();

    let output = export_sprite_sheet(
        &sprite,
        &frames,
        &ExportOptions {
            layout: LayoutStrategy::Packed,
            sprite_name: "big".to_owned(),
        },
    )
    .unwrap();

    let json = decode_sheet_json(&output.json_bytes);
    let frames_json = json["frames"].as_array().unwrap();

    // Verify no two frame rects overlap using the actual frame dimensions.
    for i in 0..frames_json.len() {
        for j in (i + 1)..frames_json.len() {
            let a = &frames_json[i]["frame"];
            let b = &frames_json[j]["frame"];
            let ax = a["x"].as_i64().unwrap();
            let ay = a["y"].as_i64().unwrap();
            let aw = a["w"].as_i64().unwrap();
            let ah = a["h"].as_i64().unwrap();
            let bx = b["x"].as_i64().unwrap();
            let by = b["y"].as_i64().unwrap();
            let bw = b["w"].as_i64().unwrap();
            let bh = b["h"].as_i64().unwrap();
            let overlap = ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah;
            assert!(!overlap, "frames {i} and {j} overlap");
        }
    }
}

#[test]
fn large_packed_sheet_all_frames_within_sheet_bounds() {
    let canvas = Size::new(8, 8);
    let sprite = sprite_with_n_frames("bounds", canvas, 64);
    // Fully opaque — no trimming; each frame's right/bottom edge is x+8, y+8.
    let frames: Vec<PixelBuffer> = (0..64)
        .map(|_| solid_buf(8, 8, Rgba::opaque(1, 2, 3)))
        .collect();

    let output = export_sprite_sheet(
        &sprite,
        &frames,
        &ExportOptions {
            layout: LayoutStrategy::Packed,
            sprite_name: "bounds".to_owned(),
        },
    )
    .unwrap();

    let json = decode_sheet_json(&output.json_bytes);
    let sheet_w = json["meta"]["size"]["w"].as_u64().unwrap();
    let sheet_h = json["meta"]["size"]["h"].as_u64().unwrap();
    let frames_json = json["frames"].as_array().unwrap();

    for (i, f) in frames_json.iter().enumerate() {
        let x = f["frame"]["x"].as_u64().unwrap();
        let y = f["frame"]["y"].as_u64().unwrap();
        let fw = f["frame"]["w"].as_u64().unwrap();
        let fh = f["frame"]["h"].as_u64().unwrap();
        assert!(
            x + fw <= sheet_w,
            "frame {i} right edge {x}+{fw} exceeds sheet width {sheet_w}"
        );
        assert!(
            y + fh <= sheet_h,
            "frame {i} bottom edge {y}+{fh} exceeds sheet height {sheet_h}"
        );
    }
}

// ── Animated sprite with frame tags ──────────────────────────────────────────

#[test]
fn frame_tags_appear_in_json_with_correct_fields() {
    let canvas = Size::new(16, 16);
    let mut sprite = sprite_with_n_frames("anim", canvas, 8);
    sprite.frame_tags.push(FrameTag {
        name: "idle".to_owned(),
        range: FrameRange::new(FrameIndex::new(0), FrameIndex::new(1)),
        loop_direction: LoopDirection::Forward,
        repeat: 0,
        user_data: UserData::default(),
    });
    sprite.frame_tags.push(FrameTag {
        name: "walk".to_owned(),
        range: FrameRange::new(FrameIndex::new(2), FrameIndex::new(5)),
        loop_direction: LoopDirection::PingPong,
        repeat: 3,
        user_data: UserData::default(),
    });
    sprite.frame_tags.push(FrameTag {
        name: "death".to_owned(),
        range: FrameRange::new(FrameIndex::new(6), FrameIndex::new(7)),
        loop_direction: LoopDirection::PingPongReverse,
        repeat: 1,
        user_data: UserData::default(),
    });

    let frames: Vec<PixelBuffer> = (0..8).map(|_| transparent_buf(16, 16)).collect();

    let output = export_sprite_sheet(
        &sprite,
        &frames,
        &ExportOptions {
            layout: LayoutStrategy::ByRow,
            sprite_name: "anim".to_owned(),
        },
    )
    .unwrap();

    let json = decode_sheet_json(&output.json_bytes);
    let tags = json["meta"]["frameTags"].as_array().unwrap();
    assert_eq!(tags.len(), 3);

    assert_eq!(tags[0]["name"], "idle");
    assert_eq!(tags[0]["from"], 0);
    assert_eq!(tags[0]["to"], 1);
    assert_eq!(tags[0]["direction"], "forward");
    assert_eq!(tags[0]["repeat"], 0);
    assert_eq!(tags[0]["color"], "#000000ff");

    assert_eq!(tags[1]["name"], "walk");
    assert_eq!(tags[1]["direction"], "pingpong");
    assert_eq!(tags[1]["repeat"], 3);

    assert_eq!(tags[2]["name"], "death");
    assert_eq!(tags[2]["direction"], "pingpong_reverse");
}

#[test]
fn frame_duration_from_sprite_frames() {
    let canvas = Size::new(8, 8);
    let mut sprite = Sprite::empty(SpriteId::new(1), "timed", canvas);
    sprite.frames.push(Frame {
        duration_ms: 200,
        user_data: UserData::default(),
    });
    sprite.frames.push(Frame {
        duration_ms: 50,
        user_data: UserData::default(),
    });

    let frames = vec![transparent_buf(8, 8), transparent_buf(8, 8)];

    let output = export_sprite_sheet(
        &sprite,
        &frames,
        &ExportOptions {
            layout: LayoutStrategy::ByRow,
            sprite_name: "timed".to_owned(),
        },
    )
    .unwrap();

    let json = decode_sheet_json(&output.json_bytes);
    assert_eq!(json["frames"][0]["duration"], 200);
    assert_eq!(json["frames"][1]["duration"], 50);
}

// ── Reference layer exclusion ─────────────────────────────────────────────────

#[test]
fn reference_layers_excluded_from_meta_layers() {
    let canvas = Size::new(8, 8);
    let mut sprite = sprite_with_n_frames("ref_test", canvas, 1);
    sprite.layers.push(Layer::raster(LayerId::new(1), "body"));
    sprite.layers.push(Layer {
        id: LayerId::new(2),
        name: "ref_guide".to_owned(),
        kind: LayerKind::Reference {
            image: PixelBufferId::new(99),
            origin: IVec2::zero(),
        },
        blend_mode: BlendMode::Normal,
        opacity: 128,
        visible: true,
        locked: false,
        parent: None,
        effects: Vec::new(),
        user_data: UserData::default(),
    });
    sprite.layers.push(Layer::raster(LayerId::new(3), "shadow"));

    let frames = vec![transparent_buf(8, 8)];
    let output = export_sprite_sheet(
        &sprite,
        &frames,
        &ExportOptions {
            layout: LayoutStrategy::Grid { cols: 1 },
            sprite_name: "ref_test".to_owned(),
        },
    )
    .unwrap();

    let json = decode_sheet_json(&output.json_bytes);
    let layers = json["meta"]["layers"].as_array().unwrap();
    assert_eq!(layers.len(), 2, "only raster layers — reference excluded");
    assert_eq!(layers[0]["name"], "body");
    assert_eq!(layers[1]["name"], "shadow");
}

// ── Round-trip: JSON + PNG reconstruct frame sequence ────────────────────────

#[test]
fn round_trip_reconstructs_frame_sequence() {
    // Export a 4-frame, 16 × 16 sprite with distinct colours then read
    // the PNG back, using the JSON rects to extract each frame, and verify
    // the pixel colours match the originals.
    let canvas = Size::new(16, 16);
    let sprite = sprite_with_n_frames("rt", canvas, 4);
    let original_colors = [
        Rgba::opaque(200, 10, 10),
        Rgba::opaque(10, 200, 10),
        Rgba::opaque(10, 10, 200),
        Rgba::opaque(200, 200, 10),
    ];
    let frames: Vec<PixelBuffer> = original_colors
        .iter()
        .map(|&c| solid_buf(16, 16, c))
        .collect();

    let output = export_sprite_sheet(
        &sprite,
        &frames,
        &ExportOptions {
            layout: LayoutStrategy::Grid { cols: 2 },
            sprite_name: "rt".to_owned(),
        },
    )
    .unwrap();

    // Decode the sheet PNG.
    let sheet = decode_sheet_png(&output.png_bytes);
    let json = decode_sheet_json(&output.json_bytes);
    let frames_json = json["frames"].as_array().unwrap();

    for (i, orig) in original_colors.iter().enumerate() {
        // Read the frame rect from JSON.
        let fx = frames_json[i]["frame"]["x"].as_u64().unwrap() as u32;
        let fy = frames_json[i]["frame"]["y"].as_u64().unwrap() as u32;

        // Sample the top-left pixel of the frame region.
        let px = sheet.get_pixel(fx, fy).0;
        assert_eq!(
            px,
            [orig.r, orig.g, orig.b, orig.a],
            "frame {i} top-left pixel mismatch after round-trip"
        );
    }
}

// ── Slices in JSON ────────────────────────────────────────────────────────────

#[test]
fn slices_appear_in_json_with_pivot() {
    let canvas = Size::new(32, 32);
    let mut sprite = sprite_with_n_frames("sliced", canvas, 1);
    sprite.slices.push(Slice {
        id: SliceId::new(1),
        name: "root".to_owned(),
        keys: vec![SliceKey {
            frame: FrameIndex::new(0),
            bounds: Rect::from_xywh(0, 0, 32, 32),
            nine_slice: None,
            pivot: Some(Pivot {
                offset: IVec2::new(16, 30),
            }),
        }],
        user_data: UserData::default(),
    });

    let frames = vec![transparent_buf(32, 32)];
    let output = export_sprite_sheet(
        &sprite,
        &frames,
        &ExportOptions {
            layout: LayoutStrategy::Grid { cols: 1 },
            sprite_name: "sliced".to_owned(),
        },
    )
    .unwrap();

    let json = decode_sheet_json(&output.json_bytes);
    let slices = json["meta"]["slices"].as_array().unwrap();
    assert_eq!(slices.len(), 1);
    assert_eq!(slices[0]["name"], "root");
    assert_eq!(slices[0]["color"], "#0000ffff");
    assert_eq!(slices[0]["keys"][0]["frame"], 0);
    assert_eq!(slices[0]["keys"][0]["pivot"]["x"], 16);
    assert_eq!(slices[0]["keys"][0]["pivot"]["y"], 30);
    assert!(slices[0]["keys"][0]["center"].is_null());
}

#[test]
fn slices_appear_in_json_with_nine_slice() {
    let canvas = Size::new(32, 32);
    let mut sprite = sprite_with_n_frames("nine", canvas, 1);
    sprite.slices.push(Slice {
        id: SliceId::new(1),
        name: "panel".to_owned(),
        keys: vec![SliceKey {
            frame: FrameIndex::new(0),
            bounds: Rect::from_xywh(0, 0, 32, 32),
            nine_slice: Some(NineSlice {
                center: Rect::from_xywh(4, 4, 24, 24),
            }),
            pivot: None,
        }],
        user_data: UserData::default(),
    });

    let frames = vec![transparent_buf(32, 32)];
    let output = export_sprite_sheet(
        &sprite,
        &frames,
        &ExportOptions {
            layout: LayoutStrategy::Grid { cols: 1 },
            sprite_name: "nine".to_owned(),
        },
    )
    .unwrap();

    let json = decode_sheet_json(&output.json_bytes);
    let slices = json["meta"]["slices"].as_array().unwrap();
    assert_eq!(slices[0]["keys"][0]["center"]["x"], 4);
    assert_eq!(slices[0]["keys"][0]["center"]["w"], 24);
    assert!(slices[0]["keys"][0]["pivot"].is_null());
}

// ── Meta fields ───────────────────────────────────────────────────────────────

#[test]
fn meta_fields_match_spec_constants() {
    let canvas = Size::new(8, 8);
    let sprite = sprite_with_n_frames("spec", canvas, 1);
    let frames = vec![transparent_buf(8, 8)];

    let output = export_sprite_sheet(
        &sprite,
        &frames,
        &ExportOptions {
            layout: LayoutStrategy::Grid { cols: 1 },
            sprite_name: "spec".to_owned(),
        },
    )
    .unwrap();

    let json = decode_sheet_json(&output.json_bytes);
    assert_eq!(json["meta"]["app"], "Pixhaus");
    assert_eq!(json["meta"]["version"], "1.0");
    assert_eq!(json["meta"]["format"], "RGBA8888");
    assert_eq!(json["meta"]["scale"], "1");
    assert_eq!(json["meta"]["image"], "spec.png");
}

#[test]
fn frame_filename_format_matches_spec() {
    let canvas = Size::new(8, 8);
    let sprite = sprite_with_n_frames("hero", canvas, 3);
    let frames: Vec<PixelBuffer> = (0..3).map(|_| transparent_buf(8, 8)).collect();

    let output = export_sprite_sheet(
        &sprite,
        &frames,
        &ExportOptions {
            layout: LayoutStrategy::ByRow,
            sprite_name: "hero".to_owned(),
        },
    )
    .unwrap();

    let json = decode_sheet_json(&output.json_bytes);
    assert_eq!(json["frames"][0]["filename"], "hero 0");
    assert_eq!(json["frames"][1]["filename"], "hero 1");
    assert_eq!(json["frames"][2]["filename"], "hero 2");
}
