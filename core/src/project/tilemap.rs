//! Tilemap data: a grid of tile references with per-cell flags.
//!
//! Stored as a flat row-major buffer rather than `Vec<Vec<TileCell>>`
//! to keep the layout SIMD-friendly and the allocation contiguous.
//! `cells.len()` must equal `width * height`; deserialisation does not
//! enforce this on its own, the editor validates after load.

use serde::{Deserialize, Serialize};

use super::id::TileIndex;

/// Per-cell rendering flags for a tile placement.
///
/// Stored as a `u8` bitfield so a tilemap of `W*H` cells is small
/// regardless of grid size. Flag combinations let a single tile in
/// the source set cover four orientations and the diagonal flip.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TileFlags(pub u8);

impl TileFlags {
    /// Flip horizontally before rendering.
    pub const FLIP_X: Self = Self(1 << 0);

    /// Flip vertically before rendering.
    pub const FLIP_Y: Self = Self(1 << 1);

    /// Swap X and Y axes (diagonal flip). Combined with `FLIP_X` or
    /// `FLIP_Y` this expresses 90/180/270-degree rotations.
    pub const FLIP_DIAGONAL: Self = Self(1 << 2);

    /// The empty flag set.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Returns the union of two flag sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns `true` if every flag in `other` is also set in `self`.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// One cell of a tilemap.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct TileCell {
    /// Index of the tile in the layer's tileset. `TileIndex(0)` is
    /// the empty tile by convention.
    pub index: TileIndex,
    /// Rendering flags for this cell.
    pub flags: TileFlags,
}

impl TileCell {
    /// Constructs an empty cell. Equivalent to `TileCell::default()`.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            index: TileIndex::new(0),
            flags: TileFlags::empty(),
        }
    }

    /// Returns `true` if this cell is the empty tile.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.index.get() == 0
    }
}

/// A grid of [`TileCell`]s in row-major order.
///
/// Cells are addressed by `cells[y * width + x]`. Helpers on this type
/// stay non-panicking and bounds-checked.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TilemapData {
    /// Number of columns.
    pub width: u32,
    /// Number of rows.
    pub height: u32,
    /// Cells in row-major order. Length must equal `width * height`.
    pub cells: Vec<TileCell>,
}

impl TilemapData {
    /// Constructs an all-empty tilemap of the given dimensions.
    #[must_use]
    pub fn empty(width: u32, height: u32) -> Self {
        let count = (width as usize) * (height as usize);
        Self {
            width,
            height,
            cells: vec![TileCell::empty(); count],
        }
    }

    /// Returns the cell at `(x, y)` if both coordinates are in range.
    #[must_use]
    pub fn cell(&self, x: u32, y: u32) -> Option<TileCell> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        self.cells.get(idx).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tilemap_has_correct_size() {
        let m = TilemapData::empty(4, 3);
        assert_eq!(m.cells.len(), 12);
        assert!(m.cells.iter().all(|c| c.is_empty()));
    }

    #[test]
    fn cell_lookup_in_range() {
        let mut m = TilemapData::empty(2, 2);
        m.cells[3] = TileCell {
            index: TileIndex::new(7),
            flags: TileFlags::FLIP_X,
        };
        assert_eq!(m.cell(1, 1).map(|c| c.index), Some(TileIndex::new(7)));
        assert!(m.cell(2, 0).is_none());
    }

    #[test]
    fn flags_contain_check() {
        let f = TileFlags(TileFlags::FLIP_X.0 | TileFlags::FLIP_DIAGONAL.0);
        assert!(f.contains(TileFlags::FLIP_X));
        assert!(f.contains(TileFlags::FLIP_DIAGONAL));
        assert!(!f.contains(TileFlags::FLIP_Y));
    }
}
