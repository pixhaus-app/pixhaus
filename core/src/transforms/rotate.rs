//! Lossless 90°-multiple rotations.
//!
//! [`rotate_90_cw`], [`rotate_90_ccw`], and [`rotate_180`] use integer index
//! remapping — no interpolation. They back the canvas-level rotate the shell
//! exposes. Arbitrary-angle rotation (`RotSprite`, bilinear) is editor-tool
//! work and is not part of this surface.

use crate::canvas::buffer::PixelBuffer;

use super::error::{Error, Result};

/// Rotates `buf` 90° clockwise. The output dimensions are transposed
/// (`height × width`).
///
/// # Errors
///
/// Returns [`Error::EmptyBuffer`] if `buf` is 0×0.
pub fn rotate_90_cw(buf: &PixelBuffer) -> Result<PixelBuffer> {
    if buf.is_empty() {
        return Err(Error::EmptyBuffer);
    }
    let src_w = buf.width();
    let src_h = buf.height();
    let mut out = PixelBuffer::new(src_h, src_w)?;
    // new(nx, ny) = old(ny, H - 1 - nx)
    for ny in 0..src_w {
        for nx in 0..src_h {
            if let Some(px) = buf.pixel(ny, src_h - 1 - nx) {
                out.set_pixel(nx, ny, px);
            }
        }
    }
    Ok(out)
}

/// Rotates `buf` 90° counter-clockwise. The output dimensions are transposed
/// (`height × width`).
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
    // new(nx, ny) = old(W - 1 - ny, nx)
    for ny in 0..src_w {
        for nx in 0..src_h {
            if let Some(px) = buf.pixel(src_w - 1 - ny, nx) {
                out.set_pixel(nx, ny, px);
            }
        }
    }
    Ok(out)
}

/// Rotates `buf` 180°. The output keeps the source dimensions.
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
    fn blue() -> Rgba {
        Rgba::opaque(0, 0, 255)
    }

    /// 3×2 pattern: `R G B` on the top row, blank below.
    fn pattern_3x2() -> PixelBuffer {
        let mut buf = PixelBuffer::new(3, 2).unwrap();
        buf.set_pixel(0, 0, red());
        buf.set_pixel(1, 0, green());
        buf.set_pixel(2, 0, blue());
        buf
    }

    #[test]
    fn cw_transposes_dimensions() {
        let out = rotate_90_cw(&pattern_3x2()).unwrap();
        assert_eq!((out.width(), out.height()), (2, 3));
    }

    #[test]
    fn cw_pixel_positions() {
        // After 90° CW (output 2×3): right column holds R, G, B top-to-bottom.
        let out = rotate_90_cw(&pattern_3x2()).unwrap();
        assert_eq!(out.pixel(1, 0), Some(red()));
        assert_eq!(out.pixel(1, 1), Some(green()));
        assert_eq!(out.pixel(1, 2), Some(blue()));
    }

    #[test]
    fn ccw_pixel_positions() {
        // After 90° CCW (output 2×3): left column holds B, G, R top-to-bottom.
        let out = rotate_90_ccw(&pattern_3x2()).unwrap();
        assert_eq!(out.pixel(0, 0), Some(blue()));
        assert_eq!(out.pixel(0, 1), Some(green()));
        assert_eq!(out.pixel(0, 2), Some(red()));
    }

    #[test]
    fn four_cw_rotations_is_identity() {
        let buf = pattern_3x2();
        let mut r = buf.clone();
        for _ in 0..4 {
            r = rotate_90_cw(&r).unwrap();
        }
        assert_eq!(r, buf);
    }

    #[test]
    fn cw_then_ccw_is_identity() {
        let buf = pattern_3x2();
        let back = rotate_90_ccw(&rotate_90_cw(&buf).unwrap()).unwrap();
        assert_eq!(back, buf);
    }

    #[test]
    fn rotate_180_positions() {
        let out = rotate_180(&pattern_3x2()).unwrap();
        assert_eq!((out.width(), out.height()), (3, 2));
        // The top row reverses onto the bottom row.
        assert_eq!(out.pixel(0, 1), Some(blue()));
        assert_eq!(out.pixel(1, 1), Some(green()));
        assert_eq!(out.pixel(2, 1), Some(red()));
    }

    #[test]
    fn rotate_180_twice_is_identity() {
        let buf = pattern_3x2();
        let twice = rotate_180(&rotate_180(&buf).unwrap()).unwrap();
        assert_eq!(twice, buf);
    }

    #[test]
    fn rotate_empty_errors() {
        let buf = PixelBuffer::empty();
        assert_eq!(rotate_90_cw(&buf), Err(Error::EmptyBuffer));
        assert_eq!(rotate_90_ccw(&buf), Err(Error::EmptyBuffer));
        assert_eq!(rotate_180(&buf), Err(Error::EmptyBuffer));
    }
}
