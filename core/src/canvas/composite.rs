//! Multi-layer compositor.
//!
//! Walks a back-to-front list of source buffers and composites each
//! into a backdrop buffer using its blend mode and opacity. Row work
//! is fanned out across rayon's thread pool — the canvas is sliced
//! into stride-sized chunks and each row composites independently.

use rayon::prelude::*;

use crate::project::{BlendMode, Rgba};

use super::blend::blend as blend_pixel;
use super::buffer::{PixelBuffer, RGBA_BYTES_PER_PIXEL};
use super::error::{Error, Result};

/// One layer's contribution to a composite operation.
///
/// `visible == false` and `opacity == 0` short-circuit early: the
/// compositor skips the layer without touching the backdrop.
#[derive(Copy, Clone, Debug)]
pub struct LayerInput<'a> {
    /// Pixel data for this layer. Must have the same width/height as
    /// the backdrop the compositor is targeting.
    pub buffer: &'a PixelBuffer,
    /// Blend mode applied when this layer composites onto the
    /// backdrop.
    pub mode: BlendMode,
    /// Per-layer opacity multiplier in `0..=255`. Combines
    /// multiplicatively with `src.a` per Aseprite convention.
    pub opacity: u8,
    /// Whether the layer participates in the composite. Hidden layers
    /// are still passed through the API so call sites match the
    /// document's layer order.
    pub visible: bool,
}

impl LayerInput<'_> {
    /// Whether the layer can actually contribute pixels. The
    /// compositor uses this to skip no-op work.
    #[must_use]
    pub fn contributes(&self) -> bool {
        self.visible && self.opacity > 0 && !self.buffer.is_empty()
    }
}

/// Composites `layers` onto a fresh transparent backdrop sized
/// `(width, height)`. Layers are applied in order — index 0 is the
/// bottom of the stack.
///
/// # Errors
///
/// Returns [`Error::DimensionMismatch`] the first time a layer's
/// dimensions disagree with the backdrop. The backdrop is allocated
/// before validation, so the caller pays one allocation even on
/// mismatch — practical sizes make this cheap, and the up-front
/// allocation lets the rayon path stay branch-free per row.
pub fn composite_layers(width: u32, height: u32, layers: &[LayerInput<'_>]) -> Result<PixelBuffer> {
    let mut backdrop = PixelBuffer::new(width, height)?;
    for layer in layers {
        composite_onto(&mut backdrop, layer)?;
    }
    Ok(backdrop)
}

/// Composites a single source layer onto an existing backdrop in
/// place. Skips no-op layers (`!contributes()`).
///
/// # Errors
///
/// Returns [`Error::DimensionMismatch`] if `layer.buffer` doesn't
/// match `backdrop` in width and height.
pub fn composite_onto(backdrop: &mut PixelBuffer, layer: &LayerInput<'_>) -> Result<()> {
    if !layer.contributes() {
        return Ok(());
    }
    if backdrop.width() != layer.buffer.width() || backdrop.height() != layer.buffer.height() {
        return Err(Error::DimensionMismatch {
            bw: backdrop.width(),
            bh: backdrop.height(),
            sw: layer.buffer.width(),
            sh: layer.buffer.height(),
        });
    }
    if backdrop.is_empty() {
        return Ok(());
    }

    let mode = layer.mode;
    let opacity = layer.opacity;
    let width = backdrop.width() as usize;
    let row_bytes = width * RGBA_BYTES_PER_PIXEL;

    let dst_stride = backdrop.stride() as usize;
    let src_stride = layer.buffer.stride() as usize;

    let dst_bytes = backdrop.as_bytes_mut();
    let src_bytes = layer.buffer.as_bytes();

    dst_bytes
        .par_chunks_mut(dst_stride)
        .zip(src_bytes.par_chunks(src_stride))
        .for_each(|(d_row, s_row)| {
            blend_row(&mut d_row[..row_bytes], &s_row[..row_bytes], mode, opacity);
        });

    Ok(())
}

/// Pixel count above which [`composite_region`] fans rows across rayon.
/// Below it the rayon scheduling overhead outweighs the work, so a small
/// brush dab composites serially — the common per-move case.
const PARALLEL_MIN_PIXELS: u64 = 64 * 64;

/// Composites the rectangle `(x, y, w, h)` of `layers` directly into `dst`,
/// overwriting that rect with a fresh back-to-front composite (index 0 is the
/// bottom layer). Pixels outside the rect are untouched.
///
/// This is the drawing hot path's CPU half: after a brush dab edits a cel, the
/// shell recomposites only the dirty rect across layers, then uploads just that
/// rect. Work is bounded by the rect, not the canvas (the 8K constraint). Rows
/// are fanned across rayon once the rect is large enough to be worth it.
///
/// `layers` must match `dst` in width and height; rows/columns outside `dst`
/// are clamped away. Every layer is assumed full-canvas (cel position `(0,0)`),
/// matching the shell's slice cels.
// The enumerate index `i` counts rows within the rect, bounded by the canvas
// height (<= 8192), so `y + i as u32` never truncates.
#[allow(clippy::cast_possible_truncation)]
pub fn composite_region(dst: &mut PixelBuffer, layers: &[LayerInput<'_>], x: u32, y: u32, w: u32, h: u32) {
    if dst.is_empty() || w == 0 || h == 0 {
        return;
    }
    let x1 = (x + w).min(dst.width());
    let y1 = (y + h).min(dst.height());
    if x1 <= x || y1 <= y {
        return;
    }

    let area = u64::from(x1 - x) * u64::from(y1 - y);
    let stride = dst.stride() as usize;
    let start = y as usize * stride;
    let end = y1 as usize * stride;
    let region = &mut dst.as_bytes_mut()[start..end];

    if area >= PARALLEL_MIN_PIXELS {
        region.par_chunks_mut(stride).enumerate().for_each(|(i, row)| {
            composite_span(row, layers, y + i as u32, x, x1);
        });
    } else {
        for (i, row) in region.chunks_mut(stride).enumerate() {
            composite_span(row, layers, y + i as u32, x, x1);
        }
    }
}

/// Composites the `[x0, x1)` span of one row `py` from `layers` into `row`
/// (a single destination row's bytes). Each pixel starts transparent and each
/// contributing layer blends over the accumulator in order.
#[allow(clippy::cast_possible_truncation)] // px * 4 fits usize; canvas dims <= 8192
fn composite_span(row: &mut [u8], layers: &[LayerInput<'_>], py: u32, x0: u32, x1: u32) {
    for px in x0..x1 {
        let mut acc = Rgba::transparent();
        for layer in layers {
            if !layer.contributes() {
                continue;
            }
            let Some(src) = layer.buffer.pixel(px, py) else {
                continue;
            };
            if src.a == 0 {
                continue;
            }
            acc = blend_pixel(layer.mode, src, acc, layer.opacity);
        }
        let off = px as usize * RGBA_BYTES_PER_PIXEL;
        if let Some(slot) = row.get_mut(off..off + RGBA_BYTES_PER_PIXEL) {
            slot.copy_from_slice(&acc.to_array());
        }
    }
}

/// Blends one row of source bytes into one row of destination bytes
/// in place. Both slices must be `width * 4` bytes.
fn blend_row(dst: &mut [u8], src: &[u8], mode: BlendMode, opacity: u8) {
    debug_assert_eq!(dst.len(), src.len());
    debug_assert_eq!(dst.len() % RGBA_BYTES_PER_PIXEL, 0);

    for (d_chunk, s_chunk) in dst.chunks_exact_mut(RGBA_BYTES_PER_PIXEL).zip(src.chunks_exact(RGBA_BYTES_PER_PIXEL)) {
        let s = Rgba::new(s_chunk[0], s_chunk[1], s_chunk[2], s_chunk[3]);
        // Fast path: source pixel is fully transparent.
        if s.a == 0 {
            continue;
        }
        let d = Rgba::new(d_chunk[0], d_chunk[1], d_chunk[2], d_chunk[3]);
        let result = blend_pixel(mode, s, d, opacity);
        d_chunk[0] = result.r;
        d_chunk[1] = result.g;
        d_chunk[2] = result.b;
        d_chunk[3] = result.a;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, color: Rgba) -> PixelBuffer {
        PixelBuffer::filled(width, height, color).unwrap()
    }

    #[test]
    fn composite_no_layers_returns_transparent() {
        let out = composite_layers(4, 4, &[]).unwrap();
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(out.pixel(x, y), Some(Rgba::transparent()));
            }
        }
    }

    #[test]
    fn composite_single_opaque_layer_writes_through() {
        let red = Rgba::opaque(255, 0, 0);
        let layer = solid(4, 4, red);
        let out = composite_layers(
            4,
            4,
            &[LayerInput {
                buffer: &layer,
                mode: BlendMode::Normal,
                opacity: 255,
                visible: true,
            }],
        )
        .unwrap();
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(out.pixel(x, y), Some(red));
            }
        }
    }

    #[test]
    fn composite_skips_hidden_layer() {
        let red = Rgba::opaque(255, 0, 0);
        let layer = solid(4, 4, red);
        let out = composite_layers(
            4,
            4,
            &[LayerInput {
                buffer: &layer,
                mode: BlendMode::Normal,
                opacity: 255,
                visible: false,
            }],
        )
        .unwrap();
        assert_eq!(out.pixel(0, 0), Some(Rgba::transparent()));
    }

    #[test]
    fn composite_skips_zero_opacity_layer() {
        let layer = solid(4, 4, Rgba::opaque(255, 255, 0));
        let out = composite_layers(
            4,
            4,
            &[LayerInput {
                buffer: &layer,
                mode: BlendMode::Normal,
                opacity: 0,
                visible: true,
            }],
        )
        .unwrap();
        assert_eq!(out.pixel(0, 0), Some(Rgba::transparent()));
    }

    #[test]
    fn composite_layered_normal_top_replaces_bottom() {
        let bottom = solid(2, 2, Rgba::opaque(255, 0, 0));
        let top = solid(2, 2, Rgba::opaque(0, 255, 0));
        let out = composite_layers(
            2,
            2,
            &[
                LayerInput {
                    buffer: &bottom,
                    mode: BlendMode::Normal,
                    opacity: 255,
                    visible: true,
                },
                LayerInput {
                    buffer: &top,
                    mode: BlendMode::Normal,
                    opacity: 255,
                    visible: true,
                },
            ],
        )
        .unwrap();
        assert_eq!(out.pixel(0, 0), Some(Rgba::opaque(0, 255, 0)));
    }

    #[test]
    fn composite_multiply_darkens() {
        let bottom = solid(2, 2, Rgba::opaque(200, 200, 200));
        let top = solid(2, 2, Rgba::opaque(128, 128, 128));
        let out = composite_layers(
            2,
            2,
            &[
                LayerInput {
                    buffer: &bottom,
                    mode: BlendMode::Normal,
                    opacity: 255,
                    visible: true,
                },
                LayerInput {
                    buffer: &top,
                    mode: BlendMode::Multiply,
                    opacity: 255,
                    visible: true,
                },
            ],
        )
        .unwrap();
        let pixel = out.pixel(0, 0).unwrap();
        // Multiply 200*128/255 ≈ 100
        assert_eq!(pixel.r, 100);
        assert_eq!(pixel.g, 100);
        assert_eq!(pixel.b, 100);
        assert_eq!(pixel.a, 255);
    }

    #[test]
    fn composite_rejects_dimension_mismatch() {
        let small = solid(2, 2, Rgba::opaque(255, 0, 0));
        let mut backdrop = PixelBuffer::new(4, 4).unwrap();
        let err = composite_onto(
            &mut backdrop,
            &LayerInput {
                buffer: &small,
                mode: BlendMode::Normal,
                opacity: 255,
                visible: true,
            },
        )
        .unwrap_err();
        assert_eq!(err, Error::DimensionMismatch { bw: 4, bh: 4, sw: 2, sh: 2 });
    }

    #[test]
    fn region_matches_full_composite_serial_and_parallel() {
        // Two layers (one semi-transparent checker over an opaque base) so the
        // region path must accumulate blends, not just copy.
        let bottom = solid(128, 128, Rgba::opaque(200, 50, 50));
        let mut top = PixelBuffer::new(128, 128).unwrap();
        for y in 0..128 {
            for x in 0..128 {
                if (x + y) % 2 == 0 {
                    top.set_pixel(x, y, Rgba::new(0, 0, 255, 128));
                }
            }
        }
        let layers = [
            LayerInput {
                buffer: &bottom,
                mode: BlendMode::Normal,
                opacity: 255,
                visible: true,
            },
            LayerInput {
                buffer: &top,
                mode: BlendMode::Normal,
                opacity: 200,
                visible: true,
            },
        ];
        let full = composite_layers(128, 128, &layers).unwrap();

        // A small rect drives the serial path; the full canvas (>= the parallel
        // threshold) drives the rayon path. Both must equal the full composite.
        for (x, y, w, h) in [(3u32, 5u32, 7u32, 9u32), (0, 0, 128, 128)] {
            let mut dst = PixelBuffer::new(128, 128).unwrap();
            composite_region(&mut dst, &layers, x, y, w, h);
            for py in y..y + h {
                for px in x..x + w {
                    assert_eq!(dst.pixel(px, py), full.pixel(px, py), "({px},{py}) for rect ({x},{y},{w},{h})");
                }
            }
        }
    }

    #[test]
    fn region_outside_rect_is_untouched() {
        let layer = solid(16, 16, Rgba::opaque(255, 0, 0));
        let layers = [LayerInput {
            buffer: &layer,
            mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
        }];
        let mut dst = PixelBuffer::new(16, 16).unwrap();
        composite_region(&mut dst, &layers, 4, 4, 4, 4);
        // Inside the rect: composited red. Outside: still transparent.
        assert_eq!(dst.pixel(5, 5), Some(Rgba::opaque(255, 0, 0)));
        assert_eq!(dst.pixel(0, 0), Some(Rgba::transparent()));
        assert_eq!(dst.pixel(15, 15), Some(Rgba::transparent()));
    }

    #[test]
    fn composite_handles_padded_stride() {
        // Backdrop with 8-byte row padding.
        let mut padded = PixelBuffer::from_raw(2, 2, 16, vec![0u8; 32]).expect("padded buffer constructs");
        let layer = solid(2, 2, Rgba::opaque(10, 20, 30));
        composite_onto(
            &mut padded,
            &LayerInput {
                buffer: &layer,
                mode: BlendMode::Normal,
                opacity: 255,
                visible: true,
            },
        )
        .unwrap();
        for y in 0..2 {
            for x in 0..2 {
                assert_eq!(padded.pixel(x, y), Some(Rgba::opaque(10, 20, 30)));
            }
        }
    }
}
