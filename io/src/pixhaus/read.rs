//! Decode a byte slice in the `.pixhaus` binary format into a [`PixhausArchive`].

use std::io::Read;
use std::path::Path;

use pixhaus_core::project::{SchemaError, SchemaVersion};

use crate::error::{Error, Result};

use super::migrate;
use super::schema::{
    FORMAT_MAJOR, HEADER_LEN, KNOWN_FLAGS, MAGIC, MAX_DECOMPRESSED_BODY, PixhausArchive,
};

/// Parses + validates the header and locates the compressed body slice.
///
/// Validates magic, the required-flags subset invariant, and the
/// known-required-flags check. Bounds-checks the body slice. These checks
/// run on every path (v1 fast path *and* older-major migration path) so a
/// header that should be rejected up-front never reaches decompression.
fn parse_header(data: &[u8]) -> Result<(u16, u16, &[u8])> {
    if data.len() < HEADER_LEN {
        return Err(Error::Truncated);
    }
    if &data[0..8] != MAGIC.as_slice() {
        return Err(Error::InvalidMagic);
    }
    let major = u16::from_be_bytes([data[8], data[9]]);
    let minor = u16::from_be_bytes([data[10], data[11]]);

    let feature_flags = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
    let required_flags = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);

    // Spec invariant: required_flags must be a subset of feature_flags.
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
    // Use checked arithmetic so a header claiming a near-usize::MAX
    // body_len returns Truncated rather than panicking on overflow in
    // debug builds.
    let body_end = HEADER_LEN.checked_add(body_len).ok_or(Error::Truncated)?;
    let body = data.get(HEADER_LEN..body_end).ok_or(Error::Truncated)?;
    Ok((major, minor, body))
}

/// Decodes a `MessagePack` body into a `PixhausArchive` and validates the
/// embedded data-model schema version.
///
/// Shared by the v1 fast path and the migration path so future format
/// bumps don't drift from the v1 validation logic.
///
/// Three reject cases on the embedded `Project::schema_version`:
/// - file's MAJOR below current (`< SchemaVersion::MAJOR`): a pre-release
///   build wrote this file; the stable build cannot load it. Surfaces
///   [`SchemaError::PreReleaseFile`] so the UI can guide the user to
///   re-create the project.
/// - file's MAJOR above current: a newer build wrote this file; this
///   reader is incompatible. Surfaces
///   [`Error::UnsupportedSchemaVersion`].
/// - file's MAJOR matches and MINOR may exceed: forward-compatible;
///   unknown fields are ignored by `serde(default)`. The loader still
///   emits a `tracing::warn!` so any downstream serde failure on an
///   unknown enum variant (e.g. a future blend mode) is legible in logs.
fn deserialize_and_validate(body: &[u8]) -> Result<PixhausArchive> {
    let archive: PixhausArchive = rmp_serde::from_slice(body)?;
    let schema = archive.project.schema_version;
    let current = SchemaVersion::current();
    if schema.major < current.major {
        return Err(Error::Schema(SchemaError::PreReleaseFile {
            file_major: schema.major,
        }));
    }
    if !current.is_compatible_with(schema) {
        return Err(Error::UnsupportedSchemaVersion {
            major: schema.major,
            minor: schema.minor,
        });
    }
    if schema.minor > current.minor {
        // Forward-compatible load: same major, newer minor. Additive
        // fields fall through #[serde(default)], but a future minor
        // may also introduce new enum variants that this reader has
        // not heard of — those would have already failed
        // deserialization above. Warning here makes downstream serde
        // failures legible in the log timeline.
        tracing::warn!(
            file_minor = schema.minor,
            reader_minor = current.minor,
            "loading .pixhaus file written by a newer minor schema; \
             unknown fields are tolerated but unknown enum variants \
             will fail to deserialize"
        );
    }
    Ok(archive)
}

/// Decodes a `.pixhaus` byte slice into a [`PixhausArchive`].
///
/// # Errors
///
/// - [`Error::Truncated`] — slice is shorter than the 28-byte header, or
///   the body is shorter than `body_len` in the header.
/// - [`Error::InvalidMagic`] — first 8 bytes do not match `PIXHAUS\0`.
/// - [`Error::UnsupportedVersion`] — container format major version is not 1
///   and no migration step covers it (at v1.0 no steps are registered, so
///   any older major returns this — but only after the header passes the
///   `Truncated`, `InvalidMagic`, `InconsistentFeatureFlags`, and
///   `UnknownRequiredFeatures` checks above).
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
    let (major, minor, compressed) = parse_header(data)?;

    if major > FORMAT_MAJOR {
        // Newer container format: this build has no path to open it.
        return Err(Error::UnsupportedVersion { major, minor });
    }
    if major < FORMAT_MAJOR {
        // Older format: attempt migration. apply_chain decompresses the body
        // and applies registered transformation steps, returning a decompressed
        // body in the current format. At v1.0 no migrations are registered so
        // this yields UnsupportedVersion for every older-major file that gets
        // this far — future format bumps add steps to migrate.rs without
        // touching this dispatch logic.
        let body = migrate::apply_chain(major, minor, compressed, MAX_DECOMPRESSED_BODY)?;
        return deserialize_and_validate(&body);
    }
    // major == FORMAT_MAJOR; minor is forward-compatible: ignore fields
    // we don't recognise.

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

    deserialize_and_validate(&body)
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
