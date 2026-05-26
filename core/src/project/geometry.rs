//! Geometric primitives shared across the data model.
//!
//! Pixel coordinates are integers — pixel art does not have sub-pixel
//! positions in the document model. UI-side scrolling and zoom run in
//! floats, but those values live in the editor shell, not here.

use serde::{Deserialize, Serialize};

/// A 2D integer point. Negative coordinates are valid (cels can sit
/// off-canvas, references can be placed before the origin).
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct IVec2 {
    /// Horizontal coordinate.
    pub x: i32,
    /// Vertical coordinate.
    pub y: i32,
}

impl IVec2 {
    /// Constructs a point.
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// The origin `(0, 0)`.
    #[must_use]
    pub const fn zero() -> Self {
        Self::new(0, 0)
    }
}

/// A non-empty 2D extent in pixels.
///
/// Represented as `u32` so canvas dimensions are unambiguously
/// non-negative. A zero-width or zero-height canvas is rejected at the
/// editor layer; the type itself permits zero only because `Default`
/// must be defined.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Size {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Size {
    /// Constructs a size.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Returns `true` if either dimension is zero.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Total pixel count, computed without overflow because both
    /// dimensions are `u32` and the product is `u64`.
    #[must_use]
    pub const fn pixel_count(self) -> u64 {
        (self.width as u64) * (self.height as u64)
    }
}

/// An axis-aligned rectangle in integer coordinates.
///
/// `origin` is the top-left corner; `size` extends right and down.
/// Empty rects (zero width or height) are valid and represent
/// no-selection / no-coverage.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Rect {
    /// Top-left corner in canvas coordinates.
    pub origin: IVec2,
    /// Width and height of the rectangle.
    pub size: Size,
}

impl Rect {
    /// Constructs a rect from origin and size.
    #[must_use]
    pub const fn new(origin: IVec2, size: Size) -> Self {
        Self { origin, size }
    }

    /// Constructs a rect from raw `x, y, w, h` components.
    #[must_use]
    pub const fn from_xywh(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            origin: IVec2::new(x, y),
            size: Size::new(width, height),
        }
    }

    /// Returns `true` if the rect has zero width or zero height.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.size.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_pixel_count_avoids_overflow() {
        let s = Size::new(u32::MAX, 2);
        assert_eq!(s.pixel_count(), 2 * u64::from(u32::MAX));
    }

    #[test]
    fn empty_rect_detected() {
        assert!(Rect::from_xywh(0, 0, 0, 5).is_empty());
        assert!(Rect::from_xywh(0, 0, 5, 0).is_empty());
        assert!(!Rect::from_xywh(0, 0, 5, 5).is_empty());
    }
}
