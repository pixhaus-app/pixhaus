//! Pencil stroke rasterization and brush footprint painting.
//!
//! All coordinates are in canvas pixels (integer grid). Fractional
//! screen coordinates from pointer events should be rounded to the
//! nearest integer before calling these functions.

// `i32 as u32` casts are all bounds-checked (x >= 0) before the cast;
// `u32 as i32` casts are safe for any canvas < 2^31 pixels wide.
// `f32 as i32` truncates fractional parts intentionally (round() is called
// before the cast so the value is already integral). x, y, r, d, w, h are
// idiomatic in pixel-art coordinate math. draw_line takes 8 params because
// the Bresenham algorithm is fundamentally a 4-coordinate + style operation.
#![allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    clippy::too_many_arguments
)]

use crate::canvas::PixelBuffer;
use crate::canvas::tools::dither::{DitherPattern, dither_allows};
use crate::project::Rgba;

/// Brush footprint shape applied at each painted pixel.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum BrushShape {
    /// Single canvas pixel regardless of size.
    #[default]
    Pixel,
    /// Filled circle; `size` is the diameter in pixels.
    Circle,
    /// Filled square; `size` is the side length in pixels.
    Square,
}

/// Paint the brush footprint centred at `(cx, cy)`.
///
/// For [`BrushShape::Pixel`] the `size` parameter is ignored.
/// Out-of-bounds regions are clipped silently.
pub fn paint_brush(buf: &mut PixelBuffer, cx: i32, cy: i32, color: Rgba, shape: BrushShape, size: u32) {
    match shape {
        BrushShape::Pixel => {
            if cx >= 0 && cy >= 0 {
                buf.set_pixel(cx as u32, cy as u32, color);
            }
        }
        BrushShape::Circle => {
            let d = size.max(1) as i32;
            let r = d / 2;
            let r_sq = r * r;
            let w = buf.width() as i32;
            let h = buf.height() as i32;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dy * dy <= r_sq {
                        let x = cx + dx;
                        let y = cy + dy;
                        if x >= 0 && y >= 0 && x < w && y < h {
                            buf.set_pixel(x as u32, y as u32, color);
                        }
                    }
                }
            }
        }
        BrushShape::Square => {
            let d = size.max(1) as i32;
            let half = d / 2;
            let x0 = cx - half;
            let y0 = cy - half;
            let x1 = x0 + d - 1;
            let y1 = y0 + d - 1;
            let w = buf.width() as i32;
            let h = buf.height() as i32;
            for y in y0.max(0)..=y1.min(h - 1) {
                for x in x0.max(0)..=x1.min(w - 1) {
                    buf.set_pixel(x as u32, y as u32, color);
                }
            }
        }
    }
}

/// Paint the brush footprint centred at `(cx, cy)`, writing only the
/// pixels the dither `pattern` allows, each blended at `opacity`.
///
/// Iterates the footprint via [`brush_covers`] and consults
/// [`dither_allows`] per covered offset using the pixel's canvas-absolute
/// coordinate, so the mask is pan-stable across separate stamps. Each
/// allowed pixel is written through [`PixelBuffer::set_pixel_blended`], so
/// `opacity == 255` overwrites (the fast path inside `set_pixel_blended`)
/// and `opacity == 0` is a no-op.
///
/// [`paint_brush`] stays the fast path for solid, full-opacity strokes;
/// this variant exists only when a mask or reduced opacity is in play.
/// Out-of-bounds pixels are clipped silently. With [`DitherPattern::None`]
/// and `opacity == 255` it is byte-identical to [`paint_brush`].
///
/// # Per-stroke opacity, not per-dab
///
/// This stamps a single dab. A freehand drag overlaps dabs, so blending
/// each dab at a reduced opacity would compound — the overlap would darken.
/// The Aseprite semantic is per-stroke: opacity applies once over the whole
/// stroke footprint. The shell honours it not here but at commit time, by
/// keeping a full-strength preview during the drag and redrawing once from
/// the pre-stroke `before` snapshot — blending the union footprint at the
/// chosen opacity over `before`, bounded by the dirty rect already tracked.
/// Callers that need a one-shot opaque-over-backdrop dab (a shape edge, a
/// single click, or that commit redraw) can use this directly.
pub fn paint_brush_masked(buf: &mut PixelBuffer, cx: i32, cy: i32, color: Rgba, shape: BrushShape, size: u32, pattern: DitherPattern, opacity: u8) {
    // The covered offsets fit inside the brush's bounding box. Pixel is a
    // single point; Circle and Square span [-reach, reach] around the
    // centre. The +1 guards the odd/even rounding in brush_covers.
    let reach = match shape {
        BrushShape::Pixel => 0,
        BrushShape::Circle | BrushShape::Square => (size.max(1) as i32) / 2 + 1,
    };
    let w = buf.width() as i32;
    let h = buf.height() as i32;

    for dy in -reach..=reach {
        for dx in -reach..=reach {
            if !brush_covers(shape, size, dx, dy) {
                continue;
            }
            let x = cx + dx;
            let y = cy + dy;
            if x < 0 || y < 0 || x >= w || y >= h {
                continue;
            }
            if !dither_allows(pattern, x, y) {
                continue;
            }
            buf.set_pixel_blended(x as u32, y as u32, color, opacity);
        }
    }
}

/// Paint the brush footprint centred at `(cx, cy)`, blending every covered
/// pixel at `opacity` (no dither mask).
///
/// A thin alias for [`paint_brush_masked`] with [`DitherPattern::None`].
/// At `opacity == 255` it overwrites identically to [`paint_brush`]; at
/// `opacity == 0` it is a no-op. See the per-stroke note on
/// [`paint_brush_masked`] for the freehand-drag semantic.
pub fn paint_brush_blended(buf: &mut PixelBuffer, cx: i32, cy: i32, color: Rgba, shape: BrushShape, size: u32, opacity: u8) {
    paint_brush_masked(buf, cx, cy, color, shape, size, DitherPattern::None, opacity);
}

/// Whether the brush footprint covers the offset `(dx, dy)` from its centre.
///
/// The single source of truth for the brush shape: [`paint_brush`] paints
/// exactly the offsets for which this returns `true`, and the editor's cursor
/// gizmo traces the boundary of the same set, so the on-canvas outline always
/// matches the painted pixels. A test below pins the two together.
#[must_use]
pub fn brush_covers(shape: BrushShape, size: u32, dx: i32, dy: i32) -> bool {
    match shape {
        BrushShape::Pixel => dx == 0 && dy == 0,
        BrushShape::Circle => {
            let r = (size.max(1) as i32) / 2;
            dx * dx + dy * dy <= r * r
        }
        BrushShape::Square => {
            let d = size.max(1) as i32;
            let half = d / 2;
            let lo = -half;
            let hi = -half + d - 1;
            dx >= lo && dx <= hi && dy >= lo && dy <= hi
        }
    }
}

/// Draw a Bresenham line from `(x0, y0)` to `(x1, y1)`, painting the
/// brush footprint at every pixel on the line. Returns the number of
/// brush stamps placed (one per line pixel, including both endpoints).
pub fn draw_line(buf: &mut PixelBuffer, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba, shape: BrushShape, size: u32) -> usize {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    let mut x = x0;
    let mut y = y0;
    let mut stamps = 0;

    loop {
        paint_brush(buf, x, y, color, shape, size);
        stamps += 1;
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }

    stamps
}

/// Stamp the brush along `points`, optionally bridging from a prior point.
///
/// This is the incremental core of stroke rasterization: it paints only
/// the supplied `points`, connecting them with Bresenham lines, with **no**
/// pixel-perfect post-pass. Pass `from = Some(prev)` to bridge a line from
/// the last point of an earlier segment to the first of `points` so a
/// freehand drag stays continuous across batches; pass `from = None` for a
/// fresh segment (the first point is stamped on its own).
///
/// Because [`PixelBuffer::set_pixel`] overwrites rather than blends,
/// stamping is idempotent: feeding a point list in chunks produces the same
/// pixels as one [`draw_stroke`] call over the concatenation (sans the
/// pixel-perfect pass). Returns the number of `paint_brush` stamps performed
/// — the per-segment work, used to assert linear (not quadratic) scaling.
pub fn stamp_segment(buf: &mut PixelBuffer, from: Option<[f32; 2]>, points: &[[f32; 2]], color: Rgba, shape: BrushShape, size: u32) -> usize {
    if points.is_empty() {
        return 0;
    }

    let mut stamps = 0;
    let (mut px, mut py, rest) = if let Some(prev) = from {
        (prev[0].round() as i32, prev[1].round() as i32, points)
    } else {
        let first = points[0];
        let fx = first[0].round() as i32;
        let fy = first[1].round() as i32;
        paint_brush(buf, fx, fy, color, shape, size);
        stamps += 1;
        (fx, fy, &points[1..])
    };

    for p in rest {
        let nx = p[0].round() as i32;
        let ny = p[1].round() as i32;
        stamps += draw_line(buf, px, py, nx, ny, color, shape, size);
        px = nx;
        py = ny;
    }

    stamps
}

/// Bresenham line from `(x0, y0)` to `(x1, y1)`, stamping a dither-masked,
/// `opacity`-blended brush at every line pixel.
///
/// The mask-and-blend counterpart of [`draw_line`]. With
/// [`DitherPattern::None`] and `opacity == 255` it paints the same pixels as
/// [`draw_line`]. Below 255 it blends per dab, so dabs that overlap — wide
/// brushes always, and the shared endpoint between joined segments — compound.
/// Use it for the dither gate at full opacity, or for a single straight edge;
/// the per-stroke commit redraw blends each *unique* footprint pixel once over
/// `before` via [`PixelBuffer::set_pixel_blended`] (see [`paint_brush_masked`]).
/// Returns the brush-stamp count.
pub fn draw_line_masked(
    buf: &mut PixelBuffer,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: Rgba,
    shape: BrushShape,
    size: u32,
    pattern: DitherPattern,
    opacity: u8,
) -> usize {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    let mut x = x0;
    let mut y = y0;
    let mut stamps = 0;

    loop {
        paint_brush_masked(buf, x, y, color, shape, size, pattern, opacity);
        stamps += 1;
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }

    stamps
}

/// Stamp a dither-masked, `opacity`-blended brush along `points`, bridging
/// from `from` like [`stamp_segment`].
///
/// The mask-and-blend counterpart of [`stamp_segment`]; with
/// [`DitherPattern::None`] and `opacity == 255` it stamps the same pixels.
/// Below 255 it blends per dab, so overlapping dabs — wide brushes, and the
/// shared point where segments join — compound. That is why the per-stroke
/// commit redraw does NOT use this for the blend: it blends each *unique*
/// footprint pixel once over the pre-stroke `before` via
/// [`PixelBuffer::set_pixel_blended`] (see [`paint_brush_masked`]). Reach for
/// this for the dither gate at full opacity, where overwrite is idempotent and
/// the seam double-stamp is harmless. Returns the brush-stamp count.
pub fn stamp_segment_masked(
    buf: &mut PixelBuffer,
    from: Option<[f32; 2]>,
    points: &[[f32; 2]],
    color: Rgba,
    shape: BrushShape,
    size: u32,
    pattern: DitherPattern,
    opacity: u8,
) -> usize {
    if points.is_empty() {
        return 0;
    }

    let mut stamps = 0;
    let (mut px, mut py, rest) = if let Some(prev) = from {
        (prev[0].round() as i32, prev[1].round() as i32, points)
    } else {
        let first = points[0];
        let fx = first[0].round() as i32;
        let fy = first[1].round() as i32;
        paint_brush_masked(buf, fx, fy, color, shape, size, pattern, opacity);
        stamps += 1;
        (fx, fy, &points[1..])
    };

    for p in rest {
        let nx = p[0].round() as i32;
        let ny = p[1].round() as i32;
        stamps += draw_line_masked(buf, px, py, nx, ny, color, shape, size, pattern, opacity);
        px = nx;
        py = ny;
    }

    stamps
}

/// Draw a freehand stroke through `points` (canvas-space `[x, y]` pairs).
///
/// Consecutive points are connected with Bresenham lines. When
/// `pixel_perfect` is true, a post-pass removes corner artifacts that
/// arise from diagonal steps: pixels with no orthogonal stroke neighbour
/// but two diagonal stroke neighbours are erased.
pub fn draw_stroke(buf: &mut PixelBuffer, points: &[[f32; 2]], color: Rgba, shape: BrushShape, size: u32, pixel_perfect: bool) {
    stamp_segment(buf, None, points, color, shape, size);

    if pixel_perfect && shape == BrushShape::Pixel {
        remove_pixel_perfect_artifacts(buf, color);
    }
}

/// Remove corner artifacts from a pixel-perfect pencil stroke.
///
/// A pixel is a corner artifact when it has no orthogonal stroke
/// neighbours but exactly two diagonal stroke neighbours — it sits at a
/// bent corner and makes the line look "doubled". Removing it gives the
/// clean pixel-art diagonal look Aseprite produces.
fn remove_pixel_perfect_artifacts(buf: &mut PixelBuffer, stroke_color: Rgba) {
    let w = buf.width() as i32;
    let h = buf.height() as i32;
    let mut to_erase: Vec<(u32, u32)> = Vec::new();

    for y in 0..h {
        for x in 0..w {
            if buf.pixel(x as u32, y as u32) != Some(stroke_color) {
                continue;
            }
            let ortho = count_neighbors(buf, x, y, w, h, stroke_color, &[(-1, 0), (1, 0), (0, -1), (0, 1)]);
            let diag = count_neighbors(buf, x, y, w, h, stroke_color, &[(-1, -1), (1, -1), (-1, 1), (1, 1)]);
            if ortho == 0 && diag >= 2 {
                to_erase.push((x as u32, y as u32));
            }
        }
    }

    for (x, y) in to_erase {
        buf.set_pixel(x, y, Rgba::transparent());
    }
}

fn count_neighbors(buf: &PixelBuffer, x: i32, y: i32, w: i32, h: i32, color: Rgba, offsets: &[(i32, i32)]) -> usize {
    offsets
        .iter()
        .filter(|&&(dx, dy)| {
            let nx = x + dx;
            let ny = y + dy;
            nx >= 0 && ny >= 0 && nx < w && ny < h && buf.pixel(nx as u32, ny as u32) == Some(color)
        })
        .count()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::canvas::PixelBuffer;
    use crate::project::Rgba;
    use proptest::prelude::*;

    fn transparent_buf(w: u32, h: u32) -> PixelBuffer {
        PixelBuffer::new(w, h).unwrap()
    }

    #[test]
    fn paint_pixel_single_pixel() {
        let mut buf = transparent_buf(8, 8);
        let red = Rgba::opaque(255, 0, 0);
        paint_brush(&mut buf, 3, 4, red, BrushShape::Pixel, 1);
        assert_eq!(buf.pixel(3, 4), Some(red));
        assert_eq!(buf.pixel(2, 4), Some(Rgba::transparent()));
    }

    #[test]
    fn paint_circle_fills_disk() {
        let mut buf = transparent_buf(16, 16);
        let blue = Rgba::opaque(0, 0, 255);
        // Size 5 circle at (7,7): radius 2, so pixel (7,7) is painted.
        paint_brush(&mut buf, 7, 7, blue, BrushShape::Circle, 5);
        assert_eq!(buf.pixel(7, 7), Some(blue));
        // Corners of the 5x5 bounding box should NOT be painted (outside circle).
        assert_ne!(buf.pixel(5, 5), Some(blue));
    }

    #[test]
    fn paint_square_fills_box() {
        let mut buf = transparent_buf(16, 16);
        let green = Rgba::opaque(0, 255, 0);
        paint_brush(&mut buf, 8, 8, green, BrushShape::Square, 4);
        // centre pixel
        assert_eq!(buf.pixel(8, 8), Some(green));
        // corner of the 4x4 box (half=2, so x0=6, y0=6, x1=9, y1=9)
        assert_eq!(buf.pixel(6, 6), Some(green));
        assert_eq!(buf.pixel(9, 9), Some(green));
    }

    #[test]
    fn draw_line_connects_two_points() {
        let mut buf = transparent_buf(16, 16);
        let white = Rgba::opaque(255, 255, 255);
        draw_line(&mut buf, 0, 0, 7, 7, white, BrushShape::Pixel, 1);
        // Diagonal: each step should be painted
        assert_eq!(buf.pixel(0, 0), Some(white));
        assert_eq!(buf.pixel(7, 7), Some(white));
    }

    #[test]
    fn draw_stroke_single_point() {
        let mut buf = transparent_buf(8, 8);
        let c = Rgba::opaque(100, 100, 100);
        draw_stroke(&mut buf, &[[3.0, 3.0]], c, BrushShape::Pixel, 1, false);
        assert_eq!(buf.pixel(3, 3), Some(c));
    }

    #[test]
    fn draw_stroke_two_points_forms_line() {
        let mut buf = transparent_buf(16, 16);
        let c = Rgba::opaque(200, 200, 200);
        draw_stroke(&mut buf, &[[0.0, 0.0], [10.0, 0.0]], c, BrushShape::Pixel, 1, false);
        // Horizontal line: all pixels from x=0 to x=10 should be painted
        for x in 0u32..=10 {
            assert_eq!(buf.pixel(x, 0), Some(c), "pixel ({x}, 0) should be painted");
        }
    }

    #[test]
    fn paint_brush_clamps_to_buffer() {
        let mut buf = transparent_buf(4, 4);
        let c = Rgba::opaque(1, 2, 3);
        // Painting at a position that would overflow the buffer should not panic.
        paint_brush(&mut buf, 3, 3, c, BrushShape::Square, 6);
        // Pixels within bounds should be painted; no panic from out-of-bounds.
        assert_eq!(buf.pixel(3, 3), Some(c));
    }

    #[test]
    fn brush_covers_matches_paint_brush() {
        // The cursor gizmo traces `brush_covers`; it must agree with the pixels
        // `paint_brush` actually sets, for every shape and size.
        let color = Rgba::opaque(200, 30, 30);
        let (cx, cy) = (100, 100);
        for shape in [BrushShape::Pixel, BrushShape::Circle, BrushShape::Square] {
            for size in [1u32, 2, 3, 5, 8, 40] {
                let mut buf = transparent_buf(200, 200);
                paint_brush(&mut buf, cx, cy, color, shape, size);
                let range = size as i32 + 2;
                for dy in -range..=range {
                    for dx in -range..=range {
                        let painted = buf.pixel((cx + dx) as u32, (cy + dy) as u32) == Some(color);
                        assert_eq!(painted, brush_covers(shape, size, dx, dy), "{shape:?} size {size} at ({dx},{dy})");
                    }
                }
            }
        }
    }

    /// Stamps `points` in `chunk`-sized batches, bridging each batch to the
    /// previous one — the incremental pattern the stroke session uses.
    fn stamp_chunked(buf: &mut PixelBuffer, points: &[[f32; 2]], chunk: usize, color: Rgba, shape: BrushShape, size: u32) -> usize {
        let mut last: Option<[f32; 2]> = None;
        let mut total = 0;
        for batch in points.chunks(chunk) {
            total += stamp_segment(buf, last, batch, color, shape, size);
            if let Some(p) = batch.last() {
                last = Some(*p);
            }
        }
        total
    }

    #[test]
    fn stamp_segment_chunked_matches_one_shot() {
        // The incremental hot path must be byte-identical to a single
        // draw_stroke over the same points (non-pixel-perfect), because
        // set_pixel overwrites. Check every shape and a couple of chunkings.
        let c = Rgba::opaque(180, 60, 220);
        let points: Vec<[f32; 2]> = (0..40).map(|i| [3.0 + i as f32 * 0.7, 5.0 + (i as f32 * 0.5).sin() * 9.0]).collect();

        for shape in [BrushShape::Pixel, BrushShape::Circle, BrushShape::Square] {
            for size in [1u32, 8] {
                let mut one_shot = transparent_buf(64, 64);
                draw_stroke(&mut one_shot, &points, c, shape, size, false);

                for chunk in [1usize, 3, 7] {
                    let mut incremental = transparent_buf(64, 64);
                    stamp_chunked(&mut incremental, &points, chunk, c, shape, size);
                    assert_eq!(
                        incremental.as_bytes(),
                        one_shot.as_bytes(),
                        "chunk={chunk} shape={shape:?} size={size} diverged from one-shot",
                    );
                }
            }
        }
    }

    #[test]
    fn stamp_segment_work_is_linear_in_points() {
        // Regression trap for the O(n^2) re-rasterization: building a stroke
        // point-by-point must cost work proportional to stroke length, not to
        // its square. A horizontal pixel line of N points spaced 1px apart
        // stamps ~2 pixels per 1px segment, so the total is ~2N. The old
        // "re-stamp all accumulated points every extend" approach summed to
        // ~N^2/2 and would blow past this linear bound.
        const N: usize = 500;
        let c = Rgba::opaque(10, 20, 30);
        let points: Vec<[f32; 2]> = (0..N).map(|i| [i as f32, 0.0]).collect();

        let mut buf = PixelBuffer::new(N as u32, 1).unwrap();
        let total = stamp_chunked(&mut buf, &points, 1, c, BrushShape::Pixel, 1);

        // Generous linear ceiling (≈2N expected). A quadratic regression at
        // N=500 would produce ~125_000 stamps, far above this.
        assert!(
            total <= 3 * N,
            "stamp work {total} exceeds linear bound {} for {N} points — possible O(n^2) regression",
            3 * N,
        );
    }

    #[test]
    fn masked_checker_fill_leaves_about_half() {
        // A checker-masked fill over a region writes ~half the pixels. The
        // count is exact for an even-area region: a WxH block with W*H even
        // has exactly W*H/2 even-parity cells.
        let color = Rgba::opaque(255, 255, 255);
        let (w, h) = (32u32, 32u32);
        let mut buf = transparent_buf(w, h);

        // Stamp every pixel with a single-pixel masked brush.
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                paint_brush_masked(&mut buf, x, y, color, BrushShape::Pixel, 1, DitherPattern::Checker, 255);
            }
        }

        let written = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|&(x, y)| buf.pixel(x, y) == Some(color))
            .count();
        let total = (w * h) as usize;
        // Exactly half for an even-area region.
        assert_eq!(written, total / 2, "checker fill should write exactly half of {total} pixels");
    }

    #[test]
    fn masked_checker_writes_only_allowed_pixels() {
        // Within the footprint, every written pixel must be checker-allowed.
        let color = Rgba::opaque(10, 200, 30);
        let mut buf = transparent_buf(32, 32);
        paint_brush_masked(&mut buf, 16, 16, color, BrushShape::Square, 8, DitherPattern::Checker, 255);

        for y in 0..32i32 {
            for x in 0..32i32 {
                if buf.pixel(x as u32, y as u32) == Some(color) {
                    assert!(dither_allows(DitherPattern::Checker, x, y), "wrote disallowed pixel at ({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn paint_brush_blended_at_255_matches_paint_brush() {
        let color = Rgba::opaque(200, 30, 30);
        for shape in [BrushShape::Pixel, BrushShape::Circle, BrushShape::Square] {
            for size in [1u32, 3, 8] {
                let mut solid = transparent_buf(64, 64);
                paint_brush(&mut solid, 30, 30, color, shape, size);
                let mut blended = transparent_buf(64, 64);
                paint_brush_blended(&mut blended, 30, 30, color, shape, size, 255);
                assert_eq!(solid.as_bytes(), blended.as_bytes(), "blended 255 diverged at shape={shape:?} size={size}");
            }
        }
    }

    #[test]
    fn paint_brush_blended_at_zero_is_noop() {
        let backdrop = Rgba::opaque(10, 20, 30);
        let untouched = PixelBuffer::filled(32, 32, backdrop).unwrap();
        let mut buf = untouched.clone();
        paint_brush_blended(&mut buf, 16, 16, Rgba::opaque(255, 0, 0), BrushShape::Circle, 6, 0);
        assert_eq!(buf.as_bytes(), untouched.as_bytes(), "opacity 0 brush must be a no-op");
    }

    #[test]
    fn paint_brush_blended_at_128_blends_each_pixel() {
        let backdrop = Rgba::opaque(20, 80, 160);
        let src = Rgba::opaque(200, 40, 60);
        let expected = crate::canvas::blend::blend_normal(src, backdrop, 128);
        let mut buf = PixelBuffer::filled(8, 8, backdrop).unwrap();
        paint_brush_blended(&mut buf, 4, 4, src, BrushShape::Square, 3, 128);
        // Centre is covered, so it should hold the half-strength blend.
        assert_eq!(buf.pixel(4, 4), Some(expected));
        // A pixel well outside the footprint keeps the backdrop.
        assert_eq!(buf.pixel(0, 0), Some(backdrop));
    }

    #[test]
    fn draw_line_masked_at_255_matches_draw_line() {
        let color = Rgba::opaque(255, 255, 255);
        let mut solid = transparent_buf(32, 32);
        draw_line(&mut solid, 2, 3, 28, 19, color, BrushShape::Pixel, 1);
        let mut masked = transparent_buf(32, 32);
        draw_line_masked(&mut masked, 2, 3, 28, 19, color, BrushShape::Pixel, 1, DitherPattern::None, 255);
        assert_eq!(solid.as_bytes(), masked.as_bytes(), "masked line at None/255 diverged from draw_line");
    }

    #[test]
    fn stamp_segment_masked_at_255_matches_stamp_segment() {
        let color = Rgba::opaque(180, 60, 220);
        let points: Vec<[f32; 2]> = (0..20).map(|i| [3.0 + i as f32 * 1.3, 5.0 + (i as f32 * 0.5).cos() * 7.0]).collect();
        for shape in [BrushShape::Pixel, BrushShape::Circle, BrushShape::Square] {
            for size in [1u32, 5] {
                let mut solid = transparent_buf(64, 64);
                stamp_segment(&mut solid, None, &points, color, shape, size);
                let mut masked = transparent_buf(64, 64);
                stamp_segment_masked(&mut masked, None, &points, color, shape, size, DitherPattern::None, 255);
                assert_eq!(solid.as_bytes(), masked.as_bytes(), "masked segment diverged at shape={shape:?} size={size}");
            }
        }
    }

    #[test]
    fn stamp_segment_masked_applies_dither_gate() {
        // The masked path gates each dab through the dither pattern. At full
        // opacity (the seam-safe case — overwrite is idempotent, so the start
        // pixel painted twice is harmless), a checker stroke leaves the source
        // only on checker-allowed cells and the backdrop everywhere else.
        let backdrop = Rgba::opaque(20, 80, 160);
        let src = Rgba::opaque(200, 40, 60);

        let mut buf = PixelBuffer::filled(16, 1, backdrop).unwrap();
        stamp_segment_masked(
            &mut buf,
            None,
            &[[0.0, 0.0], [15.0, 0.0]],
            src,
            BrushShape::Pixel,
            1,
            DitherPattern::Checker,
            255,
        );

        for x in 0u32..16 {
            let allowed = dither_allows(DitherPattern::Checker, x as i32, 0);
            let want = if allowed { src } else { backdrop };
            assert_eq!(buf.pixel(x, 0), Some(want), "pixel ({x}, 0) mismatch (checker allowed = {allowed})");
        }
    }

    #[test]
    fn per_stroke_commit_over_before_blends_union_once() {
        // The per-stroke-not-per-dab rule, the way the plan has the shell enforce
        // it: a live drag accumulates FULL-strength dabs, then commit redraws once
        // from the pre-stroke `before`, blending each unique footprint pixel a
        // single time via set_pixel_blended over the union. Two overlapping dabs
        // must leave one 128 blend on the overlap, never a doubled one. This is
        // why the shell redraws from `before` rather than blending dab-by-dab —
        // stamp_segment_masked itself double-stamps seam pixels and would compound.
        let backdrop = Rgba::opaque(20, 80, 160);
        let src = Rgba::opaque(200, 40, 60);
        let opacity = 128u8;
        let expected = crate::canvas::blend::blend_normal(src, backdrop, opacity);

        // 1. Live preview: accumulate the stroke at full strength.
        let mut live = PixelBuffer::filled(16, 1, backdrop).unwrap();
        stamp_segment(&mut live, None, &[[2.0, 0.0], [10.0, 0.0]], src, BrushShape::Pixel, 1);
        stamp_segment(&mut live, Some([10.0, 0.0]), &[[6.0, 0.0]], src, BrushShape::Pixel, 1);

        // 2. The union footprint is every pixel the preview changed off `before`.
        let before = PixelBuffer::filled(16, 1, backdrop).unwrap();
        let union: Vec<(u32, u32)> = (0..16).filter(|&x| live.pixel(x, 0) != before.pixel(x, 0)).map(|x| (x, 0u32)).collect();

        // 3. Commit: blend each union pixel exactly once over `before`.
        let mut committed = before.clone();
        for &(x, y) in &union {
            committed.set_pixel_blended(x, y, src, opacity);
        }

        for &(x, y) in &union {
            assert_eq!(committed.pixel(x, y), Some(expected), "union pixel ({x}, {y}) compounded beyond a single blend");
        }
        // Pixels outside the union keep the backdrop.
        assert_eq!(committed.pixel(0, 0), Some(backdrop));
    }

    proptest! {
        /// `paint_brush_masked` with `None` and full opacity is byte-identical
        /// to `paint_brush` for every shape, size, and position.
        #[test]
        fn masked_none_matches_paint_brush(
            shape_idx in 0usize..3,
            size in 1u32..40,
            cx in 0i32..64,
            cy in 0i32..64,
        ) {
            let shape = [BrushShape::Pixel, BrushShape::Circle, BrushShape::Square][shape_idx];
            let color = Rgba::opaque(123, 45, 67);

            let mut a = transparent_buf(64, 64);
            paint_brush(&mut a, cx, cy, color, shape, size);

            let mut b = transparent_buf(64, 64);
            paint_brush_masked(&mut b, cx, cy, color, shape, size, DitherPattern::None, 255);

            prop_assert_eq!(a.as_bytes(), b.as_bytes(), "masked None diverged at shape={:?} size={} ({},{})", shape, size, cx, cy);
        }
    }
}
