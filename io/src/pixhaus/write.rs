//! Encode a [`PixhausArchive`] into the `.pixhaus` binary format.

use std::path::Path;

use crate::error::{Error, Result};

use super::schema::{FORMAT_MAJOR, FORMAT_MINOR, MAGIC, PixhausArchive, ZSTD_LEVEL};

/// Encodes `archive` into the `.pixhaus` binary format and returns the
/// resulting byte vector.
///
/// The body is MessagePack-serialized then zstd-compressed at the
/// schema's `ZSTD_LEVEL` (3 — zstd default). The 28-byte header is
/// prepended uncompressed.
pub fn encode(archive: &PixhausArchive) -> Result<Vec<u8>> {
    let body_raw = rmp_serde::to_vec_named(archive)?;
    let body = zstd::encode_all(body_raw.as_slice(), ZSTD_LEVEL).map_err(Error::Io)?;

    let feature_flags: u32 = archive.project.feature_flags.0;
    // v1.0: every present feature is required; future minors may relax this.
    let required_flags: u32 = feature_flags;
    let body_len: u64 = body.len() as u64;

    let mut out = Vec::with_capacity(28 + body.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT_MAJOR.to_be_bytes());
    out.extend_from_slice(&FORMAT_MINOR.to_be_bytes());
    out.extend_from_slice(&feature_flags.to_be_bytes());
    out.extend_from_slice(&required_flags.to_be_bytes());
    out.extend_from_slice(&body_len.to_be_bytes());
    out.extend_from_slice(&body);

    Ok(out)
}

/// Encodes `archive` and writes the result to `path`, creating or
/// truncating the file.
pub fn encode_to_file(archive: &PixhausArchive, path: impl AsRef<Path>) -> Result<()> {
    let bytes = encode(archive)?;
    std::fs::write(path, bytes)?;
    Ok(())
}
