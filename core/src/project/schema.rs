//! Schema version and feature flags for the Pixhaus data model.
//!
//! These appear at the top of every serialized project. The version is
//! a contract between writers and readers: additive changes bump
//! [`SchemaVersion::MINOR`], breaking changes bump [`SchemaVersion::MAJOR`]
//! and require a migration. The feature-flag bitfield lets a writer
//! advertise which optional sub-features are present without bumping
//! the version every time.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Versioned identity of the data model.
///
/// Encoded explicitly so old binaries can refuse to load files written
/// by a newer major version, and so newer binaries can transparently
/// upgrade older minor versions.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct SchemaVersion {
    /// Breaking-change counter. A reader must reject files whose
    /// `major` is higher than the one it was built against.
    pub major: u16,
    /// Additive-change counter. A reader may load files with a higher
    /// `minor` than its own, ignoring fields it doesn't recognise.
    pub minor: u16,
}

impl SchemaVersion {
    /// Major version this build of Pixhaus writes.
    pub const MAJOR: u16 = 4;

    /// Minor version this build of Pixhaus writes.
    pub const MINOR: u16 = 2;

    /// The version emitted by this build.
    #[must_use]
    pub const fn current() -> Self {
        Self {
            major: Self::MAJOR,
            minor: Self::MINOR,
        }
    }

    /// Returns `true` if a reader at `self` can load a file written at
    /// `other`. Same-major, any-minor is loadable; a newer major is not.
    #[must_use]
    pub const fn is_compatible_with(self, other: Self) -> bool {
        self.major == other.major
    }
}

impl Default for SchemaVersion {
    fn default() -> Self {
        Self::current()
    }
}

/// Optional features advertised by the writer.
///
/// Every flag is a single bit. Adding a flag is additive (bump
/// [`SchemaVersion::MINOR`]); removing or repurposing one is a break
/// (bump `MAJOR`). The serialized form is a `u32` so flags persist as
/// a compact bitfield rather than a JSON array.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FeatureFlags(pub u32);

impl FeatureFlags {
    /// File contains tilemap layers and tilesets.
    pub const TILEMAPS: Self = Self(1 << 0);

    /// File contains reference layers (non-exported visual aids).
    pub const REFERENCES: Self = Self(1 << 1);

    /// File contains animation entries beyond raw frame tags.
    pub const ANIMATIONS: Self = Self(1 << 2);

    /// File contains slices with nine-slice or pivot metadata.
    pub const SLICES: Self = Self(1 << 3);

    /// File contains AI verb history (preview-then-commit transcripts).
    pub const VERB_HISTORY: Self = Self(1 << 4);

    /// All flags currently defined by this build.
    pub const ALL: Self = Self(
        Self::TILEMAPS.0
            | Self::REFERENCES.0
            | Self::ANIMATIONS.0
            | Self::SLICES.0
            | Self::VERB_HISTORY.0,
    );

    /// Returns the empty set.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Returns the union of two flag sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns `true` if every flag in `other` is also set in `self`.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// Schema-level rejection reasons surfaced by readers.
///
/// Distinct from container-format errors: this carries data-model
/// versioning failures so the UI can render targeted guidance
/// (re-create a pre-release file, upgrade Pixhaus to load a newer file)
/// rather than a generic "could not open" string.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SchemaError {
    /// File was written by a pre-release Pixhaus (major version below
    /// the current one). These builds shipped data shapes that the
    /// stable build can no longer load — the only path forward is to
    /// re-create the project with the current build.
    #[error(
        "This file was created with a pre-release Pixhaus and is not supported. \
         Re-create it with the current version."
    )]
    PreReleaseFile {
        /// The file's recorded major version, surfaced in error logs.
        file_major: u16,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_major_is_compatible() {
        let v = SchemaVersion { major: 4, minor: 5 };
        let other = SchemaVersion { major: 4, minor: 0 };
        assert!(v.is_compatible_with(other));
    }

    #[test]
    fn newer_major_is_incompatible() {
        let v = SchemaVersion { major: 4, minor: 0 };
        let other = SchemaVersion { major: 5, minor: 0 };
        assert!(!v.is_compatible_with(other));
    }

    #[test]
    fn current_version_is_major_4_minor_2() {
        let v = SchemaVersion::current();
        assert_eq!(v.major, 4);
        assert_eq!(v.minor, 2);
    }

    #[test]
    fn pre_release_file_error_carries_major() {
        let err = SchemaError::PreReleaseFile { file_major: 1 };
        assert!(matches!(err, SchemaError::PreReleaseFile { file_major: 1 }));
    }

    #[test]
    fn flags_union_and_contain() {
        let f = FeatureFlags::TILEMAPS.union(FeatureFlags::SLICES);
        assert!(f.contains(FeatureFlags::TILEMAPS));
        assert!(f.contains(FeatureFlags::SLICES));
        assert!(!f.contains(FeatureFlags::ANIMATIONS));
    }

    #[test]
    fn empty_flags_serialize_as_zero() {
        let json = serde_json::to_string(&FeatureFlags::empty()).unwrap();
        assert_eq!(json, "0");
    }
}
