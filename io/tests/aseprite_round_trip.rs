//! Aseprite round-trip integration coverage.
//!
//! The full corpus is paused until B9.5 reinstates the importer /
//! exporter against the library data model. During the B9.1–B9.5
//! window every translation path returns
//! [`pixhaus_io::Error::LegacyImportUnsupported`]; the assertions here
//! pin that contract so a regression that silently re-enables a broken
//! translation surfaces immediately, and so the test harness keeps
//! exercising the public API while the heavy fixtures sit idle.
//!
//! When B9.5 restores the importer, replace these stubs with the
//! pre-cleanup body kept in the git history at `feat/stream-b9.1`'s
//! parent commit.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc
)]

use pixhaus_core::project::Project;
use pixhaus_io::Error;
use pixhaus_io::aseprite::{
    AsepriteDocument, DocumentFrame, archive_to_document, document_to_archive,
};
use pixhaus_io::pixhaus::PixhausArchive;

#[test]
fn document_to_archive_returns_legacy_import_unsupported() {
    let mut doc = AsepriteDocument::empty(8, 8);
    doc.frames.push(DocumentFrame::new(100));
    let err = document_to_archive(&doc, "test").unwrap_err();
    assert!(
        matches!(err, Error::LegacyImportUnsupported { format: "aseprite" }),
        "expected LegacyImportUnsupported, got: {err:?}"
    );
}

#[test]
fn archive_to_document_returns_legacy_import_unsupported() {
    let archive = PixhausArchive::new(Project::new("test"));
    let err = archive_to_document(&archive).unwrap_err();
    assert!(
        matches!(err, Error::LegacyImportUnsupported { format: "aseprite" }),
        "expected LegacyImportUnsupported, got: {err:?}"
    );
}
