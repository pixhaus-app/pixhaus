//! Schema migration for `.pixhaus` files from older format versions.
//!
//! When the container format major version bumps past 1, add a migration
//! step to [`apply_chain`] so older files can still be opened. Each step
//! receives the raw zstd-compressed body from the old file, decompresses it,
//! transforms the `MessagePack` bytes to match the next version, and returns
//! a decompressed body ready for the following step (or for deserialization
//! if it's the final step).
//!
//! At format v1.0 the chain is empty — there is no prior format to migrate
//! from.
//!
//! # Adding a migration
//!
//! When `FORMAT_MAJOR` bumps to 2, add a step inside [`apply_chain`]:
//!
//! ```ignore
//! if major == 1 {
//!     let raw = decompress(compressed, limit)?;
//!     compressed = zstd::encode_all(&*v1_to_v2(raw)?, 3).map_err(Error::Io)?;
//!     major = 2;
//! }
//! if major == FORMAT_MAJOR {
//!     return decompress(&compressed, limit);
//! }
//! ```
//!
//! The pattern decompresses the old body, transforms it, then re-compresses
//! before handing off to the next step — keeping each step's transform
//! isolated and independently testable.

use crate::error::{Error, Result};

/// Attempts to upgrade an old-format compressed body to the current format.
///
/// `major` and `minor` identify the source container format version.
/// `compressed` is the raw zstd-compressed body from the file. `limit` is
/// the decompression safety cap that applies to each intermediate step.
///
/// Returns the **decompressed** body in the current format on success, ready
/// for `rmp_serde::from_slice`. Returns [`Error::UnsupportedVersion`] if no
/// migration path exists for the given version.
pub(super) fn apply_chain(
    major: u16,
    minor: u16,
    _compressed: &[u8],
    _limit: u64,
) -> Result<Vec<u8>> {
    // No migrations defined yet. See module-level doc for how to add one.
    Err(Error::UnsupportedVersion { major, minor })
}
