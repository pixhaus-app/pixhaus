//! Integration tests for the `.pixhaus` round-trip pipeline.
//!
//! Three non-trivial fixtures:
//! 1. Empty project — no sprites, no buffers.
//! 2. Full structural project — all types from core's sample fixture, no pixel bytes.
//! 3. Project with pixel buffers — raster and tileset buffers with real bytes.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::disallowed_methods,
    clippy::cast_possible_truncation
)]

use pixhaus_core::fixtures::sample_project;
use pixhaus_core::project::{FeatureFlags, PixelBufferId, Project};
use pixhaus_io::pixhaus::{PixelBufferEntry, PixhausArchive, decode, encode};
use rstest::rstest;

// ── pixel data helpers ───────────────────────────────────────────────────────

fn rgba_checkerboard(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let offset = ((y * width + x) * 4) as usize;
            let on = (x + y) % 2 == 0;
            pixels[offset] = if on { 255 } else { 0 };
            pixels[offset + 1] = 0;
            pixels[offset + 2] = if on { 0 } else { 255 };
            pixels[offset + 3] = 255;
        }
    }
    pixels
}

fn indexed_gradient(width: u32, height: u32) -> Vec<u8> {
    (0..(width * height)).map(|i| (i % 256) as u8).collect()
}

// ── fixtures ─────────────────────────────────────────────────────────────────

fn empty_archive() -> PixhausArchive {
    PixhausArchive::new(Project::new("empty"))
}

fn structural_archive() -> PixhausArchive {
    // sample_project exercises every type in the data model; no pixel bytes.
    PixhausArchive::new(sample_project())
}

fn full_archive() -> PixhausArchive {
    // sample_project references PixelBufferId(10), (1000), (2000).
    let project = sample_project();
    let buffers = vec![
        PixelBufferEntry {
            id: 10,
            width: 32,
            height: 32,
            stride: 32 * 4,
            pixels: rgba_checkerboard(32, 32),
        },
        PixelBufferEntry {
            id: 1000,
            width: 32,
            height: 32,
            stride: 32 * 4,
            pixels: rgba_checkerboard(32, 32),
        },
        // Tileset inline: 16 tiles of 8×8 packed vertically (8×128 strip).
        PixelBufferEntry {
            id: 2000,
            width: 8,
            height: 128,
            stride: 8,
            pixels: indexed_gradient(8, 128),
        },
    ];
    PixhausArchive { project, buffers }
}

// ── round-trip ────────────────────────────────────────────────────────────────

#[rstest]
#[case::empty(empty_archive())]
#[case::structural(structural_archive())]
#[case::with_buffers(full_archive())]
fn round_trip(#[case] archive: PixhausArchive) {
    let encoded = encode(&archive).expect("encode failed");
    let decoded = decode(&encoded).expect("decode failed");

    assert_eq!(decoded.project.metadata.name, archive.project.metadata.name,);
    assert_eq!(decoded.project.feature_flags, archive.project.feature_flags);
    assert_eq!(decoded.project.sprites.len(), archive.project.sprites.len());
    assert_eq!(decoded.buffers.len(), archive.buffers.len());

    for (orig, back) in archive.buffers.iter().zip(decoded.buffers.iter()) {
        assert_eq!(back.id, orig.id);
        assert_eq!(back.width, orig.width);
        assert_eq!(back.height, orig.height);
        assert_eq!(back.stride, orig.stride);
        assert_eq!(back.pixels, orig.pixels);
    }
}

// ── compression effectiveness ─────────────────────────────────────────────────

#[test]
fn compressed_body_is_smaller_than_raw_msgpack() {
    let archive = full_archive();
    let encoded = encode(&archive).expect("encode failed");
    let raw_msgpack = rmp_serde::to_vec_named(&archive).expect("msgpack failed");

    // Header is 28 bytes; the rest is the compressed body.
    let compressed_body_len = encoded.len() - 28;
    assert!(
        compressed_body_len < raw_msgpack.len(),
        "expected compressed ({compressed_body_len} B) < raw msgpack ({} B)",
        raw_msgpack.len()
    );
}

// ── header validation ─────────────────────────────────────────────────────────

#[test]
fn rejects_wrong_magic() {
    let mut bytes = encode(&empty_archive()).unwrap();
    bytes[0] = b'X';
    assert!(matches!(
        decode(&bytes).unwrap_err(),
        pixhaus_io::Error::InvalidMagic,
    ));
}

#[test]
fn rejects_unsupported_major_version() {
    let mut bytes = encode(&empty_archive()).unwrap();
    // format_major at offset 8–9
    bytes[8] = 0;
    bytes[9] = 99;
    assert!(matches!(
        decode(&bytes).unwrap_err(),
        pixhaus_io::Error::UnsupportedVersion { major: 99, .. },
    ));
}

#[test]
fn rejects_unknown_required_features() {
    let mut bytes = encode(&empty_archive()).unwrap();
    // required_flags at offset 16–19
    let unknown: u32 = 1 << 31;
    let flag_bytes = unknown.to_be_bytes();
    bytes[16..20].copy_from_slice(&flag_bytes);
    assert!(matches!(
        decode(&bytes).unwrap_err(),
        pixhaus_io::Error::UnknownRequiredFeatures { .. },
    ));
}

#[test]
fn rejects_truncated_header() {
    let bytes = &encode(&empty_archive()).unwrap()[..10];
    assert!(matches!(
        decode(bytes).unwrap_err(),
        pixhaus_io::Error::Truncated,
    ));
}

#[test]
fn rejects_truncated_body() {
    let mut bytes = encode(&empty_archive()).unwrap();
    // Inflate body_len at offset 20–27 to trigger Truncated on body read.
    let inflated: u64 = 1_000_000;
    bytes[20..28].copy_from_slice(&inflated.to_be_bytes());
    assert!(matches!(
        decode(&bytes).unwrap_err(),
        pixhaus_io::Error::Truncated,
    ));
}

// ── feature flag propagation ──────────────────────────────────────────────────

#[test]
fn feature_flags_survive_round_trip() {
    let mut archive = empty_archive();
    archive.project.feature_flags = FeatureFlags::TILEMAPS.union(FeatureFlags::ANIMATIONS);

    let encoded = encode(&archive).unwrap();
    let decoded = decode(&encoded).unwrap();

    assert_eq!(decoded.project.feature_flags, archive.project.feature_flags);
}

// ── pixel buffer lookup ───────────────────────────────────────────────────────

#[test]
fn buffer_lookup_finds_by_id() {
    let archive = full_archive();
    let entry = archive.buffer(PixelBufferId::new(10));
    assert!(entry.is_some());
    assert_eq!(entry.unwrap().width, 32);
}

#[test]
fn buffer_lookup_returns_none_for_missing_id() {
    assert!(full_archive().buffer(PixelBufferId::new(9999)).is_none());
}

// ── cel linkage survives round-trip ──────────────────────────────────────────

#[test]
fn linked_cel_round_trips_correctly() {
    use pixhaus_core::project::CelData;

    let project = sample_project();

    // Capture all needed data (all Copy types) before moving `project`.
    let (layer_id, frame_index, source_frame) = {
        let linked = project.sprites[0]
            .cels
            .iter()
            .find(|c| matches!(&c.data, CelData::Linked { .. }))
            .expect("sample_project must have a linked cel");

        let sf = match &linked.data {
            CelData::Linked { source_frame } => *source_frame,
            _ => unreachable!(),
        };
        (linked.layer_id, linked.frame_index, sf)
    };

    let encoded = encode(&PixhausArchive::new(project)).unwrap();
    let decoded = decode(&encoded).unwrap();

    let back = decoded.project.sprites[0]
        .cels
        .iter()
        .find(|c| c.layer_id == layer_id && c.frame_index == frame_index)
        .expect("linked cel not found after round-trip");

    assert!(matches!(
        &back.data,
        CelData::Linked { source_frame: sf } if *sf == source_frame,
    ));
}
