//! Engine-handoff animation entries.
//!
//! [`super::frame::FrameTag`] organises the timeline for the editor.
//! `Animation` is the analogous entry on the engine side: a named
//! frame range with a playback hint, plus a `speed_multiplier` an
//! exporter can write into the target engine's animation clip.

use serde::{Deserialize, Serialize};

use super::frame::{FrameRange, LoopDirection};
use super::id::AnimationId;
use super::user_data::UserData;

/// A named animation entry referencing a frame range.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Animation {
    /// Stable identifier.
    pub id: AnimationId,
    /// Display name. Exporters use this verbatim as the engine-side
    /// clip name; pick something legal for the target runtime.
    pub name: String,
    /// Inclusive range of frames this animation plays.
    pub range: FrameRange,
    /// Direction in which the range plays.
    pub loop_direction: LoopDirection,
    /// Multiplier applied to per-frame durations during playback. `1.0`
    /// is the editor speed; `2.0` plays twice as fast.
    pub speed_multiplier: f32,
    /// Free-form user metadata.
    #[serde(skip_serializing_if = "UserData::is_empty", default)]
    pub user_data: UserData,
}

impl Animation {
    /// Constructs an animation that plays `range` forward at editor speed.
    #[must_use]
    pub fn forward(id: AnimationId, name: impl Into<String>, range: FrameRange) -> Self {
        Self {
            id,
            name: name.into(),
            range,
            loop_direction: LoopDirection::Forward,
            speed_multiplier: 1.0,
            user_data: UserData::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::id::FrameIndex;

    #[test]
    fn forward_constructor_defaults() {
        let r = FrameRange::new(FrameIndex::new(0), FrameIndex::new(3));
        let a = Animation::forward(AnimationId::new(1), "idle", r);
        assert_eq!(a.loop_direction, LoopDirection::Forward);
        assert!((a.speed_multiplier - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn animation_round_trip() {
        let a = Animation::forward(AnimationId::new(1), "walk", FrameRange::new(FrameIndex::new(0), FrameIndex::new(3)));
        let json = serde_json::to_string(&a).unwrap();
        let back: Animation = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back);
    }
}
