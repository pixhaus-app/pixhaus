//! Slices: named rectangular regions on a sprite.
//!
//! A slice carries a base rectangle plus optional nine-slice insets
//! and a pivot. Engines (Unity importer, web preview) consume slices
//! to spawn UI sprites and to anchor instantiated objects.
//!
//! Pixhaus mirrors Aseprite's slice model: a slice can change shape
//! per-frame via a sequence of [`SliceKey`]s. The first key applies
//! from frame 0 onwards; subsequent keys override starting at their
//! `frame`.

use serde::{Deserialize, Serialize};

use super::geometry::{IVec2, Rect};
use super::id::{FrameIndex, SliceId};
use super::user_data::UserData;

/// A per-frame override of a slice's geometry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SliceKey {
    /// Frame from which this key takes effect (until the next key).
    pub frame: FrameIndex,
    /// The slice rectangle in canvas coordinates.
    pub bounds: Rect,
    /// Optional nine-slice insets.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub nine_slice: Option<NineSlice>,
    /// Optional pivot point (relative to `bounds.origin`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub pivot: Option<Pivot>,
}

/// Nine-slice insets for a slice's center patch.
///
/// `center` is the inner rectangle (in pixels relative to the slice's
/// `bounds.origin`); the four edges and four corners are derived from
/// it at render time.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NineSlice {
    /// Center patch rectangle, relative to the slice bounds.
    pub center: Rect,
}

/// Pivot point for a slice, relative to its `bounds.origin`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Pivot {
    /// Offset from `bounds.origin` to the pivot, in pixels.
    pub offset: IVec2,
}

/// A named rectangular region on the sprite.
///
/// `keys` is non-empty when constructed by the editor; the first key
/// holds the slice's base geometry and subsequent keys are per-frame
/// overrides.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Slice {
    /// Stable identifier.
    pub id: SliceId,
    /// Display name. Exporters use this verbatim as the engine-side
    /// sub-asset name.
    pub name: String,
    /// Per-frame slice geometry. Index `0` is the base; later entries
    /// override starting at their declared `frame`.
    pub keys: Vec<SliceKey>,
    /// Free-form user metadata.
    #[serde(skip_serializing_if = "UserData::is_empty", default)]
    pub user_data: UserData,
}

impl Slice {
    /// Constructs a single-key slice with no nine-slice or pivot.
    #[must_use]
    pub fn simple(id: SliceId, name: impl Into<String>, bounds: Rect) -> Self {
        Self {
            id,
            name: name.into(),
            keys: vec![SliceKey {
                frame: FrameIndex::new(0),
                bounds,
                nine_slice: None,
                pivot: None,
            }],
            user_data: UserData::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::geometry::Size;

    #[test]
    fn simple_slice_has_one_key() {
        let s = Slice::simple(SliceId::new(1), "ui_button", Rect::from_xywh(0, 0, 16, 16));
        assert_eq!(s.keys.len(), 1);
        assert!(s.keys[0].nine_slice.is_none());
        assert!(s.keys[0].pivot.is_none());
    }

    #[test]
    fn slice_round_trip() {
        let s = Slice {
            id: SliceId::new(1),
            name: "ui_button".into(),
            keys: vec![SliceKey {
                frame: FrameIndex::new(0),
                bounds: Rect::from_xywh(0, 0, 32, 32),
                nine_slice: Some(NineSlice {
                    center: Rect {
                        origin: IVec2::new(4, 4),
                        size: Size::new(24, 24),
                    },
                }),
                pivot: Some(Pivot { offset: IVec2::new(16, 16) }),
            }],
            user_data: UserData::default(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Slice = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}
