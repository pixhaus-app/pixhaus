//! Geometric shape drawing for the rectangle and ellipse tools.
//!
//! Each function has an outline and a filled variant. Coordinates are
//! inclusive canvas pixels; `(x0, y0)` is the top-left corner and
//! `(x1, y1)` is the bottom-right corner. The functions normalise order
//! so callers can pass corners in either order.

// All `i32 as u32` casts in this module happen after an explicit `>= 0` bounds
// check that the borrow checker can't see. The `u32 as i32` casts are safe for
// any reasonable canvas size (< 2^31 pixels). The f64 -> i32 truncation in the
// scanline fill is intentional (floor). The i32 -> f64 / i32 -> i64 widening
// casts are lossless and could use `From`, but the explicit form is clearer
// in arithmetic expressions.
#![allow(clippy::cast_sign_loss, clippy::cast_possible_wrap, clippy::cast_possible_truncation, clippy::cast_lossless)]

use crate::canvas::PixelBuffer;
use crate::project::Rgba;

/// Draw the outline of a rectangle from `(x0, y0)` to `(x1, y1)`.
pub fn draw_rect(buf: &mut PixelBuffer, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba) {
    let (x0, x1) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
    let (y0, y1) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
    let w = buf.width() as i32;
    let h = buf.height() as i32;

    for x in x0.max(0)..=x1.min(w - 1) {
        if y0 >= 0 && y0 < h {
            buf.set_pixel(x as u32, y0 as u32, color);
        }
        if y1 >= 0 && y1 < h {
            buf.set_pixel(x as u32, y1 as u32, color);
        }
    }
    for y in y0.max(0)..=y1.min(h - 1) {
        if x0 >= 0 && x0 < w {
            buf.set_pixel(x0 as u32, y as u32, color);
        }
        if x1 >= 0 && x1 < w {
            buf.set_pixel(x1 as u32, y as u32, color);
        }
    }
}

/// Draw a filled rectangle from `(x0, y0)` to `(x1, y1)`.
pub fn draw_filled_rect(buf: &mut PixelBuffer, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba) {
    let (x0, x1) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
    let (y0, y1) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
    let w = buf.width() as i32;
    let h = buf.height() as i32;

    for y in y0.max(0)..=y1.min(h - 1) {
        for x in x0.max(0)..=x1.min(w - 1) {
            buf.set_pixel(x as u32, y as u32, color);
        }
    }
}

/// Draw the outline of an ellipse inscribed within the bounding box
/// `(x0, y0)` to `(x1, y1)` using the midpoint ellipse algorithm.
pub fn draw_ellipse(buf: &mut PixelBuffer, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba) {
    let (x0, x1) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
    let (y0, y1) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };

    let cx = (x0 + x1) / 2;
    let cy = (y0 + y1) / 2;
    let rx = (x1 - x0) / 2;
    let ry = (y1 - y0) / 2;

    midpoint_ellipse_outline(buf, cx, cy, rx, ry, color);
}

/// Draw a filled ellipse inscribed within the bounding box.
pub fn draw_filled_ellipse(buf: &mut PixelBuffer, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba) {
    let (x0, x1) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
    let (y0, y1) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };

    let cx = (x0 + x1) / 2;
    let cy = (y0 + y1) / 2;
    let rx = (x1 - x0) / 2;
    let ry = (y1 - y0) / 2;

    let w = buf.width() as i32;
    let h = buf.height() as i32;
    let rx2 = (rx * rx).max(1);
    let ry2 = (ry * ry).max(1);

    for dy in -ry..=ry {
        // Horizontal extent of the ellipse at this scanline
        let half_width = ((rx2 as f64 * (1.0 - (dy * dy) as f64 / ry2 as f64)).max(0.0).sqrt()) as i32;
        let xa = (cx - half_width).max(0);
        let xb = (cx + half_width).min(w - 1);
        let py = cy + dy;
        if py < 0 || py >= h {
            continue;
        }
        for x in xa..=xb {
            buf.set_pixel(x as u32, py as u32, color);
        }
    }
}

/// Midpoint ellipse algorithm — plots the 4-way symmetric outline.
fn midpoint_ellipse_outline(buf: &mut PixelBuffer, cx: i32, cy: i32, rx: i32, ry: i32, color: Rgba) {
    if rx <= 0 || ry <= 0 {
        buf.set_pixel(cx.max(0) as u32, cy.max(0) as u32, color);
        return;
    }

    let mut x = 0i64;
    let mut y = ry as i64;
    let rx2 = (rx as i64) * (rx as i64);
    let ry2 = (ry as i64) * (ry as i64);
    let mut d1 = ry2 - rx2 * (ry as i64) + rx2 / 4;
    let mut dx = 2 * ry2 * x;
    let mut dy = 2 * rx2 * y;

    while dx < dy {
        plot_symmetric(buf, cx, cy, x as i32, y as i32, color);
        x += 1;
        dx += 2 * ry2;
        if d1 < 0 {
            d1 += dx + ry2;
        } else {
            y -= 1;
            dy -= 2 * rx2;
            d1 += dx - dy + ry2;
        }
    }

    let mut d2 = ry2 * (x * x + x) + rx2 * (y - 1) * (y - 1) - rx2 * ry2;
    while y >= 0 {
        plot_symmetric(buf, cx, cy, x as i32, y as i32, color);
        y -= 1;
        dy -= 2 * rx2;
        if d2 > 0 {
            d2 += rx2 - dy;
        } else {
            x += 1;
            dx += 2 * ry2;
            d2 += dx - dy + rx2;
        }
    }
}

fn plot_symmetric(buf: &mut PixelBuffer, cx: i32, cy: i32, x: i32, y: i32, color: Rgba) {
    let w = buf.width() as i32;
    let h = buf.height() as i32;
    for (px, py) in [(cx + x, cy + y), (cx - x, cy + y), (cx + x, cy - y), (cx - x, cy - y)] {
        if px >= 0 && py >= 0 && px < w && py < h {
            buf.set_pixel(px as u32, py as u32, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::PixelBuffer;
    use crate::project::Rgba;

    const T: Rgba = Rgba { r: 0, g: 0, b: 0, a: 0 };
    const R: Rgba = Rgba { r: 255, g: 0, b: 0, a: 255 };

    fn transparent_buf(w: u32, h: u32) -> PixelBuffer {
        PixelBuffer::new(w, h).unwrap()
    }

    #[test]
    fn filled_rect_covers_region() {
        let mut buf = transparent_buf(8, 8);
        draw_filled_rect(&mut buf, 1, 1, 5, 5, R);
        for y in 1u32..=5 {
            for x in 1u32..=5 {
                assert_eq!(buf.pixel(x, y), Some(R), "({x},{y}) should be filled");
            }
        }
        // Outside should remain transparent
        assert_eq!(buf.pixel(0, 0), Some(T));
        assert_eq!(buf.pixel(6, 6), Some(T));
    }

    #[test]
    fn rect_outline_only_border() {
        let mut buf = transparent_buf(8, 8);
        draw_rect(&mut buf, 1, 1, 5, 5, R);
        // Corners painted
        assert_eq!(buf.pixel(1, 1), Some(R));
        assert_eq!(buf.pixel(5, 5), Some(R));
        // Interior should be transparent
        assert_eq!(buf.pixel(3, 3), Some(T));
    }

    #[test]
    fn filled_ellipse_paints_centre() {
        let mut buf = transparent_buf(16, 16);
        draw_filled_ellipse(&mut buf, 2, 2, 12, 12, R);
        // Centre of the ellipse should be painted
        assert_eq!(buf.pixel(7, 7), Some(R));
        // Corners of bounding box should NOT be painted (outside ellipse)
        assert_eq!(buf.pixel(2, 2), Some(T));
    }

    #[test]
    fn draw_rect_normalises_reversed_corners() {
        let mut buf = transparent_buf(8, 8);
        draw_rect(&mut buf, 5, 5, 1, 1, R);
        // Top-left corner
        assert_eq!(buf.pixel(1, 1), Some(R));
        // Bottom-right corner
        assert_eq!(buf.pixel(5, 5), Some(R));
    }
}
