//! Timeline frames and frame tags.
//!
//! A sprite owns a dense `Vec<Frame>` indexed by [`FrameIndex`]. Each
//! frame carries timing and optional metadata; the per-cel pixel data
//! lives separately in the cel collection so frames stay cheap to
//! re-order.
//!
//! [`FrameTag`] names a sub-range of frames for editor convenience;
//! [`super::animation::Animation`] is the engine-handoff equivalent
//! that exporters consume.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::id::FrameIndex;
use super::user_data::UserData;

/// A single frame in the timeline.
///
/// `duration_ms` is independent of any frame tag's playback speed —
/// tags multiply, they don't replace.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Frame {
    /// Display duration in milliseconds. The minimum honoured by
    /// downstream players (Unity importer, web preview) is `1`.
    pub duration_ms: u32,
    /// Free-form user metadata.
    #[serde(skip_serializing_if = "UserData::is_empty", default)]
    pub user_data: UserData,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            duration_ms: 100,
            user_data: UserData::default(),
        }
    }
}

/// Inclusive range of frames `[start, end]`.
///
/// `end < start` is rejected at the editor layer; the type itself
/// permits any pair so deserialization of malformed files surfaces a
/// validation error rather than failing to parse.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FrameRange {
    /// First frame in the range, inclusive.
    pub start: FrameIndex,
    /// Last frame in the range, inclusive.
    pub end: FrameIndex,
}

impl FrameRange {
    /// Constructs a range. Caller is responsible for ordering.
    #[must_use]
    pub const fn new(start: FrameIndex, end: FrameIndex) -> Self {
        Self { start, end }
    }

    /// Number of frames covered, or `0` if `end < start`.
    #[must_use]
    pub fn len(self) -> u32 {
        let s = self.start.get();
        let e = self.end.get();
        if e < s { 0 } else { e - s + 1 }
    }

    /// Whether the range covers no frames (i.e. `end < start`).
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.end.get() < self.start.get()
    }
}

/// Direction in which a tagged or animated frame range plays.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum LoopDirection {
    /// Play `start` → `end`, then return to `start`.
    #[default]
    Forward,
    /// Play `end` → `start`, then return to `end`.
    Reverse,
    /// Play `start` → `end` → `start` → `end` ….
    PingPong,
    /// Play `end` → `start` → `end` → `start` ….
    PingPongReverse,
}

/// A named, contiguous range of frames in the timeline.
///
/// Used for editor organization and as the source of truth for
/// per-tag playback options. Engine-side animation entries (see
/// [`super::animation::Animation`]) may mirror these or extend them
/// with handoff-specific metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FrameTag {
    /// Display name in the timeline.
    pub name: String,
    /// Inclusive range of frames covered by the tag.
    pub range: FrameRange,
    /// Direction in which the range plays.
    pub loop_direction: LoopDirection,
    /// Number of times the tag should repeat. `0` is conventionally
    /// "loop forever"; a positive value bounds playback.
    pub repeat: u16,
    /// Free-form user metadata.
    #[serde(skip_serializing_if = "UserData::is_empty", default)]
    pub user_data: UserData,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_default_is_100ms() {
        assert_eq!(Frame::default().duration_ms, 100);
    }

    #[test]
    fn frame_range_len_inclusive() {
        let r = FrameRange::new(FrameIndex::new(2), FrameIndex::new(5));
        assert_eq!(r.len(), 4);
        assert!(!r.is_empty());
    }

    #[test]
    fn empty_range_when_end_before_start() {
        let r = FrameRange::new(FrameIndex::new(5), FrameIndex::new(2));
        assert_eq!(r.len(), 0);
        assert!(r.is_empty());
    }

    #[test]
    fn frame_tag_round_trip() {
        let t = FrameTag {
            name: "walk".into(),
            range: FrameRange::new(FrameIndex::new(0), FrameIndex::new(3)),
            loop_direction: LoopDirection::PingPong,
            repeat: 0,
            user_data: UserData::default(),
        };
        let bytes = rmp_serde::to_vec_named(&t).unwrap();
        let back: FrameTag = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn loop_direction_serializes_snake_case() {
        let json = serde_json::to_string(&LoopDirection::PingPongReverse).unwrap();
        assert_eq!(json, "\"ping_pong_reverse\"");
    }
}
