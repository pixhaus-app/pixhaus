//! Sprite export geometry: hard alpha masking and fixed-cell atlas packing.
//!
//! Pure pixel and geometry work, no IO and no encoding — those live in the
//! shell, which owns the `image`-crate encoders and the `rfd` save dialog. This
//! module is the shared half the GUI exporter and the headless CLI both call so
//! the two paths never drift.
//!
//! Two operations:
//!
//! - [`hard_mask_alpha`] clamps every alpha to `0` or `255` at a threshold. GIF
//!   has 1-bit alpha; encoding straight RGBA leaves a halo of semi-transparent
//!   fringe because the quantizer dithers the partially-transparent edge into
//!   visible speckle. Hard-masking before the encode kills that halo.
//! - [`pack_atlas`] lays equal-size frames into a fixed-cell grid. Every frame
//!   shares the sprite canvas size, so this is a grid pack, not a bin-packer;
//!   the layout picks the column count and the rest is arithmetic.

use crate::canvas::buffer::{PixelBuffer, RGBA_BYTES_PER_PIXEL};
use crate::project::Rgba;

/// Why an export geometry op could not run.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// [`pack_atlas`] was handed an empty frame slice.
    #[error("no frames to export")]
    NoFrames,
    /// A frame's dimensions differ from the first frame's — the grid pack
    /// requires every cell to share one size.
    #[error("frame {index} is {got:?}, expected {expected:?}")]
    SizeMismatch {
        /// Index of the offending frame.
        index: usize,
        /// The frame's actual `(width, height)`.
        got: (u32, u32),
        /// The `(width, height)` every frame must match (the first frame's).
        expected: (u32, u32),
    },
    /// The packed atlas would exceed the per-side dimension limit (the 8K
    /// constraint), so it is rejected rather than allocating hundreds of MB.
    #[error("atlas dimensions {w}x{h} exceed the {max}px limit")]
    TooLarge {
        /// Atlas width that overflowed.
        w: u32,
        /// Atlas height that overflowed.
        h: u32,
        /// The per-side maximum.
        max: u32,
    },
}

/// Clamps every alpha byte in `buf` to `0` or `255`: an alpha `>= threshold`
/// becomes fully opaque, anything below becomes fully transparent. RGB channels
/// are left untouched, so a later opacity tweak does not reveal a colour shift.
///
/// The forge default threshold is `128`. The output is what the GIF encoder
/// wants — every edge pixel is either kept or routed to the reserved
/// transparent index, with no in-between to dither into a halo.
///
/// Iterates the trimmed row of each line (skipping any SIMD stride padding),
/// matching the row-walk in [`crate::transforms::normalize::chroma_key`].
#[must_use]
pub fn hard_mask_alpha(buf: &PixelBuffer, threshold: u8) -> PixelBuffer {
    let mut out = buf.clone();
    for y in 0..out.height() {
        let Some(row) = out.row_mut(y) else { continue };
        for chunk in row.chunks_exact_mut(RGBA_BYTES_PER_PIXEL) {
            chunk[3] = if chunk[3] >= threshold { 255 } else { 0 };
        }
    }
    out
}

/// How [`pack_atlas`] arranges the frames into a grid.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AtlasLayout {
    /// One row: `frames.len()` columns, a single row.
    SingleRow,
    /// A fixed column count; rows follow from the frame count. A `0` count is
    /// treated as `1` so the pack never divides by zero.
    Columns(u32),
    /// The near-square grid `ceil(sqrt(n))` columns wide.
    AutoSquare,
}

/// Options for [`pack_atlas`].
#[derive(Copy, Clone, Debug)]
pub struct AtlasOptions {
    /// The grid arrangement.
    pub layout: AtlasLayout,
    /// Largest atlas dimension per side. Defaults to `8192` via
    /// [`AtlasOptions::default`], matching the canvas 8K limit.
    pub max_dim: u32,
}

impl Default for AtlasOptions {
    fn default() -> Self {
        Self {
            layout: AtlasLayout::AutoSquare,
            max_dim: 8192,
        }
    }
}

/// Where one frame landed in the packed atlas, in atlas pixel space.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FrameRect {
    /// The source frame index this rect holds.
    pub index: usize,
    /// Left edge in the atlas.
    pub x: u32,
    /// Top edge in the atlas.
    pub y: u32,
    /// Cell width (the shared frame width).
    pub w: u32,
    /// Cell height (the shared frame height).
    pub h: u32,
}

/// A packed atlas: the composited grid image plus where each frame landed.
#[derive(Clone, Debug)]
pub struct Atlas {
    /// The packed grid as one [`PixelBuffer`]. Cells past the frame count are
    /// left transparent.
    pub image: PixelBuffer,
    /// Column count of the grid.
    pub columns: u32,
    /// One rect per input frame, in input order.
    pub frames: Vec<FrameRect>,
}

/// Resolves the column count for `n` frames under `layout`. Never returns `0`.
fn columns_for(layout: AtlasLayout, n: u32) -> u32 {
    match layout {
        AtlasLayout::SingleRow => n.max(1),
        AtlasLayout::Columns(c) => c.max(1),
        AtlasLayout::AutoSquare => {
            // ceil(sqrt(n)) without floats: smallest c with c*c >= n.
            let mut c = 1u32;
            while c.checked_mul(c).is_none_or(|sq| sq < n) {
                c += 1;
            }
            c
        }
    }
}

/// Packs equal-size `frames` into a fixed-cell grid under `opts`.
///
/// Every frame must share the first frame's dimensions; a mismatch is a
/// [`ExportError::SizeMismatch`]. The grid's column count comes from
/// [`AtlasOptions::layout`]; the row count follows from the frame count. Cells
/// beyond the last frame are left transparent (e.g. seven frames in a 3×3 grid
/// leave the last two cells empty). The atlas image is rejected with
/// [`ExportError::TooLarge`] if either side would exceed
/// [`AtlasOptions::max_dim`].
///
/// # Errors
///
/// - [`ExportError::NoFrames`] when `frames` is empty.
/// - [`ExportError::SizeMismatch`] when a frame's size differs from the first.
/// - [`ExportError::TooLarge`] when the packed atlas would exceed `max_dim` on
///   either side, or when the dimension math overflows `u32`.
pub fn pack_atlas(frames: &[PixelBuffer], opts: AtlasOptions) -> Result<Atlas, ExportError> {
    let first = frames.first().ok_or(ExportError::NoFrames)?;
    let (cell_w, cell_h) = (first.width(), first.height());
    let expected = (cell_w, cell_h);
    for (index, frame) in frames.iter().enumerate() {
        let got = (frame.width(), frame.height());
        if got != expected {
            return Err(ExportError::SizeMismatch { index, got, expected });
        }
    }

    let n = u32::try_from(frames.len()).unwrap_or(u32::MAX);
    let columns = columns_for(opts.layout, n);
    // Rows = ceil(n / columns); columns is never zero.
    let rows = n.div_ceil(columns);

    let atlas_w = cell_w.checked_mul(columns);
    let atlas_h = cell_h.checked_mul(rows);
    let (atlas_w, atlas_h) = match (atlas_w, atlas_h) {
        (Some(w), Some(h)) if w <= opts.max_dim && h <= opts.max_dim => (w, h),
        // Report the would-be size when it fits in u32, else the saturated edge.
        _ => {
            return Err(ExportError::TooLarge {
                w: atlas_w.unwrap_or(u32::MAX),
                h: atlas_h.unwrap_or(u32::MAX),
                max: opts.max_dim,
            });
        }
    };

    // A zero-area cell yields an empty atlas with placement rects still recorded.
    let mut image = PixelBuffer::filled(atlas_w, atlas_h, Rgba::transparent()).map_err(|_| ExportError::TooLarge {
        w: atlas_w,
        h: atlas_h,
        max: opts.max_dim,
    })?;

    let mut rects = Vec::with_capacity(frames.len());
    for (i, frame) in frames.iter().enumerate() {
        let slot = u32::try_from(i).unwrap_or(0);
        let dst_x = (slot % columns) * cell_w;
        let dst_y = (slot / columns) * cell_h;
        blit(&mut image, frame, dst_x, dst_y);
        rects.push(FrameRect {
            index: i,
            x: dst_x,
            y: dst_y,
            w: cell_w,
            h: cell_h,
        });
    }

    Ok(Atlas { image, columns, frames: rects })
}

/// Copies `src` into `dst` with its top-left at `(dst_x, dst_y)`, pixel for
/// pixel. The atlas is pre-sized to hold every cell, so no clipping is needed,
/// but each write is bounds-checked by `set_pixel`.
fn blit(dst: &mut PixelBuffer, src: &PixelBuffer, dst_x: u32, dst_y: u32) {
    for sy in 0..src.height() {
        for sx in 0..src.width() {
            if let Some(px) = src.pixel(sx, sy) {
                dst.set_pixel(dst_x + sx, dst_y + sy, px);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A solid-colour frame at the given size.
    fn frame(w: u32, h: u32, color: Rgba) -> PixelBuffer {
        PixelBuffer::filled(w, h, color).unwrap()
    }

    mod hard_mask_alpha {
        use super::*;
        use proptest::prelude::*;
        use rstest::rstest;

        #[rstest]
        #[case::just_below_threshold(127, 0)]
        #[case::at_threshold(128, 255)]
        #[case::full(255, 255)]
        #[case::zero(0, 0)]
        fn clamps_alpha_at_default_threshold(#[case] input_alpha: u8, #[case] expected_alpha: u8) {
            let buf = frame(2, 2, Rgba::new(10, 20, 30, input_alpha));
            let out = hard_mask_alpha(&buf, 128);
            let px = out.pixel(0, 0).unwrap();
            assert_eq!(px.a, expected_alpha, "alpha {input_alpha} at threshold 128 should clamp to {expected_alpha}");
        }

        #[test]
        fn leaves_rgb_untouched() {
            let buf = frame(3, 3, Rgba::new(11, 22, 33, 200));
            let out = hard_mask_alpha(&buf, 128);
            let px = out.pixel(1, 1).unwrap();
            assert_eq!((px.r, px.g, px.b), (11, 22, 33), "RGB must survive the mask");
        }

        #[test]
        fn preserves_dimensions() {
            let buf = frame(5, 7, Rgba::new(0, 0, 0, 64));
            let out = hard_mask_alpha(&buf, 128);
            assert_eq!((out.width(), out.height()), (5, 7));
        }

        proptest! {
            #[test]
            fn every_output_alpha_is_zero_or_255(
                a in 0u8..=255,
                threshold in 1u8..=255,
            ) {
                let buf = frame(2, 2, Rgba::new(1, 2, 3, a));
                let out = hard_mask_alpha(&buf, threshold);
                for px in out.pixels() {
                    prop_assert!(px.a == 0 || px.a == 255, "alpha {} is neither 0 nor 255", px.a);
                }
            }

            #[test]
            fn is_idempotent(
                a in 0u8..=255,
                threshold in 1u8..=255,
            ) {
                let buf = frame(2, 2, Rgba::new(4, 5, 6, a));
                let once = hard_mask_alpha(&buf, threshold);
                let twice = hard_mask_alpha(&once, threshold);
                prop_assert_eq!(once.as_bytes(), twice.as_bytes(), "a second mask must change nothing");
            }
        }
    }

    mod pack_atlas {
        use super::*;

        #[test]
        fn empty_input_is_no_frames() {
            let err = pack_atlas(&[], AtlasOptions::default()).unwrap_err();
            assert!(matches!(err, ExportError::NoFrames), "expected NoFrames, got {err:?}");
        }

        #[test]
        fn differing_frame_is_size_mismatch() {
            let frames = [frame(4, 4, Rgba::opaque(255, 0, 0)), frame(8, 4, Rgba::opaque(0, 255, 0))];
            let err = pack_atlas(&frames, AtlasOptions::default()).unwrap_err();
            assert!(
                matches!(
                    err,
                    ExportError::SizeMismatch {
                        index: 1,
                        got: (8, 4),
                        expected: (4, 4)
                    }
                ),
                "expected SizeMismatch on frame 1, got {err:?}"
            );
        }

        #[test]
        fn columns_3_over_7_frames_is_3x3_with_two_empty_cells() {
            let frames: Vec<_> = (0..7).map(|_| frame(4, 4, Rgba::opaque(10, 20, 30))).collect();
            let atlas = pack_atlas(
                &frames,
                AtlasOptions {
                    layout: AtlasLayout::Columns(3),
                    max_dim: 8192,
                },
            )
            .unwrap();
            assert_eq!(atlas.columns, 3);
            // 7 frames, 3 cols -> 3 rows -> 12x12 atlas, two trailing cells empty.
            assert_eq!((atlas.image.width(), atlas.image.height()), (12, 12));
            assert_eq!(atlas.frames.len(), 7);
            assert_eq!(
                atlas.frames[6],
                FrameRect {
                    index: 6,
                    x: 0,
                    y: 8,
                    w: 4,
                    h: 4
                }
            );
            // The two trailing cells (col 1 and 2 of the last row) stay transparent.
            assert_eq!(atlas.image.pixel(4, 8), Some(Rgba::transparent()));
            assert_eq!(atlas.image.pixel(8, 8), Some(Rgba::transparent()));
        }

        #[test]
        fn auto_square_over_4_frames_is_2x2() {
            let frames: Vec<_> = (0..4).map(|_| frame(3, 3, Rgba::opaque(1, 2, 3))).collect();
            let atlas = pack_atlas(&frames, AtlasOptions::default()).unwrap();
            assert_eq!(atlas.columns, 2);
            assert_eq!((atlas.image.width(), atlas.image.height()), (6, 6));
            assert_eq!(
                atlas.frames[3],
                FrameRect {
                    index: 3,
                    x: 3,
                    y: 3,
                    w: 3,
                    h: 3
                }
            );
        }

        #[test]
        fn single_row_over_3_frames_is_one_row() {
            let frames: Vec<_> = (0..3).map(|_| frame(5, 6, Rgba::opaque(9, 9, 9))).collect();
            let atlas = pack_atlas(
                &frames,
                AtlasOptions {
                    layout: AtlasLayout::SingleRow,
                    max_dim: 8192,
                },
            )
            .unwrap();
            assert_eq!(atlas.columns, 3);
            assert_eq!((atlas.image.width(), atlas.image.height()), (15, 6));
        }

        #[test]
        fn oversized_atlas_is_too_large() {
            // Two 5000-wide frames in a single row -> 10000 wide, past 8192.
            let frames = [frame(5000, 10, Rgba::opaque(0, 0, 0)), frame(5000, 10, Rgba::opaque(0, 0, 0))];
            let err = pack_atlas(
                &frames,
                AtlasOptions {
                    layout: AtlasLayout::SingleRow,
                    max_dim: 8192,
                },
            )
            .unwrap_err();
            assert!(
                matches!(err, ExportError::TooLarge { w: 10000, h: 10, max: 8192 }),
                "expected TooLarge 10000x10, got {err:?}"
            );
        }

        #[test]
        fn places_each_frames_pixels_at_its_rect() {
            let red = frame(2, 2, Rgba::opaque(255, 0, 0));
            let green = frame(2, 2, Rgba::opaque(0, 255, 0));
            let atlas = pack_atlas(
                &[red, green],
                AtlasOptions {
                    layout: AtlasLayout::SingleRow,
                    max_dim: 8192,
                },
            )
            .unwrap();
            // Frame 0 (red) at x=0; frame 1 (green) at x=2.
            assert_eq!(atlas.image.pixel(0, 0), Some(Rgba::opaque(255, 0, 0)));
            assert_eq!(atlas.image.pixel(2, 0), Some(Rgba::opaque(0, 255, 0)));
        }

        /// Visual regression: a packed two-frame atlas compared against a
        /// committed baseline PNG. Catches a packing/placement regression the
        /// rect-level asserts might miss. Regenerate with
        /// `PIXHAUS_UPDATE_SNAPSHOTS=1 cargo test -p pixhaus-core
        /// pack_atlas::two_frame_atlas_matches_baseline` and audit the image.
        #[test]
        fn two_frame_atlas_matches_baseline() {
            use std::path::Path;

            // Two 8x8 frames with distinct content: a red square and a green
            // square, each with a transparent border so the keyed look is
            // visible in the fixture.
            let mut red = frame(8, 8, Rgba::transparent());
            let mut green = frame(8, 8, Rgba::transparent());
            for y in 1..7 {
                for x in 1..7 {
                    red.set_pixel(x, y, Rgba::opaque(220, 40, 40));
                    green.set_pixel(x, y, Rgba::opaque(40, 200, 60));
                }
            }
            let atlas = pack_atlas(
                &[red, green],
                AtlasOptions {
                    layout: AtlasLayout::SingleRow,
                    max_dim: 8192,
                },
            )
            .unwrap();
            let actual: image::RgbaImage = image::RgbaImage::from_raw(atlas.image.width(), atlas.image.height(), atlas.image.as_bytes().to_vec())
                .expect("buffer bytes form a valid RGBA image");

            let baseline_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/pack_atlas_two_frame.png");

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
            assert!(result.score >= 0.999, "packed atlas diverged from baseline: score = {}", result.score);
        }
    }
}
