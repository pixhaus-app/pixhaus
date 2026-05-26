//! Gap-closing pre-pass for the magic wand.
//!
//! Adapted from `OpenToonz` `toonz/sources/common/trop/tautoclose.cpp`
//! under BSD-3-Clause. See `THIRD_PARTY_NOTICES.md`.
//!
//! # Approach
//!
//! The closer walks the buffer once, builds a boolean ink mask using a
//! Rec.709 luma threshold, then classifies each ink pixel by its
//! 8-neighbour signature against the generated `super::skeleton_lut`
//! table (private module). Endpoints — strokes that terminate with no inward
//! continuation — are paired with their nearest geometric neighbour
//! whose tangent points back toward them. A standard integer Bresenham
//! segment is rasterized into a [`SelectionMask`] for each accepted
//! pair, producing the closure mask the magic wand consults.
//!
//! The pre-pass treats the input as read-only; the magic wand entry
//! point [`super::algorithms::magic_wand_with_gap_close`] clones the
//! buffer, stamps the closure pixels as opaque black, and runs the
//! existing flood-fill against the stamped clone.

// The classifier and geometry use the standard pixel-art shuttle
// between integer pixel addresses and float distance/angle math.
// Wrap the whole module in the same `as`-cast allows that
// `transforms::antialias` uses.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_lossless,
    clippy::similar_names
)]

use crate::canvas::PixelBuffer;

use super::error::Result;
use super::mask::SelectionMask;

/// Tuning knobs for [`close_gaps`] and
/// [`super::algorithms::magic_wand_with_gap_close`].
#[derive(Copy, Clone, Debug)]
pub struct GapCloseConfig {
    /// Maximum gap, in pixels, the closer will try to bridge.
    pub closing_distance: u32,
    /// Maximum angle (radians) between an endpoint's connecting
    /// direction and the displacement to its partner. `FRAC_PI_2`
    /// means "anywhere in the forward hemisphere".
    pub closing_angle_rad: f32,
    /// Luma threshold below which a pixel counts as ink.
    pub ink_threshold: u8,
}

impl Default for GapCloseConfig {
    fn default() -> Self {
        Self {
            closing_distance: 10,
            closing_angle_rad: std::f32::consts::FRAC_PI_2,
            ink_threshold: 128,
        }
    }
}

/// Builds a mask containing only the bridge pixels that connect
/// nearby endpoint pairs in the ink outline.
///
/// The output covers the same canvas dimensions as `buffer`; bridge
/// pixels are coverage 255, everything else is 0. The caller composes
/// this with their own working buffer or selection mask — typically
/// via [`super::algorithms::magic_wand_with_gap_close`], which stamps
/// the bridge into a clone of the input buffer before flood-filling.
pub fn close_gaps(buffer: &PixelBuffer, config: &GapCloseConfig) -> Result<SelectionMask> {
    let w = buffer.width() as usize;
    let h = buffer.height() as usize;

    let mut closure = SelectionMask::new(buffer.width(), buffer.height())?;
    if w == 0 || h == 0 {
        return Ok(closure);
    }

    let mask = extract_ink_mask(buffer, config.ink_threshold);
    let classes = classify_ink_pixels(&mask, w, h);
    let pairs = pair_endpoints(&mask, &classes, w, h, config.closing_distance, config.closing_angle_rad);

    for (a, b) in pairs {
        rasterize_segment(&mut closure, (a.0 as u32, a.1 as u32), (b.0 as u32, b.1 as u32));
    }
    Ok(closure)
}

/// Returns a boolean ink mask, row-major, with one entry per pixel.
///
/// A pixel is "ink" when its Rec.709 integer luma is below
/// `threshold`. Alpha is ignored on the assumption that ink outlines
/// are opaque; if the artist stores ink as transparent black the
/// caller can pre-multiply before invoking.
pub(crate) fn extract_ink_mask(buffer: &PixelBuffer, threshold: u8) -> Vec<bool> {
    let w = buffer.width();
    let h = buffer.height();
    let mut out = Vec::with_capacity((w as usize) * (h as usize));
    for y in 0..h {
        for x in 0..w {
            // `pixel(x, y)` is `None` only when out of bounds; the
            // loops above already keep us inside, but treat the
            // missing case as background to stay panic-free.
            let p = buffer.pixel(x, y).unwrap_or_else(|| crate::project::Rgba::new(255, 255, 255, 255));
            let luma = (54u32 * u32::from(p.r) + 183 * u32::from(p.g) + 19 * u32::from(p.b)) >> 8;
            out.push(luma < u32::from(threshold));
        }
    }
    out
}

/// Classifies every ink pixel in `mask` by its 8-neighbour signature.
///
/// Non-ink pixels stay as `Isolated` in the output; callers should
/// only consult entries whose underlying `mask[i]` is `true`.
pub(crate) fn classify_ink_pixels(mask: &[bool], w: usize, h: usize) -> Vec<super::skeleton_lut::Classification> {
    use super::skeleton_lut::{Classification, SKELETON_LUT};
    let mut out = vec![Classification::Isolated; mask.len()];
    if w == 0 || h == 0 {
        return out;
    }
    for y in 0..h {
        for x in 0..w {
            if !mask[y * w + x] {
                continue;
            }
            let code = neighbour_code(mask, w, h, x, y);
            out[y * w + x] = SKELETON_LUT[code as usize];
        }
    }
    out
}

/// `OpenToonz` 8-neighbour packed code (`tautoclose.cpp:54-59`).
fn neighbour_code(mask: &[bool], w: usize, h: usize, x: usize, y: usize) -> u8 {
    let bit = |dx: i32, dy: i32, shift: u32| -> u8 {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if nx < 0 || ny < 0 {
            return 0;
        }
        let nxu = nx as usize;
        let nyu = ny as usize;
        if nxu >= w || nyu >= h {
            return 0;
        }
        if mask[nyu * w + nxu] { 1 << shift } else { 0 }
    };
    bit(-1, 1, 0) | bit(0, 1, 1) | bit(1, 1, 2) | bit(-1, 0, 3) | bit(1, 0, 4) | bit(-1, -1, 5) | bit(0, -1, 6) | bit(1, -1, 7)
}

/// Estimates a unit tangent at endpoint `(x, y)` by walking up to
/// three pixels inward along the ink ridge.
///
/// The `OpenToonz` endpoint tangent (`tautoclose.cpp` ~ `visitEndpoint`)
/// runs a Bresenham-style multi-step walk and averages the
/// displacement vector. We use a simplified version: start at the
/// endpoint, follow each step to the unique ink neighbour that we
/// haven't visited, and return the unit vector from the endpoint to
/// the last-visited pixel. The simplification matters only for
/// near-degenerate strokes; the pairing test guards against bad
/// tangents by also requiring the connecting direction itself to fall
/// within the angle cone.
fn estimate_tangent(mask: &[bool], w: usize, h: usize, x: usize, y: usize) -> (f32, f32) {
    const STEPS: usize = 3;

    let in_bounds = |px: i32, py: i32| -> bool { px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h };
    let is_ink = |px: i32, py: i32| -> bool { in_bounds(px, py) && mask[(py as usize) * w + (px as usize)] };

    let mut visited = vec![false; mask.len()];
    visited[y * w + x] = true;
    let mut cx = x as i32;
    let mut cy = y as i32;
    let mut last = (cx, cy);

    for _ in 0..STEPS {
        // Look at the 8 neighbours; pick an unvisited ink one.
        let mut next: Option<(i32, i32)> = None;
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = cx + dx;
                let ny = cy + dy;
                if !is_ink(nx, ny) {
                    continue;
                }
                let idx = (ny as usize) * w + (nx as usize);
                if visited[idx] {
                    continue;
                }
                next = Some((nx, ny));
                break;
            }
            if next.is_some() {
                break;
            }
        }
        let Some((nx, ny)) = next else { break };
        visited[(ny as usize) * w + (nx as usize)] = true;
        cx = nx;
        cy = ny;
        last = (cx, cy);
    }

    let dx = (x as f32) - (last.0 as f32);
    let dy = (y as f32) - (last.1 as f32);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-6 {
        // Couldn't walk inward — fall back to zero so the connecting
        // direction alone decides the pairing.
        (0.0, 0.0)
    } else {
        (dx / len, dy / len)
    }
}

/// Pairs each endpoint with its nearest compatible partner under the
/// distance and angle constraints. Pairings are mutually exclusive:
/// once an endpoint is matched it's removed from further
/// consideration.
pub(crate) fn pair_endpoints(
    mask: &[bool],
    classes: &[super::skeleton_lut::Classification],
    w: usize,
    h: usize,
    max_distance: u32,
    max_angle: f32,
) -> Vec<((usize, usize), (usize, usize))> {
    use super::skeleton_lut::Classification;

    let endpoints: Vec<(usize, usize)> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| classes[y * w + x] == Classification::Endpoint)
        .collect();

    // Use i64 so the squared distance can't overflow for large canvases.
    let max_d2: i64 = i64::from(max_distance) * i64::from(max_distance);
    let cos_threshold = max_angle.cos();

    let mut paired = vec![false; endpoints.len()];
    let mut out: Vec<((usize, usize), (usize, usize))> = Vec::new();

    // Precompute tangents once per endpoint.
    let tangents: Vec<(f32, f32)> = endpoints.iter().map(|&(ex, ey)| estimate_tangent(mask, w, h, ex, ey)).collect();

    for i in 0..endpoints.len() {
        if paired[i] {
            continue;
        }
        let (xi, yi) = endpoints[i];
        let ti = tangents[i];
        let mut best: Option<(usize, i64)> = None;
        for j in (i + 1)..endpoints.len() {
            if paired[j] {
                continue;
            }
            let (xj, yj) = endpoints[j];
            let dx = xj as i64 - xi as i64;
            let dy = yj as i64 - yi as i64;
            let d2 = dx * dx + dy * dy;
            if d2 == 0 || d2 > max_d2 {
                continue;
            }
            let tj = tangents[j];
            if !angle_compatible(ti, tj, (xi, yi), (xj, yj), cos_threshold) {
                continue;
            }
            if best.is_none_or(|(_, prev)| d2 < prev) {
                best = Some((j, d2));
            }
        }
        if let Some((j, _)) = best {
            paired[i] = true;
            paired[j] = true;
            out.push((endpoints[i], endpoints[j]));
        }
    }
    out
}

fn angle_compatible(ti: (f32, f32), tj: (f32, f32), pi: (usize, usize), pj: (usize, usize), cos_threshold: f32) -> bool {
    let dx = pj.0 as f32 - pi.0 as f32;
    let dy = pj.1 as f32 - pi.1 as f32;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-6 {
        return false;
    }
    let nx = dx / len;
    let ny = dy / len;

    // If either tangent is degenerate (couldn't walk inward), fall back
    // to "no tangent constraint" for that side. Without this, an
    // endpoint with no inward neighbours would never pair.
    let ti_ok = (ti.0.abs() + ti.1.abs()) < 1e-6 || (ti.0 * nx + ti.1 * ny) >= cos_threshold;
    let tj_ok = (tj.0.abs() + tj.1.abs()) < 1e-6 || (-tj.0 * nx + -tj.1 * ny) >= cos_threshold;
    ti_ok && tj_ok
}

/// Bresenham line into a `SelectionMask`. Sets every pixel along the
/// segment to coverage 255.
pub(crate) fn rasterize_segment(mask: &mut SelectionMask, a: (u32, u32), b: (u32, u32)) {
    let (mut x0, mut y0) = (a.0 as i64, a.1 as i64);
    let (x1, y1) = (b.0 as i64, b.1 as i64);
    let dx = (x1 - x0).abs();
    let sx: i64 = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy: i64 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let w = i64::from(mask.width());
    let h = i64::from(mask.height());
    loop {
        if x0 >= 0 && y0 >= 0 && x0 < w && y0 < h {
            // Casts are safe by the bounds check above.
            mask.set(x0 as u32, y0 as u32, 255);
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::skeleton_lut::Classification;
    use super::*;
    use crate::project::Rgba;

    fn black() -> Rgba {
        Rgba::new(0, 0, 0, 255)
    }
    fn white() -> Rgba {
        Rgba::new(255, 255, 255, 255)
    }

    fn horizontal_line_buffer(len: u32) -> PixelBuffer {
        let mut buf = PixelBuffer::filled(len.max(1), 1, white()).unwrap();
        for x in 0..len {
            buf.set_pixel(x, 0, black());
        }
        buf
    }

    fn two_segments_with_gap_size(gap: u32) -> PixelBuffer {
        // Two 3-pixel horizontal segments with `gap` blank pixels
        // between them. Width spans both segments plus the gap.
        let seg = 3u32;
        let w = seg + gap + seg;
        let mut buf = PixelBuffer::filled(w, 1, white()).unwrap();
        for x in 0..seg {
            buf.set_pixel(x, 0, black());
        }
        for x in (seg + gap)..(seg + gap + seg) {
            buf.set_pixel(x, 0, black());
        }
        buf
    }

    #[test]
    fn config_defaults_match_doc() {
        let c = GapCloseConfig::default();
        assert_eq!(c.closing_distance, 10);
        assert!((c.closing_angle_rad - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert_eq!(c.ink_threshold, 128);
    }

    #[test]
    fn ink_mask_marks_pixels_below_threshold() {
        let mut buf = PixelBuffer::filled(4, 2, white()).unwrap();
        for x in 0..4 {
            buf.set_pixel(x, 0, black());
        }
        let mask = extract_ink_mask(&buf, 128);
        assert_eq!(mask.len(), 8);
        // Top row is ink.
        assert!(mask[0]);
        assert!(mask[1]);
        assert!(mask[2]);
        assert!(mask[3]);
        // Second row is background.
        assert!(!mask[4]);
        assert!(!mask[5]);
    }

    #[test]
    fn ink_mask_respects_threshold_boundary() {
        // A grey value exactly at threshold should be background; one
        // below should be ink.
        let mut buf = PixelBuffer::filled(2, 1, white()).unwrap();
        buf.set_pixel(0, 0, Rgba::new(127, 127, 127, 255));
        buf.set_pixel(1, 0, Rgba::new(128, 128, 128, 255));
        let m = extract_ink_mask(&buf, 128);
        assert!(m[0]);
        assert!(!m[1]);
    }

    #[test]
    fn classify_endpoint_at_line_terminus() {
        // 5-pixel horizontal line: (0,0)..(4,0).
        let buf = horizontal_line_buffer(5);
        let mask = extract_ink_mask(&buf, 128);
        let w = buf.width() as usize;
        let h = buf.height() as usize;
        let classes = classify_ink_pixels(&mask, w, h);
        // Endpoints at both ends, borders in the middle.
        assert_eq!(classes[0], Classification::Endpoint);
        assert_eq!(classes[4], Classification::Endpoint);
        assert_eq!(classes[2], Classification::Border);
    }

    #[test]
    fn pair_endpoints_at_2px_gap() {
        // Two 3-pixel segments with a 2-pixel gap: ink at x in [0..3)
        // and [5..8). Expected endpoints at (2, 0) and (5, 0).
        let buf = two_segments_with_gap_size(2);
        let mask = extract_ink_mask(&buf, 128);
        let w = buf.width() as usize;
        let h = buf.height() as usize;
        let classes = classify_ink_pixels(&mask, w, h);
        let pairs = pair_endpoints(&mask, &classes, w, h, 10, std::f32::consts::FRAC_PI_2);
        assert_eq!(pairs.len(), 1);
        let (a, b) = pairs[0];
        let pair_set = [(a, b), (b, a)];
        assert!(
            pair_set.iter().any(|(p, q)| *p == (2, 0) && *q == (5, 0)),
            "expected (2,0)<->(5,0), got {:?}",
            pairs[0]
        );
    }

    #[test]
    fn pair_endpoints_does_not_pair_12px_gap_at_default_distance() {
        let buf = two_segments_with_gap_size(12);
        let mask = extract_ink_mask(&buf, 128);
        let w = buf.width() as usize;
        let h = buf.height() as usize;
        let classes = classify_ink_pixels(&mask, w, h);
        let pairs = pair_endpoints(&mask, &classes, w, h, 10, std::f32::consts::FRAC_PI_2);
        assert!(pairs.is_empty(), "got {pairs:?}");
    }

    #[test]
    fn bresenham_horizontal_segment() {
        let mut mask = SelectionMask::new(10, 1).unwrap();
        rasterize_segment(&mut mask, (2, 0), (5, 0));
        for x in 2..=5 {
            assert!(mask.is_selected(x, 0));
        }
        assert!(!mask.is_selected(1, 0));
        assert!(!mask.is_selected(6, 0));
    }

    #[test]
    fn bresenham_diagonal_segment() {
        let mut mask = SelectionMask::new(10, 10).unwrap();
        rasterize_segment(&mut mask, (0, 0), (4, 4));
        for i in 0..=4 {
            assert!(mask.is_selected(i, i));
        }
        // Off-diagonal pixels should remain unset.
        assert!(!mask.is_selected(1, 0));
        assert!(!mask.is_selected(0, 1));
    }

    #[test]
    fn close_gaps_bridges_short_gap() {
        let buf = two_segments_with_gap_size(2);
        let m = close_gaps(&buf, &GapCloseConfig::default()).unwrap();
        // The endpoints are (2, 0) and (5, 0). Bresenham between them
        // covers x = 2..=5 — including the two gap pixels (3, 0) and
        // (4, 0).
        assert!(m.is_selected(3, 0));
        assert!(m.is_selected(4, 0));
    }

    #[test]
    fn close_gaps_leaves_large_gap_unbridged() {
        let buf = two_segments_with_gap_size(12);
        let m = close_gaps(&buf, &GapCloseConfig::default()).unwrap();
        assert!(!m.is_selected(3, 0));
        assert_eq!(m.selected_count(), 0);
    }
}
