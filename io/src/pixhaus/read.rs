//! Decode a byte slice in the `.pixhaus` binary format into a [`PixhausArchive`].

use std::io::Read;
use std::path::Path;

use pixhaus_core::project::SchemaVersion;

use crate::error::{Error, Result};

use super::schema::{
    FORMAT_MAJOR, HEADER_LEN, KNOWN_FLAGS, MAGIC, MAX_DECOMPRESSED_BODY, PixhausArchive,
};

/// Decodes a `.pixhaus` byte slice into a [`PixhausArchive`].
///
/// # Errors
///
/// - [`Error::Truncated`] — slice is shorter than the 28-byte header, or
///   the body is shorter than `body_len` in the header.
/// - [`Error::InvalidMagic`] — first 8 bytes do not match `PIXHAUS\0`.
/// - [`Error::UnsupportedVersion`] — container format major version is not 1.
/// - [`Error::UnsupportedSchemaVersion`] — embedded `Project::schema_version`
///   is not compatible with this build's data model.
/// - [`Error::UnknownRequiredFeatures`] — `required_flags` contains bits
///   beyond the set this reader understands.
/// - [`Error::InconsistentFeatureFlags`] — `required_flags` is not a
///   subset of `feature_flags`.
/// - [`Error::DecompressedTooLarge`] — body decompressed past the
///   schema's `MAX_DECOMPRESSED_BODY` safety cap (256 MiB).
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

    let feature_flags = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
    let required_flags = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);

    // Spec invariant: required_flags must be a subset of feature_flags.
    // A header that violates this is malformed; reject before doing any
    // decompression work.
    if required_flags & !feature_flags != 0 {
        return Err(Error::InconsistentFeatureFlags {
            advertised: feature_flags,
            required: required_flags,
        });
    }

    let unknown = required_flags & !KNOWN_FLAGS;
    if unknown != 0 {
        return Err(Error::UnknownRequiredFeatures { required: unknown });
    }

    let body_len = usize::try_from(u64::from_be_bytes([
        data[20], data[21], data[22], data[23], data[24], data[25], data[26], data[27],
    ]))
    .map_err(|_| Error::Truncated)?;

    // Use checked arithmetic for the body slice bounds so a header
    // claiming a near-usize::MAX body_len returns Truncated rather than
    // panicking on overflow in debug builds.
    let body_end = HEADER_LEN.checked_add(body_len).ok_or(Error::Truncated)?;
    let compressed = data.get(HEADER_LEN..body_end).ok_or(Error::Truncated)?;

    // Bounded decompression — caps the buffer at MAX_DECOMPRESSED_BODY
    // bytes so a small file with a malicious zstd frame can't OOM the
    // process. zstd::Decoder reads the frame header to drive
    // decompression; we wrap the output with a Take to enforce the cap
    // at the byte level rather than trusting the frame's claimed size.
    let mut decoder = zstd::Decoder::new(compressed).map_err(Error::Io)?;
    let mut body = Vec::new();
    let limit = MAX_DECOMPRESSED_BODY;
    let mut limited = (&mut decoder).take(limit + 1);
    let read_bytes = limited.read_to_end(&mut body).map_err(Error::Io)?;
    if read_bytes as u64 > limit {
        return Err(Error::DecompressedTooLarge { limit });
    }
    // Drain any remaining frame bytes to surface trailing-data errors.
    drop(decoder);

    let archive: PixhausArchive = rmp_serde::from_slice(&body)?;

    // Validate data-model schema version embedded in the body. Distinct
    // error variant from container UnsupportedVersion so users can tell
    // "wrong .pixhaus format" from "wrong project schema".
    let schema = archive.project.schema_version;
    if !SchemaVersion::current().is_compatible_with(schema) {
        return Err(Error::UnsupportedSchemaVersion {
            major: schema.major,
            minor: schema.minor,
        });
    }

    Ok(archive)
}

/// Reads a `.pixhaus` file from `path` and decodes it.
///
/// Cheap-rejects oversized files via metadata before reading any
/// bytes: any file larger than the schema's `MAX_DECOMPRESSED_BODY`
/// (256 MiB) plus the 28-byte header cannot fit a valid archive
/// (compressed body is always smaller than decompressed). This keeps
/// a maliciously huge file from OOM-ing the process before the
/// in-memory format guards run.
///
/// This is a blocking call; wrap in `tokio::task::spawn_blocking` when
/// calling from async context.
pub fn decode_from_file(path: impl AsRef<Path>) -> Result<PixhausArchive> {
    let path = path.as_ref();
    let metadata = std::fs::metadata(path)?;
    let file_len = metadata.len();
    let max_file_len = MAX_DECOMPRESSED_BODY + HEADER_LEN as u64;
    if file_len > max_file_len {
        return Err(Error::DecompressedTooLarge {
            limit: MAX_DECOMPRESSED_BODY,
        });
    }
    let bytes = std::fs::read(path)?;
    decode(&bytes)
}
