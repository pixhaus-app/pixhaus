//! Smart-slice: detect sprite-sheet frame boundaries from transparency.
//!
//! [`detect_frames`] flood-fills the transparent background inward from the
//! image border, treats every unreached pixel as foreground, labels the
//! foreground into connected components, and returns one bounding box per
//! component. Interior transparent pixels (a donut hole, an eye) stay part of
//! their sprite because the border flood never reaches them.
//!
//! Ported from Pixelorama's `smart_slice`
//! (`src/UI/Dialogs/ImportPreviewDialog.gd`). See `THIRD_PARTY_NOTICES.md`.

// Bounding boxes cross the u32<->i32 boundary; image dimensions stay well
// under 2^31 so these casts never wrap or lose sign in practice.
#![allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]

use std::collections::VecDeque;

use crate::canvas::PixelBuffer;
use crate::project::geometry::Rect;

/// Options controlling [`detect_frames`].
#[derive(Copy, Clone, Debug)]
pub struct SmartSliceOpts {
    /// A pixel counts as background only if its alpha is at or below this.
    /// `0` treats only fully-transparent pixels as background.
    pub alpha_threshold: u8,
    /// Components whose bounding box area is smaller than this are dropped as
    /// noise. `1` keeps everything.
    pub min_area: u32,
}

impl Default for SmartSliceOpts {
    fn default() -> Self {
        Self {
            alpha_threshold: 0,
            min_area: 1,
        }
    }
}

/// Detects sprite frames in `image` and returns their bounding boxes in
/// reading order (top to bottom, then left to right within a row band).
///
/// Returns an empty vector for an empty image or one with no foreground.
#[must_use]
pub fn detect_frames(image: &PixelBuffer, opts: SmartSliceOpts) -> Vec<Rect> {
    let w = image.width();
    let h = image.height();
    if w == 0 || h == 0 {
        return Vec::new();
    }

    let idx = |x: u32, y: u32| (y * w + x) as usize;
    let is_background = |x: u32, y: u32| {
        image
            .pixel(x, y)
            .is_none_or(|c| c.a <= opts.alpha_threshold)
    };

    // 1. Flood the background inward from every border pixel. `outside[i]`
    //    is true for background pixels reachable from the border.
    let mut outside = vec![false; (w * h) as usize];
    let mut queue: VecDeque<(u32, u32)> = VecDeque::new();
    let enqueue = |x: u32, y: u32, outside: &mut [bool], queue: &mut VecDeque<(u32, u32)>| {
        if is_background(x, y) && !outside[idx(x, y)] {
            outside[idx(x, y)] = true;
            queue.push_back((x, y));
        }
    };
    for x in 0..w {
        enqueue(x, 0, &mut outside, &mut queue);
        enqueue(x, h - 1, &mut outside, &mut queue);
    }
    for y in 0..h {
        enqueue(0, y, &mut outside, &mut queue);
        enqueue(w - 1, y, &mut outside, &mut queue);
    }
    while let Some((x, y)) = queue.pop_front() {
        for (nx, ny) in four_neighbours(x, y, w, h) {
            if is_background(nx, ny) && !outside[idx(nx, ny)] {
                outside[idx(nx, ny)] = true;
                queue.push_back((nx, ny));
            }
        }
    }

    // 2. Label connected foreground components (4-connectivity), tracking the
    //    bounding box of each as we go.
    let is_foreground = |x: u32, y: u32| !is_background(x, y) || !outside[idx(x, y)];
    let mut seen = vec![false; (w * h) as usize];
    let mut boxes: Vec<Rect> = Vec::new();

    for sy in 0..h {
        for sx in 0..w {
            if seen[idx(sx, sy)] || !is_foreground(sx, sy) {
                continue;
            }
            let (mut min_x, mut min_y, mut max_x, mut max_y) = (sx, sy, sx, sy);
            seen[idx(sx, sy)] = true;
            queue.push_back((sx, sy));
            while let Some((x, y)) = queue.pop_front() {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                for (nx, ny) in four_neighbours(x, y, w, h) {
                    if !seen[idx(nx, ny)] && is_foreground(nx, ny) {
                        seen[idx(nx, ny)] = true;
                        queue.push_back((nx, ny));
                    }
                }
            }
            let bw = max_x - min_x + 1;
            let bh = max_y - min_y + 1;
            if bw * bh >= opts.min_area {
                boxes.push(Rect::from_xywh(min_x as i32, min_y as i32, bw, bh));
            }
        }
    }

    // 3. Reading order: group into rows by vertical overlap, then sort by x.
    boxes.sort_by(|a, b| {
        let ay = a.origin.y;
        let by = b.origin.y;
        // Same row band when their y-origins are within the shorter height.
        let band = a.size.height.min(b.size.height) as i32 / 2;
        if (ay - by).abs() <= band {
            a.origin.x.cmp(&b.origin.x)
        } else {
            ay.cmp(&by)
        }
    });
    boxes
}

/// In-bounds 4-connected neighbours of `(x, y)`.
fn four_neighbours(x: u32, y: u32, w: u32, h: u32) -> impl Iterator<Item = (u32, u32)> {
    let mut out = Vec::with_capacity(4);
    if x > 0 {
        out.push((x - 1, y));
    }
    if x + 1 < w {
        out.push((x + 1, y));
    }
    if y > 0 {
        out.push((x, y - 1));
    }
    if y + 1 < h {
        out.push((x, y + 1));
    }
    out.into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Rgba;

    const CLEAR: Rgba = Rgba::new(0, 0, 0, 0);
    const FILL: Rgba = Rgba::new(255, 0, 0, 255);

    fn buf(w: u32, h: u32, rows: &[&[Rgba]]) -> PixelBuffer {
        let mut b = PixelBuffer::new(w, h).unwrap();
        for (y, row) in rows.iter().enumerate() {
            for (x, &c) in row.iter().enumerate() {
                b.set_pixel(x as u32, y as u32, c);
            }
        }
        b
    }

    #[test]
    fn empty_image_has_no_frames() {
        let b = PixelBuffer::empty();
        assert!(detect_frames(&b, SmartSliceOpts::default()).is_empty());
    }

    #[test]
    fn fully_transparent_has_no_frames() {
        let b = PixelBuffer::new(4, 4).unwrap();
        assert!(detect_frames(&b, SmartSliceOpts::default()).is_empty());
    }

    #[test]
    fn detects_two_separated_sprites() {
        // Two 1px dots separated by a transparent column.
        let b = buf(5, 1, &[&[FILL, CLEAR, CLEAR, CLEAR, FILL]]);
        let frames = detect_frames(&b, SmartSliceOpts::default());
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], Rect::from_xywh(0, 0, 1, 1));
        assert_eq!(frames[1], Rect::from_xywh(4, 0, 1, 1));
    }

    #[test]
    fn interior_hole_stays_one_frame() {
        // A 3x3 ring with a transparent center: one frame, full bbox.
        let b = buf(
            3,
            3,
            &[
                &[FILL, FILL, FILL],
                &[FILL, CLEAR, FILL],
                &[FILL, FILL, FILL],
            ],
        );
        let frames = detect_frames(&b, SmartSliceOpts::default());
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], Rect::from_xywh(0, 0, 3, 3));
    }

    #[test]
    fn reading_order_sorts_top_left_first() {
        // Bottom-right sprite and top-left sprite in a 3x3 grid; expect the
        // top-left one first.
        let b = buf(
            3,
            3,
            &[
                &[FILL, CLEAR, CLEAR],
                &[CLEAR, CLEAR, CLEAR],
                &[CLEAR, CLEAR, FILL],
            ],
        );
        let frames = detect_frames(&b, SmartSliceOpts::default());
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], Rect::from_xywh(0, 0, 1, 1));
        assert_eq!(frames[1], Rect::from_xywh(2, 2, 1, 1));
    }

    #[test]
    fn min_area_drops_noise() {
        let b = buf(5, 1, &[&[FILL, CLEAR, CLEAR, CLEAR, FILL]]);
        let frames = detect_frames(
            &b,
            SmartSliceOpts {
                alpha_threshold: 0,
                min_area: 2,
            },
        );
        assert!(frames.is_empty());
    }
}
