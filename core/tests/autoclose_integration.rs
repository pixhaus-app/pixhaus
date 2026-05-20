//! Integration tests for the gap-closing magic wand (S57).
//!
//! These exercise the public API of `pixhaus_core::selection` as an
//! external consumer would: build a sprite with broken outlines, run
//! [`magic_wand_with_gap_close`], and confirm the closure pre-pass
//! both bridges short gaps and refuses to bridge gaps that exceed
//! [`GapCloseConfig::closing_distance`].

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::panic,
    clippy::disallowed_methods
)]

use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::{IVec2, Rgba};
use pixhaus_core::selection::{
    Connectivity, GapCloseConfig, close_gaps, magic_wand, magic_wand_with_gap_close,
};

/// Builds a `size × size` white canvas with a black square outline at
/// `margin` pixels from each side, and a single `gap_pixels`-wide hole
/// on the top edge of the outline (centred horizontally).
fn square_outline_with_top_gap(size: u32, margin: u32, gap_pixels: u32) -> PixelBuffer {
    let mut buf = PixelBuffer::filled(size, size, Rgba::opaque(255, 255, 255)).unwrap();
    let ink = Rgba::opaque(0, 0, 0);
    let top = margin;
    let bottom = size - margin - 1;
    let left = margin;
    let right = size - margin - 1;
    let gap_start = size / 2 - gap_pixels / 2;
    let gap_end = gap_start + gap_pixels;
    for x in left..=right {
        if !(gap_start..gap_end).contains(&x) {
            buf.set_pixel(x, top, ink);
        }
        buf.set_pixel(x, bottom, ink);
    }
    for y in top..=bottom {
        buf.set_pixel(left, y, ink);
        buf.set_pixel(right, y, ink);
    }
    buf
}

#[test]
fn close_gaps_returns_empty_mask_for_solid_canvas() {
    // No endpoints — no bridges. Solid white canvas: every pixel is
    // background, so no ink pixels at all.
    let buf = PixelBuffer::filled(8, 8, Rgba::opaque(255, 255, 255)).unwrap();
    let m = close_gaps(&buf, &GapCloseConfig::default()).unwrap();
    assert_eq!(m.selected_count(), 0);
}

#[test]
fn magic_wand_with_gap_close_contains_fill_to_box_interior() {
    let buf = square_outline_with_top_gap(20, 3, 2);
    // Without closure, the fill leaks out the top gap and into the
    // entire white frame.
    let leak = magic_wand(&buf, 10, 10, 0, Connectivity::Four).unwrap();
    assert!(leak.is_selected(0, 0), "baseline must leak");

    // With closure, the fill stays inside the outline.
    let inside = magic_wand_with_gap_close(
        &buf,
        IVec2 { x: 10, y: 10 },
        0,
        Connectivity::Four,
        Some(GapCloseConfig::default()),
    )
    .unwrap();
    assert!(inside.is_selected(10, 10));
    assert!(!inside.is_selected(0, 0));
}

#[test]
fn magic_wand_with_gap_close_skips_wide_gap() {
    // A gap wider than `closing_distance` (default 10) shouldn't be
    // bridged. Use a 12-wide gap with a 32×32 canvas so the outline
    // endpoints are still within the buffer.
    let buf = square_outline_with_top_gap(32, 3, 12);
    let leak = magic_wand_with_gap_close(
        &buf,
        IVec2 { x: 16, y: 16 },
        0,
        Connectivity::Four,
        Some(GapCloseConfig::default()),
    )
    .unwrap();
    // The fill still leaks out the unclosed gap, so (0, 0) is reached.
    assert!(leak.is_selected(0, 0));
}

#[test]
fn magic_wand_with_gap_close_none_matches_plain_magic_wand() {
    // gap_config = None must behave exactly like magic_wand.
    let buf = square_outline_with_top_gap(20, 3, 2);
    let plain = magic_wand(&buf, 10, 10, 0, Connectivity::Four).unwrap();
    let via_none =
        magic_wand_with_gap_close(&buf, IVec2 { x: 10, y: 10 }, 0, Connectivity::Four, None)
            .unwrap();
    assert_eq!(plain.selected_count(), via_none.selected_count());
}
