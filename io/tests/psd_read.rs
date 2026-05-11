//! PSD import integration coverage.
//!
//! Paused for the B9.1–B9.5 window: [`pixhaus_io::psd::decode`] returns
//! [`Error::LegacyImportUnsupported`] until B9.5 reinstates the
//! importer against the library data model.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc
)]

use pixhaus_io::Error;
use pixhaus_io::psd::decode;

/// The smallest well-formed PSD byte sequence the `psd` crate accepts.
fn minimal_psd_bytes() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"8BPS"); // signature
    out.extend_from_slice(&1u16.to_be_bytes()); // version
    out.extend_from_slice(&[0u8; 6]); // reserved
    out.extend_from_slice(&3u16.to_be_bytes()); // channels (RGB)
    out.extend_from_slice(&1u32.to_be_bytes()); // height
    out.extend_from_slice(&1u32.to_be_bytes()); // width
    out.extend_from_slice(&8u16.to_be_bytes()); // bits/channel
    out.extend_from_slice(&3u16.to_be_bytes()); // RGB color mode
    out.extend_from_slice(&0u32.to_be_bytes()); // color mode data length
    out.extend_from_slice(&0u32.to_be_bytes()); // image resources length
    out.extend_from_slice(&0u32.to_be_bytes()); // layer/mask info length
    out.extend_from_slice(&0u16.to_be_bytes()); // compression
    out.extend_from_slice(&[0u8; 3]); // RGB pixel
    out
}

#[test]
fn psd_import_returns_legacy_import_unsupported() {
    let bytes = minimal_psd_bytes();
    let err = decode(&bytes, "test").unwrap_err();
    assert!(
        matches!(err, Error::LegacyImportUnsupported { format: "psd" }),
        "expected LegacyImportUnsupported, got: {err:?}"
    );
}
