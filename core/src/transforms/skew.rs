//! Skew (shear) operations.
//!
//! Skewing shifts each row or column by an amount proportional to its
//! position. The resulting transform is non-pixel-perfect for
//! non-integer factors (sub-pixel offsets are rounded to the nearest
//! integer pixel). Pixels that shift outside the buffer bounds are
//! discarded; positions that no longer receive a source pixel are
//! filled with transparent.
//!
//! **Usage note:** skew is documented as a non-pixel-perfect operation.
//! For sharp pixel art the factor is most useful as an integer or a
//! simple fraction (e.g. `0.5`, `1.0`, `2.0`).

use crate::canvas::buffer::PixelBuffer;

use super::error::{Error, Result};

/// Horizontally shears `buf` by `factor`.
///
/// Each row `y` is shifted right by `round(factor * y)` pixels. A
/// positive `factor` leans the image to the right; a negative `factor`
/// leans it to the left.
///
/// The output has the same dimensions as the source. Shifted pixels
/// that land outside the buffer are discarded; vacated columns become
/// transparent.
///
/// # Errors
///
/// Returns [`Error::EmptyBuffer`] if `buf` is 0×0.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_lossless,
    clippy::cast_possible_wrap
)]
pub fn skew_x(buf: &PixelBuffer, factor: f32) -> Result<PixelBuffer> {
    if buf.is_empty() {
        return Err(Error::EmptyBuffer);
    }
    let w = buf.width();
    let h = buf.height();
    let mut out = PixelBuffer::new(w, h)?;

    for y in 0..h {
        let shift = (factor * y as f32).round() as i32;
        for x in 0..w {
            // The destination column for source pixel at (x, y).
            let dst_x = x as i64 + shift as i64;
            if dst_x >= 0 && dst_x < w as i64 {
                if let Some(px) = buf.pixel(x, y) {
                    out.set_pixel(dst_x as u32, y, px);
                }
            }
        }
    }

    Ok(out)
}

/// Vertically shears `buf` by `factor`.
///
/// Each column `x` is shifted down by `round(factor * x)` pixels. A
/// positive `factor` leans the image downward; a negative `factor`
/// leans it upward.
///
/// The output has the same dimensions as the source. Shifted pixels
/// that land outside the buffer are discarded; vacated rows become
/// transparent.
///
/// # Errors
///
/// Returns [`Error::EmptyBuffer`] if `buf` is 0×0.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_lossless,
    clippy::cast_possible_wrap
)]
pub fn skew_y(buf: &PixelBuffer, factor: f32) -> Result<PixelBuffer> {
    if buf.is_empty() {
        return Err(Error::EmptyBuffer);
    }
    let w = buf.width();
    let h = buf.height();
    let mut out = PixelBuffer::new(w, h)?;

    for x in 0..w {
        let shift = (factor * x as f32).round() as i32;
        for y in 0..h {
            let dst_y = y as i64 + shift as i64;
            if dst_y >= 0 && dst_y < h as i64 {
                if let Some(px) = buf.pixel(x, y) {
                    out.set_pixel(x, dst_y as u32, px);
                }
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
    fn transparent() -> Rgba {
        Rgba::transparent()
    }

    #[test]
    fn skew_x_zero_is_identity() {
        let mut buf = PixelBuffer::new(4, 4).unwrap();
        buf.set_pixel(1, 1, red());
        let out = skew_x(&buf, 0.0).unwrap();
        assert_eq!(out.pixel(1, 1), Some(red()));
    }

    #[test]
    fn skew_y_zero_is_identity() {
        let mut buf = PixelBuffer::new(4, 4).unwrap();
        buf.set_pixel(1, 1, red());
        let out = skew_y(&buf, 0.0).unwrap();
        assert_eq!(out.pixel(1, 1), Some(red()));
    }

    #[test]
    fn skew_x_positive_shifts_rows_right() {
        // factor = 1.0: row y shifts right by y pixels.
        let mut buf = PixelBuffer::new(8, 4).unwrap();
        // Put a red pixel at (0, 2). After skew_x(1.0) it should move to (0+2, 2) = (2, 2).
        buf.set_pixel(0, 2, red());
        let out = skew_x(&buf, 1.0).unwrap();
        assert_eq!(out.pixel(2, 2), Some(red()));
        assert_eq!(out.pixel(0, 2), Some(transparent()));
    }

    #[test]
    fn skew_y_positive_shifts_columns_down() {
        // factor = 1.0: column x shifts down by x pixels.
        let mut buf = PixelBuffer::new(4, 8).unwrap();
        // Red at (2, 0) moves to (2, 0+2) = (2, 2).
        buf.set_pixel(2, 0, red());
        let out = skew_y(&buf, 1.0).unwrap();
        assert_eq!(out.pixel(2, 2), Some(red()));
        assert_eq!(out.pixel(2, 0), Some(transparent()));
    }

    #[test]
    fn skew_x_clips_out_of_bounds() {
        // factor = 2.0 on a small buffer: most pixels shift out.
        let mut buf = PixelBuffer::new(4, 4).unwrap();
        buf.set_pixel(3, 3, red()); // would shift to (3+6, 3) = out of bounds
        let out = skew_x(&buf, 2.0).unwrap();
        assert_eq!(out.pixel(3, 3), Some(transparent()));
    }

    #[test]
    fn skew_empty_errors() {
        let buf = PixelBuffer::empty();
        assert_eq!(skew_x(&buf, 1.0), Err(Error::EmptyBuffer));
        assert_eq!(skew_y(&buf, 1.0), Err(Error::EmptyBuffer));
    }

    #[test]
    fn skew_x_preserves_dimensions() {
        let buf = PixelBuffer::new(8, 6).unwrap();
        let out = skew_x(&buf, 0.5).unwrap();
        assert_eq!(out.width(), 8);
        assert_eq!(out.height(), 6);
    }

    #[test]
    fn skew_y_preserves_dimensions() {
        let buf = PixelBuffer::new(8, 6).unwrap();
        let out = skew_y(&buf, 0.5).unwrap();
        assert_eq!(out.width(), 8);
        assert_eq!(out.height(), 6);
    }
}
