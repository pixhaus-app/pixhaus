//! Pixel buffer scaling operations.
//!
//! Three entry points:
//!
//! - [`scale_nearest`] — resize to any target dimensions using
//!   nearest-neighbor sampling. Default for pixel art; no blurring.
//! - [`scale_integer`] — integer-multiple upscale. Each source pixel
//!   expands to an `N×N` block; strictly lossless.
//! - [`scale_integer_down`] — integer-divisor downscale. Samples the
//!   top-left pixel of each `N×N` block; lossless for pixel art
//!   (no averaging or blending).

use crate::canvas::buffer::PixelBuffer;

use super::error::{Error, Result};

/// Resizes `buf` to `(new_w, new_h)` using nearest-neighbor sampling.
///
/// The output dimensions may be larger or smaller than the input.
/// Each output pixel maps to the source pixel whose center is closest
/// in normalized UV space.
///
/// # Errors
///
/// - [`Error::EmptyBuffer`] if `buf` is 0×0.
/// - [`Error::DimensionOverflow`] if `new_w` or `new_h` is zero
///   (zero dimensions are disallowed for a non-empty result).
/// - [`Error::Buffer`] if the output buffer allocation fails.
pub fn scale_nearest(buf: &PixelBuffer, new_w: u32, new_h: u32) -> Result<PixelBuffer> {
    if buf.is_empty() {
        return Err(Error::EmptyBuffer);
    }
    if new_w == 0 || new_h == 0 {
        return Err(Error::DimensionOverflow);
    }

    let src_w = buf.width();
    let src_h = buf.height();
    let mut out = PixelBuffer::new(new_w, new_h)?;

    for dy in 0..new_h {
        // Integer nearest-neighbor: multiply before dividing avoids float rounding.
        let sy = (dy * src_h / new_h).min(src_h - 1);
        for dx in 0..new_w {
            let sx = (dx * src_w / new_w).min(src_w - 1);
            if let Some(px) = buf.pixel(sx, sy) {
                out.set_pixel(dx, dy, px);
            }
        }
    }

    Ok(out)
}

/// Enlarges `buf` by an integer `factor` (≥ 1).
///
/// Each source pixel expands to an `factor × factor` block of identical
/// pixels. This is lossless and is the standard way to view pixel art
/// at 2×, 3×, 4× magnification.
///
/// `factor = 1` returns a clone.
///
/// # Errors
///
/// - [`Error::EmptyBuffer`] if `buf` is 0×0.
/// - [`Error::InvalidScaleFactor`] if `factor` is 0.
/// - [`Error::DimensionOverflow`] if `buf.width() * factor` or
///   `buf.height() * factor` overflows `u32`.
/// - [`Error::Buffer`] on allocation failure.
pub fn scale_integer(buf: &PixelBuffer, factor: u32) -> Result<PixelBuffer> {
    if buf.is_empty() {
        return Err(Error::EmptyBuffer);
    }
    if factor == 0 {
        return Err(Error::InvalidScaleFactor(0));
    }

    let new_w = buf.width().checked_mul(factor).ok_or(Error::DimensionOverflow)?;
    let new_h = buf.height().checked_mul(factor).ok_or(Error::DimensionOverflow)?;
    let mut out = PixelBuffer::new(new_w, new_h)?;

    for sy in 0..buf.height() {
        for sx in 0..buf.width() {
            if let Some(px) = buf.pixel(sx, sy) {
                let out_col = sx * factor;
                let out_row = sy * factor;
                for dy in 0..factor {
                    for dx in 0..factor {
                        out.set_pixel(out_col + dx, out_row + dy, px);
                    }
                }
            }
        }
    }

    Ok(out)
}

/// Shrinks `buf` by an integer `divisor` (≥ 1) using nearest-neighbor
/// (top-left block sample).
///
/// The output dimensions are `ceil(width / divisor) × ceil(height / divisor)`.
/// For pixel art this preserves visual fidelity better than averaging.
///
/// `divisor = 1` returns a clone.
///
/// # Errors
///
/// - [`Error::EmptyBuffer`] if `buf` is 0×0.
/// - [`Error::InvalidScaleFactor`] if `divisor` is 0.
/// - [`Error::Buffer`] on allocation failure.
pub fn scale_integer_down(buf: &PixelBuffer, divisor: u32) -> Result<PixelBuffer> {
    if buf.is_empty() {
        return Err(Error::EmptyBuffer);
    }
    if divisor == 0 {
        return Err(Error::InvalidScaleFactor(0));
    }

    let new_w = buf.width().div_ceil(divisor);
    let new_h = buf.height().div_ceil(divisor);
    let mut out = PixelBuffer::new(new_w, new_h)?;

    for dy in 0..new_h {
        for dx in 0..new_w {
            let sx = (dx * divisor).min(buf.width() - 1);
            let sy = (dy * divisor).min(buf.height() - 1);
            if let Some(px) = buf.pixel(sx, sy) {
                out.set_pixel(dx, dy, px);
            }
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Rgba;

    fn red() -> Rgba {
        Rgba::opaque(255, 0, 0)
    }
    fn green() -> Rgba {
        Rgba::opaque(0, 255, 0)
    }

    #[test]
    fn nearest_identity() {
        let mut buf = PixelBuffer::new(2, 2).unwrap();
        buf.set_pixel(0, 0, red());
        let out = scale_nearest(&buf, 2, 2).unwrap();
        assert_eq!(out.pixel(0, 0), Some(red()));
        assert_eq!(out.width(), 2);
        assert_eq!(out.height(), 2);
    }

    #[test]
    fn nearest_upscale_2x() {
        let mut buf = PixelBuffer::new(2, 2).unwrap();
        buf.set_pixel(0, 0, red());
        buf.set_pixel(1, 1, green());
        let out = scale_nearest(&buf, 4, 4).unwrap();
        assert_eq!(out.width(), 4);
        assert_eq!(out.height(), 4);
        // top-left quadrant should be red
        assert_eq!(out.pixel(0, 0), Some(red()));
        assert_eq!(out.pixel(1, 1), Some(red()));
        // bottom-right quadrant should be green
        assert_eq!(out.pixel(2, 2), Some(green()));
        assert_eq!(out.pixel(3, 3), Some(green()));
    }

    #[test]
    fn nearest_downscale_2x() {
        let mut buf = PixelBuffer::new(4, 4).unwrap();
        buf.set_pixel(0, 0, red());
        buf.set_pixel(2, 2, green());
        let out = scale_nearest(&buf, 2, 2).unwrap();
        assert_eq!(out.width(), 2);
        assert_eq!(out.height(), 2);
        assert_eq!(out.pixel(0, 0), Some(red()));
        assert_eq!(out.pixel(1, 1), Some(green()));
    }

    #[test]
    fn nearest_empty_errors() {
        let buf = PixelBuffer::empty();
        assert_eq!(scale_nearest(&buf, 2, 2), Err(Error::EmptyBuffer));
    }

    #[test]
    fn nearest_zero_target_errors() {
        let buf = PixelBuffer::new(4, 4).unwrap();
        assert_eq!(scale_nearest(&buf, 0, 4), Err(Error::DimensionOverflow));
        assert_eq!(scale_nearest(&buf, 4, 0), Err(Error::DimensionOverflow));
    }

    #[test]
    fn integer_upscale_2x_expands_blocks() {
        let mut buf = PixelBuffer::new(2, 2).unwrap();
        buf.set_pixel(0, 0, red());
        buf.set_pixel(1, 1, green());
        let out = scale_integer(&buf, 2).unwrap();
        assert_eq!(out.width(), 4);
        assert_eq!(out.height(), 4);
        // top-left 2×2 block is red
        for y in 0..2 {
            for x in 0..2 {
                assert_eq!(out.pixel(x, y), Some(red()), "at ({x},{y})");
            }
        }
        // bottom-right 2×2 block is green
        for y in 2..4 {
            for x in 2..4 {
                assert_eq!(out.pixel(x, y), Some(green()), "at ({x},{y})");
            }
        }
    }

    #[test]
    fn integer_upscale_1x_is_clone() {
        let mut buf = PixelBuffer::new(3, 3).unwrap();
        buf.set_pixel(1, 1, red());
        let out = scale_integer(&buf, 1).unwrap();
        assert_eq!(out, buf);
    }

    #[test]
    fn integer_upscale_zero_factor_errors() {
        let buf = PixelBuffer::new(2, 2).unwrap();
        assert_eq!(scale_integer(&buf, 0), Err(Error::InvalidScaleFactor(0)));
    }

    #[test]
    fn integer_down_2x_samples_top_left() {
        let mut buf = PixelBuffer::new(4, 4).unwrap();
        buf.set_pixel(0, 0, red()); // top-left of first block
        buf.set_pixel(2, 2, green()); // top-left of last block
        let out = scale_integer_down(&buf, 2).unwrap();
        assert_eq!(out.width(), 2);
        assert_eq!(out.height(), 2);
        assert_eq!(out.pixel(0, 0), Some(red()));
        assert_eq!(out.pixel(1, 1), Some(green()));
    }

    #[test]
    fn integer_down_1x_is_clone() {
        let mut buf = PixelBuffer::new(3, 3).unwrap();
        buf.set_pixel(2, 2, red());
        let out = scale_integer_down(&buf, 1).unwrap();
        assert_eq!(out, buf);
    }

    #[test]
    fn integer_down_zero_divisor_errors() {
        let buf = PixelBuffer::new(2, 2).unwrap();
        assert_eq!(scale_integer_down(&buf, 0), Err(Error::InvalidScaleFactor(0)));
    }

    #[test]
    fn integer_down_odd_dimension_rounds_up() {
        // 5×5 ÷ 2 = 3×3 (ceiling)
        let buf = PixelBuffer::new(5, 5).unwrap();
        let out = scale_integer_down(&buf, 2).unwrap();
        assert_eq!(out.width(), 3);
        assert_eq!(out.height(), 3);
    }
}
