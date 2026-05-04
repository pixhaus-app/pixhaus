//! Error types for the `pixhaus-io` crate.

use thiserror::Error;

/// Errors returned by the Pixhaus I/O crate.
#[derive(Debug, Error)]
pub enum Error {
    /// Byte sequence at offset 0 does not match the expected `PIXHAUS\0` magic.
    #[error("not a .pixhaus file")]
    InvalidMagic,

    /// The file's container format major version is higher than this
    /// reader supports. Distinct from
    /// [`Self::UnsupportedSchemaVersion`], which targets the embedded
    /// data-model version.
    #[error("unsupported format version {major}.{minor}")]
    UnsupportedVersion {
        /// Major version field from the file header.
        major: u16,
        /// Minor version field from the file header.
        minor: u16,
    },

    /// The data-model schema version embedded in the body is not
    /// compatible with this reader. Distinct from
    /// [`Self::UnsupportedVersion`] so error messages disambiguate
    /// container drift from project-format drift.
    #[error("unsupported project schema version {major}.{minor}")]
    UnsupportedSchemaVersion {
        /// Major version of `Project::schema_version`.
        major: u16,
        /// Minor version of `Project::schema_version`.
        minor: u16,
    },

    /// The file requires feature flags this reader doesn't understand.
    /// `required` contains only the unknown bits.
    #[error("file requires unknown feature flags: {required:#010x}")]
    UnknownRequiredFeatures {
        /// Bitmask of required flags not present in `KNOWN_FLAGS`.
        required: u32,
    },

    /// The header's `required_flags` includes bits absent from
    /// `feature_flags`. The spec mandates `required ⊆ feature` — this
    /// is the reader-side enforcement.
    #[error(
        "invalid feature flags: required {required:#010x} not subset of advertised {advertised:#010x}"
    )]
    InconsistentFeatureFlags {
        /// `feature_flags` field from the header (offset 12-15).
        advertised: u32,
        /// `required_flags` field from the header (offset 16-19).
        required: u32,
    },

    /// The compressed body's decompressed size exceeded the safety
    /// cap. Prevents a small `.pixhaus` file claiming a multi-GB
    /// decompressed body and OOM-ing the process before any
    /// deserialization runs.
    #[error("decompressed body exceeded safety cap of {limit} bytes")]
    DecompressedTooLarge {
        /// The cap that was breached, in bytes.
        limit: u64,
    },

    /// The byte slice ended before the expected field was reached.
    #[error("file is truncated or corrupt")]
    Truncated,

    /// A filesystem or stream operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The compressed body could not be decoded.
    #[error("deserialization failed: {0}")]
    Deserialize(#[from] rmp_serde::decode::Error),

    /// The archive could not be encoded to `MessagePack`.
    #[error("serialization failed: {0}")]
    Serialize(#[from] rmp_serde::encode::Error),
}

/// Crate-local result alias.
pub type Result<T> = std::result::Result<T, Error>;
