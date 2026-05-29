//! Grid-sheet slicing: cut one composite sheet into ordered cell buffers.
//!
//! This is the inverse of laying frames out in a grid. A creative model paints
//! one `rows × cols` sheet on a solid key background; the deterministic slicer
//! cuts it into cells the normalize/key tail then cleans. The slicer makes no
//! aesthetic decision — it only decides geometry, so the same `(rows, cols)` (or
//! the same explicit rects) reproduces the same cuts.
//!
//! Two entry points:
//!
//! - [`slice_grid`] cuts `rows * cols` equal cells, row-major. Cell size is
//!   floor-divided, so a sheet whose dimensions are not a clean multiple of the
//!   grid drops the trailing remainder rather than emitting a short final cell.
//!   That keeps every cell uniform, which the shared-scale normalize pass needs.
//! - [`slice_rects`] cuts at explicit `(x, y, w, h)` rectangles, for Structures
//!   that declare gutters, labels, or a palette swatch the naive grid would
//!   mis-cut. Each rect is clamped to the sheet by the underlying [`crop`].
//!
//! Both loop over the existing public [`crop`] — there is no new pixel-copy
//! primitive here, only the cell math.

use crate::canvas::buffer::PixelBuffer;

use super::error::{Error, Result};
use super::resize::crop;

/// Cuts `sheet` into `rows * cols` equal cells in row-major order.
///
/// Cell dimensions are floor-divided (`sheet.width() / cols`,
/// `sheet.height() / rows`); any trailing remainder on the right or bottom edge
/// is dropped so every returned cell is the same size. The result is ordered
/// left-to-right within a row, then top-to-bottom — cell `r * cols + c` is the
/// sub-rect rooted at `(c * cell_w, r * cell_h)`. Deterministic.
///
/// # Errors
///
/// [`Error::EmptyBuffer`] when `sheet` is 0×0, when `rows` or `cols` is 0, or
/// when the floor-divided cell size collapses to zero (the grid is finer than
/// the sheet on an axis).
pub fn slice_grid(sheet: &PixelBuffer, rows: u32, cols: u32) -> Result<Vec<PixelBuffer>> {
    if sheet.is_empty() || rows == 0 || cols == 0 {
        return Err(Error::EmptyBuffer);
    }
    let cell_w = sheet.width() / cols;
    let cell_h = sheet.height() / rows;
    if cell_w == 0 || cell_h == 0 {
        return Err(Error::EmptyBuffer);
    }

    let mut cells = Vec::with_capacity((rows as usize) * (cols as usize));
    for r in 0..rows {
        for c in 0..cols {
            // `c < cols` and `r < rows`, so `c * cell_w < cols * cell_w <=
            // width` and likewise for the height — the origin is always inside
            // the sheet and the floor-divided cell never overhangs.
            cells.push(crop(sheet, c * cell_w, r * cell_h, cell_w, cell_h)?);
        }
    }
    Ok(cells)
}

/// Cuts `sheet` at the explicit `(x, y, w, h)` rectangles, one cell per rect, in
/// the order given.
///
/// Each rect is passed verbatim to [`crop`], which clamps it to the sheet: a
/// rect that overhangs an edge yields a cell of the requested size with the
/// out-of-bounds region left transparent, never an error. Use this over
/// [`slice_grid`] when a Structure declares gutters or a non-cell region (a
/// label row, a palette swatch) the uniform grid would mis-cut.
///
/// # Errors
///
/// [`Error::EmptyBuffer`] when `sheet` is 0×0, when `rects` is empty, or when
/// any rect has zero width or height (a zero-area cell yields no buffer).
pub fn slice_rects(sheet: &PixelBuffer, rects: &[(u32, u32, u32, u32)]) -> Result<Vec<PixelBuffer>> {
    if sheet.is_empty() || rects.is_empty() {
        return Err(Error::EmptyBuffer);
    }
    rects.iter().map(|&(x, y, w, h)| crop(sheet, x, y, w, h)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Rgba;

    use proptest::prelude::*;

    fn color(seed: u8) -> Rgba {
        Rgba::opaque(seed, 255 - seed, 128)
    }

    /// A `cols`-wide, `rows`-tall sheet of `cell`-sized blocks, each block filled
    /// with a distinct opaque colour keyed off its row-major index, on a magenta
    /// surround so a remainder strip is visibly not a cell.
    fn block_sheet(rows: u32, cols: u32, cell: u32, pad_right: u32, pad_bottom: u32) -> PixelBuffer {
        let width = cols * cell + pad_right;
        let height = rows * cell + pad_bottom;
        let mut sheet = PixelBuffer::filled(width, height, Rgba::opaque(255, 0, 255)).expect("sheet");
        for r in 0..rows {
            for c in 0..cols {
                #[allow(clippy::cast_possible_truncation)]
                let fill = color((r * cols + c) as u8);
                for y in 0..cell {
                    for x in 0..cell {
                        sheet.set_pixel(c * cell + x, r * cell + y, fill);
                    }
                }
            }
        }
        sheet
    }

    mod slice_grid {
        use super::*;

        #[test]
        fn cuts_a_2x2_sheet_into_four_row_major_cells() {
            // One distinct opaque square per quadrant; the cut order is
            // top-left, top-right, bottom-left, bottom-right.
            let sheet = block_sheet(2, 2, 3, 0, 0);
            let cells = slice_grid(&sheet, 2, 2).expect("slice");
            assert_eq!(cells.len(), 4);
            for (i, cell) in cells.iter().enumerate() {
                assert_eq!((cell.width(), cell.height()), (3, 3), "cell {i} size");
                #[allow(clippy::cast_possible_truncation)]
                let expected = color(i as u8);
                assert_eq!(cell.pixel(1, 1), Some(expected), "cell {i} centre colour");
            }
        }

        #[test]
        fn cell_origin_matches_row_major_quadrant() {
            // Pin the geometry: cell (r, c) is rooted at (c*cell_w, r*cell_h).
            // The bottom-right cell's top-left pixel is the sheet's (3, 3).
            let sheet = block_sheet(2, 2, 3, 0, 0);
            let cells = slice_grid(&sheet, 2, 2).expect("slice");
            let bottom_right = &cells[3];
            assert_eq!(bottom_right.pixel(0, 0), sheet.pixel(3, 3));
            assert_eq!(bottom_right.pixel(2, 2), sheet.pixel(5, 5));
        }

        #[test]
        fn drops_the_trailing_remainder_so_cells_stay_uniform() {
            // A 7-wide sheet over 2 cols floor-divides to 3-wide cells; the 7th
            // column is dropped, not folded into a short final cell.
            let sheet = block_sheet(2, 2, 3, 1, 1);
            let cells = slice_grid(&sheet, 2, 2).expect("slice");
            assert_eq!(cells.len(), 4);
            for cell in &cells {
                assert_eq!((cell.width(), cell.height()), (3, 3), "uniform cell size");
            }
        }

        #[test]
        fn errors_on_empty_sheet() {
            assert_eq!(slice_grid(&PixelBuffer::empty(), 2, 2), Err(Error::EmptyBuffer));
        }

        #[test]
        fn errors_on_zero_rows_or_cols() {
            let sheet = block_sheet(2, 2, 3, 0, 0);
            assert_eq!(slice_grid(&sheet, 0, 2), Err(Error::EmptyBuffer));
            assert_eq!(slice_grid(&sheet, 2, 0), Err(Error::EmptyBuffer));
        }

        #[test]
        fn errors_when_the_grid_is_finer_than_the_sheet() {
            // A 4-wide sheet over 5 cols floor-divides to a zero-wide cell.
            let sheet = block_sheet(1, 4, 1, 0, 0);
            assert_eq!(slice_grid(&sheet, 1, 5), Err(Error::EmptyBuffer));
        }

        #[test]
        fn one_by_one_returns_the_whole_sheet() {
            let sheet = block_sheet(1, 1, 4, 0, 0);
            let cells = slice_grid(&sheet, 1, 1).expect("slice");
            assert_eq!(cells.len(), 1);
            assert_eq!(cells[0], sheet);
        }
    }

    mod slice_rects {
        use super::*;

        #[test]
        fn cuts_non_uniform_cells_at_the_given_rects() {
            // Two cells of different sizes, plus a gutter the rects skip over.
            let sheet = block_sheet(1, 2, 4, 0, 0);
            let rects = [(0, 0, 4, 4), (5, 0, 3, 4)];
            let cells = slice_rects(&sheet, &rects).expect("slice");
            assert_eq!(cells.len(), 2);
            assert_eq!((cells[0].width(), cells[0].height()), (4, 4));
            assert_eq!((cells[1].width(), cells[1].height()), (3, 4));
            assert_eq!(cells[0].pixel(0, 0), Some(color(0)));
        }

        #[test]
        fn clamps_an_overhanging_rect_to_transparent() {
            // A rect rooted inside the sheet but overhanging the right/bottom
            // edge yields a full-size cell with the off-sheet region transparent.
            let sheet = block_sheet(1, 1, 2, 0, 0);
            let cells = slice_rects(&sheet, &[(1, 1, 3, 3)]).expect("slice");
            assert_eq!((cells[0].width(), cells[0].height()), (3, 3));
            let opaque = cells[0].pixels().filter(|p| p.a != 0).count();
            assert_eq!(opaque, 1, "only the one in-bounds pixel is copied");
        }

        #[test]
        fn errors_on_empty_rect_list() {
            let sheet = block_sheet(1, 1, 2, 0, 0);
            assert_eq!(slice_rects(&sheet, &[]), Err(Error::EmptyBuffer));
        }

        #[test]
        fn errors_on_a_zero_area_rect() {
            let sheet = block_sheet(1, 1, 2, 0, 0);
            assert_eq!(slice_rects(&sheet, &[(0, 0, 0, 2)]), Err(Error::EmptyBuffer));
        }

        #[test]
        fn errors_on_empty_sheet() {
            assert_eq!(slice_rects(&PixelBuffer::empty(), &[(0, 0, 1, 1)]), Err(Error::EmptyBuffer));
        }
    }

    proptest! {
        /// Random sheet sizes and grids never panic and yield exactly
        /// `rows * cols` cells, each the floor-divided cell size.
        #[test]
        fn slice_grid_yields_uniform_cells(
            width in 1u32..=64,
            height in 1u32..=64,
            rows in 1u32..=8,
            cols in 1u32..=8,
        ) {
            let sheet = PixelBuffer::filled(width, height, Rgba::opaque(255, 0, 255)).expect("sheet");
            let cell_w = width / cols;
            let cell_h = height / rows;
            match slice_grid(&sheet, rows, cols) {
                Ok(cells) => {
                    // Cells are produced only when both axes have room.
                    prop_assert!(cell_w > 0 && cell_h > 0);
                    prop_assert_eq!(cells.len(), (rows as usize) * (cols as usize));
                    for cell in &cells {
                        prop_assert_eq!((cell.width(), cell.height()), (cell_w, cell_h));
                    }
                }
                Err(Error::EmptyBuffer) => {
                    // The only failure is a collapsed cell on a finer-than-sheet grid.
                    prop_assert!(cell_w == 0 || cell_h == 0);
                }
                Err(other) => prop_assert!(false, "unexpected error: {other:?}"),
            }
        }
    }

    /// Visual regression: a 2×3 sheet sliced into six cells, recomposited into a
    /// single horizontal strip, compared against a committed baseline PNG.
    /// Regenerate with `PIXHAUS_UPDATE_SNAPSHOTS=1 cargo test -p pixhaus-core
    /// slice_grid_strip_matches_baseline` after an intentional change, and audit
    /// the new image in the PR.
    #[test]
    fn slice_grid_strip_matches_baseline() {
        use std::path::Path;

        let sheet = block_sheet(2, 3, 8, 0, 0);
        let cells = slice_grid(&sheet, 2, 3).expect("slice");
        let cell = 8u32;
        #[allow(clippy::cast_possible_truncation)]
        let strip_w = cell * cells.len() as u32;
        let mut strip = PixelBuffer::new(strip_w, cell).expect("strip");
        for (i, c) in cells.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let ox = i as u32 * cell;
            for y in 0..cell {
                for x in 0..cell {
                    if let Some(px) = c.pixel(x, y) {
                        strip.set_pixel(ox + x, y, px);
                    }
                }
            }
        }
        let actual: image::RgbaImage =
            image::RgbaImage::from_raw(strip.width(), strip.height(), strip.as_bytes().to_vec()).expect("strip bytes form a valid RGBA image");

        let baseline_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/slice_grid_strip.png");
        if std::env::var_os("PIXHAUS_UPDATE_SNAPSHOTS").is_some() {
            if let Some(dir) = baseline_path.parent() {
                std::fs::create_dir_all(dir).expect("create snapshot dir");
            }
            actual.save(&baseline_path).expect("write baseline png");
            return;
        }
        let baseline = image::open(&baseline_path)
            .expect("baseline png is present; regenerate with PIXHAUS_UPDATE_SNAPSHOTS=1")
            .to_rgba8();
        let result = image_compare::rgba_hybrid_compare(&actual, &baseline).expect("image dimensions match");
        assert!(result.score >= 0.999, "sliced strip diverged from baseline: score = {}", result.score);
    }
}
