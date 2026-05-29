//! Flood-fill algorithm for the fill-bucket tool.
//!
//! The BFS implementation visits pixels in 4-connected order. An 8-connected
//! mode is not implemented here — diagonal connectivity is rarely what pixel
//! artists want from the bucket tool and causes unexpected "leaks" at
//! diagonal borders.

// Standard pixel-art math: x, y, w, h, c are idiomatic here. All i32->u32
// casts are preceded by an x >= 0 guard; u32->i32 is safe for any canvas
// < 2^31 pixels wide.
#![allow(clippy::many_single_char_names, clippy::cast_possible_wrap, clippy::cast_sign_loss)]

use std::collections::VecDeque;

use crate::canvas::PixelBuffer;
use crate::project::Rgba;

/// Flood-fill starting at `(x, y)`, replacing pixels that match the
/// colour at the seed point (within `tolerance`) with `fill_color`.
///
/// `tolerance` compares each channel independently: a pixel matches if
/// every channel differs from the seed by no more than `tolerance`.
/// Pass `0` for exact-match, `255` to fill everything reachable.
///
/// No-op when `(x, y)` is out of bounds or the seed already equals
/// `fill_color` within tolerance.
pub fn flood_fill(buf: &mut PixelBuffer, x: i32, y: i32, fill_color: Rgba, tolerance: u8) {
    if x < 0 || y < 0 || x >= buf.width() as i32 || y >= buf.height() as i32 {
        return;
    }

    let Some(target) = buf.pixel(x as u32, y as u32) else {
        return;
    };

    // Skip if the seed is already within tolerance of the fill color to
    // avoid a full-canvas repaint when the user clicks an already-filled area.
    if target.within_tolerance(fill_color, tolerance) {
        return;
    }

    let w = buf.width();
    let h = buf.height();
    let mut visited = vec![false; (w as usize) * (h as usize)];
    let mut queue: VecDeque<(u32, u32)> = VecDeque::new();
    queue.push_back((x as u32, y as u32));

    while let Some((cx, cy)) = queue.pop_front() {
        let idx = cy as usize * w as usize + cx as usize;
        if visited[idx] {
            continue;
        }
        visited[idx] = true;

        let Some(current) = buf.pixel(cx, cy) else {
            continue;
        };

        if !current.within_tolerance(target, tolerance) {
            continue;
        }

        buf.set_pixel(cx, cy, fill_color);

        if cx > 0 {
            queue.push_back((cx - 1, cy));
        }
        if cx + 1 < w {
            queue.push_back((cx + 1, cy));
        }
        if cy > 0 {
            queue.push_back((cx, cy - 1));
        }
        if cy + 1 < h {
            queue.push_back((cx, cy + 1));
        }
    }
}

/// Flood-fill like [`flood_fill`], but blend `fill_color` at `opacity`
/// (`0..=255`) over each filled pixel instead of overwriting it.
///
/// `opacity == 255` is the overwrite path and delegates straight to
/// [`flood_fill`], so a full-opacity bucket stays byte-identical. At a
/// reduced opacity the matched region is still determined by `tolerance`
/// against the seed colour — the BFS matches on the pre-fill pixel, marks
/// each cell visited before writing, and writes through
/// [`PixelBuffer::set_pixel_blended`], so every pixel is blended exactly
/// once (no compounding across the flood). `opacity == 0` fills nothing.
///
/// No-op when `(x, y)` is out of bounds or — at full opacity — the seed
/// already equals `fill_color` within tolerance.
pub fn flood_fill_blended(buf: &mut PixelBuffer, x: i32, y: i32, fill_color: Rgba, tolerance: u8, opacity: u8) {
    if opacity == 255 {
        flood_fill(buf, x, y, fill_color, tolerance);
        return;
    }
    if opacity == 0 {
        return;
    }
    if x < 0 || y < 0 || x >= buf.width() as i32 || y >= buf.height() as i32 {
        return;
    }

    let Some(target) = buf.pixel(x as u32, y as u32) else {
        return;
    };

    let w = buf.width();
    let h = buf.height();
    let mut visited = vec![false; (w as usize) * (h as usize)];
    let mut queue: VecDeque<(u32, u32)> = VecDeque::new();
    queue.push_back((x as u32, y as u32));

    while let Some((cx, cy)) = queue.pop_front() {
        let idx = cy as usize * w as usize + cx as usize;
        if visited[idx] {
            continue;
        }
        visited[idx] = true;

        let Some(current) = buf.pixel(cx, cy) else {
            continue;
        };

        if !current.within_tolerance(target, tolerance) {
            continue;
        }

        buf.set_pixel_blended(cx, cy, fill_color, opacity);

        if cx > 0 {
            queue.push_back((cx - 1, cy));
        }
        if cx + 1 < w {
            queue.push_back((cx + 1, cy));
        }
        if cy > 0 {
            queue.push_back((cx, cy - 1));
        }
        if cy + 1 < h {
            queue.push_back((cx, cy + 1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::PixelBuffer;
    use crate::project::Rgba;

    fn buf_from_rows(w: u32, h: u32, rows: &[&[Rgba]]) -> PixelBuffer {
        let mut buf = PixelBuffer::new(w, h).unwrap();
        for (y, row) in rows.iter().enumerate() {
            for (x, &color) in row.iter().enumerate() {
                buf.set_pixel(x as u32, y as u32, color);
            }
        }
        buf
    }

    const T: Rgba = Rgba { r: 0, g: 0, b: 0, a: 0 }; // transparent
    const R: Rgba = Rgba { r: 255, g: 0, b: 0, a: 255 }; // red
    const B: Rgba = Rgba { r: 0, g: 0, b: 255, a: 255 }; // blue
    const G: Rgba = Rgba { r: 0, g: 255, b: 0, a: 255 }; // green (fill target)

    #[test]
    fn fill_simple_region() {
        let mut buf = buf_from_rows(3, 3, &[&[T, T, T], &[T, T, T], &[T, T, T]]);
        flood_fill(&mut buf, 1, 1, R, 0);
        for y in 0..3u32 {
            for x in 0..3u32 {
                assert_eq!(buf.pixel(x, y), Some(R));
            }
        }
    }

    #[test]
    fn fill_bounded_by_wall() {
        // Red border with transparent interior
        let mut buf = PixelBuffer::new(5, 5).unwrap();
        for x in 0..5u32 {
            buf.set_pixel(x, 0, R);
            buf.set_pixel(x, 4, R);
        }
        for y in 0..5u32 {
            buf.set_pixel(0, y, R);
            buf.set_pixel(4, y, R);
        }
        // Fill the interior at (2,2)
        flood_fill(&mut buf, 2, 2, B, 0);
        assert_eq!(buf.pixel(2, 2), Some(B));
        assert_eq!(buf.pixel(1, 1), Some(B));
        // Border should be unchanged
        assert_eq!(buf.pixel(0, 0), Some(R));
        assert_eq!(buf.pixel(4, 4), Some(R));
    }

    #[test]
    fn fill_does_not_cross_different_color() {
        // Two separate transparent regions separated by a red vertical bar
        let mut buf = PixelBuffer::new(5, 3).unwrap();
        for y in 0..3u32 {
            buf.set_pixel(2, y, R); // vertical wall
        }
        flood_fill(&mut buf, 0, 0, G, 0);
        // Left side filled
        assert_eq!(buf.pixel(0, 0), Some(G));
        assert_eq!(buf.pixel(1, 1), Some(G));
        // Right side NOT filled
        assert_eq!(buf.pixel(3, 0), Some(T));
        assert_eq!(buf.pixel(4, 2), Some(T));
    }

    #[test]
    fn fill_with_tolerance_matches_near_colors() {
        let mut buf = PixelBuffer::new(3, 1).unwrap();
        // Near-transparent: alpha varies slightly
        buf.set_pixel(0, 0, Rgba::new(0, 0, 0, 5));
        buf.set_pixel(1, 0, Rgba::new(0, 0, 0, 8));
        buf.set_pixel(2, 0, Rgba::new(0, 0, 0, 3));
        flood_fill(&mut buf, 0, 0, R, 10);
        // All pixels within tolerance 10 of the seed (alpha=5) are filled
        assert_eq!(buf.pixel(0, 0), Some(R));
        assert_eq!(buf.pixel(1, 0), Some(R));
        assert_eq!(buf.pixel(2, 0), Some(R));
    }

    #[test]
    fn fill_out_of_bounds_is_noop() {
        let mut buf = PixelBuffer::new(4, 4).unwrap();
        flood_fill(&mut buf, -1, 0, R, 0);
        flood_fill(&mut buf, 0, 100, R, 0);
        // No pixels modified
        for y in 0..4u32 {
            for x in 0..4u32 {
                assert_eq!(buf.pixel(x, y), Some(T));
            }
        }
    }

    #[test]
    fn fill_already_matching_is_noop() {
        let mut buf = PixelBuffer::filled(4, 4, R).unwrap();
        flood_fill(&mut buf, 2, 2, R, 0);
        // All pixels still red
        assert_eq!(buf.pixel(0, 0), Some(R));
    }

    #[test]
    fn blended_fill_at_255_matches_flood_fill() {
        let mut overwrite = PixelBuffer::filled(5, 5, B).unwrap();
        flood_fill(&mut overwrite, 2, 2, R, 0);
        let mut blended = PixelBuffer::filled(5, 5, B).unwrap();
        flood_fill_blended(&mut blended, 2, 2, R, 0, 255);
        assert_eq!(blended.as_bytes(), overwrite.as_bytes(), "opacity 255 fill must overwrite identically");
    }

    #[test]
    fn blended_fill_at_zero_is_noop() {
        let untouched = PixelBuffer::filled(5, 5, B).unwrap();
        let mut buf = untouched.clone();
        flood_fill_blended(&mut buf, 2, 2, R, 0, 0);
        assert_eq!(buf.as_bytes(), untouched.as_bytes(), "opacity 0 fill must change nothing");
    }

    #[test]
    fn blended_fill_at_128_blends_each_cell_once() {
        // A uniform region filled at half opacity holds a single blend per
        // pixel — the flood visits each cell once, so there is no compounding.
        let backdrop = B;
        let src = R;
        let expected = crate::canvas::blend::blend_normal(src, backdrop, 128);
        let mut buf = PixelBuffer::filled(4, 4, backdrop).unwrap();
        flood_fill_blended(&mut buf, 0, 0, src, 0, 128);
        for y in 0..4u32 {
            for x in 0..4u32 {
                assert_eq!(buf.pixel(x, y), Some(expected), "cell ({x}, {y}) is not a single 128 blend");
            }
        }
    }

    #[test]
    fn blended_fill_respects_walls() {
        // Reduced opacity must not change which pixels the flood reaches.
        let mut buf = PixelBuffer::new(5, 3).unwrap();
        for y in 0..3u32 {
            buf.set_pixel(2, y, R); // vertical wall
        }
        flood_fill_blended(&mut buf, 0, 0, G, 0, 128);
        let expected_left = crate::canvas::blend::blend_normal(G, T, 128);
        assert_eq!(buf.pixel(0, 0), Some(expected_left));
        assert_eq!(buf.pixel(1, 1), Some(expected_left));
        // Right side untouched; wall untouched.
        assert_eq!(buf.pixel(3, 0), Some(T));
        assert_eq!(buf.pixel(2, 0), Some(R));
    }

    #[test]
    fn blended_fill_out_of_bounds_is_noop() {
        let untouched = PixelBuffer::filled(4, 4, B).unwrap();
        let mut buf = untouched.clone();
        flood_fill_blended(&mut buf, -1, 0, R, 0, 128);
        flood_fill_blended(&mut buf, 0, 100, R, 0, 128);
        assert_eq!(buf.as_bytes(), untouched.as_bytes());
    }
}
