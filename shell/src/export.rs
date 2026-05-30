//! GUI sprite export: encode composited frames to disk in one of three shapes.
//!
//! The pure pixel/geometry half — hard alpha masking and atlas packing — lives
//! in [`pixhaus_core::export`]. This module is the encode + IO half: it opens an
//! `rfd` save dialog off the UI thread, encodes with the `image` crate, and
//! writes the result. The shell wires it from the studio Land stage and the
//! editor File menu; the worker reports back over
//! [`crate::app::ShellMsg::SpriteExported`].
//!
//! Three shapes:
//!
//! - [`ExportKind::PngSequence`] — one transparent PNG per frame,
//!   `{base}_{i:03}.png`.
//! - [`ExportKind::LoopingGif`] — an infinitely-looping GIF with a hard alpha
//!   mask (`>=128` opaque, else the reserved transparent index) so the soft
//!   fringe a naive RGBA encode dithers into a halo is gone.
//! - [`ExportKind::Atlas`] — a packed PNG plus a `{base}.atlas.json` manifest of
//!   frame rects.
//!
//! All encoding runs on `spawn_blocking`: the `rfd` sync dialog and the encode
//! both block, and v2's egui loop is synchronous, so doing either inline would
//! freeze the window.

use std::io::BufWriter;
use std::path::{Path, PathBuf};

use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame, RgbaImage};
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::export::{AtlasLayout, AtlasOptions, hard_mask_alpha, pack_atlas};
use serde::Serialize;

/// Alpha at or above this is forced opaque before a GIF encode; below it the
/// pixel is routed to the reserved transparent index. The forge default.
const GIF_ALPHA_THRESHOLD: u8 = 128;

/// Which of the three export shapes the worker writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExportKind {
    /// One transparent PNG per frame.
    PngSequence,
    /// An infinitely-looping, hard-masked GIF.
    LoopingGif,
    /// A packed atlas PNG plus a sidecar JSON manifest.
    Atlas {
        /// Grid column count.
        columns: u32,
    },
}

/// A request to export the active sprite's composited frames. Built on the UI
/// thread from owned [`PixelBuffer`]s so no document borrow crosses the worker
/// boundary.
pub(crate) struct ExportRequest {
    /// Which shape to write.
    pub kind: ExportKind,
    /// Composited frames, in timeline order, all at the sprite canvas size.
    pub frames: Vec<PixelBuffer>,
    /// Per-frame delay in milliseconds (GIF timing + atlas manifest metadata).
    pub frame_ms: u32,
    /// File-stem the worker bases output names on (no extension).
    pub base_name: String,
    /// Test-only target path, injected to bypass the `rfd` dialog. `None` in
    /// production, where the worker opens the save dialog itself.
    #[cfg(test)]
    pub dest: Option<PathBuf>,
}

impl ExportRequest {
    /// Builds a production request (no injected destination).
    pub(crate) fn new(kind: ExportKind, frames: Vec<PixelBuffer>, frame_ms: u32, base_name: String) -> Self {
        Self {
            kind,
            frames,
            frame_ms,
            base_name,
            #[cfg(test)]
            dest: None,
        }
    }

    /// The chosen destination path: the injected test path when present, else
    /// the `rfd` save dialog for the request's kind. `None` is a cancelled
    /// dialog. The default file name carries the kind's extension; the PNG
    /// sequence picks a base path the worker suffixes per frame.
    fn pick_dest(&self) -> Option<PathBuf> {
        #[cfg(test)]
        if let Some(dest) = &self.dest {
            return Some(dest.clone());
        }
        let (title, file_name, ext): (&str, String, &str) = match self.kind {
            ExportKind::PngSequence => ("Export PNG frame sequence", format!("{}.png", self.base_name), "png"),
            ExportKind::LoopingGif => ("Export looping GIF", format!("{}.gif", self.base_name), "gif"),
            ExportKind::Atlas { .. } => ("Export packed atlas", format!("{}.png", self.base_name), "png"),
        };
        rfd::FileDialog::new()
            .set_title(title)
            .set_file_name(file_name)
            .add_filter(ext, &[ext])
            .save_file()
    }
}

/// The atlas sidecar manifest, written next to the atlas PNG as
/// `{base}.atlas.json`. Lets the Unity importer slice the sheet without
/// guessing the grid.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct AtlasManifest {
    /// Cell width (the shared frame width).
    pub cell_w: u32,
    /// Cell height (the shared frame height).
    pub cell_h: u32,
    /// Grid column count.
    pub columns: u32,
    /// Per-frame delay in milliseconds.
    pub frame_ms: u32,
    /// One entry per frame, in order.
    pub frames: Vec<ManifestFrame>,
}

/// One frame's placement in the atlas manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct ManifestFrame {
    /// Source frame index.
    pub index: usize,
    /// Left edge in the atlas.
    pub x: u32,
    /// Top edge in the atlas.
    pub y: u32,
    /// Cell width.
    pub w: u32,
    /// Cell height.
    pub h: u32,
}

impl AtlasManifest {
    /// Builds the manifest from a packed atlas and the frame timing.
    fn from_atlas(atlas: &pixhaus_core::export::Atlas, cell_w: u32, cell_h: u32, frame_ms: u32) -> Self {
        Self {
            cell_w,
            cell_h,
            columns: atlas.columns,
            frame_ms,
            frames: atlas
                .frames
                .iter()
                .map(|r| ManifestFrame {
                    index: r.index,
                    x: r.x,
                    y: r.y,
                    w: r.w,
                    h: r.h,
                })
                .collect(),
        }
    }
}

/// Runs the export off the UI thread: pick a destination via `rfd`, encode with
/// the `image` crate, and write to disk. `Ok(None)` means the dialog was
/// cancelled. Never touches the document — the frames are owned by the request.
///
/// # Errors
///
/// Returns a user-facing message string when the dialog open fails, the frames
/// are empty, encoding fails, or a file write fails.
pub(crate) fn run_export(req: ExportRequest) -> Result<Option<PathBuf>, String> {
    if req.frames.is_empty() {
        return Err("nothing to export: the sprite has no frames".to_owned());
    }
    let Some(path) = req.pick_dest() else {
        return Ok(None);
    };
    // Consume the request: the frames are owned by it, so destructuring moves
    // them into the encoder rather than borrowing across the worker boundary.
    let ExportRequest { kind, frames, frame_ms, .. } = req;
    match kind {
        ExportKind::PngSequence => write_png_sequence(&frames, &path),
        ExportKind::LoopingGif => write_looping_gif(&frames, frame_ms, &path),
        ExportKind::Atlas { columns } => write_atlas(&frames, columns, frame_ms, &path),
    }
}

/// Writes each frame as `{stem}_{i:03}.png` beside the chosen path. Returns the
/// first frame's path so the status line names a concrete file.
fn write_png_sequence(frames: &[PixelBuffer], path: &Path) -> Result<Option<PathBuf>, String> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("frame").to_owned();
    let mut first: Option<PathBuf> = None;
    for (i, frame) in frames.iter().enumerate() {
        let img = to_rgba_image(frame)?;
        let frame_path = dir.join(format!("{stem}_{i:03}.png"));
        img.save(&frame_path).map_err(|e| format!("writing {}: {e}", frame_path.display()))?;
        if first.is_none() {
            first = Some(frame_path);
        }
    }
    Ok(first)
}

/// Hard-masks every frame's alpha then encodes an infinitely-looping GIF. The
/// hard mask guarantees each edge pixel is alpha `0` or `255`, so the encoder
/// routes the transparent pixels to the reserved index and the soft-fringe halo
/// is gone.
fn write_looping_gif(frames: &[PixelBuffer], frame_ms: u32, path: &Path) -> Result<Option<PathBuf>, String> {
    let file = std::fs::File::create(path).map_err(|e| format!("creating {}: {e}", path.display()))?;
    let mut encoder = GifEncoder::new(BufWriter::new(file));
    encoder.set_repeat(Repeat::Infinite).map_err(|e| format!("setting gif repeat: {e}"))?;
    for frame in frames {
        let masked = hard_mask_alpha(frame, GIF_ALPHA_THRESHOLD);
        let img = to_rgba_image(&masked)?;
        let delay = Delay::from_numer_denom_ms(frame_ms, 1);
        encoder
            .encode_frame(Frame::from_parts(img, 0, 0, delay))
            .map_err(|e| format!("encoding gif frame: {e}"))?;
    }
    drop(encoder);
    Ok(Some(path.to_path_buf()))
}

/// Packs the frames into an atlas PNG and writes a `{stem}.atlas.json` manifest
/// beside it.
fn write_atlas(frames: &[PixelBuffer], columns: u32, frame_ms: u32, path: &Path) -> Result<Option<PathBuf>, String> {
    let opts = AtlasOptions {
        layout: AtlasLayout::Columns(columns),
        ..AtlasOptions::default()
    };
    let atlas = pack_atlas(frames, opts).map_err(|e| e.to_string())?;
    // Cell size is exact off any frame rect; fall back to the first frame's own
    // dimensions when the atlas somehow recorded no rects.
    let (cell_w, cell_h) = atlas
        .frames
        .first()
        .map_or_else(|| frames.first().map_or((0, 0), |f| (f.width(), f.height())), |r| (r.w, r.h));

    let img = to_rgba_image(&atlas.image)?;
    img.save(path).map_err(|e| format!("writing {}: {e}", path.display()))?;

    let manifest = AtlasManifest::from_atlas(&atlas, cell_w, cell_h, frame_ms);
    let manifest_path = atlas_manifest_path(path);
    let json = serde_json::to_string_pretty(&manifest).map_err(|e| format!("serializing manifest: {e}"))?;
    std::fs::write(&manifest_path, json).map_err(|e| format!("writing {}: {e}", manifest_path.display()))?;
    Ok(Some(path.to_path_buf()))
}

/// Derives the manifest path `{dir}/{stem}.atlas.json` from the atlas PNG path.
fn atlas_manifest_path(png_path: &Path) -> PathBuf {
    let dir = png_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = png_path.file_stem().and_then(|s| s.to_str()).unwrap_or("atlas");
    dir.join(format!("{stem}.atlas.json"))
}

/// Converts a tightly-packed RGBA [`PixelBuffer`] into an `image::RgbaImage`,
/// dropping any stride row padding. Mirrors `headless.rs::to_rgba_image` — a
/// padded buffer would otherwise smear the encode.
fn to_rgba_image(frame: &PixelBuffer) -> Result<RgbaImage, String> {
    let (w, h) = (frame.width(), frame.height());
    let tight: Vec<u8> = if frame.stride() == w * 4 {
        frame.as_bytes().to_vec()
    } else {
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        let row = (w * 4) as usize;
        for chunk in frame.as_bytes().chunks_exact(frame.stride() as usize) {
            out.extend_from_slice(&chunk[..row]);
        }
        out
    };
    RgbaImage::from_raw(w, h, tight).ok_or_else(|| "frame buffer size mismatch".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::decode_clip;
    use pixhaus_core::project::Rgba;

    /// A solid frame with a one-pixel semi-transparent edge ring (alpha 64) so
    /// the halo test has fringe to remove.
    fn frame_with_soft_ring(size: u32) -> PixelBuffer {
        let mut buf = PixelBuffer::filled(size, size, Rgba::opaque(200, 60, 90)).unwrap();
        for i in 0..size {
            for (x, y) in [(0, i), (size - 1, i), (i, 0), (i, size - 1)] {
                buf.set_pixel(x, y, Rgba::new(200, 60, 90, 64));
            }
        }
        buf
    }

    mod looping_gif {
        use super::*;

        #[test]
        fn re_decodes_with_no_partial_alpha_and_round_trips_frame_count() {
            let dir = tempfile::tempdir().unwrap();
            let dest = dir.path().join("loop.gif");
            let frames: Vec<_> = (0..3).map(|_| frame_with_soft_ring(8)).collect();
            let req = ExportRequest {
                kind: ExportKind::LoopingGif,
                frames,
                frame_ms: 100,
                base_name: "loop".to_owned(),
                dest: Some(dest.clone()),
            };
            let out = run_export(req).unwrap().expect("export wrote a path");
            assert_eq!(out, dest);

            let bytes = std::fs::read(&dest).unwrap();
            let decoded = decode_clip(&bytes, "image/gif", 10).expect("re-decode the exported gif");
            assert_eq!(decoded.len(), 3, "frame count must round-trip");
            for frame in &decoded {
                for px in frame.pixels.chunks_exact(4) {
                    let a = px[3];
                    assert!(a == 0 || a == 255, "decoded alpha {a} proves a halo survived the hard mask");
                }
            }
        }
    }

    mod png_sequence {
        use super::*;

        #[test]
        fn writes_one_named_file_per_frame_at_source_size() {
            let dir = tempfile::tempdir().unwrap();
            let dest = dir.path().join("seq.png");
            let frames: Vec<_> = (0..4).map(|_| PixelBuffer::filled(6, 5, Rgba::opaque(10, 20, 30)).unwrap()).collect();
            let req = ExportRequest {
                kind: ExportKind::PngSequence,
                frames,
                frame_ms: 100,
                base_name: "seq".to_owned(),
                dest: Some(dest),
            };
            run_export(req).unwrap().expect("sequence wrote a path");
            for i in 0..4 {
                let path = dir.path().join(format!("seq_{i:03}.png"));
                let img = image::open(&path).unwrap_or_else(|e| panic!("frame {i} at {}: {e}", path.display())).to_rgba8();
                assert_eq!(img.dimensions(), (6, 5), "frame {i} must decode to the source size");
            }
        }
    }

    mod atlas {
        use super::*;

        #[test]
        fn writes_one_png_and_one_json_with_rect_per_frame() {
            let dir = tempfile::tempdir().unwrap();
            let dest = dir.path().join("atlas.png");
            let frames: Vec<_> = (0..5).map(|_| PixelBuffer::filled(8, 8, Rgba::opaque(1, 2, 3)).unwrap()).collect();
            let req = ExportRequest {
                kind: ExportKind::Atlas { columns: 3 },
                frames,
                frame_ms: 100,
                base_name: "atlas".to_owned(),
                dest: Some(dest.clone()),
            };
            run_export(req).unwrap().expect("atlas wrote a path");

            assert!(dest.exists(), "atlas png must exist");
            let manifest_path = dir.path().join("atlas.atlas.json");
            assert!(manifest_path.exists(), "atlas manifest json must exist");

            let json = std::fs::read_to_string(&manifest_path).unwrap();
            let manifest: serde_json::Value = serde_json::from_str(&json).unwrap();
            let rects = manifest["frames"].as_array().expect("frames is an array");
            assert_eq!(rects.len(), 5, "manifest rect count must equal the frame count");
        }

        #[test]
        fn manifest_matches_snapshot() {
            use pixhaus_core::export::{Atlas, FrameRect};
            // Build a deterministic atlas value and snapshot the manifest JSON.
            let atlas = Atlas {
                image: PixelBuffer::filled(8, 8, Rgba::transparent()).unwrap(),
                columns: 2,
                frames: vec![
                    FrameRect {
                        index: 0,
                        x: 0,
                        y: 0,
                        w: 4,
                        h: 4,
                    },
                    FrameRect {
                        index: 1,
                        x: 4,
                        y: 0,
                        w: 4,
                        h: 4,
                    },
                    FrameRect {
                        index: 2,
                        x: 0,
                        y: 4,
                        w: 4,
                        h: 4,
                    },
                ],
            };
            let manifest = AtlasManifest::from_atlas(&atlas, 4, 4, 100);
            let json = serde_json::to_string_pretty(&manifest).unwrap();
            insta::assert_snapshot!(json);
        }
    }

    mod run_export {
        use super::*;

        #[test]
        fn empty_frames_is_an_error() {
            let req = ExportRequest {
                kind: ExportKind::LoopingGif,
                frames: Vec::new(),
                frame_ms: 100,
                base_name: "x".to_owned(),
                dest: Some(PathBuf::from("unused.gif")),
            };
            let err = run_export(req).unwrap_err();
            assert!(err.contains("no frames"), "expected a no-frames message, got: {err}");
        }
    }
}
