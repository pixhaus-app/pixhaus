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

use super::id::FrameIndex;
use super::user_data::UserData;

/// A single frame in the timeline.
///
/// `duration_ms` is independent of any frame tag's playback speed —
/// tags multiply, they don't replace.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    /// Display duration in milliseconds. The minimum honoured by
    /// downstream players (Unity importer, web preview) is `1`.
    pub duration_ms: u32,
    /// Float multiplier on the frame's effective on-screen time, adopted
    /// from Pixelorama's per-frame `duration`. `1.0` plays at the base
    /// `duration_ms`; `2.0` holds twice as long; `0.5` half. Lets an artist
    /// re-time a beat ("this frame holds 4x") without editing milliseconds,
    /// while `duration_ms` stays the absolute base that round-trips with
    /// Aseprite. Files written before this field load with `1.0` and re-save
    /// with the field omitted. See `THIRD_PARTY_NOTICES.md`.
    #[serde(default = "default_duration_mul", skip_serializing_if = "is_unit_mul")]
    pub duration_mul: f32,
    /// Free-form user metadata.
    #[serde(skip_serializing_if = "UserData::is_empty", default)]
    pub user_data: UserData,
}

fn default_duration_mul() -> f32 {
    1.0
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_unit_mul(m: &f32) -> bool {
    (*m - 1.0).abs() < f32::EPSILON
}

impl Frame {
    /// Effective on-screen duration in milliseconds, `duration_ms` scaled by
    /// [`Self::duration_mul`] and floored at `1`.
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn effective_duration_ms(&self) -> u32 {
        let scaled = (self.duration_ms as f32 * self.duration_mul).round();
        (scaled.max(1.0) as u32).max(1)
    }
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            duration_ms: 100,
            duration_mul: 1.0,
            user_data: UserData::default(),
        }
    }
}

/// Inclusive range of frames `[start, end]`.
///
/// `end < start` is rejected at the editor layer; the type itself
/// permits any pair so deserialization of malformed files surfaces a
/// validation error rather than failing to parse.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
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
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

impl LoopDirection {
    /// Expands an inclusive `[start, end]` frame range into the order frames
    /// should be emitted for this direction.
    ///
    /// Direction is applied here, at expansion (export) time, rather than
    /// baked into frame data — so the same tagged range can export forward
    /// for a walk cycle and ping-pong for a hover idle just by choosing a
    /// different [`LoopDirection`] at the call site. Ping-pong does not
    /// duplicate the turnaround frames. Adopted from Pixelorama's
    /// export-time `AnimationDirection`; see `THIRD_PARTY_NOTICES.md`.
    #[must_use]
    pub fn play_order(self, start: u32, end: u32) -> Vec<u32> {
        if end < start {
            return Vec::new();
        }
        let fwd: Vec<u32> = (start..=end).collect();
        match self {
            Self::Forward => fwd,
            Self::Reverse => fwd.into_iter().rev().collect(),
            Self::PingPong => {
                let mut out = fwd.clone();
                out.extend((start.saturating_add(1)..end).rev());
                out
            }
            Self::PingPongReverse => {
                let mut out: Vec<u32> = fwd.iter().copied().rev().collect();
                out.extend(start.saturating_add(1)..end);
                out
            }
        }
    }
}

/// A named, contiguous range of frames in the timeline.
///
/// Used for editor organization and as the source of truth for
/// per-tag playback options. Engine-side animation entries (see
/// [`super::animation::Animation`]) may mirror these or extend them
/// with handoff-specific metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
    fn effective_duration_scales_by_multiplier() {
        let f = Frame {
            duration_ms: 100,
            duration_mul: 2.0,
            user_data: UserData::default(),
        };
        assert_eq!(f.effective_duration_ms(), 200);
        let half = Frame {
            duration_ms: 100,
            duration_mul: 0.5,
            user_data: UserData::default(),
        };
        assert_eq!(half.effective_duration_ms(), 50);
    }

    #[test]
    fn effective_duration_floors_at_one() {
        let f = Frame {
            duration_ms: 1,
            duration_mul: 0.0,
            user_data: UserData::default(),
        };
        assert_eq!(f.effective_duration_ms(), 1);
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
    fn play_order_forward_and_reverse() {
        assert_eq!(LoopDirection::Forward.play_order(0, 3), vec![0, 1, 2, 3]);
        assert_eq!(LoopDirection::Reverse.play_order(0, 3), vec![3, 2, 1, 0]);
    }

    #[test]
    fn play_order_ping_pong_no_duplicate_endpoints() {
        assert_eq!(
            LoopDirection::PingPong.play_order(0, 3),
            vec![0, 1, 2, 3, 2, 1]
        );
        assert_eq!(
            LoopDirection::PingPongReverse.play_order(0, 3),
            vec![3, 2, 1, 0, 1, 2]
        );
    }

    #[test]
    fn play_order_single_frame_and_empty() {
        assert_eq!(LoopDirection::PingPong.play_order(5, 5), vec![5]);
        assert!(LoopDirection::Forward.play_order(3, 1).is_empty());
    }

    #[test]
    fn play_order_ping_pong_single_frame_at_u32_max_does_not_overflow() {
        assert_eq!(
            LoopDirection::PingPong.play_order(u32::MAX, u32::MAX),
            vec![u32::MAX]
        );
        assert_eq!(
            LoopDirection::PingPongReverse.play_order(u32::MAX, u32::MAX),
            vec![u32::MAX]
        );
    }

    #[test]
    fn loop_direction_serializes_snake_case() {
        let json = serde_json::to_string(&LoopDirection::PingPongReverse).unwrap();
        assert_eq!(json, "\"ping_pong_reverse\"");
    }
}
