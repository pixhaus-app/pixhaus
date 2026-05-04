//! Sprite sheet packing algorithms.
//!
//! Three layout strategies are supported:
//!
//! - [`LayoutStrategy::Grid`] — uniform grid; caller picks the column count.
//! - [`LayoutStrategy::ByRow`] — one frame per row; the simplest possible layout.
//! - [`LayoutStrategy::Packed`] — Skyline bin-packing for a near-square sheet.
//!
//! All strategies require uniform frame dimensions (every frame must have the
//! same width and height as `Sprite.canvas`).

use pixhaus_core::project::geometry::Size;

use crate::error::{Error, Result};

/// Strategy for placing frames on the sprite sheet.
#[derive(Debug, Clone)]
pub enum LayoutStrategy {
    /// Place frames in a uniform grid with a fixed column count.
    ///
    /// Frames are arranged left-to-right, top-to-bottom. `cols` must be
    /// non-zero. The row count is `ceil(frame_count / cols)`.
    Grid {
        /// Column count. Must be non-zero.
        cols: u32,
    },
    /// One frame per row.
    ///
    /// Sheet width equals frame width; sheet height equals frame height times
    /// the frame count. Equivalent to `Grid { cols: 1 }` but more explicit.
    ByRow,
    /// Bin-packed layout that minimises wasted area.
    ///
    /// Uses a Skyline algorithm to place frames. The initial sheet width is
    /// chosen so the sheet is approximately square. Because sprite sheet
    /// frames always have uniform dimensions, the packed layout degenerates
    /// to an optimised grid with minimal wasted cells.
    Packed,
}

/// Placement of one frame within the sprite sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FramePlacement {
    /// Left edge of the frame in sheet pixel coordinates.
    pub x: u32,
    /// Top edge of the frame in sheet pixel coordinates.
    pub y: u32,
}

/// Output of a packing operation.
#[derive(Debug)]
pub struct PackResult {
    /// Per-frame placements, in frame-index order.
    pub placements: Vec<FramePlacement>,
    /// Packed sheet width in pixels.
    pub sheet_width: u32,
    /// Packed sheet height in pixels.
    pub sheet_height: u32,
}

/// Pack `frame_count` frames of uniform `frame_size` using `strategy`.
///
/// # Errors
///
/// - [`Error::NoFrames`] when `frame_count == 0`.
/// - [`Error::GridColsZero`] when `strategy` is `Grid { cols: 0 }`.
pub fn pack_frames(
    frame_count: usize,
    frame_size: Size,
    strategy: &LayoutStrategy,
) -> Result<PackResult> {
    if frame_count == 0 {
        return Err(Error::NoFrames);
    }
    // frame_count > 0 here; sprite sheet frames are bounded by display memory,
    // so truncation to u32 is safe in practice.
    #[allow(clippy::cast_possible_truncation)]
    let n = frame_count as u32;
    let fw = frame_size.width;
    let fh = frame_size.height;

    match strategy {
        LayoutStrategy::Grid { cols } => {
            if *cols == 0 {
                return Err(Error::GridColsZero);
            }
            let cols = *cols;
            let rows = n.div_ceil(cols);
            let placements = (0..n)
                .map(|i| FramePlacement {
                    x: (i % cols) * fw,
                    y: (i / cols) * fh,
                })
                .collect();
            Ok(PackResult {
                placements,
                sheet_width: fw * cols,
                sheet_height: fh * rows,
            })
        }
        LayoutStrategy::ByRow => {
            let placements = (0..n).map(|i| FramePlacement { x: 0, y: i * fh }).collect();
            Ok(PackResult {
                placements,
                sheet_width: fw,
                sheet_height: fh * n,
            })
        }
        LayoutStrategy::Packed => Ok(pack_skyline(n, frame_size)),
    }
}

/// Skyline-based packing for the [`LayoutStrategy::Packed`] variant.
fn pack_skyline(n: u32, frame_size: Size) -> PackResult {
    let fw = frame_size.width;
    let fh = frame_size.height;

    // Target a square sheet: ceil(sqrt(N)) columns.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let target_cols = f64::from(n).sqrt().ceil() as u32;
    let sheet_w = fw * target_cols.max(1);

    let mut skyline = Skyline::new(sheet_w);
    let mut placements = Vec::with_capacity(n as usize);

    for _ in 0..n {
        let (x, y) = skyline.place(fw, fh);
        placements.push(FramePlacement { x, y });
    }

    PackResult {
        placements,
        sheet_width: sheet_w,
        sheet_height: skyline.max_height(),
    }
}

/// Skyline bin-packer.
///
/// Maintains a step-function "skyline" (the top profile of placed
/// rectangles) and inserts each new rectangle at the lowest-y,
/// leftmost-x position. The skyline is stored as a sorted list of
/// `(x_start, height)` segments; each segment covers from its
/// `x_start` to the next segment's `x_start` (or to `width` for the
/// last entry).
struct Skyline {
    /// Step-function segments `(x_start, height)`, sorted ascending by `x`.
    segs: Vec<(u32, u32)>,
    /// Sheet width. No rect may extend past this.
    width: u32,
}

impl Skyline {
    fn new(width: u32) -> Self {
        // One segment covering [0, width) at height 0.
        Self {
            segs: vec![(0, 0)],
            width,
        }
    }

    /// Maximum height reached by any placed rectangle.
    fn max_height(&self) -> u32 {
        self.segs.iter().map(|&(_, h)| h).max().unwrap_or(0)
    }

    /// Height of the skyline at position `x`.
    ///
    /// Returns the height of the segment whose `x_start <= x`. Uses
    /// `partition_point` to binary-search the sorted segment list.
    fn height_at(&self, x: u32) -> u32 {
        let pos = self.segs.partition_point(|&(sx, _)| sx <= x);
        if pos == 0 { 0 } else { self.segs[pos - 1].1 }
    }

    /// Maximum height (ceiling) in the range `[x, x + w)`.
    ///
    /// Returns `None` if the range would exceed `width`.
    fn ceiling(&self, x: u32, w: u32) -> Option<u32> {
        let x_end = x + w;
        if x_end > self.width {
            return None;
        }
        // The ceiling is the max of: the height of the segment containing x,
        // and the heights of all segments that start strictly inside [x, x_end).
        let base = self.height_at(x);
        let interior = self
            .segs
            .iter()
            .filter(|&&(sx, _)| sx > x && sx < x_end)
            .map(|&(_, h)| h)
            .max()
            .unwrap_or(0);
        Some(base.max(interior))
    }

    /// Place a `w × h` rectangle at the lowest-y, leftmost-x position.
    ///
    /// Tries every existing segment start as a candidate `x`, picks the
    /// one with the lowest ceiling (ties broken by leftmost `x`), then
    /// updates the skyline.
    fn place(&mut self, w: u32, h: u32) -> (u32, u32) {
        let mut best_x = 0u32;
        let mut best_y = u32::MAX;

        // Collect starts first to avoid holding an immutable borrow while mutating.
        let starts: Vec<u32> = self.segs.iter().map(|&(x, _)| x).collect();

        for x in starts {
            if let Some(y) = self.ceiling(x, w) {
                if y < best_y {
                    best_y = y;
                    best_x = x;
                }
            }
        }

        if best_y == u32::MAX {
            // Sheet is narrower than w — stack on the full height.
            best_y = self.max_height();
            best_x = 0;
        }

        self.update(best_x, best_y, w, h);
        (best_x, best_y)
    }

    /// Update the skyline after placing a `w × h` rect at `(x, y)`.
    ///
    /// The segment in `[x, x + w)` is raised to `y + h`. Segments that
    /// start inside this range are removed; the boundary at `x + w` is
    /// restored to the pre-placement height if it was not already a
    /// segment start.
    fn update(&mut self, x: u32, y: u32, w: u32, h: u32) {
        let new_top = y + h;
        let x_end = x + w;

        // Capture the height at x_end *before* modifying the skyline —
        // we need this to restore the segment boundary at x_end.
        let right_h = if x_end < self.width {
            self.height_at(x_end)
        } else {
            0
        };

        // Remove all segments that start in [x, x_end).
        self.segs.retain(|&(sx, _)| sx < x || sx >= x_end);

        // Insert the new segment at x.
        let insert_pos = self.segs.partition_point(|&(sx, _)| sx < x);
        self.segs.insert(insert_pos, (x, new_top));

        // Restore the segment boundary at x_end if it was erased.
        if x_end < self.width {
            let end_pos = self.segs.partition_point(|&(sx, _)| sx < x_end);
            if self.segs.get(end_pos).is_none_or(|&(sx, _)| sx != x_end) {
                self.segs.insert(end_pos, (x_end, right_h));
            }
        }

        self.merge();
    }

    /// Merge adjacent segments that share the same height.
    fn merge(&mut self) {
        let mut i = 1;
        while i < self.segs.len() {
            if self.segs[i - 1].1 == self.segs[i].1 {
                self.segs.remove(i);
            } else {
                i += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(w: u32, h: u32) -> Size {
        Size::new(w, h)
    }

    // ── Error cases ─────────────────────────────────────────────────────────

    #[test]
    fn no_frames_returns_error() {
        assert!(matches!(
            pack_frames(0, size(16, 16), &LayoutStrategy::ByRow),
            Err(Error::NoFrames)
        ));
    }

    #[test]
    fn grid_cols_zero_returns_error() {
        assert!(matches!(
            pack_frames(4, size(16, 16), &LayoutStrategy::Grid { cols: 0 }),
            Err(Error::GridColsZero)
        ));
    }

    // ── Grid ────────────────────────────────────────────────────────────────

    #[test]
    fn grid_single_row_four_frames() {
        let r = pack_frames(4, size(16, 16), &LayoutStrategy::Grid { cols: 4 }).unwrap();
        assert_eq!(r.sheet_width, 64);
        assert_eq!(r.sheet_height, 16);
        assert_eq!(r.placements[0], FramePlacement { x: 0, y: 0 });
        assert_eq!(r.placements[1], FramePlacement { x: 16, y: 0 });
        assert_eq!(r.placements[3], FramePlacement { x: 48, y: 0 });
    }

    #[test]
    fn grid_wraps_to_second_row() {
        let r = pack_frames(6, size(16, 16), &LayoutStrategy::Grid { cols: 4 }).unwrap();
        assert_eq!(r.sheet_width, 64);
        assert_eq!(r.sheet_height, 32);
        assert_eq!(r.placements[4], FramePlacement { x: 0, y: 16 });
        assert_eq!(r.placements[5], FramePlacement { x: 16, y: 16 });
    }

    #[test]
    fn grid_single_frame() {
        let r = pack_frames(1, size(32, 32), &LayoutStrategy::Grid { cols: 1 }).unwrap();
        assert_eq!(r.sheet_width, 32);
        assert_eq!(r.sheet_height, 32);
        assert_eq!(r.placements[0], FramePlacement { x: 0, y: 0 });
    }

    // ── ByRow ────────────────────────────────────────────────────────────────

    #[test]
    fn by_row_stacks_vertically() {
        let r = pack_frames(3, size(32, 32), &LayoutStrategy::ByRow).unwrap();
        assert_eq!(r.sheet_width, 32);
        assert_eq!(r.sheet_height, 96);
        assert_eq!(r.placements[0], FramePlacement { x: 0, y: 0 });
        assert_eq!(r.placements[1], FramePlacement { x: 0, y: 32 });
        assert_eq!(r.placements[2], FramePlacement { x: 0, y: 64 });
    }

    // ── Packed (Skyline) ─────────────────────────────────────────────────────

    #[test]
    fn packed_single_frame_at_origin() {
        let r = pack_frames(1, size(16, 16), &LayoutStrategy::Packed).unwrap();
        assert_eq!(r.placements[0], FramePlacement { x: 0, y: 0 });
        assert_eq!(r.sheet_height, 16);
    }

    #[test]
    fn packed_four_frames_fit_in_2x2_grid() {
        let r = pack_frames(4, size(16, 16), &LayoutStrategy::Packed).unwrap();
        // sqrt(4) = 2, so 2 cols × 16 = 32 wide, 32 tall.
        assert_eq!(r.sheet_width, 32);
        assert_eq!(r.sheet_height, 32);
        assert_eq!(r.placements.len(), 4);
        no_overlaps(&r.placements, 16, 16);
    }

    #[test]
    fn packed_nine_frames_no_overlap() {
        let r = pack_frames(9, size(16, 16), &LayoutStrategy::Packed).unwrap();
        assert_eq!(r.placements.len(), 9);
        no_overlaps(&r.placements, 16, 16);
    }

    #[test]
    fn packed_hundred_frames_no_overlap() {
        let r = pack_frames(100, size(8, 8), &LayoutStrategy::Packed).unwrap();
        assert_eq!(r.placements.len(), 100);
        no_overlaps(&r.placements, 8, 8);
    }

    // ── Skyline internals ────────────────────────────────────────────────────

    #[test]
    fn skyline_two_frames_side_by_side() {
        let mut s = Skyline::new(32);
        let (x0, y0) = s.place(16, 16);
        assert_eq!((x0, y0), (0, 0));
        let (x1, y1) = s.place(16, 16);
        assert_eq!((x1, y1), (16, 0));
        assert_eq!(s.max_height(), 16);
    }

    #[test]
    fn skyline_third_frame_starts_new_row() {
        let mut s = Skyline::new(32);
        s.place(16, 16);
        s.place(16, 16);
        let (_, y) = s.place(16, 16);
        assert_eq!(y, 16, "third frame should start a new row");
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// Assert that no two placements overlap given uniform frame size.
    fn no_overlaps(placements: &[FramePlacement], fw: u32, fh: u32) {
        for i in 0..placements.len() {
            for j in (i + 1)..placements.len() {
                let a = placements[i];
                let b = placements[j];
                let overlap_x = a.x < b.x + fw && b.x < a.x + fw;
                let overlap_y = a.y < b.y + fh && b.y < a.y + fh;
                assert!(
                    !(overlap_x && overlap_y),
                    "frames {i} and {j} overlap: {a:?} and {b:?}"
                );
            }
        }
    }
}
