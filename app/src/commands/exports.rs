//! Export commands: PNG sprite sheet, animated GIF / WebP, Tiled `.tmx`.
//!
//! Each command resolves the active sprite, composites every raster
//! frame into a `PixelBuffer`, and hands the result to the matching
//! `pixhaus_io` exporter on the blocking-thread pool. Tilemap and
//! reference layers are skipped during composite — tilemaps export
//! through `export_tmx` instead, and reference layers are excluded by
//! contract (see `LayerKind::Reference` in `pixhaus_core`).
//!
//! These commands are write-only against disk and read-only against
//! the document store; they never mutate the project. The lock is
//! released before the IO encoder runs, so a long export does not
//! block other commands.

use std::path::PathBuf;

use pixhaus_core::canvas::{LayerInput, PixelBuffer, apply_effects, composite_layers};
use pixhaus_core::project::{
    Cel, CelData, FrameIndex, Layer, LayerId, LayerKind, PixelBufferId, Sprite, SpriteId,
};
use pixhaus_io::animated::{
    DitherMode, GifOptions, LoopCount, PaletteMode, WebPOptions, encode_gif, encode_webp,
};
use pixhaus_io::pixhaus::PixelBufferEntry;
use pixhaus_io::png::{ExportOptions, LayoutStrategy, export_sprite_sheet};
use pixhaus_io::tiled::{TiledExportOptions, TiledLayerInput, TilesetExportInput, export_tilemap};
use serde::Deserialize;
use tauri::State;
use tokio::task::JoinError;

use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

// ── Argument types ────────────────────────────────────────────────────────────

/// Arguments for a PNG sprite sheet export.
#[derive(Debug, Deserialize)]
pub struct ExportPngArgs {
    /// Sprite to export.
    pub sprite_id: SpriteId,
    /// Output path for the `.png`. The sibling `.json` is written next
    /// to it with the same stem.
    pub output_path: String,
    /// Number of columns in the packed grid. `None` means "single row".
    #[serde(default)]
    pub grid_cols: Option<u32>,
}

/// Arguments for an animated GIF export.
#[derive(Debug, Deserialize)]
pub struct ExportGifArgs {
    /// Sprite to export.
    pub sprite_id: SpriteId,
    /// Output `.gif` path.
    pub output_path: String,
}

/// Arguments for an animated WebP export.
#[derive(Debug, Deserialize)]
pub struct ExportWebpArgs {
    /// Sprite to export.
    pub sprite_id: SpriteId,
    /// Output `.webp` path.
    pub output_path: String,
}

/// Arguments for a Tiled `.tmx` export.
#[derive(Debug, Deserialize)]
pub struct ExportTmxArgs {
    /// Sprite carrying the tilemap layers to export.
    pub sprite_id: SpriteId,
    /// Output `.tmx` path. Sibling `.tsx` files share the directory.
    pub output_path: String,
}

// ── Public commands ───────────────────────────────────────────────────────────

/// Exports the active sprite as a PNG sprite sheet plus Aseprite-format
/// JSON metadata. The two files are written next to each other under the
/// same stem (e.g. `hero.png` + `hero.json`).
#[tauri::command(async, rename_all = "snake_case")]
pub async fn export_png_sprite_sheet(
    args: ExportPngArgs,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let (sprite, frames) = snapshot_sprite_for_export(args.sprite_id, &state).await?;

    let output_path = PathBuf::from(&args.output_path);
    let json_path = output_path.with_extension("json");
    let stem = output_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("sprite")
        .to_owned();
    let layout = match args.grid_cols {
        Some(cols) if cols > 0 => LayoutStrategy::Grid { cols },
        _ => LayoutStrategy::ByRow,
    };

    spawn_blocking_export(move || -> Result<(), pixhaus_io::Error> {
        let output = export_sprite_sheet(
            &sprite,
            &frames,
            &ExportOptions {
                layout,
                sprite_name: stem,
            },
        )?;
        std::fs::write(&output_path, &output.png_bytes).map_err(pixhaus_io::Error::Io)?;
        std::fs::write(&json_path, &output.json_bytes).map_err(pixhaus_io::Error::Io)?;
        Ok(())
    })
    .await
}

/// Exports the active sprite as an animated `.gif` using the project's
/// frame durations. Falls back to global `NeuQuant` quantization for the
/// 256-colour palette (the safest default for arbitrary RGBA content).
#[tauri::command(async, rename_all = "snake_case")]
pub async fn export_animated_gif(
    args: ExportGifArgs,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let (sprite, frames) = snapshot_sprite_for_export(args.sprite_id, &state).await?;
    let output_path = PathBuf::from(&args.output_path);
    let pairs = pair_frames_with_durations(&sprite, &frames);

    spawn_blocking_export(move || -> Result<(), pixhaus_io::Error> {
        let mut bytes: Vec<u8> = Vec::new();
        encode_gif(
            &pairs,
            &GifOptions {
                palette_mode: PaletteMode::GlobalQuantize,
                dither: DitherMode::default(),
                repeat: LoopCount::Infinite,
            },
            None,
            &mut bytes,
        )?;
        std::fs::write(&output_path, &bytes).map_err(pixhaus_io::Error::Io)?;
        Ok(())
    })
    .await
}

/// Exports the active sprite as an animated `.webp` (lossless VP8L by
/// default, the right choice for pixel art).
#[tauri::command(async, rename_all = "snake_case")]
pub async fn export_animated_webp(
    args: ExportWebpArgs,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let (sprite, frames) = snapshot_sprite_for_export(args.sprite_id, &state).await?;
    let output_path = PathBuf::from(&args.output_path);
    let pairs = pair_frames_with_durations(&sprite, &frames);

    spawn_blocking_export(move || -> Result<(), pixhaus_io::Error> {
        let bytes = encode_webp(&pairs, &WebPOptions::default())?;
        std::fs::write(&output_path, &bytes).map_err(pixhaus_io::Error::Io)?;
        Ok(())
    })
    .await
}

/// Exports tilemap layers from the active sprite as a Tiled `.tmx` plus
/// one `.tsx` per tileset. The output directory is the parent of
/// `output_path`; tileset image paths are written as `"{name}.png"`
/// strings into the TSX files (matching Tiled's expectation that
/// tileset images live alongside the `.tsx`).
#[tauri::command(async, rename_all = "snake_case")]
pub async fn export_tmx(args: ExportTmxArgs, state: State<'_, AppState>) -> CommandResult<()> {
    let sprite_snap = snapshot_sprite_only(args.sprite_id, &state).await?;
    let output_path = PathBuf::from(&args.output_path);
    let parent = output_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    let stem = output_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("tilemap")
        .to_owned();

    spawn_blocking_export(move || -> Result<(), pixhaus_io::Error> {
        let tilesets_input: Vec<TilesetExportInput<'_>> = sprite_snap
            .tilesets
            .iter()
            .map(|ts| TilesetExportInput {
                tileset: ts,
                image_path: format!("{}.png", ts.name),
            })
            .collect();

        // One TiledLayerInput per tilemap layer with at least one cel
        // on frame 0. This keeps export deterministic and predictable;
        // multi-frame tilemap export ships in a follow-up.
        let mut layer_inputs: Vec<TiledLayerInput<'_>> = Vec::new();
        for layer in &sprite_snap.layers {
            let LayerKind::Tilemap { tileset } = &layer.kind else {
                continue;
            };
            let tileset_index = sprite_snap
                .tilesets
                .iter()
                .position(|t| t.id == *tileset)
                .ok_or_else(|| {
                    pixhaus_io::Error::TiledSchemaViolation(format!(
                        "tilemap layer {:?} references missing tileset {}",
                        layer.id,
                        tileset.get(),
                    ))
                })?;
            let Some(cel) = sprite_snap
                .cels
                .iter()
                .find(|c| c.layer_id == layer.id && c.frame_index == FrameIndex::new(0))
            else {
                continue;
            };
            let CelData::Tilemap { data } = &cel.data else {
                continue;
            };
            layer_inputs.push(TiledLayerInput {
                name: layer.name.as_str(),
                data,
                tileset_index,
            });
        }

        let output = export_tilemap(
            &tilesets_input,
            &layer_inputs,
            &TiledExportOptions { name: stem },
        )?;
        std::fs::write(&output_path, output.tmx.as_bytes()).map_err(pixhaus_io::Error::Io)?;
        for tsx in &output.tilesets {
            let tsx_path = parent.join(&tsx.filename);
            std::fs::write(&tsx_path, tsx.tsx.as_bytes()).map_err(pixhaus_io::Error::Io)?;
        }
        Ok(())
    })
    .await
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Composite every frame of `sprite_id` and clone the sprite metadata
/// for handoff to the blocking encoder thread. Holds the read lock for
/// the duration of the snapshot, then releases it before encoding so a
/// long export doesn't block the editor.
async fn snapshot_sprite_for_export(
    sprite_id: SpriteId,
    state: &State<'_, AppState>,
) -> CommandResult<(Sprite, Vec<PixelBuffer>)> {
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
    if sprite.frames.is_empty() {
        return Err(AppCommandError::Validation {
            detail: "sprite has no frames; add a frame before exporting".into(),
        });
    }

    let mut frames: Vec<PixelBuffer> = Vec::with_capacity(sprite.frames.len());
    for (frame_idx, _) in sprite.frames.iter().enumerate() {
        let frame_u32 = u32::try_from(frame_idx).map_err(|_| AppCommandError::Validation {
            detail: format!("frame index {frame_idx} exceeds u32::MAX"),
        })?;
        let buf = composite_one_frame(sprite, frame_u32, &doc.pixel_buffers).map_err(|detail| {
            AppCommandError::Validation {
                detail: format!("composite frame {frame_idx}: {detail}"),
            }
        })?;
        frames.push(buf);
    }

    Ok((sprite.clone(), frames))
}

/// Lightweight snapshot for tilemap export — we only need the sprite,
/// not composited pixel buffers. Lifted out of
/// `snapshot_sprite_for_export` so TMX export doesn't spend cycles
/// compositing raster layers it will never write.
async fn snapshot_sprite_only(
    sprite_id: SpriteId,
    state: &State<'_, AppState>,
) -> CommandResult<Sprite> {
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
    Ok(sprite.clone())
}

/// Composites the raster layers of `sprite` at `frame_index` against a
/// blank canvas and returns the resulting RGBA buffer. Tilemap and
/// reference layers are skipped; tilemap export goes through TMX, and
/// reference layers are excluded from exports by data-model contract.
fn composite_one_frame(
    sprite: &Sprite,
    frame_index: u32,
    buffers: &[PixelBufferEntry],
) -> Result<PixelBuffer, String> {
    let canvas_w = sprite.canvas.width;
    let canvas_h = sprite.canvas.height;

    // Resolve each visible raster layer's contribution for this frame
    // into an owned PixelBuffer. We allocate transparent fillers for
    // missing cels so the LayerInput slice for `composite_layers` is
    // dense and back-to-front.
    let mut layer_bufs: Vec<(PixelBuffer, &Layer)> = Vec::new();
    for layer in &sprite.layers {
        if !matches!(layer.kind, LayerKind::Raster) {
            continue;
        }
        let buf = match resolve_cel_buffer(&sprite.cels, layer.id, frame_index, buffers) {
            Some(entry) => buffer_from_entry(entry, canvas_w, canvas_h)?,
            None => PixelBuffer::new(canvas_w, canvas_h)
                .map_err(|e| format!("alloc transparent layer: {e:?}"))?,
        };
        // Non-destructive layer effects are applied to a copy of the
        // composited pixels before blending; the cel data is untouched.
        let buf = if layer.effects.is_empty() {
            buf
        } else {
            apply_effects(&buf, &layer.effects).map_err(|e| format!("layer effects: {e:?}"))?
        };
        layer_bufs.push((buf, layer));
    }

    let inputs: Vec<LayerInput<'_>> = layer_bufs
        .iter()
        .map(|(buf, layer)| LayerInput {
            buffer: buf,
            mode: layer.blend_mode,
            opacity: layer.opacity,
            visible: layer.visible,
        })
        .collect();

    composite_layers(canvas_w, canvas_h, &inputs).map_err(|e| format!("composite_layers: {e:?}"))
}

/// Walks linked cels to find the underlying raster buffer for a
/// `(layer, frame)` address, returning the matching pixel buffer entry
/// from the document's buffer registry.
fn resolve_cel_buffer<'b>(
    cels: &[Cel],
    layer_id: LayerId,
    frame_index: u32,
    buffers: &'b [PixelBufferEntry],
) -> Option<&'b PixelBufferEntry> {
    let cel = cels
        .iter()
        .find(|c| c.layer_id == layer_id && c.frame_index.get() == frame_index)?;
    let buffer_id = match &cel.data {
        CelData::Raster { buffer, .. } => *buffer,
        CelData::Linked { source_frame } => {
            // A linked cel borrows another frame's cel on the same
            // layer. Fail-soft if the link is dangling.
            let source = cels
                .iter()
                .find(|c| c.layer_id == layer_id && c.frame_index == *source_frame)?;
            match &source.data {
                CelData::Raster { buffer, .. } => *buffer,
                _ => return None,
            }
        }
        CelData::Tilemap { .. } => return None,
    };
    find_buffer(buffers, buffer_id)
}

/// Looks up a pixel buffer entry by id.
fn find_buffer(buffers: &[PixelBufferEntry], id: PixelBufferId) -> Option<&PixelBufferEntry> {
    buffers.iter().find(|e| e.id == id.get())
}

/// Builds an owned [`PixelBuffer`] from a pixel-buffer registry entry.
///
/// Resizes onto the sprite canvas when the entry differs from the
/// expected dimensions: the data model permits cels smaller than the
/// canvas, but the exporters want canvas-sized layer buffers. The
/// "blit at origin" stub here is conservative — cels off (0, 0) can
/// land in the wrong spot. A correct cel-position blit lands when
/// stream S15 wires up real placement; today every cel is created at
/// origin by `ensure_raster_buffer`.
fn buffer_from_entry(
    entry: &PixelBufferEntry,
    canvas_w: u32,
    canvas_h: u32,
) -> Result<PixelBuffer, String> {
    if entry.width == canvas_w && entry.height == canvas_h && entry.stride == canvas_w * 4 {
        return PixelBuffer::from_raw(
            entry.width,
            entry.height,
            entry.stride,
            entry.pixels.clone(),
        )
        .map_err(|e| format!("from_raw: {e:?}"));
    }

    // Off-size or padded-stride entry: blit row-by-row into a fresh
    // canvas-sized buffer at (0, 0). Crops to the canvas if the cel is
    // larger.
    let mut canvas =
        PixelBuffer::new(canvas_w, canvas_h).map_err(|e| format!("alloc canvas buffer: {e:?}"))?;
    let copy_w = entry.width.min(canvas_w) as usize;
    let copy_h = entry.height.min(canvas_h) as usize;
    let dst_stride = (canvas_w * 4) as usize;
    let src_stride = entry.stride as usize;
    let dst = canvas.as_bytes_mut();
    for y in 0..copy_h {
        let src_off = y * src_stride;
        let dst_off = y * dst_stride;
        let src_row = entry
            .pixels
            .get(src_off..src_off + copy_w * 4)
            .ok_or_else(|| format!("source row {y} out of range"))?;
        let dst_row = dst
            .get_mut(dst_off..dst_off + copy_w * 4)
            .ok_or_else(|| format!("dest row {y} out of range"))?;
        dst_row.copy_from_slice(src_row);
    }
    Ok(canvas)
}

/// Pairs each composited frame with its `duration_ms` for the animated
/// encoders. Owns the buffers (clones aren't free, but `encode_gif` /
/// `encode_webp` need owned `PixelBuffer` values).
fn pair_frames_with_durations(sprite: &Sprite, frames: &[PixelBuffer]) -> Vec<(PixelBuffer, u32)> {
    sprite
        .frames
        .iter()
        .zip(frames.iter())
        .map(|(meta, buf)| (buf.clone(), meta.duration_ms))
        .collect()
}

/// Wraps a synchronous IO encoder closure in `tokio::task::spawn_blocking`
/// and maps both the join error and the inner result onto the IPC error
/// contract. Centralised here so every export command shares the same
/// failure shape.
async fn spawn_blocking_export<F>(f: F) -> CommandResult<()>
where
    F: FnOnce() -> Result<(), pixhaus_io::Error> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|join_err: JoinError| {
            let detail = if join_err.is_panic() {
                "export task panicked".to_owned()
            } else if join_err.is_cancelled() {
                "export task cancelled (runtime shutdown)".to_owned()
            } else {
                format!("export task did not complete: {join_err}")
            };
            AppCommandError::Validation { detail }
        })??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::geometry::Size;
    use pixhaus_core::project::id::FrameIndex;
    use pixhaus_core::project::{Frame, Layer, LayerId, Rgba, Sprite, SpriteId};

    fn sprite_with_one_raster_layer(canvas: Size) -> Sprite {
        let mut s = Sprite::empty(SpriteId::new(1), "tester", canvas);
        s.layers.push(Layer::raster(LayerId::new(1), "bg"));
        s.frames.push(Frame::default());
        s
    }

    #[test]
    fn composite_no_cels_returns_transparent_canvas() {
        let s = sprite_with_one_raster_layer(Size::new(4, 4));
        let buf = composite_one_frame(&s, 0, &[]).unwrap();
        assert_eq!(buf.width(), 4);
        assert_eq!(buf.height(), 4);
        // Every pixel should be fully transparent.
        for y in 0..4 {
            for x in 0..4 {
                let p = buf.pixel(x, y).unwrap();
                assert_eq!(p.a, 0, "pixel ({x},{y}) expected transparent");
            }
        }
    }

    #[test]
    fn composite_with_filled_cel_returns_layer_pixels() {
        let canvas = Size::new(2, 2);
        let mut s = sprite_with_one_raster_layer(canvas);
        // One raster cel referencing buffer 7.
        s.cels.push(pixhaus_core::project::Cel::raster(
            LayerId::new(1),
            FrameIndex::new(0),
            pixhaus_core::project::PixelBufferId::new(7),
            canvas,
        ));
        let red = Rgba::opaque(255, 0, 0);
        let pixels: Vec<u8> = (0..4).flat_map(|_| [red.r, red.g, red.b, red.a]).collect();
        let buffers = vec![PixelBufferEntry {
            id: 7,
            width: 2,
            height: 2,
            stride: 8,
            pixels,
        }];

        let buf = composite_one_frame(&s, 0, &buffers).unwrap();
        for y in 0..2 {
            for x in 0..2 {
                assert_eq!(buf.pixel(x, y).unwrap(), red);
            }
        }
    }

    #[test]
    fn composite_skips_invisible_layer() {
        let canvas = Size::new(2, 2);
        let mut s = sprite_with_one_raster_layer(canvas);
        s.layers[0].visible = false;
        s.cels.push(pixhaus_core::project::Cel::raster(
            LayerId::new(1),
            FrameIndex::new(0),
            pixhaus_core::project::PixelBufferId::new(1),
            canvas,
        ));
        let buffers = vec![PixelBufferEntry {
            id: 1,
            width: 2,
            height: 2,
            stride: 8,
            pixels: vec![255u8; 16],
        }];
        let buf = composite_one_frame(&s, 0, &buffers).unwrap();
        // Hidden layer must not contribute; result stays transparent.
        for y in 0..2 {
            for x in 0..2 {
                assert_eq!(buf.pixel(x, y).unwrap().a, 0);
            }
        }
    }
}
