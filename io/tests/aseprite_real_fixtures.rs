//! Round-trip integration tests against the binary `.aseprite` fixtures
//! committed under `examples/aseprite-roundtrip/`.
//!
//! Each fixture represents one shape from the brief's matrix:
//! single-frame raster, multi-frame with tags, group with non-Normal
//! blend, indexed-with-palette, and tilemap with a tileset. For each:
//!
//! 1. Read the fixture bytes from disk.
//! 2. Decode → `AsepriteDocument`.
//! 3. Convert → `PixhausArchive`, asserting structural fields and that
//!    no `ConversionWarning` fires for input the spec says shouldn't
//!    drop anything.
//! 4. Convert back → `AsepriteDocument`, encode, and compare bytes
//!    against the original fixture. Encoding is deterministic in this
//!    codebase (zlib level 6, no random padding) so the equality should
//!    hold byte-for-byte.
//!
//! The fixtures themselves are produced by `generate_real_fixtures.rs`
//! when `PIXHAUS_REGEN_ASEPRITE_FIXTURES=1` is set; on a normal CI run
//! they are read-only inputs.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::disallowed_methods
)]

use std::path::PathBuf;

use pixhaus_core::project::{BlendMode, CelData, ColorMode, LayerKind, LoopDirection};
use pixhaus_io::aseprite::{
    ConversionWarning, archive_to_document, decode, document_to_archive, encode,
};
use pixhaus_io::pixhaus::PixhausArchive;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("examples")
        .join("aseprite-roundtrip")
        .join(name)
}

fn load_archive(name: &str) -> (Vec<u8>, PixhausArchive, Vec<ConversionWarning>) {
    let path = fixture_path(name);
    let bytes =
        std::fs::read(&path).unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()));
    let doc = decode(&bytes).expect("decode fixture");
    let converted = document_to_archive(&doc, name).expect("convert fixture");
    (bytes, converted.archive, converted.warnings)
}

fn assert_byte_stable(name: &str, original: &[u8], archive: &PixhausArchive) {
    let doc = archive_to_document(archive);
    let encoded = encode(&doc).expect("encode round-trip bytes");
    assert_eq!(
        encoded.len(),
        original.len(),
        "round-trip byte length mismatch for {name}: orig={} new={}",
        original.len(),
        encoded.len()
    );
    if encoded != original {
        // Locate the first divergence so test output points at the
        // offset rather than dumping kilobytes of unrelated bytes.
        let first = encoded
            .iter()
            .zip(original.iter())
            .position(|(a, b)| a != b)
            .unwrap_or(0);
        panic!(
            "round-trip bytes differ at offset {first} for {name}: orig={:#04x} new={:#04x}",
            original[first], encoded[first]
        );
    }
}

#[test]
fn single_frame_rgba_round_trips_byte_for_byte() {
    let (bytes, archive, warnings) = load_archive("single-frame-rgba.aseprite");
    let sprite = archive.project.sprites.first().expect("sprite");
    assert_eq!(sprite.canvas.width, 8);
    assert_eq!(sprite.canvas.height, 8);
    assert_eq!(sprite.color_mode, ColorMode::Rgba);
    assert_eq!(sprite.layers.len(), 1);
    assert_eq!(sprite.layers[0].name, "main");
    assert!(matches!(sprite.layers[0].kind, LayerKind::Raster));
    assert_eq!(sprite.frames.len(), 1);
    assert_eq!(sprite.cels.len(), 1);
    assert!(
        warnings.is_empty(),
        "no warnings expected for plain RGBA sprite, got {warnings:?}"
    );
    assert_byte_stable("single-frame-rgba.aseprite", &bytes, &archive);
}

#[test]
fn multi_frame_tags_round_trips_byte_for_byte() {
    let (bytes, archive, warnings) = load_archive("multi-frame-tags.aseprite");
    let sprite = archive.project.sprites.first().expect("sprite");
    assert_eq!(sprite.frames.len(), 3);
    assert_eq!(sprite.cels.len(), 3);
    assert_eq!(sprite.frame_tags.len(), 1);
    let tag = &sprite.frame_tags[0];
    assert_eq!(tag.name, "loop");
    assert_eq!(tag.loop_direction, LoopDirection::Forward);
    assert_eq!(tag.range.start.get(), 0);
    assert_eq!(tag.range.end.get(), 2);
    assert!(
        warnings.is_empty(),
        "no warnings expected for tagged animation, got {warnings:?}"
    );
    assert_byte_stable("multi-frame-tags.aseprite", &bytes, &archive);
}

#[test]
fn group_multiply_round_trips_byte_for_byte() {
    let (bytes, archive, warnings) = load_archive("group-multiply.aseprite");
    let sprite = archive.project.sprites.first().expect("sprite");
    assert_eq!(sprite.layers.len(), 2);
    let group = sprite
        .layers
        .iter()
        .find(|l| matches!(l.kind, LayerKind::Group { .. }))
        .expect("group layer");
    assert_eq!(group.blend_mode, BlendMode::Multiply);
    assert_eq!(group.name, "fx");
    let raster = sprite
        .layers
        .iter()
        .find(|l| matches!(l.kind, LayerKind::Raster))
        .expect("raster layer");
    assert_eq!(raster.parent, Some(group.id));
    assert!(
        warnings.is_empty(),
        "no warnings expected for layered RGBA sprite, got {warnings:?}"
    );
    assert_byte_stable("group-multiply.aseprite", &bytes, &archive);
}

#[test]
fn indexed_with_palette_round_trips_byte_for_byte() {
    let (bytes, archive, warnings) = load_archive("indexed-with-palette.aseprite");
    let sprite = archive.project.sprites.first().expect("sprite");
    assert_eq!(sprite.color_mode, ColorMode::Indexed);
    assert_eq!(sprite.transparent_color_index, Some(0));
    let palette = sprite.palettes.first().expect("palette");
    assert_eq!(palette.colors.len(), 4);
    assert_eq!(palette.colors[1].name.as_deref(), Some("red"));
    assert_eq!(palette.colors[2].name.as_deref(), Some("green"));
    assert_eq!(palette.colors[3].name.as_deref(), Some("blue"));
    assert!(
        warnings.is_empty(),
        "no warnings expected for indexed sprite, got {warnings:?}"
    );
    assert_byte_stable("indexed-with-palette.aseprite", &bytes, &archive);
}

#[test]
fn tilemap_with_tileset_round_trips_byte_for_byte() {
    let (bytes, archive, warnings) = load_archive("tilemap-with-tileset.aseprite");
    let sprite = archive.project.sprites.first().expect("sprite");
    assert_eq!(sprite.layers.len(), 1);
    assert!(matches!(sprite.layers[0].kind, LayerKind::Tilemap { .. }));
    assert_eq!(sprite.tilesets.len(), 1);
    let tileset = &sprite.tilesets[0];
    assert_eq!(tileset.tile_count, 3);
    assert_eq!(tileset.base_index, 1);
    let cel = sprite.cels.first().expect("tilemap cel");
    match &cel.data {
        CelData::Tilemap { data } => {
            assert_eq!(data.width, 2);
            assert_eq!(data.height, 2);
        }
        _ => panic!("expected tilemap cel"),
    }
    assert!(
        warnings.is_empty(),
        "no warnings expected for inline tilemap sprite, got {warnings:?}"
    );
    assert_byte_stable("tilemap-with-tileset.aseprite", &bytes, &archive);
}
