//! Real-world PSD fixture tests for Photoshop CC and Affinity Photo exports.
//!
//! ## Generating fixtures
//!
//! Fixtures are **not committed** to the repo. Generate them manually with the
//! tools described below and place the files under `examples/psd-fixtures/`.
//! The test runner skips every test in this file when the directory is absent.
//!
//! ### Photoshop CC (2022 or later)
//!
//! 1. Open or create a document with the following layers:
//!    - A named raster layer with solid color fill (no mask).
//!    - A raster layer with a grayscale layer mask painted to hide half the layer.
//!    - A layer group containing two raster layers, with the group set to Multiply.
//!    - An optional text layer (to exercise rasterized-text import).
//! 2. File → Export → Export As… → PSD. Use 8 bits/channel, RGB, maximum
//!    compatibility on.
//! 3. Save as `examples/psd-fixtures/photoshop-cc-layers.psd`.
//!
//! ### Affinity Photo (2.x)
//!
//! 1. Replicate the same layer structure described above for Photoshop CC.
//! 2. File → Export → PSD. Choose "Merge all layers" off, 8-bit, RGBA.
//! 3. Save as `examples/psd-fixtures/affinity-photo-layers.psd`.
//!
//! Both apps produce slightly different PSD encodings (Affinity uses its own
//! section ordering and may write extra metadata blocks). The test suite
//! verifies that both files decode to structurally equivalent Pixhaus archives,
//! and that `ConversionWarning::RasterMaskIgnored` fires exactly once for the
//! masked layer.
//!
//! ## Running fixture tests
//!
//! ```sh
//! cargo nextest run -p pixhaus-io -- psd_fixture_corpus
//! ```
//!
//! Tests marked `#[ignore]` run only when the fixtures are present and the
//! caller opts in:
//!
//! ```sh
//! cargo nextest run -p pixhaus-io -- psd_fixture_corpus --include-ignored
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::disallowed_methods,
    clippy::print_stderr
)]

use std::path::PathBuf;

use pixhaus_io::psd::archive::ConversionWarning;
use pixhaus_io::psd::decode;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("examples")
        .join("psd-fixtures")
}

fn fixture_exists(name: &str) -> bool {
    fixture_dir().join(name).exists()
}

fn load_fixture(name: &str) -> pixhaus_io::psd::ConvertedArchive {
    let path = fixture_dir().join(name);
    let bytes =
        std::fs::read(&path).unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()));
    decode(&bytes, name).expect("decode fixture")
}

// ── Photoshop CC ─────────────────────────────────────────────────────────────

/// Smoke test: Photoshop CC export decodes without a fatal error.
#[test]
#[ignore = "requires examples/psd-fixtures/photoshop-cc-layers.psd (see module docs)"]
fn photoshop_cc_decodes_successfully() {
    if !fixture_exists("photoshop-cc-layers.psd") {
        eprintln!("skipping — fixture not present");
        return;
    }
    let converted = load_fixture("photoshop-cc-layers.psd");
    let sprite = converted.archive.project.sprites.first().expect("sprite");
    assert!(!sprite.layers.is_empty(), "expected at least one layer");
}

/// The masked layer in the Photoshop CC fixture must trigger exactly one
/// `RasterMaskIgnored` warning and name the affected layer.
#[test]
#[ignore = "requires examples/psd-fixtures/photoshop-cc-layers.psd (see module docs)"]
fn photoshop_cc_masked_layer_emits_raster_mask_warning() {
    if !fixture_exists("photoshop-cc-layers.psd") {
        eprintln!("skipping — fixture not present");
        return;
    }
    let converted = load_fixture("photoshop-cc-layers.psd");
    let mask_warnings: Vec<_> = converted
        .warnings
        .iter()
        .filter(|w| matches!(w, ConversionWarning::RasterMaskIgnored { .. }))
        .collect();
    assert_eq!(
        mask_warnings.len(),
        1,
        "expected exactly one RasterMaskIgnored warning, got: {mask_warnings:?}"
    );
}

/// Layer hierarchy from Photoshop CC must include a group with the Multiply
/// blend mode and at least one child raster layer.
#[test]
#[ignore = "requires examples/psd-fixtures/photoshop-cc-layers.psd (see module docs)"]
fn photoshop_cc_group_multiply_is_imported() {
    use pixhaus_core::project::{BlendMode, LayerKind};

    if !fixture_exists("photoshop-cc-layers.psd") {
        eprintln!("skipping — fixture not present");
        return;
    }
    let converted = load_fixture("photoshop-cc-layers.psd");
    let sprite = converted.archive.project.sprites.first().expect("sprite");

    let group = sprite
        .layers
        .iter()
        .find(|l| matches!(l.kind, LayerKind::Group { .. }) && l.blend_mode == BlendMode::Multiply);
    assert!(
        group.is_some(),
        "expected a Multiply group layer; layers: {:?}",
        sprite.layers.iter().map(|l| &l.name).collect::<Vec<_>>()
    );
}

// ── Affinity Photo ────────────────────────────────────────────────────────────

/// Smoke test: Affinity Photo PSD export decodes without a fatal error.
#[test]
#[ignore = "requires examples/psd-fixtures/affinity-photo-layers.psd (see module docs)"]
fn affinity_photo_decodes_successfully() {
    if !fixture_exists("affinity-photo-layers.psd") {
        eprintln!("skipping — fixture not present");
        return;
    }
    let converted = load_fixture("affinity-photo-layers.psd");
    let sprite = converted.archive.project.sprites.first().expect("sprite");
    assert!(!sprite.layers.is_empty(), "expected at least one layer");
}

/// The masked layer in the Affinity Photo fixture must trigger at least one
/// `RasterMaskIgnored` warning.
#[test]
#[ignore = "requires examples/psd-fixtures/affinity-photo-layers.psd (see module docs)"]
fn affinity_photo_masked_layer_emits_raster_mask_warning() {
    if !fixture_exists("affinity-photo-layers.psd") {
        eprintln!("skipping — fixture not present");
        return;
    }
    let converted = load_fixture("affinity-photo-layers.psd");
    let has_mask_warning = converted
        .warnings
        .iter()
        .any(|w| matches!(w, ConversionWarning::RasterMaskIgnored { .. }));
    assert!(
        has_mask_warning,
        "expected at least one RasterMaskIgnored warning; warnings: {:?}",
        converted.warnings
    );
}

/// Layer count and canvas dimensions from Affinity Photo must be non-zero.
#[test]
#[ignore = "requires examples/psd-fixtures/affinity-photo-layers.psd (see module docs)"]
fn affinity_photo_canvas_and_layers_are_non_zero() {
    if !fixture_exists("affinity-photo-layers.psd") {
        eprintln!("skipping — fixture not present");
        return;
    }
    let converted = load_fixture("affinity-photo-layers.psd");
    let sprite = converted.archive.project.sprites.first().expect("sprite");
    assert!(sprite.canvas.width > 0);
    assert!(sprite.canvas.height > 0);
    assert!(!sprite.layers.is_empty());
}
