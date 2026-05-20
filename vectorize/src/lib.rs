//! Centerline vectorization.
//!
//! Converts a raster ink layer into a `VectorImage` of styled strokes
//! via a four-stage pipeline: polygonize, skeletonize, organize, and
//! stroke fit.
//!
//! Adapted from OpenToonz `toonz/sources/toonzlib/` under BSD-3-Clause.
//! See `THIRD_PARTY_NOTICES.md`.

// The skeleton, contour, and graph routines work with raster pixel
// coordinates that fit comfortably in `u32`/`i32`, so the casts in
// the pixel-walking loops are sound by construction. Document and
// allow the pedantic cast lints at crate level rather than littering
// every site with `#[allow]`.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_lossless,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::doc_markdown,
    clippy::missing_const_for_fn,
    clippy::needless_range_loop
)]
// Tests in this crate use `unwrap`/`expect` freely; production code in
// the rest of the crate keeps the workspace-level clippy denials.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::missing_panics_doc,
        clippy::disallowed_methods,
        clippy::unnecessary_mut_passed,
        clippy::float_cmp
    )
)]

pub mod config;
pub mod error;
pub mod types;

pub(crate) mod contour;
pub(crate) mod organize;
pub(crate) mod skeleton;
pub(crate) mod stroke_fit;

pub use config::CenterlineConfig;
pub use error::{Error, Result};
pub use types::{Stroke, VectorImage, Vertex};

use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::Palette;

/// Converts `raster` into a `VectorImage` of centerline strokes.
///
/// The pipeline runs four stages:
///
/// 1. Polygonize — extract closed polygon contours from each
///    connected ink region.
/// 2. Skeletonize — distance transform plus Zhang-Suen thinning
///    yields a medial-axis graph annotated with per-pixel thickness.
/// 3. Organize — prune branches shorter than
///    `config.min_segment_length` and merge through degree-2 nodes.
/// 4. Stroke fit — Ramer-Douglas-Peucker simplification with
///    thickness-preserving stroke construction per remaining edge.
///
/// `palette` is currently only validated for emptiness; the
/// integration with the cleanup verb that consumes its style IDs is
/// a follow-up stream.
///
/// # Errors
///
/// - `Error::EmptyRaster` when `raster` has zero width or height.
/// - `Error::EmptyPalette` when `palette.colors` is empty.
pub fn centerline_vectorize(
    raster: &PixelBuffer,
    palette: &Palette,
    config: &CenterlineConfig,
) -> Result<VectorImage> {
    if raster.width() == 0 || raster.height() == 0 {
        return Err(Error::EmptyRaster);
    }
    if palette.colors.is_empty() {
        return Err(Error::EmptyPalette);
    }

    let width = raster.width();
    let height = raster.height();
    let ink_mask = extract_ink_mask(raster);

    // Stage 1: polygonize. Held for future API exposure; the medial
    // axis is sufficient for stroke construction in this PR.
    let _contours = contour::polygonize(&ink_mask, width, height);

    // Stage 2: skeletonize + graph extraction.
    let skel = skeleton::skeletonize(&ink_mask, width, height);
    let graph = skeleton::extract_graph(&skel);

    // Stage 3: organize.
    let organized = organize::organize(graph, config);

    // Stage 4: stroke fit.
    let strokes: Vec<Stroke> = organized
        .edges
        .iter()
        .map(|e| stroke_fit::fit_stroke(&e.path, config))
        .filter(|s| s.vertices.len() >= 2)
        .collect();

    Ok(VectorImage {
        strokes,
        width,
        height,
    })
}

/// Builds a row-major boolean ink mask from `raster`.
///
/// A pixel is "ink" when its Rec.709 integer luma is below 128.
/// Mirrors the helper in `core::selection::autoclose` so the two
/// pipelines agree on what counts as ink.
fn extract_ink_mask(raster: &PixelBuffer) -> Vec<bool> {
    let w = raster.width();
    let h = raster.height();
    let mut mask = Vec::with_capacity((w as usize) * (h as usize));
    for y in 0..h {
        for x in 0..w {
            let p = raster
                .pixel(x, y)
                .unwrap_or_else(|| pixhaus_core::project::Rgba::new(255, 255, 255, 255));
            let luma = (54u32 * u32::from(p.r) + 183 * u32::from(p.g) + 19 * u32::from(p.b)) >> 8;
            mask.push(luma < 128);
        }
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::{PaletteId, Rgba};

    fn solid_palette() -> Palette {
        Palette::from_colors(PaletteId::new(0), "test", vec![Rgba::new(0, 0, 0, 255)])
    }

    #[test]
    fn empty_raster_errors() {
        let buf = PixelBuffer::empty();
        let palette = solid_palette();
        let err = centerline_vectorize(&buf, &palette, &CenterlineConfig::default()).unwrap_err();
        assert!(matches!(err, Error::EmptyRaster));
    }

    #[test]
    fn empty_palette_errors() {
        let buf = PixelBuffer::new(4, 4).unwrap();
        let palette = Palette::from_colors(PaletteId::new(0), "empty", vec![]);
        let err = centerline_vectorize(&buf, &palette, &CenterlineConfig::default()).unwrap_err();
        assert!(matches!(err, Error::EmptyPalette));
    }

    #[test]
    fn extract_ink_mask_marks_dark_pixels() {
        let mut buf = PixelBuffer::new(2, 1).unwrap();
        buf.set_pixel(0, 0, Rgba::opaque(0, 0, 0));
        buf.set_pixel(1, 0, Rgba::opaque(255, 255, 255));
        let mask = extract_ink_mask(&buf);
        assert_eq!(mask, vec![true, false]);
    }
}
