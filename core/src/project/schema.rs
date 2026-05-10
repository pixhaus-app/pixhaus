//! Schema version and feature flags for the Pixhaus data model.
//!
//! These appear at the top of every serialized project. The version is
//! a contract between writers and readers: additive changes bump
//! [`SchemaVersion::MINOR`], breaking changes bump [`SchemaVersion::MAJOR`]
//! and require a migration. The feature-flag bitfield lets a writer
//! advertise which optional sub-features are present without bumping
//! the version every time.
//!
//! See `docs/planning/work/bedrock.md#b3-project-file-format-pixhaus`
//! for how this lands on disk.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Versioned identity of the data model.
///
/// Encoded explicitly so old binaries can refuse to load files written
/// by a newer major version, and so newer binaries can transparently
/// upgrade older minor versions.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
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
    pub const MAJOR: u16 = 1;

    /// Minor version this build of Pixhaus writes.
    ///
    /// Bumped from `0` to `1` for B9.1: the `Project` struct gained the
    /// optional `library` and `active` fields. Both deserialise via
    /// `serde(default)` so files written at minor `0` continue to load.
    pub const MINOR: u16 = 1;

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
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(type = "number")]
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
    ///
    /// Single source of truth for "which feature bits does this Pixhaus
    /// version know about?". Downstream crates (notably `pixhaus-io`)
    /// consume this rather than redeclaring the bitmask, so adding a
    /// new flag in this enum without touching every consumer cannot
    /// silently desync.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_version_round_trips() {
        let v = SchemaVersion::current();
        let bytes = rmp_serde::to_vec_named(&v).unwrap();
        let back: SchemaVersion = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn same_major_is_compatible() {
        let v = SchemaVersion { major: 1, minor: 5 };
        let other = SchemaVersion { major: 1, minor: 0 };
        assert!(v.is_compatible_with(other));
    }

    #[test]
    fn newer_major_is_incompatible() {
        let v = SchemaVersion { major: 1, minor: 0 };
        let other = SchemaVersion { major: 2, minor: 0 };
        assert!(!v.is_compatible_with(other));
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
