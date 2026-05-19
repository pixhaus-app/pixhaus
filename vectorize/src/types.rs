//! Public output types: `Vertex`, `Stroke`, `VectorImage`.
//!
//! Adapted from OpenToonz `toonz/sources/include/tstroke.h` shape
//! under BSD-3-Clause. See `THIRD_PARTY_NOTICES.md`.

use serde::{Deserialize, Serialize};

/// A single vertex on a stroke's centerline.
///
/// Coordinates are in raster pixel space (top-left origin). `thickness`
/// is the local stroke half-width — the distance from the centerline
/// to either edge at this vertex.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    /// Horizontal coordinate in pixel space.
    pub x: f32,
    /// Vertical coordinate in pixel space.
    pub y: f32,
    /// Local stroke half-width at this vertex.
    pub thickness: f32,
}

impl Vertex {
    /// Constructs a vertex.
    #[must_use]
    pub const fn new(x: f32, y: f32, thickness: f32) -> Self {
        Self { x, y, thickness }
    }
}

/// A single styled stroke: a polyline of `Vertex` values plus the
/// palette style ID that drives its rendering.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    /// Ordered vertices that define the centerline.
    pub vertices: Vec<Vertex>,
    /// Palette style ID. Currently always `0` — wiring up the style
    /// classifier is a follow-up stream.
    pub style_id: u32,
}

/// The result of vectorizing a raster ink layer.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VectorImage {
    /// All strokes extracted from the input raster.
    pub strokes: Vec<Stroke>,
    /// Width of the source raster, in pixels.
    pub width: u32,
    /// Height of the source raster, in pixels.
    pub height: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_image_default_is_empty() {
        let v = VectorImage::default();
        assert!(v.strokes.is_empty());
        assert_eq!(v.width, 0);
        assert_eq!(v.height, 0);
    }

    #[test]
    fn vertex_new_round_trips() {
        let v = Vertex::new(1.5, 2.5, 3.0);
        assert!((v.x - 1.5).abs() < f32::EPSILON);
        assert!((v.y - 2.5).abs() < f32::EPSILON);
        assert!((v.thickness - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn stroke_holds_vertices_and_style() {
        let s = Stroke {
            vertices: vec![Vertex::new(0.0, 0.0, 1.0), Vertex::new(10.0, 0.0, 1.0)],
            style_id: 7,
        };
        assert_eq!(s.vertices.len(), 2);
        assert_eq!(s.style_id, 7);
    }
}
