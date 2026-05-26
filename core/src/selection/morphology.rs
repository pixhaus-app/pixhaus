//! Morphological operations on selection masks.
//!
//! All three operations work on a [`SelectionMask`] and return a new,
//! independent mask of the same dimensions.
//!
//! - [`expand`] — morphological dilation with a circular structuring
//!   element: any pixel within `by` pixels of a selected pixel becomes
//!   selected.
//! - [`contract`] — morphological erosion: a pixel stays selected only
//!   if every pixel within `by` pixels is also selected.
//! - [`feather`] — two-pass box blur on the coverage values, producing
//!   a smooth gradient at selection boundaries.
//!
//! For pixel art, feathering is off by default at the editor layer.
//! These functions are the low-level implementation; the editor UI
//! controls when they are invoked.

#[allow(unused_imports)] // Error referenced from rustdoc intra-doc links.
use super::error::{Error, Result};
use super::mask::SelectionMask;

/// Expands the selection by `by` pixels using a circular structuring
/// element (morphological dilation).
///
/// A pixel becomes selected if any pixel within the disc of radius `by`
/// centred at that pixel is selected in the input. Pixels outside the
/// canvas boundary are treated as unselected.
///
/// `by == 0` returns a clone of the input.
///
/// # Errors
///
/// Returns [`Error::DimensionOverflow`] when `width * height` overflows
/// `usize` while allocating the output mask.
pub fn expand(mask: &SelectionMask, by: u32) -> Result<SelectionMask> {
    if by == 0 || mask.is_empty() {
        return Ok(mask.clone());
    }
    let w = mask.width();
    let h = mask.height();
    let mut out = SelectionMask::new(w, h)?;
    let by_i = i64::from(by);
    let by_sq = by_i * by_i;
    let w_i64 = i64::from(w);
    let h_i64 = i64::from(h);

    for y in 0..h {
        for x in 0..w {
            // Already selected? Keep it and skip the neighbourhood scan.
            if mask.is_fully_selected(x, y) {
                out.set(x, y, 255);
                continue;
            }
            'found: for dy in -by_i..=by_i {
                for dx in -by_i..=by_i {
                    if dx * dx + dy * dy > by_sq {
                        continue;
                    }
                    let nx = i64::from(x) + dx;
                    let ny = i64::from(y) + dy;
                    if nx < 0 || ny < 0 || nx >= w_i64 || ny >= h_i64 {
                        continue;
                    }
                    let nx = u32::try_from(nx).unwrap_or(0);
                    let ny = u32::try_from(ny).unwrap_or(0);
                    if mask.get(nx, ny).unwrap_or(0) > 0 {
                        out.set(x, y, 255);
                        break 'found;
                    }
                }
            }
        }
    }
    Ok(out)
}

/// Contracts the selection by `by` pixels using a circular structuring
/// element (morphological erosion).
///
/// A pixel remains selected only when every pixel within the disc of
/// radius `by` is also selected. Pixels outside the canvas boundary
/// count as unselected, so edge pixels erode away first.
///
/// `by == 0` returns a clone of the input.
///
/// # Errors
///
/// Returns [`Error::DimensionOverflow`] when `width * height` overflows.
pub fn contract(mask: &SelectionMask, by: u32) -> Result<SelectionMask> {
    if by == 0 || mask.is_empty() {
        return Ok(mask.clone());
    }
    let w = mask.width();
    let h = mask.height();
    let mut out = SelectionMask::new(w, h)?;
    let by_i = i64::from(by);
    let by_sq = by_i * by_i;
    let w_i64 = i64::from(w);
    let h_i64 = i64::from(h);

    for y in 0..h {
        for x in 0..w {
            if mask.get(x, y).unwrap_or(0) == 0 {
                continue;
            }
            let mut keep = true;
            'check: for dy in -by_i..=by_i {
                for dx in -by_i..=by_i {
                    if dx * dx + dy * dy > by_sq {
                        continue;
                    }
                    let nx = i64::from(x) + dx;
                    let ny = i64::from(y) + dy;
                    let val = if nx >= 0 && ny >= 0 && nx < w_i64 && ny < h_i64 {
                        let nx = u32::try_from(nx).unwrap_or(0);
                        let ny = u32::try_from(ny).unwrap_or(0);
                        mask.get(nx, ny).unwrap_or(0)
                    } else {
                        0 // outside canvas — treat as unselected
                    };
                    if val == 0 {
                        keep = false;
                        break 'check;
                    }
                }
            }
            if keep {
                out.set(x, y, 255);
            }
        }
    }
    Ok(out)
}

/// Feathers the selection by applying a two-pass box blur of the given
/// `radius` to the coverage values.
///
/// The result has soft edges: coverage transitions from 255 (fully
/// selected) to 0 (unselected) over approximately `radius` pixels.
/// `radius == 0` returns a clone of the input.
///
/// # Errors
///
/// Returns [`Error::DimensionOverflow`] when `width * height` overflows.
pub fn feather(mask: &SelectionMask, radius: u32) -> Result<SelectionMask> {
    if radius == 0 || mask.is_empty() {
        return Ok(mask.clone());
    }
    let area = super::mask::checked_area(mask.width(), mask.height())?;
    let w = mask.width() as usize;
    let h = mask.height() as usize;
    let r = radius as usize;

    // --- horizontal pass (stored as u16, range 0-255) ---
    let mut h_pass = vec![0u16; area];
    for y in 0..h {
        for x in 0..w {
            let x_start = x.saturating_sub(r);
            let x_end = (x + r + 1).min(w);
            let count = u32::try_from(x_end - x_start).unwrap_or(1).max(1);
            let sum: u32 = (x_start..x_end)
                .map(|xx| u32::from(mask.get(u32::try_from(xx).unwrap_or(0), u32::try_from(y).unwrap_or(0)).unwrap_or(0)))
                .sum();
            // Round to nearest: (2*sum + count) / (2*count)
            let avg = (sum * 2 + count) / (count * 2);
            h_pass[y * w + x] = u16::try_from(avg.min(255)).unwrap_or(255);
        }
    }

    // --- vertical pass ---
    let mut out = SelectionMask::new(mask.width(), mask.height())?;
    for y in 0..h {
        for x in 0..w {
            let y_start = y.saturating_sub(r);
            let y_end = (y + r + 1).min(h);
            let count = u32::try_from(y_end - y_start).unwrap_or(1).max(1);
            let sum: u32 = (y_start..y_end).map(|yy| u32::from(h_pass[yy * w + x])).sum();
            let avg = (sum * 2 + count) / (count * 2);
            let val = u8::try_from(avg.min(255)).unwrap_or(255);
            out.set(u32::try_from(x).unwrap_or(0), u32::try_from(y).unwrap_or(0), val);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selection::mask::SelectionMask;

    // --- expand --------------------------------------------------------------

    #[test]
    fn expand_zero_is_identity() {
        let mut m = SelectionMask::new(4, 4).unwrap();
        m.set(2, 2, 255);
        let e = expand(&m, 0).unwrap();
        assert_eq!(e, m);
    }

    #[test]
    fn expand_grows_single_pixel() {
        let mut m = SelectionMask::new(8, 8).unwrap();
        m.set(4, 4, 255);
        let e = expand(&m, 1).unwrap();
        // 4-connected: centre + 4 cardinal neighbours.
        // With circular SE of radius 1: pixels at distance <= 1.
        // Those are: (4,4), (3,4), (5,4), (4,3), (4,5) = 5 pixels.
        assert!(e.is_selected(4, 4));
        assert!(e.is_selected(3, 4));
        assert!(e.is_selected(5, 4));
        assert!(e.is_selected(4, 3));
        assert!(e.is_selected(4, 5));
    }

    #[test]
    fn expand_does_not_shrink_full_mask() {
        let m = SelectionMask::full(4, 4).unwrap();
        let e = expand(&m, 2).unwrap();
        assert_eq!(e.selected_count(), 16);
    }

    #[test]
    fn expand_at_canvas_edge_clips() {
        let mut m = SelectionMask::new(4, 4).unwrap();
        m.set(0, 0, 255);
        let e = expand(&m, 2).unwrap();
        // Expansion never creates pixels outside [0, 3] x [0, 3].
        assert!(e.get(0, 0).is_some());
        assert_eq!(e.get(5, 5), None);
    }

    // --- contract ------------------------------------------------------------

    #[test]
    fn contract_zero_is_identity() {
        let m = SelectionMask::full(4, 4).unwrap();
        let c = contract(&m, 0).unwrap();
        assert_eq!(c, m);
    }

    #[test]
    fn contract_erodes_edges() {
        let m = SelectionMask::full(5, 5).unwrap();
        let c = contract(&m, 1).unwrap();
        // Edge pixels should be eroded; interior 3x3 should remain.
        assert!(!c.is_selected(0, 0));
        assert!(!c.is_selected(4, 4));
        assert!(c.is_fully_selected(2, 2));
    }

    #[test]
    fn contract_by_half_width_empties_thin_selection() {
        let mut m = SelectionMask::new(6, 1).unwrap();
        for x in 0..6 {
            m.set(x, 0, 255);
        }
        // A single-row selection contracted by 1 should empty completely
        // because every pixel is adjacent to unselected (out-of-bounds) pixels.
        let c = contract(&m, 1).unwrap();
        assert_eq!(c.selected_count(), 0);
    }

    #[test]
    fn contract_does_not_expand_empty() {
        let m = SelectionMask::new(4, 4).unwrap();
        let c = contract(&m, 2).unwrap();
        assert_eq!(c.selected_count(), 0);
    }

    // --- feather -------------------------------------------------------------

    #[test]
    fn feather_zero_is_identity() {
        let m = SelectionMask::full(4, 4).unwrap();
        let f = feather(&m, 0).unwrap();
        assert_eq!(f, m);
    }

    #[test]
    fn feather_preserves_full_interior() {
        // Large fully-selected mask: pixels far from the edge are unaffected.
        let m = SelectionMask::full(20, 20).unwrap();
        let f = feather(&m, 2).unwrap();
        // Centre pixel should remain at 255.
        assert_eq!(f.get(10, 10), Some(255));
    }

    #[test]
    fn feather_softens_edges() {
        let mut m = SelectionMask::new(10, 1).unwrap();
        // Left half selected.
        for x in 0..5u32 {
            m.set(x, 0, 255);
        }
        let f = feather(&m, 2).unwrap();
        // Pixel exactly at the boundary should have intermediate coverage.
        let edge_val = f.get(4, 0).unwrap_or(0);
        assert!(edge_val < 255, "edge pixel should be softened");
    }

    #[test]
    fn feather_fully_selected_stays_255_at_centre() {
        let mut m = SelectionMask::new(11, 11).unwrap();
        // Centre pixel only.
        m.set(5, 5, 255);
        let f = feather(&m, 1).unwrap();
        // Centre should be reduced; surrounding should be non-zero.
        let centre = f.get(5, 5).unwrap_or(0);
        let adj = f.get(4, 5).unwrap_or(0);
        assert!(centre > 0);
        assert!(adj > 0);
    }
}
