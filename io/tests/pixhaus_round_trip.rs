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
use pixhaus_core::project::{FeatureFlags, PixelBufferId, Project, SchemaError, SchemaVersion};
use pixhaus_io::pixhaus::{
    PixelBufferEntry, PixhausArchive, decode, decode_from_file, encode, encode_to_file,
};
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

    // Full structural equality. sample_project covers every B2 type, so
    // a regression in any field (layer kinds, cel data, frame tags,
    // animations, palettes, slices, tilesets, selection state) surfaces
    // here rather than slipping through a partial assertion list.
    assert_eq!(decoded.project, archive.project);

    // Buffers are not part of Project's PartialEq; assert separately.
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
    // Set bit 31 in BOTH feature_flags (offset 12-15) and required_flags
    // (offset 16-19). The flags-subset check passes (required ⊆ feature),
    // so the unknown-required-features branch fires.
    let unknown: u32 = 1 << 31;
    let flag_bytes = unknown.to_be_bytes();
    bytes[12..16].copy_from_slice(&flag_bytes);
    bytes[16..20].copy_from_slice(&flag_bytes);
    assert!(matches!(
        decode(&bytes).unwrap_err(),
        pixhaus_io::Error::UnknownRequiredFeatures { .. },
    ));
}

#[test]
fn rejects_inconsistent_feature_flags() {
    let mut bytes = encode(&empty_archive()).unwrap();
    // required_flags has a bit absent from feature_flags. Spec invariant
    // violation; reader rejects before any decompression work.
    let bit: u32 = 1 << 0;
    let bit_bytes = bit.to_be_bytes();
    // feature_flags stays at 0, required_flags carries bit 0.
    bytes[16..20].copy_from_slice(&bit_bytes);
    assert!(matches!(
        decode(&bytes).unwrap_err(),
        pixhaus_io::Error::InconsistentFeatureFlags {
            advertised: 0,
            required: 1
        },
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

// ── safety guards: decompression cap, schema version, write-time flag check

#[test]
fn rejects_decompression_bomb() {
    // 256 MiB + 1 byte of zeros compresses to a few KB but exceeds the
    // MAX_DECOMPRESSED_BODY cap on decode. Synthesising the file
    // directly (rather than via encode) keeps the test fast and lets
    // us bypass the now-stricter encode flag validation.
    const CAP: usize = 256 * 1024 * 1024;
    let bomb = vec![0u8; CAP + 1];
    let compressed = zstd::encode_all(bomb.as_slice(), 3).expect("compress");

    let mut bytes = Vec::with_capacity(28 + compressed.len());
    bytes.extend_from_slice(b"PIXHAUS\0"); // magic
    bytes.extend_from_slice(&1u16.to_be_bytes()); // format major
    bytes.extend_from_slice(&0u16.to_be_bytes()); // format minor
    bytes.extend_from_slice(&0u32.to_be_bytes()); // feature_flags
    bytes.extend_from_slice(&0u32.to_be_bytes()); // required_flags
    bytes.extend_from_slice(&(compressed.len() as u64).to_be_bytes());
    bytes.extend_from_slice(&compressed);

    let err = decode(&bytes).unwrap_err();
    assert!(matches!(
        err,
        pixhaus_io::Error::DecompressedTooLarge { .. },
    ));
}

#[test]
fn rejects_incompatible_schema_version() {
    let mut project = Project::new("future");
    // Bump the major past anything this build supports.
    project.schema_version = SchemaVersion {
        major: 99,
        minor: 0,
    };
    let archive = PixhausArchive::new(project);
    let bytes = encode(&archive).expect("encode failed");

    let err = decode(&bytes).unwrap_err();
    match err {
        pixhaus_io::Error::UnsupportedSchemaVersion { major, .. } => {
            assert_eq!(major, 99);
        }
        other => panic!("expected UnsupportedSchemaVersion, got: {other:?}"),
    }
}

/// Pre-B9 files were written by builds whose Project shape no longer
/// loads. The reader must reject these with `SchemaError::PreReleaseFile`
/// carrying the file's declared major, so the UI can route to "re-create
/// with the current version" rather than show a generic decode error.
#[test]
fn rejects_pre_release_file_with_typed_error() {
    let mut project = Project::new("pre-release");
    // The shape on disk is whatever the current Project writes; only the
    // schema_version field needs to be back-dated to look like a pre-B9
    // payload. The reader's gate runs against this field, not against
    // the surrounding structural drift.
    project.schema_version = SchemaVersion { major: 1, minor: 1 };
    let archive = PixhausArchive::new(project);
    let bytes = encode(&archive).expect("encode failed");

    let err = decode(&bytes).unwrap_err();
    match err {
        pixhaus_io::Error::Schema(SchemaError::PreReleaseFile { file_major }) => {
            assert_eq!(file_major, 1);
        }
        other => panic!("expected Schema(PreReleaseFile), got: {other:?}"),
    }
}

#[test]
fn write_rejects_unknown_feature_flags() {
    let mut archive = empty_archive();
    // Set a flag bit this build doesn't define.
    archive.project.feature_flags = FeatureFlags(1 << 31);
    let err = encode(&archive).unwrap_err();
    assert!(matches!(
        err,
        pixhaus_io::Error::UnknownRequiredFeatures { .. },
    ));
}

// ── filesystem helpers exercise encode_to_file / decode_from_file ──────────

#[test]
fn fs_round_trip_full_archive() {
    let archive = full_archive();
    // Unique filename under the OS temp dir; clean up at the end.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let path = std::env::temp_dir().join(format!(
        "pixhaus-fs-roundtrip-{}-{nanos}.pixhaus",
        std::process::id(),
    ));

    encode_to_file(&archive, &path).expect("encode_to_file should succeed");
    let loaded = decode_from_file(&path).expect("decode_from_file should succeed");
    let _ = std::fs::remove_file(&path);

    // Same depth as the in-memory round_trip: full structural equality
    // on Project plus per-buffer byte equality. Catches regressions in
    // encode_to_file/decode_from_file that the in-memory codec wouldn't
    // surface (path handling, tempfile rename, file size guard).
    assert_eq!(loaded.project, archive.project);
    assert_eq!(loaded.buffers.len(), archive.buffers.len());
    for (a, b) in archive.buffers.iter().zip(loaded.buffers.iter()) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.width, b.width);
        assert_eq!(a.height, b.height);
        assert_eq!(a.stride, b.stride);
        assert_eq!(a.pixels, b.pixels);
    }
}

// ── cel linkage survives round-trip ──────────────────────────────────────────

#[test]
fn linked_cel_round_trips_correctly() {
    use pixhaus_core::project::CelData;

    let project = sample_project();
    let hero_sprite_id = project
        .sprites_iter()
        .next()
        .expect("sample_project must have at least one sprite")
        .0
        .sprite
        .id;

    // Capture all needed data (all Copy types) before moving `project`.
    let (layer_id, frame_index, source_frame) = {
        let hero = project
            .sprite(hero_sprite_id)
            .expect("hero sprite resolves via library accessor");
        let linked = hero
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

    let back_hero = decoded
        .project
        .sprite(hero_sprite_id)
        .expect("hero sprite survives the round-trip");
    let back = back_hero
        .cels
        .iter()
        .find(|c| c.layer_id == layer_id && c.frame_index == frame_index)
        .expect("linked cel not found after round-trip");

    assert!(matches!(
        &back.data,
        CelData::Linked { source_frame: sf } if *sf == source_frame,
    ));
}
