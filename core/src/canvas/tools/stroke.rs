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
pub fn paint_brush(
    buf: &mut PixelBuffer,
    cx: i32,
    cy: i32,
    color: Rgba,
    shape: BrushShape,
    size: u32,
) {
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

/// Draw a Bresenham line from `(x0, y0)` to `(x1, y1)`, painting the
/// brush footprint at every pixel on the line. Returns the number of
/// brush stamps placed (one per line pixel, including both endpoints).
pub fn draw_line(
    buf: &mut PixelBuffer,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: Rgba,
    shape: BrushShape,
    size: u32,
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
pub fn stamp_segment(
    buf: &mut PixelBuffer,
    from: Option<[f32; 2]>,
    points: &[[f32; 2]],
    color: Rgba,
    shape: BrushShape,
    size: u32,
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

/// Draw a freehand stroke through `points` (canvas-space `[x, y]` pairs).
///
/// Consecutive points are connected with Bresenham lines. When
/// `pixel_perfect` is true, a post-pass removes corner artifacts that
/// arise from diagonal steps: pixels with no orthogonal stroke neighbour
/// but two diagonal stroke neighbours are erased.
pub fn draw_stroke(
    buf: &mut PixelBuffer,
    points: &[[f32; 2]],
    color: Rgba,
    shape: BrushShape,
    size: u32,
    pixel_perfect: bool,
) {
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
            let ortho = count_neighbors(
                buf,
                x,
                y,
                w,
                h,
                stroke_color,
                &[(-1, 0), (1, 0), (0, -1), (0, 1)],
            );
            let diag = count_neighbors(
                buf,
                x,
                y,
                w,
                h,
                stroke_color,
                &[(-1, -1), (1, -1), (-1, 1), (1, 1)],
            );
            if ortho == 0 && diag >= 2 {
                to_erase.push((x as u32, y as u32));
            }
        }
    }

    for (x, y) in to_erase {
        buf.set_pixel(x, y, Rgba::transparent());
    }
}

fn count_neighbors(
    buf: &PixelBuffer,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color: Rgba,
    offsets: &[(i32, i32)],
) -> usize {
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
mod tests {
    use super::*;
    use crate::canvas::PixelBuffer;
    use crate::project::Rgba;

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
        draw_stroke(
            &mut buf,
            &[[0.0, 0.0], [10.0, 0.0]],
            c,
            BrushShape::Pixel,
            1,
            false,
        );
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

    /// Stamps `points` in `chunk`-sized batches, bridging each batch to the
    /// previous one — the incremental pattern the stroke session uses.
    fn stamp_chunked(
        buf: &mut PixelBuffer,
        points: &[[f32; 2]],
        chunk: usize,
        color: Rgba,
        shape: BrushShape,
        size: u32,
    ) -> usize {
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
        let points: Vec<[f32; 2]> = (0..40)
            .map(|i| [3.0 + i as f32 * 0.7, 5.0 + (i as f32 * 0.5).sin() * 9.0])
            .collect();

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
}
