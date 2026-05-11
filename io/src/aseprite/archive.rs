//! Stubbed during the B9 project-library migration.
//!
//! Both translation directions reference `Project.sprites`, which the
//! B9.1 cleanup removed. Restoring the importer / exporter against the
//! library data model is the B9.5 work item. Until then both functions
//! return [`Error::LegacyImportUnsupported`] so the editor can surface
//! a typed, user-readable error rather than crashing on a missing
//! field.
//!
//! The public types ([`ConvertedArchive`], [`ConversionWarning`]) stay
//! to keep downstream `use` statements compiling; B9.5 fills the
//! variant list back in.

use crate::error::{Error, Result};
use crate::pixhaus::PixhausArchive;

use super::document::AsepriteDocument;

/// Non-fatal warnings produced during conversion.
///
/// Empty during the B9.1–B9.5 window because every translation path
/// returns an error before any warning can be raised. The enum stays
/// `pub` so downstream `match` arms over the public API continue to
/// compile; B9.5 repopulates the variant list.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConversionWarning {}

/// Result of converting an [`AsepriteDocument`] into a [`PixhausArchive`].
///
/// Stays for API compatibility; constructed only by [`document_to_archive`]
/// once B9.5 reinstates the importer.
#[derive(Clone, Debug)]
pub struct ConvertedArchive {
    /// The translated archive.
    pub archive: PixhausArchive,
    /// Warnings raised during conversion.
    pub warnings: Vec<ConversionWarning>,
}

/// Translate an [`AsepriteDocument`] into a [`PixhausArchive`].
///
/// # Errors
///
/// Returns [`Error::LegacyImportUnsupported`] unconditionally during
/// the B9.1–B9.5 window. The implementation is restored in B9.5 with a
/// library-aware translation that produces a `Custom`-kind entity per
/// imported sprite instead of pushing into the removed
/// `Project.sprites` field.
pub fn document_to_archive(
    _doc: &AsepriteDocument,
    _sprite_name: impl Into<String>,
) -> Result<ConvertedArchive> {
    Err(Error::LegacyImportUnsupported { format: "aseprite" })
}

/// Translate a [`PixhausArchive`] into an [`AsepriteDocument`].
///
/// # Errors
///
/// Returns [`Error::LegacyImportUnsupported`] unconditionally during
/// the B9.1–B9.5 window. The exporter reads `Project.sprites`, which
/// no longer exists; B9.5 walks `Project.library` instead.
pub fn archive_to_document(_archive: &PixhausArchive) -> Result<AsepriteDocument> {
    Err(Error::LegacyImportUnsupported { format: "aseprite" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pixhaus::PixhausArchive;
    use pixhaus_core::project::Project;

    #[test]
    fn document_to_archive_returns_legacy_import_unsupported() {
        let doc = AsepriteDocument::empty(8, 8);
        let err = document_to_archive(&doc, "test").unwrap_err();
        assert!(matches!(
            err,
            Error::LegacyImportUnsupported { format: "aseprite" }
        ));
    }

    #[test]
    fn archive_to_document_returns_legacy_import_unsupported() {
        let archive = PixhausArchive::new(Project::new("test"));
        let err = archive_to_document(&archive).unwrap_err();
        assert!(matches!(
            err,
            Error::LegacyImportUnsupported { format: "aseprite" }
        ));
    }
}
