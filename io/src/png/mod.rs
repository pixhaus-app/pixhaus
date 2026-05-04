//! PNG sprite sheet + JSON metadata export (stream S10).
//!
//! Packs composited frame images into a single sprite sheet PNG and emits
//! Aseprite-compatible JSON metadata, per the Unity handoff spec in
//! `docs/unity-handoff.md`.
//!
//! # Usage
//!
//! ```no_run
//! use pixhaus_io::png::{ExportOptions, LayoutStrategy, export_sprite_sheet};
//! use pixhaus_core::canvas::PixelBuffer;
//! use pixhaus_core::project::Sprite;
//! # fn sprite() -> Sprite { todo!() }
//! # fn frames() -> Vec<PixelBuffer> { todo!() }
//!
//! let sprite = sprite();
//! let composited_frames = frames();
//!
//! let output = export_sprite_sheet(
//!     &sprite,
//!     &composited_frames,
//!     &ExportOptions {
//!         layout: LayoutStrategy::Grid { cols: 8 },
//!         sprite_name: sprite.name.clone(),
//!     },
//! )?;
//!
//! std::fs::write("hero.png", &output.png_bytes)?;
//! std::fs::write("hero.json", &output.json_bytes)?;
//! # Ok::<(), pixhaus_io::Error>(())
//! ```

mod json;
mod pack;

pub use pack::LayoutStrategy;

use std::io::Cursor;

use image::{ImageFormat, RgbaImage};
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::Sprite;
use pixhaus_core::project::geometry::Size;

use crate::error::{Error, Result};

// ── Public types ──────────────────────────────────────────────────────────────

/// Options for a sprite sheet export.
pub struct ExportOptions {
    /// Frame layout strategy.
    pub layout: LayoutStrategy,
    /// Base name used for frame filenames and the PNG reference in the JSON.
    ///
    /// Frame filenames take the form `"{sprite_name} {frame_index}"`. The PNG
    /// file is referenced in `meta.image` as `"{sprite_name}.png"`.
    pub sprite_name: String,
}

/// Output of [`export_sprite_sheet`].
pub struct SheetOutput {
    /// PNG-encoded sprite sheet image.
    pub png_bytes: Vec<u8>,
    /// Aseprite-compatible JSON metadata (pretty-printed UTF-8).
    pub json_bytes: Vec<u8>,
}

// ── Alpha-trim ────────────────────────────────────────────────────────────────

/// Alpha-trim result for one frame.
///
/// Describes the bounding box of non-transparent pixels within the canvas,
/// in canvas pixel coordinates. Used to pack at trimmed dimensions and to
/// populate the Aseprite `spriteSourceSize` / `trimmed` JSON fields.
struct FrameTrim {
    /// Left edge of the opaque bbox in canvas coordinates.
    x: u32,
    /// Top edge of the opaque bbox in canvas coordinates.
    y: u32,
    /// Width of the opaque bbox (or 1 for a fully-transparent frame).
    w: u32,
    /// Height of the opaque bbox (or 1 for a fully-transparent frame).
    h: u32,
    /// `true` when the bbox is smaller than the full canvas.
    trimmed: bool,
}

/// Compute the alpha-trim bbox for a single composited frame.
///
/// Scans every pixel for `alpha > 0`. If the non-transparent region is
/// smaller than the canvas, returns a trimmed bbox. A fully-transparent
/// frame returns a 1×1 bbox at the origin so it still occupies a slot in
/// the sheet (avoiding index drift) without wasting space.
fn compute_trim(buf: &PixelBuffer) -> FrameTrim {
    let w = buf.width();
    let h = buf.height();

    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut has_opaque = false;

    for y in 0..h {
        let Some(row) = buf.row(y) else { continue };
        for x in 0..w {
            let off = (x as usize) * 4;
            let Some(alpha) = row.get(off + 3) else {
                continue;
            };
            if *alpha > 0 {
                has_opaque = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    if !has_opaque {
        return FrameTrim {
            x: 0,
            y: 0,
            w: 1,
            h: 1,
            trimmed: true,
        };
    }

    let bw = max_x - min_x + 1;
    let bh = max_y - min_y + 1;

    // If the bbox touches all four canvas edges the frame is not trimmed.
    if min_x == 0 && min_y == 0 && bw == w && bh == h {
        FrameTrim {
            x: 0,
            y: 0,
            w,
            h,
            trimmed: false,
        }
    } else {
        FrameTrim {
            x: min_x,
            y: min_y,
            w: bw,
            h: bh,
            trimmed: true,
        }
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Export `sprite` as a sprite sheet PNG + Aseprite-compatible JSON.
///
/// `composited_frames` must contain one [`PixelBuffer`] per frame, in
/// frame-index order. Each buffer must have the same dimensions as
/// `sprite.canvas`. Missing frames (no cel on any layer) should be supplied
/// as fully transparent buffers of the correct size — the exporter always
/// includes every frame in the sheet.
///
/// Frames are alpha-trimmed before packing: only the bounding box of
/// non-transparent pixels is blitted into the sheet, and the pack algorithm
/// uses the trimmed dimensions. Fully-transparent frames collapse to a 1×1
/// slot so that frame indices remain stable.
///
/// # Errors
///
/// - [`Error::NoFrames`] when `sprite.frames` is empty.
/// - [`Error::FrameCountMismatch`] when `composited_frames.len()` differs from
///   `sprite.frames.len()`.
/// - [`Error::FrameSizeMismatch`] when any buffer's dimensions differ from
///   `sprite.canvas`.
/// - [`Error::GridColsZero`] when `options.layout` is `Grid { cols: 0 }`.
/// - [`Error::SheetTooLarge`] when the packed sheet would exceed
///   [`pack::MAX_SHEET_DIM`] on either axis.
/// - [`Error::PngEncode`] when the `image` crate fails to encode the PNG.
/// - [`Error::JsonSerialize`] when JSON serialization fails.
pub fn export_sprite_sheet(
    sprite: &Sprite,
    composited_frames: &[PixelBuffer],
    options: &ExportOptions,
) -> Result<SheetOutput> {
    validate_frames(sprite, composited_frames)?;

    let trims: Vec<FrameTrim> = composited_frames.iter().map(compute_trim).collect();
    let trimmed_sizes: Vec<Size> = trims.iter().map(|t| Size::new(t.w, t.h)).collect();

    let pack_result = pack::pack_frames(&trimmed_sizes, &options.layout)?;

    let sheet_image = render_sheet(
        composited_frames,
        &pack_result.placements,
        &trims,
        pack_result.sheet_width,
        pack_result.sheet_height,
    );

    let png_bytes = encode_png(&sheet_image)?;
    let json_bytes = json::build_json(
        sprite,
        &pack_result.placements,
        &trims,
        pack_result.sheet_width,
        pack_result.sheet_height,
        &options.sprite_name,
    )?;

    Ok(SheetOutput {
        png_bytes,
        json_bytes,
    })
}

// ── Validation ────────────────────────────────────────────────────────────────

fn validate_frames(sprite: &Sprite, composited_frames: &[PixelBuffer]) -> Result<()> {
    if sprite.frames.is_empty() {
        return Err(Error::NoFrames);
    }
    if composited_frames.len() != sprite.frames.len() {
        return Err(Error::FrameCountMismatch {
            buffers: composited_frames.len(),
            frames: sprite.frames.len(),
        });
    }
    let expected_w = sprite.canvas.width;
    let expected_h = sprite.canvas.height;
    for (index, buf) in composited_frames.iter().enumerate() {
        if buf.width() != expected_w || buf.height() != expected_h {
            return Err(Error::FrameSizeMismatch {
                index,
                expected_w,
                expected_h,
                actual_w: buf.width(),
                actual_h: buf.height(),
            });
        }
    }
    Ok(())
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Blit all composited frames onto a blank RGBA sheet, each at its trimmed region.
fn render_sheet(
    frames: &[PixelBuffer],
    placements: &[pack::FramePlacement],
    trims: &[FrameTrim],
    width: u32,
    height: u32,
) -> RgbaImage {
    let mut sheet = RgbaImage::new(width, height);

    for ((buf, placement), trim) in frames.iter().zip(placements.iter()).zip(trims.iter()) {
        blit_trimmed(&mut sheet, buf, placement.x, placement.y, trim);
    }

    sheet
}

/// Copy the trimmed region of `src` onto `sheet` at `(dest_x, dest_y)`.
///
/// Uses `row()` so padded (non-tightly-packed) buffers work correctly.
fn blit_trimmed(
    sheet: &mut RgbaImage,
    src: &PixelBuffer,
    dest_x: u32,
    dest_y: u32,
    trim: &FrameTrim,
) {
    for dy in 0..trim.h {
        let Some(row) = src.row(trim.y + dy) else {
            continue;
        };
        let sheet_y = dest_y + dy;
        for dx in 0..trim.w {
            let off = ((trim.x + dx) as usize) * 4;
            let Some(bytes) = row.get(off..off + 4) else {
                continue;
            };
            sheet.put_pixel(
                dest_x + dx,
                sheet_y,
                image::Rgba([bytes[0], bytes[1], bytes[2], bytes[3]]),
            );
        }
    }
}

// ── PNG encoding ──────────────────────────────────────────────────────────────

fn encode_png(image: &RgbaImage) -> Result<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::new());
    image
        .write_to(&mut cursor, ImageFormat::Png)
        .map_err(Error::PngEncode)?;
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::geometry::{IVec2, Size};
    use pixhaus_core::project::id::{FrameIndex, LayerId, PixelBufferId, SpriteId};
    use pixhaus_core::project::layer::{Layer, LayerKind};
    use pixhaus_core::project::{
        BlendMode, Frame, FrameRange, FrameTag, LoopDirection, Rgba, Sprite, UserData,
    };

    fn make_frame_buf(w: u32, h: u32, color: Rgba) -> PixelBuffer {
        PixelBuffer::filled(w, h, color).unwrap()
    }

    fn sprite_with_frames(canvas: Size, frame_count: usize) -> Sprite {
        let mut sprite = Sprite::empty(SpriteId::new(1), "test", canvas);
        for _ in 0..frame_count {
            sprite.frames.push(Frame::default());
        }
        sprite
    }

    // ── Validation ────────────────────────────────────────────────────────────

    #[test]
    fn rejects_empty_sprite() {
        let sprite = Sprite::empty(SpriteId::new(1), "empty", Size::new(16, 16));
        let result = export_sprite_sheet(
            &sprite,
            &[],
            &ExportOptions {
                layout: LayoutStrategy::ByRow,
                sprite_name: "empty".to_owned(),
            },
        );
        assert!(matches!(result, Err(Error::NoFrames)));
    }

    #[test]
    fn rejects_buffer_count_mismatch() {
        let sprite = sprite_with_frames(Size::new(16, 16), 3);
        let frames = vec![make_frame_buf(16, 16, Rgba::transparent())]; // 1 instead of 3
        let result = export_sprite_sheet(
            &sprite,
            &frames,
            &ExportOptions {
                layout: LayoutStrategy::ByRow,
                sprite_name: "hero".to_owned(),
            },
        );
        assert!(matches!(result, Err(Error::FrameCountMismatch { .. })));
    }

    #[test]
    fn rejects_wrong_frame_size() {
        let sprite = sprite_with_frames(Size::new(32, 32), 1);
        let frames = vec![make_frame_buf(16, 16, Rgba::transparent())]; // wrong size
        let result = export_sprite_sheet(
            &sprite,
            &frames,
            &ExportOptions {
                layout: LayoutStrategy::ByRow,
                sprite_name: "hero".to_owned(),
            },
        );
        assert!(matches!(
            result,
            Err(Error::FrameSizeMismatch { index: 0, .. })
        ));
    }

    // ── Successful export ─────────────────────────────────────────────────────

    #[test]
    fn single_frame_grid_export_succeeds() {
        let canvas = Size::new(16, 16);
        let sprite = sprite_with_frames(canvas, 1);
        let red = Rgba::opaque(255, 0, 0);
        let frames = vec![make_frame_buf(16, 16, red)];

        let output = export_sprite_sheet(
            &sprite,
            &frames,
            &ExportOptions {
                layout: LayoutStrategy::Grid { cols: 1 },
                sprite_name: "hero".to_owned(),
            },
        )
        .unwrap();

        assert!(!output.png_bytes.is_empty());
        assert!(!output.json_bytes.is_empty());

        // PNG must start with the PNG magic bytes.
        assert_eq!(&output.png_bytes[..8], b"\x89PNG\r\n\x1a\n");

        // JSON must parse without error.
        let parsed: serde_json::Value = serde_json::from_slice(&output.json_bytes).unwrap();
        assert_eq!(parsed["meta"]["app"], "Pixhaus");
        assert_eq!(parsed["meta"]["image"], "hero.png");
        assert_eq!(parsed["frames"][0]["filename"], "hero 0");
        assert_eq!(parsed["frames"][0]["duration"], 100);
    }

    #[test]
    fn four_frame_grid_sheet_has_correct_dimensions() {
        let canvas = Size::new(16, 16);
        let sprite = sprite_with_frames(canvas, 4);
        // Fully opaque — no trimming; cell size equals full canvas.
        let opaque = Rgba::opaque(10, 20, 30);
        let frames: Vec<PixelBuffer> = (0..4).map(|_| make_frame_buf(16, 16, opaque)).collect();

        let output = export_sprite_sheet(
            &sprite,
            &frames,
            &ExportOptions {
                layout: LayoutStrategy::Grid { cols: 2 },
                sprite_name: "hero".to_owned(),
            },
        )
        .unwrap();

        let parsed: serde_json::Value = serde_json::from_slice(&output.json_bytes).unwrap();
        assert_eq!(parsed["meta"]["size"]["w"], 32);
        assert_eq!(parsed["meta"]["size"]["h"], 32);
        assert_eq!(parsed["frames"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn frame_pixels_appear_at_correct_sheet_position() {
        // Frame 0: red. Frame 1: green. Grid { cols: 2 } puts them side by side.
        let canvas = Size::new(4, 4);
        let sprite = sprite_with_frames(canvas, 2);
        let red = Rgba::opaque(255, 0, 0);
        let green = Rgba::opaque(0, 255, 0);
        let frames = vec![make_frame_buf(4, 4, red), make_frame_buf(4, 4, green)];

        let output = export_sprite_sheet(
            &sprite,
            &frames,
            &ExportOptions {
                layout: LayoutStrategy::Grid { cols: 2 },
                sprite_name: "test".to_owned(),
            },
        )
        .unwrap();

        // Decode the PNG and check pixel colours.
        let img = image::load_from_memory(&output.png_bytes)
            .unwrap()
            .into_rgba8();

        // Frame 0 at (0,0): should be red.
        let px0 = img.get_pixel(0, 0);
        assert_eq!(px0.0, [255, 0, 0, 255]);

        // Frame 1 at (4,0): should be green.
        let px1 = img.get_pixel(4, 0);
        assert_eq!(px1.0, [0, 255, 0, 255]);
    }

    #[test]
    fn json_includes_frame_tags() {
        let canvas = Size::new(8, 8);
        let mut sprite = sprite_with_frames(canvas, 4);
        sprite.frame_tags.push(FrameTag {
            name: "walk".to_owned(),
            range: FrameRange::new(FrameIndex::new(0), FrameIndex::new(3)),
            loop_direction: LoopDirection::PingPong,
            repeat: 0,
            user_data: UserData::default(),
        });

        let opaque = Rgba::opaque(1, 2, 3);
        let frames: Vec<PixelBuffer> = (0..4).map(|_| make_frame_buf(8, 8, opaque)).collect();

        let output = export_sprite_sheet(
            &sprite,
            &frames,
            &ExportOptions {
                layout: LayoutStrategy::ByRow,
                sprite_name: "anim".to_owned(),
            },
        )
        .unwrap();

        let parsed: serde_json::Value = serde_json::from_slice(&output.json_bytes).unwrap();
        let tags = parsed["meta"]["frameTags"].as_array().unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0]["name"], "walk");
        assert_eq!(tags[0]["direction"], "pingpong");
        assert_eq!(tags[0]["from"], 0);
        assert_eq!(tags[0]["to"], 3);
    }

    #[test]
    fn json_excludes_reference_layers() {
        let canvas = Size::new(8, 8);
        let mut sprite = sprite_with_frames(canvas, 1);
        sprite.layers.push(Layer::raster(LayerId::new(1), "body"));
        sprite.layers.push(Layer {
            id: LayerId::new(2),
            name: "reference".to_owned(),
            kind: LayerKind::Reference {
                image: PixelBufferId::new(1),
                origin: IVec2::zero(),
            },
            blend_mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            locked: false,
            parent: None,
            user_data: UserData::default(),
        });

        let frames = vec![make_frame_buf(8, 8, Rgba::transparent())];
        let output = export_sprite_sheet(
            &sprite,
            &frames,
            &ExportOptions {
                layout: LayoutStrategy::Grid { cols: 1 },
                sprite_name: "sprite".to_owned(),
            },
        )
        .unwrap();

        let parsed: serde_json::Value = serde_json::from_slice(&output.json_bytes).unwrap();
        let layers = parsed["meta"]["layers"].as_array().unwrap();
        assert_eq!(layers.len(), 1, "reference layer must be excluded");
        assert_eq!(layers[0]["name"], "body");
    }

    // ── Alpha-trim ────────────────────────────────────────────────────────────

    #[test]
    fn fully_opaque_frame_is_not_trimmed() {
        let canvas = Size::new(16, 16);
        let sprite = sprite_with_frames(canvas, 1);
        let frames = vec![make_frame_buf(16, 16, Rgba::opaque(255, 0, 0))];

        let output = export_sprite_sheet(
            &sprite,
            &frames,
            &ExportOptions {
                layout: LayoutStrategy::Grid { cols: 1 },
                sprite_name: "solid".to_owned(),
            },
        )
        .unwrap();

        let parsed: serde_json::Value = serde_json::from_slice(&output.json_bytes).unwrap();
        let f = &parsed["frames"][0];
        assert_eq!(f["trimmed"], false);
        assert_eq!(f["frame"]["w"], 16);
        assert_eq!(f["frame"]["h"], 16);
        assert_eq!(f["spriteSourceSize"]["x"], 0);
        assert_eq!(f["spriteSourceSize"]["y"], 0);
        assert_eq!(f["spriteSourceSize"]["w"], 16);
        assert_eq!(f["spriteSourceSize"]["h"], 16);
    }

    #[test]
    fn off_center_sprite_gets_trimmed_metadata() {
        // 16 × 16 canvas; only pixels at (8..15, 12..15) are opaque (bottom-right 8×4).
        let canvas = Size::new(16, 16);
        let sprite = sprite_with_frames(canvas, 1);

        let mut buf = PixelBuffer::new(16, 16).unwrap();
        // Paint 8 columns × 4 rows in the bottom-right corner.
        for y in 12u32..16 {
            for x in 8u32..16 {
                buf.set_pixel(x, y, Rgba::opaque(200, 100, 50));
            }
        }
        let frames = vec![buf];

        let output = export_sprite_sheet(
            &sprite,
            &frames,
            &ExportOptions {
                layout: LayoutStrategy::Grid { cols: 1 },
                sprite_name: "trimtest".to_owned(),
            },
        )
        .unwrap();

        let parsed: serde_json::Value = serde_json::from_slice(&output.json_bytes).unwrap();
        let f = &parsed["frames"][0];

        assert_eq!(f["trimmed"], true, "frame should be trimmed");
        // The packed frame rect in the sheet is the trimmed 8 × 4 region.
        assert_eq!(f["frame"]["w"], 8);
        assert_eq!(f["frame"]["h"], 4);
        // spriteSourceSize describes where the trimmed region sits on the canvas.
        assert_eq!(f["spriteSourceSize"]["x"], 8);
        assert_eq!(f["spriteSourceSize"]["y"], 12);
        assert_eq!(f["spriteSourceSize"]["w"], 8);
        assert_eq!(f["spriteSourceSize"]["h"], 4);
        // sourceSize is always the full canvas.
        assert_eq!(f["sourceSize"]["w"], 16);
        assert_eq!(f["sourceSize"]["h"], 16);
    }
}
