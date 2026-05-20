// Adapted from OpenToonz toonz/sources/toonzlib/centerlinepolygonizer.cpp
// under BSD-3-Clause. See THIRD_PARTY_NOTICES.md.

//! Contour extraction: raster ink mask to closed polygons.
//!
//! Each connected ink region produces one outer contour (traced
//! counter-clockwise in image coordinates) and one inner contour per
//! interior hole (traced clockwise). The implementation is a
//! Moore-neighbor tracing pass with a Jacob stopping criterion: a
//! contour closes when we revisit the start pixel from the same
//! incoming direction we entered it with.

/// A closed contour around an ink region or hole.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Contour {
    /// Pixel coordinates along the contour, in trace order.
    pub vertices: Vec<(i32, i32)>,
    /// `true` for the outer boundary of an ink region, `false` for a
    /// hole inside one.
    pub is_outer: bool,
}

/// Walks the ink mask and emits one `Contour` per region boundary.
///
/// Outer contours and hole contours are emitted in mask-scan order.
/// The first contour for any connected ink region is its outer
/// boundary; subsequent contours that lie inside it are holes.
pub(crate) fn polygonize(ink_mask: &[bool], width: u32, height: u32) -> Vec<Contour> {
    let w = width as usize;
    let h = height as usize;
    if w == 0 || h == 0 || ink_mask.len() != w * h {
        return Vec::new();
    }

    // Track which ink pixel started which contour so we don't retrace
    // the same boundary twice. We mark the starting pixel of every
    // contour we've already emitted.
    let mut traced_outer = vec![false; w * h];
    let mut traced_inner = vec![false; w * h];
    // Region IDs so each connected ink component is processed once.
    let mut region = vec![u32::MAX; w * h];
    let mut next_region: u32 = 0;

    let mut out = Vec::new();

    for y in 0..h {
        for x in 0..w {
            if !ink_mask[y * w + x] {
                continue;
            }
            if region[y * w + x] != u32::MAX {
                continue;
            }
            // New connected ink component. Flood-fill its region ID,
            // then trace its boundaries.
            let rid = next_region;
            next_region += 1;
            flood_label(ink_mask, &mut region, w, h, x, y, rid);

            // Trace outer contour starting at this pixel — it is by
            // construction the topmost-leftmost pixel of the region.
            if !traced_outer[y * w + x] {
                if let Some(verts) = trace_moore(ink_mask, w, h, (x, y), Direction::West, true) {
                    mark_starts(&verts, &mut traced_outer, w);
                    out.push(Contour {
                        vertices: verts,
                        is_outer: true,
                    });
                }
            }

            // Look for holes inside this region. Any background pixel
            // surrounded by region `rid` is the seed of a hole. We
            // trace the inner contour by walking around the hole from
            // an adjacent ink pixel.
            collect_inner_contours(ink_mask, &region, rid, w, h, &mut traced_inner, &mut out);
        }
    }

    out
}

/// Connected-component flood fill that paints `region` with `rid` for
/// every ink pixel reachable from `(sx, sy)` via 4-connectivity.
fn flood_label(
    ink: &[bool],
    region: &mut [u32],
    w: usize,
    h: usize,
    sx: usize,
    sy: usize,
    rid: u32,
) {
    let mut stack = vec![(sx, sy)];
    while let Some((x, y)) = stack.pop() {
        if region[y * w + x] != u32::MAX {
            continue;
        }
        if !ink[y * w + x] {
            continue;
        }
        region[y * w + x] = rid;
        if x > 0 {
            stack.push((x - 1, y));
        }
        if x + 1 < w {
            stack.push((x + 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if y + 1 < h {
            stack.push((x, y + 1));
        }
    }
}

/// Walks the 8 cardinals + diagonals around a contour cursor.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Direction {
    East,
    NorthEast,
    North,
    NorthWest,
    West,
    SouthWest,
    South,
    SouthEast,
}

impl Direction {
    const ALL: [Self; 8] = [
        Self::East,
        Self::NorthEast,
        Self::North,
        Self::NorthWest,
        Self::West,
        Self::SouthWest,
        Self::South,
        Self::SouthEast,
    ];

    fn step(self) -> (i32, i32) {
        match self {
            Self::East => (1, 0),
            Self::NorthEast => (1, -1),
            Self::North => (0, -1),
            Self::NorthWest => (-1, -1),
            Self::West => (-1, 0),
            Self::SouthWest => (-1, 1),
            Self::South => (0, 1),
            Self::SouthEast => (1, 1),
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|d| *d == self).unwrap_or(0)
    }

    /// Direction we approached the current cell from, given the
    /// direction we moved in.
    fn back_from(self) -> Self {
        Self::ALL[(self.index() + 4) % 8]
    }
}

/// Moore-neighbor contour trace.
///
/// Starts at `start`, walks the boundary by scanning the 8-neighbors
/// counterclockwise (for outer) or clockwise (for inner) from the
/// previous direction's back-neighbor, and stops when we revisit the
/// start pixel after the first step (Jacob's stopping criterion).
fn trace_moore(
    ink: &[bool],
    w: usize,
    h: usize,
    start: (usize, usize),
    enter_from: Direction,
    outer: bool,
) -> Option<Vec<(i32, i32)>> {
    let in_bounds = |x: i32, y: i32| x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h;
    let is_ink = |x: i32, y: i32| in_bounds(x, y) && ink[(y as usize) * w + x as usize];

    let (sx, sy) = (start.0 as i32, start.1 as i32);
    let mut contour = vec![(sx, sy)];

    // Initial scan: rotate around the start pixel from the
    // entry-back direction, looking for the first ink neighbour.
    let mut prev = enter_from;
    let start_enter_from = enter_from;
    let mut cur = (sx, sy);

    // Cap iteration as a safety net; a w*h-pixel boundary can be at
    // most ~8 * w * h long in pathological cases. Use a generous cap.
    let max_steps = (w * h * 8).max(256);
    let mut steps = 0;

    loop {
        steps += 1;
        if steps > max_steps {
            return Some(contour);
        }
        // Scan 8 directions starting from the back-neighbour, rotating
        // CCW for outer contours and CW for inner contours.
        let start_idx = prev.back_from().index();
        let mut next: Option<(i32, i32, Direction)> = None;
        for k in 1..=8 {
            let idx = if outer {
                (start_idx + k) % 8
            } else {
                (start_idx + 8 - k) % 8
            };
            let dir = Direction::ALL[idx];
            let (dx, dy) = dir.step();
            let nx = cur.0 + dx;
            let ny = cur.1 + dy;
            if is_ink(nx, ny) {
                next = Some((nx, ny, dir));
                break;
            }
        }

        let Some((nx, ny, ndir)) = next else {
            // Isolated pixel: no neighbours. Return the single-vertex
            // contour we already have.
            return Some(contour);
        };

        // Jacob stopping criterion: stop only when we re-enter the start
        // pixel from the same direction as the initial scan. Stopping on
        // any revisit of the start pixel can close early and truncate a
        // contour that legitimately touches the start from another side.
        if (nx, ny) == (sx, sy) && ndir == start_enter_from {
            return Some(contour);
        }

        contour.push((nx, ny));
        prev = ndir;
        cur = (nx, ny);
    }
}

/// Marks every traced pixel as already-seen so we don't retrace it.
fn mark_starts(verts: &[(i32, i32)], marks: &mut [bool], w: usize) {
    for &(x, y) in verts {
        if x < 0 || y < 0 {
            continue;
        }
        let idx = (y as usize) * w + x as usize;
        if idx < marks.len() {
            marks[idx] = true;
        }
    }
}

/// Finds holes inside region `rid` and traces their inner contours.
///
/// A background pixel that has any 4-neighbour belonging to region
/// `rid` and is reachable only via region `rid` (i.e. not connected
/// to the outside background) is the seed of an interior hole.
fn collect_inner_contours(
    ink: &[bool],
    region: &[u32],
    rid: u32,
    w: usize,
    h: usize,
    traced_inner: &mut [bool],
    out: &mut Vec<Contour>,
) {
    // Flood-fill the background pixels reachable from any image-border
    // cell — those are the "outside". Anything left over and bordered
    // by region `rid` ink is an interior hole.
    let mut outside = vec![false; w * h];
    let mut stack = Vec::new();
    for x in 0..w {
        if !ink[x] {
            stack.push((x, 0));
        }
        let bot = (h - 1) * w + x;
        if !ink[bot] {
            stack.push((x, h - 1));
        }
    }
    for y in 0..h {
        if !ink[y * w] {
            stack.push((0, y));
        }
        if !ink[y * w + (w - 1)] {
            stack.push((w - 1, y));
        }
    }
    while let Some((x, y)) = stack.pop() {
        let idx = y * w + x;
        if outside[idx] || ink[idx] {
            continue;
        }
        outside[idx] = true;
        if x > 0 {
            stack.push((x - 1, y));
        }
        if x + 1 < w {
            stack.push((x + 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if y + 1 < h {
            stack.push((x, y + 1));
        }
    }

    // Walk in scan order; the first background cell of a new hole that
    // is adjacent to region `rid` becomes the trace seed. Mark visited
    // hole cells so subsequent scans skip them.
    let mut hole_visited = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if ink[idx] || outside[idx] || hole_visited[idx] {
                continue;
            }
            // This is a hole pixel inside some ink region. Confirm at
            // least one ink neighbour belongs to region `rid`; if not,
            // skip — it belongs to a different region.
            let mut touches_rid = false;
            for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                    continue;
                }
                let nidx = (ny as usize) * w + nx as usize;
                if ink[nidx] && region[nidx] == rid {
                    touches_rid = true;
                    break;
                }
            }
            if !touches_rid {
                // Flood the hole component so we don't keep checking
                // its other cells.
                flood_hole(ink, &mut hole_visited, w, h, x, y);
                continue;
            }

            // Find the adjacent ink pixel that seeds the trace. Pick
            // the one to the west when possible so trace_moore enters
            // from the east (inside the hole).
            let seed = find_inner_seed(ink, region, rid, w, h, x, y);
            if let Some((ix, iy, enter)) = seed {
                if !traced_inner[iy * w + ix] {
                    if let Some(verts) = trace_moore(ink, w, h, (ix, iy), enter, false) {
                        mark_starts(&verts, traced_inner, w);
                        out.push(Contour {
                            vertices: verts,
                            is_outer: false,
                        });
                    }
                }
            }
            flood_hole(ink, &mut hole_visited, w, h, x, y);
        }
    }
}

fn flood_hole(ink: &[bool], hole: &mut [bool], w: usize, h: usize, sx: usize, sy: usize) {
    let mut stack = vec![(sx, sy)];
    while let Some((x, y)) = stack.pop() {
        let idx = y * w + x;
        if hole[idx] || ink[idx] {
            continue;
        }
        hole[idx] = true;
        if x > 0 {
            stack.push((x - 1, y));
        }
        if x + 1 < w {
            stack.push((x + 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if y + 1 < h {
            stack.push((x, y + 1));
        }
    }
}

/// Given a hole pixel at `(hx, hy)`, finds an adjacent ink pixel that
/// belongs to region `rid` and returns the direction we approach it
/// from. We prefer west-of-hole because trace_moore with `outer=false`
/// (clockwise rotation) reliably winds clockwise around the hole.
fn find_inner_seed(
    ink: &[bool],
    region: &[u32],
    rid: u32,
    w: usize,
    h: usize,
    hx: usize,
    hy: usize,
) -> Option<(usize, usize, Direction)> {
    // Prefer the ink pixel directly above the hole pixel; we enter it
    // from the south, then the CW scan starts from the south-back =
    // north and rotates CW (north -> NW -> W -> ...). That direction
    // happens to wind clockwise around the hole.
    let candidates = [
        (hx as i32, hy as i32 - 1, Direction::South),
        (hx as i32 - 1, hy as i32, Direction::East),
        (hx as i32, hy as i32 + 1, Direction::North),
        (hx as i32 + 1, hy as i32, Direction::West),
    ];
    for (ix, iy, enter) in candidates {
        if ix < 0 || iy < 0 || ix as usize >= w || iy as usize >= h {
            continue;
        }
        let idx = (iy as usize) * w + ix as usize;
        if ink[idx] && region[idx] == rid {
            return Some((ix as usize, iy as usize, enter));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_mask_yields_no_contours() {
        let mask = vec![false; 9];
        let contours = polygonize(&mask, 3, 3);
        assert!(contours.is_empty());
    }

    #[test]
    fn single_pixel_blob_yields_one_contour() {
        let mut mask = vec![false; 9];
        mask[4] = true; // center of 3x3
        let contours = polygonize(&mask, 3, 3);
        assert_eq!(contours.len(), 1);
        assert!(contours[0].is_outer);
        assert_eq!(contours[0].vertices, vec![(1, 1)]);
    }

    #[test]
    fn solid_square_yields_one_outer_contour() {
        let mask = vec![true; 16]; // 4x4 solid
        let contours = polygonize(&mask, 4, 4);
        assert_eq!(contours.len(), 1);
        assert!(contours[0].is_outer);
        // Every perimeter pixel should appear in the trace.
        assert!(contours[0].vertices.len() >= 4);
    }

    #[test]
    fn solid_square_with_hole_yields_outer_plus_inner() {
        // 6x6 with a 2x2 hole in the middle.
        let mut mask = vec![true; 36];
        for y in 2..=3 {
            for x in 2..=3 {
                mask[y * 6 + x] = false;
            }
        }
        let contours = polygonize(&mask, 6, 6);
        assert_eq!(contours.len(), 2);
        let outer = contours.iter().filter(|c| c.is_outer).count();
        let inner = contours.iter().filter(|c| !c.is_outer).count();
        assert_eq!(outer, 1);
        assert_eq!(inner, 1);
    }

    #[test]
    fn two_disconnected_blobs_yield_two_contours() {
        // 5x1: two single-pixel blobs separated by a gap.
        let mask = vec![true, false, false, false, true];
        let contours = polygonize(&mask, 5, 1);
        assert_eq!(contours.len(), 2);
        assert!(contours.iter().all(|c| c.is_outer));
    }

    #[test]
    fn dimension_mismatch_yields_no_contours() {
        let mask = vec![true; 4];
        let contours = polygonize(&mask, 3, 3);
        assert!(contours.is_empty());
    }
}
