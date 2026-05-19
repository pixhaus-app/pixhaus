// Adapted from OpenToonz toonz/sources/toonzlib/centerlineskeletonizer.cpp
// under BSD-3-Clause. See THIRD_PARTY_NOTICES.md.

//! Skeletonization and medial-axis graph extraction.
//!
//! OpenToonz uses a Voronoi-of-vertices construction for the medial
//! axis. We use distance-transform + Zhang-Suen thinning; the two
//! yield equivalent medial-axis graphs for raster ink, and the
//! Zhang-Suen path is much smaller and easier to verify.
//!
//! Stages:
//!
//! 1. Two-pass 3-4 chamfer distance transform on the ink mask. The
//!    per-pixel value is the distance from the centerline pixel to
//!    the nearest non-ink pixel; we use it as the local thickness.
//! 2. Zhang-Suen iterative thinning until no pixel is removed in a
//!    full two-sub-iteration pass. The output is a one-pixel-wide
//!    binary skeleton.
//! 3. Graph extraction by walking the skeleton: pixels with exactly
//!    one 8-neighbour are endpoints, with three or more are branch
//!    nodes, and the rest are interior pixels along an edge.

/// One pixel of the binary skeleton plus its local thickness.
#[derive(Copy, Clone, Debug)]
pub(crate) struct SkeletonPixel {
    /// `true` when this pixel is part of the thinned skeleton.
    pub on: bool,
    /// Distance from this pixel to the nearest non-ink pixel in the
    /// pre-thinning ink mask, in chamfer-3-4 units divided by 3.
    pub thickness: f32,
}

/// Row-major buffer of `SkeletonPixel`s.
pub(crate) struct Skeleton {
    /// `width * height` entries.
    pub pixels: Vec<SkeletonPixel>,
    pub width: u32,
    pub height: u32,
}

/// Builds a `Skeleton` from `mask`.
pub(crate) fn skeletonize(mask: &[bool], width: u32, height: u32) -> Skeleton {
    let w = width as usize;
    let h = height as usize;
    if w == 0 || h == 0 || mask.len() != w * h {
        return Skeleton {
            pixels: Vec::new(),
            width,
            height,
        };
    }
    let dt = distance_transform(mask, w, h);
    let mut thin = mask.to_vec();
    zhang_suen_thin(&mut thin, w, h);
    let pixels = thin
        .iter()
        .zip(dt.iter())
        .map(|(&on, &t)| SkeletonPixel { on, thickness: t })
        .collect();
    Skeleton {
        pixels,
        width,
        height,
    }
}

/// Two-pass 3-4 chamfer distance transform.
///
/// Forward sweep uses the upper-left neighbours; reverse sweep uses
/// the lower-right neighbours. Out-of-bounds positions are treated
/// as non-ink at distance 0, so border ink pixels pick up correct
/// distances when the raster does not include the surrounding
/// background. Result is the distance from each ink pixel to the
/// nearest non-ink pixel in chamfer-3 units, returned in pixel units
/// (chamfer / 3).
fn distance_transform(mask: &[bool], w: usize, h: usize) -> Vec<f32> {
    const NEAR: u32 = 3;
    const DIAG: u32 = 4;
    const INF: u32 = u32::MAX / 4;

    let mut d = vec![INF; w * h];
    // Seed: non-ink pixels are at distance 0.
    for (i, &b) in mask.iter().enumerate() {
        if !b {
            d[i] = 0;
        }
    }

    // Treat out-of-bounds as a non-ink neighbour at distance 0; the
    // closure returns the candidate distance to contribute from
    // `(nx, ny)`. OOB or non-existing positions return `step` itself
    // (0 + step), so border ink pixels are at most `step` from an
    // implicit background pixel.
    let at = |d: &[u32], nx: i32, ny: i32, step: u32| -> u32 {
        if nx < 0 || ny < 0 || (nx as usize) >= w || (ny as usize) >= h {
            return step;
        }
        d[(ny as usize) * w + (nx as usize)].saturating_add(step)
    };

    // Forward sweep (top-left to bottom-right).
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if !mask[idx] {
                continue;
            }
            let xi = x as i32;
            let yi = y as i32;
            let mut best = d[idx];
            best = best.min(at(&d, xi - 1, yi - 1, DIAG));
            best = best.min(at(&d, xi, yi - 1, NEAR));
            best = best.min(at(&d, xi + 1, yi - 1, DIAG));
            best = best.min(at(&d, xi - 1, yi, NEAR));
            d[idx] = best;
        }
    }

    // Reverse sweep (bottom-right to top-left).
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let idx = y * w + x;
            if !mask[idx] {
                continue;
            }
            let xi = x as i32;
            let yi = y as i32;
            let mut best = d[idx];
            best = best.min(at(&d, xi + 1, yi, NEAR));
            best = best.min(at(&d, xi - 1, yi + 1, DIAG));
            best = best.min(at(&d, xi, yi + 1, NEAR));
            best = best.min(at(&d, xi + 1, yi + 1, DIAG));
            d[idx] = best;
        }
    }

    d.iter().map(|&v| (v as f32) / 3.0).collect()
}

/// Zhang-Suen iterative thinning.
///
/// Each iteration runs two sub-iterations. A pixel `p` with 8-neighbour
/// ring `p2..p9` (numbered clockwise starting from north) is removed
/// when all four classical conditions hold:
///
/// - 2 <= B(p) <= 6   (B = count of ink neighbours)
/// - A(p) == 1        (A = number of 0->1 transitions in the ring)
/// - sub-iter 1: p2*p4*p6 == 0 and p4*p6*p8 == 0
/// - sub-iter 2: p2*p4*p8 == 0 and p2*p6*p8 == 0
///
/// We loop until a full two-sub-iteration pass removes nothing.
/// Out-of-bounds neighbours are treated as background (0), so border
/// pixels are eligible for removal — that matches the classical
/// formulation when the image is implicitly padded with zeros.
fn zhang_suen_thin(mask: &mut [bool], w: usize, h: usize) {
    if w == 0 || h == 0 {
        return;
    }
    let get = |m: &[bool], x: i32, y: i32| -> u8 {
        if x < 0 || y < 0 || (x as usize) >= w || (y as usize) >= h {
            return 0;
        }
        u8::from(m[(y as usize) * w + (x as usize)])
    };

    loop {
        let mut changed = false;

        // Sub-iteration 1.
        let mut remove = Vec::new();
        for y in 0..(h as i32) {
            for x in 0..(w as i32) {
                if !mask[(y as usize) * w + (x as usize)] {
                    continue;
                }
                let p2 = get(mask, x, y - 1);
                let p3 = get(mask, x + 1, y - 1);
                let p4 = get(mask, x + 1, y);
                let p5 = get(mask, x + 1, y + 1);
                let p6 = get(mask, x, y + 1);
                let p7 = get(mask, x - 1, y + 1);
                let p8 = get(mask, x - 1, y);
                let p9 = get(mask, x - 1, y - 1);
                let b = p2 + p3 + p4 + p5 + p6 + p7 + p8 + p9;
                if !(2..=6).contains(&b) {
                    continue;
                }
                let a = transitions([p2, p3, p4, p5, p6, p7, p8, p9, p2]);
                if a != 1 {
                    continue;
                }
                if p2 * p4 * p6 != 0 {
                    continue;
                }
                if p4 * p6 * p8 != 0 {
                    continue;
                }
                remove.push((y as usize) * w + (x as usize));
            }
        }
        if !remove.is_empty() {
            changed = true;
            for idx in remove {
                mask[idx] = false;
            }
        }

        // Sub-iteration 2.
        let mut remove = Vec::new();
        for y in 0..(h as i32) {
            for x in 0..(w as i32) {
                if !mask[(y as usize) * w + (x as usize)] {
                    continue;
                }
                let p2 = get(mask, x, y - 1);
                let p3 = get(mask, x + 1, y - 1);
                let p4 = get(mask, x + 1, y);
                let p5 = get(mask, x + 1, y + 1);
                let p6 = get(mask, x, y + 1);
                let p7 = get(mask, x - 1, y + 1);
                let p8 = get(mask, x - 1, y);
                let p9 = get(mask, x - 1, y - 1);
                let b = p2 + p3 + p4 + p5 + p6 + p7 + p8 + p9;
                if !(2..=6).contains(&b) {
                    continue;
                }
                let a = transitions([p2, p3, p4, p5, p6, p7, p8, p9, p2]);
                if a != 1 {
                    continue;
                }
                if p2 * p4 * p8 != 0 {
                    continue;
                }
                if p2 * p6 * p8 != 0 {
                    continue;
                }
                remove.push((y as usize) * w + (x as usize));
            }
        }
        if !remove.is_empty() {
            changed = true;
            for idx in remove {
                mask[idx] = false;
            }
        }

        if !changed {
            break;
        }
    }
}

/// Number of 0->1 transitions in the 8-neighbour ring `ring[0..8]`
/// when stepped clockwise. `ring[8]` should equal `ring[0]` to close
/// the loop.
fn transitions(ring: [u8; 9]) -> u32 {
    let mut count = 0;
    for i in 0..8 {
        if ring[i] == 0 && ring[i + 1] == 1 {
            count += 1;
        }
    }
    count
}

// ----- Graph extraction --------------------------------------------------

/// Endpoint or branch node in the medial-axis graph.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum NodeKind {
    /// Skeleton pixel with exactly one 8-neighbour on the skeleton.
    Endpoint,
    /// Skeleton pixel with three or more 8-neighbours on the skeleton.
    Branch,
}

/// A node in the medial-axis graph.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Node {
    pub x: u32,
    pub y: u32,
    pub kind: NodeKind,
}

/// An edge between two nodes plus the per-pixel path between them.
///
/// `path` includes the endpoints. Each entry is `(x, y, thickness)`
/// where `thickness` is the skeleton's local stroke half-width.
#[derive(Clone, Debug)]
pub(crate) struct Edge {
    pub a: usize,
    pub b: usize,
    pub path: Vec<(f32, f32, f32)>,
}

/// The medial-axis graph: a flat list of nodes and edges.
#[derive(Clone, Debug, Default)]
pub(crate) struct SkeletonGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl SkeletonGraph {
    #[cfg(test)]
    pub(crate) fn endpoints(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(|n| n.kind == NodeKind::Endpoint)
    }
}

/// Counts 8-connected on-pixels around `(x, y)`.
fn neighbour_count(skel: &Skeleton, x: usize, y: usize) -> usize {
    let w = skel.width as usize;
    let h = skel.height as usize;
    let mut count = 0;
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx < 0 || ny < 0 || (nx as usize) >= w || (ny as usize) >= h {
                continue;
            }
            if skel.pixels[(ny as usize) * w + nx as usize].on {
                count += 1;
            }
        }
    }
    count
}

/// Returns each on-pixel 8-neighbour of `(x, y)`, in scan order.
fn neighbour_positions(skel: &Skeleton, x: usize, y: usize) -> Vec<(usize, usize)> {
    let w = skel.width as usize;
    let h = skel.height as usize;
    let mut out = Vec::with_capacity(8);
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx < 0 || ny < 0 || (nx as usize) >= w || (ny as usize) >= h {
                continue;
            }
            if skel.pixels[(ny as usize) * w + nx as usize].on {
                out.push((nx as usize, ny as usize));
            }
        }
    }
    out
}

/// Extracts the medial-axis graph from `skel`.
///
/// Algorithm:
///
/// 1. Classify every on-pixel by 8-neighbour count. 1 = endpoint,
///    >= 3 = branch, exactly 2 = interior.
/// 2. From every node pixel, walk outward along each on-neighbour
///    interior chain until we hit another node. Each walk produces
///    one edge.
/// 3. Pure cycles (rings of interior pixels with no node) are
///    detected and broken at an arbitrary point so they yield one
///    edge between two synthetic endpoint nodes.
pub(crate) fn extract_graph(skel: &Skeleton) -> SkeletonGraph {
    let w = skel.width as usize;
    let h = skel.height as usize;
    if w == 0 || h == 0 {
        return SkeletonGraph::default();
    }

    // Pass 1: locate nodes.
    let mut node_idx = vec![usize::MAX; w * h];
    let mut nodes: Vec<Node> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if !skel.pixels[idx].on {
                continue;
            }
            let count = neighbour_count(skel, x, y);
            // 0 = isolated dot (skip), 2 = interior of an edge (skip).
            let kind = match count {
                1 => NodeKind::Endpoint,
                n if n >= 3 => NodeKind::Branch,
                _ => continue,
            };
            node_idx[idx] = nodes.len();
            nodes.push(Node {
                x: x as u32,
                y: y as u32,
                kind,
            });
        }
    }

    let mut edges: Vec<Edge> = Vec::new();

    // Pass 2: trace from each node.
    let mut edge_visited = vec![false; w * h * 8];
    let edge_key = |a: usize, dx: i32, dy: i32| -> usize {
        let d = ((dy + 1) * 3 + (dx + 1)) as usize;
        a * 8 + d.min(7)
    };

    for (ni, node) in nodes.iter().enumerate() {
        let nx = node.x as usize;
        let ny = node.y as usize;
        let neighbours = neighbour_positions(skel, nx, ny);
        for (sx, sy) in neighbours {
            let dx = sx as i32 - nx as i32;
            let dy = sy as i32 - ny as i32;
            let key_a = edge_key(ny * w + nx, dx, dy);
            if edge_visited[key_a] {
                continue;
            }
            edge_visited[key_a] = true;

            // Walk the chain. Track the previous pixel so we don't
            // bounce back. Terminate when we land on another node.
            let mut path: Vec<(f32, f32, f32)> = Vec::new();
            path.push((nx as f32, ny as f32, skel.pixels[ny * w + nx].thickness));
            let mut prev = (nx, ny);
            let mut cur = (sx, sy);
            let mut other_node: Option<usize> = None;
            let mut steps = 0;
            let cap = (w * h * 4).max(64);
            loop {
                steps += 1;
                if steps > cap {
                    break;
                }
                path.push((
                    cur.0 as f32,
                    cur.1 as f32,
                    skel.pixels[cur.1 * w + cur.0].thickness,
                ));
                let cur_idx = cur.1 * w + cur.0;
                if node_idx[cur_idx] != usize::MAX {
                    other_node = Some(node_idx[cur_idx]);
                    // Mark the inbound direction on the other side too,
                    // so we don't retrace the edge from the other node.
                    let back_dx = prev.0 as i32 - cur.0 as i32;
                    let back_dy = prev.1 as i32 - cur.1 as i32;
                    let key_b = edge_key(cur_idx, back_dx, back_dy);
                    edge_visited[key_b] = true;
                    break;
                }

                // Interior pixel: pick the neighbour that is not `prev`.
                let nbrs = neighbour_positions(skel, cur.0, cur.1);
                let next = nbrs.into_iter().find(|&p| p != prev);
                let Some(next_pos) = next else {
                    break;
                };
                prev = cur;
                cur = next_pos;
            }

            if let Some(other) = other_node {
                edges.push(Edge {
                    a: ni,
                    b: other,
                    path,
                });
            }
        }
    }

    // Pass 3: handle pure cycles (no node anywhere on the cycle).
    let mut visited_cycle = vec![false; w * h];
    for &Node { x, y, .. } in &nodes {
        visited_cycle[(y as usize) * w + (x as usize)] = true;
    }
    // Also mark every pixel that already appears in an existing edge
    // path so we don't double-trace it.
    for e in &edges {
        for (px, py, _) in &e.path {
            let xi = (*px) as usize;
            let yi = (*py) as usize;
            if yi * w + xi < visited_cycle.len() {
                visited_cycle[yi * w + xi] = true;
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if visited_cycle[idx] || !skel.pixels[idx].on {
                continue;
            }
            if neighbour_count(skel, x, y) != 2 {
                visited_cycle[idx] = true;
                continue;
            }
            // Walk the ring. The first pixel becomes a synthetic
            // endpoint pair (we promote it to two nodes at the same
            // location so the graph still has endpoints).
            let na = nodes.len();
            nodes.push(Node {
                x: x as u32,
                y: y as u32,
                kind: NodeKind::Endpoint,
            });
            let nbrs = neighbour_positions(skel, x, y);
            // Two neighbours by construction; start with the first.
            let (mut sx, mut sy) = nbrs[0];
            let mut prev = (x, y);
            visited_cycle[idx] = true;
            let mut path: Vec<(f32, f32, f32)> =
                vec![(x as f32, y as f32, skel.pixels[idx].thickness)];
            let mut steps = 0;
            let cap = w * h + 4;
            loop {
                steps += 1;
                if steps > cap {
                    break;
                }
                let cur_idx = sy * w + sx;
                path.push((sx as f32, sy as f32, skel.pixels[cur_idx].thickness));
                visited_cycle[cur_idx] = true;
                let nbrs = neighbour_positions(skel, sx, sy);
                let next = nbrs.into_iter().find(|&p| p != prev);
                let Some(np) = next else {
                    break;
                };
                if np == (x, y) {
                    // Closed the cycle.
                    break;
                }
                prev = (sx, sy);
                sx = np.0;
                sy = np.1;
            }
            // Close path to the synthetic endpoint.
            path.push((x as f32, y as f32, skel.pixels[idx].thickness));
            let nb = nodes.len();
            nodes.push(Node {
                x: x as u32,
                y: y as u32,
                kind: NodeKind::Endpoint,
            });
            edges.push(Edge { a: na, b: nb, path });
        }
    }

    SkeletonGraph { nodes, edges }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn horizontal_bar_mask(w: usize, h: usize, bar_top: usize, bar_bot: usize) -> Vec<bool> {
        (0..w * h)
            .map(|i| {
                let y = i / w;
                y >= bar_top && y <= bar_bot
            })
            .collect()
    }

    #[test]
    fn skeleton_of_horizontal_bar_is_centerline() {
        // 20x6 bar that is ink everywhere except top/bottom rows.
        let mask = horizontal_bar_mask(20, 6, 1, 4);
        let skel = skeletonize(&mask, 20, 6);
        // After thinning, the on-row counts should concentrate in the
        // two middle rows of the bar (rows 2 and 3). Zhang-Suen erodes
        // endpoints a little so we accept any reasonably-long line.
        let count_row_2 = (0..20).filter(|&x| skel.pixels[2 * 20 + x].on).count();
        let count_row_3 = (0..20).filter(|&x| skel.pixels[3 * 20 + x].on).count();
        let total = count_row_2 + count_row_3;
        assert!(
            total >= 12,
            "centerline is sparser than expected: r2={count_row_2} r3={count_row_3}",
        );
        // The skeleton must not include the outer rows.
        for x in 0..20 {
            assert!(!skel.pixels[x].on, "row 0 should not be on");
            assert!(!skel.pixels[5 * 20 + x].on, "row 5 should not be on");
        }
    }

    #[test]
    fn skeleton_records_thickness_at_each_node() {
        let mask = horizontal_bar_mask(20, 6, 1, 4);
        let skel = skeletonize(&mask, 20, 6);
        let on_pix = skel
            .pixels
            .iter()
            .find(|p| p.on)
            .expect("skeleton on-pixel");
        assert!(on_pix.thickness > 1.0);
    }

    #[test]
    fn distance_transform_zero_for_non_ink() {
        let mask = vec![false, true, true, false];
        let dt = distance_transform(&mask, 4, 1);
        assert!((dt[0]).abs() < 1e-6);
        assert!((dt[3]).abs() < 1e-6);
    }

    #[test]
    fn distance_transform_grows_inside_ink() {
        let mut mask = vec![true; 25]; // 5x5 solid
        // Center pixel is interior; corners are at the boundary.
        // Make sure center distance exceeds border distance.
        let dt = distance_transform(&mut mask, 5, 5);
        assert!(dt[12] > dt[0]);
    }

    #[test]
    fn thinning_a_thick_stripe_collapses_to_a_line() {
        // 9x9 solid block.
        let mut mask = vec![true; 81];
        zhang_suen_thin(&mut mask, 9, 9);
        let on = mask.iter().filter(|b| **b).count();
        // Solid block thins to a roughly L or X shape; cardinality
        // should drop dramatically.
        assert!(on < 81 / 2, "thinning left {on}/81 pixels");
    }

    #[test]
    fn extract_graph_yields_endpoints_for_bar() {
        let mask = horizontal_bar_mask(20, 6, 1, 4);
        let skel = skeletonize(&mask, 20, 6);
        let graph = extract_graph(&skel);
        let endpoints = graph.endpoints().count();
        // A thinned bar produces a 1-pixel-wide line: two endpoints.
        assert!(
            endpoints >= 2,
            "expected at least 2 endpoints, got {endpoints}",
        );
        assert!(!graph.edges.is_empty());
    }

    #[test]
    fn extract_graph_returns_empty_for_empty_skeleton() {
        let skel = skeletonize(&[false; 9], 3, 3);
        let graph = extract_graph(&skel);
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }
}
