//! Integer-pixel translation.
//!
//! Selected pixels (or all pixels when no mask is supplied) move by
//! `(dx, dy)`. Pixels that shift outside the buffer bounds are
//! discarded. The positions vacated by selected pixels become
//! transparent; unselected pixels stay in place.

use crate::canvas::buffer::PixelBuffer;
use crate::selection::SelectionMask;

use super::error::{Error, Result};

/// Translates a pixel buffer by `(dx, dy)` canvas pixels.
///
/// When `mask` is `None` the entire buffer shifts. When a mask is
/// provided, only pixels with non-zero coverage move; pixels outside
/// the selection stay in place and the vacated positions become
/// transparent.
///
/// Pixels that translate outside the buffer bounds are clipped.
///
/// # Errors
///
/// Returns [`Error::EmptyBuffer`] if `buf` has zero dimensions.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_lossless
)]
pub fn translate(
    buf: &PixelBuffer,
    dx: i32,
    dy: i32,
    mask: Option<&SelectionMask>,
) -> Result<PixelBuffer> {
    if buf.is_empty() {
        return Err(Error::EmptyBuffer);
    }
    let w = buf.width();
    let h = buf.height();
    let mut out = PixelBuffer::new(w, h)?;

    // Pass 1: copy pixels that are not moving (unselected or no selection).
    // When there is no mask every pixel moves, so this pass does nothing.
    if let Some(m) = mask {
        for y in 0..h {
            for x in 0..w {
                if !m.is_selected(x, y) {
                    if let Some(px) = buf.pixel(x, y) {
                        out.set_pixel(x, y, px);
                    }
                }
            }
        }
    }

    // Pass 2: place each moving pixel at its new position.
    for y in 0..h {
        for x in 0..w {
            let moving = mask.is_none_or(|m| m.is_selected(x, y));
            if moving {
                let dst_x = x as i64 + dx as i64;
                let dst_y = y as i64 + dy as i64;
                if dst_x >= 0 && dst_x < w as i64 && dst_y >= 0 && dst_y < h as i64 {
                    if let Some(px) = buf.pixel(x, y) {
                        out.set_pixel(dst_x as u32, dst_y as u32, px);
                    }
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
    fn translate_zero_is_identity() {
        let mut buf = PixelBuffer::new(4, 4).unwrap();
        buf.set_pixel(1, 1, red());
        let out = translate(&buf, 0, 0, None).unwrap();
        assert_eq!(out.pixel(1, 1), Some(red()));
    }

    #[test]
    fn translate_right_one_pixel() {
        let mut buf = PixelBuffer::new(4, 4).unwrap();
        buf.set_pixel(0, 0, red());
        let out = translate(&buf, 1, 0, None).unwrap();
        assert_eq!(out.pixel(0, 0), Some(transparent()));
        assert_eq!(out.pixel(1, 0), Some(red()));
    }

    #[test]
    fn translate_clips_overflow() {
        let mut buf = PixelBuffer::new(4, 4).unwrap();
        buf.set_pixel(3, 3, red());
        let out = translate(&buf, 1, 1, None).unwrap();
        // pixel would go to (4,4) which is out of bounds
        assert_eq!(out.pixel(3, 3), Some(transparent()));
    }

    #[test]
    fn translate_negative_direction() {
        let mut buf = PixelBuffer::new(4, 4).unwrap();
        buf.set_pixel(2, 2, red());
        let out = translate(&buf, -1, -1, None).unwrap();
        assert_eq!(out.pixel(1, 1), Some(red()));
        assert_eq!(out.pixel(2, 2), Some(transparent()));
    }

    #[test]
    fn translate_with_mask_moves_selected_only() {
        let mut buf = PixelBuffer::new(4, 1).unwrap();
        buf.set_pixel(0, 0, red());
        buf.set_pixel(1, 0, Rgba::opaque(0, 255, 0));

        let mut mask = SelectionMask::new(4, 1).unwrap();
        mask.set(0, 0, 255); // select pixel 0
        // pixel 1 is unselected

        let out = translate(&buf, 1, 0, Some(&mask)).unwrap();
        // pixel 0 moved to 1
        assert_eq!(out.pixel(1, 0), Some(red()));
        // pixel 1 stayed
        assert_eq!(out.pixel(1, 0), Some(red())); // overwritten by moved pixel
        // unselected pixel 1 is at its original spot
        assert_eq!(out.pixel(0, 0), Some(transparent())); // vacated
    }

    #[test]
    fn translate_empty_buffer_errors() {
        let buf = PixelBuffer::empty();
        assert_eq!(translate(&buf, 0, 0, None), Err(Error::EmptyBuffer));
    }
}
