//! Pure grid math — frame coordinates and cell sizes — kept testable in
//! isolation from the verb's async pipeline.
//!
//! The cell-size invariants are lifted verbatim from `FalSprite`
//! `public/app.js:469-527` (MIT, lovisdotio): cells are read row-major
//! and dimensions floor-truncate rather than round. Pixhaus rendering is
//! deterministic; non-divisible widths drop the remainder rather than
//! producing fractional cell sizes that disagree across frames.

use crate::plugin::context::PixelData;
use crate::plugin::error::{Result, VerbError};

/// Position of one cell in the grid, in (col, row) order.
///
/// Cells are addressed in reading order: `id = row * grid_size + col`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct GridCoord {
    /// Column index, `0..grid_size`.
    pub col: u8,
    /// Row index, `0..grid_size`.
    pub row: u8,
}

/// Returns the row/column for `frame_id` in a square grid of side
/// `grid_size`. Row-major order matches the upstream's playback loop.
///
/// `grid_size` is bounded at 6 by the verb's validation; `frame_id` is
/// always `< grid_size²`, so both `frame_id % g` and `frame_id / g` fit
/// in a `u8`. The truncation casts are annotated for the lint rather
/// than rewritten as `try_into`, which would force the caller to thread
/// a `Result` for invariants we own.
#[must_use]
pub fn frame_coord(frame_id: u32, grid_size: u8) -> GridCoord {
    let g = u32::from(grid_size.max(1));
    #[allow(clippy::cast_possible_truncation)]
    let col = (frame_id % g) as u8;
    #[allow(clippy::cast_possible_truncation)]
    let row = (frame_id / g) as u8;
    GridCoord { col, row }
}

/// Cell size for a sheet of dimensions `sheet_w × sheet_h` packed in a
/// square grid of side `grid_size`. Floor-divides so non-divisible
/// dimensions drop the remainder.
#[must_use]
pub fn cell_size(sheet_w: u32, sheet_h: u32, grid_size: u8) -> (u32, u32) {
    let g = u32::from(grid_size.max(1));
    (sheet_w / g, sheet_h / g)
}

/// Slices a square-packed RGBA8 sheet into `grid_size²` cell buffers in
/// row-major order.
///
/// The cell dimensions are computed via [`cell_size`] (floor-divided);
/// any remainder pixels at the right or bottom of the sheet are
/// discarded so every output cel has the same dimensions. The sheet
/// must be RGBA8 (`bytes_per_pixel == 4`); other formats return
/// [`VerbError::Schema`].
///
/// Returns `grid_size²` tightly-packed `PixelData` buffers.
pub fn slice_grid(sheet: &PixelData, grid_size: u8) -> Result<Vec<PixelData>> {
    if sheet.bytes_per_pixel != 4 {
        return Err(VerbError::Schema(
            "animated_sprite_sheet: sheet pixel buffer must be RGBA8".into(),
        ));
    }
    if !sheet.is_well_formed() {
        return Err(VerbError::Schema(
            "animated_sprite_sheet: sheet pixel buffer dimensions and byte count are inconsistent"
                .into(),
        ));
    }
    if grid_size < 1 {
        return Err(VerbError::Schema(
            "animated_sprite_sheet: grid_size must be at least 1".into(),
        ));
    }

    let (cell_w, cell_h) = cell_size(sheet.width, sheet.height, grid_size);
    if cell_w == 0 || cell_h == 0 {
        return Err(VerbError::Schema(format!(
            "animated_sprite_sheet: sheet {}×{} too small for {grid_size}×{grid_size} grid",
            sheet.width, sheet.height,
        )));
    }

    let stride = sheet.stride as usize;
    let g = u32::from(grid_size);
    let total = (g * g) as usize;
    let mut out = Vec::with_capacity(total);

    let cell_cols = cell_w as usize;
    let cell_rows = cell_h as usize;
    let row_bytes = cell_cols * 4;

    for id in 0..total {
        #[allow(clippy::cast_possible_truncation)]
        let coord = frame_coord(id as u32, grid_size);
        let x0 = u32::from(coord.col) * cell_w;
        let y0 = u32::from(coord.row) * cell_h;

        let mut bytes = vec![0u8; cell_cols * cell_rows * 4];
        for row in 0..cell_rows {
            let src_off = (y0 as usize + row) * stride + (x0 as usize) * 4;
            let dst_off = row * row_bytes;
            let src = sheet
                .bytes
                .get(src_off..src_off + row_bytes)
                .ok_or_else(|| {
                    VerbError::Schema(
                        "animated_sprite_sheet: cell row slice out of range — \
                     sheet dimensions disagree with declared stride"
                            .into(),
                    )
                })?;
            bytes[dst_off..dst_off + row_bytes].copy_from_slice(src);
        }
        out.push(PixelData::rgba8(cell_w, cell_h, bytes));
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_coord_row_major_4x4() {
        assert_eq!(frame_coord(0, 4), GridCoord { col: 0, row: 0 });
        assert_eq!(frame_coord(1, 4), GridCoord { col: 1, row: 0 });
        assert_eq!(frame_coord(3, 4), GridCoord { col: 3, row: 0 });
        assert_eq!(frame_coord(4, 4), GridCoord { col: 0, row: 1 });
        assert_eq!(frame_coord(15, 4), GridCoord { col: 3, row: 3 });
    }

    #[test]
    fn frame_coord_handles_zero_grid() {
        // grid_size 0 is invalid input; the implementation clamps to 1 so it
        // doesn't divide by zero. Verb-level validation rejects out-of-range
        // grid sizes before this is reached.
        let c = frame_coord(0, 0);
        assert_eq!(c, GridCoord { col: 0, row: 0 });
    }

    #[test]
    fn cell_size_floors_non_divisible_widths() {
        // 17 / 4 = 4 remainder 1 — remainder is dropped.
        assert_eq!(cell_size(17, 17, 4), (4, 4));
        // 100 / 6 = 16 remainder 4
        assert_eq!(cell_size(100, 100, 6), (16, 16));
        // Divisible — exact.
        assert_eq!(cell_size(128, 128, 4), (32, 32));
    }

    #[test]
    fn cell_size_clamps_zero_grid() {
        assert_eq!(cell_size(64, 64, 0), (64, 64));
    }

    fn make_sheet(width: u32, height: u32, fill: u8) -> PixelData {
        PixelData::rgba8(width, height, vec![fill; (width * height * 4) as usize])
    }

    #[test]
    fn slice_grid_returns_grid_squared_cells() {
        let sheet = make_sheet(64, 64, 200);
        let cells = slice_grid(&sheet, 4).unwrap();
        assert_eq!(cells.len(), 16);
        for cell in &cells {
            assert_eq!(cell.width, 16);
            assert_eq!(cell.height, 16);
            assert_eq!(cell.bytes_per_pixel, 4);
            assert_eq!(cell.stride, 16 * 4);
            assert_eq!(cell.bytes.len(), 16 * 16 * 4);
        }
    }

    #[test]
    fn slice_grid_preserves_pixel_content() {
        // Build a 4×4 sheet where each cell is a single color so we can
        // verify row-major ordering after the slice.
        // Layout (each cell is 2×2):
        //   [R G B Y]
        //   [W K M C]
        // Grid size = 4 columns, 2 rows — but slice_grid handles square
        // grids only, so build a 4×4 sheet (8×8 pixels) instead.
        let mut bytes = Vec::with_capacity(8 * 8 * 4);
        for y in 0..8u32 {
            for x in 0..8u32 {
                let col = x / 2;
                let row = y / 2;
                let id = row * 4 + col;
                // Encode the cell id into the red channel so we can
                // verify ordering.
                let r = u8::try_from(id).unwrap();
                bytes.extend_from_slice(&[r, 0, 0, 255]);
            }
        }
        let sheet = PixelData::rgba8(8, 8, bytes);
        let cells = slice_grid(&sheet, 4).unwrap();
        assert_eq!(cells.len(), 16);
        for (i, cell) in cells.iter().enumerate() {
            let id = u8::try_from(i).unwrap();
            for chunk in cell.bytes.chunks_exact(4) {
                assert_eq!(chunk[0], id, "cell {i} should contain only id {id}");
            }
        }
    }

    #[test]
    fn slice_grid_floors_non_divisible_sheet() {
        // 17×17 / 4×4 = 4×4 cells, 1 px remainder dropped.
        let sheet = make_sheet(17, 17, 0);
        let cells = slice_grid(&sheet, 4).unwrap();
        assert_eq!(cells.len(), 16);
        for cell in cells {
            assert_eq!(cell.width, 4);
            assert_eq!(cell.height, 4);
        }
    }

    #[test]
    fn slice_grid_rejects_non_rgba8() {
        let bad = PixelData {
            width: 8,
            height: 8,
            bytes_per_pixel: 3,
            stride: 24,
            bytes: vec![0u8; 8 * 8 * 3],
        };
        assert!(matches!(slice_grid(&bad, 4), Err(VerbError::Schema(_))));
    }

    #[test]
    fn slice_grid_rejects_malformed_buffer() {
        let bad = PixelData {
            width: 8,
            height: 8,
            bytes_per_pixel: 4,
            stride: 32,
            bytes: vec![0u8; 8], // should be 256 bytes
        };
        assert!(matches!(slice_grid(&bad, 4), Err(VerbError::Schema(_))));
    }

    #[test]
    fn slice_grid_rejects_zero_grid() {
        let sheet = make_sheet(8, 8, 0);
        assert!(matches!(slice_grid(&sheet, 0), Err(VerbError::Schema(_))));
    }

    #[test]
    fn slice_grid_rejects_sheet_smaller_than_grid() {
        // 2×2 sheet with grid_size=4 — cell would be 0×0.
        let sheet = make_sheet(2, 2, 0);
        assert!(matches!(slice_grid(&sheet, 4), Err(VerbError::Schema(_))));
    }

    #[test]
    fn slice_is_deterministic() {
        let sheet = make_sheet(32, 32, 128);
        let a = slice_grid(&sheet, 4).unwrap();
        let b = slice_grid(&sheet, 4).unwrap();
        assert_eq!(a.len(), b.len());
        for (a_cell, b_cell) in a.iter().zip(b.iter()) {
            assert_eq!(a_cell.bytes, b_cell.bytes);
        }
    }
}
