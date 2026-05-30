//! Grid-sheet slicing: cut a single image into a row-major grid of cells.
//!
//! The AI static-sheet path asks an image backend for one sheet that packs every
//! animation frame into a grid, then slices it back into frames. This module owns
//! the slicing half.

use serde::{Deserialize, Serialize};

use super::error::{Error, Result};
use super::resize::crop;
use crate::canvas::buffer::PixelBuffer;

/// The slicing geometry for a grid sheet. All margins are in sheet pixels.
///
/// A sheet is cut into `rows * cols` cells, row-major (left to right, top to
/// bottom). `offset_x`/`offset_y` move the grid origin in from the top-left
/// (for a sheet with a border), `gutter_x`/`gutter_y` are the gaps between
/// cells, and `inset` trims every cell inward on all four sides (to drop a
/// per-cell border without changing the grid spacing).
///
/// `overrides` (Phase B) moves individual interior dividers off the uniform
/// grid, for variable cell sizes; it defaults empty, so a grid with no
/// overrides resolves exactly as the uniform Phase A spec. See [`SliceOverrides`].
///
/// [`SliceGrid::uniform`] builds the zero-margin case that reproduces
/// [`slice_grid`] exactly.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SliceGrid {
    /// Number of rows (cells down).
    pub rows: u32,
    /// Number of columns (cells across).
    pub cols: u32,
    /// Pixels skipped on the left before the first column.
    pub offset_x: u32,
    /// Pixels skipped on the top before the first row.
    pub offset_y: u32,
    /// Horizontal gap between adjacent columns.
    pub gutter_x: u32,
    /// Vertical gap between adjacent rows.
    pub gutter_y: u32,
    /// Per-cell inward trim applied on all four sides.
    pub inset: u32,
    /// Per-divider position overrides for the non-uniform Phase B cut. Empty for
    /// a uniform grid (the Phase A case), which is the serde default so older
    /// data without the field loads with no overrides.
    #[serde(default, skip_serializing_if = "SliceOverrides::is_empty")]
    pub overrides: SliceOverrides,
}

/// Custom interior-divider positions for a non-uniform [`SliceGrid`] cut.
///
/// Each entry is `(divider_index, position)` in sheet pixels. An x-divider with
/// index `d` (in `1..cols`) is the boundary between column `d - 1` and column
/// `d`; moving it sets column `d - 1`'s right edge to `position` and column
/// `d`'s left edge to `position + gutter_x`, so variable column widths fall out.
/// y-dividers work the same way over rows. Indices outside `1..cols` / `1..rows`
/// are ignored, and the uniform divider is used for any index without an entry.
///
/// An empty `SliceOverrides` means "no overrides": the resolve falls through to
/// the uniform [`slice_grid_spec`] path and is byte-identical to it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SliceOverrides {
    /// Overridden vertical dividers (between columns), `(divider_index, x)`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub x: Vec<(u32, u32)>,
    /// Overridden horizontal dividers (between rows), `(divider_index, y)`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub y: Vec<(u32, u32)>,
}

impl SliceOverrides {
    /// True when no divider is overridden — the uniform Phase A case.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.x.is_empty() && self.y.is_empty()
    }
}

impl SliceGrid {
    /// Build the zero-margin grid: `rows * cols` cells with no offset, gutter,
    /// inset, or overrides.
    ///
    /// This is the configuration that reproduces [`slice_grid`] exactly.
    pub fn uniform(rows: u32, cols: u32) -> Self {
        SliceGrid {
            rows,
            cols,
            offset_x: 0,
            offset_y: 0,
            gutter_x: 0,
            gutter_y: 0,
            inset: 0,
            overrides: SliceOverrides::default(),
        }
    }
}

/// Cut `sheet` into `rows * cols` equal cells, row-major (left to right, top to
/// bottom).
///
/// Every cell is the same size: `width / cols` by `height / rows`. Remainder
/// pixels on the right and bottom edges are dropped, matching how the static
/// sheet packs frames on an exact grid.
///
/// # Errors
///
/// Returns [`Error::EmptyBuffer`] if `sheet` has zero area, if `rows` or `cols`
/// is zero, or if either derived cell dimension would be zero.
pub fn slice_grid(sheet: &PixelBuffer, rows: u32, cols: u32) -> Result<Vec<PixelBuffer>> {
    if rows == 0 || cols == 0 {
        return Err(Error::EmptyBuffer);
    }
    let cell_w = sheet.width() / cols;
    let cell_h = sheet.height() / rows;
    slice_grid_impl(sheet, rows, cols, cell_w, cell_h)
}

fn slice_grid_impl(sheet: &PixelBuffer, rows: u32, cols: u32, cell_w: u32, cell_h: u32) -> Result<Vec<PixelBuffer>> {
    if cell_w == 0 || cell_h == 0 {
        return Err(Error::EmptyBuffer);
    }
    let mut cells = Vec::with_capacity((rows * cols) as usize);
    for r in 0..rows {
        for c in 0..cols {
            let x = c * cell_w;
            let y = r * cell_h;
            let cell = crop(sheet, x, y, cell_w, cell_h)?;
            cells.push(cell);
        }
    }
    Ok(cells)
}

/// Cut `sheet` into cells per `spec`, row-major (left to right, top to bottom).
///
/// The cell size is derived from the sheet size and the grid margins:
///
/// ```text
/// cell_w = (width  - offset_x - (cols - 1) * gutter_x) / cols
/// cell_h = (height - offset_y - (rows - 1) * gutter_y) / rows
/// ```
///
/// Cell `(r, c)` is rooted at `(offset_x + c * (cell_w + gutter_x) + inset,
/// offset_y + r * (cell_h + gutter_y) + inset)` with size
/// `(cell_w - 2 * inset, cell_h - 2 * inset)`. Every cut routes through
/// [`crop`]. A [`SliceGrid::uniform`] spec reproduces [`slice_grid`] exactly.
///
/// All margin arithmetic saturates, so an over-large margin or inset collapses
/// a derived dimension to zero rather than wrapping.
///
/// # Errors
///
/// Returns [`Error::EmptyBuffer`] if `rows` or `cols` is zero, or if any
/// derived dimension collapses to zero (a margin larger than the sheet, or an
/// inset that eats the whole cell).
pub fn slice_grid_spec(sheet: &PixelBuffer, spec: &SliceGrid) -> Result<Vec<PixelBuffer>> {
    // Phase B: a spec with per-divider overrides resolves to explicit rects and
    // cuts through `slice_rects`. The uniform (no-override) path below stays the
    // exact Phase A result.
    if !spec.overrides.is_empty() {
        return slice_grid_resolve(sheet, spec);
    }

    let SliceGrid {
        rows,
        cols,
        offset_x,
        offset_y,
        gutter_x,
        gutter_y,
        inset,
        overrides: _,
    } = *spec;

    if rows == 0 || cols == 0 {
        return Err(Error::EmptyBuffer);
    }

    // Width left for the columns after the leading offset and the inter-column
    // gutters, then divided evenly. Saturating subtraction means an over-large
    // margin yields zero usable width, which the cell-size guard rejects below.
    let cols_gutter = gutter_x.saturating_mul(cols - 1);
    let rows_gutter = gutter_y.saturating_mul(rows - 1);
    let usable_w = sheet.width().saturating_sub(offset_x).saturating_sub(cols_gutter);
    let usable_h = sheet.height().saturating_sub(offset_y).saturating_sub(rows_gutter);
    let cell_w = usable_w / cols;
    let cell_h = usable_h / rows;

    // The inset trims both sides, so it consumes `2 * inset` of the cell.
    let inset2 = inset.saturating_mul(2);
    let inner_w = cell_w.saturating_sub(inset2);
    let inner_h = cell_h.saturating_sub(inset2);
    if inner_w == 0 || inner_h == 0 {
        return Err(Error::EmptyBuffer);
    }

    let mut cells = Vec::with_capacity((rows * cols) as usize);
    for r in 0..rows {
        for c in 0..cols {
            let x = offset_x + c * (cell_w + gutter_x) + inset;
            let y = offset_y + r * (cell_h + gutter_y) + inset;
            let cell = crop(sheet, x, y, inner_w, inner_h)?;
            cells.push(cell);
        }
    }
    Ok(cells)
}

/// Resolves a [`SliceGrid`] with per-divider overrides to explicit rects and
/// cuts through [`slice_rects`]. The non-uniform Phase B path: each overridden
/// interior divider moves the boundary between two adjacent cells, so cell
/// widths and heights vary.
///
/// The uniform divider positions seed the cut; an override `(d, pos)` for x
/// replaces divider `d` (the boundary after column `d - 1`), setting that
/// column's right edge to `pos` and the next column's left edge to
/// `pos + gutter_x`. y works the same over rows. The leading offset, trailing
/// edge, gutters on non-overridden dividers, and the per-cell inset all carry
/// through from the uniform grid, so an empty override set yields the exact
/// Phase A rects — callers should prefer [`slice_grid_spec`], which falls
/// through here only when overrides are present.
///
/// # Errors
///
/// Returns [`Error::EmptyBuffer`] if `rows` or `cols` is zero, or if any
/// resolved cell collapses to zero area (an override that crosses its
/// neighbour, or an inset that eats the cell).
pub fn slice_grid_resolve(sheet: &PixelBuffer, spec: &SliceGrid) -> Result<Vec<PixelBuffer>> {
    if spec.rows == 0 || spec.cols == 0 {
        return Err(Error::EmptyBuffer);
    }
    let xs = resolved_axis_edges(sheet.width(), spec.cols, spec.offset_x, spec.gutter_x, &spec.overrides.x);
    let ys = resolved_axis_edges(sheet.height(), spec.rows, spec.offset_y, spec.gutter_y, &spec.overrides.y);

    // The inset trims both sides of every resolved cell, matching the uniform
    // path. Saturating so an over-large inset collapses the cell (rejected below).
    let inset = spec.inset;
    let inset2 = inset.saturating_mul(2);

    let mut rects = Vec::with_capacity((spec.rows * spec.cols) as usize);
    for (yt, yb) in &ys {
        for (xl, xr) in &xs {
            let w = xr.saturating_sub(*xl).saturating_sub(inset2);
            let h = yb.saturating_sub(*yt).saturating_sub(inset2);
            if w == 0 || h == 0 {
                return Err(Error::EmptyBuffer);
            }
            rects.push((xl + inset, yt + inset, w, h));
        }
    }
    slice_rects(sheet, &rects)
}

/// The `(left, right)` edge pairs for one axis after applying any divider
/// overrides. `count` cells span `span` pixels from `offset`, with `gutter`
/// between cells; the uniform cell size seeds every edge, then each override
/// `(index, position)` (index in `1..count`) repositions that interior divider.
///
/// An override sets cell `index - 1`'s right edge to `position` and cell
/// `index`'s left edge to `position + gutter`. The returned pairs are clamped
/// to `[0, span]` and ordered so a resolved cell never reads negative width
/// (the caller rejects a zero-width cell).
fn resolved_axis_edges(span: u32, count: u32, offset: u32, gutter: u32, overrides: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let count = count.max(1);
    let gutter_total = gutter.saturating_mul(count - 1);
    let usable = span.saturating_sub(offset).saturating_sub(gutter_total);
    let cell = usable / count;

    // Seed the uniform left/right of every cell, then overwrite the shared
    // dividers the overrides move. Saturate the seed like the rest of the
    // module: `x_edges`/`y_edges` are public and run on a `SliceGrid`
    // deserialized straight from a file, so an adversarial offset/gutter must
    // clamp rather than overflow in a debug build.
    let mut lefts: Vec<u32> = (0..count)
        .map(|c| offset.saturating_add(c.saturating_mul(cell.saturating_add(gutter))))
        .collect();
    let mut rights: Vec<u32> = lefts.iter().map(|&l| l.saturating_add(cell)).collect();
    for &(index, pos) in overrides {
        // Divider `index` is the boundary after cell `index - 1`; ignore an
        // index that names no interior divider.
        if index == 0 || index >= count {
            continue;
        }
        let pos = pos.min(span);
        let prev = (index - 1) as usize;
        let next = index as usize;
        // Keep the pair ordered against its own cell's far edge so a crossed
        // divider collapses to zero rather than wrapping.
        rights[prev] = pos.max(lefts[prev]).min(span);
        lefts[next] = pos.saturating_add(gutter).max(rights[prev]).min(span);
    }
    lefts.into_iter().zip(rights).map(|(l, r)| (l, r.max(l))).collect()
}

impl SliceGrid {
    /// The resolved `(left, right)` edge pairs for every column over a
    /// `sheet_w`-wide sheet, with any x overrides applied. One pair per column,
    /// left to right; the gap between a pair's right and the next pair's left is
    /// the gutter. The shell's slice gizmo draws and hit-tests against these so
    /// the overlay matches the cut [`slice_grid_resolve`] makes.
    #[must_use]
    pub fn x_edges(&self, sheet_w: u32) -> Vec<(u32, u32)> {
        resolved_axis_edges(sheet_w, self.cols, self.offset_x, self.gutter_x, &self.overrides.x)
    }

    /// The resolved `(top, bottom)` edge pairs for every row over a
    /// `sheet_h`-tall sheet, with any y overrides applied. The vertical
    /// companion to [`SliceGrid::x_edges`].
    #[must_use]
    pub fn y_edges(&self, sheet_h: u32) -> Vec<(u32, u32)> {
        resolved_axis_edges(sheet_h, self.rows, self.offset_y, self.gutter_y, &self.overrides.y)
    }
}

/// Cut `sheet` into the rectangles in `rects` (each `(x, y, w, h)`), in order.
///
/// # Errors
///
/// Returns [`Error::EmptyBuffer`] if any rectangle has zero area or escapes the
/// sheet bounds.
pub fn slice_rects(sheet: &PixelBuffer, rects: &[(u32, u32, u32, u32)]) -> Result<Vec<PixelBuffer>> {
    rects.iter().map(|&(x, y, w, h)| crop(sheet, x, y, w, h)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::buffer::PixelBuffer;
    use proptest::prelude::*;
    use rstest::rstest;

    /// Build a sheet where each pixel encodes its (x, y) so cuts are checkable:
    /// the red channel is the low byte of x, green the low byte of y.
    fn gradient_sheet(width: u32, height: u32) -> PixelBuffer {
        let mut px = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                // Low byte only; `& 0xff` keeps each value in 0..=255.
                px.push(low_byte(x));
                px.push(low_byte(y));
                px.push(0);
                px.push(255);
            }
        }
        PixelBuffer::from_raw(width, height, width * 4, px).expect("valid buffer")
    }

    /// The low byte of `v` as a `u8`, masked so the cast can never truncate.
    fn low_byte(v: u32) -> u8 {
        u8::try_from(v & 0xff).expect("masked to a single byte")
    }

    /// The top-left pixel of a cell, as the `(x, y)` it encodes.
    fn corner(cell: &PixelBuffer) -> (u8, u8) {
        let px = cell.pixel(0, 0).expect("cell has a top-left pixel");
        (px.r, px.g)
    }

    #[rstest]
    #[case(2, 2)]
    #[case(1, 4)]
    #[case(4, 1)]
    #[case(3, 5)]
    fn slices_into_equal_cells(#[case] rows: u32, #[case] cols: u32) {
        let sheet = gradient_sheet(64, 64);
        let cells = slice_grid(&sheet, rows, cols).expect("slice");
        assert_eq!(cells.len() as u32, rows * cols);
        let cell_w = 64 / cols;
        let cell_h = 64 / rows;
        for cell in &cells {
            assert_eq!(cell.width(), cell_w);
            assert_eq!(cell.height(), cell_h);
        }
    }

    #[test]
    fn rejects_zero_grid() {
        let sheet = gradient_sheet(16, 16);
        assert!(matches!(slice_grid(&sheet, 0, 4), Err(Error::EmptyBuffer)));
        assert!(matches!(slice_grid(&sheet, 4, 0), Err(Error::EmptyBuffer)));
    }

    #[test]
    fn rejects_cell_smaller_than_grid() {
        let sheet = gradient_sheet(2, 2);
        assert!(matches!(slice_grid(&sheet, 4, 4), Err(Error::EmptyBuffer)));
    }

    #[test]
    fn uniform_builds_zero_margin_grid() {
        let grid = SliceGrid::uniform(2, 3);
        assert_eq!(grid.rows, 2);
        assert_eq!(grid.cols, 3);
        assert_eq!(grid.offset_x, 0);
        assert_eq!(grid.offset_y, 0);
        assert_eq!(grid.gutter_x, 0);
        assert_eq!(grid.gutter_y, 0);
        assert_eq!(grid.inset, 0);
    }

    /// The load-bearing invariant: a uniform spec slices byte-identically to the
    /// zero-margin `slice_grid`.
    #[rstest]
    #[case(64, 64, 2, 2)]
    #[case(64, 64, 1, 4)]
    #[case(64, 64, 4, 1)]
    #[case(48, 60, 3, 5)]
    #[case(30, 30, 7, 7)]
    fn uniform_matches_slice_grid(#[case] width: u32, #[case] height: u32, #[case] rows: u32, #[case] cols: u32) {
        let sheet = gradient_sheet(width, height);
        let plain = slice_grid(&sheet, rows, cols).expect("slice_grid");
        let spec = slice_grid_spec(&sheet, &SliceGrid::uniform(rows, cols)).expect("slice_grid_spec");
        assert_eq!(plain.len(), spec.len());
        for (a, b) in plain.iter().zip(spec.iter()) {
            assert_eq!(a.width(), b.width());
            assert_eq!(a.height(), b.height());
            assert_eq!(a.as_bytes(), b.as_bytes());
        }
    }

    #[test]
    fn offset_shifts_the_grid_origin() {
        // A 64x64 sheet, 2x2 grid, offset 8 px in from the top-left. Usable area
        // is 56x56, so each cell is 28x28 and the first cell starts at (8, 8).
        let sheet = gradient_sheet(64, 64);
        let spec = SliceGrid {
            offset_x: 8,
            offset_y: 8,
            ..SliceGrid::uniform(2, 2)
        };
        let cells = slice_grid_spec(&sheet, &spec).expect("slice");
        assert_eq!(cells.len(), 4);
        for cell in &cells {
            assert_eq!(cell.width(), 28);
            assert_eq!(cell.height(), 28);
        }
        assert_eq!(corner(&cells[0]), (8, 8));
        assert_eq!(corner(&cells[1]), (36, 8));
        assert_eq!(corner(&cells[2]), (8, 36));
        assert_eq!(corner(&cells[3]), (36, 36));
    }

    #[test]
    fn gutter_inserts_gaps_between_cells() {
        // 64x64 sheet, 2x2 grid, 4 px gutter both ways. Usable width is
        // 64 - (2-1)*4 = 60, so each cell is 30x30. Cell stride is cell + gutter
        // = 34, so the second column starts at x = 34.
        let sheet = gradient_sheet(64, 64);
        let spec = SliceGrid {
            gutter_x: 4,
            gutter_y: 4,
            ..SliceGrid::uniform(2, 2)
        };
        let cells = slice_grid_spec(&sheet, &spec).expect("slice");
        for cell in &cells {
            assert_eq!(cell.width(), 30);
            assert_eq!(cell.height(), 30);
        }
        assert_eq!(corner(&cells[0]), (0, 0));
        assert_eq!(corner(&cells[1]), (34, 0));
        assert_eq!(corner(&cells[2]), (0, 34));
        assert_eq!(corner(&cells[3]), (34, 34));
    }

    #[test]
    fn inset_trims_each_cell_inward() {
        // 64x64 sheet, 2x2 grid (32x32 cells), inset 4 px. Each cell shrinks to
        // 24x24 and its origin moves in by the inset.
        let sheet = gradient_sheet(64, 64);
        let spec = SliceGrid {
            inset: 4,
            ..SliceGrid::uniform(2, 2)
        };
        let cells = slice_grid_spec(&sheet, &spec).expect("slice");
        for cell in &cells {
            assert_eq!(cell.width(), 24);
            assert_eq!(cell.height(), 24);
        }
        assert_eq!(corner(&cells[0]), (4, 4));
        assert_eq!(corner(&cells[1]), (36, 4));
        assert_eq!(corner(&cells[2]), (4, 36));
        assert_eq!(corner(&cells[3]), (36, 36));
    }

    #[rstest]
    // Zero rows or cols.
    #[case(SliceGrid::uniform(0, 4))]
    #[case(SliceGrid::uniform(4, 0))]
    // Offset larger than the sheet.
    #[case(SliceGrid { offset_x: 100, ..SliceGrid::uniform(2, 2) })]
    #[case(SliceGrid { offset_y: 100, ..SliceGrid::uniform(2, 2) })]
    // Gutters that consume the whole sheet.
    #[case(SliceGrid { gutter_x: 100, ..SliceGrid::uniform(2, 2) })]
    // An inset that eats the whole cell (32x32 cells, inset 16 leaves nothing).
    #[case(SliceGrid { inset: 16, ..SliceGrid::uniform(2, 2) })]
    fn rejects_collapsed_dimensions(#[case] spec: SliceGrid) {
        let sheet = gradient_sheet(64, 64);
        assert!(matches!(slice_grid_spec(&sheet, &spec), Err(Error::EmptyBuffer)));
    }

    #[test]
    fn slice_grid_spec_round_trips_through_rmp() {
        let grid = SliceGrid {
            rows: 3,
            cols: 5,
            offset_x: 2,
            offset_y: 4,
            gutter_x: 1,
            gutter_y: 1,
            inset: 2,
            ..SliceGrid::uniform(3, 5)
        };
        let bytes = rmp_serde::to_vec(&grid).expect("encode");
        let back: SliceGrid = rmp_serde::from_slice(&bytes).expect("decode");
        assert_eq!(grid, back);
    }

    proptest! {
        #[test]
        fn slice_grid_never_panics(
            width in 1u32..=128,
            height in 1u32..=128,
            rows in 1u32..=16,
            cols in 1u32..=16,
        ) {
            let sheet = gradient_sheet(width, height);
            let _ = slice_grid(&sheet, rows, cols);
        }
    }

    proptest! {
        #[test]
        fn slice_grid_yields_grid_cells(
            cells_w in 1u32..=8,
            cells_h in 1u32..=8,
            cell in 1u32..=16,
        ) {
            let width = cells_w * cell;
            let height = cells_h * cell;
            let sheet = gradient_sheet(width, height);
            let cells = slice_grid(&sheet, cells_h, cells_w).expect("slice");
            prop_assert_eq!(cells.len() as u32, cells_w * cells_h);
            for c in &cells {
                prop_assert_eq!(c.width(), cell);
                prop_assert_eq!(c.height(), cell);
            }
        }
    }

    proptest! {
        #[test]
        fn slice_grid_spec_never_panics(
            width in 1u32..=128,
            height in 1u32..=128,
            rows in 0u32..=16,
            cols in 0u32..=16,
            offset_x in 0u32..=32,
            offset_y in 0u32..=32,
            gutter_x in 0u32..=16,
            gutter_y in 0u32..=16,
            inset in 0u32..=16,
        ) {
            let sheet = gradient_sheet(width, height);
            let spec = SliceGrid {
                rows,
                cols,
                offset_x,
                offset_y,
                gutter_x,
                gutter_y,
                inset,
                ..SliceGrid::uniform(rows, cols)
            };
            let _ = slice_grid_spec(&sheet, &spec);
        }
    }

    proptest! {
        #[test]
        fn slice_grid_spec_yields_derived_cells(
            cols in 1u32..=8,
            rows in 1u32..=8,
            cell in 2u32..=16,
            offset_x in 0u32..=8,
            offset_y in 0u32..=8,
            gutter_x in 0u32..=4,
            gutter_y in 0u32..=4,
            inset in 0u32..=1,
        ) {
            // Size the sheet so the derived cell width and height are exact: the
            // usable span divides evenly into `cell`-wide cells.
            let width = offset_x + cols * cell + (cols - 1) * gutter_x;
            let height = offset_y + rows * cell + (rows - 1) * gutter_y;
            let sheet = gradient_sheet(width, height);
            let spec = SliceGrid {
                rows,
                cols,
                offset_x,
                offset_y,
                gutter_x,
                gutter_y,
                inset,
                ..SliceGrid::uniform(rows, cols)
            };
            match slice_grid_spec(&sheet, &spec) {
                Ok(cells) => {
                    prop_assert_eq!(cells.len() as u32, rows * cols);
                    let inner = cell - 2 * inset;
                    for c in &cells {
                        prop_assert_eq!(c.width(), inner);
                        prop_assert_eq!(c.height(), inner);
                    }
                }
                Err(Error::EmptyBuffer) => {
                    // Only an inset that eats the whole cell collapses here.
                    prop_assert!(cell <= 2 * inset);
                }
                Err(e) => prop_assert!(false, "unexpected error: {e:?}"),
            }
        }
    }

    #[test]
    fn slice_rects_cuts_named_regions() {
        let sheet = gradient_sheet(32, 32);
        let rects = [(0u32, 0u32, 8u32, 8u32), (8, 8, 16, 16)];
        let cells = slice_rects(&sheet, &rects).expect("slice");
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].width(), 8);
        assert_eq!(cells[0].height(), 8);
        assert_eq!(cells[1].width(), 16);
        assert_eq!(cells[1].height(), 16);
    }

    #[test]
    fn empty_overrides_is_the_uniform_case() {
        assert!(SliceOverrides::default().is_empty());
        assert!(SliceGrid::uniform(2, 3).overrides.is_empty());
    }

    #[test]
    fn x_edges_seed_a_uniform_grid() {
        // A plain 4-wide, 2-col grid over 8 px: two 4-wide cells, no gap.
        let spec = SliceGrid::uniform(1, 2);
        assert_eq!(spec.x_edges(8), vec![(0, 4), (4, 8)]);
    }

    #[test]
    fn y_edges_seed_a_uniform_grid() {
        let spec = SliceGrid::uniform(2, 1);
        assert_eq!(spec.y_edges(8), vec![(0, 4), (4, 8)]);
    }

    #[test]
    fn x_edges_saturate_on_an_adversarial_gutter() {
        // A spec deserialized from a file can carry a huge gutter that would
        // overflow the non-saturating seed in a debug build. The clamp keeps
        // every edge within the span instead of panicking.
        let spec = SliceGrid {
            rows: 1,
            cols: 3,
            gutter_x: u32::MAX,
            ..SliceGrid::uniform(1, 3)
        };
        let edges = spec.x_edges(16);
        assert_eq!(edges.len(), 3, "one pair per column");
        for (lo, hi) in edges {
            assert!(lo <= 16 && hi <= 16, "edges stay within the span: ({lo}, {hi})");
            assert!(lo <= hi, "a cell never inverts: ({lo}, {hi})");
        }
    }

    #[test]
    fn y_edges_saturate_on_an_adversarial_offset() {
        let spec = SliceGrid {
            rows: 2,
            cols: 1,
            offset_y: u32::MAX,
            ..SliceGrid::uniform(2, 1)
        };
        let edges = spec.y_edges(16);
        assert_eq!(edges.len(), 2);
        for (lo, hi) in edges {
            assert!(lo <= hi, "no inverted cell on a saturated offset: ({lo}, {hi})");
        }
    }

    #[test]
    fn x_edges_seed_uniform_then_apply_the_override() {
        let uniform = SliceGrid::uniform(1, 2);
        assert_eq!(uniform.x_edges(64), vec![(0, 32), (32, 64)], "uniform cell pairs");
        let overridden = SliceGrid {
            overrides: SliceOverrides {
                x: vec![(1, 20)],
                y: Vec::new(),
            },
            ..SliceGrid::uniform(1, 2)
        };
        assert_eq!(overridden.x_edges(64), vec![(0, 20), (20, 64)], "the override moves the shared divider");
    }

    #[test]
    fn y_edges_mirror_x_edges_over_rows() {
        let spec = SliceGrid {
            overrides: SliceOverrides {
                x: Vec::new(),
                y: vec![(1, 10)],
            },
            ..SliceGrid::uniform(2, 1)
        };
        assert_eq!(spec.y_edges(32), vec![(0, 10), (10, 32)]);
    }

    /// The Phase B superset invariant: a spec with no per-divider overrides
    /// resolves byte-identically to the uniform Phase A `slice_grid_spec`.
    #[rstest]
    #[case(64, 64, 2, 2)]
    #[case(64, 64, 1, 4)]
    #[case(48, 60, 3, 5)]
    #[case(64, 64, 4, 4)]
    fn resolve_with_no_overrides_matches_slice_grid_spec(#[case] width: u32, #[case] height: u32, #[case] rows: u32, #[case] cols: u32) {
        let sheet = gradient_sheet(width, height);
        let spec = SliceGrid {
            offset_x: 2,
            offset_y: 1,
            gutter_x: 2,
            gutter_y: 1,
            ..SliceGrid::uniform(rows, cols)
        };
        let uniform = slice_grid_spec(&sheet, &spec).expect("slice_grid_spec");
        let resolved = slice_grid_resolve(&sheet, &spec).expect("slice_grid_resolve");
        assert_eq!(uniform.len(), resolved.len());
        for (a, b) in uniform.iter().zip(resolved.iter()) {
            assert_eq!(a.width(), b.width());
            assert_eq!(a.height(), b.height());
            assert_eq!(a.as_bytes(), b.as_bytes());
        }
    }

    /// `slice_grid_spec` routes a spec with overrides through the resolver, so
    /// the public entry point produces the non-uniform cut on its own.
    #[test]
    fn slice_grid_spec_routes_overrides_to_the_resolver() {
        // 64-wide, 2 cols: uniform divider at x = 32. Override it to x = 20, so
        // column 0 is 20 wide and column 1 is 44 wide.
        let sheet = gradient_sheet(64, 32);
        let spec = SliceGrid {
            overrides: SliceOverrides {
                x: vec![(1, 20)],
                y: Vec::new(),
            },
            ..SliceGrid::uniform(1, 2)
        };
        let cells = slice_grid_spec(&sheet, &spec).expect("slice");
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].width(), 20, "the overridden divider sets column 0's width");
        assert_eq!(cells[1].width(), 44, "column 1 takes the rest");
        assert_eq!(corner(&cells[0]), (0, 0));
        assert_eq!(corner(&cells[1]), (20, 0));
    }

    #[test]
    fn override_varies_row_heights() {
        // 32-tall, 2 rows: uniform divider at y = 16. Override to y = 10, so
        // row 0 is 10 tall and row 1 is 22 tall.
        let sheet = gradient_sheet(32, 32);
        let spec = SliceGrid {
            overrides: SliceOverrides {
                x: Vec::new(),
                y: vec![(1, 10)],
            },
            ..SliceGrid::uniform(2, 1)
        };
        let cells = slice_grid_resolve(&sheet, &spec).expect("slice");
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].height(), 10);
        assert_eq!(cells[1].height(), 22);
        assert_eq!(corner(&cells[0]), (0, 0));
        assert_eq!(corner(&cells[1]), (0, 10));
    }

    #[test]
    fn override_with_a_gutter_keeps_the_gap() {
        // 64-wide, 2 cols, 4 px gutter: uniform cell is (64-4)/2 = 30, divider
        // (col 0 right) at 30, col 1 left at 34. Override the divider to 20: col
        // 0 is 20 wide, col 1 left moves to 24, so col 1 is 64-24 = 40 wide.
        let sheet = gradient_sheet(64, 16);
        let spec = SliceGrid {
            gutter_x: 4,
            overrides: SliceOverrides {
                x: vec![(1, 20)],
                y: Vec::new(),
            },
            ..SliceGrid::uniform(1, 2)
        };
        let cells = slice_grid_resolve(&sheet, &spec).expect("slice");
        assert_eq!(cells[0].width(), 20);
        assert_eq!(corner(&cells[0]), (0, 0));
        assert_eq!(corner(&cells[1]), (24, 0), "the gutter still separates the cells after the override");
    }

    #[test]
    fn override_index_out_of_range_is_ignored() {
        // Divider index 5 names no interior divider of a 2-column grid, so the
        // cut stays uniform.
        let sheet = gradient_sheet(64, 16);
        let spec = SliceGrid {
            overrides: SliceOverrides {
                x: vec![(5, 10)],
                y: Vec::new(),
            },
            ..SliceGrid::uniform(1, 2)
        };
        let cells = slice_grid_resolve(&sheet, &spec).expect("slice");
        assert_eq!(cells[0].width(), 32, "an out-of-range override leaves the uniform cut");
        assert_eq!(cells[1].width(), 32);
    }

    #[test]
    fn override_that_crosses_its_neighbour_collapses() {
        // Pushing the divider past the right edge collapses column 1 to zero
        // width, which the resolver rejects rather than wrapping.
        let sheet = gradient_sheet(64, 16);
        let spec = SliceGrid {
            overrides: SliceOverrides {
                x: vec![(1, 64)],
                y: Vec::new(),
            },
            ..SliceGrid::uniform(1, 2)
        };
        assert!(matches!(slice_grid_resolve(&sheet, &spec), Err(Error::EmptyBuffer)));
    }

    #[test]
    fn resolve_rejects_zero_grid() {
        let sheet = gradient_sheet(16, 16);
        let spec = SliceGrid {
            overrides: SliceOverrides {
                x: vec![(1, 4)],
                y: Vec::new(),
            },
            ..SliceGrid::uniform(0, 4)
        };
        assert!(matches!(slice_grid_resolve(&sheet, &spec), Err(Error::EmptyBuffer)));
    }

    #[test]
    fn slice_grid_with_overrides_round_trips_through_rmp() {
        let grid = SliceGrid {
            offset_x: 2,
            gutter_x: 1,
            overrides: SliceOverrides {
                x: vec![(1, 12), (2, 30)],
                y: vec![(1, 8)],
            },
            ..SliceGrid::uniform(2, 3)
        };
        let bytes = rmp_serde::to_vec_named(&grid).expect("encode");
        let back: SliceGrid = rmp_serde::from_slice(&bytes).expect("decode");
        assert_eq!(grid, back);
    }

    #[test]
    fn slice_grid_without_overrides_omits_the_field_and_old_data_loads() {
        // A uniform grid omits the overrides field on the wire, and a record
        // written before the field existed still decodes (overrides default empty).
        let grid = SliceGrid::uniform(2, 2);
        let json = serde_json::to_string(&grid).expect("encode");
        assert!(!json.contains("overrides"), "an empty override set is omitted: {json}");

        let legacy = r#"{"rows":2,"cols":2,"offset_x":0,"offset_y":0,"gutter_x":0,"gutter_y":0,"inset":0}"#;
        let back: SliceGrid = serde_json::from_str(legacy).expect("decode legacy slice grid");
        assert!(back.overrides.is_empty(), "missing overrides default to empty");
        assert_eq!(back, grid);
    }
}
