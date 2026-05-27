//! Whole-buffer mirror operations.
//!
//! Both functions mirror the entire buffer and are lossless — no
//! interpolation. They back the canvas-level flip the shell exposes; the
//! selection-aware, mask-constrained variants are editor-tool work and are not
//! part of this surface.

use crate::canvas::buffer::PixelBuffer;

use super::error::{Error, Result};

/// Mirrors `buf` left-to-right. The output keeps the source dimensions.
///
/// # Errors
///
/// Returns [`Error::EmptyBuffer`] if `buf` has zero dimensions.
pub fn flip_horizontal(buf: &PixelBuffer) -> Result<PixelBuffer> {
    if buf.is_empty() {
        return Err(Error::EmptyBuffer);
    }
    let w = buf.width();
    let h = buf.height();
    let mut out = PixelBuffer::new(w, h)?;
    for y in 0..h {
        for x in 0..w {
            if let Some(px) = buf.pixel(w - 1 - x, y) {
                out.set_pixel(x, y, px);
            }
        }
    }
    Ok(out)
}

/// Mirrors `buf` top-to-bottom. The output keeps the source dimensions.
///
/// # Errors
///
/// Returns [`Error::EmptyBuffer`] if `buf` has zero dimensions.
pub fn flip_vertical(buf: &PixelBuffer) -> Result<PixelBuffer> {
    if buf.is_empty() {
        return Err(Error::EmptyBuffer);
    }
    let w = buf.width();
    let h = buf.height();
    let mut out = PixelBuffer::new(w, h)?;
    for y in 0..h {
        for x in 0..w {
            if let Some(px) = buf.pixel(x, h - 1 - y) {
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

    #[test]
    fn flip_h_two_pixels() {
        let mut buf = PixelBuffer::new(2, 1).unwrap();
        buf.set_pixel(0, 0, red());
        buf.set_pixel(1, 0, green());
        let out = flip_horizontal(&buf).unwrap();
        assert_eq!(out.pixel(0, 0), Some(green()));
        assert_eq!(out.pixel(1, 0), Some(red()));
    }

    #[test]
    fn flip_h_keeps_center_on_odd_width() {
        let mut buf = PixelBuffer::new(3, 1).unwrap();
        buf.set_pixel(0, 0, red());
        buf.set_pixel(1, 0, green());
        let out = flip_horizontal(&buf).unwrap();
        assert_eq!(out.pixel(1, 0), Some(green()));
        assert_eq!(out.pixel(2, 0), Some(red()));
    }

    #[test]
    fn flip_h_twice_is_identity() {
        let mut buf = PixelBuffer::new(4, 4).unwrap();
        buf.set_pixel(0, 0, red());
        buf.set_pixel(3, 1, green());
        let twice = flip_horizontal(&flip_horizontal(&buf).unwrap()).unwrap();
        assert_eq!(twice, buf);
    }

    #[test]
    fn flip_v_two_rows() {
        let mut buf = PixelBuffer::new(1, 2).unwrap();
        buf.set_pixel(0, 0, red());
        buf.set_pixel(0, 1, green());
        let out = flip_vertical(&buf).unwrap();
        assert_eq!(out.pixel(0, 0), Some(green()));
        assert_eq!(out.pixel(0, 1), Some(red()));
    }

    #[test]
    fn flip_v_twice_is_identity() {
        let mut buf = PixelBuffer::new(4, 4).unwrap();
        buf.set_pixel(1, 0, red());
        buf.set_pixel(2, 3, green());
        let twice = flip_vertical(&flip_vertical(&buf).unwrap()).unwrap();
        assert_eq!(twice, buf);
    }

    #[test]
    fn flip_empty_errors() {
        let buf = PixelBuffer::empty();
        assert_eq!(flip_horizontal(&buf), Err(Error::EmptyBuffer));
        assert_eq!(flip_vertical(&buf), Err(Error::EmptyBuffer));
    }
}
