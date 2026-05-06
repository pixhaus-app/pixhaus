//! Rotation operations.
//!
//! # Exact 90°/180°/270° rotations
//!
//! [`rotate_90_cw`], [`rotate_90_ccw`], and [`rotate_180`] use integer
//! index remapping — no interpolation, fully lossless.
//!
//! # Arbitrary angle: `RotSprite`
//!
//! [`rotate_rotsprite`] implements the `RotSprite` algorithm for
//! pixel-art-quality arbitrary rotation:
//!
//! 1. Scale the source up 4× using two passes of the Scale2x (EPX)
//!    algorithm.
//! 2. Rotate the enlarged image with bilinear interpolation.
//! 3. Scale back down 4× to the original dimensions using
//!    nearest-neighbor.
//!
//! Working in the 4× domain gives bilinear interpolation enough
//! information to reconstruct pixel-art edges cleanly. The result is
//! significantly better than direct bilinear rotation on the source.
//!
//! # Arbitrary angle: bilinear
//!
//! [`rotate_bilinear`] applies bilinear interpolation directly on the
//! source. Intended for non-pixel-art content or as an opt-in when
//! rendering speed matters more than pixel fidelity.
//!
//! # Reference
//!
//! `RotSprite` algorithm: <https://en.wikipedia.org/wiki/Pixel_art_scaling_algorithms#RotSprite>

use crate::canvas::buffer::PixelBuffer;
use crate::project::Rgba;

use super::error::{Error, Result};
use super::scale::{scale_integer, scale_nearest};

// ──────────────────────────────────────────────────────────────────────────────
// Exact lossless rotations (multiples of 90°)
// ──────────────────────────────────────────────────────────────────────────────

/// Rotates `buf` 90° clockwise.
///
/// The output dimensions are transposed: `height × width`.
///
/// # Errors
///
/// Returns [`Error::EmptyBuffer`] if `buf` is 0×0.
pub fn rotate_90_cw(buf: &PixelBuffer) -> Result<PixelBuffer> {
    if buf.is_empty() {
        return Err(Error::EmptyBuffer);
    }
    // Source W×H → output H×W.
    let src_w = buf.width();
    let src_h = buf.height();
    let mut out = PixelBuffer::new(src_h, src_w)?;

    // Mapping: new(nx, ny) = old(ny, H - 1 - nx)
    for ny in 0..src_w {
        for nx in 0..src_h {
            let sx = ny;
            let sy = src_h - 1 - nx;
            if let Some(px) = buf.pixel(sx, sy) {
                out.set_pixel(nx, ny, px);
            }
        }
    }

    Ok(out)
}

/// Rotates `buf` 90° counter-clockwise.
///
/// The output dimensions are transposed: `height × width`.
///
/// # Errors
///
/// Returns [`Error::EmptyBuffer`] if `buf` is 0×0.
pub fn rotate_90_ccw(buf: &PixelBuffer) -> Result<PixelBuffer> {
    if buf.is_empty() {
        return Err(Error::EmptyBuffer);
    }
    let src_w = buf.width();
    let src_h = buf.height();
    let mut out = PixelBuffer::new(src_h, src_w)?;

    // Mapping: new(nx, ny) = old(W - 1 - ny, nx)
    for ny in 0..src_w {
        for nx in 0..src_h {
            let sx = src_w - 1 - ny;
            let sy = nx;
            if let Some(px) = buf.pixel(sx, sy) {
                out.set_pixel(nx, ny, px);
            }
        }
    }

    Ok(out)
}

/// Rotates `buf` 180° (equivalent to flipping both axes).
///
/// The output has the same dimensions as the source.
///
/// # Errors
///
/// Returns [`Error::EmptyBuffer`] if `buf` is 0×0.
pub fn rotate_180(buf: &PixelBuffer) -> Result<PixelBuffer> {
    if buf.is_empty() {
        return Err(Error::EmptyBuffer);
    }
    let w = buf.width();
    let h = buf.height();
    let mut out = PixelBuffer::new(w, h)?;

    for y in 0..h {
        for x in 0..w {
            if let Some(px) = buf.pixel(w - 1 - x, h - 1 - y) {
                out.set_pixel(x, y, px);
            }
        }
    }

    Ok(out)
}

// ──────────────────────────────────────────────────────────────────────────────
// Scale2x / EPX helpers used internally by RotSprite
// ──────────────────────────────────────────────────────────────────────────────

/// One pass of the Scale2x (EPX) pixel-art upscaler.
///
/// Each source pixel at (sx, sy) produces a 2×2 block in the output.
/// Edge-neighbor comparisons decide whether to place the neighbor
/// colour or the source colour in each quadrant. The result preserves
/// diagonal edges characteristic of pixel art far better than bilinear
/// upscaling.
///
/// Reference: <https://www.scale2x.it/algorithm>
fn scale2x(buf: &PixelBuffer) -> PixelBuffer {
    let src_w = buf.width();
    let src_h = buf.height();
    // Allocation can't fail here: we're doubling dimensions that already fit
    // in u32; the maximum buf.width() * 2 fits because buf was already valid.
    let mut out = scale_integer(buf, 2).unwrap_or_else(|_| buf.clone());

    // Edge-clamp: EPX spec says border pixels treat missing neighbors as
    // themselves, preventing spurious rules from firing on image edges.
    let sample = |x: u32, y: u32| -> Rgba { buf.pixel(x, y).unwrap_or(Rgba::transparent()) };
    let last_x = src_w.saturating_sub(1);
    let last_y = src_h.saturating_sub(1);

    for src_y in 0..src_h {
        for src_x in 0..src_w {
            let center = sample(src_x, src_y);
            let above = sample(src_x, src_y.saturating_sub(1));
            let right = sample((src_x + 1).min(last_x), src_y);
            let left = sample(src_x.saturating_sub(1), src_y);
            let below = sample(src_x, (src_y + 1).min(last_y));

            let out_x = src_x * 2;
            let out_y = src_y * 2;

            // EPX rules — each quadrant takes the neighbour colour when two
            // perpendicular neighbours match and the opposite two don't.
            let q0 = if left == above && left != below && above != right {
                above
            } else {
                center
            }; // top-left
            let q1 = if above == right && above != left && right != below {
                right
            } else {
                center
            }; // top-right
            let q2 = if below == left && below != right && left != above {
                left
            } else {
                center
            }; // bottom-left
            let q3 = if right == below && right != above && below != left {
                below
            } else {
                center
            }; // bottom-right

            out.set_pixel(out_x, out_y, q0);
            out.set_pixel(out_x + 1, out_y, q1);
            out.set_pixel(out_x, out_y + 1, q2);
            out.set_pixel(out_x + 1, out_y + 1, q3);
        }
    }

    out
}

// ──────────────────────────────────────────────────────────────────────────────
// Bilinear sampling helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Linearly interpolates between two `u8` channel values.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[inline]
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let result = f32::from(a) + (f32::from(b) - f32::from(a)) * t;
    result.round().clamp(0.0, 255.0) as u8
}

/// Linearly interpolates between two [`Rgba`] values in straight-alpha space.
#[inline]
fn lerp_rgba(a: Rgba, b: Rgba, t: f32) -> Rgba {
    Rgba::new(
        lerp_u8(a.r, b.r, t),
        lerp_u8(a.g, b.g, t),
        lerp_u8(a.b, b.b, t),
        lerp_u8(a.a, b.a, t),
    )
}

/// Samples `buf` at sub-pixel position `(x, y)` with bilinear
/// interpolation. Out-of-bounds samples contribute transparent black.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]
pub(super) fn bilinear_sample(buf: &PixelBuffer, x: f32, y: f32) -> Rgba {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let fx = x - x.floor();
    let fy = y - y.floor();

    let sample = |px: i32, py: i32| -> Rgba {
        let in_bounds = px >= 0 && py >= 0 && px < buf.width() as i32 && py < buf.height() as i32;
        if in_bounds {
            buf.pixel(px as u32, py as u32)
                .unwrap_or(Rgba::transparent())
        } else {
            Rgba::transparent()
        }
    };

    let p00 = sample(x0, y0);
    let p10 = sample(x0 + 1, y0);
    let p01 = sample(x0, y0 + 1);
    let p11 = sample(x0 + 1, y0 + 1);

    lerp_rgba(lerp_rgba(p00, p10, fx), lerp_rgba(p01, p11, fx), fy)
}

// ──────────────────────────────────────────────────────────────────────────────
// Rotation with bilinear interpolation
// ──────────────────────────────────────────────────────────────────────────────

/// Rotates `buf` by `angle_rad` using bilinear interpolation.
///
/// Rotation is around the canvas centre. Pixels that rotate outside
/// the buffer bounds become transparent. The output has the same
/// dimensions as the source.
///
/// # Errors
///
/// Returns [`Error::EmptyBuffer`] if `buf` is 0×0.
#[allow(clippy::cast_precision_loss)]
pub fn rotate_bilinear(buf: &PixelBuffer, angle_rad: f32) -> Result<PixelBuffer> {
    if buf.is_empty() {
        return Err(Error::EmptyBuffer);
    }
    let w = buf.width();
    let h = buf.height();
    let cx = (w as f32 - 1.0) / 2.0;
    let cy = (h as f32 - 1.0) / 2.0;

    // Inverse rotation: map each output pixel back to its source position.
    let cos_a = (-angle_rad).cos();
    let sin_a = (-angle_rad).sin();

    let mut out = PixelBuffer::new(w, h)?;

    for dy in 0..h {
        for dx in 0..w {
            let rx = dx as f32 - cx;
            let ry = dy as f32 - cy;
            let sx = cos_a * rx - sin_a * ry + cx;
            let sy = sin_a * rx + cos_a * ry + cy;
            out.set_pixel(dx, dy, bilinear_sample(buf, sx, sy));
        }
    }

    Ok(out)
}

// ──────────────────────────────────────────────────────────────────────────────
// RotSprite
// ──────────────────────────────────────────────────────────────────────────────

/// Rotates `buf` by `angle_deg` using the `RotSprite` algorithm.
///
/// Exact 90°/180°/270° angles are handled losslessly without scaling.
/// For all other angles:
///
/// 1. The source is upscaled 4× using two passes of Scale2x.
/// 2. The upscaled image is rotated with bilinear interpolation.
/// 3. The result is scaled back down to the original dimensions using
///    nearest-neighbor.
///
/// The output has the same dimensions as the source. Pixels that
/// rotate outside the bounds become transparent.
///
/// # Errors
///
/// Returns [`Error::EmptyBuffer`] if `buf` is 0×0.
pub fn rotate_rotsprite(buf: &PixelBuffer, angle_deg: f32) -> Result<PixelBuffer> {
    // Tolerance for treating an angle as an exact multiple of 90°.
    const TOL: f32 = 0.01;

    if buf.is_empty() {
        return Err(Error::EmptyBuffer);
    }

    // Normalise to [0, 360).
    let angle = ((angle_deg % 360.0) + 360.0) % 360.0;

    // Handle exact multiples of 90° losslessly.
    if angle < TOL || (angle - 360.0).abs() < TOL {
        return Ok(buf.clone());
    }
    if (angle - 90.0).abs() < TOL {
        return rotate_90_cw(buf);
    }
    if (angle - 180.0).abs() < TOL {
        return rotate_180(buf);
    }
    if (angle - 270.0).abs() < TOL {
        return rotate_90_ccw(buf);
    }

    // Upscale 4× (two Scale2x passes).
    let up2 = scale2x(buf);
    let up4 = scale2x(&up2);

    // Rotate the upscaled image with bilinear interpolation.
    let rotated = rotate_bilinear(&up4, angle_deg.to_radians())?;

    // Scale back to original dimensions using nearest-neighbor.
    scale_nearest(&rotated, buf.width(), buf.height())
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn red() -> Rgba {
        Rgba::opaque(255, 0, 0)
    }
    fn green() -> Rgba {
        Rgba::opaque(0, 255, 0)
    }
    fn blue() -> Rgba {
        Rgba::opaque(0, 0, 255)
    }

    /// Build a 3×2 test pattern:
    /// ```text
    /// R G B
    /// . . .
    /// ```
    fn pattern_3x2() -> PixelBuffer {
        let mut buf = PixelBuffer::new(3, 2).unwrap();
        buf.set_pixel(0, 0, red());
        buf.set_pixel(1, 0, green());
        buf.set_pixel(2, 0, blue());
        buf
    }

    #[test]
    fn rotate_90_cw_dimensions() {
        let buf = pattern_3x2(); // 3×2
        let out = rotate_90_cw(&buf).unwrap();
        assert_eq!(out.width(), 2); // transposed
        assert_eq!(out.height(), 3);
    }

    #[test]
    fn rotate_90_cw_pixel_positions() {
        // 3×2 source:
        //   R G B   (row 0)
        //   . . .   (row 1)
        //
        // After 90° CW (output 2×3):
        //   . R   (row 0)
        //   . G   (row 1)
        //   . B   (row 2)
        let buf = pattern_3x2();
        let out = rotate_90_cw(&buf).unwrap();
        assert_eq!(out.pixel(1, 0), Some(red()));
        assert_eq!(out.pixel(1, 1), Some(green()));
        assert_eq!(out.pixel(1, 2), Some(blue()));
    }

    #[test]
    fn rotate_90_ccw_dimensions() {
        let buf = pattern_3x2();
        let out = rotate_90_ccw(&buf).unwrap();
        assert_eq!(out.width(), 2);
        assert_eq!(out.height(), 3);
    }

    #[test]
    fn rotate_90_ccw_pixel_positions() {
        // 3×2 source:
        //   R G B   (row 0)
        //   . . .   (row 1)
        //
        // After 90° CCW (output 2×3):
        //   B .   (row 0)
        //   G .   (row 1)
        //   R .   (row 2)
        let buf = pattern_3x2();
        let out = rotate_90_ccw(&buf).unwrap();
        assert_eq!(out.pixel(0, 0), Some(blue()));
        assert_eq!(out.pixel(0, 1), Some(green()));
        assert_eq!(out.pixel(0, 2), Some(red()));
    }

    #[test]
    fn rotate_180_pixel_positions() {
        let buf = pattern_3x2();
        let out = rotate_180(&buf).unwrap();
        assert_eq!(out.width(), 3);
        assert_eq!(out.height(), 2);
        // top-right of source (blue) should appear at bottom-left of output
        assert_eq!(out.pixel(0, 1), Some(blue()));
        assert_eq!(out.pixel(1, 1), Some(green()));
        assert_eq!(out.pixel(2, 1), Some(red()));
    }

    #[test]
    fn four_cw_rotations_is_identity() {
        let buf = pattern_3x2();
        let r1 = rotate_90_cw(&buf).unwrap();
        let r2 = rotate_90_cw(&r1).unwrap();
        let r3 = rotate_90_cw(&r2).unwrap();
        let r4 = rotate_90_cw(&r3).unwrap();
        assert_eq!(r4, buf);
    }

    #[test]
    fn cw_then_ccw_is_identity() {
        let buf = pattern_3x2();
        let cw = rotate_90_cw(&buf).unwrap();
        let back = rotate_90_ccw(&cw).unwrap();
        assert_eq!(back, buf);
    }

    #[test]
    fn rotate_empty_errors() {
        let buf = PixelBuffer::empty();
        assert!(rotate_90_cw(&buf).is_err());
        assert!(rotate_90_ccw(&buf).is_err());
        assert!(rotate_180(&buf).is_err());
        assert!(rotate_bilinear(&buf, 0.0).is_err());
        assert!(rotate_rotsprite(&buf, 45.0).is_err());
    }

    #[test]
    fn rotsprite_zero_degrees_is_identity() {
        let mut buf = PixelBuffer::new(4, 4).unwrap();
        buf.set_pixel(1, 1, red());
        let out = rotate_rotsprite(&buf, 0.0).unwrap();
        assert_eq!(out, buf);
    }

    #[test]
    fn rotsprite_90_cw_matches_exact() {
        let buf = pattern_3x2();
        let via_rotsprite = rotate_rotsprite(&buf, 90.0).unwrap();
        let exact = rotate_90_cw(&buf).unwrap();
        assert_eq!(via_rotsprite, exact);
    }

    #[test]
    fn rotsprite_180_matches_exact() {
        let buf = pattern_3x2();
        let via_rotsprite = rotate_rotsprite(&buf, 180.0).unwrap();
        let exact = rotate_180(&buf).unwrap();
        assert_eq!(via_rotsprite, exact);
    }

    #[test]
    fn rotsprite_270_matches_exact() {
        let buf = pattern_3x2();
        let via_rotsprite = rotate_rotsprite(&buf, 270.0).unwrap();
        let exact = rotate_90_ccw(&buf).unwrap();
        assert_eq!(via_rotsprite, exact);
    }

    #[test]
    fn rotsprite_45_preserves_dimensions() {
        let buf = PixelBuffer::new(16, 16).unwrap();
        let out = rotate_rotsprite(&buf, 45.0).unwrap();
        assert_eq!(out.width(), 16);
        assert_eq!(out.height(), 16);
    }

    #[test]
    fn bilinear_sample_centre_pixel() {
        let mut buf = PixelBuffer::new(3, 3).unwrap();
        buf.set_pixel(1, 1, red());
        // Sampling exactly at the centre pixel should return red.
        let px = bilinear_sample(&buf, 1.0, 1.0);
        assert_eq!(px, red());
    }

    #[test]
    fn scale2x_doubles_dimensions() {
        let buf = PixelBuffer::new(4, 6).unwrap();
        let out = scale2x(&buf);
        assert_eq!(out.width(), 8);
        assert_eq!(out.height(), 12);
    }

    #[test]
    fn scale2x_uniform_colour_unchanged() {
        // EPX on a uniform colour produces the same colour everywhere.
        let buf = PixelBuffer::filled(4, 4, red()).unwrap();
        let out = scale2x(&buf);
        for y in 0..out.height() {
            for x in 0..out.width() {
                assert_eq!(out.pixel(x, y), Some(red()), "at ({x},{y})");
            }
        }
    }
}
