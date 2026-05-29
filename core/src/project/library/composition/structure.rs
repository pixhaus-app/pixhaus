//! The layout contract. A Structure defines the canvas and panels; the
//! `ai::compose` resolver derives both layout prose and slice rectangles
//! from it, so prose and geometry cannot desync.

use serde::{Deserialize, Serialize};

use super::Dimensions;

/// Stable id for a Structure. Built-ins use reverse-DNS
/// (`pixhaus.builtin.structure.character`); a project record reuses that id
/// to shadow the built-in, or takes a fresh project slug.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StructureId(pub String);

/// Canvas layout contract for AI generation.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Structure {
    pub id: StructureId,
    pub name: String,
    pub output: StructureOutput,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub layout_negatives: String,
}

/// Output shape of a Structure.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructureOutput {
    /// One free-composition image; no panels.
    Single,
    /// Structured multi-panel sheet.
    Paneled { canvas: Dimensions, panels: Vec<StructurePanel> },
}

/// One named panel within a paneled structure.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructurePanel {
    pub label: String,
    pub rect: PanelRect,
    /// Prose with `{canvas_w}`, `{canvas_h}`, `{panel_w}`, `{panel_h}`,
    /// `{label}` tokens interpolated by the resolver.
    pub prose_fragment: String,
    pub slot: PanelSlot,
}

/// Pixel bounding box for a panel within the canvas.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Semantic role of a panel in a structure.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanelSlot {
    View,
    Expression,
    Callout,
    Outfit,
    PaletteSwatch,
    Generic,
}

/// Smallest compact `(rows, cols)` grid holding `n` cells, picking the squarest
/// factor pair whose product is at least `n`, with columns greater than or equal
/// to rows by at most one.
///
/// This is the asset-plan rule the forge calls `multirow-grid-over-singlerow`:
/// a body strip laid out 1×N gives the model no vertical anchor and drifts in
/// scale, so a frame count maps to a compact 2D grid instead. `n` is clamped to
/// at least 1, so `0` maps to `(1, 1)`.
///
/// ```
/// use pixhaus_core::project::library::composition::compact_grid_shape;
/// assert_eq!(compact_grid_shape(4), (2, 2));
/// assert_eq!(compact_grid_shape(6), (2, 3));
/// assert_eq!(compact_grid_shape(9), (3, 3));
/// assert_eq!(compact_grid_shape(16), (4, 4));
/// assert_eq!(compact_grid_shape(5), (2, 3));
/// assert_eq!(compact_grid_shape(7), (3, 3));
/// ```
#[must_use]
pub fn compact_grid_shape(n: u32) -> (u32, u32) {
    let n = n.max(1);
    // The squarest grid is rooted at ceil(sqrt(n)) columns: pick the smallest
    // column count whose square is at least n, then the smallest row count that,
    // multiplied by those columns, still holds every cell. This keeps the pair
    // square (cols - rows <= 1) and the product minimal (rows * cols >= n with
    // no smaller pair fitting), which is exactly smallest-useful-output.
    let mut cols = 1u32;
    while cols * cols < n {
        cols += 1;
    }
    let rows = n.div_ceil(cols);
    (rows, cols)
}

/// Tiles `region` into `rows` × `cols` equal cells in row-major order.
///
/// Cell dimensions are floor-divided (`region.w / cols`, `region.h / rows`); any
/// trailing remainder on the right or bottom edge is dropped so every returned
/// cell is the same size — the shared-scale normalize pass downstream needs
/// uniform cells. Each cell is offset by the region origin, so the result tiles
/// the sub-rect, not the whole canvas. Cell `r * cols + c` is rooted at
/// `(region.x + c * cell_w, region.y + r * cell_h)`.
///
/// This is the inverse of [`crate::transforms::sheet::slice_grid`]: the same
/// floor-divided cell math authors the panel rectangles here that the slicer
/// later cuts. Returns an empty vector when `rows` or `cols` is `0`, or when the
/// floor-divided cell collapses to zero on either axis (the grid is finer than
/// the region), so every returned rect is non-empty and lies fully inside
/// `region`.
///
/// ```
/// use pixhaus_core::project::library::composition::{grid_rects, PanelRect};
/// let region = PanelRect { x: 0, y: 0, w: 100, h: 100 };
/// let cells = grid_rects(region, 2, 2);
/// assert_eq!(cells.len(), 4);
/// assert_eq!(cells[0], PanelRect { x: 0, y: 0, w: 50, h: 50 });
/// assert_eq!(cells[3], PanelRect { x: 50, y: 50, w: 50, h: 50 });
/// ```
#[must_use]
pub fn grid_rects(region: PanelRect, rows: u32, cols: u32) -> Vec<PanelRect> {
    if rows == 0 || cols == 0 {
        return Vec::new();
    }
    let cell_w = region.w / cols;
    let cell_h = region.h / rows;
    if cell_w == 0 || cell_h == 0 {
        return Vec::new();
    }
    let mut cells = Vec::with_capacity((rows as usize) * (cols as usize));
    for r in 0..rows {
        for c in 0..cols {
            cells.push(PanelRect {
                x: region.x + c * cell_w,
                y: region.y + r * cell_h,
                w: cell_w,
                h: cell_h,
            });
        }
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    use proptest::prelude::*;
    use rstest::rstest;

    mod compact_grid_shape {
        use super::*;

        #[rstest]
        #[case::one(1, (1, 1))]
        #[case::four(4, (2, 2))]
        #[case::six(6, (2, 3))]
        #[case::nine(9, (3, 3))]
        #[case::sixteen(16, (4, 4))]
        #[case::five(5, (2, 3))]
        #[case::seven(7, (3, 3))]
        fn maps_frame_count_to_the_squarest_grid(#[case] n: u32, #[case] expected: (u32, u32)) {
            assert_eq!(super::super::compact_grid_shape(n), expected);
        }

        #[test]
        fn zero_clamps_to_a_single_cell() {
            // n is clamped to >= 1 so a degenerate request never yields a 0x0 grid.
            assert_eq!(super::super::compact_grid_shape(0), (1, 1));
        }

        proptest! {
            /// The grid always holds every cell and stays square: rows * cols >= n
            /// and the column count never leads the row count by more than one.
            #[test]
            fn holds_n_and_stays_square(n in 1u32..=256) {
                let (rows, cols) = super::super::compact_grid_shape(n);
                prop_assert!(rows * cols >= n, "grid {rows}x{cols} must hold {n}");
                prop_assert!(cols >= rows, "columns must not trail rows: {rows}x{cols}");
                prop_assert!(cols - rows <= 1, "grid must be square: {rows}x{cols}");
            }
        }
    }

    mod grid_rects {
        use super::*;

        const REGION: PanelRect = PanelRect { x: 0, y: 0, w: 100, h: 100 };

        #[test]
        fn tiles_a_region_into_row_major_cells() {
            let cells = super::super::grid_rects(REGION, 2, 3);
            assert_eq!(cells.len(), 6);
            // Row-major: the first row is y=0, the second y=33 (floor 100/2 = 50).
            assert_eq!(cells[0], PanelRect { x: 0, y: 0, w: 33, h: 50 });
            assert_eq!(cells[1], PanelRect { x: 33, y: 0, w: 33, h: 50 });
            assert_eq!(cells[3], PanelRect { x: 0, y: 50, w: 33, h: 50 });
        }

        #[test]
        fn offsets_cells_by_the_region_origin() {
            let region = PanelRect { x: 10, y: 20, w: 40, h: 40 };
            let cells = super::super::grid_rects(region, 2, 2);
            assert_eq!(cells[0], PanelRect { x: 10, y: 20, w: 20, h: 20 });
            assert_eq!(cells[3], PanelRect { x: 30, y: 40, w: 20, h: 20 });
        }

        #[test]
        fn cells_share_one_width_and_height() {
            let cells = super::super::grid_rects(REGION, 3, 3);
            let (w, h) = (cells[0].w, cells[0].h);
            for cell in &cells {
                assert_eq!((cell.w, cell.h), (w, h), "cells must be uniform: {cell:?}");
            }
        }

        #[test]
        fn drops_the_trailing_remainder() {
            // 7 wide over 2 cols floor-divides to 3-wide cells; the 7th column is
            // dropped, not folded into a short final cell.
            let region = PanelRect { x: 0, y: 0, w: 7, h: 7 };
            let cells = super::super::grid_rects(region, 2, 2);
            for cell in &cells {
                assert_eq!((cell.w, cell.h), (3, 3), "uniform after dropping the remainder");
            }
        }

        #[test]
        fn empty_for_zero_rows_or_cols() {
            assert!(super::super::grid_rects(REGION, 0, 2).is_empty());
            assert!(super::super::grid_rects(REGION, 2, 0).is_empty());
        }

        #[test]
        fn empty_when_the_grid_is_finer_than_the_region() {
            let region = PanelRect { x: 0, y: 0, w: 4, h: 1 };
            assert!(super::super::grid_rects(region, 1, 5).is_empty(), "a zero-wide cell yields no rects");
        }

        proptest! {
            /// For any region and grid, every cell lies fully inside the region —
            /// the core anti-clip guarantee — cells never overlap, and the count
            /// is exactly rows * cols (or empty when a cell collapses to zero).
            #[test]
            fn cells_are_contained_and_non_overlapping(
                rx in 0u32..=64,
                ry in 0u32..=64,
                rw in 1u32..=128,
                rh in 1u32..=128,
                rows in 1u32..=8,
                cols in 1u32..=8,
            ) {
                let region = PanelRect { x: rx, y: ry, w: rw, h: rh };
                let cells = super::super::grid_rects(region, rows, cols);
                let cell_w = rw / cols;
                let cell_h = rh / rows;
                if cell_w == 0 || cell_h == 0 {
                    prop_assert!(cells.is_empty(), "a collapsed cell yields no rects");
                    return Ok(());
                }
                prop_assert_eq!(cells.len(), (rows as usize) * (cols as usize));
                for cell in &cells {
                    // Fully inside the region on every edge.
                    prop_assert!(cell.x >= region.x, "left edge in region: {cell:?}");
                    prop_assert!(cell.y >= region.y, "top edge in region: {cell:?}");
                    prop_assert!(cell.x + cell.w <= region.x + region.w, "right edge in region: {cell:?}");
                    prop_assert!(cell.y + cell.h <= region.y + region.h, "bottom edge in region: {cell:?}");
                }
                // No two cells overlap: row-major indices map to disjoint origins.
                for (i, a) in cells.iter().enumerate() {
                    for b in &cells[i + 1..] {
                        let disjoint_x = a.x + a.w <= b.x || b.x + b.w <= a.x;
                        let disjoint_y = a.y + a.h <= b.y || b.y + b.h <= a.y;
                        prop_assert!(disjoint_x || disjoint_y, "cells overlap: {a:?} {b:?}");
                    }
                }
            }
        }
    }

    fn sample() -> Structure {
        Structure {
            id: StructureId("test.s".into()),
            name: "Test".into(),
            output: StructureOutput::Paneled {
                canvas: Dimensions { width: 100, height: 200 },
                panels: vec![StructurePanel {
                    label: "front".into(),
                    rect: PanelRect { x: 0, y: 0, w: 50, h: 100 },
                    prose_fragment: "front view {panel_w}x{panel_h}".into(),
                    slot: PanelSlot::View,
                }],
            },
            layout_negatives: "overlapping views".into(),
        }
    }

    #[test]
    fn structure_round_trips() {
        let s = sample();
        let json = serde_json::to_string(&s).unwrap();
        let back: Structure = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn single_output_serializes_as_snake_case() {
        let json = serde_json::to_string(&StructureOutput::Single).unwrap();
        assert_eq!(json, r#""single""#);
    }

    #[test]
    fn panel_slot_serializes_as_snake_case() {
        assert_eq!(serde_json::to_string(&PanelSlot::PaletteSwatch).unwrap(), r#""palette_swatch""#);
    }
}
