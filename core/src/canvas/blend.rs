//! Aseprite-compatible blend mode math.
//!
//! Every function in this module is a direct translation of the
//! corresponding routine in Aseprite's `src/doc/blend_funcs.cpp`. The
//! per-channel separable modes use Aseprite's `MUL_UN8`/`DIV_UN8`
//! macros, reproduced here as `mul_un8` / `div_un8`, so 8-bit
//! results round-trip byte-for-byte against an Aseprite-rendered
//! reference. The HSL non-separable modes follow the W3C "Compositing
//! and Blending Level 1" reference algorithm in `f64`, matching
//! Aseprite's own implementation.
//!
//! The public entry point is [`blend`]: pick a [`BlendMode`], pass
//! source and backdrop colors plus an opacity multiplier, and receive
//! the composited result.

use crate::project::{BlendMode, Rgba};

/// Multiplies two 8-bit values and divides by 255 with rounding.
///
/// `(a * b + 127) / 255` computed without a 32-bit divide. Equivalent
/// to Aseprite's `MUL_UN8` macro.
#[inline]
#[must_use]
pub const fn mul_un8(a: u8, b: u8) -> u8 {
    let t = (a as u32) * (b as u32) + 0x80;
    let result = ((t >> 8) + t) >> 8;
    // Bounds: max(t) = 255 * 255 + 0x80 = 65_153; the algorithm guarantees
    // the final `result` fits in 8 bits — Aseprite's blend_funcs relies on
    // that invariant byte-for-byte.
    #[allow(clippy::cast_possible_truncation)]
    {
        result as u8
    }
}

/// Computes `(a * 255 + b/2) / b` for `b > 0`. Equivalent to
/// Aseprite's `DIV_UN8` macro.
#[inline]
#[must_use]
const fn div_un8(a: u8, b: u8) -> u8 {
    debug_assert!(b > 0);
    let num = (a as u32) * 0xff + ((b as u32) / 2);
    let result = num / (b as u32);
    // Bounds: `num <= 255*255 + 127 = 65_152`; dividing by `b >= 1`
    // leaves the result in `0..=255`.
    #[allow(clippy::cast_possible_truncation)]
    {
        result as u8
    }
}

/// Per-channel multiply.
#[inline]
#[must_use]
pub const fn channel_multiply(b: u8, s: u8) -> u8 {
    mul_un8(b, s)
}

/// Per-channel screen.
#[inline]
#[must_use]
pub fn channel_screen(b: u8, s: u8) -> u8 {
    // u16 math: `b + s` can exceed 255, but `b + s - mul_un8(b, s)`
    // always fits in `u8`. `saturating_add` would lose precision in the
    // intermediate, so screen against `(255, 255)` would collapse to 0.
    let sum = u16::from(b) + u16::from(s) - u16::from(mul_un8(b, s));
    u8::try_from(sum).unwrap_or(255)
}

/// Per-channel darken (`min`).
#[inline]
#[must_use]
pub const fn channel_darken(b: u8, s: u8) -> u8 {
    if b < s { b } else { s }
}

/// Per-channel lighten (`max`).
#[inline]
#[must_use]
pub const fn channel_lighten(b: u8, s: u8) -> u8 {
    if b > s { b } else { s }
}

/// Per-channel color-dodge.
#[inline]
#[must_use]
pub const fn channel_color_dodge(b: u8, s: u8) -> u8 {
    if b == 0 {
        return 0;
    }
    let inv_s = 255 - s;
    if b >= inv_s {
        return 255;
    }
    div_un8(b, inv_s)
}

/// Per-channel color-burn.
#[inline]
#[must_use]
pub const fn channel_color_burn(b: u8, s: u8) -> u8 {
    if b == 255 {
        return 255;
    }
    let inv_b = 255 - b;
    if inv_b >= s {
        return 0;
    }
    255 - div_un8(inv_b, s)
}

/// Per-channel addition (saturating add).
#[inline]
#[must_use]
pub const fn channel_addition(b: u8, s: u8) -> u8 {
    b.saturating_add(s)
}

/// Per-channel hard light.
#[inline]
#[must_use]
pub fn channel_hard_light(b: u8, s: u8) -> u8 {
    if s < 128 {
        // multiply(b, 2*s); 2*s fits in u8 because s < 128.
        let s2 = s.wrapping_mul(2);
        mul_un8(b, s2)
    } else {
        // screen(b, 2*s - 255); 2*s in 256..=510 minus 255 fits in u8.
        let s2 = u16::from(s) * 2 - 255;
        channel_screen(b, u8::try_from(s2).unwrap_or(255))
    }
}

/// Per-channel overlay = `hard_light(src, dst)`.
#[inline]
#[must_use]
pub fn channel_overlay(b: u8, s: u8) -> u8 {
    channel_hard_light(s, b)
}

/// Per-channel soft light, W3C / Aseprite f64 reference.
#[inline]
#[must_use]
pub fn channel_soft_light(b: u8, s: u8) -> u8 {
    let b = f64::from(b) / 255.0;
    let s = f64::from(s) / 255.0;

    let d = if b <= 0.25 {
        ((16.0 * b - 12.0) * b + 4.0) * b
    } else {
        b.sqrt()
    };

    let r = if s <= 0.5 {
        b - (1.0 - 2.0 * s) * b * (1.0 - b)
    } else {
        b + (2.0 * s - 1.0) * (d - b)
    };

    quantize_unit(r)
}

/// Per-channel difference.
#[inline]
#[must_use]
pub const fn channel_difference(b: u8, s: u8) -> u8 {
    b.abs_diff(s)
}

/// Per-channel exclusion.
#[inline]
#[must_use]
pub fn channel_exclusion(b: u8, s: u8) -> u8 {
    let m = mul_un8(b, s);
    let sum = u16::from(b) + u16::from(s) - 2 * u16::from(m);
    // sum <= 510 - 0 = 510; algebraic max under valid u8 inputs is 255.
    u8::try_from(sum).unwrap_or(255)
}

/// Per-channel subtract.
#[inline]
#[must_use]
pub const fn channel_subtract(b: u8, s: u8) -> u8 {
    b.saturating_sub(s)
}

/// Per-channel divide.
#[inline]
#[must_use]
pub const fn channel_divide(b: u8, s: u8) -> u8 {
    if b == 0 {
        return 0;
    }
    if b >= s {
        return 255;
    }
    div_un8(b, s)
}

// -- HSL non-separable helpers (W3C compositing-1 reference) --------

#[inline]
fn lum_f64(red: f64, grn: f64, blu: f64) -> f64 {
    0.3 * red + 0.59 * grn + 0.11 * blu
}

#[inline]
fn sat_f64(red: f64, grn: f64, blu: f64) -> f64 {
    red.max(grn).max(blu) - red.min(grn).min(blu)
}

fn clip_color(red: &mut f64, grn: &mut f64, blu: &mut f64) {
    let lum = lum_f64(*red, *grn, *blu);
    let mn = red.min(*grn).min(*blu);
    let mx = red.max(*grn).max(*blu);

    if mn < 0.0 {
        let denom = lum - mn;
        if denom != 0.0 {
            *red = lum + ((*red - lum) * lum) / denom;
            *grn = lum + ((*grn - lum) * lum) / denom;
            *blu = lum + ((*blu - lum) * lum) / denom;
        }
    }

    if mx > 1.0 {
        let denom = mx - lum;
        if denom != 0.0 {
            *red = lum + ((*red - lum) * (1.0 - lum)) / denom;
            *grn = lum + ((*grn - lum) * (1.0 - lum)) / denom;
            *blu = lum + ((*blu - lum) * (1.0 - lum)) / denom;
        }
    }
}

fn set_lum(red: &mut f64, grn: &mut f64, blu: &mut f64, target: f64) {
    let delta = target - lum_f64(*red, *grn, *blu);
    *red += delta;
    *grn += delta;
    *blu += delta;
    clip_color(red, grn, blu);
}

#[allow(clippy::float_cmp)]
fn set_sat(red: &mut f64, grn: &mut f64, blu: &mut f64, target_sat: f64) {
    // Identify the channel ordering. Tie-breaks follow Aseprite's
    // chained `if` pattern: a min-tie resolves to the first matching
    // channel, and the result is identical to Aseprite for any input
    // that arose from u8 quantization.
    let mn = red.min(*grn).min(*blu);
    let mx = red.max(*grn).max(*blu);

    if mx == mn {
        *red = 0.0;
        *grn = 0.0;
        *blu = 0.0;
        return;
    }

    let span = mx - mn;
    let scale = |v: f64| (v - mn) * target_sat / span;

    if *red == mn {
        if *grn == mx {
            *blu = scale(*blu);
            *grn = target_sat;
            *red = 0.0;
        } else {
            *grn = scale(*grn);
            *blu = target_sat;
            *red = 0.0;
        }
    } else if *grn == mn {
        if *red == mx {
            *blu = scale(*blu);
            *red = target_sat;
            *grn = 0.0;
        } else {
            *red = scale(*red);
            *blu = target_sat;
            *grn = 0.0;
        }
    } else if *red == mx {
        *grn = scale(*grn);
        *red = target_sat;
        *blu = 0.0;
    } else {
        *red = scale(*red);
        *grn = target_sat;
        *blu = 0.0;
    }
}

#[inline]
fn to_unit(c: u8) -> f64 {
    f64::from(c) / 255.0
}

#[inline]
fn quantize_unit(c: f64) -> u8 {
    let scaled = c * 255.0 + 0.5;
    let clamped = scaled.clamp(0.0, 255.0);
    // `clamped` is in `[0, 255]` and finite, so the cast is exact.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        clamped as u8
    }
}

/// Hue blend: `SetLum(SetSat(src, sat(dst)), lum(dst))`.
#[must_use]
pub fn channels_hue(b_rgb: [u8; 3], s_rgb: [u8; 3]) -> [u8; 3] {
    let (br, bg, bb) = (to_unit(b_rgb[0]), to_unit(b_rgb[1]), to_unit(b_rgb[2]));
    let (mut sr, mut sg, mut sb) = (to_unit(s_rgb[0]), to_unit(s_rgb[1]), to_unit(s_rgb[2]));
    set_sat(&mut sr, &mut sg, &mut sb, sat_f64(br, bg, bb));
    set_lum(&mut sr, &mut sg, &mut sb, lum_f64(br, bg, bb));
    [quantize_unit(sr), quantize_unit(sg), quantize_unit(sb)]
}

/// Saturation blend: `SetLum(SetSat(dst, sat(src)), lum(dst))`.
#[must_use]
pub fn channels_saturation(b_rgb: [u8; 3], s_rgb: [u8; 3]) -> [u8; 3] {
    let (mut br, mut bg, mut bb) = (to_unit(b_rgb[0]), to_unit(b_rgb[1]), to_unit(b_rgb[2]));
    let (sr, sg, sb) = (to_unit(s_rgb[0]), to_unit(s_rgb[1]), to_unit(s_rgb[2]));
    let backup_lum = lum_f64(br, bg, bb);
    set_sat(&mut br, &mut bg, &mut bb, sat_f64(sr, sg, sb));
    set_lum(&mut br, &mut bg, &mut bb, backup_lum);
    [quantize_unit(br), quantize_unit(bg), quantize_unit(bb)]
}

/// Color blend: `SetLum(src, lum(dst))`.
#[must_use]
pub fn channels_color(b_rgb: [u8; 3], s_rgb: [u8; 3]) -> [u8; 3] {
    let (br, bg, bb) = (to_unit(b_rgb[0]), to_unit(b_rgb[1]), to_unit(b_rgb[2]));
    let (mut sr, mut sg, mut sb) = (to_unit(s_rgb[0]), to_unit(s_rgb[1]), to_unit(s_rgb[2]));
    set_lum(&mut sr, &mut sg, &mut sb, lum_f64(br, bg, bb));
    [quantize_unit(sr), quantize_unit(sg), quantize_unit(sb)]
}

/// Luminosity blend: `SetLum(dst, lum(src))`.
#[must_use]
pub fn channels_luminosity(b_rgb: [u8; 3], s_rgb: [u8; 3]) -> [u8; 3] {
    let (mut br, mut bg, mut bb) = (to_unit(b_rgb[0]), to_unit(b_rgb[1]), to_unit(b_rgb[2]));
    let (sr, sg, sb) = (to_unit(s_rgb[0]), to_unit(s_rgb[1]), to_unit(s_rgb[2]));
    set_lum(&mut br, &mut bg, &mut bb, lum_f64(sr, sg, sb));
    [quantize_unit(br), quantize_unit(bg), quantize_unit(bb)]
}

/// Replaces `src.rgb` with `mode(backdrop.rgb, src.rgb)` while keeping
/// `src.a`. This is Aseprite's "premix" step: the alpha composite
/// then runs in [`blend_normal`] using the modified source.
#[must_use]
pub fn premix(mode: BlendMode, src: Rgba, dst: Rgba) -> Rgba {
    let s_rgb = [src.r, src.g, src.b];
    let d_rgb = [dst.r, dst.g, dst.b];

    let mixed: [u8; 3] = match mode {
        BlendMode::Normal => return src,
        BlendMode::Darken => [
            channel_darken(d_rgb[0], s_rgb[0]),
            channel_darken(d_rgb[1], s_rgb[1]),
            channel_darken(d_rgb[2], s_rgb[2]),
        ],
        BlendMode::Multiply => [
            channel_multiply(d_rgb[0], s_rgb[0]),
            channel_multiply(d_rgb[1], s_rgb[1]),
            channel_multiply(d_rgb[2], s_rgb[2]),
        ],
        BlendMode::ColorBurn => [
            channel_color_burn(d_rgb[0], s_rgb[0]),
            channel_color_burn(d_rgb[1], s_rgb[1]),
            channel_color_burn(d_rgb[2], s_rgb[2]),
        ],
        BlendMode::Lighten => [
            channel_lighten(d_rgb[0], s_rgb[0]),
            channel_lighten(d_rgb[1], s_rgb[1]),
            channel_lighten(d_rgb[2], s_rgb[2]),
        ],
        BlendMode::Screen => [
            channel_screen(d_rgb[0], s_rgb[0]),
            channel_screen(d_rgb[1], s_rgb[1]),
            channel_screen(d_rgb[2], s_rgb[2]),
        ],
        BlendMode::ColorDodge => [
            channel_color_dodge(d_rgb[0], s_rgb[0]),
            channel_color_dodge(d_rgb[1], s_rgb[1]),
            channel_color_dodge(d_rgb[2], s_rgb[2]),
        ],
        BlendMode::Addition => [
            channel_addition(d_rgb[0], s_rgb[0]),
            channel_addition(d_rgb[1], s_rgb[1]),
            channel_addition(d_rgb[2], s_rgb[2]),
        ],
        BlendMode::Overlay => [
            channel_overlay(d_rgb[0], s_rgb[0]),
            channel_overlay(d_rgb[1], s_rgb[1]),
            channel_overlay(d_rgb[2], s_rgb[2]),
        ],
        BlendMode::SoftLight => [
            channel_soft_light(d_rgb[0], s_rgb[0]),
            channel_soft_light(d_rgb[1], s_rgb[1]),
            channel_soft_light(d_rgb[2], s_rgb[2]),
        ],
        BlendMode::HardLight => [
            channel_hard_light(d_rgb[0], s_rgb[0]),
            channel_hard_light(d_rgb[1], s_rgb[1]),
            channel_hard_light(d_rgb[2], s_rgb[2]),
        ],
        BlendMode::Difference => [
            channel_difference(d_rgb[0], s_rgb[0]),
            channel_difference(d_rgb[1], s_rgb[1]),
            channel_difference(d_rgb[2], s_rgb[2]),
        ],
        BlendMode::Exclusion => [
            channel_exclusion(d_rgb[0], s_rgb[0]),
            channel_exclusion(d_rgb[1], s_rgb[1]),
            channel_exclusion(d_rgb[2], s_rgb[2]),
        ],
        BlendMode::Subtract => [
            channel_subtract(d_rgb[0], s_rgb[0]),
            channel_subtract(d_rgb[1], s_rgb[1]),
            channel_subtract(d_rgb[2], s_rgb[2]),
        ],
        BlendMode::Divide => [
            channel_divide(d_rgb[0], s_rgb[0]),
            channel_divide(d_rgb[1], s_rgb[1]),
            channel_divide(d_rgb[2], s_rgb[2]),
        ],
        BlendMode::Hue => channels_hue(d_rgb, s_rgb),
        BlendMode::Saturation => channels_saturation(d_rgb, s_rgb),
        BlendMode::Color => channels_color(d_rgb, s_rgb),
        BlendMode::Luminosity => channels_luminosity(d_rgb, s_rgb),
    };

    Rgba::new(mixed[0], mixed[1], mixed[2], src.a)
}

/// Standard "src over dst" alpha composite, matching Aseprite's
/// `rgba_blender_normal`. `opacity` is the per-layer multiplier in
/// `0..=255`.
#[must_use]
pub fn blend_normal(src: Rgba, dst: Rgba, opacity: u8) -> Rgba {
    let s_a = mul_un8(src.a, opacity);

    if dst.a == 0 {
        // Backdrop fully transparent — emit the modulated source.
        return Rgba::new(src.r, src.g, src.b, s_a);
    }
    if s_a == 0 {
        return dst;
    }

    let d_a = dst.a;
    // r_a = s_a + d_a - mul_un8(d_a, s_a); always fits in u8 because the
    // formula is the standard "src over dst" alpha and never exceeds 255.
    let mixed_term = mul_un8(d_a, s_a);
    let r_a_wide = u16::from(s_a) + u16::from(d_a) - u16::from(mixed_term);
    if r_a_wide == 0 {
        return Rgba::transparent();
    }
    let r_a = u8::try_from(r_a_wide).unwrap_or(255);

    // Aseprite formula:  Rc = Bc + (Sc - Bc) * Sa / Ra
    let mix = |b: u8, s: u8| -> u8 {
        let backdrop = i32::from(b);
        let source = i32::from(s);
        let v = backdrop + (source - backdrop) * i32::from(s_a) / i32::from(r_a);
        // The composite formula keeps the result in [0, 255] for any
        // valid u8 inputs; clamp defensively against rounding drift.
        let clamped = v.clamp(0, 255);
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        {
            clamped as u8
        }
    };

    Rgba::new(mix(dst.r, src.r), mix(dst.g, src.g), mix(dst.b, src.b), r_a)
}

/// Composite `src` over `dst` using `mode` and a per-source opacity.
///
/// This is the single function the compositor and per-pixel callers
/// hit. For [`BlendMode::Normal`], the result is plain alpha
/// compositing. For every other mode, the source RGB is replaced with
/// the mode-specific blend of `(dst.rgb, src.rgb)` first, and the
/// alpha composite then runs as usual — matching Aseprite's
/// `rgba_blender_*` family.
#[must_use]
pub fn blend(mode: BlendMode, src: Rgba, dst: Rgba, opacity: u8) -> Rgba {
    let modulated = premix(mode, src, dst);
    blend_normal(modulated, dst, opacity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn mul_un8_matches_division() {
        for a in 0u8..=255 {
            for b in 0u8..=255 {
                let want = (u32::from(a) * u32::from(b) + 127) / 255;
                assert_eq!(u32::from(mul_un8(a, b)), want, "{a}*{b}");
            }
        }
    }

    #[test]
    fn div_un8_for_a_le_b_stays_in_range() {
        for b in 1u8..=255 {
            for a in 0u8..=b {
                let v = div_un8(a, b);
                let want = (u32::from(a) * 255 + u32::from(b) / 2) / u32::from(b);
                assert_eq!(u32::from(v), want);
            }
        }
    }

    #[test]
    fn normal_opaque_src_replaces_dst() {
        let src = Rgba::new(200, 50, 25, 255);
        let dst = Rgba::opaque(0, 0, 0);
        assert_eq!(blend_normal(src, dst, 255), src);
    }

    #[test]
    fn normal_transparent_src_keeps_dst() {
        let src = Rgba::transparent();
        let dst = Rgba::opaque(10, 20, 30);
        assert_eq!(blend_normal(src, dst, 255), dst);
    }

    #[test]
    fn normal_against_transparent_dst_modulates_alpha() {
        let src = Rgba::new(255, 128, 0, 200);
        let result = blend_normal(src, Rgba::transparent(), 128);
        // Sa = 200 * 128 / 255 ≈ 100 (rounded)
        assert_eq!(result.r, 255);
        assert_eq!(result.g, 128);
        assert_eq!(result.b, 0);
        assert_eq!(result.a, mul_un8(200, 128));
    }

    #[rstest]
    // Multiply: 255*255 -> 255, 0*255 -> 0, 128*128 -> ~64
    #[case(BlendMode::Multiply, [255, 128, 0], [255, 128, 0], [255, 64, 0])]
    // Screen: 255+255-255*255/255=255, 128+128-64=192, 0+0=0
    #[case(BlendMode::Screen, [255, 128, 0], [255, 128, 0], [255, 192, 0])]
    // Darken: min
    #[case(BlendMode::Darken, [200, 100, 50], [50, 100, 200], [50, 100, 50])]
    // Lighten: max
    #[case(BlendMode::Lighten, [200, 100, 50], [50, 100, 200], [200, 100, 200])]
    // Difference: |b-s|
    #[case(BlendMode::Difference, [200, 100, 50], [50, 100, 200], [150, 0, 150])]
    // Addition: saturating add
    #[case(BlendMode::Addition, [200, 100, 50], [60, 100, 250], [255, 200, 255])]
    // Subtract: max(b - s, 0)
    #[case(BlendMode::Subtract, [200, 100, 50], [60, 100, 250], [140, 0, 0])]
    fn separable_modes_match_known_outputs(
        #[case] mode: BlendMode,
        #[case] dst: [u8; 3],
        #[case] src: [u8; 3],
        #[case] expected: [u8; 3],
    ) {
        let s = Rgba::new(src[0], src[1], src[2], 255);
        let d = Rgba::new(dst[0], dst[1], dst[2], 255);
        let out = blend(mode, s, d, 255);
        assert_eq!([out.r, out.g, out.b], expected, "{mode:?}");
    }

    #[test]
    fn color_dodge_extremes() {
        // dodge(0, x) = 0
        assert_eq!(channel_color_dodge(0, 100), 0);
        // dodge(b, 255) = 255 (since 255-s = 0, b >= 0 always true)
        assert_eq!(channel_color_dodge(100, 255), 255);
        // dodge(b, 0) = b (255-0=255, b/255*255 = b)
        for b in 0u8..=255 {
            assert_eq!(channel_color_dodge(b, 0), b, "b={b}");
        }
    }

    #[test]
    fn color_burn_extremes() {
        // burn(255, x) = 255
        assert_eq!(channel_color_burn(255, 100), 255);
        // burn(0, x) = 0 unless x = 0
        assert_eq!(channel_color_burn(0, 200), 0);
        // burn(b, 255) = b
        for b in 0u8..=255 {
            assert_eq!(channel_color_burn(b, 255), b, "b={b}");
        }
    }

    #[test]
    fn divide_extremes() {
        // divide(0, x) = 0
        assert_eq!(channel_divide(0, 100), 0);
        // divide(x, 0) = 255 when x > 0 (b >= 0, hits b >= s branch)
        assert_eq!(channel_divide(100, 0), 255);
        // divide(b, b) = 255
        for b in 1u8..=255 {
            assert_eq!(channel_divide(b, b), 255, "b={b}");
        }
    }

    #[test]
    fn hard_light_and_overlay_are_symmetric() {
        // overlay(b, s) == hard_light(s, b)
        for b in [0u8, 64, 128, 192, 255] {
            for s in [0u8, 64, 128, 192, 255] {
                assert_eq!(
                    channel_overlay(b, s),
                    channel_hard_light(s, b),
                    "b={b}, s={s}"
                );
            }
        }
    }

    #[test]
    fn exclusion_symmetric() {
        for b in [0u8, 50, 128, 200, 255] {
            for s in [0u8, 50, 128, 200, 255] {
                assert_eq!(channel_exclusion(b, s), channel_exclusion(s, b));
            }
        }
    }

    #[test]
    fn soft_light_neutral_at_half_gray() {
        // soft_light(b, 128) ≈ b for any b — the W3C formula maps
        // s=0.5 to identity (within rounding).
        for b in [10u8, 80, 128, 200, 250] {
            let v = channel_soft_light(b, 128);
            assert!(v.abs_diff(b) <= 1, "b={b}, got {v}");
        }
    }

    #[test]
    fn hue_blend_lum_matches_dst() {
        // Hue blend: result has dst's luminosity.
        let src = [255u8, 0, 0]; // red
        let dst = [128u8, 128, 64];
        let dst_lum = lum_f64(to_unit(dst[0]), to_unit(dst[1]), to_unit(dst[2]));
        let out = channels_hue(dst, src);
        let out_lum = lum_f64(to_unit(out[0]), to_unit(out[1]), to_unit(out[2]));
        assert!(
            (out_lum - dst_lum).abs() < 1.5 / 255.0,
            "got {out_lum} want {dst_lum}"
        );
    }

    #[test]
    fn color_blend_lum_matches_dst() {
        // Color blend: SetLum(src, lum(dst)) — output lum == dst lum.
        let src = [200u8, 50, 25];
        let dst = [40u8, 200, 80];
        let out = channels_color(dst, src);
        let out_lum = lum_f64(to_unit(out[0]), to_unit(out[1]), to_unit(out[2]));
        let dst_lum = lum_f64(to_unit(dst[0]), to_unit(dst[1]), to_unit(dst[2]));
        assert!((out_lum - dst_lum).abs() < 1.5 / 255.0);
    }

    #[test]
    fn premix_normal_returns_src_unchanged() {
        let src = Rgba::new(10, 20, 30, 40);
        let dst = Rgba::new(50, 60, 70, 200);
        assert_eq!(premix(BlendMode::Normal, src, dst), src);
    }

    #[test]
    fn blend_with_zero_opacity_keeps_dst() {
        let src = Rgba::opaque(255, 0, 0);
        let dst = Rgba::opaque(0, 255, 0);
        let out = blend(BlendMode::Multiply, src, dst, 0);
        assert_eq!(out, dst);
    }

    #[test]
    fn blend_into_transparent_dst_uses_opacity() {
        let src = Rgba::opaque(100, 150, 200);
        let out = blend(BlendMode::Multiply, src, Rgba::transparent(), 128);
        // multiply against transparent dst (rgb=0,0,0) zeros the source
        // RGB; only the modulated alpha survives.
        assert_eq!(out, Rgba::new(0, 0, 0, mul_un8(255, 128)));
    }
}
