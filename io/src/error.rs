//! Error types for the `pixhaus-io` crate.

use thiserror::Error;

/// Errors returned by the Pixhaus I/O crate.
#[derive(Debug, Error)]
pub enum Error {
    /// Byte sequence at offset 0 does not match the expected `PIXHAUS\0` magic.
    #[error("not a .pixhaus file")]
    InvalidMagic,

    /// The file's format major version is higher than this reader supports.
    #[error("unsupported format version {major}.{minor}")]
    UnsupportedVersion {
        /// Major version field from the file header or body schema.
        major: u16,
        /// Minor version field from the file header or body schema.
        minor: u16,
    },

    /// The file requires feature flags this reader doesn't understand.
    /// `required` contains only the unknown bits.
    #[error("file requires unknown feature flags: {required:#010x}")]
    UnknownRequiredFeatures {
        /// Bitmask of required flags not present in `KNOWN_FLAGS`.
        required: u32,
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
