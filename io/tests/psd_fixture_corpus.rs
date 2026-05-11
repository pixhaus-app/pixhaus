//! Real-world PSD fixture coverage.
//!
//! Paused for the B9.1–B9.5 window: the importer returns a typed error
//! rather than translating a PSD into a Pixhaus archive. Fixtures and
//! their generation instructions are kept in the git history at the
//! parent of `feat/stream-b9.1`; B9.5 restores the test body alongside
//! the library-aware importer.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::disallowed_methods
)]

use std::path::PathBuf;

use pixhaus_io::Error;
use pixhaus_io::psd::decode_from_file;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("io crate has parent")
        .join("examples")
        .join("psd-fixtures")
}

#[test]
fn psd_fixture_import_returns_typed_error_when_present() {
    let dir = fixture_dir();
    if !dir.is_dir() {
        // Fixtures are not committed; this test is a no-op in their
        // absence. When B9.5 restores the importer, replace this body
        // with the real round-trip assertions from git history.
        return;
    }
    // Pick any file; the importer rejects before reading the document.
    let Some(entry) = std::fs::read_dir(&dir)
        .expect("read fixture dir")
        .filter_map(Result::ok)
        .find(|e| e.path().extension().and_then(|s| s.to_str()) == Some("psd"))
    else {
        return;
    };
    let err = decode_from_file(entry.path()).unwrap_err();
    assert!(
        matches!(err, Error::LegacyImportUnsupported { format: "psd" }),
        "expected LegacyImportUnsupported, got: {err:?}"
    );
}
