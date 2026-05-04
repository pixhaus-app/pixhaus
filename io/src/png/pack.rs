//! Sprite sheet packing algorithms.
//!
//! Three layout strategies are supported:
//!
//! - [`LayoutStrategy::Grid`] — uniform grid; caller picks the column count.
//! - [`LayoutStrategy::ByRow`] — one frame per row; the simplest possible layout.
//! - [`LayoutStrategy::Packed`] — Skyline bin-packing for a near-square sheet.
//!
//! All strategies accept per-frame sizes so that alpha-trimmed frames pack
//! at their trimmed dimensions rather than the full canvas size.

use pixhaus_core::project::geometry::Size;

use crate::error::{Error, Result};

/// Maximum pixels allowed on any single side of the packed sheet.
///
/// Most GPUs cap texture dimensions at 8 192 or 16 384 px; a sheet beyond
/// 16 384 × 16 384 is also unusable as a Unity sprite atlas. Inputs that
/// would produce a larger sheet fail with [`Error::SheetTooLarge`] before
/// any allocation occurs.
pub const MAX_SHEET_DIM: u32 = 16_384;

/// Strategy for placing frames on the sprite sheet.
#[derive(Debug, Clone)]
pub enum LayoutStrategy {
    /// Place frames in a uniform grid with a fixed column count.
    ///
    /// Frames are arranged left-to-right, top-to-bottom. `cols` must be
    /// non-zero. The cell size is the maximum trimmed width × maximum
    /// trimmed height across all frames, so the grid stays rectangular
    /// even when individual frames differ in size after alpha-trim.
    Grid {
        /// Column count. Must be non-zero.
        cols: u32,
    },
    /// One frame per row.
    ///
    /// Sheet width equals the maximum trimmed frame width; each row height
    /// equals the trimmed height of the frame in that row.
    ByRow,
    /// Bin-packed layout that minimises wasted area.
    ///
    /// Uses a Skyline algorithm to place frames. Each frame is packed at
    /// its trimmed dimensions. The initial sheet width targets a square
    /// sheet based on the total trimmed area.
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

/// Pack `frame_sizes` using `strategy`.
///
/// Each element of `frame_sizes` is the effective (possibly alpha-trimmed)
/// width and height for that frame. Pass the full canvas size for frames
/// that were not trimmed.
///
/// # Errors
///
/// - [`Error::NoFrames`] when `frame_sizes` is empty.
/// - [`Error::GridColsZero`] when `strategy` is `Grid { cols: 0 }`.
/// - [`Error::SheetTooLarge`] when the resulting sheet would exceed
///   [`MAX_SHEET_DIM`] on either axis before any allocation is attempted.
pub fn pack_frames(frame_sizes: &[Size], strategy: &LayoutStrategy) -> Result<PackResult> {
    if frame_sizes.is_empty() {
        return Err(Error::NoFrames);
    }
    if let LayoutStrategy::Grid { cols: 0 } = strategy {
        return Err(Error::GridColsZero);
    }

    let result = match strategy {
        LayoutStrategy::Grid { cols } => pack_grid(frame_sizes, *cols),
        LayoutStrategy::ByRow => pack_by_row(frame_sizes),
        LayoutStrategy::Packed => pack_skyline(frame_sizes),
    };

    if result.sheet_width > MAX_SHEET_DIM || result.sheet_height > MAX_SHEET_DIM {
        return Err(Error::SheetTooLarge {
            width: result.sheet_width,
            height: result.sheet_height,
            max: MAX_SHEET_DIM,
        });
    }

    Ok(result)
}

// ── Strategy implementations ─────────────────────────────────────────────────

/// Grid layout: uniform cell size = max trimmed width × max trimmed height.
#[allow(clippy::cast_possible_truncation)]
fn pack_grid(frame_sizes: &[Size], cols: u32) -> PackResult {
    let n = frame_sizes.len() as u32;
    let max_w = frame_sizes.iter().map(|s| s.width).max().unwrap_or(1);
    let max_h = frame_sizes.iter().map(|s| s.height).max().unwrap_or(1);
    let rows = n.div_ceil(cols);
    let placements = (0..n)
        .map(|i| FramePlacement {
            x: (i % cols) * max_w,
            y: (i / cols) * max_h,
        })
        .collect();
    PackResult {
        placements,
        sheet_width: max_w * cols,
        sheet_height: max_h * rows,
    }
}

/// `ByRow` layout: max trimmed width × cumulative trimmed heights.
fn pack_by_row(frame_sizes: &[Size]) -> PackResult {
    let max_w = frame_sizes.iter().map(|s| s.width).max().unwrap_or(1);
    let mut y = 0u32;
    let placements = frame_sizes
        .iter()
        .map(|s| {
            let p = FramePlacement { x: 0, y };
            y += s.height;
            p
        })
        .collect();
    PackResult {
        placements,
        sheet_width: max_w,
        sheet_height: y,
    }
}

/// Skyline-based packing for the [`LayoutStrategy::Packed`] variant.
///
/// Initial sheet width is `ceil(sqrt(total_trimmed_area))`, clamped to at
/// least the widest single frame. For uniform-size inputs this is identical
/// to the previous `ceil(sqrt(n)) * fw` formula.
fn pack_skyline(frame_sizes: &[Size]) -> PackResult {
    let max_w = frame_sizes.iter().map(|s| s.width).max().unwrap_or(1);
    let total_area: u64 = frame_sizes
        .iter()
        .map(|s| u64::from(s.width) * u64::from(s.height))
        .sum();

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let sheet_w = ((total_area as f64).sqrt().ceil() as u32).max(max_w);

    let mut skyline = Skyline::new(sheet_w);
    let mut placements = Vec::with_capacity(frame_sizes.len());

    for s in frame_sizes {
        let (x, y) = skyline.place(s.width, s.height);
        placements.push(FramePlacement { x, y });
    }

    PackResult {
        placements,
        sheet_width: sheet_w,
        sheet_height: skyline.max_height(),
    }
}

// ── Skyline bin-packer ───────────────────────────────────────────────────────

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

    fn uniform(count: usize, w: u32, h: u32) -> Vec<Size> {
        vec![size(w, h); count]
    }

    // ── Error cases ──────────────────────────────────────────────────────────

    #[test]
    fn no_frames_returns_error() {
        assert!(matches!(
            pack_frames(&[], &LayoutStrategy::ByRow),
            Err(Error::NoFrames)
        ));
    }

    #[test]
    fn grid_cols_zero_returns_error() {
        assert!(matches!(
            pack_frames(&uniform(4, 16, 16), &LayoutStrategy::Grid { cols: 0 }),
            Err(Error::GridColsZero)
        ));
    }

    #[test]
    fn sheet_too_large_is_rejected() {
        // 65 frames of 256 × 256 in ByRow → height = 65 × 256 = 16 640 > 16 384.
        // The check fires before any allocation.
        assert!(matches!(
            pack_frames(&uniform(65, 256, 256), &LayoutStrategy::ByRow),
            Err(Error::SheetTooLarge { .. })
        ));
    }

    // ── Grid ─────────────────────────────────────────────────────────────────

    #[test]
    fn grid_single_row_four_frames() {
        let r = pack_frames(&uniform(4, 16, 16), &LayoutStrategy::Grid { cols: 4 }).unwrap();
        assert_eq!(r.sheet_width, 64);
        assert_eq!(r.sheet_height, 16);
        assert_eq!(r.placements[0], FramePlacement { x: 0, y: 0 });
        assert_eq!(r.placements[1], FramePlacement { x: 16, y: 0 });
        assert_eq!(r.placements[3], FramePlacement { x: 48, y: 0 });
    }

    #[test]
    fn grid_wraps_to_second_row() {
        let r = pack_frames(&uniform(6, 16, 16), &LayoutStrategy::Grid { cols: 4 }).unwrap();
        assert_eq!(r.sheet_width, 64);
        assert_eq!(r.sheet_height, 32);
        assert_eq!(r.placements[4], FramePlacement { x: 0, y: 16 });
        assert_eq!(r.placements[5], FramePlacement { x: 16, y: 16 });
    }

    #[test]
    fn grid_single_frame() {
        let r = pack_frames(&uniform(1, 32, 32), &LayoutStrategy::Grid { cols: 1 }).unwrap();
        assert_eq!(r.sheet_width, 32);
        assert_eq!(r.sheet_height, 32);
        assert_eq!(r.placements[0], FramePlacement { x: 0, y: 0 });
    }

    // ── ByRow ────────────────────────────────────────────────────────────────

    #[test]
    fn by_row_stacks_vertically() {
        let r = pack_frames(&uniform(3, 32, 32), &LayoutStrategy::ByRow).unwrap();
        assert_eq!(r.sheet_width, 32);
        assert_eq!(r.sheet_height, 96);
        assert_eq!(r.placements[0], FramePlacement { x: 0, y: 0 });
        assert_eq!(r.placements[1], FramePlacement { x: 0, y: 32 });
        assert_eq!(r.placements[2], FramePlacement { x: 0, y: 64 });
    }

    #[test]
    fn by_row_variable_heights() {
        // Trimmed frames with different heights.
        let sizes = vec![size(10, 5), size(10, 8), size(10, 3)];
        let r = pack_frames(&sizes, &LayoutStrategy::ByRow).unwrap();
        assert_eq!(r.sheet_width, 10);
        assert_eq!(r.sheet_height, 16); // 5 + 8 + 3
        assert_eq!(r.placements[0], FramePlacement { x: 0, y: 0 });
        assert_eq!(r.placements[1], FramePlacement { x: 0, y: 5 });
        assert_eq!(r.placements[2], FramePlacement { x: 0, y: 13 });
    }

    // ── Packed (Skyline) ─────────────────────────────────────────────────────

    #[test]
    fn packed_single_frame_at_origin() {
        let r = pack_frames(&uniform(1, 16, 16), &LayoutStrategy::Packed).unwrap();
        assert_eq!(r.placements[0], FramePlacement { x: 0, y: 0 });
        assert_eq!(r.sheet_height, 16);
    }

    #[test]
    fn packed_four_frames_fit_in_2x2_grid() {
        let r = pack_frames(&uniform(4, 16, 16), &LayoutStrategy::Packed).unwrap();
        // sqrt(4 * 16 * 16) = sqrt(1024) = 32
        assert_eq!(r.sheet_width, 32);
        assert_eq!(r.sheet_height, 32);
        assert_eq!(r.placements.len(), 4);
        no_overlaps(&r.placements, 16, 16);
    }

    #[test]
    fn packed_nine_frames_no_overlap() {
        let r = pack_frames(&uniform(9, 16, 16), &LayoutStrategy::Packed).unwrap();
        assert_eq!(r.placements.len(), 9);
        no_overlaps(&r.placements, 16, 16);
    }

    #[test]
    fn packed_hundred_frames_no_overlap() {
        let r = pack_frames(&uniform(100, 8, 8), &LayoutStrategy::Packed).unwrap();
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
