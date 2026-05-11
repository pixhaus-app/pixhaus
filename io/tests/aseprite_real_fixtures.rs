//! Real-fixture aseprite round-trip coverage.
//!
//! Paused for the B9.1–B9.5 window: the importer / exporter returns a
//! typed error rather than translating, so the fixture round-trip
//! cannot complete. The single test below pins that contract on a real
//! file on disk so a regression that silently re-enables a broken
//! translation surfaces immediately.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::disallowed_methods
)]

use std::path::PathBuf;

use pixhaus_io::Error;
use pixhaus_io::aseprite::{decode_from_file, document_to_archive};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("io crate has parent")
        .join("examples")
        .join("aseprite-roundtrip")
        .join(name)
}

#[test]
fn legacy_aseprite_import_returns_typed_error() {
    let path = fixture("single-frame-rgba.aseprite");
    let doc = decode_from_file(&path).expect("decode fixture");
    let err = document_to_archive(&doc, "real-fixture").unwrap_err();
    assert!(
        matches!(err, Error::LegacyImportUnsupported { format: "aseprite" }),
        "expected LegacyImportUnsupported, got: {err:?}"
    );
}
