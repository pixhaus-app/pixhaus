//! Decode a byte slice in the `.pixhaus` binary format into a [`PixhausArchive`].

use std::path::Path;

use pixhaus_core::project::SchemaVersion;

use crate::error::{Error, Result};

use super::schema::{FORMAT_MAJOR, HEADER_LEN, KNOWN_FLAGS, MAGIC, PixhausArchive};

/// Decodes a `.pixhaus` byte slice into a [`PixhausArchive`].
///
/// # Errors
///
/// - [`Error::Truncated`] — slice is shorter than the 28-byte header, or
///   the body is shorter than `body_len` in the header.
/// - [`Error::InvalidMagic`] — first 8 bytes do not match `PIXHAUS\0`.
/// - [`Error::UnsupportedVersion`] — format major version is not 1.
/// - [`Error::UnknownRequiredFeatures`] — `required_flags` contains bits
///   beyond the set this reader understands.
/// - [`Error::Io`] — zstd decompression failed.
/// - [`Error::Deserialize`] — `MessagePack` body could not be decoded.
pub fn decode(data: &[u8]) -> Result<PixhausArchive> {
    if data.len() < HEADER_LEN {
        return Err(Error::Truncated);
    }

    if &data[0..8] != MAGIC.as_slice() {
        return Err(Error::InvalidMagic);
    }

    let major = u16::from_be_bytes([data[8], data[9]]);
    let minor = u16::from_be_bytes([data[10], data[11]]);

    if major != FORMAT_MAJOR {
        return Err(Error::UnsupportedVersion { major, minor });
    }
    // minor is forward-compatible: ignore fields we don't recognise.

    // feature_flags in the header is advisory; project.feature_flags inside
    // the body is authoritative. We only need required_flags here.
    let required_flags = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);

    let unknown = required_flags & !KNOWN_FLAGS;
    if unknown != 0 {
        return Err(Error::UnknownRequiredFeatures { required: unknown });
    }

    let body_len = usize::try_from(u64::from_be_bytes([
        data[20], data[21], data[22], data[23], data[24], data[25], data[26], data[27],
    ]))
    .map_err(|_| Error::Truncated)?;

    let compressed = data
        .get(HEADER_LEN..HEADER_LEN + body_len)
        .ok_or(Error::Truncated)?;

    let body = zstd::decode_all(compressed).map_err(Error::Io)?;

    let archive: PixhausArchive = rmp_serde::from_slice(&body)?;

    // Validate data-model schema version embedded in the body.
    let schema = archive.project.schema_version;
    if !SchemaVersion::current().is_compatible_with(schema) {
        return Err(Error::UnsupportedVersion {
            major: schema.major,
            minor: schema.minor,
        });
    }

    Ok(archive)
}

/// Reads a `.pixhaus` file from `path` and decodes it.
///
/// This is a blocking call; wrap in `tokio::task::spawn_blocking` when
/// calling from async context.
pub fn decode_from_file(path: impl AsRef<Path>) -> Result<PixhausArchive> {
    let bytes = std::fs::read(path)?;
    decode(&bytes)
}
