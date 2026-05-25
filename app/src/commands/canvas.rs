//! Canvas operation commands.
//!
//! Pixel-drawing operations (`draw_stroke`, `fill`) are stubbed until stream
//! S01 (pixel buffer and blend modes) lands. Transform operations are
//! implemented as of stream S04. Viewport, selection, and composite-info
//! commands are fully implemented.

use std::sync::Arc;

use base64::Engine;
use pixhaus_core::canvas::tools::{BrushShape, draw_stroke, flood_fill};
use pixhaus_core::canvas::{LayerInput, PixelBuffer, composite_onto};
use pixhaus_core::project::{
    CanvasState, Cel, CelData, FrameIndex, IVec2, Layer, LayerId, LayerKind, Palette,
    PixelBufferId, Rect, Rgba, SelectionRegion, SelectionState, Size, SpriteId,
};
use pixhaus_core::selection::GapCloseConfig;
use pixhaus_core::selection::SelectionMask;
use pixhaus_core::selection::algorithms::{
    Connectivity, color_range, magic_wand, magic_wand_with_gap_close, select_ellipse,
    select_polygon,
};
use pixhaus_core::transforms::{
    self, MlaaConfig, RotationAlgorithm, ScaleMode, TransformSpec, morphological_antialias,
};
use pixhaus_io::pixhaus::PixelBufferEntry;
use pixhaus_vectorize::{CenterlineConfig, VectorImage, centerline_vectorize};
use serde::{Deserialize, Serialize};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::{AppCommandError, CommandResult};
use crate::pixel_history::{PixelOp, PixelOpBatch};
use crate::state::{AppState, StrokeSession};

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
    /// Arbitrary rotation via nearest-neighbor sampling (hard edges).
    RotateNearest {
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
                algorithm: RotationAlgorithm::RotSprite,
            },
            Self::RotateBilinear { degrees } => TransformSpec::RotateArbitrary {
                degrees: *degrees,
                algorithm: RotationAlgorithm::Bilinear,
            },
            Self::RotateNearest { degrees } => TransformSpec::RotateArbitrary {
                degrees: *degrees,
                algorithm: RotationAlgorithm::NearestNeighbor,
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

/// Byte length of a binary tile frame header: six little-endian `u32`
/// fields (`sprite_id`, `frame_index`, `tile_x`, `tile_y`, `width`,
/// `height`) followed by the raw RGBA8 bytes. Kept in sync with the decoder
/// in `ui/src/canvas/Canvas.tsx`.
const TILE_FRAME_HEADER_LEN: usize = 24;

/// Geometry of one composited tile: which sprite/frame/tile it covers and
/// its pixel dimensions. Groups the fields [`send_tile`] and the binary
/// frame header share so the function stays under the argument-count lint.
struct TileSlice {
    sprite_id: u32,
    frame_index: u32,
    tile_x: u32,
    tile_y: u32,
    width: u32,
    height: u32,
}

/// Registers the renderer's binary tile channel.
///
/// The frontend creates a `Channel` and passes it here once per renderer
/// init. From then on composited tiles travel through the channel as raw
/// bytes instead of base64 `canvas:tile-dirty` events — the hot path that
/// makes drawing usable on `WebView2` (Windows). Replaces any previously
/// registered channel (e.g. after a webview reload).
#[tauri::command(async)]
pub async fn canvas_set_tile_channel(
    channel: Channel<InvokeResponseBody>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut guard = state
        .tile_channel
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = Some(channel);
    Ok(())
}

/// Packs a tile into the binary frame the renderer's channel decoder
/// expects: a six-field little-endian `u32` header (see
/// [`TILE_FRAME_HEADER_LEN`]) followed by the raw RGBA bytes. The matching
/// decoder lives in `ui/src/canvas/Canvas.tsx`.
fn encode_tile_frame(slice: &TileSlice, rgba: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(TILE_FRAME_HEADER_LEN + rgba.len());
    for field in [
        slice.sprite_id,
        slice.frame_index,
        slice.tile_x,
        slice.tile_y,
        slice.width,
        slice.height,
    ] {
        buf.extend_from_slice(&field.to_le_bytes());
    }
    buf.extend_from_slice(rgba);
    buf
}

/// Ships one composited tile to the renderer.
///
/// Fast path: when the renderer has registered a binary channel
/// (`canvas_set_tile_channel`), pack a `[u32; 6]` little-endian header plus
/// the raw RGBA bytes and send them as `InvokeResponseBody::Raw`. Tauri
/// routes payloads over 1 KiB through the webview's fetch transport, which
/// avoids base64 inflation and the slow JSON event bridge on `WebView2`.
///
/// Fallback: no channel registered yet (the first composited tiles can fire
/// before the renderer's setup command lands) — emit the legacy base64
/// `canvas:tile-dirty` event so the tile is not lost. Exactly one transport
/// runs per tile, so the renderer never double-uploads.
///
/// Both paths are fire-and-forget: a send/emit failure is logged, never
/// propagated, so a transient IPC hiccup can't fail the drawing command.
fn send_tile(app: &AppHandle, slice: &TileSlice, rgba: &[u8]) {
    let &TileSlice {
        sprite_id,
        frame_index,
        tile_x,
        tile_y,
        width,
        height,
    } = slice;
    if let Some(state) = app.try_state::<AppState>() {
        let channel = state
            .tile_channel
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if let Some(channel) = channel {
            let frame = encode_tile_frame(slice, rgba);
            if let Err(err) = channel.send(InvokeResponseBody::Raw(frame)) {
                tracing::warn!("failed to send tile ({tile_x},{tile_y}) over channel: {err}");
            }
            return;
        }
    }

    let payload = TileDirtyPayload {
        sprite_id,
        frame_index,
        tile_x,
        tile_y,
        width,
        height,
        data: base64::engine::general_purpose::STANDARD.encode(rgba),
    };
    if let Err(err) = app.emit("canvas:tile-dirty", payload) {
        tracing::warn!("failed to emit canvas:tile-dirty tile ({tile_x},{tile_y}): {err}");
    }
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

/// Ships every tile of a pixel buffer to the renderer after a drawing
/// operation. One tile per call to [`send_tile`]; each carries the extracted
/// RGBA bytes for its region. Stride-aware: reads at `buf_stride` bytes per
/// row and packs the tile data tight before handing it to the transport.
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

            send_tile(
                app,
                &TileSlice {
                    sprite_id,
                    frame_index,
                    tile_x: tx,
                    tile_y: ty,
                    width: w,
                    height: h,
                },
                &tile_bytes,
            );
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

/// Returns `Err(LayerLocked)` if the named layer or any ancestor group
/// is locked. The check matches Aseprite: a locked group blocks paint on
/// every layer beneath it, even if those layers' own `locked` is false.
///
/// Pure function over `&[Layer]` so unit tests construct the input
/// directly without spinning up an `AppState`.
fn check_layer_writable(layers: &[Layer], layer_id: LayerId) -> CommandResult<()> {
    let lookup = |id: LayerId| -> CommandResult<&Layer> {
        layers
            .iter()
            .find(|l| l.id == id)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "layer".into(),
                id: u64::from(id.get()),
            })
    };
    let layer = lookup(layer_id)?;
    if layer.locked {
        return Err(AppCommandError::LayerLocked {
            layer_id: layer_id.get(),
        });
    }
    let mut cursor = layer.parent;
    while let Some(parent_id) = cursor {
        let parent = lookup(parent_id)?;
        if parent.locked {
            return Err(AppCommandError::LayerLocked {
                layer_id: layer_id.get(),
            });
        }
        cursor = parent.parent;
    }
    Ok(())
}

/// Wrapper around [`check_layer_writable`] that resolves the sprite from
/// a [`crate::state::DocumentStore`]. Used by mutating canvas commands as
/// a one-line guard before they touch pixel buffers.
fn ensure_layer_writable(
    doc: &crate::state::DocumentStore,
    sprite_id: SpriteId,
    layer_id: LayerId,
) -> CommandResult<()> {
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    let sprite = project
        .sprite(sprite_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?;
    check_layer_writable(&sprite.layers, layer_id)
}

/// Composites every visible raster layer of `sprite_id` at `frame_index`
/// into a single canvas-sized [`PixelBuffer`]. Layers are walked
/// bottom-to-top in `sprite.layers` order, matching Aseprite's stacking.
///
/// Group rows contribute no pixels themselves; their children composite
/// normally because the iteration is flat over `sprite.layers`. Tilemap
/// and reference layers are skipped — wiring those into the composite
/// is its own follow-up (the renderer treats them via separate paths).
///
/// A raster layer with no cel for this frame contributes nothing
/// (transparent), and a cel whose buffer dimensions don't match the
/// sprite canvas is also skipped — that mismatch is a separate bug
/// surface and silently dropping it here is the conservative choice
/// while the migration to canvas-sized buffers settles.
fn composite_frame(
    doc: &crate::state::DocumentStore,
    sprite_id: SpriteId,
    frame_index: u32,
) -> CommandResult<PixelBuffer> {
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    let sprite = project
        .sprite(sprite_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?;
    let canvas_w = sprite.canvas.width;
    let canvas_h = sprite.canvas.height;
    let mut backdrop =
        PixelBuffer::new(canvas_w, canvas_h).map_err(|e| AppCommandError::Validation {
            detail: e.to_string(),
        })?;

    for layer in &sprite.layers {
        if !matches!(layer.kind, LayerKind::Raster) {
            continue;
        }
        // Skip layers that can't contribute before doing buffer work.
        // `composite_onto` short-circuits via `LayerInput::contributes`
        // but only AFTER we've cloned bytes into a temporary buffer,
        // which is the expensive part on the per-extend hot path.
        if !layer.visible || layer.opacity == 0 {
            continue;
        }
        let Some(buf_id) = find_cel_buffer(&sprite.cels, layer.id, frame_index) else {
            continue;
        };
        let Some(entry) = doc.pixel_buffers.iter().find(|e| e.id == buf_id.get()) else {
            continue;
        };
        if entry.width != canvas_w || entry.height != canvas_h {
            continue;
        }
        // `from_raw` requires owning the bytes; cloning here is one
        // canvas-sized allocation per visible layer per composite call.
        // For typical sprite sizes this is microseconds even before
        // rayon kicks in inside `composite_onto`.
        let pbuf = PixelBuffer::from_raw(
            entry.width,
            entry.height,
            entry.stride,
            entry.pixels.clone(),
        )
        .map_err(|e| AppCommandError::Validation {
            detail: e.to_string(),
        })?;
        let input = LayerInput {
            buffer: &pbuf,
            mode: layer.blend_mode,
            opacity: layer.opacity,
            visible: layer.visible,
        };
        composite_onto(&mut backdrop, &input).map_err(|e| AppCommandError::Validation {
            detail: e.to_string(),
        })?;
    }
    Ok(backdrop)
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
            .sprite(sprite_id)
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
        .sprite_mut(sprite_id)
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
///
/// The emitted tiles carry the COMPOSITED frame, not the just-mutated
/// layer's raw bytes. Without that, the renderer's flat tile cache —
/// keyed by `(sprite, frame, tx, ty)` with no layer dimension — would
/// overwrite other layers' contributions to the same tile coordinates,
/// making them disappear from the viewport on every stroke.
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

    doc.pixel_history.push(PixelOpBatch {
        label: label.to_owned(),
        ops: vec![op],
    });

    let composited = composite_frame(doc, sprite_id, frame_index)?;
    emit_buffer_tiles(
        app,
        sprite_id.get(),
        frame_index,
        composited.width(),
        composited.height(),
        composited.stride(),
        composited.as_bytes(),
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
            .sprite(sprite_id)
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

/// Recomposites the active frame and ships every tile to the renderer.
///
/// Called by the UI after layer state changes that have no pixel-mutation
/// IPC of their own — visibility toggle, opacity slider, blend-mode
/// dropdown — so the viewport reflects the new composite immediately.
/// Without this round-trip those toggles would update the layer struct
/// in Rust but leave the client's tile cache stale.
///
/// Locked toggles are deliberately excluded from the UI's call sites
/// because `locked` has no visual effect; the recomposite cost is non-
/// trivial enough to skip when the result is identical.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_recomposite_frame(
    app: AppHandle,
    sprite_id: SpriteId,
    frame_index: u32,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let doc = state.doc.read().await;
    let composited = composite_frame(&doc, sprite_id, frame_index)?;
    emit_buffer_tiles(
        &app,
        sprite_id.get(),
        frame_index,
        composited.width(),
        composited.height(),
        composited.stride(),
        composited.as_bytes(),
    );
    Ok(())
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

    ensure_layer_writable(doc, args.sprite_id, args.layer_id)?;

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

// ── Stroke sessions ──────────────────────────────────────────────────────────
//
// `canvas_draw_stroke` above is a one-shot path: full point list in, one
// undo entry out. It is right for shape tools (rect/ellipse) that build
// the points client-side and have nothing to preview during the drag.
//
// Freehand drawing wants real-time feedback. The frontend dispatches
// begin → extend × N → end so the backend re-paints the layer on every
// extend (visible immediately) but only records ONE undo entry on end
// (so Ctrl+Z reverts the whole drag, not partial strokes).
//
// Sessions live in `DocumentStore::active_strokes` keyed by `u32`
// session id. Mid-flight pixels are written straight to the buffer; the
// session retains a `Arc<Vec<u8>>` of the pre-stroke pixels so each
// extend can re-rasterize from a clean baseline cheaply. If a session
// is abandoned (browser loses focus, frontend crash) it lingers in the
// map until the next session begins on the same buffer; that next-begin
// discards the orphan without committing it. The mid-flight pixels
// remain on the buffer — no rollback — so the user keeps the visual but
// loses one undo step.
//
// The frontend promise-queues extend / end calls so they reach the
// backend strictly in order — without that, an `extend` could land
// after the `end` that meant to commit it and either drop the points
// or fail with `NotFound`.

/// Arguments for `canvas_begin_stroke`.
#[derive(Debug, Deserialize)]
pub struct BeginStrokeArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Target layer.
    pub layer_id: LayerId,
    /// Target frame (0-indexed).
    pub frame_index: u32,
    /// Stroke color. Ignored when `erase` is `true`.
    pub color: Rgba,
    /// Brush shape: `"pixel"`, `"circle"`, or `"square"`.
    #[serde(default = "default_brush_shape")]
    pub brush_shape: String,
    /// Brush diameter in canvas pixels.
    #[serde(default = "default_brush_size")]
    pub brush_size: u32,
    /// Enable pixel-perfect post-pass.
    #[serde(default)]
    pub pixel_perfect: bool,
    /// Erase mode: paint transparent instead of `color`.
    #[serde(default)]
    pub erase: bool,
    /// Optional first point. Saves a round-trip when the begin and
    /// first extend would otherwise carry the same single coordinate.
    #[serde(default)]
    pub first_point: Option<[f32; 2]>,
}

/// Arguments for `canvas_extend_stroke`.
#[derive(Debug, Deserialize)]
pub struct ExtendStrokeArgs {
    /// Session id returned by `canvas_begin_stroke`.
    pub session_id: u32,
    /// Points appended since the previous extend (or since begin).
    pub new_points: Vec<[f32; 2]>,
}

/// Arguments for `canvas_end_stroke`.
#[derive(Debug, Deserialize)]
pub struct EndStrokeArgs {
    /// Session id returned by `canvas_begin_stroke`.
    pub session_id: u32,
    /// Final point chunk to apply before the commit.
    #[serde(default)]
    pub new_points: Vec<[f32; 2]>,
}

/// Pure rasterize: produces the post-stroke pixels without mutating
/// any shared state or emitting events. Extracted so tests can assert
/// the brush math without standing up a Tauri runtime.
fn rasterize_session_pixels(session: &StrokeSession) -> CommandResult<Vec<u8>> {
    let mut pbuf = PixelBuffer::from_raw(
        session.buf_width,
        session.buf_height,
        session.buf_stride,
        (*session.initial_pixels).clone(),
    )
    .map_err(|e| AppCommandError::Validation {
        detail: e.to_string(),
    })?;
    let color = if session.erase {
        Rgba::transparent()
    } else {
        session.color
    };
    let shape = parse_brush_shape(&session.brush_shape);
    draw_stroke(
        &mut pbuf,
        &session.points,
        color,
        shape,
        session.brush_size,
        session.pixel_perfect,
    );
    Ok(pbuf.as_bytes().to_vec())
}

/// Pixel coords above 2^23 round-trip imprecisely through f32 anyway,
/// and any real canvas is much smaller — clamp to a safely-representable
/// ±16M window before casting.
const F32_INT_LIMIT: f32 = 8_388_608.0; // 2^23

/// Range of tiles `tx_min..=tx_max, ty_min..=ty_max` that cover the
/// pixel-space rectangle painted by `points` with a brush of diameter
/// `brush_size`. Returns `None` when the rect doesn't intersect the
/// buffer at all (e.g. all points are off-canvas, or the points slice
/// is empty, or every coord is non-finite).
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::similar_names,
    reason = "intermediate i64 math then clamping to in-range u32; values stay in [0, buf-1]"
)]
fn dirty_tile_range(
    points: &[[f32; 2]],
    brush_size: u32,
    buf_w: u32,
    buf_h: u32,
) -> Option<(u32, u32, u32, u32)> {
    if points.is_empty() || buf_w == 0 || buf_h == 0 {
        return None;
    }
    // Brush extends `radius` pixels in each direction from a point. A
    // single-pixel brush has radius 0 (one pixel painted at the point).
    let radius = i64::from(brush_size.saturating_sub(1) / 2);
    let buf_w_i = i64::from(buf_w);
    let buf_h_i = i64::from(buf_h);

    let mut min_x = i64::MAX;
    let mut min_y = i64::MAX;
    let mut max_x = i64::MIN;
    let mut max_y = i64::MIN;
    for p in points {
        let xf = p[0].round();
        let yf = p[1].round();
        if !xf.is_finite() || !yf.is_finite() {
            continue;
        }
        let xi = xf.clamp(-F32_INT_LIMIT, F32_INT_LIMIT) as i64;
        let yi = yf.clamp(-F32_INT_LIMIT, F32_INT_LIMIT) as i64;
        min_x = min_x.min(xi.saturating_sub(radius));
        min_y = min_y.min(yi.saturating_sub(radius));
        max_x = max_x.max(xi.saturating_add(radius));
        max_y = max_y.max(yi.saturating_add(radius));
    }
    if max_x == i64::MIN {
        // Every point was non-finite.
        return None;
    }

    let min_x = min_x.max(0);
    let min_y = min_y.max(0);
    let max_x = max_x.min(buf_w_i - 1);
    let max_y = max_y.min(buf_h_i - 1);
    if min_x > max_x || min_y > max_y {
        return None;
    }

    let tx_min = (min_x as u32) / TILE_SIZE;
    let ty_min = (min_y as u32) / TILE_SIZE;
    let tx_max = (max_x as u32) / TILE_SIZE;
    let ty_max = (max_y as u32) / TILE_SIZE;
    Some((tx_min, tx_max, ty_min, ty_max))
}

/// Ships the tiles that cover the painted region of `new_points` (plus
/// brush radius) to the renderer via [`send_tile`]. Other tiles are
/// untouched by this extend and the renderer's tile cache is still
/// valid for them.
#[allow(
    clippy::similar_names,
    reason = "tx_min/tx_max/ty_min/ty_max convey tile-rect bounds clearly"
)]
fn emit_dirty_tiles_for_points(
    app: &AppHandle,
    session: &StrokeSession,
    new_points: &[[f32; 2]],
    composited: &PixelBuffer,
) {
    // Tile slicing pulls dimensions from the composited buffer, not the
    // session. The two have always matched in practice (both are
    // `canvas_w * 4`) but coupling the session's stride to a buffer
    // we're slicing is a latent corruption hazard if the cel buffer
    // ever ends up with a padded stride (.psd / .aseprite imports,
    // SIMD-aligned scratch buffers). Use the actual buffer's layout.
    let buf_w = composited.width();
    let buf_h = composited.height();
    let buf_stride = composited.stride() as usize;
    let pixels = composited.as_bytes();

    let Some((tx_min, tx_max, ty_min, ty_max)) =
        dirty_tile_range(new_points, session.brush_size, buf_w, buf_h)
    else {
        return;
    };
    for ty in ty_min..=ty_max {
        for tx in tx_min..=tx_max {
            let x0 = tx * TILE_SIZE;
            let y0 = ty * TILE_SIZE;
            let w = (buf_w - x0).min(TILE_SIZE);
            let h = (buf_h - y0).min(TILE_SIZE);
            let mut tile_bytes = vec![0u8; (w * h * 4) as usize];
            for row in 0..h {
                let src = ((y0 + row) as usize * buf_stride + x0 as usize * 4).min(pixels.len());
                let dst = row as usize * w as usize * 4;
                let copy_len = (w as usize * 4).min(pixels.len().saturating_sub(src));
                tile_bytes[dst..dst + copy_len].copy_from_slice(&pixels[src..src + copy_len]);
            }
            send_tile(
                app,
                &TileSlice {
                    sprite_id: session.sprite_id.get(),
                    frame_index: session.frame_index,
                    tile_x: tx,
                    tile_y: ty,
                    width: w,
                    height: h,
                },
                &tile_bytes,
            );
        }
    }
}

/// Re-rasterizes a session into its target buffer and emits tile-dirty
/// events for the tiles intersecting `new_points`. Does NOT record undo.
/// Returns the post-paint pixels so callers can use them for subsequent
/// operations (e.g. `canvas_end_stroke` capturing the after-state for
/// the undo entry).
///
/// The emitted tiles carry the COMPOSITED frame across all visible
/// layers, not just the active layer. See `commit_pixel_op` for why.
fn rasterize_session_and_emit(
    doc: &mut crate::state::DocumentStore,
    app: &AppHandle,
    session: &StrokeSession,
    new_points: &[[f32; 2]],
) -> CommandResult<Vec<u8>> {
    let pixels = rasterize_session_pixels(session)?;
    let entry = doc
        .pixel_buffers
        .iter_mut()
        .find(|e| e.id == session.buffer_id.get())
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "pixel buffer".into(),
            id: u64::from(session.buffer_id.get()),
        })?;
    entry.pixels.clone_from(&pixels);

    let composited = composite_frame(doc, session.sprite_id, session.frame_index)?;
    emit_dirty_tiles_for_points(app, session, new_points, &composited);
    Ok(pixels)
}

/// Begins a freehand stroke session on a layer cel.
///
/// Lazily creates a transparent cel + buffer if none exists for
/// `(layer, frame)`. Captures the buffer's current pixels as the stroke's
/// before-state and returns a session id the frontend uses for subsequent
/// `canvas_extend_stroke` and `canvas_end_stroke` calls.
///
/// If a session is already active for the same buffer (the previous
/// session was abandoned without an end call) it is dropped without
/// commit. The mid-flight pixels of the orphan stay on the buffer — no
/// rollback — so the user keeps the visual but loses that one undo step.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_begin_stroke(
    app: AppHandle,
    args: BeginStrokeArgs,
    state: State<'_, AppState>,
) -> CommandResult<u32> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;

    ensure_layer_writable(doc, args.sprite_id, args.layer_id)?;

    let buffer_id = ensure_raster_buffer(doc, args.sprite_id, args.layer_id, args.frame_index)?;

    // Drop any orphan session on this buffer.
    doc.active_strokes
        .retain(|_, s| s.buffer_id.get() != buffer_id.get());

    let (initial_pixels, buf_width, buf_height, buf_stride) = {
        let entry = doc
            .pixel_buffers
            .iter()
            .find(|e| e.id == buffer_id.get())
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "pixel buffer".into(),
                id: u64::from(buffer_id.get()),
            })?;
        (
            Arc::new(entry.pixels.clone()),
            entry.width,
            entry.height,
            entry.stride,
        )
    };

    let session_id = doc.next_session_id;
    doc.next_session_id += 1;

    let label = if args.erase { "eraser" } else { "stroke" }.to_owned();

    let mut session = StrokeSession {
        sprite_id: args.sprite_id,
        layer_id: args.layer_id,
        frame_index: args.frame_index,
        buffer_id,
        buf_width,
        buf_height,
        buf_stride,
        initial_pixels,
        points: Vec::new(),
        color: args.color,
        brush_shape: args.brush_shape,
        brush_size: args.brush_size,
        pixel_perfect: args.pixel_perfect,
        erase: args.erase,
        label,
    };
    if let Some(p) = args.first_point {
        session.points.push(p);
    }

    // Paint the first point immediately if one was provided so the user
    // sees the click anchor before the first mousemove. Clone is O(1)
    // because `initial_pixels` is `Arc`.
    if !session.points.is_empty() {
        let snapshot = session.clone();
        let new_points = snapshot.points.clone();
        rasterize_session_and_emit(doc, &app, &snapshot, &new_points)?;
    }
    doc.active_strokes.insert(session_id, session);

    Ok(session_id)
}

/// Extends an in-flight stroke with new points and re-paints.
///
/// Re-rasterizes the buffer from the session's pre-stroke pixels with
/// the cumulative point list, writes the result, and emits
/// `canvas:tile-dirty`. Does not push to `pixel_history` — that happens
/// once on `canvas_end_stroke`.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_extend_stroke(
    app: AppHandle,
    args: ExtendStrokeArgs,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;

    // Resolve sprite/layer from the session before re-checking the lock.
    // The session was lock-checked at begin, but a separate IPC caller (a
    // plugin or script) could land an extend without a begin, and a UI
    // toggle could in principle land between begin and extend.
    let (sprite_id, layer_id) = {
        let session =
            doc.active_strokes
                .get(&args.session_id)
                .ok_or_else(|| AppCommandError::NotFound {
                    entity: "stroke session".into(),
                    id: u64::from(args.session_id),
                })?;
        (session.sprite_id, session.layer_id)
    };
    ensure_layer_writable(doc, sprite_id, layer_id)?;

    let snapshot = {
        let session = doc
            .active_strokes
            .get_mut(&args.session_id)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "stroke session".into(),
                id: u64::from(args.session_id),
            })?;
        session.points.extend_from_slice(&args.new_points);
        session.clone()
    };
    rasterize_session_and_emit(doc, &app, &snapshot, &args.new_points)?;
    Ok(())
}

/// Commits an in-flight stroke as one undo entry.
///
/// Optionally accepts a final `new_points` chunk (the frontend uses it
/// to flush points collected after the last RAF tick before mouseup).
/// Re-paints once more so the persisted pixels match what the user
/// actually saw, pushes a single `PixelOpBatch` covering
/// `initial_pixels → final pixels`, and removes the session.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_end_stroke(
    app: AppHandle,
    args: EndStrokeArgs,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;

    let (sprite_id, layer_id) = {
        let session =
            doc.active_strokes
                .get(&args.session_id)
                .ok_or_else(|| AppCommandError::NotFound {
                    entity: "stroke session".into(),
                    id: u64::from(args.session_id),
                })?;
        (session.sprite_id, session.layer_id)
    };
    ensure_layer_writable(doc, sprite_id, layer_id)?;

    // Validate up front so we never start re-rasterizing for an unknown
    // id and have to roll back. The clone here is O(1) because
    // `initial_pixels` is `Arc`.
    let snapshot = {
        let session = doc
            .active_strokes
            .get_mut(&args.session_id)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "stroke session".into(),
                id: u64::from(args.session_id),
            })?;
        session.points.extend_from_slice(&args.new_points);
        session.clone()
    };
    let after_pixels = rasterize_session_and_emit(doc, &app, &snapshot, &args.new_points)?;

    // Move the session out so we own its initial_pixels for the undo
    // entry. Existence is guaranteed by the validation above (no
    // `await` between the two).
    let session =
        doc.active_strokes
            .remove(&args.session_id)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "stroke session".into(),
                id: u64::from(args.session_id),
            })?;

    // The `Arc` started life inside this session and isn't shared with
    // anything else by end-time (snapshot was discarded above); try to
    // unwrap to avoid one final clone, fall back if for some reason the
    // refcount is still > 1.
    let before_pixels =
        Arc::try_unwrap(session.initial_pixels).unwrap_or_else(|arc| (*arc).clone());

    let op = PixelOp {
        buffer_id: session.buffer_id.get(),
        buf_width: session.buf_width,
        buf_height: session.buf_height,
        buf_stride: session.buf_stride,
        sprite_id: session.sprite_id.get(),
        frame_index: session.frame_index,
        before: before_pixels,
        after: after_pixels,
    };
    doc.pixel_history.push(PixelOpBatch {
        label: session.label,
        ops: vec![op],
    });
    doc.dirty = true;
    Ok(())
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

    ensure_layer_writable(doc, args.sprite_id, args.layer_id)?;

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

/// Applies morphological anti-aliasing (MLAA) to the active layer's cel at
/// the active frame.
///
/// `threshold` is the per-channel max-diff classifier (default 16). `softness`
/// controls how aggressively separation lines are smoothed (default 128;
/// `0` is a no-op). Both default values match `OpenToonz`'s recommended
/// starting points for 8-bit RGBA content.
///
/// The result replaces the cel buffer in place; a `PixelOpBatch` is pushed
/// to the undo stack and the affected tiles re-composite for the renderer.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_apply_mlaa(
    sprite_id: SpriteId,
    layer_id: LayerId,
    threshold: Option<u8>,
    softness: Option<u8>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<()> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;

    let MlaaPrep {
        buffer_id,
        frame_index,
        before,
        src,
        config,
    } = mlaa_prep_in_doc(doc, sprite_id, layer_id, threshold, softness)?;

    // Morphological AA is a CPU-bound full-image filter; run it off the
    // async runtime thread.
    let dst = tokio::task::spawn_blocking(move || morphological_antialias(&src, &config))
        .await
        .map_err(|e| AppCommandError::Validation {
            detail: format!("mlaa task failed: {e}"),
        })?
        .map_err(|e| AppCommandError::Validation {
            detail: e.to_string(),
        })?;

    commit_pixel_op(
        doc,
        &app,
        buffer_id,
        sprite_id,
        frame_index,
        before,
        dst.as_bytes().to_vec(),
        "mlaa",
    )
}

/// The cel-side inputs MLAA needs before the (off-thread) filter runs.
struct MlaaPrep {
    buffer_id: PixelBufferId,
    frame_index: u32,
    before: Vec<u8>,
    src: PixelBuffer,
    config: MlaaConfig,
}

/// Resolves the active frame, ensures a raster buffer exists, and reads
/// the cel into a [`PixelBuffer`] plus the MLAA config. Split out from
/// the command so it can be unit-tested against a `DocumentStore`
/// without the `tauri::State`/`AppHandle` machinery.
fn mlaa_prep_in_doc(
    doc: &mut crate::state::DocumentStore,
    sprite_id: SpriteId,
    layer_id: LayerId,
    threshold: Option<u8>,
    softness: Option<u8>,
) -> CommandResult<MlaaPrep> {
    ensure_layer_writable(doc, sprite_id, layer_id)?;

    // Resolve the active frame; default to frame 0 if no canvas is set.
    let frame_index = doc
        .project
        .as_ref()
        .and_then(|p| p.canvas.active_frame)
        .map_or(0, FrameIndex::get);

    let buffer_id = ensure_raster_buffer(doc, sprite_id, layer_id, frame_index)?;

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

    let src = PixelBuffer::from_raw(w, h, stride, before.clone()).map_err(|e| {
        AppCommandError::Validation {
            detail: e.to_string(),
        }
    })?;

    let defaults = MlaaConfig::default();
    let config = MlaaConfig {
        threshold: threshold.unwrap_or(defaults.threshold),
        softness: softness.unwrap_or(defaults.softness),
    };

    Ok(MlaaPrep {
        buffer_id,
        frame_index,
        before,
        src,
        config,
    })
}

/// Vectorizes a raster layer's cel into a `VectorImage` of centerline
/// strokes. No raster mutation; the result is returned to the caller so
/// a follow-up surface (SVG export, vector preview overlay) can consume
/// it. Pixhaus is raster-only by design, so `VectorImage` has no render
/// path yet — this command exposes the pipeline for inspection while the
/// sink lands.
///
/// `palette` resolution: prefer the project's `brush.active_palette`
/// when set; otherwise fall back to the sprite's first palette. Returns
/// an error when neither is available — `centerline_vectorize` rejects
/// an empty palette.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn vector_vectorize_layer(
    sprite_id: SpriteId,
    layer_id: LayerId,
    state: State<'_, AppState>,
) -> CommandResult<VectorImage> {
    // Extract the cel buffer and palette under the read lock, then drop
    // the lock before the CPU-bound vectorization so we don't hold it
    // across the blocking work.
    let (buf, palette) = {
        let doc = state.doc.read().await;
        vectorize_inputs_from_doc(&doc, sprite_id, layer_id)?
    };

    // Centerline vectorization is CPU-bound; run it off the async runtime.
    let result = tokio::task::spawn_blocking(move || {
        centerline_vectorize(&buf, &palette, &CenterlineConfig::default())
    })
    .await
    .map_err(|e| AppCommandError::Validation {
        detail: format!("vectorize task failed: {e}"),
    })?
    .map_err(|e| AppCommandError::Validation {
        detail: e.to_string(),
    })?;

    tracing::info!(
        sprite_id = sprite_id.get(),
        layer_id = layer_id.get(),
        strokes = result.strokes.len(),
        "vector_vectorize_layer produced vector image"
    );

    Ok(result)
}

/// Resolves the active frame, reads the cel buffer, and selects the
/// palette (active palette, else the sprite's first) for vectorization.
/// Split out from the command so it can be unit-tested against a
/// `DocumentStore` without the `tauri::State` machinery.
fn vectorize_inputs_from_doc(
    doc: &crate::state::DocumentStore,
    sprite_id: SpriteId,
    layer_id: LayerId,
) -> CommandResult<(PixelBuffer, Palette)> {
    let frame_index = doc
        .project
        .as_ref()
        .and_then(|p| p.canvas.active_frame)
        .unwrap_or(FrameIndex::new(0));

    let (_cel_pos, buf) = load_cel_buffer(doc, sprite_id, layer_id, frame_index)?;

    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    let sprite = project
        .sprite(sprite_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?;
    let active_id = project.brush.active_palette;
    let palette = active_id
        .and_then(|pid| sprite.palettes.iter().find(|p| p.id == pid))
        .or_else(|| sprite.palettes.first())
        .cloned()
        .ok_or_else(|| AppCommandError::Validation {
            detail: "vectorize requires a palette with at least one color".into(),
        })?;
    Ok((buf, palette))
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

    ensure_layer_writable(&doc, args.sprite_id, args.layer_id)?;

    // ── Phase 1: read project state, then release the borrow ──────────────
    // `doc.project` and `doc.pixel_buffers` are separate fields but Rust
    // can't tell that through a shared `&mut doc` borrow, so we extract
    // everything we need from the project before touching pixel_buffers.
    let (buf_id, cel_pos, selection_region) = {
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;

        let sprite = project
            .sprite(args.sprite_id)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(args.sprite_id.get()),
            })?;
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
        (buf_id, cel.position, selection_region)
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
        let sprite =
            project
                .sprite_mut(args.sprite_id)
                .ok_or_else(|| AppCommandError::NotFound {
                    entity: "sprite".into(),
                    id: u64::from(args.sprite_id.get()),
                })?;
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

    // ── Emit tile-dirty events with the transformed pixels ─────────────────
    // The renderer keys its GPU tile cache on the cel buffer, so we ship
    // the buffer's actual bytes — extracted stride-aware by the helper.
    // Emitting all-zero payloads here is what made flips look like a reset.
    emit_buffer_tiles(
        &app,
        args.sprite_id.get(),
        args.frame_index,
        new_w,
        new_h,
        new_stride,
        &doc.pixel_buffers[entry_idx].pixels,
    );

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
            .sprite(sprite_id)
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

/// Allocates a fresh `PixelBufferId`, stores `mask` as a 1-byte-per-pixel
/// buffer, and returns a `SelectionState` whose region points at it.
///
/// Selection masks live alongside RGBA pixel buffers in `doc.pixel_buffers`
/// because they share the storage / save / undo machinery. Masks have stride
/// equal to width (no padding) and one byte per pixel — distinct from the
/// `width * 4` stride of an RGBA buffer.
///
/// Computes minimum-enclosing-rect bounds from the mask itself so the
/// returned `SelectionRegion::Mask.bounds` honours its documented
/// contract. Drops the previous selection's mask buffer (if any) before
/// pushing the new one — without this, every wand/lasso/invert leaks the
/// outgoing buffer into `pixel_buffers`, and `project_save` persists every
/// leak. Cel-referenced buffers are never touched.
fn commit_mask_selection(
    doc: &mut crate::state::DocumentStore,
    mask: &SelectionMask,
    anchor_layer: Option<LayerId>,
) -> SelectionState {
    // GC the outgoing selection mask buffer if there is one. We only drop
    // entries whose id matches the current selection's mask — never any
    // entry referenced by a cel.
    if let Some(project) = doc.project.as_ref() {
        if let Some(SelectionRegion::Mask { mask: prior_id, .. }) = &project.selection.region {
            let prior = prior_id.get();
            doc.pixel_buffers.retain(|e| e.id != prior);
        }
    }

    let id = PixelBufferId::new(doc.next_id);
    doc.next_id += 1;
    let width = mask.width();
    let height = mask.height();
    doc.pixel_buffers.push(PixelBufferEntry {
        id: id.get(),
        width,
        height,
        stride: width,
        pixels: mask.as_bytes().to_vec(),
    });
    SelectionState {
        region: Some(SelectionRegion::Mask {
            bounds: mask.bounds(),
            mask: id,
        }),
        anchor_layer,
    }
}

/// Looks up the cel for `(sprite_id, layer_id, frame_index)` and returns
/// the cel position plus a fully decoded `PixelBuffer`.
///
/// Follows `CelData::Linked` to its source frame on the same layer (with
/// a depth limit to defend against pathological cycles in malformed
/// projects). Errors when the sprite, layer, cel, or pixel buffer is
/// missing, when the resolved cel is a tilemap (selection algorithms
/// don't operate on tile data), or when a link cycle exceeds
/// `MAX_LINK_DEPTH`.
fn load_cel_buffer(
    doc: &crate::state::DocumentStore,
    sprite_id: SpriteId,
    layer_id: LayerId,
    frame_index: FrameIndex,
) -> Result<(IVec2, PixelBuffer), AppCommandError> {
    /// Defensive cap on linked-cel chasing. The data model prohibits
    /// cycles by convention but doesn't enforce it; this keeps a
    /// malformed project from spinning forever.
    const MAX_LINK_DEPTH: usize = 32;

    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    let sprite = project
        .sprite(sprite_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?;

    let mut current_frame = frame_index;
    let mut hops = 0usize;
    let (cel_pos, buf_id) = loop {
        if hops > MAX_LINK_DEPTH {
            return Err(AppCommandError::Validation {
                detail: format!(
                    "linked cel chain exceeded depth {MAX_LINK_DEPTH} on layer {}",
                    layer_id.get()
                ),
            });
        }
        let cel = sprite
            .cels
            .iter()
            .find(|c| c.layer_id == layer_id && c.frame_index == current_frame)
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "cel".into(),
                id: u64::from(current_frame.get()),
            })?;
        match &cel.data {
            CelData::Raster { buffer, .. } => break (cel.position, *buffer),
            CelData::Linked { source_frame } => {
                current_frame = *source_frame;
                hops += 1;
            }
            CelData::Tilemap { .. } => {
                return Err(AppCommandError::Unimplemented {
                    stream: "raster cels only (tilemap cels are not yet sampled)".into(),
                });
            }
        }
    };

    let entry = doc
        .pixel_buffers
        .iter()
        .find(|e| e.id == buf_id.get())
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "pixel buffer".into(),
            id: u64::from(buf_id.get()),
        })?;
    let buf = PixelBuffer::from_raw(
        entry.width,
        entry.height,
        entry.stride,
        entry.pixels.clone(),
    )
    .map_err(|e| AppCommandError::Validation {
        detail: e.to_string(),
    })?;
    Ok((cel_pos, buf))
}

/// Returns the canvas dimensions for `sprite_id`.
fn sprite_canvas_size(
    doc: &crate::state::DocumentStore,
    sprite_id: SpriteId,
) -> Result<(u32, u32), AppCommandError> {
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    let sprite = project
        .sprite(sprite_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?;
    Ok((sprite.canvas.width, sprite.canvas.height))
}

/// Inverts the current selection.
///
/// A `Rect` selection is upgraded to a `Mask` (the inverse of an
/// axis-aligned rectangle is not a rectangle). An empty selection inverts
/// to "everything" — a full-canvas rectangle. A full-canvas rectangle
/// inverts to "nothing" (`region = None`).
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_invert_selection(
    sprite_id: SpriteId,
    anchor_layer: Option<LayerId>,
    state: State<'_, AppState>,
) -> CommandResult<SelectionState> {
    let mut doc = state.doc.write().await;
    let (canvas_w, canvas_h) = sprite_canvas_size(&doc, sprite_id)?;
    let canvas_rect = Rect::from_xywh(0, 0, canvas_w, canvas_h);

    let current_region = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?
        .selection
        .region
        .clone();

    // Reconstruct the current mask in canvas coordinates so we can
    // invert it. None → empty mask; Rect → fill the rect; Mask → load
    // the bytes (mask buffers are stored canvas-sized — see
    // `commit_mask_selection`).
    let current_mask = match current_region {
        None => {
            SelectionMask::new(canvas_w, canvas_h).map_err(|e| AppCommandError::Validation {
                detail: e.to_string(),
            })?
        }
        Some(SelectionRegion::Rect { bounds }) => pixhaus_core::selection::algorithms::select_rect(
            canvas_w, canvas_h, bounds,
        )
        .map_err(|e| AppCommandError::Validation {
            detail: e.to_string(),
        })?,
        Some(SelectionRegion::Mask { mask: mask_id, .. }) => {
            let entry = doc
                .pixel_buffers
                .iter()
                .find(|e| e.id == mask_id.get())
                .ok_or_else(|| AppCommandError::NotFound {
                    entity: "pixel buffer".into(),
                    id: u64::from(mask_id.get()),
                })?;
            SelectionMask::from_raw(entry.width, entry.height, entry.pixels.clone()).map_err(
                |e| AppCommandError::Validation {
                    detail: e.to_string(),
                },
            )?
        }
    };

    let inverted = current_mask.invert();
    let selected = inverted.selected_count();
    let new_state = if selected == 0 {
        // Inverted selection covers nothing — clear it.
        SelectionState {
            region: None,
            anchor_layer,
        }
    } else if selected == canvas_w.saturating_mul(canvas_h) {
        // Inverted selection covers everything — represent as a rect.
        SelectionState {
            region: Some(SelectionRegion::Rect {
                bounds: canvas_rect,
            }),
            anchor_layer,
        }
    } else {
        commit_mask_selection(&mut doc, &inverted, anchor_layer)
    };

    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.selection = new_state.clone();
    doc.dirty = true;
    Ok(new_state)
}

/// Gap-closing pre-pass tuning. Every field is optional; absent fields
/// fall back to [`GapCloseConfig::default`].
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct GapCloseRequest {
    /// Maximum gap (in pixels) the closer will try to bridge. Defaults
    /// to 10 when omitted.
    #[serde(default)]
    pub closing_distance: Option<u32>,
    /// Maximum angle (radians) between an endpoint's connecting direction
    /// and the displacement to its partner. Defaults to `FRAC_PI_2`
    /// (~1.5708) when omitted.
    #[serde(default)]
    pub closing_angle_rad: Option<f32>,
    /// Luma threshold below which a pixel counts as ink. Defaults to 128
    /// when omitted.
    #[serde(default)]
    pub ink_threshold: Option<u8>,
}

impl GapCloseRequest {
    /// Resolves the request against [`GapCloseConfig::default`].
    fn resolve(&self) -> GapCloseConfig {
        let defaults = GapCloseConfig::default();
        GapCloseConfig {
            closing_distance: self.closing_distance.unwrap_or(defaults.closing_distance),
            closing_angle_rad: self.closing_angle_rad.unwrap_or(defaults.closing_angle_rad),
            ink_threshold: self.ink_threshold.unwrap_or(defaults.ink_threshold),
        }
    }
}

/// Selects a contiguous region via flood-fill from a seed pixel.
///
/// `(seed_x, seed_y)` are canvas-space coordinates. The flood-fill runs
/// against the anchor layer's pixel buffer at the active frame; the
/// resulting mask covers the full canvas (zeros outside the cel) so it can
/// compose with subsequent selection ops.
///
/// When `gap_close` is `Some`, the core runs a gap-closing pre-pass before
/// the flood-fill, stamping bridge pixels into a working copy of the cel
/// buffer so the flood respects almost-closed outlines.
// Tauri commands take their arguments as a flat list because that's the
// IPC contract; collapsing them into a struct would change the JS-side
// payload shape. Eight scalars is the right surface here.
#[allow(clippy::too_many_arguments)]
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_select_magic_wand(
    sprite_id: SpriteId,
    anchor_layer: Option<LayerId>,
    seed_x: i32,
    seed_y: i32,
    tolerance: u8,
    connectivity: String,
    gap_close: Option<GapCloseRequest>,
    state: State<'_, AppState>,
) -> CommandResult<SelectionState> {
    let layer_id = anchor_layer.ok_or_else(|| AppCommandError::Validation {
        detail: "magic wand requires an anchor layer".into(),
    })?;
    let mode = match connectivity.as_str() {
        "four" | "4" => Connectivity::Four,
        "eight" | "8" => Connectivity::Eight,
        other => {
            return Err(AppCommandError::Validation {
                detail: format!("unknown connectivity {other:?}; expected \"four\" or \"eight\""),
            });
        }
    };

    let mut doc = state.doc.write().await;
    let (canvas_w, canvas_h) = sprite_canvas_size(&doc, sprite_id)?;
    let frame_index = doc
        .project
        .as_ref()
        .and_then(|p| p.canvas.active_frame)
        .unwrap_or(FrameIndex::new(0));

    let (cel_pos, buf) = load_cel_buffer(&doc, sprite_id, layer_id, frame_index)?;

    // Translate canvas-space seed to cel-local coords. `i64` math keeps
    // the bounds check unambiguous regardless of cel position sign.
    let local_xi = i64::from(seed_x) - i64::from(cel_pos.x);
    let local_yj = i64::from(seed_y) - i64::from(cel_pos.y);
    let in_bounds = local_xi >= 0
        && local_yj >= 0
        && local_xi < i64::from(buf.width())
        && local_yj < i64::from(buf.height());
    if !in_bounds {
        // Seed is outside the cel — return an empty selection.
        let new_state = SelectionState {
            region: None,
            anchor_layer: Some(layer_id),
        };
        let project = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?;
        project.selection = new_state.clone();
        doc.dirty = true;
        return Ok(new_state);
    }
    let local_x = u32::try_from(local_xi).unwrap_or(0);
    let local_y = u32::try_from(local_yj).unwrap_or(0);
    let cel_mask = if let Some(req) = gap_close.as_ref() {
        let cfg = req.resolve();
        // `local_xi` and `local_yj` were bounds-checked above against
        // buf.width()/height(); both are non-negative and fit in i32 as
        // long as the buffer dims do. Surface a Validation error
        // otherwise rather than panic.
        let bad_seed = || AppCommandError::Validation {
            detail: "seed out of i32 range".into(),
        };
        let seed = IVec2 {
            x: i32::try_from(local_xi).map_err(|_| bad_seed())?,
            y: i32::try_from(local_yj).map_err(|_| bad_seed())?,
        };
        magic_wand_with_gap_close(&buf, seed, tolerance, mode, Some(cfg)).map_err(|e| {
            AppCommandError::Validation {
                detail: e.to_string(),
            }
        })?
    } else {
        magic_wand(&buf, local_x, local_y, tolerance, mode).map_err(|e| {
            AppCommandError::Validation {
                detail: e.to_string(),
            }
        })?
    };

    // Lift the cel-sized mask onto a canvas-sized one at cel_pos.
    let canvas_mask = lift_mask_to_canvas(&cel_mask, cel_pos, canvas_w, canvas_h)?;
    let new_state = commit_mask_selection(&mut doc, &canvas_mask, Some(layer_id));

    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.selection = new_state.clone();
    doc.dirty = true;
    Ok(new_state)
}

/// Selects all pixels within a given color tolerance of the target color.
///
/// Operates on the anchor layer's pixel buffer at the active frame; the
/// produced mask is canvas-sized.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_select_color_range(
    sprite_id: SpriteId,
    anchor_layer: Option<LayerId>,
    _x: i32,
    _y: i32,
    target_color: Rgba,
    tolerance: u8,
    state: State<'_, AppState>,
) -> CommandResult<SelectionState> {
    let layer_id = anchor_layer.ok_or_else(|| AppCommandError::Validation {
        detail: "color range requires an anchor layer".into(),
    })?;

    let mut doc = state.doc.write().await;
    let (canvas_w, canvas_h) = sprite_canvas_size(&doc, sprite_id)?;
    let frame_index = doc
        .project
        .as_ref()
        .and_then(|p| p.canvas.active_frame)
        .unwrap_or(FrameIndex::new(0));

    let (cel_pos, buf) = load_cel_buffer(&doc, sprite_id, layer_id, frame_index)?;
    let cel_mask =
        color_range(&buf, target_color, tolerance).map_err(|e| AppCommandError::Validation {
            detail: e.to_string(),
        })?;

    let canvas_mask = lift_mask_to_canvas(&cel_mask, cel_pos, canvas_w, canvas_h)?;
    let new_state = commit_mask_selection(&mut doc, &canvas_mask, Some(layer_id));

    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.selection = new_state.clone();
    doc.dirty = true;
    Ok(new_state)
}

/// Selects the polygon defined by the given canvas-space points.
///
/// The polygon is auto-closed; an empty or single-point input produces an
/// empty selection. The output mask covers the whole canvas.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_select_lasso(
    sprite_id: SpriteId,
    anchor_layer: Option<LayerId>,
    points: Vec<IVec2>,
    state: State<'_, AppState>,
) -> CommandResult<SelectionState> {
    let mut doc = state.doc.write().await;
    let (canvas_w, canvas_h) = sprite_canvas_size(&doc, sprite_id)?;
    let mask =
        select_polygon(canvas_w, canvas_h, &points).map_err(|e| AppCommandError::Validation {
            detail: e.to_string(),
        })?;
    let new_state = if mask.selected_count() == 0 {
        SelectionState {
            region: None,
            anchor_layer,
        }
    } else {
        commit_mask_selection(&mut doc, &mask, anchor_layer)
    };

    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.selection = new_state.clone();
    doc.dirty = true;
    Ok(new_state)
}

/// Selects the ellipse inscribed in the given canvas-space `bounds`.
///
/// Unlike the rectangle marquee, an ellipse is not axis-aligned-rectangular,
/// so it commits as a `Mask` region (the same shape wand / lasso / color-range
/// produce). An empty or degenerate ellipse clears the selection.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_select_ellipse(
    sprite_id: SpriteId,
    anchor_layer: Option<LayerId>,
    bounds: Rect,
    state: State<'_, AppState>,
) -> CommandResult<SelectionState> {
    let mut doc = state.doc.write().await;
    let (canvas_w, canvas_h) = sprite_canvas_size(&doc, sprite_id)?;
    let mask =
        select_ellipse(canvas_w, canvas_h, bounds).map_err(|e| AppCommandError::Validation {
            detail: e.to_string(),
        })?;
    let new_state = if mask.selected_count() == 0 {
        SelectionState {
            region: None,
            anchor_layer,
        }
    } else {
        commit_mask_selection(&mut doc, &mask, anchor_layer)
    };

    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.selection = new_state.clone();
    doc.dirty = true;
    Ok(new_state)
}

/// The current selection mask, cropped to its bounding box, for the renderer.
///
/// The marching-ants pass needs the actual per-pixel mask to trace a
/// non-rectangular outline. Mask buffers are stored canvas-sized; cropping to
/// `bounds` keeps the payload that crosses the IPC bridge proportional to the
/// selection rather than the whole canvas.
#[derive(Debug, Serialize)]
pub struct SelectionMaskData {
    /// Left edge of the cropped region in canvas coordinates.
    pub x: i32,
    /// Top edge of the cropped region in canvas coordinates.
    pub y: i32,
    /// Width of the cropped region in pixels.
    pub width: u32,
    /// Height of the cropped region in pixels.
    pub height: u32,
    /// Row-major inclusion bytes (`0` = out, `255` = in), `width * height` long.
    pub data: Vec<u8>,
}

/// Crops a canvas-sized mask buffer (`stride` bytes per row, one byte per
/// pixel) to `bounds`, clamped to the buffer. Returns the cropped region as a
/// tightly packed (stride == width) byte run.
fn crop_mask_region(
    width: u32,
    height: u32,
    stride: u32,
    pixels: &[u8],
    bounds: Rect,
) -> Result<SelectionMaskData, AppCommandError> {
    let bx = u32::try_from(bounds.origin.x.max(0))
        .unwrap_or(0)
        .min(width);
    let by = u32::try_from(bounds.origin.y.max(0))
        .unwrap_or(0)
        .min(height);
    let bw = bounds.size.width.min(width - bx);
    let bh = bounds.size.height.min(height - by);

    let mut data = Vec::with_capacity((bw as usize) * (bh as usize));
    for row in 0..bh {
        let src_y = (by + row) as usize;
        let start = src_y * (stride as usize) + (bx as usize);
        let end = start + (bw as usize);
        let chunk = pixels
            .get(start..end)
            .ok_or_else(|| AppCommandError::Validation {
                detail: "mask buffer smaller than its declared bounds".into(),
            })?;
        data.extend_from_slice(chunk);
    }

    Ok(SelectionMaskData {
        x: i32::try_from(bx).unwrap_or(i32::MAX),
        y: i32::try_from(by).unwrap_or(i32::MAX),
        width: bw,
        height: bh,
        data,
    })
}

/// Resolves the mask payload for a selection region against the document's
/// pixel buffers. `Mask` regions crop to their bounds; `Rect`, empty, and
/// `None` regions need no payload (they render from their bounds alone).
fn selection_mask_payload(
    region: Option<&SelectionRegion>,
    buffers: &[PixelBufferEntry],
) -> Result<Option<SelectionMaskData>, AppCommandError> {
    let (bounds, mask_id) = match region {
        Some(SelectionRegion::Mask { bounds, mask }) => (*bounds, *mask),
        _ => return Ok(None),
    };

    let entry = buffers
        .iter()
        .find(|e| e.id == mask_id.get())
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "pixel buffer".into(),
            id: u64::from(mask_id.get()),
        })?;

    let cropped = crop_mask_region(
        entry.width,
        entry.height,
        entry.stride,
        &entry.pixels,
        bounds,
    )?;
    Ok(Some(cropped))
}

/// Returns the current selection mask cropped to its bounds, or `None` when
/// the selection is empty or rectangular (rect selections render from their
/// bounds alone, so they need no mask payload).
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_get_selection_mask(
    state: State<'_, AppState>,
) -> CommandResult<Option<SelectionMaskData>> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;

    selection_mask_payload(project.selection.region.as_ref(), &doc.pixel_buffers)
}

/// Lifts a cel-local mask onto a canvas-sized mask, placing it at `cel_pos`.
///
/// Pixels outside the cel are zero (unselected). The output dimensions are
/// `(canvas_w, canvas_h)`.
fn lift_mask_to_canvas(
    cel_mask: &SelectionMask,
    cel_pos: IVec2,
    canvas_w: u32,
    canvas_h: u32,
) -> Result<SelectionMask, AppCommandError> {
    let mut out =
        SelectionMask::new(canvas_w, canvas_h).map_err(|e| AppCommandError::Validation {
            detail: e.to_string(),
        })?;
    for cy in 0..cel_mask.height() {
        for cx in 0..cel_mask.width() {
            let v = cel_mask.get(cx, cy).unwrap_or(0);
            if v == 0 {
                continue;
            }
            let canvas_x = i64::from(cel_pos.x) + i64::from(cx);
            let canvas_y = i64::from(cel_pos.y) + i64::from(cy);
            if canvas_x < 0
                || canvas_y < 0
                || canvas_x >= i64::from(canvas_w)
                || canvas_y >= i64::from(canvas_h)
            {
                continue;
            }
            let dst_x = u32::try_from(canvas_x).unwrap_or(0);
            let dst_y = u32::try_from(canvas_y).unwrap_or(0);
            out.set(dst_x, dst_y, v);
        }
    }
    Ok(out)
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
    use pixhaus_core::project::{
        ActiveTarget, AiMetadata, Entity, EntityContent, EntityDefaults, EntityId, EntityKind,
        NamedSprite, Project, Size, Sprite, StateId, UserData,
    };

    /// Wraps `sprite` into `project.library` as a fresh `Custom`-kind
    /// entity with one primary state. Mirrors the production helper in
    /// `sprite_add`; lives here so tests don't need to walk the same
    /// id-counter ceremony.
    fn install_sprite(project: &mut Project, sprite: Sprite) {
        let entity_id = EntityId::new(1);
        let state_id = StateId::new(1);
        let name = sprite.name.clone();
        project.library.entities.push(Entity {
            id: entity_id,
            kind: EntityKind::Custom("Sprite".into()),
            name,
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
        project.active = ActiveTarget::State {
            entity_id,
            state_id,
        };
    }

    #[test]
    fn lift_mask_places_at_offset_and_clips() {
        // A 4x4 cel-local mask, fully selected, placed at (2, 1) on a
        // 6x4 canvas. The lifted mask should have selected pixels in
        // x ∈ [2, 6) and y ∈ [1, 4) — i.e. the 4x3 intersection of the
        // cel rect with the canvas rect.
        let mut cel_mask = SelectionMask::full(4, 4).unwrap();
        // Knock one pixel out so the test catches a zero-fill bug too.
        cel_mask.set(0, 0, 0);
        let lifted = lift_mask_to_canvas(&cel_mask, IVec2 { x: 2, y: 1 }, 6, 4).unwrap();
        // Pixel (2, 1) is the cel's (0, 0), which we cleared.
        assert!(!lifted.is_selected(2, 1));
        // Pixel (3, 1) is the cel's (1, 0), which is selected.
        assert!(lifted.is_selected(3, 1));
        // Pixel (5, 3) is the cel's (3, 2), inside both rects.
        assert!(lifted.is_selected(5, 3));
        // The cel's (3, 3) lifts to canvas (5, 4) — outside the canvas.
        // No assertion needed, just shouldn't panic.
        // Pixel (0, 0) is outside the cel's rect; should remain unselected.
        assert!(!lifted.is_selected(0, 0));
    }

    #[test]
    fn crop_mask_region_extracts_bounds() {
        // 4x4 canvas mask; select a 2x2 block at (1, 1).
        let mut pixels = vec![0u8; 16];
        for (y, x) in [(1u32, 1u32), (1, 2), (2, 1), (2, 2)] {
            pixels[(y * 4 + x) as usize] = 255;
        }
        let bounds = Rect::from_xywh(1, 1, 2, 2);
        let cropped = crop_mask_region(4, 4, 4, &pixels, bounds).unwrap();
        assert_eq!((cropped.x, cropped.y), (1, 1));
        assert_eq!((cropped.width, cropped.height), (2, 2));
        assert_eq!(cropped.data, vec![255, 255, 255, 255]);
    }

    #[test]
    fn crop_mask_region_clamps_to_buffer() {
        // Bounds that overhang the buffer get clamped instead of panicking.
        let pixels = vec![255u8; 9]; // 3x3 fully selected
        let bounds = Rect::from_xywh(2, 2, 10, 10);
        let cropped = crop_mask_region(3, 3, 3, &pixels, bounds).unwrap();
        assert_eq!((cropped.x, cropped.y), (2, 2));
        assert_eq!((cropped.width, cropped.height), (1, 1));
        assert_eq!(cropped.data, vec![255]);
    }

    #[test]
    fn selection_mask_payload_none_for_rect_and_empty() {
        // Rect and empty selections carry no mask payload.
        let rect = SelectionRegion::Rect {
            bounds: Rect::from_xywh(0, 0, 4, 4),
        };
        assert!(selection_mask_payload(Some(&rect), &[]).unwrap().is_none());
        assert!(selection_mask_payload(None, &[]).unwrap().is_none());
    }

    #[test]
    fn selection_mask_payload_crops_mask_region() {
        // A 4x4 canvas mask with a 2x2 block selected at (1, 1), bounds tight.
        let mut pixels = vec![0u8; 16];
        for (y, x) in [(1u32, 1u32), (1, 2), (2, 1), (2, 2)] {
            pixels[(y * 4 + x) as usize] = 255;
        }
        let buffers = vec![PixelBufferEntry {
            id: 7,
            width: 4,
            height: 4,
            stride: 4,
            pixels,
        }];
        let region = SelectionRegion::Mask {
            bounds: Rect::from_xywh(1, 1, 2, 2),
            mask: PixelBufferId::new(7),
        };
        let payload = selection_mask_payload(Some(&region), &buffers)
            .unwrap()
            .expect("mask region yields a payload");
        assert_eq!((payload.x, payload.y), (1, 1));
        assert_eq!((payload.width, payload.height), (2, 2));
        assert_eq!(payload.data, vec![255, 255, 255, 255]);
    }

    #[test]
    fn selection_mask_payload_errors_on_missing_buffer() {
        // A mask region pointing at an absent buffer is a not-found error.
        let region = SelectionRegion::Mask {
            bounds: Rect::from_xywh(0, 0, 2, 2),
            mask: PixelBufferId::new(99),
        };
        assert!(selection_mask_payload(Some(&region), &[]).is_err());
    }

    #[test]
    fn select_ellipse_empty_for_degenerate_bounds() {
        // canvas_select_ellipse clears the selection when the inscribed ellipse
        // covers no pixels; a zero-size rect is the degenerate case.
        let mask = select_ellipse(16, 16, Rect::from_xywh(5, 5, 0, 0)).unwrap();
        assert_eq!(mask.selected_count(), 0);
    }

    #[test]
    fn select_ellipse_selects_interior_for_normal_bounds() {
        // A normal ellipse selects pixels, so the command commits a mask region.
        let mask = select_ellipse(16, 16, Rect::from_xywh(2, 2, 10, 10)).unwrap();
        assert!(mask.selected_count() > 0);
    }

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
    fn encode_tile_frame_header_then_payload() {
        // Header is six little-endian u32s in field order, then raw RGBA.
        // The decoder in ui/src/canvas/Canvas.tsx reads the same layout.
        let slice = TileSlice {
            sprite_id: 7,
            frame_index: 2,
            tile_x: 1,
            tile_y: 3,
            width: 1,
            height: 1,
        };
        let rgba = [10u8, 20, 30, 40];
        let buf = encode_tile_frame(&slice, &rgba);

        assert_eq!(buf.len(), TILE_FRAME_HEADER_LEN + rgba.len());
        let field = |i: usize| u32::from_le_bytes(buf[i..i + 4].try_into().unwrap());
        assert_eq!(field(0), 7, "sprite_id");
        assert_eq!(field(4), 2, "frame_index");
        assert_eq!(field(8), 1, "tile_x");
        assert_eq!(field(12), 3, "tile_y");
        assert_eq!(field(16), 1, "width");
        assert_eq!(field(20), 1, "height");
        assert_eq!(&buf[TILE_FRAME_HEADER_LEN..], &rgba);
    }

    #[test]
    fn canvas_composite_metadata_matches_sprite() {
        let mut project = Project::new("test");
        let sprite = Sprite::empty(SpriteId::new(1), "hero", Size::new(64, 48));
        install_sprite(&mut project, sprite);

        let (named, _) = project.sprites_iter().next().unwrap();
        let sprite = &named.sprite;
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

    // ── Stroke session tests ──────────────────────────────────────────────

    fn opaque_red_session(initial_pixels: Vec<u8>, points: Vec<[f32; 2]>) -> StrokeSession {
        // 4x4 RGBA buffer.
        StrokeSession {
            sprite_id: SpriteId::new(1),
            layer_id: LayerId::new(2),
            frame_index: 0,
            buffer_id: PixelBufferId::new(3),
            buf_width: 4,
            buf_height: 4,
            buf_stride: 16,
            initial_pixels: Arc::new(initial_pixels),
            points,
            color: Rgba::opaque(255, 0, 0),
            brush_shape: "pixel".to_owned(),
            brush_size: 1,
            pixel_perfect: false,
            erase: false,
            label: "stroke".to_owned(),
        }
    }

    #[test]
    fn rasterize_session_pixels_passthrough_when_empty() {
        let initial = vec![0u8; 4 * 4 * 4];
        let session = opaque_red_session(initial.clone(), vec![]);
        let result = rasterize_session_pixels(&session).unwrap();
        assert_eq!(result, initial);
    }

    #[test]
    fn rasterize_session_pixels_paints_single_point() {
        let initial = vec![0u8; 4 * 4 * 4];
        let session = opaque_red_session(initial, vec![[0.0, 0.0]]);
        let result = rasterize_session_pixels(&session).unwrap();
        // First pixel = opaque red.
        assert_eq!(&result[0..4], &[255, 0, 0, 255]);
        // Second pixel = still transparent.
        assert_eq!(&result[4..8], &[0, 0, 0, 0]);
    }

    #[test]
    fn rasterize_session_pixels_idempotent_for_same_inputs() {
        // The whole point of session-based drawing is that re-rasterizing
        // from the captured initial_pixels with the same point list
        // produces the same output. Without that, partial extends could
        // accumulate and Ctrl+Z would step through them — which is the
        // bug this change fixes.
        let initial = vec![0u8; 4 * 4 * 4];
        let points = vec![[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]];
        let session = opaque_red_session(initial, points);
        let first = rasterize_session_pixels(&session).unwrap();
        let second = rasterize_session_pixels(&session).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn rasterize_session_pixels_eraser_clears_to_transparent() {
        let initial = vec![255u8; 4 * 4 * 4]; // fully opaque white
        let mut session = opaque_red_session(initial, vec![[0.0, 0.0]]);
        session.erase = true;
        let result = rasterize_session_pixels(&session).unwrap();
        // First pixel cleared to fully transparent.
        assert_eq!(&result[0..4], &[0, 0, 0, 0]);
        // Untouched pixel keeps its original opaque white.
        assert_eq!(&result[4..8], &[255, 255, 255, 255]);
    }

    #[test]
    fn stroke_session_orphan_retain_discards_same_buffer() {
        // The discard-on-next-begin pattern: `retain` drops every session
        // whose buffer matches the one the new stroke targets.
        use std::collections::HashMap;

        let initial = vec![0u8; 4];
        let mut active: HashMap<u32, StrokeSession> = HashMap::new();
        active.insert(99, opaque_red_session(initial.clone(), vec![[0.0, 0.0]]));
        active.insert(
            100,
            StrokeSession {
                sprite_id: SpriteId::new(1),
                layer_id: LayerId::new(2),
                frame_index: 0,
                buffer_id: PixelBufferId::new(7), // different buffer
                buf_width: 1,
                buf_height: 1,
                buf_stride: 4,
                initial_pixels: Arc::new(initial.clone()),
                points: vec![],
                color: Rgba::opaque(0, 255, 0),
                brush_shape: "pixel".to_owned(),
                brush_size: 1,
                pixel_perfect: false,
                erase: false,
                label: "stroke".to_owned(),
            },
        );

        // Mirror the begin_stroke discard step: drop sessions on buffer 3.
        active.retain(|_, s| s.buffer_id.get() != 3);

        assert!(!active.contains_key(&99));
        assert!(active.contains_key(&100));
    }

    #[test]
    fn dirty_tile_range_pixel_at_origin() {
        // 4x4 buffer, brush size 1, single pixel at (0,0). With
        // TILE_SIZE = 256 that's tile (0,0).
        let r = dirty_tile_range(&[[0.0, 0.0]], 1, 4, 4).unwrap();
        assert_eq!(r, (0, 0, 0, 0));
    }

    #[test]
    fn dirty_tile_range_off_canvas_returns_none() {
        // Point well off the buffer — clamping makes the rect empty.
        let r = dirty_tile_range(&[[100.0, 100.0]], 1, 4, 4);
        assert_eq!(r, None);
    }

    #[test]
    fn dirty_tile_range_brush_radius_widens_rect() {
        // Brush size 3 = radius 1; point at (5, 5) on a 16x16 buffer
        // covers (4..=6, 4..=6). Still tile (0,0).
        let r = dirty_tile_range(&[[5.0, 5.0]], 3, 16, 16).unwrap();
        assert_eq!(r, (0, 0, 0, 0));
    }

    #[test]
    fn dirty_tile_range_spans_multiple_tiles() {
        // Wide buffer big enough to contain multiple TILE_SIZE columns.
        // Two points: one in tile column 0, one in tile column 1.
        let buf_w = TILE_SIZE * 2 + 10;
        let buf_h = TILE_SIZE + 10;
        let p0: [f32; 2] = [10.0, 10.0]; // tile (0,0)
        // f32 with explicit literal — `as f32` from `u32` triggers
        // clippy's cast_precision_loss; spelling out the float avoids it.
        let p1: [f32; 2] = [261.0, 261.0]; // TILE_SIZE + 5
        let r = dirty_tile_range(&[p0, p1], 1, buf_w, buf_h).unwrap();
        assert_eq!(r, (0, 1, 0, 1));
    }

    #[test]
    fn dirty_tile_range_empty_points_returns_none() {
        let r = dirty_tile_range(&[], 1, 16, 16);
        assert_eq!(r, None);
    }

    // ── Lock enforcement (Phase A) ────────────────────────────────────────

    fn unlocked_raster(id: u32, name: &str) -> Layer {
        Layer::raster(LayerId::new(id), name)
    }

    fn locked_raster(id: u32, name: &str) -> Layer {
        let mut l = Layer::raster(LayerId::new(id), name);
        l.locked = true;
        l
    }

    fn group(id: u32, name: &str, locked: bool) -> Layer {
        use pixhaus_core::project::{BlendMode, LayerKind, UserData};
        Layer {
            id: LayerId::new(id),
            name: name.into(),
            kind: LayerKind::Group { collapsed: false },
            blend_mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            locked,
            parent: None,
            effects: Vec::new(),
            user_data: UserData::default(),
        }
    }

    #[test]
    fn check_layer_writable_passes_for_unlocked_layer() {
        let layers = vec![unlocked_raster(1, "background")];
        assert_eq!(check_layer_writable(&layers, LayerId::new(1)), Ok(()));
    }

    #[test]
    fn check_layer_writable_rejects_locked_layer() {
        let layers = vec![locked_raster(1, "background")];
        assert_eq!(
            check_layer_writable(&layers, LayerId::new(1)),
            Err(AppCommandError::LayerLocked { layer_id: 1 })
        );
    }

    #[test]
    fn check_layer_writable_rejects_when_ancestor_group_is_locked() {
        // Group 10 is locked; child raster 11 is unlocked but contained.
        let mut child = unlocked_raster(11, "leaf");
        child.parent = Some(LayerId::new(10));
        let layers = vec![group(10, "fx", true), child];
        assert_eq!(
            check_layer_writable(&layers, LayerId::new(11)),
            Err(AppCommandError::LayerLocked { layer_id: 11 })
        );
    }

    #[test]
    fn check_layer_writable_passes_when_unrelated_layer_locked() {
        // A locked sibling does not block paint on the target.
        let layers = vec![unlocked_raster(1, "target"), locked_raster(2, "decoration")];
        assert_eq!(check_layer_writable(&layers, LayerId::new(1)), Ok(()));
    }

    #[test]
    fn check_layer_writable_walks_multi_level_group_chain() {
        // outer (locked) > inner > leaf — leaf inherits the outer lock
        // through inner, which itself is unlocked.
        let mut inner = group(20, "inner", false);
        inner.parent = Some(LayerId::new(10));
        let mut leaf = unlocked_raster(21, "leaf");
        leaf.parent = Some(LayerId::new(20));
        let layers = vec![group(10, "outer", true), inner, leaf];
        assert_eq!(
            check_layer_writable(&layers, LayerId::new(21)),
            Err(AppCommandError::LayerLocked { layer_id: 21 })
        );
    }

    #[test]
    fn check_layer_writable_returns_not_found_for_missing_layer() {
        let layers = vec![unlocked_raster(1, "background")];
        assert!(matches!(
            check_layer_writable(&layers, LayerId::new(99)),
            Err(AppCommandError::NotFound { .. })
        ));
    }

    // ── Composite-on-draw (Phase B) ───────────────────────────────────────

    fn doc_with_two_raster_layers(canvas_w: u32, canvas_h: u32) -> crate::state::DocumentStore {
        // Layer 1 (id=1, bottom): red pixel at (0, 0).
        // Layer 2 (id=2, top):    green pixel at (1, 0).
        // Both layers cover the full canvas with otherwise-transparent
        // pixels so composite_onto's dimension check passes.
        let mut doc = crate::state::DocumentStore::default();
        let mut project = Project::new("test");
        let mut sprite = Sprite::empty(SpriteId::new(1), "hero", Size::new(canvas_w, canvas_h));
        sprite.layers.push(Layer::raster(LayerId::new(1), "bottom"));
        sprite.layers.push(Layer::raster(LayerId::new(2), "top"));

        let stride = canvas_w * 4;
        let buf_len = (stride * canvas_h) as usize;

        let mut bottom_pixels = vec![0u8; buf_len];
        // Pixel (0,0) → opaque red.
        bottom_pixels[0..4].copy_from_slice(&[255, 0, 0, 255]);

        let mut top_pixels = vec![0u8; buf_len];
        // Pixel (1,0) → opaque green.
        top_pixels[4..8].copy_from_slice(&[0, 255, 0, 255]);

        doc.pixel_buffers.push(PixelBufferEntry {
            id: 100,
            width: canvas_w,
            height: canvas_h,
            stride,
            pixels: bottom_pixels,
        });
        doc.pixel_buffers.push(PixelBufferEntry {
            id: 101,
            width: canvas_w,
            height: canvas_h,
            stride,
            pixels: top_pixels,
        });
        sprite.cels.push(Cel::raster(
            LayerId::new(1),
            FrameIndex::new(0),
            PixelBufferId::new(100),
            Size::new(canvas_w, canvas_h),
        ));
        sprite.cels.push(Cel::raster(
            LayerId::new(2),
            FrameIndex::new(0),
            PixelBufferId::new(101),
            Size::new(canvas_w, canvas_h),
        ));

        install_sprite(&mut project, sprite);
        doc.project = Some(project);
        doc
    }

    fn pixel_at(buf: &PixelBuffer, x: u32, y: u32) -> [u8; 4] {
        let p = buf.pixel(x, y).expect("pixel in bounds");
        [p.r, p.g, p.b, p.a]
    }

    #[test]
    fn composite_frame_includes_pixels_from_both_layers() {
        // The bug: drawing on layer 2 made layer 1 disappear because
        // tile-dirty events shipped layer 2's raw bytes. With composite_frame
        // the tile bytes should carry both layers' contributions.
        let doc = doc_with_two_raster_layers(4, 4);
        let composite = composite_frame(&doc, SpriteId::new(1), 0).expect("composite");
        assert_eq!(pixel_at(&composite, 0, 0), [255, 0, 0, 255]);
        assert_eq!(pixel_at(&composite, 1, 0), [0, 255, 0, 255]);
    }

    #[test]
    fn composite_frame_skips_hidden_layer() {
        // Hidden layer 1 → composite should not contain its red pixel.
        let mut doc = doc_with_two_raster_layers(4, 4);
        if let Some(project) = doc.project.as_mut()
            && let Some((named, _)) = project.sprites_iter_mut().next()
            && let Some(bottom) = named.sprite.layers.first_mut()
        {
            bottom.visible = false;
        }
        let composite = composite_frame(&doc, SpriteId::new(1), 0).expect("composite");
        // Layer 1 hidden → (0,0) is fully transparent.
        assert_eq!(pixel_at(&composite, 0, 0), [0, 0, 0, 0]);
        // Layer 2 still visible → (1,0) green.
        assert_eq!(pixel_at(&composite, 1, 0), [0, 255, 0, 255]);
    }

    #[test]
    fn composite_frame_skips_zero_opacity_layer() {
        let mut doc = doc_with_two_raster_layers(4, 4);
        if let Some(project) = doc.project.as_mut()
            && let Some((named, _)) = project.sprites_iter_mut().next()
            && let Some(top) = named.sprite.layers.get_mut(1)
        {
            top.opacity = 0;
        }
        let composite = composite_frame(&doc, SpriteId::new(1), 0).expect("composite");
        // Layer 1 still opaque → (0,0) red.
        assert_eq!(pixel_at(&composite, 0, 0), [255, 0, 0, 255]);
        // Layer 2 zero-opacity → (1,0) transparent.
        assert_eq!(pixel_at(&composite, 1, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn composite_frame_returns_transparent_for_empty_sprite() {
        let mut doc = crate::state::DocumentStore::default();
        let mut project = Project::new("test");
        install_sprite(
            &mut project,
            Sprite::empty(SpriteId::new(1), "hero", Size::new(8, 8)),
        );
        doc.project = Some(project);
        let composite = composite_frame(&doc, SpriteId::new(1), 0).expect("composite");
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(pixel_at(&composite, x, y), [0, 0, 0, 0]);
            }
        }
    }

    #[test]
    fn composite_frame_top_normal_replaces_bottom_at_overlapping_pixel() {
        // When two layers paint the same pixel and the top is opaque
        // Normal, the top wins — the standard Aseprite stacking rule.
        let canvas_w = 4u32;
        let canvas_h = 4u32;
        let mut doc = crate::state::DocumentStore::default();
        let mut project = Project::new("test");
        let mut sprite = Sprite::empty(SpriteId::new(1), "hero", Size::new(canvas_w, canvas_h));
        sprite.layers.push(Layer::raster(LayerId::new(1), "bottom"));
        sprite.layers.push(Layer::raster(LayerId::new(2), "top"));

        let stride = canvas_w * 4;
        let buf_len = (stride * canvas_h) as usize;
        let mut bottom_pixels = vec![0u8; buf_len];
        bottom_pixels[0..4].copy_from_slice(&[255, 0, 0, 255]); // red at (0,0)
        let mut top_pixels = vec![0u8; buf_len];
        top_pixels[0..4].copy_from_slice(&[0, 255, 0, 255]); // green at (0,0)

        doc.pixel_buffers.push(PixelBufferEntry {
            id: 100,
            width: canvas_w,
            height: canvas_h,
            stride,
            pixels: bottom_pixels,
        });
        doc.pixel_buffers.push(PixelBufferEntry {
            id: 101,
            width: canvas_w,
            height: canvas_h,
            stride,
            pixels: top_pixels,
        });
        sprite.cels.push(Cel::raster(
            LayerId::new(1),
            FrameIndex::new(0),
            PixelBufferId::new(100),
            Size::new(canvas_w, canvas_h),
        ));
        sprite.cels.push(Cel::raster(
            LayerId::new(2),
            FrameIndex::new(0),
            PixelBufferId::new(101),
            Size::new(canvas_w, canvas_h),
        ));
        install_sprite(&mut project, sprite);
        doc.project = Some(project);

        let composite = composite_frame(&doc, SpriteId::new(1), 0).expect("composite");
        assert_eq!(pixel_at(&composite, 0, 0), [0, 255, 0, 255]);
    }

    // ── Vectorize (S59) ────────────────────────────────────────────────────

    #[test]
    fn vectorize_produces_non_empty_strokes_for_a_16x16_outline() {
        // 16x16 buffer with a black-outline square in the middle. The
        // exact same call shape the command uses, just without the
        // Tauri state plumbing.
        // White RGB + full alpha for every pixel.
        let mut bytes = vec![255u8; 16 * 16 * 4];
        // Paint a 12x12 ink border at (2,2)..(14,14).
        for y in 2..14 {
            for x in 2..14 {
                if x == 2 || x == 13 || y == 2 || y == 13 {
                    let off = (y * 16 + x) * 4;
                    bytes[off] = 0;
                    bytes[off + 1] = 0;
                    bytes[off + 2] = 0;
                }
            }
        }
        let buf = PixelBuffer::from_raw(16, 16, 16 * 4, bytes).unwrap();
        let palette = pixhaus_core::project::Palette::from_colors(
            pixhaus_core::project::PaletteId::new(1),
            "ink",
            vec![Rgba::opaque(0, 0, 0)],
        );
        let result =
            centerline_vectorize(&buf, &palette, &CenterlineConfig::default()).expect("vectorize");
        assert_eq!(result.width, 16);
        assert_eq!(result.height, 16);
        assert!(
            !result.strokes.is_empty(),
            "expected centerline_vectorize to produce at least one stroke for an outline"
        );
    }

    // ── Command-level prep for MLAA + vectorize (S56/S59) ──────────────────

    /// A document with one raster layer carrying a 12x12 ink outline on a
    /// white field, plus a one-colour palette. Enough for the MLAA and
    /// vectorize command helpers to resolve a frame, buffer, and palette.
    fn doc_with_ink_layer_and_palette() -> crate::state::DocumentStore {
        use pixhaus_core::project::{PaletteId, Sprite};

        let mut doc = crate::state::DocumentStore::default();
        let mut project = Project::new("test");
        let mut sprite = Sprite::empty(SpriteId::new(1), "hero", Size::new(16, 16));
        sprite.layers.push(Layer::raster(LayerId::new(1), "ink"));
        sprite.palettes.push(Palette::from_colors(
            PaletteId::new(1),
            "ink",
            vec![Rgba::opaque(0, 0, 0)],
        ));

        let mut bytes = vec![255u8; 16 * 16 * 4];
        for y in 2..14 {
            for x in 2..14 {
                if x == 2 || x == 13 || y == 2 || y == 13 {
                    let off = (y * 16 + x) * 4;
                    bytes[off] = 0;
                    bytes[off + 1] = 0;
                    bytes[off + 2] = 0;
                }
            }
        }
        doc.pixel_buffers.push(PixelBufferEntry {
            id: 200,
            width: 16,
            height: 16,
            stride: 16 * 4,
            pixels: bytes,
        });
        sprite.cels.push(Cel::raster(
            LayerId::new(1),
            FrameIndex::new(0),
            PixelBufferId::new(200),
            Size::new(16, 16),
        ));
        install_sprite(&mut project, sprite);
        doc.project = Some(project);
        doc
    }

    #[test]
    fn vectorize_inputs_from_doc_resolves_buffer_and_palette() {
        let doc = doc_with_ink_layer_and_palette();
        let (buf, palette) =
            vectorize_inputs_from_doc(&doc, SpriteId::new(1), LayerId::new(1)).expect("inputs");
        assert_eq!(buf.width(), 16);
        assert!(!palette.colors.is_empty());
        // The resolved inputs vectorize end-to-end.
        let vi =
            centerline_vectorize(&buf, &palette, &CenterlineConfig::default()).expect("vectorize");
        assert!(!vi.strokes.is_empty());
    }

    #[test]
    fn vectorize_inputs_from_doc_unknown_sprite_is_error() {
        let doc = doc_with_ink_layer_and_palette();
        let err = vectorize_inputs_from_doc(&doc, SpriteId::new(999), LayerId::new(1)).unwrap_err();
        assert!(matches!(err, AppCommandError::NotFound { .. }));
    }

    #[test]
    fn mlaa_prep_resolves_active_frame_and_reads_cel() {
        let mut doc = doc_with_ink_layer_and_palette();
        let prep = mlaa_prep_in_doc(&mut doc, SpriteId::new(1), LayerId::new(1), None, None)
            .expect("prep");
        assert_eq!(
            prep.frame_index, 0,
            "defaults to frame 0 when no canvas set"
        );
        assert_eq!(prep.config, MlaaConfig::default());
        assert_eq!(prep.src.width(), 16);
        assert_eq!(prep.before.len(), 16 * 16 * 4);
        // The filter runs on the prepared source without error.
        let dst = morphological_antialias(&prep.src, &prep.config).expect("mlaa");
        assert_eq!(dst.width(), 16);
        assert_eq!(dst.height(), 16);
    }

    #[test]
    fn mlaa_prep_respects_explicit_config() {
        let mut doc = doc_with_ink_layer_and_palette();
        let prep = mlaa_prep_in_doc(
            &mut doc,
            SpriteId::new(1),
            LayerId::new(1),
            Some(32),
            Some(64),
        )
        .expect("prep");
        assert_eq!(prep.config.threshold, 32);
        assert_eq!(prep.config.softness, 64);
    }

    // ── MLAA defaults (S56) ────────────────────────────────────────────────

    /// Same default-resolution shape the Tauri command uses inline, so a
    /// future change to `MlaaConfig::default()` moves the command's
    /// defaults in lock-step.
    fn resolve_mlaa(threshold: Option<u8>, softness: Option<u8>) -> MlaaConfig {
        let defaults = MlaaConfig::default();
        MlaaConfig {
            threshold: threshold.unwrap_or(defaults.threshold),
            softness: softness.unwrap_or(defaults.softness),
        }
    }

    #[test]
    fn mlaa_config_resolves_to_openttoonz_defaults_when_unset() {
        let resolved = resolve_mlaa(None, None);
        assert_eq!(resolved, MlaaConfig::default());
        assert_eq!(resolved.threshold, 16);
        assert_eq!(resolved.softness, 128);
    }

    #[test]
    fn mlaa_config_resolves_explicit_overrides() {
        let resolved = resolve_mlaa(Some(32), Some(64));
        assert_eq!(resolved.threshold, 32);
        assert_eq!(resolved.softness, 64);
    }

    #[test]
    fn mlaa_smooths_staircase_into_distinct_buffer() {
        // 4x4 horizontal staircase: top-left 2x2 block opaque red, the
        // remaining pixels transparent. MLAA must produce a buffer that
        // differs from the input (the diagonal step gets softened).
        let mut bytes = vec![0u8; 4 * 4 * 4];
        for y in 0..2 {
            for x in 0..2 {
                let i = (y * 4 + x) * 4;
                bytes[i] = 255;
                bytes[i + 3] = 255;
            }
        }
        let src = PixelBuffer::from_raw(4, 4, 16, bytes.clone()).unwrap();
        let dst = morphological_antialias(&src, &MlaaConfig::default()).unwrap();
        assert_eq!(dst.width(), 4);
        assert_eq!(dst.height(), 4);
        assert_ne!(
            dst.as_bytes(),
            bytes.as_slice(),
            "expected MLAA to modify the staircase"
        );
    }

    // ── Gap-close request (S57) ───────────────────────────────────────────

    #[test]
    fn gap_close_request_resolve_uses_defaults_for_missing_fields() {
        let req = GapCloseRequest {
            closing_distance: None,
            closing_angle_rad: None,
            ink_threshold: None,
        };
        let resolved = req.resolve();
        let defaults = GapCloseConfig::default();
        assert_eq!(resolved.closing_distance, defaults.closing_distance);
        assert!((resolved.closing_angle_rad - defaults.closing_angle_rad).abs() < f32::EPSILON);
        assert_eq!(resolved.ink_threshold, defaults.ink_threshold);
    }

    #[test]
    fn gap_close_request_resolve_overrides_explicit_fields() {
        let req = GapCloseRequest {
            closing_distance: Some(20),
            closing_angle_rad: Some(0.5),
            ink_threshold: Some(64),
        };
        let resolved = req.resolve();
        assert_eq!(resolved.closing_distance, 20);
        assert!((resolved.closing_angle_rad - 0.5).abs() < f32::EPSILON);
        assert_eq!(resolved.ink_threshold, 64);
    }

    #[test]
    fn gap_close_request_deserializes_from_partial_json() {
        // Only `closing_distance` provided — the others fall back to None.
        let json = r#"{"closing_distance": 15}"#;
        let req: GapCloseRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.closing_distance, Some(15));
        assert_eq!(req.closing_angle_rad, None);
        assert_eq!(req.ink_threshold, None);
        let resolved = req.resolve();
        let defaults = GapCloseConfig::default();
        assert_eq!(resolved.closing_distance, 15);
        assert!((resolved.closing_angle_rad - defaults.closing_angle_rad).abs() < f32::EPSILON);
        assert_eq!(resolved.ink_threshold, defaults.ink_threshold);
    }

    #[test]
    fn gap_close_request_deserializes_from_empty_object() {
        let req: GapCloseRequest = serde_json::from_str("{}").unwrap();
        assert!(req.closing_distance.is_none());
        assert!(req.closing_angle_rad.is_none());
        assert!(req.ink_threshold.is_none());
    }
}
