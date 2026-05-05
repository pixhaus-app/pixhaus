//! PSD file decoding: entry points for loading from bytes or from disk.

use std::path::Path;

use crate::error::{Error, Result};

use super::archive::{self, ConvertedArchive};

/// Decode a PSD file from a byte slice.
///
/// `name` is used as the sprite name in the resulting Pixhaus project.
///
/// # Errors
///
/// Returns [`Error::PsdParse`] for any parse error reported by the `psd`
/// crate, or for color modes/bit depths that cannot be converted.
pub fn decode(bytes: &[u8], name: &str) -> Result<ConvertedArchive> {
    let psd = psd::Psd::from_bytes(bytes).map_err(|e| Error::PsdParse(format!("{e:?}")))?;
    archive::document_to_archive(&psd, name)
}

/// Decode a PSD file from the filesystem.
///
/// The sprite name defaults to the file stem (the filename without extension).
///
/// # Errors
///
/// Returns [`Error::Io`] if the file cannot be read, [`Error::PsdParse`]
/// for any parse or conversion error.
pub fn decode_from_file(path: impl AsRef<Path>) -> Result<ConvertedArchive> {
    let path = path.as_ref();
    let bytes = std::fs::read(path)?;
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    decode(&bytes, name)
}
