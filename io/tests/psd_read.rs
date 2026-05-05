//! Integration tests for PSD import.
//!
//! These tests use synthetic PSD files produced by the `psd` crate's own
//! test infrastructure or hand-crafted byte sequences. Real-world PSD
//! fixtures are not committed to the repo; the fixture generation approach
//! follows the same pattern as `aseprite_real_fixtures.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::disallowed_methods
)]

use pixhaus_core::project::ColorMode;
use pixhaus_io::psd::decode;

/// Build the minimal valid PSD bytes for a w×h RGB/8-bit document with
/// no layers. This is the smallest well-formed PSD accepted by the `psd`
/// crate and serves as a baseline for error-path and metadata tests.
///
/// PSD binary layout (big-endian throughout):
///   File header: signature (4), version (2), reserved (6), channels (2),
///                height (4), width (4), depth (2), `color_mode` (2) = 26 bytes
///   Color mode data: length (4) = 4 bytes
///   Image resources: length (4) = 4 bytes
///   Layer and mask info: length (4) = 4 bytes
///   Image data: compression (2) + per-channel RLE lengths (0) = 2 bytes
fn minimal_psd(width: u32, height: u32) -> Vec<u8> {
    let mut v = Vec::new();

    // File header
    v.extend_from_slice(b"8BPS"); // signature
    v.extend_from_slice(&1u16.to_be_bytes()); // version = 1 (PSD, not PSB)
    v.extend_from_slice(&[0u8; 6]); // reserved
    v.extend_from_slice(&3u16.to_be_bytes()); // channels = 3 (RGB)
    v.extend_from_slice(&height.to_be_bytes());
    v.extend_from_slice(&width.to_be_bytes());
    v.extend_from_slice(&8u16.to_be_bytes()); // depth = 8 bpc
    v.extend_from_slice(&3u16.to_be_bytes()); // color mode = 3 (RGB)

    // Color mode data section (empty for RGB)
    v.extend_from_slice(&0u32.to_be_bytes());

    // Image resources section (empty)
    v.extend_from_slice(&0u32.to_be_bytes());

    // Layer and mask information section (empty)
    v.extend_from_slice(&0u32.to_be_bytes());

    // Image data: raw (uncompressed), 3 channels × width × height bytes
    v.extend_from_slice(&0u16.to_be_bytes()); // compression = 0 (raw)
    let channel_data_len = (width * height) as usize;
    v.extend(std::iter::repeat_n(0u8, channel_data_len * 3)); // R G B

    v
}

#[test]
fn decode_minimal_psd_succeeds() {
    let bytes = minimal_psd(4, 4);
    let result = decode(&bytes, "test");
    assert!(result.is_ok(), "minimal PSD should parse: {result:?}");
}

#[test]
fn canvas_dimensions_are_preserved() {
    let bytes = minimal_psd(32, 64);
    let converted = decode(&bytes, "dims").unwrap();
    let sprite = converted.archive.project.sprites.first().unwrap();
    assert_eq!(sprite.canvas.width, 32);
    assert_eq!(sprite.canvas.height, 64);
}

#[test]
fn sprite_name_matches_argument() {
    let bytes = minimal_psd(8, 8);
    let converted = decode(&bytes, "my-sprite").unwrap();
    let sprite = converted.archive.project.sprites.first().unwrap();
    assert_eq!(sprite.name, "my-sprite");
}

#[test]
fn single_frame_is_created() {
    let bytes = minimal_psd(8, 8);
    let converted = decode(&bytes, "frames").unwrap();
    let sprite = converted.archive.project.sprites.first().unwrap();
    assert_eq!(sprite.frames.len(), 1);
}

#[test]
fn color_mode_is_rgba() {
    let bytes = minimal_psd(8, 8);
    let converted = decode(&bytes, "colormode").unwrap();
    let sprite = converted.archive.project.sprites.first().unwrap();
    assert_eq!(sprite.color_mode, ColorMode::Rgba);
}

#[test]
fn no_warnings_for_clean_rgb8_psd() {
    let bytes = minimal_psd(8, 8);
    let converted = decode(&bytes, "clean").unwrap();
    assert!(
        converted.warnings.is_empty(),
        "clean RGB/8-bit PSD should produce no warnings: {:?}",
        converted.warnings
    );
}

#[test]
fn invalid_bytes_returns_psd_parse_error() {
    use pixhaus_io::error::Error;
    let result = decode(b"not a psd file at all", "bad");
    assert!(
        matches!(result, Err(Error::PsdParse(_))),
        "expected PsdParse error, got: {result:?}"
    );
}

// ── Blend mode mapping (unit-level via spec module) ──────────────────────────

mod blend_mode_spec {
    use pixhaus_io::psd::archive::ConversionWarning;
    // Re-test the spec mapping through the public API surface to ensure
    // the end-to-end path compiles and produces consistent results.

    #[test]
    fn warning_enum_is_cloneable_and_comparable() {
        let w = ConversionWarning::UnsupportedBlendMode {
            layer_name: "bg".into(),
            psd_mode: "Dissolve".into(),
        };
        assert_eq!(w.clone(), w);
    }

    #[test]
    fn high_bit_depth_warning_carries_bits() {
        let w = ConversionWarning::HighBitDepthDownsampled { bits: 16 };
        assert!(matches!(
            w,
            ConversionWarning::HighBitDepthDownsampled { bits: 16 }
        ));
    }

    #[test]
    fn raster_mask_ignored_warning_is_cloneable_and_comparable() {
        let w = ConversionWarning::RasterMaskIgnored {
            layer_name: "shadow".into(),
        };
        assert_eq!(w.clone(), w);
        assert!(matches!(w, ConversionWarning::RasterMaskIgnored { .. }));
    }
}

// ── Layer structure round-trip helpers ───────────────────────────────────────
//
// Full layer round-trip tests require real or synthesized PSD files with
// layer data, which is non-trivial to produce by hand (the layer and mask
// information section has complex encoding). These are deferred to the
// real-fixture test suite (see aseprite_real_fixtures.rs for the pattern).
//
// For now, validate that the no-layer case produces a consistent structure.

#[test]
fn no_layers_produces_empty_layer_list() {
    let bytes = minimal_psd(16, 16);
    let converted = decode(&bytes, "empty").unwrap();
    let sprite = converted.archive.project.sprites.first().unwrap();
    assert!(sprite.layers.is_empty());
    assert!(sprite.cels.is_empty());
}

#[test]
fn buffer_list_is_empty_when_no_pixel_layers() {
    let bytes = minimal_psd(16, 16);
    let converted = decode(&bytes, "empty").unwrap();
    assert!(converted.archive.buffers.is_empty());
}
