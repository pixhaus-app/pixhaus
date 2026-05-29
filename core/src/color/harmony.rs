//! Color harmony tools.
//!
//! Each function takes a base [`Rgba`] and returns one or more
//! harmonically related colors. Hue rotation is done in HSV space;
//! lightness variation uses HSL. Alpha is always passed through from
//! the input.

use crate::project::color::Rgba;

use super::space::{from_hsl, rotate_hue, to_hsl};

/// Returns the complementary color (hue rotated 180°).
pub fn complement(color: Rgba) -> Rgba {
    rotate_hue(color, 180.0)
}

/// Returns the two split-complementary colors (hue + 150° and hue + 210°).
pub fn split_complement(color: Rgba) -> [Rgba; 2] {
    [rotate_hue(color, 150.0), rotate_hue(color, 210.0)]
}

/// Returns the two triadic colors (hue + 120° and hue + 240°).
pub fn triad(color: Rgba) -> [Rgba; 2] {
    [rotate_hue(color, 120.0), rotate_hue(color, 240.0)]
}

/// Returns the three tetradic (square) colors (hue + 90°, 180°, 270°).
pub fn tetrad(color: Rgba) -> [Rgba; 3] {
    [rotate_hue(color, 90.0), rotate_hue(color, 180.0), rotate_hue(color, 270.0)]
}

/// Returns `count` analogous colors centered on `color`.
///
/// The output includes `color` itself. With `count = 5` and
/// `step_deg = 30`, the result covers hues at -60°, -30°, 0°, +30°,
/// +60° relative to the input hue.
///
/// Returns an empty `Vec` if `count == 0`.
pub fn analogous(color: Rgba, count: usize, step_deg: f32) -> Vec<Rgba> {
    if count == 0 {
        return Vec::new();
    }
    // count and i are small (palette-scale), precision loss is not a concern
    #[allow(clippy::cast_precision_loss)]
    let half = (count as f32 - 1.0) / 2.0;
    (0..count)
        .map(|i| {
            #[allow(clippy::cast_precision_loss)]
            let offset = (i as f32 - half) * step_deg;
            rotate_hue(color, offset)
        })
        .collect()
}

/// Returns `steps` monochromatic colors: same hue and saturation as
/// `color`, with lightness evenly spaced from `0.1` to `0.9`.
///
/// Returns an empty `Vec` if `steps == 0`.
pub fn monochromatic(color: Rgba, steps: usize) -> Vec<Rgba> {
    if steps == 0 {
        return Vec::new();
    }
    let (h, s, _l) = to_hsl(color);
    if steps == 1 {
        return vec![from_hsl(h, s, 0.5, color.a)];
    }
    (0..steps)
        .map(|i| {
            #[allow(clippy::cast_precision_loss)]
            let lightness = 0.1 + 0.8 * (i as f32 / (steps - 1) as f32);
            from_hsl(h, s, lightness, color.a)
        })
        .collect()
}

/// Returns a 5-color analogous palette using the standard ±30° step.
///
/// Convenience wrapper around [`analogous`].
pub fn analogous_standard(color: Rgba) -> [Rgba; 5] {
    let v = analogous(color, 5, 30.0);
    [v[0], v[1], v[2], v[3], v[4]]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn red() -> Rgba {
        Rgba::opaque(255, 0, 0)
    }

    #[test]
    fn complement_of_red_is_cyan() {
        let c = complement(red());
        assert!(c.r < 10, "r={}", c.r);
        assert!(c.g > 240, "g={}", c.g);
        assert!(c.b > 240, "b={}", c.b);
    }

    #[test]
    fn split_complement_returns_two_colors() {
        let sc = split_complement(red());
        assert_ne!(sc[0], red());
        assert_ne!(sc[1], red());
        assert_ne!(sc[0], sc[1]);
    }

    #[test]
    fn triad_returns_two_colors() {
        let t = triad(red());
        assert_ne!(t[0], red());
        assert_ne!(t[1], red());
    }

    #[test]
    fn tetrad_returns_three_colors() {
        let sq = tetrad(red());
        assert_ne!(sq[0], red());
        assert_ne!(sq[1], red());
        assert_ne!(sq[2], red());
    }

    #[test]
    fn analogous_center_matches_input() {
        let a = analogous(red(), 5, 30.0);
        // Middle element (index 2) should be the original color
        assert_eq!(a.len(), 5);
        assert_eq!(a[2], red());
    }

    #[test]
    fn analogous_zero_count_is_empty() {
        assert!(analogous(red(), 0, 30.0).is_empty());
    }

    #[test]
    fn analogous_one_returns_input() {
        let a = analogous(red(), 1, 30.0);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0], red());
    }

    #[test]
    fn monochromatic_preserves_alpha() {
        let c = Rgba::new(255, 0, 0, 200);
        for color in monochromatic(c, 5) {
            assert_eq!(color.a, 200);
        }
    }

    #[test]
    fn monochromatic_zero_steps_is_empty() {
        assert!(monochromatic(red(), 0).is_empty());
    }

    #[test]
    fn monochromatic_steps_vary_lightness() {
        let steps = monochromatic(red(), 5);
        assert_eq!(steps.len(), 5);
        // First step should be darker than last step
        let first_brightness = u32::from(steps[0].r) + u32::from(steps[0].g) + u32::from(steps[0].b);
        let last_brightness = u32::from(steps[4].r) + u32::from(steps[4].g) + u32::from(steps[4].b);
        assert!(first_brightness < last_brightness, "expected lighter last step");
    }

    #[test]
    fn from_hsv_round_trip_preserves_hue() {
        // Test that rotate_hue is internally consistent
        let c = red();
        let rotated = rotate_hue(c, 360.0);
        assert!((i16::from(rotated.r) - i16::from(c.r)).abs() <= 2);
        assert!((i16::from(rotated.g) - i16::from(c.g)).abs() <= 2);
        assert!((i16::from(rotated.b) - i16::from(c.b)).abs() <= 2);
    }
}
