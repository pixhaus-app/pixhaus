//! Encode a [`PixhausArchive`] into the `.pixhaus` binary format.

use std::path::Path;

use crate::error::{Error, Result};

use super::schema::{FORMAT_MAJOR, FORMAT_MINOR, KNOWN_FLAGS, MAGIC, PixhausArchive, ZSTD_LEVEL};

/// Encodes `archive` into the `.pixhaus` binary format and returns the
/// resulting byte vector.
///
/// The body is MessagePack-serialized then zstd-compressed at the
/// schema's `ZSTD_LEVEL` (3 — zstd default). The 28-byte header is
/// prepended uncompressed.
///
/// # Errors
///
/// - [`Error::UnknownRequiredFeatures`] — the project carries feature
///   flag bits this build does not recognise. Writing such a project
///   would either lie about required features (if the header masks
///   them silently) or burn an unreadable file (if it propagates them
///   into `required_flags`); refusing at write time keeps the contract
///   honest.
/// - [`Error::Serialize`] — `MessagePack` encoding failed.
/// - [`Error::Io`] — zstd compression failed.
pub fn encode(archive: &PixhausArchive) -> Result<Vec<u8>> {
    // Refuse to write unknown flag bits. Silent masking would produce a
    // header/body mismatch (header advertises fewer features than the
    // serialized Project actually uses); propagating them would burn an
    // unreadable file (the reader rejects unknown required bits). The
    // honest path is to error so the caller fixes the project before we
    // touch disk.
    let raw_flags = archive.project.feature_flags.0;
    let unknown = raw_flags & !KNOWN_FLAGS;
    if unknown != 0 {
        return Err(Error::UnknownRequiredFeatures { required: unknown });
    }
    let feature_flags = raw_flags;

    let body_raw = rmp_serde::to_vec_named(archive)?;
    let body = zstd::encode_all(body_raw.as_slice(), ZSTD_LEVEL).map_err(Error::Io)?;

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

/// Encodes `archive` and writes the result to `path`.
///
/// Atomic on Unix and Windows where the underlying filesystem honors
/// `rename`: writes to `<path>.tmp-<pid>` first, fsyncs, then renames
/// over the destination. A crash or disk-full mid-write leaves the
/// previous file at `path` intact rather than truncating it.
pub fn encode_to_file(archive: &PixhausArchive, path: impl AsRef<Path>) -> Result<()> {
    use std::fs::{File, rename};
    use std::io::Write;

    let bytes = encode(archive)?;
    let path = path.as_ref();
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));

    // Drop the file handle before the rename so Windows releases the
    // share-lock; fsync first so a crash leaves either the old file
    // intact or a complete new one — never a torn write under either.
    {
        let mut f = File::create(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    }
    if let Err(e) = rename(&tmp, path) {
        // Best-effort cleanup; ignore the error — the temp file is
        // self-identifying and a stale one is better than masking the
        // rename failure the caller actually needs to see.
        let _ = std::fs::remove_file(&tmp);
        return Err(Error::Io(e));
    }
    Ok(())
}
