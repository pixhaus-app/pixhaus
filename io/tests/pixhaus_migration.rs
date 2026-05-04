//! Migration dispatch tests for the `.pixhaus` format.
//!
//! At v1.0 there are no prior format versions to migrate from, so
//! [`pixhaus_io::pixhaus::migrate`]'s `apply_chain` immediately returns
//! `UnsupportedVersion` for any `major < FORMAT_MAJOR`. These tests lock in
//! the dispatch contract so future contributors can see exactly which
//! assertion to extend when they add a real migration step.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc
)]

use pixhaus_io::pixhaus::decode;

// ── synthetic-fixture helpers ─────────────────────────────────────────────────

/// Builds a minimal syntactically-valid `.pixhaus` byte buffer whose container
/// format version is set to `(major, minor)`.
///
/// The body is intentionally empty (`body_len = 0`) — the migration dispatch
/// rejects the file before touching the body, so body content is irrelevant.
fn v0_header(major: u16, minor: u16) -> Vec<u8> {
    let mut buf = Vec::with_capacity(28);
    buf.extend_from_slice(b"PIXHAUS\0"); // magic (8 B)
    buf.extend_from_slice(&major.to_be_bytes()); // format_major (2 B)
    buf.extend_from_slice(&minor.to_be_bytes()); // format_minor (2 B)
    buf.extend_from_slice(&0u32.to_be_bytes()); // feature_flags (4 B)
    buf.extend_from_slice(&0u32.to_be_bytes()); // required_flags (4 B)
    buf.extend_from_slice(&0u64.to_be_bytes()); // body_len = 0 (8 B)
    buf
}

// ── dispatch-path tests ───────────────────────────────────────────────────────

/// Verifies that a v0.0 file reaches `migrate::apply_chain` and surfaces
/// `UnsupportedVersion` — proving the dispatch hook in `decode` is wired up.
///
/// When v1.x needs to ingest v0 files: add the migration step to
/// `migrate::apply_chain`, then change this assertion to expect a successful
/// decode (or to check the migrated content) rather than `UnsupportedVersion`.
#[test]
fn v0_file_reaches_migration_dispatch_and_returns_unsupported_version() {
    let bytes = v0_header(0, 0);
    let err = decode(&bytes).unwrap_err();
    assert!(
        matches!(
            err,
            pixhaus_io::Error::UnsupportedVersion { major: 0, minor: 0 }
        ),
        "expected UnsupportedVersion {{ major: 0, minor: 0 }}, got: {err:?}",
    );
}

/// Same dispatch path, non-zero minor — confirms minor is threaded through
/// `apply_chain` unmodified so a future migration step can inspect it.
#[test]
fn v0_file_with_nonzero_minor_preserves_minor_in_error() {
    let bytes = v0_header(0, 7);
    let err = decode(&bytes).unwrap_err();
    assert!(
        matches!(
            err,
            pixhaus_io::Error::UnsupportedVersion { major: 0, minor: 7 }
        ),
        "expected UnsupportedVersion {{ major: 0, minor: 7 }}, got: {err:?}",
    );
}
