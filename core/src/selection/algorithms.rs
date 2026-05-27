//! Selection algorithms that produce a [`SelectionMask`] from geometric
//! or pixel-based inputs.
//!
//! ## Algorithms
//!
//! - [`select_rect`] — axis-aligned rectangle
//! - [`select_ellipse`] — filled ellipse inscribed in a bounding rect
//! - [`select_polygon`] — scanline-filled freehand polygon (auto-closed)
//! - [`magic_wand`] — flood-fill from a seed pixel within a tolerance
//! - [`color_range`] — global color-range select within a tolerance
//!
//! All algorithms snap to pixel boundaries; sub-pixel geometry is
//! intentionally absent (pixel-art bias).

use std::collections::VecDeque;

use crate::canvas::PixelBuffer;
use crate::project::{IVec2, Rect, Rgba};

use super::autoclose::{self, GapCloseConfig};
use super::error::{Error, Result};
use super::mask::SelectionMask;

/// Flood-fill connectivity mode for [`magic_wand`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Connectivity {
    /// Only the four cardinal neighbours (N, E, S, W).
    Four,
    /// All eight neighbours including diagonals.
    Eight,
}

// --- helpers -----------------------------------------------------------------

/// Clamps `v` to `[0, limit]` and converts to `u32`.
///
/// Used to map signed/wide canvas coordinates back to the `[0, canvas_dim]`
/// range without triggering pedantic cast lints.
fn clamp_canvas(v: i64, limit: u32) -> u32 {
    // v.max(0) is non-negative; min(i64::from(limit)) keeps it within u32 range.
    u32::try_from(v.max(0).min(i64::from(limit))).unwrap_or(0)
}

// --- shape algorithms --------------------------------------------------------

/// Selects all pixels inside the given `bounds` rectangle, clipped to
/// the canvas of size `(canvas_w, canvas_h)`.
///
/// An empty rect produces an empty mask.
///
/// # Errors
///
/// Returns [`Error::DimensionOverflow`] if `canvas_w * canvas_h`
/// overflows `usize` (relevant on 32-bit hosts and at the extreme end
/// of 64-bit).
pub fn select_rect(canvas_w: u32, canvas_h: u32, bounds: Rect) -> Result<SelectionMask> {
    let mut mask = SelectionMask::new(canvas_w, canvas_h)?;
    if bounds.is_empty() || canvas_w == 0 || canvas_h == 0 {
        return Ok(mask);
    }
    let ox = i64::from(bounds.origin.x);
    let oy = i64::from(bounds.origin.y);
    let x0 = clamp_canvas(ox, canvas_w);
    let y0 = clamp_canvas(oy, canvas_h);
    let x1 = clamp_canvas(ox + i64::from(bounds.size.width), canvas_w);
    let y1 = clamp_canvas(oy + i64::from(bounds.size.height), canvas_h);

    for y in y0..y1 {
        for x in x0..x1 {
            mask.set(x, y, 255);
        }
    }
    Ok(mask)
}

/// Selects all pixels whose centres fall inside the ellipse inscribed
/// in `bounds`, clipped to the canvas.
///
/// Uses integer-safe point-in-ellipse testing on pixel centres.
///
/// # Errors
///
/// Returns [`Error::DimensionOverflow`] when `canvas_w * canvas_h`
/// overflows `usize`.
pub fn select_ellipse(canvas_w: u32, canvas_h: u32, bounds: Rect) -> Result<SelectionMask> {
    let mut mask = SelectionMask::new(canvas_w, canvas_h)?;
    if bounds.is_empty() || canvas_w == 0 || canvas_h == 0 {
        return Ok(mask);
    }

    // Work in 64-bit fixed-point to keep the arithmetic exact.
    // Multiply by 2 so the centre and half-axes are always integers.
    let x2_left = i64::from(bounds.origin.x) * 2;
    let y2_top = i64::from(bounds.origin.y) * 2;
    let x2_right = x2_left + i64::from(bounds.size.width) * 2;
    let y2_bottom = y2_top + i64::from(bounds.size.height) * 2;

    // Centre of the ellipse in the doubled coordinate space.
    let cx2 = x2_left + i64::from(bounds.size.width);
    let cy2 = y2_top + i64::from(bounds.size.height);

    // Semi-axes in the doubled space.
    let a2 = i64::from(bounds.size.width);
    let b2 = i64::from(bounds.size.height);

    // Degenerate: zero-radius ellipse.
    if a2 == 0 || b2 == 0 {
        return Ok(mask);
    }

    let a2sq = a2 * a2;
    let b2sq = b2 * b2;

    let x_start = clamp_canvas(x2_left / 2, canvas_w);
    let y_start = clamp_canvas(y2_top / 2, canvas_h);
    let x_end = clamp_canvas(x2_right / 2 + 1, canvas_w);
    let y_end = clamp_canvas(y2_bottom / 2 + 1, canvas_h);

    for y in y_start..y_end {
        for x in x_start..x_end {
            // Pixel centre in doubled coordinate space.
            let px2 = i64::from(x) * 2 + 1;
            let py2 = i64::from(y) * 2 + 1;
            let dx = px2 - cx2;
            let dy = py2 - cy2;
            // (dx/a)^2 + (dy/b)^2 <= 1  <=>  dx^2 * b2^2 + dy^2 * a2^2 <= (a2 * b2)^2
            // All in doubled space so a2, b2, dx, dy are all 2x the real values.
            // The inequality is homogeneous so the 2x factor cancels.
            if dx * dx * b2sq + dy * dy * a2sq <= a2sq * b2sq {
                mask.set(x, y, 255);
            }
        }
    }
    Ok(mask)
}

/// Selects all pixels inside a polygon defined by `points` using an
/// even-odd scanline fill.
///
/// The polygon is automatically closed (last point connects back to
/// first). Fewer than three points produce an empty mask.
///
/// # Errors
///
/// Returns [`Error::DimensionOverflow`] when `canvas_w * canvas_h`
/// overflows `usize`.
pub fn select_polygon(canvas_w: u32, canvas_h: u32, points: &[IVec2]) -> Result<SelectionMask> {
    let mut mask = SelectionMask::new(canvas_w, canvas_h)?;
    if points.len() < 3 || canvas_w == 0 || canvas_h == 0 {
        return Ok(mask);
    }

    let n = points.len();
    let scan_y_limit = i32::try_from(canvas_h).unwrap_or(i32::MAX);

    for scan_y in 0..scan_y_limit {
        let mut intersections: Vec<f64> = Vec::new();
        let mut j = n - 1;
        for i in 0..n {
            let pi = points[i];
            let pj = points[j];
            // Half-open interval: edge contributes when one end is
            // strictly above the scanline and the other is on or below.
            let pi_above = pi.y <= scan_y;
            if pi_above != (pj.y <= scan_y) {
                let t = f64::from(scan_y - pi.y) / f64::from(pj.y - pi.y);
                let xi = f64::from(pi.x) + t * f64::from(pj.x - pi.x);
                intersections.push(xi);
            }
            j = i;
        }

        // Sort and fill between pairs.
        intersections.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut k = 0;
        let canvas_w_i32 = i32::try_from(canvas_w).unwrap_or(i32::MAX);
        while k + 1 < intersections.len() {
            // Safe: intersection values are bounded by polygon vertex coords (i32 range).
            #[allow(clippy::cast_possible_truncation)]
            let x0 = intersections[k].ceil() as i32;
            #[allow(clippy::cast_possible_truncation)]
            let x1 = intersections[k + 1].floor() as i32;
            // Clip to the canvas BEFORE iterating. A polygon vertex
            // far outside the canvas (say x = 1_000_000_000) used to
            // make the inner loop visit a billion pixels and skip them
            // one at a time; clip first so the loop bound matches the
            // actual fillable range.
            let lo = x0.max(0);
            let hi = x1.min(canvas_w_i32 - 1);
            if hi < lo {
                k += 2;
                continue;
            }
            for x in lo..=hi {
                let xu = u32::try_from(x).unwrap_or(0);
                mask.set(xu, u32::try_from(scan_y).unwrap_or(0), 255);
            }
            k += 2;
        }
    }

    Ok(mask)
}

// --- pixel-based algorithms --------------------------------------------------

/// Flood-fills from `(seed_x, seed_y)`, selecting all pixels reachable
/// through `connectivity` whose colour matches the seed within
/// `tolerance`.
///
/// Tolerance is a per-channel absolute-difference threshold. Two colours
/// match when every channel's absolute difference is ≤ `tolerance`.
/// `tolerance = 0` selects only exact matches; `tolerance = 255` selects
/// everything.
///
/// # Errors
///
/// - [`Error::SeedOutOfBounds`] if `(seed_x, seed_y)` is outside the buffer.
/// - [`Error::DimensionOverflow`] if `width * height` overflows `usize`.
pub fn magic_wand(buffer: &PixelBuffer, seed_x: u32, seed_y: u32, tolerance: u8, connectivity: Connectivity) -> Result<SelectionMask> {
    let w = buffer.width();
    let h = buffer.height();
    if seed_x >= w || seed_y >= h {
        return Err(Error::SeedOutOfBounds {
            x: seed_x,
            y: seed_y,
            width: w,
            height: h,
        });
    }

    let seed_color = buffer.pixel(seed_x, seed_y).unwrap_or(Rgba::transparent());

    let area = super::mask::checked_area(w, h)?;
    let mut mask = SelectionMask::new(w, h)?;
    let mut visited = vec![false; area];
    let mut queue: VecDeque<(u32, u32)> = VecDeque::new();

    let seed_idx = (seed_y as usize) * (w as usize) + (seed_x as usize);
    visited[seed_idx] = true;
    queue.push_back((seed_x, seed_y));

    let offsets: &[(i32, i32)] = match connectivity {
        Connectivity::Four => &[(0, -1), (0, 1), (-1, 0), (1, 0)],
        Connectivity::Eight => &[(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)],
    };

    let w_i32 = i32::try_from(w).unwrap_or(i32::MAX);
    let h_i32 = i32::try_from(h).unwrap_or(i32::MAX);

    while let Some((x, y)) = queue.pop_front() {
        let Some(color) = buffer.pixel(x, y) else {
            continue;
        };
        if !color.within_tolerance(seed_color, tolerance) {
            continue;
        }

        mask.set(x, y, 255);

        for (dx, dy) in offsets {
            let nx = i32::try_from(x).unwrap_or(i32::MAX) + dx;
            let ny = i32::try_from(y).unwrap_or(i32::MAX) + dy;
            if nx < 0 || ny < 0 || nx >= w_i32 || ny >= h_i32 {
                continue;
            }
            let nx = u32::try_from(nx).unwrap_or(0);
            let ny = u32::try_from(ny).unwrap_or(0);
            let idx = (ny as usize) * (w as usize) + (nx as usize);
            if !visited[idx] {
                visited[idx] = true;
                queue.push_back((nx, ny));
            }
        }
    }

    Ok(mask)
}

/// Flood-fills like [`magic_wand`], but first runs the gap-closing
/// pre-pass from [`autoclose`] so short breaks in ink outlines stop
/// the fill from leaking out.
///
/// The closure is applied to a clone of `buffer`: bridge pixels are
/// stamped in opaque black (the typical ink color in pixel art) so
/// the existing flood-fill rejects them as "not matching the seed".
/// With `gap_config = None` this is equivalent to calling
/// [`magic_wand`] directly, just with a redundant buffer clone.
///
/// # Errors
///
/// - [`Error::SeedOutOfBounds`] if `seed` is outside the buffer.
/// - [`Error::DimensionOverflow`] if `width * height` overflows.
pub fn magic_wand_with_gap_close(
    buffer: &PixelBuffer,
    seed: IVec2,
    tolerance: u8,
    connectivity: Connectivity,
    gap_config: Option<GapCloseConfig>,
) -> Result<SelectionMask> {
    let seed_x = u32::try_from(seed.x).map_err(|_| Error::SeedOutOfBounds {
        x: 0,
        y: 0,
        width: buffer.width(),
        height: buffer.height(),
    })?;
    let seed_y = u32::try_from(seed.y).map_err(|_| Error::SeedOutOfBounds {
        x: 0,
        y: 0,
        width: buffer.width(),
        height: buffer.height(),
    })?;

    let Some(cfg) = gap_config else {
        return magic_wand(buffer, seed_x, seed_y, tolerance, connectivity);
    };

    let closure = autoclose::close_gaps(buffer, &cfg)?;
    let mut working = buffer.clone();
    stamp_closure_pixels(&mut working, &closure);
    magic_wand(&working, seed_x, seed_y, tolerance, connectivity)
}

/// Writes opaque black at every pixel selected by `closure`. The
/// existing [`magic_wand`] will then refuse to cross those pixels
/// unless the seed itself is black (which would defeat the purpose).
fn stamp_closure_pixels(buffer: &mut PixelBuffer, closure: &SelectionMask) {
    let ink = Rgba::new(0, 0, 0, 255);
    let w = closure.width().min(buffer.width());
    let h = closure.height().min(buffer.height());
    for y in 0..h {
        for x in 0..w {
            if closure.is_selected(x, y) {
                buffer.set_pixel(x, y, ink);
            }
        }
    }
}

/// Selects all pixels in `buffer` whose colour matches `reference`
/// within `tolerance`, regardless of spatial connectivity.
///
/// Tolerance is a per-channel absolute-difference threshold (same
/// semantics as [`magic_wand`]).
///
/// # Errors
///
/// Returns [`Error::DimensionOverflow`] when `width * height` overflows.
pub fn color_range(buffer: &PixelBuffer, reference: Rgba, tolerance: u8) -> Result<SelectionMask> {
    let w = buffer.width();
    let h = buffer.height();
    let mut mask = SelectionMask::new(w, h)?;

    for y in 0..h {
        for x in 0..w {
            let Some(color) = buffer.pixel(x, y) else {
                continue;
            };
            if color.within_tolerance(reference, tolerance) {
                mask.set(x, y, 255);
            }
        }
    }

    Ok(mask)
}

#[cfg(test)]
mod tests {
    use crate::project::{IVec2, Rect};

    use super::*;

    fn red() -> Rgba {
        Rgba::opaque(255, 0, 0)
    }
    fn blue() -> Rgba {
        Rgba::opaque(0, 0, 255)
    }
    fn green() -> Rgba {
        Rgba::opaque(0, 255, 0)
    }

    // --- rect ----------------------------------------------------------------

    #[test]
    fn rect_fills_bounds() {
        let m = select_rect(8, 8, Rect::from_xywh(2, 2, 4, 3)).unwrap();
        assert_eq!(m.selected_count(), 12);
        assert!(m.is_fully_selected(2, 2));
        assert!(m.is_fully_selected(5, 4));
        assert!(!m.is_selected(1, 2));
        assert!(!m.is_selected(6, 2));
    }

    #[test]
    fn rect_clips_to_canvas() {
        let m = select_rect(4, 4, Rect::from_xywh(-1, -1, 6, 6)).unwrap();
        assert_eq!(m.selected_count(), 16);
    }

    #[test]
    fn rect_empty_bounds_returns_empty() {
        let m = select_rect(8, 8, Rect::from_xywh(2, 2, 0, 4)).unwrap();
        assert_eq!(m.selected_count(), 0);
    }

    #[test]
    fn rect_out_of_canvas_returns_empty() {
        let m = select_rect(8, 8, Rect::from_xywh(10, 10, 4, 4)).unwrap();
        assert_eq!(m.selected_count(), 0);
    }

    // --- ellipse -------------------------------------------------------------

    #[test]
    fn ellipse_1x1_selects_single_pixel() {
        let m = select_ellipse(8, 8, Rect::from_xywh(3, 3, 1, 1)).unwrap();
        assert!(m.is_fully_selected(3, 3));
        assert_eq!(m.selected_count(), 1);
    }

    #[test]
    fn ellipse_interior_is_filled() {
        // 8x8 ellipse — centre pixel must always be selected.
        let m = select_ellipse(16, 16, Rect::from_xywh(0, 0, 8, 8)).unwrap();
        assert!(m.is_fully_selected(4, 4));
    }

    #[test]
    fn ellipse_corners_excluded() {
        // A 4x4 bounding rect; the corners of the rect are outside the ellipse.
        let m = select_ellipse(8, 8, Rect::from_xywh(0, 0, 4, 4)).unwrap();
        assert!(!m.is_selected(0, 0));
        assert!(!m.is_selected(3, 0));
        assert!(!m.is_selected(0, 3));
        assert!(!m.is_selected(3, 3));
    }

    #[test]
    fn ellipse_clips_to_canvas() {
        let m = select_ellipse(4, 4, Rect::from_xywh(-2, -2, 8, 8)).unwrap();
        // All 16 pixels should be inside the ellipse after clipping.
        assert_eq!(m.selected_count(), 16);
    }

    // --- polygon -------------------------------------------------------------

    #[test]
    fn polygon_fewer_than_three_points_is_empty() {
        let m = select_polygon(8, 8, &[IVec2::new(0, 0), IVec2::new(4, 4)]).unwrap();
        assert_eq!(m.selected_count(), 0);
    }

    #[test]
    fn polygon_triangle_fills_interior() {
        // Triangle with vertices (0,0), (7,0), (3,7)
        let pts = [IVec2::new(0, 0), IVec2::new(7, 0), IVec2::new(3, 7)];
        let m = select_polygon(8, 8, &pts).unwrap();
        // Interior point must be selected.
        assert!(m.is_selected(3, 3));
        // Outside the triangle.
        assert!(!m.is_selected(0, 7));
    }

    #[test]
    fn polygon_square_fills_interior() {
        // Square with vertices at (1,1), (5,1), (5,5), (1,5).
        // The half-open scanline rule fills y=1..4, x=1..5 = 20 pixels.
        // The bottom row at y=5 is not filled (it's on the boundary).
        let pts = [IVec2::new(1, 1), IVec2::new(5, 1), IVec2::new(5, 5), IVec2::new(1, 5)];
        let m = select_polygon(8, 8, &pts).unwrap();
        // Interior pixel.
        assert!(m.is_selected(3, 3));
        // Top-left vertex row is included.
        assert!(m.is_selected(1, 1));
        // Outside left edge.
        assert!(!m.is_selected(0, 3));
        // Bottom edge (y=5) is on the polygon boundary but not filled by
        // the half-open scanline convention.
        assert!(!m.is_selected(3, 5));
        // Expected filled area: y in [1,4], x in [1,5] = 4 * 5 = 20.
        assert_eq!(m.selected_count(), 20);
    }

    // --- magic wand ----------------------------------------------------------

    fn make_two_color_buffer() -> PixelBuffer {
        // 4x4 buffer: top-left 2x2 red, rest blue.
        let mut buf = PixelBuffer::new(4, 4).unwrap();
        for y in 0..4u32 {
            for x in 0..4u32 {
                if x < 2 && y < 2 {
                    buf.set_pixel(x, y, red());
                } else {
                    buf.set_pixel(x, y, blue());
                }
            }
        }
        buf
    }

    #[test]
    fn magic_wand_selects_connected_region() {
        let buf = make_two_color_buffer();
        let m = magic_wand(&buf, 0, 0, 0, Connectivity::Four).unwrap();
        // Red 2x2 selected.
        assert_eq!(m.selected_count(), 4);
        assert!(m.is_fully_selected(0, 0));
        assert!(m.is_fully_selected(1, 1));
        assert!(!m.is_selected(2, 0));
    }

    #[test]
    fn magic_wand_eight_connected_crosses_diagonal_gap() {
        // L-shaped blue region around a single red pixel diagonal to seed.
        let mut buf = PixelBuffer::filled(3, 3, blue()).unwrap();
        buf.set_pixel(1, 1, red());
        // 8-connected from (0,0) blue: should NOT cross through the red
        // pixel to reach (2,2) — they are connected via other blue pixels.
        let m = magic_wand(&buf, 0, 0, 0, Connectivity::Eight).unwrap();
        // The single red pixel in the middle should not be selected.
        assert!(!m.is_selected(1, 1));
    }

    #[test]
    fn magic_wand_out_of_bounds_seed_errors() {
        let buf = PixelBuffer::new(4, 4).unwrap();
        assert!(magic_wand(&buf, 5, 0, 0, Connectivity::Four).is_err());
    }

    #[test]
    fn magic_wand_with_tolerance_expands_selection() {
        // All pixels identical → tolerance 0 selects all.
        let buf = PixelBuffer::filled(4, 4, red()).unwrap();
        let m = magic_wand(&buf, 0, 0, 0, Connectivity::Four).unwrap();
        assert_eq!(m.selected_count(), 16);
    }

    #[test]
    fn magic_wand_tolerance_bridges_slight_differences() {
        let mut buf = PixelBuffer::filled(4, 1, red()).unwrap();
        // Slightly different red — off by 10 in R channel.
        buf.set_pixel(2, 0, Rgba::opaque(245, 0, 0));
        // Tolerance 0 stops at pixel 2.
        let m0 = magic_wand(&buf, 0, 0, 0, Connectivity::Four).unwrap();
        assert_eq!(m0.selected_count(), 2);
        // Tolerance 10 bridges the gap.
        let m10 = magic_wand(&buf, 0, 0, 10, Connectivity::Four).unwrap();
        assert_eq!(m10.selected_count(), 4);
    }

    // --- color range ---------------------------------------------------------

    #[test]
    fn color_range_selects_all_matching() {
        let buf = make_two_color_buffer();
        let m = color_range(&buf, red(), 0).unwrap();
        assert_eq!(m.selected_count(), 4);
        for y in 0..2u32 {
            for x in 0..2u32 {
                assert!(m.is_fully_selected(x, y));
            }
        }
        assert!(!m.is_selected(2, 0));
    }

    #[test]
    fn color_range_tolerance_255_selects_all() {
        let buf = make_two_color_buffer();
        let m = color_range(&buf, green(), 255).unwrap();
        assert_eq!(m.selected_count(), 16);
    }

    // --- magic wand with gap close ------------------------------------------

    /// Builds a `size × size` canvas with a square outline drawn at
    /// `margin` pixels from each side and a `gap_pixels`-wide hole
    /// centred on the *top* edge of the outline. Interior is white,
    /// outline is black, exterior (the margin frame) is also white so
    /// any leak through the gap is visible at e.g. `(margin / 2,
    /// margin / 2)`.
    fn square_outline_with_top_gap(size: u32, margin: u32, gap_pixels: u32) -> PixelBuffer {
        assert!(margin >= 2, "fixture needs room above the outline for leakage");
        let mut buf = PixelBuffer::filled(size, size, Rgba::opaque(255, 255, 255)).unwrap();
        let ink = Rgba::opaque(0, 0, 0);
        let top = margin;
        let bottom = size - margin - 1;
        let left = margin;
        let right = size - margin - 1;
        let gap_start = size / 2 - gap_pixels / 2;
        let gap_end = gap_start + gap_pixels;
        // Top + bottom edges.
        for x in left..=right {
            if !(gap_start..gap_end).contains(&x) {
                buf.set_pixel(x, top, ink);
            }
            buf.set_pixel(x, bottom, ink);
        }
        // Left + right edges.
        for y in top..=bottom {
            buf.set_pixel(left, y, ink);
            buf.set_pixel(right, y, ink);
        }
        buf
    }

    #[test]
    fn magic_wand_with_gap_close_disabled_matches_magic_wand() {
        let buf = make_two_color_buffer();
        let seed = IVec2 { x: 0, y: 0 };
        let direct = magic_wand(&buf, 0, 0, 0, Connectivity::Four).unwrap();
        let routed = magic_wand_with_gap_close(&buf, seed, 0, Connectivity::Four, None).unwrap();
        assert_eq!(direct.selected_count(), routed.selected_count());
    }

    #[test]
    fn magic_wand_with_gap_close_fills_through_2px_gap() {
        // 20x20 canvas, outline at margin = 3 (top edge at y = 3),
        // 2-pixel gap centred at x = 10. Seed inside the box;
        // exterior pixel (0, 0) is in the white margin above the
        // outline.
        let buf = square_outline_with_top_gap(20, 3, 2);
        let seed = IVec2 { x: 10, y: 10 };
        let mask = magic_wand_with_gap_close(&buf, seed, 0, Connectivity::Four, Some(GapCloseConfig::default())).unwrap();
        // Interior pixel must be selected.
        assert!(mask.is_selected(10, 10));
        // Outside the box (above the outline) must not be reached.
        assert!(!mask.is_selected(0, 0));
        // Sanity: without the gap close the fill leaks through the
        // top gap into the exterior white margin.
        let leaked = magic_wand(&buf, 10, 10, 0, Connectivity::Four).unwrap();
        assert!(leaked.is_selected(0, 0), "baseline fill should leak");
    }

    #[test]
    fn color_range_zero_tolerance_exact_match_only() {
        let buf = make_two_color_buffer();
        let m = color_range(&buf, blue(), 0).unwrap();
        assert_eq!(m.selected_count(), 12);
    }
}
