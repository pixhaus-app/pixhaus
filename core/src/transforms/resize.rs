//! Canvas resize: change the buffer's dimensions without scaling pixels.
//!
//! Growing the canvas pads with transparent pixels; shrinking it crops. A
//! 9-point [`CanvasAnchor`] decides where the existing content sits in the new
//! extent. This is distinct from [`super::scale`], which resamples the pixels
//! to a new size — resize keeps every pixel at its original scale and only
//! moves the canvas border.

use crate::canvas::buffer::PixelBuffer;

use super::error::{Error, Result};

/// Where existing content anchors when the canvas is resized.
///
/// The default is [`CanvasAnchor::Center`]: content stays centred, with
/// padding or cropping split evenly across opposite edges.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum CanvasAnchor {
    /// Pin to the top-left corner.
    TopLeft,
    /// Pin to the top edge, centred horizontally.
    Top,
    /// Pin to the top-right corner.
    TopRight,
    /// Pin to the left edge, centred vertically.
    Left,
    /// Keep content centred (default).
    #[default]
    Center,
    /// Pin to the right edge, centred vertically.
    Right,
    /// Pin to the bottom-left corner.
    BottomLeft,
    /// Pin to the bottom edge, centred horizontally.
    Bottom,
    /// Pin to the bottom-right corner.
    BottomRight,
}

impl CanvasAnchor {
    /// The `(horizontal, vertical)` fractions the anchor places content at,
    /// each `0` (start edge), `1` (centre), or `2` (far edge). The offset that
    /// positions `old` inside `new` is `(new - old) * fraction / 2`.
    const fn fractions(self) -> (i64, i64) {
        match self {
            Self::TopLeft => (0, 0),
            Self::Top => (1, 0),
            Self::TopRight => (2, 0),
            Self::Left => (0, 1),
            Self::Center => (1, 1),
            Self::Right => (2, 1),
            Self::BottomLeft => (0, 2),
            Self::Bottom => (1, 2),
            Self::BottomRight => (2, 2),
        }
    }
}

/// Resizes `buf` to `(new_w, new_h)`, anchoring existing content per `anchor`.
///
/// Pixels keep their original scale: the overlap between the old and new
/// extents is copied verbatim, padding is transparent, and anything outside the
/// new extent is dropped. Padding/cropping is distributed according to
/// `anchor`.
///
/// # Errors
///
/// - [`Error::EmptyBuffer`] if `buf` is 0×0.
/// - [`Error::DimensionOverflow`] if `new_w` or `new_h` is zero.
/// - [`Error::Buffer`] if the output allocation fails.
pub fn resize_canvas(buf: &PixelBuffer, new_w: u32, new_h: u32, anchor: CanvasAnchor) -> Result<PixelBuffer> {
    if buf.is_empty() {
        return Err(Error::EmptyBuffer);
    }
    if new_w == 0 || new_h == 0 {
        return Err(Error::DimensionOverflow);
    }

    let old_w = i64::from(buf.width());
    let old_h = i64::from(buf.height());
    let (fx, fy) = anchor.fractions();
    // Offset of the old origin inside the new canvas. Negative when shrinking
    // toward the far edge — that's a crop, handled by the bounds check below.
    let off_x = (i64::from(new_w) - old_w) * fx / 2;
    let off_y = (i64::from(new_h) - old_h) * fy / 2;

    let mut out = PixelBuffer::new(new_w, new_h)?;
    for sy in 0..buf.height() {
        let dy = off_y + i64::from(sy);
        if dy < 0 || dy >= i64::from(new_h) {
            continue;
        }
        for sx in 0..buf.width() {
            let dx = off_x + i64::from(sx);
            if dx < 0 || dx >= i64::from(new_w) {
                continue;
            }
            if let Some(px) = buf.pixel(sx, sy) {
                // The bounds checks above guarantee 0 <= dx < new_w and
                // 0 <= dy < new_h, so both casts are exact and non-negative.
                #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                out.set_pixel(dx as u32, dy as u32, px);
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

    #[test]
    fn grow_top_left_keeps_origin() {
        let mut buf = PixelBuffer::new(2, 2).unwrap();
        buf.set_pixel(0, 0, red());
        let out = resize_canvas(&buf, 4, 4, CanvasAnchor::TopLeft).unwrap();
        assert_eq!((out.width(), out.height()), (4, 4));
        assert_eq!(out.pixel(0, 0), Some(red()));
        assert_eq!(out.pixel(3, 3), Some(Rgba::transparent()));
    }

    #[test]
    fn grow_bottom_right_shifts_content() {
        let mut buf = PixelBuffer::new(2, 2).unwrap();
        buf.set_pixel(0, 0, red());
        let out = resize_canvas(&buf, 4, 4, CanvasAnchor::BottomRight).unwrap();
        // Old origin moves to (2, 2); the painted pixel lands there.
        assert_eq!(out.pixel(2, 2), Some(red()));
        assert_eq!(out.pixel(0, 0), Some(Rgba::transparent()));
    }

    #[test]
    fn grow_center_splits_padding() {
        let buf = PixelBuffer::filled(2, 2, red()).unwrap();
        let out = resize_canvas(&buf, 4, 4, CanvasAnchor::Center).unwrap();
        // Content occupies the centre 2×2 block (offset 1,1).
        assert_eq!(out.pixel(0, 0), Some(Rgba::transparent()));
        assert_eq!(out.pixel(1, 1), Some(red()));
        assert_eq!(out.pixel(2, 2), Some(red()));
        assert_eq!(out.pixel(3, 3), Some(Rgba::transparent()));
    }

    #[test]
    fn shrink_top_left_crops_far_edges() {
        let mut buf = PixelBuffer::new(4, 4).unwrap();
        buf.set_pixel(0, 0, red());
        buf.set_pixel(3, 3, red());
        let out = resize_canvas(&buf, 2, 2, CanvasAnchor::TopLeft).unwrap();
        assert_eq!((out.width(), out.height()), (2, 2));
        assert_eq!(out.pixel(0, 0), Some(red())); // kept
        // The (3,3) pixel is cropped away — the only opaque pixel is (0,0).
        let opaque = out.pixels().filter(|p| p.a != 0).count();
        assert_eq!(opaque, 1);
    }

    #[test]
    fn identity_size_round_trips() {
        let mut buf = PixelBuffer::new(3, 3).unwrap();
        buf.set_pixel(1, 1, red());
        let out = resize_canvas(&buf, 3, 3, CanvasAnchor::Center).unwrap();
        assert_eq!(out, buf);
    }

    #[test]
    fn empty_and_zero_target_error() {
        assert_eq!(resize_canvas(&PixelBuffer::empty(), 4, 4, CanvasAnchor::Center), Err(Error::EmptyBuffer));
        let buf = PixelBuffer::new(2, 2).unwrap();
        assert_eq!(resize_canvas(&buf, 0, 4, CanvasAnchor::Center), Err(Error::DimensionOverflow));
    }
}
