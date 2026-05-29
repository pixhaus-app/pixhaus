//! Dither pattern masks for the dithering brush.
//!
//! A dither pattern is an on/off mask over canvas-absolute integer
//! coordinates: [`dither_allows`] answers "may the brush write this
//! pixel?" for each footprint pixel. Because the mask is keyed on the
//! canvas coordinate rather than the stroke origin, the pattern stays
//! fixed under pan and zoom — the same checker square always lands on
//! the same canvas pixel.
//!
//! This is the single source of truth shared by the interactive brush
//! (the shell's stamp gate, via [`super::paint_brush_masked`]) and any
//! future export-side dither. v1 is a binary mask of a single foreground
//! colour; there is no second-colour blend here.

/// Which footprint pixels a dithering brush is allowed to write.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum DitherPattern {
    /// No mask — every covered pixel is written (solid brush).
    #[default]
    None,
    /// 2x2 checkerboard: write when `(x + y)` is even.
    Checker,
    /// Ordered 2x2 Bayer matrix thresholded at the mid value.
    Bayer2x2,
    /// Ordered 4x4 Bayer matrix thresholded at the mid value.
    Bayer4x4,
}

/// The ordered 2x2 Bayer matrix, values `0..=3`.
const BAYER_2X2: [[u8; 2]; 2] = [[0, 2], [3, 1]];

/// The ordered 4x4 Bayer matrix, values `0..=15`.
const BAYER_4X4: [[u8; 4]; 4] = [
    [0, 8, 2, 10],
    [12, 4, 14, 6],
    [3, 11, 1, 9],
    [15, 7, 13, 5],
];

/// Whether the dither `pattern` permits a write at canvas pixel `(x, y)`.
///
/// Coordinates are canvas-absolute so the pattern is pan-stable. The
/// Bayer variants compare each cell's matrix value against a fixed mid
/// threshold, yielding a deterministic on/off mask with a period equal
/// to the matrix size (2 for `Bayer2x2`, 4 for `Bayer4x4`). `Checker`
/// has period 2; `None` always allows.
#[must_use]
pub fn dither_allows(pattern: DitherPattern, x: i32, y: i32) -> bool {
    match pattern {
        DitherPattern::None => true,
        DitherPattern::Checker => (x + y).rem_euclid(2) == 0,
        DitherPattern::Bayer2x2 => {
            #[allow(clippy::cast_sign_loss)] // rem_euclid(2) is in 0..2.
            let cx = x.rem_euclid(2) as usize;
            #[allow(clippy::cast_sign_loss)]
            let cy = y.rem_euclid(2) as usize;
            // Mid threshold of 2 over values 0..=3: cells below 2 pass,
            // leaving the two-on / two-off ordered pattern.
            BAYER_2X2[cy][cx] < 2
        }
        DitherPattern::Bayer4x4 => {
            #[allow(clippy::cast_sign_loss)] // rem_euclid(4) is in 0..4.
            let cx = x.rem_euclid(4) as usize;
            #[allow(clippy::cast_sign_loss)]
            let cy = y.rem_euclid(4) as usize;
            // Mid threshold of 8 over values 0..=15: half the cells pass.
            BAYER_4X4[cy][cx] < 8
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn none_always_allows() {
        for y in -3..3 {
            for x in -3..3 {
                assert!(dither_allows(DitherPattern::None, x, y));
            }
        }
    }

    #[test]
    fn checker_matches_parity() {
        // Allowed exactly when (x + y) is even, including negatives.
        for y in -4i32..4 {
            for x in -4i32..4 {
                let expected = (x + y).rem_euclid(2) == 0;
                assert_eq!(dither_allows(DitherPattern::Checker, x, y), expected, "checker at ({x}, {y})");
            }
        }
        // Spot the canonical 2x2 tile at the origin.
        assert!(dither_allows(DitherPattern::Checker, 0, 0));
        assert!(!dither_allows(DitherPattern::Checker, 1, 0));
        assert!(!dither_allows(DitherPattern::Checker, 0, 1));
        assert!(dither_allows(DitherPattern::Checker, 1, 1));
    }

    #[test]
    fn bayer_half_coverage_per_tile() {
        // Each Bayer tile passes exactly half its cells (mid threshold).
        let count_2x2 = (0..2).flat_map(|y| (0..2).map(move |x| (x, y))).filter(|&(x, y)| dither_allows(DitherPattern::Bayer2x2, x, y)).count();
        assert_eq!(count_2x2, 2, "Bayer2x2 should pass 2 of 4 cells");

        let count_4x4 = (0..4).flat_map(|y| (0..4).map(move |x| (x, y))).filter(|&(x, y)| dither_allows(DitherPattern::Bayer4x4, x, y)).count();
        assert_eq!(count_4x4, 8, "Bayer4x4 should pass 8 of 16 cells");
    }

    #[test]
    fn bayer_is_deterministic() {
        // Same coordinate, same answer, every call.
        for pattern in [DitherPattern::Bayer2x2, DitherPattern::Bayer4x4] {
            for y in -8..8 {
                for x in -8..8 {
                    assert_eq!(dither_allows(pattern, x, y), dither_allows(pattern, x, y));
                }
            }
        }
    }

    proptest! {
        /// Bayer patterns are stable under translation by the pattern period:
        /// shifting a coordinate by a whole tile in either axis must not
        /// change the mask. This is what makes the pattern pan-stable.
        #[test]
        fn bayer_stable_under_period_translation(x in -10_000i32..10_000, y in -10_000i32..10_000, kx in -50i32..50, ky in -50i32..50) {
            for (pattern, period) in [(DitherPattern::Bayer2x2, 2), (DitherPattern::Bayer4x4, 4)] {
                let shifted_x = x + kx * period;
                let shifted_y = y + ky * period;
                prop_assert_eq!(dither_allows(pattern, x, y), dither_allows(pattern, shifted_x, shifted_y), "{:?} not stable under period {} shift", pattern, period);
            }
            // Checker has period 2 too.
            let cx = x + kx * 2;
            let cy = y + ky * 2;
            prop_assert_eq!(dither_allows(DitherPattern::Checker, x, y), dither_allows(DitherPattern::Checker, cx, cy));
        }
    }
}
