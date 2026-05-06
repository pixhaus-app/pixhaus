//! Horizontal and vertical mirror operations.
//!
//! Both functions accept an optional [`SelectionMask`]. When a mask is
//! supplied, only the selected pixels are flipped; unselected pixels
//! stay in place. When no mask is supplied the entire buffer is
//! mirrored.
//!
//! Flips are lossless — no interpolation is applied.

use crate::canvas::buffer::PixelBuffer;
use crate::selection::SelectionMask;

use super::error::{Error, Result};

/// Mirrors `buf` left-to-right, optionally constrained to `mask`.
///
/// For each pixel pair `(x, width - 1 - x)`, the swap is performed
/// when at least one of the two pixels is selected. When neither is
/// selected the pair is left untouched. No mask is equivalent to all
/// pixels being selected (full mirror).
///
/// # Errors
///
/// Returns [`Error::EmptyBuffer`] if `buf` has zero dimensions.
pub fn flip_horizontal(buf: &PixelBuffer, mask: Option<&SelectionMask>) -> Result<PixelBuffer> {
    if buf.is_empty() {
        return Err(Error::EmptyBuffer);
    }
    let w = buf.width();
    let h = buf.height();
    let mut out = buf.clone();

    for y in 0..h {
        let mut x = 0u32;
        while x < w / 2 {
            let mirror_x = w - 1 - x;
            let left_sel = mask.is_none_or(|m| m.is_selected(x, y));
            let right_sel = mask.is_none_or(|m| m.is_selected(mirror_x, y));

            if left_sel || right_sel {
                let left_px = out.pixel(x, y).unwrap_or_default();
                let right_px = out.pixel(mirror_x, y).unwrap_or_default();
                out.set_pixel(x, y, right_px);
                out.set_pixel(mirror_x, y, left_px);
            }
            x += 1;
        }
    }

    Ok(out)
}

/// Mirrors `buf` top-to-bottom, optionally constrained to `mask`.
///
/// For each pixel pair `(x, y)` and `(x, height - 1 - y)`, the swap
/// is performed when at least one of the two pixels is selected.
/// Selection logic is identical to [`flip_horizontal`].
///
/// # Errors
///
/// Returns [`Error::EmptyBuffer`] if `buf` has zero dimensions.
pub fn flip_vertical(buf: &PixelBuffer, mask: Option<&SelectionMask>) -> Result<PixelBuffer> {
    if buf.is_empty() {
        return Err(Error::EmptyBuffer);
    }
    let w = buf.width();
    let h = buf.height();
    let mut out = buf.clone();

    let mut y = 0u32;
    while y < h / 2 {
        let mirror_y = h - 1 - y;
        for x in 0..w {
            let top_sel = mask.is_none_or(|m| m.is_selected(x, y));
            let bot_sel = mask.is_none_or(|m| m.is_selected(x, mirror_y));

            if top_sel || bot_sel {
                let top_px = out.pixel(x, y).unwrap_or_default();
                let bot_px = out.pixel(x, mirror_y).unwrap_or_default();
                out.set_pixel(x, y, bot_px);
                out.set_pixel(x, mirror_y, top_px);
            }
        }
        y += 1;
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
    fn transparent() -> Rgba {
        Rgba::transparent()
    }

    #[test]
    fn flip_h_two_pixels() {
        // [red, green] → [green, red]
        let mut buf = PixelBuffer::new(2, 1).unwrap();
        buf.set_pixel(0, 0, red());
        buf.set_pixel(1, 0, green());
        let out = flip_horizontal(&buf, None).unwrap();
        assert_eq!(out.pixel(0, 0), Some(green()));
        assert_eq!(out.pixel(1, 0), Some(red()));
    }

    #[test]
    fn flip_h_identity_on_odd_width() {
        // Center pixel of a 3-wide buffer should not move.
        let mut buf = PixelBuffer::new(3, 1).unwrap();
        buf.set_pixel(0, 0, red());
        buf.set_pixel(1, 0, green());
        buf.set_pixel(2, 0, red());
        let out = flip_horizontal(&buf, None).unwrap();
        assert_eq!(out.pixel(0, 0), Some(red()));
        assert_eq!(out.pixel(1, 0), Some(green())); // center unchanged
        assert_eq!(out.pixel(2, 0), Some(red()));
    }

    #[test]
    fn flip_h_twice_is_identity() {
        let mut buf = PixelBuffer::new(4, 4).unwrap();
        buf.set_pixel(0, 0, red());
        buf.set_pixel(3, 3, green());
        let flipped_once = flip_horizontal(&buf, None).unwrap();
        let flipped_twice = flip_horizontal(&flipped_once, None).unwrap();
        assert_eq!(flipped_twice, buf);
    }

    #[test]
    fn flip_v_two_rows() {
        let mut buf = PixelBuffer::new(1, 2).unwrap();
        buf.set_pixel(0, 0, red());
        buf.set_pixel(0, 1, green());
        let out = flip_vertical(&buf, None).unwrap();
        assert_eq!(out.pixel(0, 0), Some(green()));
        assert_eq!(out.pixel(0, 1), Some(red()));
    }

    #[test]
    fn flip_v_twice_is_identity() {
        let mut buf = PixelBuffer::new(4, 4).unwrap();
        buf.set_pixel(0, 0, red());
        buf.set_pixel(3, 3, green());
        let flipped_once = flip_vertical(&buf, None).unwrap();
        let flipped_twice = flip_vertical(&flipped_once, None).unwrap();
        assert_eq!(flipped_twice, buf);
    }

    #[test]
    fn flip_h_with_mask_selected_only() {
        // Buffer: [red, green, transparent]
        // Mask selects pixel 0 only.
        // Expected after flip: pixel 0 takes the mirror (pixel 2) value,
        // pixel 2 (unselected) takes the old pixel 0 value, pixel 1 unchanged.
        let mut buf = PixelBuffer::new(3, 1).unwrap();
        buf.set_pixel(0, 0, red());
        buf.set_pixel(1, 0, green());

        let mut mask = SelectionMask::new(3, 1).unwrap();
        mask.set(0, 0, 255);

        let out = flip_horizontal(&buf, Some(&mask)).unwrap();
        // pixel 0 selected, swapped with pixel 2 (unselected); both swap
        assert_eq!(out.pixel(0, 0), Some(transparent()));
        assert_eq!(out.pixel(2, 0), Some(red()));
        assert_eq!(out.pixel(1, 0), Some(green())); // untouched
    }

    #[test]
    fn flip_empty_errors() {
        let buf = PixelBuffer::empty();
        assert_eq!(flip_horizontal(&buf, None), Err(Error::EmptyBuffer));
        assert_eq!(flip_vertical(&buf, None), Err(Error::EmptyBuffer));
    }
}
