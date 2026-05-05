//! The `SelectionMask` type and its boolean set operations.
//!
//! A `SelectionMask` is a flat, one-byte-per-pixel coverage map aligned
//! to the canvas dimensions. `0` means not selected; `255` means fully
//! selected. Values in between arise from feathering and represent
//! partial inclusion — most tools treat `> 0` as "in the selection".
//!
//! Boolean operations (union, intersect, subtract, xor) require both
//! operands to have identical dimensions; they return [`Error::SizeMismatch`]
//! otherwise.

use super::error::{Error, Result};

/// Per-pixel selection coverage map.
///
/// Storage is a flat `Vec<u8>` in row-major order. Index for pixel
/// `(x, y)` is `y * width + x`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionMask {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

impl SelectionMask {
    /// Empty mask with all pixels deselected.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            data: vec![0u8; (width as usize) * (height as usize)],
            width,
            height,
        }
    }

    /// Fully selected mask — every pixel at coverage 255.
    #[must_use]
    pub fn full(width: u32, height: u32) -> Self {
        Self {
            data: vec![255u8; (width as usize) * (height as usize)],
            width,
            height,
        }
    }

    /// Constructs from raw coverage bytes.
    ///
    /// `data.len()` must equal `width * height`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SizeMismatch`] if the byte count does not match.
    pub fn from_raw(width: u32, height: u32, data: Vec<u8>) -> Result<Self> {
        let expected = (width as usize) * (height as usize);
        if data.len() != expected {
            return Err(Error::SizeMismatch {
                expected_w: width,
                expected_h: height,
                actual_w: width,
                actual_h: u32::try_from(data.len()).unwrap_or(u32::MAX) / height.max(1),
            });
        }
        Ok(Self {
            data,
            width,
            height,
        })
    }

    /// Width of the canvas this mask covers.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Height of the canvas this mask covers.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// `true` if both dimensions are zero.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Coverage byte for pixel `(x, y)`, or `None` if out of bounds.
    #[must_use]
    pub fn get(&self, x: u32, y: u32) -> Option<u8> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.data
            .get((y as usize) * (self.width as usize) + (x as usize))
            .copied()
    }

    /// Writes `value` at `(x, y)`. No-op for out-of-bounds coordinates.
    pub fn set(&mut self, x: u32, y: u32, value: u8) {
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        if let Some(slot) = self.data.get_mut(idx) {
            *slot = value;
        }
    }

    /// `true` if the pixel at `(x, y)` has any coverage (`> 0`).
    #[must_use]
    pub fn is_selected(&self, x: u32, y: u32) -> bool {
        self.get(x, y).unwrap_or(0) > 0
    }

    /// `true` if the pixel at `(x, y)` is fully selected (`== 255`).
    #[must_use]
    pub fn is_fully_selected(&self, x: u32, y: u32) -> bool {
        self.get(x, y).unwrap_or(0) == 255
    }

    /// Number of pixels with any coverage (`> 0`).
    #[must_use]
    pub fn selected_count(&self) -> u32 {
        u32::try_from(self.data.iter().filter(|&&v| v > 0).count()).unwrap_or(u32::MAX)
    }

    /// Borrows the raw coverage bytes in row-major order.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Returns a new mask with coverage values inverted (`255 - v`).
    #[must_use]
    pub fn invert(&self) -> Self {
        let inverted = self.data.iter().map(|&v| 255u8 - v).collect();
        Self {
            data: inverted,
            width: self.width,
            height: self.height,
        }
    }

    /// Returns a new mask that is the union of `self` and `other`.
    ///
    /// Each pixel takes the maximum coverage of the two masks. Equivalent
    /// to a logical OR on binary selections.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SizeMismatch`] if the masks differ in size.
    pub fn union(&self, other: &Self) -> Result<Self> {
        self.check_same_size(other)?;
        let data = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(&a, &b)| a.max(b))
            .collect();
        Ok(Self {
            data,
            width: self.width,
            height: self.height,
        })
    }

    /// Returns a new mask that is the intersection of `self` and `other`.
    ///
    /// Each pixel takes the minimum coverage of the two masks. Equivalent
    /// to a logical AND on binary selections.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SizeMismatch`] if the masks differ in size.
    pub fn intersect(&self, other: &Self) -> Result<Self> {
        self.check_same_size(other)?;
        let data = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(&a, &b)| a.min(b))
            .collect();
        Ok(Self {
            data,
            width: self.width,
            height: self.height,
        })
    }

    /// Returns a new mask that subtracts `other` from `self`.
    ///
    /// Each pixel's coverage is reduced by `other`'s coverage, clamping
    /// at zero. Pixels fully covered by `other` become unselected.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SizeMismatch`] if the masks differ in size.
    pub fn subtract(&self, other: &Self) -> Result<Self> {
        self.check_same_size(other)?;
        let data = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(&a, &b)| a.saturating_sub(b))
            .collect();
        Ok(Self {
            data,
            width: self.width,
            height: self.height,
        })
    }

    /// Returns a new mask that is the symmetric difference of `self` and
    /// `other`.
    ///
    /// Each pixel takes the absolute difference of the two coverage
    /// values. On binary masks this selects pixels that are in exactly
    /// one of the two masks.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SizeMismatch`] if the masks differ in size.
    pub fn xor(&self, other: &Self) -> Result<Self> {
        self.check_same_size(other)?;
        let data = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(&a, &b)| a.abs_diff(b))
            .collect();
        Ok(Self {
            data,
            width: self.width,
            height: self.height,
        })
    }

    fn check_same_size(&self, other: &Self) -> Result<()> {
        if self.width != other.width || self.height != other.height {
            return Err(Error::SizeMismatch {
                expected_w: self.width,
                expected_h: self.height,
                actual_w: other.width,
                actual_h: other.height,
            });
        }
        Ok(())
    }
}

impl Default for SelectionMask {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_mask_all_zero() {
        let m = SelectionMask::new(4, 4);
        assert_eq!(m.selected_count(), 0);
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(m.get(x, y), Some(0));
            }
        }
    }

    #[test]
    fn full_mask_all_255() {
        let m = SelectionMask::full(4, 4);
        assert_eq!(m.selected_count(), 16);
        for y in 0..4 {
            for x in 0..4 {
                assert!(m.is_fully_selected(x, y));
            }
        }
    }

    #[test]
    fn get_out_of_bounds_returns_none() {
        let m = SelectionMask::new(4, 4);
        assert_eq!(m.get(4, 0), None);
        assert_eq!(m.get(0, 4), None);
    }

    #[test]
    fn set_and_get_round_trip() {
        let mut m = SelectionMask::new(4, 4);
        m.set(1, 2, 128);
        assert_eq!(m.get(1, 2), Some(128));
        assert!(m.is_selected(1, 2));
        assert!(!m.is_fully_selected(1, 2));
    }

    #[test]
    fn invert_produces_complement() {
        let mut m = SelectionMask::new(2, 1);
        m.set(0, 0, 0);
        m.set(1, 0, 255);
        let inv = m.invert();
        assert_eq!(inv.get(0, 0), Some(255));
        assert_eq!(inv.get(1, 0), Some(0));
    }

    #[test]
    fn union_takes_max() {
        let mut a = SelectionMask::new(2, 1);
        a.set(0, 0, 100);
        a.set(1, 0, 50);
        let mut b = SelectionMask::new(2, 1);
        b.set(0, 0, 50);
        b.set(1, 0, 200);
        let u = a.union(&b).unwrap();
        assert_eq!(u.get(0, 0), Some(100));
        assert_eq!(u.get(1, 0), Some(200));
    }

    #[test]
    fn intersect_takes_min() {
        let mut a = SelectionMask::new(2, 1);
        a.set(0, 0, 100);
        a.set(1, 0, 255);
        let mut b = SelectionMask::new(2, 1);
        b.set(0, 0, 50);
        b.set(1, 0, 128);
        let i = a.intersect(&b).unwrap();
        assert_eq!(i.get(0, 0), Some(50));
        assert_eq!(i.get(1, 0), Some(128));
    }

    #[test]
    fn subtract_saturates_at_zero() {
        let mut a = SelectionMask::new(2, 1);
        a.set(0, 0, 100);
        a.set(1, 0, 50);
        let mut b = SelectionMask::new(2, 1);
        b.set(0, 0, 255);
        b.set(1, 0, 10);
        let s = a.subtract(&b).unwrap();
        assert_eq!(s.get(0, 0), Some(0));
        assert_eq!(s.get(1, 0), Some(40));
    }

    #[test]
    fn xor_abs_diff() {
        let mut a = SelectionMask::new(2, 1);
        a.set(0, 0, 200);
        a.set(1, 0, 100);
        let mut b = SelectionMask::new(2, 1);
        b.set(0, 0, 50);
        b.set(1, 0, 200);
        let x = a.xor(&b).unwrap();
        assert_eq!(x.get(0, 0), Some(150));
        assert_eq!(x.get(1, 0), Some(100));
    }

    #[test]
    fn size_mismatch_returns_error() {
        let a = SelectionMask::full(4, 4);
        let b = SelectionMask::full(8, 8);
        assert!(a.union(&b).is_err());
        assert!(a.intersect(&b).is_err());
        assert!(a.subtract(&b).is_err());
        assert!(a.xor(&b).is_err());
    }

    #[test]
    fn from_raw_validates_length() {
        let ok = SelectionMask::from_raw(2, 2, vec![0, 1, 2, 3]);
        assert!(ok.is_ok());
        let bad = SelectionMask::from_raw(2, 2, vec![0, 1]);
        assert!(bad.is_err());
    }
}
