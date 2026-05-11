//! Photoshop `.psd` import for Pixhaus.
//!
//! Stubbed during the B9.1–B9.5 window: [`decode`] returns
//! [`Error::LegacyImportUnsupported`] until B9.5 reinstates the
//! library-aware importer. The pre-cleanup module-level contract (what
//! gets imported, what is approximated, the layer-ordering invariant)
//! lives in the git history at the parent of `feat/stream-b9.1` and
//! lands back alongside the restored importer.
//!
//! [`Error::PsdParse`]: crate::error::Error::PsdParse
//! [`Error::LegacyImportUnsupported`]: crate::error::Error::LegacyImportUnsupported

pub mod archive;
pub(crate) mod spec;

mod read;

pub use archive::{ConversionWarning, ConvertedArchive};
pub use read::{decode, decode_from_file};
