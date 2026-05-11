//! Stubbed during the B9 project-library migration.
//!
//! PSD import is read-only and references `Project.sprites`, which the
//! B9.1 cleanup removed. B9.5 restores the importer against the library
//! data model. Until then [`document_to_archive`] returns
//! [`Error::LegacyImportUnsupported`].

use crate::error::{Error, Result};
use crate::pixhaus::PixhausArchive;

/// Non-fatal issues encountered while converting a PSD document.
///
/// Empty during the B9.1–B9.5 window because the importer always errors
/// before raising a warning. Stays `pub` so downstream `match` arms
/// over the public API continue to compile; B9.5 fills the variant
/// list back in.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConversionWarning {}

/// Result of converting a PSD document into a [`PixhausArchive`].
///
/// Stays for API compatibility.
#[derive(Debug)]
pub struct ConvertedArchive {
    /// The translated archive ready for further encoding or editing.
    pub archive: PixhausArchive,
    /// Non-fatal warnings raised during conversion.
    pub warnings: Vec<ConversionWarning>,
}

/// Convert a parsed [`psd::Psd`] document into a [`PixhausArchive`].
///
/// # Errors
///
/// Returns [`Error::LegacyImportUnsupported`] unconditionally during
/// the B9.1–B9.5 window. B9.5 restores the importer to synthesise a
/// `Custom`-kind entity per PSD canvas rather than pushing into the
/// removed `Project.sprites` field.
pub fn document_to_archive(_psd: &psd::Psd, _name: &str) -> Result<ConvertedArchive> {
    Err(Error::LegacyImportUnsupported { format: "psd" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_to_archive_returns_legacy_import_unsupported() {
        // Smallest valid PSD header: 16x16 RGB 8-bit. The psd crate parses
        // far more than this in real files, but the function errors before
        // examining the document so any successfully-constructed Psd is
        // fine.
        let minimal_psd: Vec<u8> = build_minimal_psd_bytes();
        let psd = psd::Psd::from_bytes(&minimal_psd).expect("minimal psd parses");
        let err = document_to_archive(&psd, "test").unwrap_err();
        assert!(matches!(
            err,
            Error::LegacyImportUnsupported { format: "psd" }
        ));
    }

    fn build_minimal_psd_bytes() -> Vec<u8> {
        // 8BPS signature + version 1 + 6 reserved + 4 channels + height + width
        // + 8 bits/channel + RGB color mode, then four empty section
        // lengths (color mode, image resources, layer info, image data).
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
        // Image data: 0 = raw, then 1*1*3 bytes of pixel data.
        out.extend_from_slice(&0u16.to_be_bytes()); // compression
        out.extend_from_slice(&[0u8; 3]); // RGB pixel
        out
    }
}
