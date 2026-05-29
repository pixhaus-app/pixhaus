//! Color space conversions between [`Rgba`] and perceptual color spaces.
//!
//! HSV, HSL, and Oklab conversions use the [`palette`] crate to keep the
//! math correct and numerically consistent. OKLCH conversions are a
//! pure-Rust port of Björn Ottosson's published Oklab matrices (see
//! <https://bottosson.github.io/posts/oklab/>), so they carry no extra
//! dependency and clamp to gamut on the way back to `[0, 255]`.

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

// ── OKLCH ──────────────────────────────────────────────────────────────────

// sRGB transfer function (gamma decode). Maps a channel in [0, 1] to linear
// light. Pure-Rust port of `color-utils.ts` `lin`.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

// Inverse sRGB transfer function (gamma encode). Port of `color-utils.ts`
// `delin`.
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

// Converts an `Rgba` to Oklab `(L, a, b)`, ignoring alpha. Björn Ottosson's
// published matrices; see the module docs.
fn rgba_to_oklab(color: Rgba) -> (f32, f32, f32) {
    let rl = srgb_to_linear(f32::from(color.r) / 255.0);
    let gl = srgb_to_linear(f32::from(color.g) / 255.0);
    let bl = srgb_to_linear(f32::from(color.b) / 255.0);
    let l = (0.412_221_47 * rl + 0.536_332_55 * gl + 0.051_445_995 * bl).cbrt();
    let m = (0.211_903_5 * rl + 0.680_699_5 * gl + 0.107_396_96 * bl).cbrt();
    let s = (0.088_302_46 * rl + 0.281_718_85 * gl + 0.629_978_7 * bl).cbrt();
    (
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    )
}

// Converts Oklab `(L, a, b)` back to an `Rgba` with the given `alpha`,
// clamping each channel to gamut. Inverse of [`rgba_to_oklab`].
fn oklab_to_rgba(l: f32, a: f32, b: f32, alpha: u8) -> Rgba {
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    let rl = 4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_;
    let gl = -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_;
    let bl = -0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_;
    Rgba::new(
        f32_to_u8(linear_to_srgb(rl) * 255.0),
        f32_to_u8(linear_to_srgb(gl) * 255.0),
        f32_to_u8(linear_to_srgb(bl) * 255.0),
        alpha,
    )
}

/// Converts `color` to OKLCH and returns `(lightness, chroma, hue_deg)`.
///
/// - Lightness is in `[0.0, 1.0]`.
/// - Chroma is in `[0.0, ~0.4]` for in-gamut sRGB colors.
/// - Hue is in `[0.0, 360.0)`.
/// - Alpha is ignored; use `color.a` directly.
///
/// Mirrors `color-utils.ts` `rgbToOklch`.
pub fn to_oklch(color: Rgba) -> (f32, f32, f32) {
    let (l, a, b) = rgba_to_oklab(color);
    let chroma = (a * a + b * b).sqrt();
    let hue = b.atan2(a).to_degrees().rem_euclid(360.0);
    (l, chroma, hue)
}

/// Constructs an [`Rgba`] from OKLCH components and an explicit `alpha`.
///
/// - `lightness` is expected in `[0.0, 1.0]`.
/// - `chroma` is expected in `[0.0, ~0.4]`.
/// - `hue_deg` is interpreted in degrees.
///
/// The result is clamped to the sRGB gamut on the way back. Mirrors
/// `color-utils.ts` `oklchToRgb`.
pub fn from_oklch(lightness: f32, chroma: f32, hue_deg: f32, alpha: u8) -> Rgba {
    let rad = hue_deg.to_radians();
    oklab_to_rgba(lightness, chroma * rad.cos(), chroma * rad.sin(), alpha)
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
            (i16::from(back.r) - i16::from(c.r)).abs() <= 1,
            "r: {} vs {}",
            back.r,
            c.r
        );
        assert!(
            (i16::from(back.g) - i16::from(c.g)).abs() <= 1,
            "g: {} vs {}",
            back.g,
            c.g
        );
        assert!(
            (i16::from(back.b) - i16::from(c.b)).abs() <= 1,
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
        assert!((i16::from(back.r) - i16::from(c.r)).abs() <= 1);
        assert!((i16::from(back.g) - i16::from(c.g)).abs() <= 1);
        assert!((i16::from(back.b) - i16::from(c.b)).abs() <= 1);
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
        assert!((i16::from(mid.a) - 128).abs() <= 1);
    }

    #[test]
    fn oklch_round_trip() {
        // Round-trip several saturated and neutral colors through OKLCH; the
        // pure-Rust matrices should land within a couple of ULP per channel,
        // matching the HSV/HSL tolerance.
        for c in [
            Rgba::opaque(100, 150, 200),
            Rgba::opaque(255, 0, 0),
            Rgba::opaque(0, 255, 0),
            Rgba::opaque(0, 0, 255),
            Rgba::opaque(200, 200, 200),
            Rgba::new(40, 90, 160, 128),
        ] {
            let (l, chroma, h) = to_oklch(c);
            let back = from_oklch(l, chroma, h, c.a);
            assert!(
                (i16::from(back.r) - i16::from(c.r)).abs() <= 2,
                "r: {} vs {} for {c:?}",
                back.r,
                c.r
            );
            assert!(
                (i16::from(back.g) - i16::from(c.g)).abs() <= 2,
                "g: {} vs {} for {c:?}",
                back.g,
                c.g
            );
            assert!(
                (i16::from(back.b) - i16::from(c.b)).abs() <= 2,
                "b: {} vs {} for {c:?}",
                back.b,
                c.b
            );
            assert_eq!(back.a, c.a);
        }
    }

    #[test]
    fn from_oklch_clamps_to_gamut() {
        // An out-of-gamut chroma must not produce channels outside [0, 255];
        // the cast clamps. (u8 cannot represent out-of-range values, so this
        // asserts the conversion does not panic and stays representable.)
        let c = from_oklch(0.6, 0.9, 30.0, 255);
        // No assertion on exact value — the point is it is a valid Rgba.
        assert_eq!(c.a, 255);
    }
}
