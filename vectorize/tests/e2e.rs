//! End-to-end fixtures for `centerline_vectorize`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::missing_panics_doc
)]

use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::{Palette, PaletteId, Rgba};
use pixhaus_vectorize::{CenterlineConfig, Error, centerline_vectorize};

/// Builds a `size x size` buffer with a one-pixel black border on a
/// white background.
fn square_outline_buffer(size: u32) -> PixelBuffer {
    let mut buf = PixelBuffer::filled(size, size, Rgba::opaque(255, 255, 255)).unwrap();
    let black = Rgba::opaque(0, 0, 0);
    for x in 0..size {
        buf.set_pixel(x, 0, black);
        buf.set_pixel(x, size - 1, black);
    }
    for y in 0..size {
        buf.set_pixel(0, y, black);
        buf.set_pixel(size - 1, y, black);
    }
    buf
}

fn simple_palette() -> Palette {
    Palette::from_colors(PaletteId::new(0), "test", vec![Rgba::opaque(0, 0, 0)])
}

#[test]
fn e2e_solid_square_outline_vectorizes() {
    let buf = square_outline_buffer(32);
    let palette = simple_palette();
    let vi = centerline_vectorize(&buf, &palette, &CenterlineConfig::default()).expect("vectorize");
    assert!(!vi.strokes.is_empty(), "expected at least one stroke");
    assert_eq!(vi.width, 32);
    assert_eq!(vi.height, 32);
}

#[test]
fn e2e_empty_raster_errors() {
    let buf = PixelBuffer::empty();
    let palette = simple_palette();
    let err = centerline_vectorize(&buf, &palette, &CenterlineConfig::default()).unwrap_err();
    assert!(matches!(err, Error::EmptyRaster));
}

#[test]
fn e2e_empty_palette_errors() {
    let buf = square_outline_buffer(8);
    let palette = Palette::from_colors(PaletteId::new(0), "empty", vec![]);
    let err = centerline_vectorize(&buf, &palette, &CenterlineConfig::default()).unwrap_err();
    assert!(matches!(err, Error::EmptyPalette));
}

#[test]
fn e2e_all_white_yields_no_strokes() {
    let buf = PixelBuffer::filled(16, 16, Rgba::opaque(255, 255, 255)).unwrap();
    let palette = simple_palette();
    let vi = centerline_vectorize(&buf, &palette, &CenterlineConfig::default()).expect("vectorize");
    assert!(vi.strokes.is_empty());
}

#[test]
fn e2e_snapshot_simple_outline() {
    let buf = square_outline_buffer(16);
    let palette = simple_palette();
    let vi = centerline_vectorize(&buf, &palette, &CenterlineConfig::default()).unwrap();
    insta::assert_yaml_snapshot!(vi);
}
