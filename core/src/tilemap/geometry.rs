//! Cell-to-pixel geometry for the four supported tile-grid shapes.
//!
//! [`cell_to_pixel`] maps a `(col, row)` grid coordinate to the top-left
//! pixel of that cell for square, isometric, and hex layouts. The rest of the
//! tilemap stack (placement, autotile, bucket fill) stays shape-agnostic and
//! only calls in here when it needs screen coordinates.
//!
//! Shape model adopted from Pixelorama's `TileSetCustom`
//! (`src/Classes/Cels/CelTileMap.gd`). See `THIRD_PARTY_NOTICES.md`.

// Tile dimensions are bounded well under 2^31, so the u32->i32 casts used to
// place cells in signed pixel space never wrap in practice.
#![allow(clippy::cast_possible_wrap)]

use crate::project::geometry::{IVec2, Size};
use crate::project::tileset::{HexOffsetAxis, TileShape};

/// Returns the top-left pixel position of cell `(col, row)` for a grid of
/// `shape` with the given per-tile `tile_size`.
///
/// `hex_offset` selects which rows/columns take the half-step stagger and is
/// ignored for [`TileShape::Square`] and [`TileShape::Isometric`]; when `None`
/// is supplied for a hex shape, the shape's default axis is used (odd rows for
/// pointy-top, odd columns for flat-top).
///
/// Coordinates are signed so isometric layouts (which place cells to the left
/// of the origin) round-trip without underflow.
#[must_use]
pub fn cell_to_pixel(
    shape: TileShape,
    hex_offset: Option<HexOffsetAxis>,
    tile_size: Size,
    col: i32,
    row: i32,
) -> IVec2 {
    let tw = tile_size.width as i32;
    let th = tile_size.height as i32;
    match shape {
        TileShape::Square => IVec2::new(col * tw, row * th),
        TileShape::Isometric => IVec2::new((col - row) * (tw / 2), (col + row) * (th / 2)),
        TileShape::HexPointy => {
            // Rows overlap vertically by a quarter tile; alternate rows shift
            // right by half a tile.
            let even_offset = matches!(hex_offset, Some(HexOffsetAxis::EvenRow));
            let staggered = if even_offset {
                row.rem_euclid(2) == 0
            } else {
                row.rem_euclid(2) != 0
            };
            let x = col * tw + if staggered { tw / 2 } else { 0 };
            let y = row * (th * 3 / 4);
            IVec2::new(x, y)
        }
        TileShape::HexFlat => {
            // Columns overlap horizontally by a quarter tile; alternate columns
            // shift down by half a tile.
            let even_offset = matches!(hex_offset, Some(HexOffsetAxis::EvenCol));
            let staggered = if even_offset {
                col.rem_euclid(2) == 0
            } else {
                col.rem_euclid(2) != 0
            };
            let x = col * (tw * 3 / 4);
            let y = row * th + if staggered { th / 2 } else { 0 };
            IVec2::new(x, y)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T16: Size = Size {
        width: 16,
        height: 16,
    };

    #[test]
    fn square_is_simple_grid() {
        assert_eq!(
            cell_to_pixel(TileShape::Square, None, T16, 0, 0),
            IVec2::new(0, 0)
        );
        assert_eq!(
            cell_to_pixel(TileShape::Square, None, T16, 2, 3),
            IVec2::new(32, 48)
        );
    }

    #[test]
    fn isometric_diamonds_interlock() {
        // Origin at (0,0). Moving +col goes down-right, +row goes down-left.
        assert_eq!(
            cell_to_pixel(TileShape::Isometric, None, T16, 0, 0),
            IVec2::new(0, 0)
        );
        assert_eq!(
            cell_to_pixel(TileShape::Isometric, None, T16, 1, 0),
            IVec2::new(8, 8)
        );
        assert_eq!(
            cell_to_pixel(TileShape::Isometric, None, T16, 0, 1),
            IVec2::new(-8, 8)
        );
    }

    #[test]
    fn hex_pointy_offsets_odd_rows() {
        // Row 0 not staggered, row 1 staggered right by half a tile.
        assert_eq!(
            cell_to_pixel(TileShape::HexPointy, None, T16, 0, 0),
            IVec2::new(0, 0)
        );
        assert_eq!(
            cell_to_pixel(TileShape::HexPointy, None, T16, 0, 1),
            IVec2::new(8, 12)
        );
        // EvenRow flips which rows stagger.
        assert_eq!(
            cell_to_pixel(
                TileShape::HexPointy,
                Some(HexOffsetAxis::EvenRow),
                T16,
                0,
                0
            ),
            IVec2::new(8, 0)
        );
    }

    #[test]
    fn hex_flat_offsets_odd_cols() {
        assert_eq!(
            cell_to_pixel(TileShape::HexFlat, None, T16, 0, 0),
            IVec2::new(0, 0)
        );
        assert_eq!(
            cell_to_pixel(TileShape::HexFlat, None, T16, 1, 0),
            IVec2::new(12, 8)
        );
    }
}
