//! Integration tests for the selection module (S03).
//!
//! These tests exercise the public API of `pixhaus_core::selection` as
//! an external consumer would, covering the full pipeline: shape
//! selection → morphology → boolean ops.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::panic,
    clippy::disallowed_methods
)]

use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::{IVec2, Rect, Rgba};
use pixhaus_core::selection::{
    Connectivity, SelectionMask, color_range, contract, expand, feather, magic_wand,
    select_ellipse, select_polygon, select_rect,
};

// ---------------------------------------------------------------------------
// Helper builders
// ---------------------------------------------------------------------------

fn checkerboard(w: u32, h: u32) -> PixelBuffer {
    let mut buf = PixelBuffer::new(w, h).unwrap();
    for y in 0..h {
        for x in 0..w {
            let c = if (x + y) % 2 == 0 {
                Rgba::opaque(0, 0, 0)
            } else {
                Rgba::opaque(255, 255, 255)
            };
            buf.set_pixel(x, y, c);
        }
    }
    buf
}

fn solid(w: u32, h: u32, color: Rgba) -> PixelBuffer {
    PixelBuffer::filled(w, h, color).unwrap()
}

// ---------------------------------------------------------------------------
// select_rect
// ---------------------------------------------------------------------------

#[test]
fn rect_selects_exact_bounds() {
    let m = select_rect(10, 10, Rect::from_xywh(2, 3, 4, 3)).unwrap();
    assert_eq!(m.selected_count(), 12);
    assert!(m.is_fully_selected(2, 3));
    assert!(m.is_fully_selected(5, 5));
    assert!(!m.is_selected(1, 3));
    assert!(!m.is_selected(6, 3));
    assert!(!m.is_selected(2, 6));
}

#[test]
fn rect_full_canvas() {
    let m = select_rect(8, 8, Rect::from_xywh(0, 0, 8, 8)).unwrap();
    assert_eq!(m.selected_count(), 64);
}

#[test]
fn rect_negative_origin_clips() {
    let m = select_rect(4, 4, Rect::from_xywh(-2, -2, 6, 6)).unwrap();
    assert_eq!(m.selected_count(), 16);
}

#[test]
fn rect_zero_size_empty() {
    let m = select_rect(8, 8, Rect::from_xywh(0, 0, 0, 0)).unwrap();
    assert_eq!(m.selected_count(), 0);
}

// ---------------------------------------------------------------------------
// select_ellipse
// ---------------------------------------------------------------------------

#[test]
fn ellipse_circle_centre_selected() {
    let m = select_ellipse(20, 20, Rect::from_xywh(0, 0, 10, 10)).unwrap();
    assert!(m.is_fully_selected(5, 5));
}

#[test]
fn ellipse_corners_excluded() {
    let m = select_ellipse(10, 10, Rect::from_xywh(1, 1, 8, 8)).unwrap();
    assert!(!m.is_selected(1, 1));
    assert!(!m.is_selected(8, 1));
    assert!(!m.is_selected(1, 8));
    assert!(!m.is_selected(8, 8));
}

#[test]
fn ellipse_wide_rect() {
    // 10 wide, 4 tall — should select more pixels than 4 wide, 4 tall circle.
    let wide = select_ellipse(20, 20, Rect::from_xywh(0, 0, 10, 4)).unwrap();
    let square = select_ellipse(20, 20, Rect::from_xywh(0, 0, 4, 4)).unwrap();
    assert!(wide.selected_count() > square.selected_count());
}

// ---------------------------------------------------------------------------
// select_polygon
// ---------------------------------------------------------------------------

#[test]
fn polygon_right_triangle() {
    // Right triangle: (0,0), (7,0), (7,7)
    let pts = [IVec2::new(0, 0), IVec2::new(7, 0), IVec2::new(7, 7)];
    let m = select_polygon(8, 8, &pts).unwrap();
    // Bottom-left corner must be outside the triangle.
    assert!(!m.is_selected(0, 7));
    // Close to the hypotenuse (inside).
    assert!(m.is_selected(5, 2));
}

#[test]
fn polygon_square_fills_interior() {
    // Square (0,0)→(7,0)→(7,7)→(0,7). Half-open scanline fills
    // y=0..6 (7 rows) × x=0..7 (8 pixels) = 56 pixels.
    // The bottom row at y=7 is on the polygon boundary and not filled.
    let pts = [
        IVec2::new(0, 0),
        IVec2::new(7, 0),
        IVec2::new(7, 7),
        IVec2::new(0, 7),
    ];
    let poly = select_polygon(8, 8, &pts).unwrap();
    // Interior pixel.
    assert!(poly.is_selected(4, 4));
    // Top row included.
    assert!(poly.is_selected(0, 0));
    // Bottom boundary row not filled.
    assert!(!poly.is_selected(4, 7));
    // Expected: 7 rows × 8 pixels = 56.
    assert_eq!(poly.selected_count(), 56);
}

#[test]
fn polygon_degenerate_two_points_empty() {
    let m = select_polygon(8, 8, &[IVec2::new(0, 0), IVec2::new(4, 4)]).unwrap();
    assert_eq!(m.selected_count(), 0);
}

// ---------------------------------------------------------------------------
// magic_wand
// ---------------------------------------------------------------------------

#[test]
fn magic_wand_selects_contiguous_color() {
    // 4x4 with a 2x2 red patch at top-left, rest blue.
    let mut buf = PixelBuffer::new(4, 4).unwrap();
    let red = Rgba::opaque(255, 0, 0);
    let blue = Rgba::opaque(0, 0, 255);
    for y in 0..4u32 {
        for x in 0..4u32 {
            buf.set_pixel(x, y, if x < 2 && y < 2 { red } else { blue });
        }
    }
    let m = magic_wand(&buf, 0, 0, 0, Connectivity::Four).unwrap();
    assert_eq!(m.selected_count(), 4);
    assert!(m.is_fully_selected(0, 0));
    assert!(m.is_fully_selected(1, 1));
    assert!(!m.is_selected(2, 0));
    assert!(!m.is_selected(0, 2));
}

#[test]
fn magic_wand_four_vs_eight_connectivity() {
    // 3x3 with diagonal red pixels forming an X through the centre.
    let mut buf = solid(3, 3, Rgba::opaque(0, 0, 255));
    let red = Rgba::opaque(255, 0, 0);
    buf.set_pixel(0, 0, red);
    buf.set_pixel(1, 1, red);
    buf.set_pixel(2, 2, red);

    // 4-connected from (0,0): only reaches (0,0) — diagonals don't connect.
    let m4 = magic_wand(&buf, 0, 0, 0, Connectivity::Four).unwrap();
    assert_eq!(m4.selected_count(), 1);

    // 8-connected: reaches (0,0), (1,1), (2,2).
    let m8 = magic_wand(&buf, 0, 0, 0, Connectivity::Eight).unwrap();
    assert_eq!(m8.selected_count(), 3);
}

#[test]
fn magic_wand_tolerance_bridges_channel_gap() {
    let base = Rgba::opaque(200, 100, 50);
    let near = Rgba::opaque(205, 100, 50); // R diff = 5
    let far = Rgba::opaque(220, 100, 50); // R diff = 20

    let mut buf = PixelBuffer::new(3, 1).unwrap();
    buf.set_pixel(0, 0, base);
    buf.set_pixel(1, 0, near);
    buf.set_pixel(2, 0, far);

    // Tolerance 5: bridges base→near, stops before far.
    let m5 = magic_wand(&buf, 0, 0, 5, Connectivity::Four).unwrap();
    assert!(m5.is_selected(0, 0));
    assert!(m5.is_selected(1, 0));
    assert!(!m5.is_selected(2, 0));

    // Tolerance 20: bridges all three.
    let m20 = magic_wand(&buf, 0, 0, 20, Connectivity::Four).unwrap();
    assert_eq!(m20.selected_count(), 3);
}

#[test]
fn magic_wand_out_of_bounds_seed_errors() {
    let buf = PixelBuffer::new(4, 4).unwrap();
    assert!(magic_wand(&buf, 4, 0, 0, Connectivity::Four).is_err());
    assert!(magic_wand(&buf, 0, 4, 0, Connectivity::Four).is_err());
}

// ---------------------------------------------------------------------------
// color_range
// ---------------------------------------------------------------------------

#[test]
fn color_range_exact_selects_matching_pixels() {
    let buf = checkerboard(4, 4);
    // Black pixels (0,0,0,255) vs white (255,255,255,255).
    let black = Rgba::opaque(0, 0, 0);
    let m = color_range(&buf, black, 0).unwrap();
    assert_eq!(m.selected_count(), 8); // half of 16
}

#[test]
fn color_range_tolerance_255_selects_all() {
    let buf = checkerboard(4, 4);
    let m = color_range(&buf, Rgba::opaque(128, 128, 128), 255).unwrap();
    assert_eq!(m.selected_count(), 16);
}

#[test]
fn color_range_non_contiguous() {
    // Top row red, bottom row blue — colour range selects all red regardless
    // of layout.
    let red = Rgba::opaque(255, 0, 0);
    let blue = Rgba::opaque(0, 0, 255);
    let mut buf = PixelBuffer::new(4, 2).unwrap();
    for x in 0..4u32 {
        buf.set_pixel(x, 0, red);
        buf.set_pixel(x, 1, blue);
    }
    let m = color_range(&buf, red, 0).unwrap();
    assert_eq!(m.selected_count(), 4);
    for x in 0..4u32 {
        assert!(m.is_fully_selected(x, 0));
        assert!(!m.is_selected(x, 1));
    }
}

// ---------------------------------------------------------------------------
// boolean operations
// ---------------------------------------------------------------------------

#[test]
fn union_combines_selections() {
    let a = select_rect(8, 8, Rect::from_xywh(0, 0, 4, 4)).unwrap();
    let b = select_rect(8, 8, Rect::from_xywh(4, 4, 4, 4)).unwrap();
    let u = a.union(&b).unwrap();
    assert_eq!(u.selected_count(), 32);
    assert!(u.is_fully_selected(0, 0));
    assert!(u.is_fully_selected(7, 7));
    assert!(!u.is_selected(4, 0));
}

#[test]
fn intersect_overlap_only() {
    let a = select_rect(8, 8, Rect::from_xywh(0, 0, 6, 6)).unwrap();
    let b = select_rect(8, 8, Rect::from_xywh(4, 4, 4, 4)).unwrap();
    let i = a.intersect(&b).unwrap();
    // Overlap region is (4..6) x (4..6) = 4 pixels.
    assert_eq!(i.selected_count(), 4);
    assert!(i.is_fully_selected(4, 4));
    assert!(!i.is_selected(0, 0));
}

#[test]
fn subtract_removes_overlap() {
    let a = select_rect(8, 8, Rect::from_xywh(0, 0, 6, 6)).unwrap();
    let b = select_rect(8, 8, Rect::from_xywh(4, 4, 4, 4)).unwrap();
    let s = a.subtract(&b).unwrap();
    // a had 36, b removes the 4 overlapping pixels.
    assert_eq!(s.selected_count(), 32);
    assert!(!s.is_selected(4, 4));
    assert!(s.is_fully_selected(0, 0));
}

#[test]
fn xor_symmetric_difference() {
    let a = select_rect(8, 8, Rect::from_xywh(0, 0, 6, 6)).unwrap(); // 36 pixels
    let b = select_rect(8, 8, Rect::from_xywh(4, 4, 4, 4)).unwrap(); // 16 pixels
    let x = a.xor(&b).unwrap();
    // Overlap (4 pixels) → 0. a-only (32) → 255. b-only (12) → 255.
    assert_eq!(x.selected_count(), 44);
    assert!(!x.is_selected(4, 4)); // in both
    assert!(x.is_fully_selected(0, 0)); // a only
    assert!(x.is_fully_selected(7, 7)); // b only
}

#[test]
fn invert_flips_selection() {
    let m = select_rect(4, 4, Rect::from_xywh(0, 0, 2, 2)).unwrap();
    let inv = m.invert();
    assert_eq!(inv.selected_count(), 12);
    assert!(!inv.is_selected(0, 0));
    assert!(inv.is_fully_selected(3, 3));
}

#[test]
fn boolean_ops_size_mismatch_errors() {
    let a = SelectionMask::full(4, 4).unwrap();
    let b = SelectionMask::full(5, 5).unwrap();
    assert!(a.union(&b).is_err());
    assert!(a.intersect(&b).is_err());
    assert!(a.subtract(&b).is_err());
    assert!(a.xor(&b).is_err());
}

// ---------------------------------------------------------------------------
// morphology
// ---------------------------------------------------------------------------

#[test]
fn expand_grows_selection() {
    let mut m = SelectionMask::new(10, 10).unwrap();
    m.set(5, 5, 255);
    let e = expand(&m, 2).unwrap();
    // Centre stays selected.
    assert!(e.is_fully_selected(5, 5));
    // Adjacent cardinal pixels.
    assert!(e.is_fully_selected(3, 5));
    assert!(e.is_fully_selected(7, 5));
    // Just outside radius — distance sqrt(3^2 + 0) = 3 > 2.
    // With radius 2, pixels at distance <= 2: i.e., (5+2,5) = (7,5) ✓
    // pixel (5,8) is at distance 3, should NOT be selected.
    assert!(!e.is_selected(5, 8));
}

#[test]
fn contract_shrinks_selection() {
    let m = select_rect(10, 10, Rect::from_xywh(0, 0, 10, 10)).unwrap();
    let c = contract(&m, 1).unwrap();
    // Edge pixels eroded.
    assert!(!c.is_selected(0, 0));
    assert!(!c.is_selected(9, 9));
    // Interior pixel kept.
    assert!(c.is_fully_selected(5, 5));
}

#[test]
fn expand_then_contract_roundtrip_approximates_original() {
    // Expanding then contracting by the same amount should roughly
    // restore the original for a convex selection (with some edge loss).
    let m = select_rect(20, 20, Rect::from_xywh(5, 5, 10, 10)).unwrap();
    let e = expand(&m, 2).unwrap();
    let ec = contract(&e, 2).unwrap();
    // Should not have gained pixels vs original.
    assert!(ec.selected_count() <= m.selected_count());
}

#[test]
fn feather_reduces_hard_edges() {
    let mut m = SelectionMask::new(12, 1).unwrap();
    for x in 0..6u32 {
        m.set(x, 0, 255);
    }
    let f = feather(&m, 3).unwrap();
    // Pixel at the far end of the selected half is still fully selected.
    assert_eq!(f.get(0, 0), Some(255));
    // Pixel at the boundary has intermediate coverage.
    let boundary = f.get(5, 0).unwrap_or(0);
    assert!(boundary > 0 && boundary < 255);
    // Pixel outside the original selection now has non-zero coverage.
    let outside = f.get(7, 0).unwrap_or(0);
    assert!(outside > 0);
}

#[test]
fn feather_preserves_deep_interior() {
    let m = SelectionMask::full(20, 20).unwrap();
    let f = feather(&m, 3).unwrap();
    // Deep interior pixel should remain at 255.
    assert_eq!(f.get(10, 10), Some(255));
}

// ---------------------------------------------------------------------------
// combined pipeline
// ---------------------------------------------------------------------------

#[test]
fn pipeline_wand_then_expand_then_invert() {
    // Solid 8x8 red buffer.
    let red = Rgba::opaque(255, 0, 0);
    let buf = solid(8, 8, red);
    // Magic wand at (0,0) with tolerance 0 selects all 64 pixels.
    let wand = magic_wand(&buf, 0, 0, 0, Connectivity::Four).unwrap();
    assert_eq!(wand.selected_count(), 64);
    // Expand does not add pixels (already full).
    let expanded = expand(&wand, 1).unwrap();
    assert_eq!(expanded.selected_count(), 64);
    // Invert selects nothing.
    let inv = expanded.invert();
    assert_eq!(inv.selected_count(), 0);
}

#[test]
fn pipeline_rect_subtract_ellipse() {
    let rect = select_rect(10, 10, Rect::from_xywh(0, 0, 10, 10)).unwrap();
    let ellipse = select_ellipse(10, 10, Rect::from_xywh(0, 0, 10, 10)).unwrap();
    let ring = rect.subtract(&ellipse).unwrap();
    // The four corners of the rect are outside the ellipse and should
    // be in the ring selection.
    assert!(ring.is_fully_selected(0, 0));
    assert!(ring.is_fully_selected(9, 0));
    // The centre of the rect is inside the ellipse and should be gone.
    assert!(!ring.is_selected(5, 5));
}
