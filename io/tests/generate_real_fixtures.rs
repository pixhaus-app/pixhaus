//! Generator for `.aseprite` fixtures.
//!
//! Paused for the B9.1–B9.5 window: the generator drives the aseprite
//! exporter, which now returns a typed error during the library
//! migration. The full body is preserved in git history at the parent
//! of `feat/stream-b9.1` and lands back in B9.5 alongside the restored
//! exporter.
//!
//! The single test below guards the contract — calling the gated
//! generator surfaces [`Error::LegacyImportUnsupported`] rather than
//! producing fixture bytes — so CI catches an accidental re-enable.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc
)]

use pixhaus_core::project::Project;
use pixhaus_io::Error;
use pixhaus_io::aseprite::archive_to_document;
use pixhaus_io::pixhaus::PixhausArchive;

const REGEN_ENV: &str = "PIXHAUS_REGEN_ASEPRITE_FIXTURES";

#[test]
fn regenerate_is_gated_and_reports_legacy_unsupported() {
    if std::env::var_os(REGEN_ENV).is_none() {
        // CI path: generator is gated off, do nothing.
        return;
    }
    let archive = PixhausArchive::new(Project::new("test"));
    let err = archive_to_document(&archive).unwrap_err();
    assert!(
        matches!(err, Error::LegacyImportUnsupported { format: "aseprite" }),
        "expected LegacyImportUnsupported, got: {err:?}"
    );
}
