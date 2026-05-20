// Adapted from OpenToonz toonz/sources/toonzlib/centerlinetostroke.cpp
// under BSD-3-Clause. See THIRD_PARTY_NOTICES.md.

//! Ramer-Douglas-Peucker simplification plus stroke construction.
//!
//! Each skeleton-graph edge becomes one `Stroke`. The path is first
//! simplified with RDP at `config.simplify_tolerance` (in pixels) and
//! then materialized into `Vertex`es whose thickness is interpolated
//! from the dense per-pixel thickness annotation. Thickness is
//! multiplied by `config.thickness_ratio` and clamped to
//! `config.max_thickness`.

use crate::config::CenterlineConfig;
use crate::types::{Stroke, Vertex};

/// Ramer-Douglas-Peucker on a polyline of `(x, y)` points.
///
/// Always retains the first and last points. Recursively keeps the
/// point that is farthest from the chord, then continues on each
/// half. Points within `tolerance` of the chord are dropped.
pub(crate) fn douglas_peucker(path: &[(f32, f32)], tolerance: f32) -> Vec<(f32, f32)> {
    if path.len() < 3 {
        return path.to_vec();
    }
    let mut keep = vec![false; path.len()];
    keep[0] = true;
    keep[path.len() - 1] = true;
    let mut stack: Vec<(usize, usize)> = vec![(0, path.len() - 1)];

    while let Some((i, j)) = stack.pop() {
        if j <= i + 1 {
            continue;
        }
        let (ax, ay) = path[i];
        let (bx, by) = path[j];
        let mut max_d = 0.0f32;
        let mut max_k = i;
        for k in (i + 1)..j {
            let (px, py) = path[k];
            let d = perpendicular_distance(ax, ay, bx, by, px, py);
            if d > max_d {
                max_d = d;
                max_k = k;
            }
        }
        if max_d > tolerance {
            keep[max_k] = true;
            stack.push((i, max_k));
            stack.push((max_k, j));
        }
    }

    path.iter()
        .enumerate()
        .filter_map(|(i, &p)| if keep[i] { Some(p) } else { None })
        .collect()
}

/// Distance from point `(px, py)` to the line segment
/// `((ax, ay), (bx, by))`. This is the point-to-segment distance, not
/// point-to-infinite-line: the projection is clamped to the segment so
/// RDP keeps points that sit beyond a segment end. Returns the chord
/// distance when the segment is degenerate.
fn perpendicular_distance(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    if len_sq < f32::EPSILON {
        let qx = px - ax;
        let qy = py - ay;
        return (qx * qx + qy * qy).sqrt();
    }
    let t = (((px - ax) * dx + (py - ay) * dy) / len_sq).clamp(0.0, 1.0);
    let cx = ax + t * dx;
    let cy = ay + t * dy;
    let qx = px - cx;
    let qy = py - cy;
    (qx * qx + qy * qy).sqrt()
}

/// Builds a `Stroke` from a `(x, y, thickness)` skeleton path.
///
/// 1. Strip the thickness column for RDP.
/// 2. Simplify the spatial polyline.
/// 3. For each kept `(x, y)`, look up the original thickness by
///    nearest-point match on the dense path and apply
///    `config.thickness_ratio` plus clamping.
pub(crate) fn fit_stroke(path: &[(f32, f32, f32)], config: &CenterlineConfig) -> Stroke {
    if path.is_empty() {
        return Stroke {
            vertices: Vec::new(),
            style_id: 0,
        };
    }
    let spatial: Vec<(f32, f32)> = path.iter().map(|&(x, y, _)| (x, y)).collect();
    let simplified = douglas_peucker(&spatial, config.simplify_tolerance);

    let vertices: Vec<Vertex> = simplified
        .into_iter()
        .map(|(x, y)| {
            let raw = thickness_at(path, x, y);
            let scaled = (raw * config.thickness_ratio).min(config.max_thickness);
            Vertex::new(x, y, scaled.max(0.0))
        })
        .collect();

    Stroke {
        vertices,
        style_id: 0,
    }
}

/// Looks up the thickness of the dense path point nearest to `(x, y)`.
fn thickness_at(path: &[(f32, f32, f32)], x: f32, y: f32) -> f32 {
    let mut best = path[0].2;
    let mut best_d = f32::MAX;
    for &(px, py, t) in path {
        let dx = px - x;
        let dy = py - y;
        let d = dx * dx + dy * dy;
        if d < best_d {
            best_d = d;
            best = t;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn douglas_peucker_keeps_short_paths_intact() {
        let p = vec![(0.0, 0.0), (1.0, 1.0)];
        let out = douglas_peucker(&p, 0.5);
        assert_eq!(out, p);
    }

    #[test]
    fn douglas_peucker_straight_line_keeps_endpoints() {
        let path = vec![(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (3.0, 0.0)];
        let out = douglas_peucker(&path, 0.5);
        assert_eq!(out, vec![(0.0, 0.0), (3.0, 0.0)]);
    }

    #[test]
    fn douglas_peucker_keeps_kink() {
        let path = vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (2.0, 1.0)];
        let out = douglas_peucker(&path, 0.1);
        // Kink point should be retained.
        assert!(out.len() >= 3);
    }

    #[test]
    fn fit_stroke_straight_line_one_segment_within_tolerance() {
        let path = vec![(0.0, 0.0, 1.0), (5.0, 0.0, 1.0), (10.0, 0.0, 1.0)];
        let stroke = fit_stroke(&path, &CenterlineConfig::default());
        assert_eq!(stroke.vertices.len(), 2);
        assert!((stroke.vertices[0].x - 0.0).abs() < 1e-6);
        assert!((stroke.vertices[1].x - 10.0).abs() < 1e-6);
    }

    #[test]
    fn fit_stroke_preserves_thickness() {
        let path = vec![(0.0, 0.0, 2.5), (10.0, 0.0, 2.5)];
        let stroke = fit_stroke(&path, &CenterlineConfig::default());
        assert!((stroke.vertices[0].thickness - 2.5).abs() < 1e-6);
    }

    #[test]
    fn fit_stroke_clamps_to_max_thickness() {
        let path = vec![(0.0, 0.0, 50.0), (10.0, 0.0, 50.0)];
        let cfg = CenterlineConfig {
            max_thickness: 10.0,
            ..CenterlineConfig::default()
        };
        let stroke = fit_stroke(&path, &cfg);
        assert!((stroke.vertices[0].thickness - 10.0).abs() < 1e-6);
    }

    #[test]
    fn fit_stroke_applies_thickness_ratio() {
        let path = vec![(0.0, 0.0, 4.0), (10.0, 0.0, 4.0)];
        let cfg = CenterlineConfig {
            thickness_ratio: 0.5,
            ..CenterlineConfig::default()
        };
        let stroke = fit_stroke(&path, &cfg);
        assert!((stroke.vertices[0].thickness - 2.0).abs() < 1e-6);
    }

    #[test]
    fn fit_stroke_empty_path_yields_no_vertices() {
        let stroke = fit_stroke(&[], &CenterlineConfig::default());
        assert!(stroke.vertices.is_empty());
    }

    #[test]
    fn perpendicular_distance_zero_on_segment() {
        let d = perpendicular_distance(0.0, 0.0, 10.0, 0.0, 5.0, 0.0);
        assert!(d < 1e-6);
    }

    #[test]
    fn perpendicular_distance_matches_height() {
        let d = perpendicular_distance(0.0, 0.0, 10.0, 0.0, 5.0, 3.0);
        assert!((d - 3.0).abs() < 1e-4);
    }

    #[test]
    fn perpendicular_distance_beyond_end_uses_endpoint() {
        // Point sits past the b endpoint along the line. Infinite-line
        // distance would be 0; segment distance is the gap to (10, 0).
        let d = perpendicular_distance(0.0, 0.0, 10.0, 0.0, 13.0, 4.0);
        assert!((d - 5.0).abs() < 1e-4, "expected 5.0, got {d}");
    }
}
