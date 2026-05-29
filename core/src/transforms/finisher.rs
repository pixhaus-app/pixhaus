//! Style-gated pixel-art finisher: downscale-to-grid + palette snap.
//!
//! This is the deterministic "make it pixel art" post-pass that runs on
//! AI-generated frames after they return at backend resolution. It does two
//! things, in order:
//!
//! 1. **True-downscale** each frame to a low pixel grid with nearest-neighbour
//!    sampling — never a bilinear/Lanczos resample. A smooth resample would
//!    reintroduce the soft edges the finisher exists to remove. This is a
//!    deliberate divergence from the literal forge port (which used LANCZOS);
//!    nearest keeps the discrete-pixel look. When the target divides the source
//!    evenly the lossless top-left integer sample is used
//!    ([`crate::transforms::scale::scale_integer_down`]); otherwise it falls
//!    back to [`crate::transforms::scale::scale_nearest`].
//! 2. **Snap** every opaque pixel to a small palette — either extracted from the
//!    frame ([`crate::color::extraction::extract_palette_from_rgba8`]) or a
//!    fixed palette supplied by the caller (typically the anchor's
//!    `extracted_palette`). Alpha-0 pixels are left byte-identical.
//!
//! The finisher itself has **no concept of art style** — it is the caller's
//! gate. The shell decides whether to call it based on the resolved
//! [`ArtStyleKind`](crate::project::library::composition::ArtStyleKind): only
//! `is_pixel()` styles finish; clean-HD and map-style skip it and land their
//! `normalize_frames` output verbatim.
//!
//! Runs after [`normalize`](super::normalize) on the animation/studio paths.
//! It is pure, synchronous CPU work bounded by the post-downscale pixel count
//! (the low grid), not the source canvas — well inside the 8K constraint.

use crate::canvas::buffer::PixelBuffer;
use crate::color::extraction::{ExtractionOptions, extract_palette_from_rgba8};
use crate::project::id::PaletteId;
use crate::project::palette::Palette;

use super::error::Result;
use super::scale::{scale_integer_down, scale_nearest};

/// Default number of swatches the extract path derives, matching the
/// eyedropper extractor's pixel-art-friendly ceiling.
pub const DEFAULT_FINISH_MAX_COLORS: usize = 32;

/// Default per-channel bit depth for the extract path, matching the
/// eyedropper extractor's default so fringe colours collapse onto their
/// dominant tone.
pub const DEFAULT_FINISH_QUANTIZE_BITS: u8 = 5;

/// Default alpha cut-off below which a pixel is treated as background and
/// left untouched by the snap.
pub const DEFAULT_FINISH_ALPHA_THRESHOLD: u8 = 128;

/// Where the snap palette comes from.
#[derive(Clone, Debug)]
pub enum PaletteSource {
    /// Build a palette from the (already downscaled) frame by frequency,
    /// capped at `max_colors`, with `quantize_bits` per channel folding
    /// near-duplicate tones together.
    Extract {
        /// Maximum number of swatches to keep.
        max_colors: usize,
        /// Bits per channel kept during extraction (`8` disables it).
        quantize_bits: u8,
    },
    /// Snap to exactly these colours — typically the anchor's
    /// `extracted_palette`, so a derivation keeps the anchor's palette.
    Fixed(Palette),
    /// No palette reduction. The downscale still runs; the colour set is
    /// left unchanged.
    None,
}

/// Tunable knobs for [`finish_frame`] / [`finish_frames`].
#[derive(Clone, Debug)]
pub struct FinishOptions {
    /// Low pixel grid to downscale to. `None` is a palette-only finish that
    /// leaves the dimensions unchanged.
    pub target_grid: Option<(u32, u32)>,
    /// How to source the snap palette.
    pub palette: PaletteSource,
    /// Pixels with alpha below this are background — never snapped, left
    /// byte-identical. Also used to gate which pixels feed the extractor.
    pub alpha_threshold: u8,
}

impl FinishOptions {
    /// The default canvas finish: downscale to `(width, height)` and snap to a
    /// palette extracted from the frame (32 colours, 5-bit quantisation).
    ///
    /// Use this when no anchor palette is available — the frame supplies its
    /// own palette.
    #[must_use]
    pub fn for_canvas(width: u32, height: u32) -> Self {
        Self {
            target_grid: Some((width, height)),
            palette: PaletteSource::Extract {
                max_colors: DEFAULT_FINISH_MAX_COLORS,
                quantize_bits: DEFAULT_FINISH_QUANTIZE_BITS,
            },
            alpha_threshold: DEFAULT_FINISH_ALPHA_THRESHOLD,
        }
    }

    /// Downscale to `grid` and snap to the supplied `palette`.
    ///
    /// The anchor-palette path: prefer this over [`Self::for_canvas`] when the
    /// anchor's `extracted_palette` is present so a derivation does not drift
    /// off the established palette (over-quantising guard from the brief).
    #[must_use]
    pub fn with_palette(grid: (u32, u32), palette: Palette) -> Self {
        Self {
            target_grid: Some(grid),
            palette: PaletteSource::Fixed(palette),
            alpha_threshold: DEFAULT_FINISH_ALPHA_THRESHOLD,
        }
    }
}

/// A finished frame: the downscaled-and-snapped buffer plus the palette it was
/// snapped to.
///
/// The `palette` is what callers persist as the variant's `extracted_palette`
/// (the extract path) or pass straight through (the fixed path). For the
/// `None` source it is an empty palette.
#[derive(Clone, Debug)]
pub struct FinishedFrame {
    /// The finished pixel buffer.
    pub buffer: PixelBuffer,
    /// The palette the buffer was snapped to.
    pub palette: Palette,
}

/// Downscales `buf` (integer divisor when exact, else nearest) then snaps the
/// result to a palette resolved from `opts.palette`.
///
/// Order is downscale-first so the extract/snap runs over the small post-grid
/// buffer, not the full-resolution source. An empty source returns an empty
/// finished frame with an empty palette.
///
/// # Errors
///
/// Propagates [`super::error::Error`] from the downscale step
/// ([`scale_integer_down`] / [`scale_nearest`]) and from pixel-buffer
/// allocation.
pub fn finish_frame(buf: &PixelBuffer, opts: &FinishOptions) -> Result<FinishedFrame> {
    if buf.is_empty() {
        return Ok(FinishedFrame {
            buffer: PixelBuffer::empty(),
            palette: empty_palette(),
        });
    }

    let mut buffer = downscale(buf, opts.target_grid)?;
    let palette = resolve_palette(&buffer, &opts.palette, opts.alpha_threshold);
    snap_to_palette(&mut buffer, &palette);

    Ok(FinishedFrame { buffer, palette })
}

/// Finishes a batch of frames with the same `opts`, in order.
///
/// Each frame resolves its own palette under [`PaletteSource::Extract`]; under
/// [`PaletteSource::Fixed`] every frame snaps to the same supplied palette. An
/// empty input yields an empty `Vec`.
///
/// # Errors
///
/// Propagates the first [`super::error::Error`] from [`finish_frame`].
pub fn finish_frames(frames: &[PixelBuffer], opts: &FinishOptions) -> Result<Vec<FinishedFrame>> {
    frames.iter().map(|f| finish_frame(f, opts)).collect()
}

/// Snaps every opaque pixel in `buf` to its nearest `palette` entry, in place.
///
/// Lifted verbatim from the shell's manual "Reduce to palette" helper so there
/// is one snap implementation. For each pixel with non-zero alpha, maps the
/// colour to [`Palette::nearest_index`] and writes back the entry's colour,
/// keeping the pixel's own alpha. Alpha-0 pixels are left byte-identical (a
/// transparent pixel has no colour to quantize). An empty palette is a no-op:
/// `nearest_index` returns `None`, so there is nothing to map to.
///
/// Works directly on the buffer's `Vec<u8>` stride rows — four bytes per pixel,
/// no intermediate `Vec<Vec<u8>>` — so the cost is bounded by the buffer's
/// pixel count, not the canvas (the 8K constraint).
pub fn snap_to_palette(buf: &mut PixelBuffer, palette: &Palette) {
    // An empty palette has no nearest entry; skip the whole pass.
    if palette.colors.is_empty() {
        return;
    }
    let height = buf.height();
    for y in 0..height {
        let Some(row) = buf.row_mut(y) else { continue };
        for px in row.chunks_exact_mut(4) {
            // px is a 4-byte RGBA chunk: leave fully-transparent pixels alone.
            if px[3] == 0 {
                continue;
            }
            let here = crate::project::Rgba::new(px[0], px[1], px[2], px[3]);
            // nearest_index ignores alpha on both sides and returns Some(_) for a
            // non-empty palette, so the get is in range; keep the pixel's alpha.
            if let Some(idx) = palette.nearest_index(here) {
                if let Some(entry) = palette.colors.get(idx) {
                    let c = entry.color;
                    px[0] = c.r;
                    px[1] = c.g;
                    px[2] = c.b;
                }
            }
        }
    }
}

/// Downscales `buf` to `target` when set: integer top-left sample when the
/// target divides both dimensions evenly (lossless, no averaging), else
/// nearest-neighbour. `None` returns a clone (palette-only finish).
fn downscale(buf: &PixelBuffer, target: Option<(u32, u32)>) -> Result<PixelBuffer> {
    let Some((tw, th)) = target else {
        return Ok(buf.clone());
    };
    let (sw, sh) = (buf.width(), buf.height());
    if tw == 0 || th == 0 {
        // A zero grid is meaningless; treat it as a palette-only finish rather
        // than erroring, keeping the source dimensions.
        return Ok(buf.clone());
    }
    if tw == sw && th == sh {
        return Ok(buf.clone());
    }
    // Prefer the lossless integer-divisor path when the target divides both
    // dimensions evenly and is a true downscale.
    if tw <= sw && th <= sh && sw % tw == 0 && sh % th == 0 && sw / tw == sh / th {
        return scale_integer_down(buf, sw / tw);
    }
    scale_nearest(buf, tw, th)
}

/// Resolves the snap palette for `buf` from `source`. The extract path runs
/// over the (already downscaled) buffer's bytes; an empty result yields an
/// empty palette.
fn resolve_palette(buf: &PixelBuffer, source: &PaletteSource, alpha_threshold: u8) -> Palette {
    match source {
        PaletteSource::Fixed(p) => p.clone(),
        PaletteSource::None => empty_palette(),
        PaletteSource::Extract { max_colors, quantize_bits } => {
            // Extraction wants a tightly-packed RGBA8 buffer. A padded stride
            // would misalign the chunks, so flatten to width*height*4 first.
            let packed = packed_rgba(buf);
            let entries = extract_palette_from_rgba8(
                &packed,
                buf.width(),
                buf.height(),
                ExtractionOptions {
                    max_colors: *max_colors,
                    quantize_bits: *quantize_bits,
                    min_opaque_alpha: alpha_threshold,
                },
            );
            // Frequency order is the extractor's; preserve it.
            Palette::from_colors(PaletteId::new(0), "finished", entries.into_iter().map(|e| e.color).collect())
        }
    }
}

/// Copies `buf` into a tightly-packed `width*height*4` RGBA8 vector, dropping
/// any stride padding so the frequency extractor reads aligned pixels.
fn packed_rgba(buf: &PixelBuffer) -> Vec<u8> {
    let row_len = (buf.width() as usize) * 4;
    let mut out = Vec::with_capacity(row_len * buf.height() as usize);
    for y in 0..buf.height() {
        if let Some(row) = buf.row(y) {
            out.extend_from_slice(row);
        }
    }
    out
}

/// An empty, unnamed palette used by the `None`/empty-frame paths.
fn empty_palette() -> Palette {
    Palette::from_colors(PaletteId::new(0), "finished", Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Rgba;
    use rstest::rstest;

    /// A 256×256 buffer painted as a 4×4 grid of 64×64 solid blocks, each a
    /// distinct opaque colour. Downscaling to 64×64 (an exact 4:1 divisor)
    /// must yield a 16×16 buffer where each block collapses to one pixel of
    /// its own colour.
    fn block_grid_256() -> PixelBuffer {
        let mut buf = PixelBuffer::new(256, 256).expect("buffer");
        for by in 0..4u32 {
            for bx in 0..4u32 {
                let c = Rgba::opaque((bx as u8) * 60 + 10, (by as u8) * 60 + 10, 80);
                for y in 0..64 {
                    for x in 0..64 {
                        buf.set_pixel(bx * 64 + x, by * 64 + y, c);
                    }
                }
            }
        }
        buf
    }

    #[test]
    fn downscale_exact_divisor_preserves_block_color() {
        // 256×256 ÷ a 64×64 target is the exact-divisor (integer) path.
        let buf = block_grid_256();
        let opts = FinishOptions {
            target_grid: Some((64, 64)),
            palette: PaletteSource::None,
            alpha_threshold: 0,
        };
        let out = finish_frame(&buf, &opts).expect("finish");
        assert_eq!(out.buffer.width(), 64);
        assert_eq!(out.buffer.height(), 64);
        // The top-left block was painted (10,10,80). Sample inside it.
        assert_eq!(out.buffer.pixel(0, 0), Some(Rgba::opaque(10, 10, 80)));
        // The block at grid (3,3) is the bottom-right; with a 4:1 collapse it
        // spans the last 16 output columns/rows.
        assert_eq!(out.buffer.pixel(63, 63), Some(Rgba::opaque(190, 190, 80)));
    }

    #[test]
    fn downscale_non_integer_target_uses_nearest() {
        // 256 is not divisible by 100, so this must take the nearest path and
        // still hit the requested dimensions exactly.
        let buf = block_grid_256();
        let opts = FinishOptions {
            target_grid: Some((100, 100)),
            palette: PaletteSource::None,
            alpha_threshold: 0,
        };
        let out = finish_frame(&buf, &opts).expect("finish");
        assert_eq!(out.buffer.width(), 100);
        assert_eq!(out.buffer.height(), 100);
        // The very top-left pixel still maps into the first block's colour.
        assert_eq!(out.buffer.pixel(0, 0), Some(Rgba::opaque(10, 10, 80)));
    }

    #[test]
    fn none_palette_downscales_but_keeps_colors() {
        // A two-colour 4×2 buffer downscaled to 2×1; with None the colour set
        // is unchanged (no snap), only the dimensions shrink.
        let mut buf = PixelBuffer::new(4, 2).expect("buffer");
        for y in 0..2 {
            buf.set_pixel(0, y, Rgba::opaque(200, 10, 10));
            buf.set_pixel(1, y, Rgba::opaque(200, 10, 10));
            buf.set_pixel(2, y, Rgba::opaque(10, 10, 200));
            buf.set_pixel(3, y, Rgba::opaque(10, 10, 200));
        }
        let opts = FinishOptions {
            target_grid: Some((2, 1)),
            palette: PaletteSource::None,
            alpha_threshold: 0,
        };
        let out = finish_frame(&buf, &opts).expect("finish");
        assert_eq!(out.buffer.width(), 2);
        assert_eq!(out.buffer.height(), 1);
        // Top-left sample of each 2×2 block: red then blue, unchanged.
        assert_eq!(out.buffer.pixel(0, 0), Some(Rgba::opaque(200, 10, 10)));
        assert_eq!(out.buffer.pixel(1, 0), Some(Rgba::opaque(10, 10, 200)));
        assert!(out.palette.colors.is_empty(), "None source yields an empty palette");
    }

    #[test]
    fn extract_palette_reduces_red_ramp_and_snaps() {
        // A horizontal red ramp at full resolution; finishing it to a low grid
        // with the extract path must reduce it to a handful of colours, and
        // every opaque output pixel must be one of the returned swatches.
        let w = 64u32;
        let mut buf = PixelBuffer::new(w, 8).expect("buffer");
        for x in 0..w {
            #[allow(clippy::cast_possible_truncation)]
            let r = (x * 255 / (w - 1)) as u8;
            for y in 0..8 {
                buf.set_pixel(x, y, Rgba::opaque(r, 0, 0));
            }
        }
        let opts = FinishOptions {
            target_grid: Some((8, 8)),
            palette: PaletteSource::Extract {
                max_colors: 8,
                quantize_bits: 5,
            },
            alpha_threshold: 1,
        };
        let out = finish_frame(&buf, &opts).expect("finish");
        assert!(!out.palette.colors.is_empty());
        assert!(out.palette.colors.len() <= 8, "ramp reduced to <= 8 colours");
        let set: Vec<Rgba> = out.palette.colors.iter().map(|e| e.color).collect();
        for y in 0..out.buffer.height() {
            for x in 0..out.buffer.width() {
                let px = out.buffer.pixel(x, y).expect("pixel");
                if px.a == 0 {
                    continue;
                }
                assert!(
                    set.iter().any(|c| c.r == px.r && c.g == px.g && c.b == px.b),
                    "opaque pixel ({x},{y}) = {px:?} not in returned palette"
                );
            }
        }
    }

    #[test]
    fn fixed_palette_snaps_to_supplied_colors_only() {
        // The buffer is a smooth grey ramp; a fixed black/white palette must
        // map every opaque pixel to exactly black or white.
        let mut buf = PixelBuffer::new(4, 1).expect("buffer");
        buf.set_pixel(0, 0, Rgba::opaque(10, 10, 10));
        buf.set_pixel(1, 0, Rgba::opaque(90, 90, 90));
        buf.set_pixel(2, 0, Rgba::opaque(170, 170, 170));
        buf.set_pixel(3, 0, Rgba::opaque(245, 245, 245));
        let fixed = Palette::from_colors(PaletteId::new(7), "bw", vec![Rgba::opaque(0, 0, 0), Rgba::opaque(255, 255, 255)]);
        let opts = FinishOptions {
            target_grid: None, // palette-only finish, keep 4×1
            palette: PaletteSource::Fixed(fixed),
            alpha_threshold: 1,
        };
        let out = finish_frame(&buf, &opts).expect("finish");
        assert_eq!(out.buffer.width(), 4);
        for x in 0..4 {
            let px = out.buffer.pixel(x, 0).expect("pixel");
            assert!(
                px == Rgba::opaque(0, 0, 0) || px == Rgba::opaque(255, 255, 255),
                "pixel {x} = {px:?} must be one of the two fixed colours"
            );
        }
        // The dark end snaps black, the light end snaps white.
        assert_eq!(out.buffer.pixel(0, 0), Some(Rgba::opaque(0, 0, 0)));
        assert_eq!(out.buffer.pixel(3, 0), Some(Rgba::opaque(255, 255, 255)));
    }

    #[test]
    fn alpha_zero_pixels_left_untouched() {
        // Ported from the shell quantize test: a fully-transparent pixel
        // carrying stray RGB survives byte-for-byte through the snap.
        let mut buf = PixelBuffer::new(2, 1).expect("buffer");
        let ghost = Rgba::new(123, 45, 67, 0);
        buf.set_pixel(0, 0, ghost);
        buf.set_pixel(1, 0, Rgba::opaque(40, 40, 40));
        let fixed = Palette::from_colors(PaletteId::new(1), "bw", vec![Rgba::opaque(0, 0, 0), Rgba::opaque(255, 255, 255)]);
        let opts = FinishOptions {
            target_grid: None,
            palette: PaletteSource::Fixed(fixed),
            alpha_threshold: 1,
        };
        let out = finish_frame(&buf, &opts).expect("finish");
        assert_eq!(out.buffer.pixel(0, 0), Some(ghost), "alpha-0 pixel left untouched");
        assert_eq!(out.buffer.pixel(1, 0), Some(Rgba::opaque(0, 0, 0)), "opaque pixel snapped");
    }

    #[test]
    fn empty_frame_is_a_noop() {
        let buf = PixelBuffer::empty();
        let out = finish_frame(&buf, &FinishOptions::for_canvas(64, 64)).expect("finish");
        assert!(out.buffer.is_empty());
        assert!(out.palette.colors.is_empty());
    }

    #[test]
    fn empty_palette_snap_is_a_noop() {
        let mut buf = PixelBuffer::filled(2, 2, Rgba::opaque(13, 17, 19)).expect("buffer");
        let before = buf.as_bytes().to_vec();
        snap_to_palette(&mut buf, &Palette::from_colors(PaletteId::new(2), "empty", vec![]));
        assert_eq!(buf.as_bytes(), before.as_slice(), "an empty palette changes nothing");
    }

    #[test]
    fn snap_to_palette_maps_opaque_to_nearest() {
        // Ported from the shell quantize test to keep the snap behaviour pinned
        // in core where the implementation now lives.
        let mut buf = PixelBuffer::new(2, 2).expect("buffer");
        buf.set_pixel(0, 0, Rgba::opaque(30, 30, 30));
        buf.set_pixel(1, 0, Rgba::opaque(200, 210, 220));
        buf.set_pixel(0, 1, Rgba::opaque(10, 5, 8));
        buf.set_pixel(1, 1, Rgba::opaque(240, 245, 250));
        let palette = Palette::from_colors(
            PaletteId::new(1),
            "bw",
            vec![Rgba::transparent(), Rgba::opaque(0, 0, 0), Rgba::opaque(255, 255, 255)],
        );
        snap_to_palette(&mut buf, &palette);
        assert_eq!(buf.pixel(0, 0), Some(Rgba::opaque(0, 0, 0)));
        assert_eq!(buf.pixel(1, 0), Some(Rgba::opaque(255, 255, 255)));
        assert_eq!(buf.pixel(0, 1), Some(Rgba::opaque(0, 0, 0)));
        assert_eq!(buf.pixel(1, 1), Some(Rgba::opaque(255, 255, 255)));
    }

    #[test]
    fn finish_frames_batches_in_order() {
        let mut a = PixelBuffer::filled(8, 8, Rgba::opaque(200, 10, 10)).expect("buffer");
        a.set_pixel(0, 0, Rgba::opaque(10, 10, 200));
        let b = PixelBuffer::filled(8, 8, Rgba::opaque(10, 200, 10)).expect("buffer");
        let opts = FinishOptions::for_canvas(4, 4);
        let out = finish_frames(&[a, b], &opts).expect("finish");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].buffer.width(), 4);
        assert_eq!(out[1].buffer.width(), 4);
        // The second frame is solid green: its extracted palette is one colour.
        // `for_canvas` extracts at 5-bit, so the green's low bits are masked
        // (10 & 0xF8 = 8); the snapped pixel and the swatch agree.
        assert_eq!(out[1].palette.colors.len(), 1);
        assert_eq!(out[1].palette.colors[0].color, Rgba::opaque(8, 200, 8));
        assert_eq!(
            out[1].buffer.pixel(0, 0),
            Some(Rgba::opaque(8, 200, 8)),
            "solid green snaps to its own 5-bit swatch"
        );
    }

    #[rstest]
    #[case((32, 32))]
    #[case((48, 24))]
    #[case((100, 100))]
    fn output_dims_match_target_grid(#[case] grid: (u32, u32)) {
        // output dims == target_grid when set, regardless of the source size,
        // across both exact-divisor (32, 48) and nearest (100) target grids.
        let buf = block_grid_256();
        let opts = FinishOptions {
            target_grid: Some(grid),
            palette: PaletteSource::Extract {
                max_colors: 16,
                quantize_bits: 5,
            },
            alpha_threshold: 1,
        };
        let out = finish_frame(&buf, &opts).expect("finish");
        assert_eq!((out.buffer.width(), out.buffer.height()), grid);
    }

    // ── insta: lock the extracted palette hex ordering ────────────────────

    #[test]
    fn for_canvas_extracted_palette_hex_snapshot() {
        // A deterministic 2-colour-dominant frame: a mostly-red field with a
        // smaller blue block. Snapshotting the hex list locks both the colours
        // and their frequency ordering (red first).
        let mut buf = PixelBuffer::filled(64, 64, Rgba::opaque(200, 20, 20)).expect("buffer");
        for y in 0..16 {
            for x in 0..16 {
                buf.set_pixel(x, y, Rgba::opaque(20, 20, 200));
            }
        }
        let out = finish_frame(&buf, &FinishOptions::for_canvas(16, 16)).expect("finish");
        let hex: Vec<String> = out
            .palette
            .colors
            .iter()
            .map(|e| format!("#{:02x}{:02x}{:02x}", e.color.r, e.color.g, e.color.b))
            .collect();
        insta::assert_debug_snapshot!(hex);
    }

    // ── proptest: snap closure + output dims ──────────────────────────────

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn every_opaque_output_pixel_is_in_returned_palette(
            seed in proptest::array::uniform32(any::<u8>()),
        ) {
            // Paint an 8×8 frame from the random seed bytes (two per pixel for
            // 32 of the 64 pixels; the rest tile), all opaque, then finish to a
            // 4×4 grid with the extract path.
            let mut buf = PixelBuffer::new(8, 8).expect("buffer");
            let mut i = 0usize;
            for y in 0..8 {
                for x in 0..8 {
                    let r = seed[i % 32];
                    let g = seed[(i + 1) % 32];
                    let b = seed[(i + 2) % 32];
                    buf.set_pixel(x, y, Rgba::opaque(r, g, b));
                    i += 1;
                }
            }
            let opts = FinishOptions::for_canvas(4, 4);
            let out = finish_frame(&buf, &opts).expect("finish");
            prop_assert_eq!((out.buffer.width(), out.buffer.height()), (4, 4));
            let set: Vec<Rgba> = out.palette.colors.iter().map(|e| e.color).collect();
            for y in 0..out.buffer.height() {
                for x in 0..out.buffer.width() {
                    let px = out.buffer.pixel(x, y).expect("pixel");
                    if px.a == 0 {
                        continue;
                    }
                    prop_assert!(set.iter().any(|c| c.r == px.r && c.g == px.g && c.b == px.b));
                }
            }
        }
    }

    // ── image-compare: a soft gradient through for_canvas vs a baseline ───

    /// Visual regression: a soft red→blue gradient run through
    /// `for_canvas(64, 64)` compared against a committed baseline PNG.
    /// Regenerate with `PIXHAUS_UPDATE_SNAPSHOTS=1 cargo test -p pixhaus-core
    /// finish_soft_gradient_matches_baseline` after an intentional change, and
    /// audit the new image in the PR.
    #[test]
    fn finish_soft_gradient_matches_baseline() {
        use std::path::Path;

        let width = 256u32;
        let height = 256u32;
        let mut buf = PixelBuffer::new(width, height).expect("buffer");
        for y in 0..height {
            for x in 0..width {
                #[allow(clippy::cast_possible_truncation)]
                let ramp = x * 255 / (width - 1);
                let red = (255 - ramp) as u8;
                let blue = ramp as u8;
                buf.set_pixel(x, y, Rgba::opaque(red, 40, blue));
            }
        }
        let out = finish_frame(&buf, &FinishOptions::for_canvas(64, 64)).expect("finish");
        let actual: image::RgbaImage =
            image::RgbaImage::from_raw(out.buffer.width(), out.buffer.height(), out.buffer.as_bytes().to_vec()).expect("buffer bytes form a valid RGBA image");

        let baseline_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/finish_soft_gradient.png");
        if std::env::var_os("PIXHAUS_UPDATE_SNAPSHOTS").is_some() {
            if let Some(dir) = baseline_path.parent() {
                std::fs::create_dir_all(dir).expect("create snapshot dir");
            }
            actual.save(&baseline_path).expect("write baseline png");
            return;
        }
        let baseline = image::open(&baseline_path)
            .expect("baseline png is present; regenerate with PIXHAUS_UPDATE_SNAPSHOTS=1")
            .to_rgba8();
        let result = image_compare::rgba_hybrid_compare(&actual, &baseline).expect("image dimensions match");
        assert!(result.score >= 0.999, "finished gradient diverged from baseline: score = {}", result.score);
    }
}
