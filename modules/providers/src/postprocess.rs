//! Provider output post-processing: chroma-key and sheet slicing.
//!
//! The proven sprite pipeline asks a model for a character on a flat pure-magenta
//! key and, for animation, an N-cell sheet. Two pure functions turn that raw output
//! into clean frames: [`chroma_key_magenta`] makes the magenta background
//! transparent, and [`slice_sheet`] cuts the sheet into equal cells in row-major
//! order. Both are CPU-only and provider-agnostic, so any provider that follows the
//! magenta-key convention reuses them.

use thiserror::Error;

use pixhaus_services::generated::GeneratedFrame;
use pixhaus_services::job::Grid;

/// Why post-processing failed.
#[derive(Debug, Error)]
pub enum PostProcessError {
    /// The byte length does not match `width * height * 4`.
    #[error("image bytes {len} do not match {width}x{height} RGBA ({expected})")]
    SizeMismatch {
        /// The byte length given.
        len: usize,
        /// The expected length `width * height * 4`.
        expected: usize,
        /// The width used.
        width: u32,
        /// The height used.
        height: u32,
    },
    /// The sheet dimensions are not divisible by the grid (so cells are not equal).
    #[error("sheet {width}x{height} is not divisible by grid {cols}x{rows}")]
    NotDivisible {
        /// Sheet width.
        width: u32,
        /// Sheet height.
        height: u32,
        /// Grid columns.
        cols: u32,
        /// Grid rows.
        rows: u32,
    },
}

/// Makes the flat pure-magenta key transparent, in place.
///
/// Clears every pixel whose colour reads as magenta (`R > 200 && G < 80 && B > 200`)
/// to fully transparent, matching the downstream key the prompt asks the model for.
/// The threshold is loose enough to catch slight anti-alias bleed at the silhouette
/// edge without touching the character's own colours (which the prompt keeps off
/// pure magenta).
pub fn chroma_key_magenta(rgba: &mut [u8]) {
    for pixel in rgba.chunks_exact_mut(4) {
        if pixel[0] > 200 && pixel[1] < 80 && pixel[2] > 200 {
            pixel[0] = 0;
            pixel[1] = 0;
            pixel[2] = 0;
            pixel[3] = 0;
        }
    }
}

/// Slices a sheet into `grid.cols * grid.rows` equal cells in row-major order.
///
/// Each returned frame is tightly packed (`stride == cell_width * 4`).
///
/// # Errors
/// Returns [`PostProcessError::SizeMismatch`] if `rgba` is not `width * height * 4`
/// bytes, or [`PostProcessError::NotDivisible`] if the sheet does not divide evenly
/// into the grid.
pub fn slice_sheet(rgba: &[u8], width: u32, height: u32, grid: Grid) -> Result<Vec<GeneratedFrame>, PostProcessError> {
    let expected = width as usize * height as usize * 4;
    if rgba.len() != expected {
        return Err(PostProcessError::SizeMismatch {
            len: rgba.len(),
            expected,
            width,
            height,
        });
    }
    if grid.cols == 0 || grid.rows == 0 || width % grid.cols != 0 || height % grid.rows != 0 {
        return Err(PostProcessError::NotDivisible {
            width,
            height,
            cols: grid.cols,
            rows: grid.rows,
        });
    }

    let cell_w = width / grid.cols;
    let cell_h = height / grid.rows;
    let src_stride = width as usize * 4;
    let cell_stride = cell_w as usize * 4;

    let mut frames = Vec::with_capacity(grid.cell_count() as usize);
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            let mut bytes = Vec::with_capacity(cell_stride * cell_h as usize);
            for y in 0..cell_h {
                // Divisibility and length are validated above, so this slice is in
                // bounds for every cell.
                let src_row = (row * cell_h + y) as usize * src_stride;
                let start = src_row + (col * cell_w) as usize * 4;
                bytes.extend_from_slice(&rgba[start..start + cell_stride]);
            }
            frames.push(GeneratedFrame {
                width: cell_w,
                height: cell_h,
                stride: cell_w * 4,
                rgba: bytes,
            });
        }
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_pure_magenta_and_spares_character_colour() {
        // Two pixels: pure magenta, then a near-magenta-but-valid character colour.
        let mut rgba = vec![255, 0, 255, 255, 200, 80, 200, 255];
        chroma_key_magenta(&mut rgba);
        assert_eq!(&rgba[0..4], &[0, 0, 0, 0], "pure magenta is cleared to transparent");
        assert_eq!(&rgba[4..8], &[200, 80, 200, 255], "R=200,G=80 are outside the threshold and kept");
    }

    #[test]
    fn slices_a_two_cell_strip_in_order() {
        // A 2x1 sheet: left cell red, right cell green (each one pixel).
        let rgba = vec![255, 0, 0, 255, 0, 255, 0, 255];
        let frames = slice_sheet(&rgba, 2, 1, Grid { cols: 2, rows: 1 }).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].rgba, vec![255, 0, 0, 255], "first cell is the left pixel");
        assert_eq!(frames[1].rgba, vec![0, 255, 0, 255], "second cell is the right pixel");
        assert_eq!((frames[0].width, frames[0].height, frames[0].stride), (1, 1, 4));
    }

    #[test]
    fn slices_a_2x2_sheet_row_major() {
        // 2x2 sheet, each cell a distinct colour, packed row-major.
        // (0,0)=A (1,0)=B / (0,1)=C (1,1)=D
        let rgba = vec![
            10, 0, 0, 255, 20, 0, 0, 255, // top row: A B
            30, 0, 0, 255, 40, 0, 0, 255, // bottom row: C D
        ];
        let frames = slice_sheet(&rgba, 2, 2, Grid { cols: 2, rows: 2 }).unwrap();
        let firsts: Vec<u8> = frames.iter().map(|f| f.rgba[0]).collect();
        assert_eq!(firsts, vec![10, 20, 30, 40], "row-major: A, B, then C, D");
    }

    #[test]
    fn rejects_a_non_divisible_grid() {
        let rgba = vec![0u8; 3 * 4]; // a 3x1 RGBA sheet
        let err = slice_sheet(&rgba, 3, 1, Grid { cols: 2, rows: 1 }).unwrap_err();
        assert!(matches!(err, PostProcessError::NotDivisible { .. }));
    }

    #[test]
    fn rejects_a_length_mismatch() {
        let err = slice_sheet(&[0u8; 7], 2, 1, Grid { cols: 2, rows: 1 }).unwrap_err();
        assert!(matches!(err, PostProcessError::SizeMismatch { .. }));
    }
}
