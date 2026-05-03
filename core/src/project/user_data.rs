//! Free-form user-attached metadata.
//!
//! Aseprite supports a per-entity user-data chunk: a text label and a
//! tint color. Several Pixhaus types expose the same affordance so
//! plugins, exporters, and the user can attach hints without forcing
//! a schema migration. Keep `text` short — the data model is not a
//! storage layer for blobs.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::color::Rgba;

/// Optional user-supplied text and tint color attached to an entity.
///
/// Both fields are optional so a missing `UserData` and a present-but-
/// empty `UserData` round-trip identically.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UserData {
    /// Short user-supplied label or note.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub text: Option<String>,
    /// Optional accent color, used by the editor to tint a row.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub color: Option<Rgba>,
}

impl UserData {
    /// Returns `true` if neither `text` nor `color` is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_none() && self.color.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_user_data_round_trips() {
        let u = UserData::default();
        assert!(u.is_empty());
        let bytes = rmp_serde::to_vec_named(&u).unwrap();
        let back: UserData = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(u, back);
    }

    #[test]
    fn populated_user_data_round_trips() {
        let u = UserData {
            text: Some("hero idle".into()),
            color: Some(Rgba::opaque(200, 100, 50)),
        };
        let bytes = rmp_serde::to_vec_named(&u).unwrap();
        let back: UserData = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(u, back);
    }
}
