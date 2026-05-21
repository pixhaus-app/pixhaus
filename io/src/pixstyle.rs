//! Portable export/import bundle for composition records. Same `MessagePack` +
//! zstd stack as the .pixhaus project file. Per spec section 10.2-10.3.

use std::io::{Read, Write};

use pixhaus_core::project::library::ProjectAi;
use pixhaus_core::project::library::composition::{PromptTemplate, Structure, Style};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const PIXSTYLE_MAGIC: &[u8; 4] = b"PXST";
const PIXSTYLE_FORMAT_VERSION: u16 = 1;

/// Error reading or writing a `.pixstyle` bundle.
#[derive(Debug, Error)]
pub enum PixstyleError {
    /// Underlying I/O failure.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// The stream did not start with the `.pixstyle` magic bytes.
    #[error("bad magic: not a .pixstyle bundle")]
    BadMagic,
    /// The bundle's format version is newer than this build supports.
    #[error("unsupported format version {0}")]
    UnsupportedVersion(u16),
    /// `MessagePack` decode failure.
    #[error("decode: {0}")]
    Decode(#[from] rmp_serde::decode::Error),
    /// `MessagePack` encode failure.
    #[error("encode: {0}")]
    Encode(#[from] rmp_serde::encode::Error),
}

/// A portable bundle of composition records.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StylePack {
    /// Bundle format version.
    pub format_version: u16,
    /// Exported Structures.
    pub structures: Vec<Structure>,
    /// Exported Styles.
    pub styles: Vec<Style>,
    /// Exported saved Prompts.
    pub prompts: Vec<PromptTemplate>,
}

/// Writes `pack` to `w` as magic + version + zstd-compressed `MessagePack`.
///
/// # Errors
/// Returns [`PixstyleError`] on I/O, encode, or compression failure.
pub fn write_pack(pack: &StylePack, mut w: impl Write) -> Result<(), PixstyleError> {
    w.write_all(PIXSTYLE_MAGIC)?;
    w.write_all(&PIXSTYLE_FORMAT_VERSION.to_le_bytes())?;
    let body = rmp_serde::to_vec_named(pack)?;
    let compressed = zstd::encode_all(&body[..], 0)?;
    w.write_all(&compressed)?;
    Ok(())
}

/// Reads a [`StylePack`] from `r`, validating the magic and version header.
///
/// # Errors
/// Returns [`PixstyleError`] on bad magic, unsupported version, I/O, or
/// decode failure.
pub fn read_pack(mut r: impl Read) -> Result<StylePack, PixstyleError> {
    let mut magic = [0u8; 4];
    r.read_exact(&mut magic)?;
    if &magic != PIXSTYLE_MAGIC {
        return Err(PixstyleError::BadMagic);
    }
    let mut ver = [0u8; 2];
    r.read_exact(&mut ver)?;
    let version = u16::from_le_bytes(ver);
    if version != PIXSTYLE_FORMAT_VERSION {
        return Err(PixstyleError::UnsupportedVersion(version));
    }
    let mut compressed = Vec::new();
    r.read_to_end(&mut compressed)?;
    let body = zstd::decode_all(&compressed[..])?;
    let pack: StylePack = rmp_serde::from_slice(&body)?;
    Ok(pack)
}

/// Extracts a [`StylePack`] from an existing project's [`ProjectAi`] for
/// copy-from-project. The source project is read-only and left untouched.
#[must_use]
pub fn read_library_from_project_ai(ai: &ProjectAi) -> StylePack {
    StylePack {
        format_version: PIXSTYLE_FORMAT_VERSION,
        structures: ai.structures.clone(),
        styles: ai.styles.clone(),
        prompts: ai.prompts.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::library::composition::{
        StructureId, StructureOutput, Style, StyleId,
    };

    fn sample_pack() -> StylePack {
        StylePack {
            format_version: PIXSTYLE_FORMAT_VERSION,
            structures: vec![Structure {
                id: StructureId("p.s".into()),
                name: "P".into(),
                output: StructureOutput::Single,
                layout_negatives: String::new(),
            }],
            styles: vec![],
            prompts: vec![],
        }
    }

    fn sample_style() -> Style {
        Style {
            id: StyleId("p.style".into()),
            name: "Sample".into(),
            modifiers: "16-bit".into(),
            look_negatives: String::new(),
            model_pref: None,
            quality: None,
        }
    }

    #[test]
    fn round_trips_through_bytes() {
        let mut buf = Vec::new();
        write_pack(&sample_pack(), &mut buf).unwrap();
        let back = read_pack(&buf[..]).unwrap();
        assert_eq!(back, sample_pack());
    }

    #[test]
    fn rejects_bad_magic() {
        let err = read_pack(&b"XXXX\x01\x00"[..]).unwrap_err();
        assert!(matches!(err, PixstyleError::BadMagic));
    }

    #[test]
    fn rejects_future_version() {
        let mut buf = Vec::new();
        buf.extend_from_slice(PIXSTYLE_MAGIC);
        buf.extend_from_slice(&99u16.to_le_bytes());
        buf.extend_from_slice(
            &zstd::encode_all(&rmp_serde::to_vec_named(&sample_pack()).unwrap()[..], 0).unwrap(),
        );
        assert!(matches!(
            read_pack(&buf[..]).unwrap_err(),
            PixstyleError::UnsupportedVersion(99)
        ));
    }

    #[test]
    fn extracts_library_from_project_ai() {
        let mut ai = ProjectAi::default();
        ai.styles.push(sample_style());
        let pack = read_library_from_project_ai(&ai);
        assert_eq!(pack.styles.len(), 1);
        assert_eq!(pack.format_version, PIXSTYLE_FORMAT_VERSION);
    }
}
