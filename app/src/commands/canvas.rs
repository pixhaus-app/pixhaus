//! Canvas operation commands.
//!
//! Pixel-drawing operations (`draw_stroke`, `fill`) are stubbed until stream
//! S01 (pixel buffer and blend modes) lands. Transform operations are
//! implemented as of stream S04. Viewport, selection, and composite-info
//! commands are fully implemented.

use base64::Engine;
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::canvas::tools::{BrushShape, draw_stroke, flood_fill};
use pixhaus_core::project::{
    CanvasState, Cel, CelData, FrameIndex, IVec2, LayerId, PixelBufferId, Rgba, SelectionRegion,
    SelectionState, Size, SpriteId,
};
use pixhaus_core::selection::SelectionMask;
use pixhaus_core::transforms::{self, RotateMode, ScaleMode, TransformSpec};
use pixhaus_io::pixhaus::PixelBufferEntry;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::error::{AppCommandError, CommandResult};
use crate::pixel_history::{PixelOp, PixelOpBatch};
use crate::state::AppState;

/// Tile size used by the canvas renderer, in canvas pixels per side.
pub const TILE_SIZE: u32 = 256;

/// Arguments for a freehand stroke.
#[derive(Debug, Deserialize)]
pub struct DrawStrokeArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Target layer.
    pub layer_id: LayerId,
    /// Target frame (0-indexed).
    pub frame_index: u32,
    /// Stroke path as `[x, y]` pairs in canvas coordinates.
    pub points: Vec<[f32; 2]>,
    /// Stroke color.
    pub color: Rgba,
    /// Per-point pressure values, same length as `points`. `1.0` = full pressure.
    pub pressure: Vec<f32>,
    /// Brush shape: `"pixel"`, `"circle"`, or `"square"`.
    #[serde(default = "default_brush_shape")]
    pub brush_shape: String,
    /// Brush diameter in canvas pixels.
    #[serde(default = "default_brush_size")]
    pub brush_size: u32,
    /// Enable pixel-perfect post-pass (removes diagonal corner artifacts).
    #[serde(default)]
    pub pixel_perfect: bool,
    /// Erase mode: draw with fully-transparent pixels instead of `color`.
    #[serde(default)]
    pub erase: bool,
}

fn default_brush_shape() -> String {
    "pixel".to_owned()
}

fn default_brush_size() -> u32 {
    1
}

/// Arguments for a flood fill. Requires S01.
#[derive(Debug, Deserialize)]
pub struct FillArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Target layer.
    pub layer_id: LayerId,
    /// Target frame (0-indexed).
    pub frame_index: u32,
    /// Seed point in canvas coordinates.
    pub x: i32,
    /// Seed point in canvas coordinates.
    pub y: i32,
    /// Fill color.
    pub color: Rgba,
    /// Tolerance for color matching (`0` = exact, `255` = match all).
    pub tolerance: u8,
}

/// One transform operation in a [`TransformArgs`] batch.
///
/// Operations are applied in order. The output of each becomes the
/// input of the next.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransformOpArg {
    /// Integer-pixel translate.
    Translate {
        /// Horizontal offset in pixels. Positive moves right.
        dx: i32,
        /// Vertical offset in pixels. Positive moves down.
        dy: i32,
    },
    /// Resize using nearest-neighbor sampling.
    ScaleNearest {
        /// Target width in pixels.
        new_width: u32,
        /// Target height in pixels.
        new_height: u32,
    },
    /// Integer-multiple upscale — each pixel becomes an N×N block.
    ScaleIntegerMultiple {
        /// Scale factor. Each source pixel becomes `factor × factor` pixels.
        factor: u32,
    },
    /// Integer-divisor downscale — samples top-left of each N×N block.
    ScaleIntegerDivisor {
        /// Divisor. Each `divisor × divisor` block maps to one output pixel.
        divisor: u32,
    },
    /// 90° clockwise rotation (lossless).
    Rotate90Cw,
    /// 90° counter-clockwise rotation (lossless).
    Rotate90Ccw,
    /// 180° rotation (lossless).
    Rotate180,
    /// Arbitrary rotation via `RotSprite` (pixel-art quality).
    RotateRotSprite {
        /// Rotation angle in degrees, counter-clockwise.
        degrees: f32,
    },
    /// Arbitrary rotation via direct bilinear interpolation.
    RotateBilinear {
        /// Rotation angle in degrees, counter-clockwise.
        degrees: f32,
    },
    /// Horizontal mirror.
    FlipHorizontal,
    /// Vertical mirror.
    FlipVertical,
    /// Horizontal shear by `factor` pixels per row.
    SkewX {
        /// Shear magnitude: horizontal offset per pixel of height.
        factor: f32,
    },
    /// Vertical shear by `factor` pixels per column.
    SkewY {
        /// Shear magnitude: vertical offset per pixel of width.
        factor: f32,
    },
    /// Projective warp — `corners` is `[TL, TR, BR, BL]` in canvas pixels.
    Perspective {
        /// Destination corners: `[top-left, top-right, bottom-right, bottom-left]`.
        corners: [(f32, f32); 4],
    },
}

impl TransformOpArg {
    fn to_transform_spec(&self) -> TransformSpec {
        match self {
            Self::Translate { dx, dy } => TransformSpec::Translate { dx: *dx, dy: *dy },
            Self::ScaleNearest {
                new_width,
                new_height,
            } => TransformSpec::Scale {
                new_width: *new_width,
                new_height: *new_height,
                mode: ScaleMode::NearestNeighbor,
            },
            Self::ScaleIntegerMultiple { factor } => TransformSpec::Scale {
                new_width: 0, // not used for IntegerMultiple
                new_height: 0,
                mode: ScaleMode::IntegerMultiple(*factor),
            },
            Self::ScaleIntegerDivisor { divisor } => TransformSpec::Scale {
                new_width: 0,
                new_height: 0,
                mode: ScaleMode::IntegerDivisor(*divisor),
            },
            Self::Rotate90Cw => TransformSpec::Rotate90Cw,
            Self::Rotate90Ccw => TransformSpec::Rotate90Ccw,
            Self::Rotate180 => TransformSpec::Rotate180,
            Self::RotateRotSprite { degrees } => TransformSpec::RotateArbitrary {
                degrees: *degrees,
                mode: RotateMode::RotSprite,
            },
            Self::RotateBilinear { degrees } => TransformSpec::RotateArbitrary {
                degrees: *degrees,
                mode: RotateMode::Bilinear,
            },
            Self::FlipHorizontal => TransformSpec::FlipHorizontal,
            Self::FlipVertical => TransformSpec::FlipVertical,
            Self::SkewX { factor } => TransformSpec::SkewX { factor: *factor },
            Self::SkewY { factor } => TransformSpec::SkewY { factor: *factor },
            Self::Perspective { corners } => TransformSpec::Perspective { corners: *corners },
        }
    }
}

/// Arguments for a transform operation batch.
#[derive(Debug, Deserialize)]
pub struct TransformArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Target layer.
    pub layer_id: LayerId,
    /// Target frame (0-indexed).
    pub frame_index: u32,
    /// Ordered list of operations to apply. Applied sequentially; the
    /// output of each becomes the input of the next.
    pub ops: Vec<TransformOpArg>,
}

/// Metadata returned by [`canvas_composite`].
///
/// Tells the frontend how many tiles cover the sprite and their dimensions.
/// Actual pixel data arrives later via `canvas:tile-dirty` events emitted when
/// drawing operations change the pixel buffer (streams S15+).
#[derive(Debug, Serialize)]
pub struct CanvasComposite {
    /// Sprite canvas width in pixels.
    pub sprite_width: u32,
    /// Sprite canvas height in pixels.
    pub sprite_height: u32,
    /// Tile side length in canvas pixels.
    pub tile_size: u32,
    /// Number of tile columns (`ceil(sprite_width / tile_size)`).
    pub tiles_x: u32,
    /// Number of tile rows (`ceil(sprite_height / tile_size)`).
    pub tiles_y: u32,
}

/// Payload emitted with each `canvas:tile-dirty` event.
///
/// The frontend renderer listens for this event and uploads `data` (base64
/// RGBA bytes) into the GPU tile cache for the matching `(sprite, frame, tx,
/// ty)` key.  One event per tile keeps payload size predictable: at most 256
/// kib for a full 256x256 tile.
#[derive(Debug, Serialize, Clone)]
pub struct TileDirtyPayload {
    /// Sprite the tile belongs to.
    pub sprite_id: u32,
    /// Frame index covered by the tile.
    pub frame_index: u32,
    /// Tile column in the sprite's tile grid.
    pub tile_x: u32,
    /// Tile row in the sprite's tile grid.
    pub tile_y: u32,
    /// Tile width in canvas pixels (may be < `TILE_SIZE` at the right edge).
    pub width: u32,
    /// Tile height in canvas pixels (may be < `TILE_SIZE` at the bottom edge).
    pub height: u32,
    /// Standard-base64-encoded RGBA8 bytes (`width * height * 4` raw bytes).
    pub data: String,
}

/// Builds an all-transparent `TileDirtyPayload` for a tile of the given size.
///
/// Used by the (currently dormant) `emit_tile_dirty_for_sprite` helper.
/// S15 will replace the all-zero buffer with real composited bytes; the
/// payload shape stays stable.
#[allow(
    dead_code,
    reason = "consumed by S15 once the pixel-buffer registry is wired"
)]
fn make_empty_tile_payload(
    sprite_id: SpriteId,
    frame_index: u32,
    tile_x: u32,
    tile_y: u32,
    width: u32,
    height: u32,
) -> TileDirtyPayload {
    let pixels = (width as usize) * (height as usize) * 4;
    // Zeroed RGBA = fully transparent.  The renderer composites tiles over a
    // checkerboard, so this looks identical to "no tile bound" but exercises
    // the upload path.
    let bytes = vec![0u8; pixels];
    let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
    TileDirtyPayload {
        sprite_id: sprite_id.get(),
        frame_index,
        tile_x,
        tile_y,
        width,
        height,
        data,
    }
}

/// Emits one `canvas:tile-dirty` event per tile covering the sprite's canvas.
///
/// Currently dormant — `canvas_composite` does *not* call this. Emitting
/// hundreds of multi-KiB transparent payloads on every sprite switch
/// produced a multi-MiB IPC burst with no visual effect (the renderer
/// composites against a checkerboard when no tile is bound). S15 will
/// re-enable a guarded version once it has real pixel data to ship,
/// likely scoped to the visible region rather than the whole canvas.
/// Events are fire-and-forget — emit failures are logged but never
/// propagated, so a transient IPC hiccup can't fail the composite call.
#[allow(
    dead_code,
    reason = "consumed by S15 once the pixel-buffer registry is wired"
)]
fn emit_tile_dirty_for_sprite(
    app: &AppHandle,
    sprite_id: SpriteId,
    frame_index: u32,
    sprite_width: u32,
    sprite_height: u32,
) {
    let tiles_x = sprite_width.div_ceil(TILE_SIZE);
    let tiles_y = sprite_height.div_ceil(TILE_SIZE);

    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let x0 = tx * TILE_SIZE;
            let y0 = ty * TILE_SIZE;
            let w = (sprite_width - x0).min(TILE_SIZE);
            let h = (sprite_height - y0).min(TILE_SIZE);
            let payload = make_empty_tile_payload(sprite_id, frame_index, tx, ty, w, h);
            if let Err(err) = app.emit("canvas:tile-dirty", payload) {
                tracing::warn!(
                    "failed to emit canvas:tile-dirty for sprite {} tile ({},{}): {}",
                    sprite_id.get(),
                    tx,
                    ty,
                    err
                );
            }
        }
    }
}

/// Emits `canvas:tile-dirty` events for every tile of a pixel buffer after
/// a drawing operation.  One event per tile; each carries the extracted
/// RGBA bytes for its region.  Stride-aware: reads at `buf_stride` bytes
/// per row and packs the tile data tight before base64-encoding.
pub(crate) fn emit_buffer_tiles(
    app: &AppHandle,
    sprite_id: u32,
    frame_index: u32,
    buf_w: u32,
    buf_h: u32,
    buf_stride: u32,
    pixels: &[u8],
) {
    let tiles_x = buf_w.div_ceil(TILE_SIZE);
    let tiles_y = buf_h.div_ceil(TILE_SIZE);

    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let x0 = tx * TILE_SIZE;
            let y0 = ty * TILE_SIZE;
            let w = (buf_w - x0).min(TILE_SIZE);
            let h = (buf_h - y0).min(TILE_SIZE);

            let mut tile_bytes = vec![0u8; (w * h * 4) as usize];
            for row in 0..h {
                let src =
                    ((y0 + row) as usize * buf_stride as usize + x0 as usize * 4).min(pixels.len());
                let dst = row as usize * w as usize * 4;
                let copy_len = (w as usize * 4).min(pixels.len().saturating_sub(src));
                tile_bytes[dst..dst + copy_len].copy_from_slice(&pixels[src..src + copy_len]);
            }

            let data = base64::engine::general_purpose::STANDARD.encode(&tile_bytes);
            let payload = TileDirtyPayload {
                sprite_id,
                frame_index,
                tile_x: tx,
                tile_y: ty,
                width: w,
                height: h,
                data,
            };
            if let Err(err) = app.emit("canvas:tile-dirty", payload) {
                tracing::warn!("failed to emit canvas:tile-dirty tile ({tx},{ty}): {err}");
            }
        }
    }
}

/// Parses a brush shape string into the core `BrushShape` enum.
fn parse_brush_shape(s: &str) -> BrushShape {
    match s {
        "circle" => BrushShape::Circle,
        "square" => BrushShape::Square,
        _ => BrushShape::Pixel,
    }
}

/// Finds an existing raster buffer id for `(layer_id, frame_index)` in `sprite.cels`.
fn find_cel_buffer(cels: &[Cel], layer_id: LayerId, frame_index: u32) -> Option<PixelBufferId> {
    cels.iter().find_map(|c| {
        if c.layer_id == layer_id && c.frame_index.get() == frame_index {
            if let CelData::Raster { buffer, .. } = c.data {
                Some(buffer)
            } else {
                None
            }
        } else {
            None
        }
    })
}

/// Ensures a raster pixel buffer exists for `(sprite_id, layer_id, frame_index)`.
///
/// If no cel exists for that address, creates a transparent buffer the size of
/// the sprite canvas and registers both the buffer and the cel. Returns the
/// buffer id.
fn ensure_raster_buffer(
    doc: &mut crate::state::DocumentStore,
    sprite_id: SpriteId,
    layer_id: LayerId,
    frame_index: u32,
) -> CommandResult<PixelBufferId> {
    let (canvas_w, canvas_h, existing) = {
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let sprite = project
            .sprites
            .iter()
            .find(|s| s.id == sprite_id)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            })?;
        let buf_id = find_cel_buffer(&sprite.cels, layer_id, frame_index);
        (sprite.canvas.width, sprite.canvas.height, buf_id)
    };

    if let Some(id) = existing {
        return Ok(id);
    }

    let new_id = PixelBufferId::new(doc.next_id);
    doc.next_id += 1;
    let stride = canvas_w * 4;
    doc.pixel_buffers.push(PixelBufferEntry {
        id: new_id.get(),
        width: canvas_w,
        height: canvas_h,
        stride,
        pixels: vec![0u8; (stride * canvas_h) as usize],
    });
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let sprite = project
        .sprites
        .iter_mut()
        .find(|s| s.id == sprite_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?;
    sprite.cels.push(Cel::raster(
        layer_id,
        FrameIndex::new(frame_index),
        new_id,
        Size::new(canvas_w, canvas_h),
    ));
    Ok(new_id)
}

/// Applies a pixel mutation, records an undo op, and emits tile-dirty events.
#[allow(clippy::too_many_arguments)]
fn commit_pixel_op(
    doc: &mut crate::state::DocumentStore,
    app: &AppHandle,
    buffer_id: PixelBufferId,
    sprite_id: SpriteId,
    frame_index: u32,
    before: Vec<u8>,
    new_pixels: Vec<u8>,
    label: &str,
) -> CommandResult<()> {
    let entry = doc
        .pixel_buffers
        .iter_mut()
        .find(|e| e.id == buffer_id.get())
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "pixel buffer".into(),
            id: u64::from(buffer_id.get()),
        })?;

    entry.pixels = new_pixels;

    let op = PixelOp {
        buffer_id: entry.id,
        buf_width: entry.width,
        buf_height: entry.height,
        buf_stride: entry.stride,
        sprite_id: sprite_id.get(),
        frame_index,
        before,
        after: entry.pixels.clone(),
    };
    let (w, h, stride) = (entry.width, entry.height, entry.stride);
    let emit_pixels = entry.pixels.clone();

    doc.pixel_history.push(PixelOpBatch {
        label: label.to_owned(),
        ops: vec![op],
    });
    emit_buffer_tiles(
        app,
        sprite_id.get(),
        frame_index,
        w,
        h,
        stride,
        &emit_pixels,
    );
    doc.dirty = true;
    Ok(())
}

/// Returns the tile grid dimensions for the given sprite.
///
/// The renderer calls this when a sprite becomes active to learn its canvas
/// size and tile layout. The renderer is also wired to consume
/// `canvas:tile-dirty` events for tile uploads, but this command does not
/// emit them: at the current stream cut there are no real pixel buffers
/// to ship, and emitting one event per tile (256 events for a 4096x4096
/// sprite, each carrying a base64-encoded 256 KiB transparent payload)
/// caused multi-MiB IPC bursts on every sprite switch with no visual
/// effect — the renderer composites against a checkerboard when no tile
/// is bound. Stream S15 is responsible for emitting tile events backed by
/// real pixel data once S01's buffer registry is wired in; the IPC
/// contract (event name + payload shape) is stable from this stream
/// onward so S15 only adds the producer.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_composite(
    sprite_id: SpriteId,
    state: State<'_, AppState>,
) -> CommandResult<CanvasComposite> {
    let (w, h) = {
        let doc = state.doc.read().await;
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let sprite = project
            .sprites
            .iter()
            .find(|s| s.id == sprite_id)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            })?;
        (sprite.canvas.width, sprite.canvas.height)
    };

    Ok(CanvasComposite {
        sprite_width: w,
        sprite_height: h,
        tile_size: TILE_SIZE,
        tiles_x: w.div_ceil(TILE_SIZE),
        tiles_y: h.div_ceil(TILE_SIZE),
    })
}

/// Paints a freehand stroke on a layer cel.
///
/// Creates a transparent cel + buffer lazily if none exists for the
/// `(layer, frame)` pair.  Pushes a `PixelOpBatch` for undo and emits
/// `canvas:tile-dirty` events for every tile that covers the buffer.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_draw_stroke(
    app: AppHandle,
    args: DrawStrokeArgs,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;

    let buffer_id = ensure_raster_buffer(doc, args.sprite_id, args.layer_id, args.frame_index)?;

    let (before, w, h, stride) = {
        let entry = doc
            .pixel_buffers
            .iter()
            .find(|e| e.id == buffer_id.get())
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "pixel buffer".into(),
                id: u64::from(buffer_id.get()),
            })?;
        (
            entry.pixels.clone(),
            entry.width,
            entry.height,
            entry.stride,
        )
    };

    let mut pbuf = PixelBuffer::from_raw(w, h, stride, before.clone()).map_err(|e| {
        AppCommandError::Validation {
            detail: e.to_string(),
        }
    })?;

    let color = if args.erase {
        Rgba::transparent()
    } else {
        args.color
    };
    let shape = parse_brush_shape(&args.brush_shape);
    draw_stroke(
        &mut pbuf,
        &args.points,
        color,
        shape,
        args.brush_size,
        args.pixel_perfect,
    );

    commit_pixel_op(
        doc,
        &app,
        buffer_id,
        args.sprite_id,
        args.frame_index,
        before,
        pbuf.as_bytes().to_vec(),
        "stroke",
    )
}

/// Flood-fills a contiguous region on a layer cel.
///
/// Creates a cel + buffer lazily, applies BFS flood-fill, then emits
/// `canvas:tile-dirty` events for the whole buffer.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_fill(
    app: AppHandle,
    args: FillArgs,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;

    let buffer_id = ensure_raster_buffer(doc, args.sprite_id, args.layer_id, args.frame_index)?;

    let (before, w, h, stride) = {
        let entry = doc
            .pixel_buffers
            .iter()
            .find(|e| e.id == buffer_id.get())
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "pixel buffer".into(),
                id: u64::from(buffer_id.get()),
            })?;
        (
            entry.pixels.clone(),
            entry.width,
            entry.height,
            entry.stride,
        )
    };

    let mut pbuf = PixelBuffer::from_raw(w, h, stride, before.clone()).map_err(|e| {
        AppCommandError::Validation {
            detail: e.to_string(),
        }
    })?;

    flood_fill(&mut pbuf, args.x, args.y, args.color, args.tolerance);

    commit_pixel_op(
        doc,
        &app,
        buffer_id,
        args.sprite_id,
        args.frame_index,
        before,
        pbuf.as_bytes().to_vec(),
        "fill",
    )
}

/// Applies one or more geometric transforms to a raster layer cel.
///
/// Operations in `args.ops` are applied sequentially; the output of each
/// step becomes the input of the next. The final result is written back to
/// the pixel buffer store and the cel's size is updated if the dimensions
/// changed (e.g. after a 90° rotation).
///
/// The active canvas selection is forwarded to transforms that support it
/// ([`TransformOpArg::Translate`], [`TransformOpArg::FlipHorizontal`],
/// [`TransformOpArg::FlipVertical`]). Other operations ignore the selection
/// and apply to the full buffer.
///
/// # Errors
///
/// - [`AppCommandError::NoActiveProject`] — no project is open.
/// - [`AppCommandError::NotFound`] — sprite, layer, cel, or pixel buffer
///   is not present.
/// - [`AppCommandError::Validation`] — transform failed (e.g. empty buffer,
///   invalid scale factor).
/// - [`AppCommandError::Unimplemented`] — cel is not a raster cel.
#[allow(
    clippy::too_many_lines,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_transform(
    args: TransformArgs,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let frame_index = FrameIndex::new(args.frame_index);

    // ── Phase 1: read project state, then release the borrow ──────────────
    // `doc.project` and `doc.pixel_buffers` are separate fields but Rust
    // can't tell that through a shared `&mut doc` borrow, so we extract
    // everything we need from the project before touching pixel_buffers.
    let (sprite_idx, buf_id, cel_pos, sprite_canvas_w, sprite_canvas_h, selection_region) = {
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;

        let sprite_idx = project
            .sprites
            .iter()
            .position(|s| s.id == args.sprite_id)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(args.sprite_id.get()),
            })?;

        let sprite = &project.sprites[sprite_idx];
        let cel = sprite
            .cels
            .iter()
            .find(|c| c.layer_id == args.layer_id && c.frame_index == frame_index)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "cel".into(),
                id: u64::from(args.frame_index),
            })?;

        let buf_id = match &cel.data {
            CelData::Raster { buffer, .. } => *buffer,
            _ => {
                return Err(AppCommandError::Unimplemented {
                    stream: "raster cels only".into(),
                });
            }
        };

        let selection_region = project.selection.region.clone();
        (
            sprite_idx,
            buf_id,
            cel.position,
            sprite.canvas.width,
            sprite.canvas.height,
            selection_region,
        )
    }; // project borrow released

    // ── Phase 2: pixel buffer operations ──────────────────────────────────
    let entry_idx = doc
        .pixel_buffers
        .iter()
        .position(|e| e.id == buf_id.get())
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "pixel buffer".into(),
            id: u64::from(buf_id.get()),
        })?;

    let pixel_buf = {
        let entry = &doc.pixel_buffers[entry_idx];
        PixelBuffer::from_raw(
            entry.width,
            entry.height,
            entry.stride,
            entry.pixels.clone(),
        )
        .map_err(|e| AppCommandError::Validation {
            detail: e.to_string(),
        })?
    };

    // ── Build a selection mask for the cel, if there is one ────────────────
    let mask: Option<SelectionMask> = match &selection_region {
        None => None,
        Some(SelectionRegion::Rect { bounds }) => {
            let bw = pixel_buf.width();
            let bh = pixel_buf.height();
            let mut m = SelectionMask::new(bw, bh).map_err(|e| AppCommandError::Validation {
                detail: e.to_string(),
            })?;
            // Convert canvas-space rect to cel-local buffer coordinates.
            let rx = bounds.origin.x - cel_pos.x;
            let ry = bounds.origin.y - cel_pos.y;
            let rw = bounds.size.width as i32;
            let rh = bounds.size.height as i32;
            for y in 0..bh as i32 {
                for x in 0..bw as i32 {
                    if x >= rx && x < rx + rw && y >= ry && y < ry + rh {
                        m.set(x as u32, y as u32, 255);
                    }
                }
            }
            if m.selected_count() == 0 {
                None
            } else {
                Some(m)
            }
        }
        Some(SelectionRegion::Mask { mask: mask_id, .. }) => doc
            .pixel_buffers
            .iter()
            .find(|e| e.id == mask_id.get())
            .and_then(|e| SelectionMask::from_raw(e.width, e.height, e.pixels.clone()).ok()),
    };

    // ── Apply each transform operation in sequence ─────────────────────────
    let mut current = pixel_buf;
    for op in &args.ops {
        let spec = op.to_transform_spec();
        current = transforms::apply_transform(&spec, &current, mask.as_ref()).map_err(|e| {
            AppCommandError::Validation {
                detail: e.to_string(),
            }
        })?;
    }

    let new_w = current.width();
    let new_h = current.height();
    let new_stride = current.stride();
    let new_pixels = current.as_bytes().to_vec();

    doc.pixel_buffers[entry_idx] = PixelBufferEntry {
        id: buf_id.get(),
        width: new_w,
        height: new_h,
        stride: new_stride,
        pixels: new_pixels,
    };

    // ── Phase 3: update project state ─────────────────────────────────────
    {
        let project = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?;
        let sprite = &mut project.sprites[sprite_idx];
        let orig_size = sprite
            .cels
            .iter()
            .find(|c| c.layer_id == args.layer_id && c.frame_index == frame_index)
            .and_then(|c| match &c.data {
                CelData::Raster { size, .. } => Some(*size),
                _ => None,
            })
            .unwrap_or_else(|| Size::new(new_w, new_h));

        if new_w != orig_size.width || new_h != orig_size.height {
            if let Some(cel) = sprite
                .cels
                .iter_mut()
                .find(|c| c.layer_id == args.layer_id && c.frame_index == frame_index)
            {
                cel.data = CelData::Raster {
                    buffer: buf_id,
                    size: Size::new(new_w, new_h),
                };
            }
        }
    } // project borrow released

    doc.dirty = true;

    // ── Emit tile-dirty events for the affected sprite region ──────────────
    // The renderer listens for canvas:tile-dirty and re-uploads changed tiles.
    // We emit the full set of tiles covering the canvas so the renderer refreshes.
    let tiles_x = sprite_canvas_w.div_ceil(TILE_SIZE);
    let tiles_y = sprite_canvas_h.div_ceil(TILE_SIZE);
    let sprite_id_raw = args.sprite_id.get();
    let frame_idx_raw = args.frame_index;

    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let x0 = tx * TILE_SIZE;
            let y0 = ty * TILE_SIZE;
            let w = (sprite_canvas_w - x0).min(TILE_SIZE);
            let h = (sprite_canvas_h - y0).min(TILE_SIZE);
            let pixels = (w as usize) * (h as usize) * 4;
            let data = base64::engine::general_purpose::STANDARD.encode(vec![0u8; pixels]);
            let payload = TileDirtyPayload {
                sprite_id: sprite_id_raw,
                frame_index: frame_idx_raw,
                tile_x: tx,
                tile_y: ty,
                width: w,
                height: h,
                data,
            };
            if let Err(err) = app.emit("canvas:tile-dirty", payload) {
                tracing::warn!(
                    "failed to emit canvas:tile-dirty for sprite {} tile ({tx},{ty}): {err}",
                    sprite_id_raw,
                );
            }
        }
    }

    Ok(())
}

/// Sets the canvas selection. Pass `None` for `region` to clear the selection.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_set_selection(
    region: Option<SelectionRegion>,
    anchor_layer: Option<LayerId>,
    state: State<'_, AppState>,
) -> CommandResult<SelectionState> {
    let mut doc = state.doc.write().await;
    let selection = SelectionState {
        region,
        anchor_layer,
    };
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.selection = selection.clone();
    doc.dirty = true;
    Ok(selection)
}

/// Selects the entire canvas of the active sprite as a rectangular region.
///
/// Returns the updated [`SelectionState`] so the UI can update its signals.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_select_all(
    sprite_id: SpriteId,
    state: State<'_, AppState>,
) -> CommandResult<SelectionState> {
    use pixhaus_core::project::Rect;

    let (w, h) = {
        let doc = state.doc.read().await;
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let sprite = project
            .sprites
            .iter()
            .find(|s| s.id == sprite_id)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            })?;
        (sprite.canvas.width, sprite.canvas.height)
    };

    let region = SelectionRegion::Rect {
        bounds: Rect::from_xywh(0, 0, w, h),
    };
    let selection = SelectionState {
        region: Some(region),
        anchor_layer: None,
    };

    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.selection = selection.clone();
    doc.dirty = true;
    Ok(selection)
}

/// Inverts the current selection.
///
/// Requires stream S01 (pixel buffers for mask operations). Returns an
/// error until S01 lands. A fully selected canvas inverted to nothing (and
/// vice versa) could be handled as a rect-only fast path, but is kept as a
/// stub for consistency with the mask-based general case.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_invert_selection(
    _sprite_id: SpriteId,
    _anchor_layer: Option<LayerId>,
    _state: State<'_, AppState>,
) -> CommandResult<SelectionState> {
    Err(AppCommandError::Unimplemented {
        stream: "S01 (pixel buffers)".into(),
    })
}

/// Selects a contiguous region via flood-fill from a seed pixel.
///
/// Requires stream S01 (pixel buffers). Returns an error until S01 lands.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_select_magic_wand(
    _sprite_id: SpriteId,
    _anchor_layer: Option<LayerId>,
    _seed_x: i32,
    _seed_y: i32,
    _tolerance: u8,
    _connectivity: String,
    _state: State<'_, AppState>,
) -> CommandResult<SelectionState> {
    Err(AppCommandError::Unimplemented {
        stream: "S01 (pixel buffers)".into(),
    })
}

/// Selects all pixels within a given color tolerance of the target color.
///
/// Requires stream S01 (pixel buffers). Returns an error until S01 lands.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_select_color_range(
    _sprite_id: SpriteId,
    _anchor_layer: Option<LayerId>,
    _x: i32,
    _y: i32,
    _target_color: Rgba,
    _tolerance: u8,
    _state: State<'_, AppState>,
) -> CommandResult<SelectionState> {
    Err(AppCommandError::Unimplemented {
        stream: "S01 (pixel buffers)".into(),
    })
}

/// Selects the polygon defined by the given points.
///
/// Requires stream S01 (pixel buffers). Returns an error until S01 lands.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_select_lasso(
    _sprite_id: SpriteId,
    _anchor_layer: Option<LayerId>,
    _points: Vec<IVec2>,
    _state: State<'_, AppState>,
) -> CommandResult<SelectionState> {
    Err(AppCommandError::Unimplemented {
        stream: "S01 (pixel buffers)".into(),
    })
}

/// Replaces the entire canvas viewport state (scroll, zoom, active ids, toggles).
///
/// The UI owns viewport state and pushes it here on every meaningful change
/// so that save/load can restore the last viewport.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_set_viewport(
    canvas: CanvasState,
    state: State<'_, AppState>,
) -> CommandResult<CanvasState> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.canvas = canvas.clone();
    doc.dirty = true;
    Ok(canvas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::{Project, Size, Sprite};

    #[test]
    fn draw_stroke_args_pressure_and_points_match() {
        let args = DrawStrokeArgs {
            sprite_id: SpriteId::new(1),
            layer_id: LayerId::new(2),
            frame_index: 0,
            points: vec![[0.0, 0.0], [1.0, 1.0]],
            color: Rgba::opaque(255, 0, 0),
            pressure: vec![1.0, 0.8],
            brush_shape: "pixel".to_owned(),
            brush_size: 1,
            pixel_perfect: false,
            erase: false,
        };
        assert_eq!(args.points.len(), args.pressure.len());
    }

    #[test]
    fn canvas_composite_tile_counts_round_up() {
        // 256 × 256 sprite → exactly 1 tile in each dimension.
        let w = 256u32;
        let h = 256u32;
        assert_eq!(w.div_ceil(TILE_SIZE), 1);
        assert_eq!(h.div_ceil(TILE_SIZE), 1);

        // 257 × 257 → 2 tiles in each dimension.
        let w2 = 257u32;
        let h2 = 257u32;
        assert_eq!(w2.div_ceil(TILE_SIZE), 2);
        assert_eq!(h2.div_ceil(TILE_SIZE), 2);
    }

    #[test]
    fn canvas_composite_large_sprite_tile_grid() {
        // 4096 × 2048 sprite → 16 × 8 tile grid.
        assert_eq!(4096u32.div_ceil(TILE_SIZE), 16);
        assert_eq!(2048u32.div_ceil(TILE_SIZE), 8);
    }

    #[test]
    fn canvas_composite_metadata_matches_sprite() {
        let mut project = Project::new("test");
        let sprite = Sprite::empty(SpriteId::new(1), "hero", Size::new(64, 48));
        project.sprites.push(sprite);

        let sprite = project.sprites.first().unwrap();
        let composite = CanvasComposite {
            sprite_width: sprite.canvas.width,
            sprite_height: sprite.canvas.height,
            tile_size: TILE_SIZE,
            tiles_x: sprite.canvas.width.div_ceil(TILE_SIZE),
            tiles_y: sprite.canvas.height.div_ceil(TILE_SIZE),
        };
        assert_eq!(composite.sprite_width, 64);
        assert_eq!(composite.sprite_height, 48);
        assert_eq!(composite.tiles_x, 1);
        assert_eq!(composite.tiles_y, 1);
    }

    #[test]
    fn empty_tile_payload_decodes_to_transparent_pixels() {
        let payload = make_empty_tile_payload(SpriteId::new(7), 3, 1, 2, 4, 4);
        assert_eq!(payload.sprite_id, 7);
        assert_eq!(payload.frame_index, 3);
        assert_eq!(payload.tile_x, 1);
        assert_eq!(payload.tile_y, 2);
        assert_eq!(payload.width, 4);
        assert_eq!(payload.height, 4);

        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&payload.data)
            .unwrap();
        assert_eq!(decoded.len(), 4 * 4 * 4);
        assert!(decoded.iter().all(|b| *b == 0));
    }

    #[test]
    fn empty_tile_payload_handles_partial_edge_tile() {
        // Right-edge tile that is only 100 pixels wide.
        let payload = make_empty_tile_payload(SpriteId::new(1), 0, 0, 0, 100, 256);
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&payload.data)
            .unwrap();
        assert_eq!(decoded.len(), 100 * 256 * 4);
    }

    #[test]
    fn canvas_select_all_region_covers_full_canvas() {
        use pixhaus_core::project::Rect;
        // Verify the rect is built with the right origin and size.
        let region = SelectionRegion::Rect {
            bounds: Rect::from_xywh(0, 0, 64, 48),
        };
        match region {
            SelectionRegion::Rect { bounds } => {
                assert_eq!(bounds.origin.x, 0);
                assert_eq!(bounds.origin.y, 0);
                assert_eq!(bounds.size.width, 64);
                assert_eq!(bounds.size.height, 48);
            }
            SelectionRegion::Mask { .. } => panic!("expected Rect variant"),
        }
    }

    #[test]
    fn empty_tile_payload_serializes_with_snake_case_keys() {
        let payload = make_empty_tile_payload(SpriteId::new(2), 0, 0, 0, 1, 1);
        let json = serde_json::to_string(&payload).unwrap();
        // The Tauri event contract uses snake_case so the TS listener can
        // destructure { sprite_id, frame_index, tile_x, tile_y } directly.
        assert!(json.contains("\"sprite_id\":2"));
        assert!(json.contains("\"frame_index\":0"));
        assert!(json.contains("\"tile_x\":0"));
        assert!(json.contains("\"tile_y\":0"));
        assert!(json.contains("\"width\":1"));
        assert!(json.contains("\"height\":1"));
        assert!(json.contains("\"data\":"));
    }
}
