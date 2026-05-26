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

use crate::project::{IVec2, Rect, Size};

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

/// Returns `width * height` as `usize`, or [`Error::DimensionOverflow`]
/// when the product exceeds `usize::MAX`. All `SelectionMask`
/// constructors and morphology helpers route through this helper so the
/// no-`unwrap`/no-`panic` rule from CLAUDE.md is enforced for every
/// allocation that scales with both dimensions.
pub(super) fn checked_area(width: u32, height: u32) -> Result<usize> {
    (width as usize).checked_mul(height as usize).ok_or(Error::DimensionOverflow { width, height })
}

impl SelectionMask {
    /// Empty mask with all pixels deselected.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DimensionOverflow`] when `width * height`
    /// exceeds `usize::MAX` (relevant on 32-bit hosts and at the
    /// extreme end of the 64-bit range).
    pub fn new(width: u32, height: u32) -> Result<Self> {
        Ok(Self {
            data: vec![0u8; checked_area(width, height)?],
            width,
            height,
        })
    }

    /// Fully selected mask — every pixel at coverage 255.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DimensionOverflow`] when `width * height`
    /// exceeds `usize::MAX`.
    pub fn full(width: u32, height: u32) -> Result<Self> {
        Ok(Self {
            data: vec![255u8; checked_area(width, height)?],
            width,
            height,
        })
    }

    /// Constructs from raw coverage bytes.
    ///
    /// `data.len()` must equal `width * height`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SizeMismatch`] if the byte count does not match.
    pub fn from_raw(width: u32, height: u32, data: Vec<u8>) -> Result<Self> {
        let expected = checked_area(width, height)?;
        if data.len() != expected {
            // The buffer is flat — there's no real "actual width × actual height"
            // pair to report. Hold height fixed and infer width from the byte
            // count so the message at least gives the caller a hint at where
            // the mismatch came from. height.max(1) avoids div-by-zero when
            // the caller passes a zero-sized mask with non-empty data.
            return Err(Error::SizeMismatch {
                expected_w: width,
                expected_h: height,
                actual_w: u32::try_from(data.len()).unwrap_or(u32::MAX) / height.max(1),
                actual_h: height,
            });
        }
        Ok(Self { data, width, height })
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

    /// `true` when the mask covers zero pixels, i.e. either dimension is zero.
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
        self.data.get((y as usize) * (self.width as usize) + (x as usize)).copied()
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

    /// Returns the smallest axis-aligned rectangle that encloses every
    /// pixel with non-zero coverage.
    ///
    /// Returns `Rect::from_xywh(0, 0, 0, 0)` for an empty mask (no
    /// pixels selected). The returned rect is in mask-local coordinates;
    /// callers that store masks at canvas-scale can use it directly,
    /// callers that store cel-local masks should offset by the cel
    /// position.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        // Sentinel-style scan: hold extremes that will be overwritten on
        // the first hit. If `min_x` is still `u32::MAX` at the end, no
        // pixel was selected.
        let mut min_x = u32::MAX;
        let mut min_y = u32::MAX;
        let mut max_x = 0u32;
        let mut max_y = 0u32;
        for y in 0..self.height {
            for x in 0..self.width {
                let idx = (y as usize) * (self.width as usize) + (x as usize);
                if self.data.get(idx).copied().unwrap_or(0) > 0 {
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }
        if min_x == u32::MAX {
            return Rect::from_xywh(0, 0, 0, 0);
        }
        // +1 because max_x/max_y are inclusive pixel indices, but Size is
        // pixel count. i32::try_from is safe — `width` and `height` are u32
        // bounded by canvas dimensions, well below i32::MAX in practice.
        Rect {
            origin: IVec2::new(i32::try_from(min_x).unwrap_or(0), i32::try_from(min_y).unwrap_or(0)),
            size: Size::new(max_x - min_x + 1, max_y - min_y + 1),
        }
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
        let data = self.data.iter().zip(other.data.iter()).map(|(&a, &b)| a.max(b)).collect();
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
        let data = self.data.iter().zip(other.data.iter()).map(|(&a, &b)| a.min(b)).collect();
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
        let data = self.data.iter().zip(other.data.iter()).map(|(&a, &b)| a.saturating_sub(b)).collect();
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
        let data = self.data.iter().zip(other.data.iter()).map(|(&a, &b)| a.abs_diff(b)).collect();
        Ok(Self {
            data,
            width: self.width,
            height: self.height,
        })
    }

    /// Returns the unit grid-line segments tracing the boundary of the selected
    /// region: every pixel edge where a selected pixel meets an unselected one
    /// (or the canvas edge). Endpoints are integer grid coordinates in canvas
    /// space, so an editor can map them straight to screen space and draw
    /// marching ants along the real silhouette — including holes and
    /// concavities — instead of a bounding box.
    #[must_use]
    pub fn boundary_segments(&self) -> Vec<[(i32, i32); 2]> {
        let mut out = Vec::new();
        for y in 0..self.height {
            for x in 0..self.width {
                if !self.is_selected(x, y) {
                    continue;
                }
                // i32 casts are safe: canvas dimensions are well below i32::MAX.
                let xi = i32::try_from(x).unwrap_or(i32::MAX);
                let yi = i32::try_from(y).unwrap_or(i32::MAX);
                if y == 0 || !self.is_selected(x, y - 1) {
                    out.push([(xi, yi), (xi + 1, yi)]);
                }
                if y + 1 >= self.height || !self.is_selected(x, y + 1) {
                    out.push([(xi, yi + 1), (xi + 1, yi + 1)]);
                }
                if x == 0 || !self.is_selected(x - 1, y) {
                    out.push([(xi, yi), (xi, yi + 1)]);
                }
                if x + 1 >= self.width || !self.is_selected(x + 1, y) {
                    out.push([(xi + 1, yi), (xi + 1, yi + 1)]);
                }
            }
        }
        out
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

// Default builds the empty (0×0) mask directly. We don't `derive` it so
// the constructor stays infallible — `SelectionMask::new` returns
// Result<Self, Error::DimensionOverflow>, but 0×0 can't overflow, so the
// derived Default would have to call `.unwrap()` outside test code.
#[allow(clippy::derivable_impls)]
impl Default for SelectionMask {
    fn default() -> Self {
        Self {
            data: Vec::new(),
            width: 0,
            height: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_mask_all_zero() {
        let m = SelectionMask::new(4, 4).unwrap();
        assert_eq!(m.selected_count(), 0);
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(m.get(x, y), Some(0));
            }
        }
    }

    #[test]
    fn full_mask_all_255() {
        let m = SelectionMask::full(4, 4).unwrap();
        assert_eq!(m.selected_count(), 16);
        for y in 0..4 {
            for x in 0..4 {
                assert!(m.is_fully_selected(x, y));
            }
        }
    }

    #[test]
    fn get_out_of_bounds_returns_none() {
        let m = SelectionMask::new(4, 4).unwrap();
        assert_eq!(m.get(4, 0), None);
        assert_eq!(m.get(0, 4), None);
    }

    #[test]
    fn set_and_get_round_trip() {
        let mut m = SelectionMask::new(4, 4).unwrap();
        m.set(1, 2, 128);
        assert_eq!(m.get(1, 2), Some(128));
        assert!(m.is_selected(1, 2));
        assert!(!m.is_fully_selected(1, 2));
    }

    #[test]
    fn bounds_empty_mask_returns_zero_rect() {
        let m = SelectionMask::new(8, 8).unwrap();
        assert_eq!(m.bounds(), Rect::from_xywh(0, 0, 0, 0));
    }

    #[test]
    fn bounds_full_mask_covers_entire_canvas() {
        let m = SelectionMask::full(4, 5).unwrap();
        assert_eq!(m.bounds(), Rect::from_xywh(0, 0, 4, 5));
    }

    #[test]
    fn bounds_single_pixel_is_one_by_one() {
        let mut m = SelectionMask::new(8, 8).unwrap();
        m.set(3, 5, 255);
        assert_eq!(m.bounds(), Rect::from_xywh(3, 5, 1, 1));
    }

    #[test]
    fn bounds_l_shape_returns_minimum_enclosing_rect() {
        // Two opposite corners selected: (1, 2) and (5, 6). Bounds should
        // be the rect that contains both, not anything tighter.
        let mut m = SelectionMask::new(8, 8).unwrap();
        m.set(1, 2, 255);
        m.set(5, 6, 255);
        assert_eq!(m.bounds(), Rect::from_xywh(1, 2, 5, 5));
    }

    #[test]
    fn invert_produces_complement() {
        let mut m = SelectionMask::new(2, 1).unwrap();
        m.set(0, 0, 0);
        m.set(1, 0, 255);
        let inv = m.invert();
        assert_eq!(inv.get(0, 0), Some(255));
        assert_eq!(inv.get(1, 0), Some(0));
    }

    #[test]
    fn union_takes_max() {
        let mut a = SelectionMask::new(2, 1).unwrap();
        a.set(0, 0, 100);
        a.set(1, 0, 50);
        let mut b = SelectionMask::new(2, 1).unwrap();
        b.set(0, 0, 50);
        b.set(1, 0, 200);
        let u = a.union(&b).unwrap();
        assert_eq!(u.get(0, 0), Some(100));
        assert_eq!(u.get(1, 0), Some(200));
    }

    #[test]
    fn intersect_takes_min() {
        let mut a = SelectionMask::new(2, 1).unwrap();
        a.set(0, 0, 100);
        a.set(1, 0, 255);
        let mut b = SelectionMask::new(2, 1).unwrap();
        b.set(0, 0, 50);
        b.set(1, 0, 128);
        let i = a.intersect(&b).unwrap();
        assert_eq!(i.get(0, 0), Some(50));
        assert_eq!(i.get(1, 0), Some(128));
    }

    #[test]
    fn subtract_saturates_at_zero() {
        let mut a = SelectionMask::new(2, 1).unwrap();
        a.set(0, 0, 100);
        a.set(1, 0, 50);
        let mut b = SelectionMask::new(2, 1).unwrap();
        b.set(0, 0, 255);
        b.set(1, 0, 10);
        let s = a.subtract(&b).unwrap();
        assert_eq!(s.get(0, 0), Some(0));
        assert_eq!(s.get(1, 0), Some(40));
    }

    #[test]
    fn xor_abs_diff() {
        let mut a = SelectionMask::new(2, 1).unwrap();
        a.set(0, 0, 200);
        a.set(1, 0, 100);
        let mut b = SelectionMask::new(2, 1).unwrap();
        b.set(0, 0, 50);
        b.set(1, 0, 200);
        let x = a.xor(&b).unwrap();
        assert_eq!(x.get(0, 0), Some(150));
        assert_eq!(x.get(1, 0), Some(100));
    }

    #[test]
    fn size_mismatch_returns_error() {
        let a = SelectionMask::full(4, 4).unwrap();
        let b = SelectionMask::full(8, 8).unwrap();
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

    #[test]
    fn boundary_of_single_pixel_is_four_edges() {
        let mut m = SelectionMask::new(8, 8).unwrap();
        m.set(3, 4, 255);
        let segs = m.boundary_segments();
        assert_eq!(segs.len(), 4);
        // The four unit edges around pixel (3,4).
        assert!(segs.contains(&[(3, 4), (4, 4)]));
        assert!(segs.contains(&[(3, 5), (4, 5)]));
        assert!(segs.contains(&[(3, 4), (3, 5)]));
        assert!(segs.contains(&[(4, 4), (4, 5)]));
    }

    #[test]
    fn boundary_of_two_pixel_run_shares_no_inner_edge() {
        // A 2x1 horizontal run has 6 boundary edges (the shared inner edge is
        // not on the boundary).
        let mut m = SelectionMask::new(8, 8).unwrap();
        m.set(2, 2, 255);
        m.set(3, 2, 255);
        assert_eq!(m.boundary_segments().len(), 6);
    }

    #[test]
    fn boundary_of_full_mask_is_the_perimeter() {
        let m = SelectionMask::full(4, 5).unwrap();
        // A filled w x h region's outline is its rectangle perimeter: 2*(w+h).
        assert_eq!(m.boundary_segments().len(), 2 * (4 + 5));
    }

    #[test]
    fn boundary_traces_a_hole() {
        // 3x3 selected with the centre cleared: outer ring (4 edges/side -> the
        // 3x3 perimeter is 12) plus the 4 edges around the hole = 16.
        let mut m = SelectionMask::full(3, 3).unwrap();
        m.set(1, 1, 0);
        assert_eq!(m.boundary_segments().len(), 12 + 4);
    }

    #[test]
    fn empty_mask_has_no_boundary() {
        let m = SelectionMask::new(8, 8).unwrap();
        assert!(m.boundary_segments().is_empty());
    }
}
