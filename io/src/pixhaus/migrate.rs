//! Schema migration for `.pixhaus` files from older format versions.
//!
//! # Chain contract
//!
//! Each migration step receives **zstd-compressed** body bytes (the same
//! format on disk), decompresses internally, transforms the `MessagePack`
//! bytes to match the next version, and re-compresses before handing off
//! to the following step. The final step decompresses one last time and
//! returns the **decompressed** bytes, ready for `rmp_serde::from_slice`.
//!
//! Re-compressing between steps keeps each transform isolated and
//! independently testable: a step author writes `Vec<u8> -> Vec<u8>` over
//! decompressed bodies and the chain handles the (de)compression bookend.
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
//!     // re-compress for the next step's input contract
//!     compressed = zstd::encode_all(&*v1_to_v2(raw)?, 3).map_err(Error::Io)?;
//!     major = 2;
//! }
//! if major == FORMAT_MAJOR {
//!     // final step: caller wants decompressed bytes
//!     return decompress(&compressed, limit);
//! }
//! ```

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
