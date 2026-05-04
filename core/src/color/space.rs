//! Color space conversions between [`Rgba`] and perceptual color spaces.
//!
//! Uses the [`palette`] crate for all color math to keep the
//! conversions correct and numerically consistent.

use palette::{Hsl, Hsv, IntoColor, Oklab, ShiftHue, Srgb};

use crate::project::color::Rgba;

// ── Internal helpers ────────────────────────────────────────────────────────

fn rgba_to_srgb(c: Rgba) -> Srgb<f32> {
    Srgb::new(
        f32::from(c.r) / 255.0,
        f32::from(c.g) / 255.0,
        f32::from(c.b) / 255.0,
    )
}

// f32 is clamped to [0.0, 255.0] and rounded before the cast, so
// truncation and sign loss cannot occur.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn f32_to_u8(v: f32) -> u8 {
    v.clamp(0.0, 255.0).round() as u8
}

fn srgb_to_rgba(srgb: Srgb<f32>, alpha: u8) -> Rgba {
    Rgba::new(
        f32_to_u8(srgb.red * 255.0),
        f32_to_u8(srgb.green * 255.0),
        f32_to_u8(srgb.blue * 255.0),
        alpha,
    )
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Converts `color` to HSV and returns `(hue_deg, saturation, value)`.
///
/// - Hue is in `[0.0, 360.0)`.
/// - Saturation and value are in `[0.0, 1.0]`.
/// - Alpha is ignored; use `color.a` directly.
pub fn to_hsv(color: Rgba) -> (f32, f32, f32) {
    let hsv: Hsv = rgba_to_srgb(color).into_color();
    (hsv.hue.into_positive_degrees(), hsv.saturation, hsv.value)
}

/// Constructs an [`Rgba`] from HSV components and an explicit `alpha`.
///
/// - `hue_deg` is reduced modulo 360 automatically.
/// - `saturation` and `value` are clamped to `[0.0, 1.0]`.
pub fn from_hsv(hue_deg: f32, saturation: f32, value: f32, alpha: u8) -> Rgba {
    let hsv = Hsv::new(hue_deg, saturation.clamp(0.0, 1.0), value.clamp(0.0, 1.0));
    let srgb: Srgb<f32> = hsv.into_color();
    srgb_to_rgba(srgb, alpha)
}

/// Converts `color` to HSL and returns `(hue_deg, saturation, lightness)`.
///
/// - Hue is in `[0.0, 360.0)`.
/// - Saturation and lightness are in `[0.0, 1.0]`.
/// - Alpha is ignored.
pub fn to_hsl(color: Rgba) -> (f32, f32, f32) {
    let hsl: Hsl = rgba_to_srgb(color).into_color();
    (
        hsl.hue.into_positive_degrees(),
        hsl.saturation,
        hsl.lightness,
    )
}

/// Constructs an [`Rgba`] from HSL components and an explicit `alpha`.
///
/// - `hue_deg` is reduced modulo 360 automatically.
/// - `saturation` and `lightness` are clamped to `[0.0, 1.0]`.
pub fn from_hsl(hue_deg: f32, saturation: f32, lightness: f32, alpha: u8) -> Rgba {
    let hsl = Hsl::new(
        hue_deg,
        saturation.clamp(0.0, 1.0),
        lightness.clamp(0.0, 1.0),
    );
    let srgb: Srgb<f32> = hsl.into_color();
    srgb_to_rgba(srgb, alpha)
}

/// Rotates the hue of `color` by `degrees` and returns the result.
///
/// Uses HSV so saturation and value are preserved. Alpha is passed through.
pub fn rotate_hue(color: Rgba, degrees: f32) -> Rgba {
    let hsv: Hsv = rgba_to_srgb(color).into_color();
    let rotated = hsv.shift_hue(degrees);
    let out: Srgb<f32> = rotated.into_color();
    srgb_to_rgba(out, color.a)
}

/// Interpolates between `a` and `b` by `t` in Oklab space.
///
/// `t = 0.0` returns `a`, `t = 1.0` returns `b`. Alpha is interpolated
/// linearly in sRGB space. Clamping is applied before conversion back to
/// `[0, 255]`.
pub fn oklab_mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    let a_lab: Oklab = rgba_to_srgb(a).into_color();
    let b_lab: Oklab = rgba_to_srgb(b).into_color();
    let mixed = Oklab::new(
        a_lab.l + (b_lab.l - a_lab.l) * t,
        a_lab.a + (b_lab.a - a_lab.a) * t,
        a_lab.b + (b_lab.b - a_lab.b) * t,
    );
    let srgb: Srgb<f32> = mixed.into_color();
    let alpha = f32_to_u8(f32::from(a.a) * (1.0 - t) + f32::from(b.a) * t);
    srgb_to_rgba(srgb, alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn red() -> Rgba {
        Rgba::opaque(255, 0, 0)
    }

    fn white() -> Rgba {
        Rgba::opaque(255, 255, 255)
    }

    fn black() -> Rgba {
        Rgba::opaque(0, 0, 0)
    }

    #[test]
    fn red_has_zero_hue() {
        let (h, s, v) = to_hsv(red());
        assert!((h - 0.0).abs() < 1.0, "hue={h}");
        assert!(s > 0.9);
        assert!(v > 0.9);
    }

    #[test]
    fn hsv_round_trip() {
        let c = Rgba::opaque(100, 150, 200);
        let (h, s, v) = to_hsv(c);
        let back = from_hsv(h, s, v, c.a);
        // Allow 1 ULP rounding on each channel
        assert!(
            (back.r as i16 - c.r as i16).abs() <= 1,
            "r: {} vs {}",
            back.r,
            c.r
        );
        assert!(
            (back.g as i16 - c.g as i16).abs() <= 1,
            "g: {} vs {}",
            back.g,
            c.g
        );
        assert!(
            (back.b as i16 - c.b as i16).abs() <= 1,
            "b: {} vs {}",
            back.b,
            c.b
        );
    }

    #[test]
    fn hsl_round_trip() {
        let c = Rgba::opaque(80, 120, 200);
        let (h, s, l) = to_hsl(c);
        let back = from_hsl(h, s, l, c.a);
        assert!((back.r as i16 - c.r as i16).abs() <= 1);
        assert!((back.g as i16 - c.g as i16).abs() <= 1);
        assert!((back.b as i16 - c.b as i16).abs() <= 1);
    }

    #[test]
    fn complement_rotates_180() {
        let c = red();
        let comp = rotate_hue(c, 180.0);
        // Red complement is cyan (0, 255, 255)
        assert!(comp.r < 10, "r={}", comp.r);
        assert!(comp.g > 240, "g={}", comp.g);
        assert!(comp.b > 240, "b={}", comp.b);
    }

    #[test]
    fn rotate_hue_preserves_alpha() {
        let c = Rgba::new(255, 0, 0, 128);
        let rotated = rotate_hue(c, 90.0);
        assert_eq!(rotated.a, 128);
    }

    #[test]
    fn oklab_mix_midpoint() {
        let a = black();
        let b = white();
        let mid = oklab_mix(a, b, 0.5);
        // Midpoint in Oklab should be perceptually neutral gray — typically ~95-130
        assert!(mid.r >= 90 && mid.r < 160, "r={}", mid.r);
        assert_eq!(mid.r, mid.g);
        assert_eq!(mid.g, mid.b);
    }

    #[test]
    fn oklab_mix_t0_returns_a() {
        let a = red();
        let b = Rgba::opaque(0, 0, 255);
        let result = oklab_mix(a, b, 0.0);
        assert_eq!(result, a);
    }

    #[test]
    fn oklab_mix_t1_returns_b() {
        let a = red();
        let b = Rgba::opaque(0, 0, 255);
        let result = oklab_mix(a, b, 1.0);
        assert_eq!(result, b);
    }

    #[test]
    fn oklab_mix_interpolates_alpha() {
        let a = Rgba::new(255, 0, 0, 0);
        let b = Rgba::new(0, 0, 255, 255);
        let mid = oklab_mix(a, b, 0.5);
        assert!((mid.a as i16 - 128).abs() <= 1);
    }
}
