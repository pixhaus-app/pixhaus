//! Animation clips: named frame ranges over a sprite's frames.
//!
//! A [`Sprite`](crate::Sprite) holds a flat list of [`Frame`](crate::Frame)s in
//! playback order. An [`AnimationClip`] names a contiguous range of them (idle,
//! walk, ...) and carries its own timing (bible 15.2). A still sprite has one frame
//! and no clips; an animated sprite has many frames and one or more clips.

use serde::{Deserialize, Serialize};

use crate::ids::ClipId;

/// How a clip plays when it reaches its last frame.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum LoopMode {
    /// Restart from the first frame (idle, walk, run).
    #[default]
    Loop,
    /// Play once and hold the last frame (attack, hurt, death).
    Once,
    /// Play forward then backward, repeating.
    PingPong,
}

/// A named, contiguous frame range over a sprite's frames, with its own timing.
///
/// `start` and `end` are inclusive indices into [`Sprite::frames`](crate::Sprite).
/// `name` is project content (e.g. `"idle"`), never a localization key.
#[derive(Clone, Debug)]
pub struct AnimationClip {
    /// Stable id within the owning sprite.
    pub id: ClipId,
    /// User-facing clip name (project content, not a localization key).
    pub name: String,
    /// First frame index of the range (inclusive).
    pub start: u32,
    /// Last frame index of the range (inclusive).
    pub end: u32,
    /// Playback rate in frames per second.
    pub fps: u16,
    /// How the clip behaves at its end.
    pub loop_mode: LoopMode,
}

impl AnimationClip {
    /// The number of frames the clip spans (inclusive of both endpoints).
    pub fn frame_count(&self) -> u32 {
        self.end.saturating_sub(self.start) + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_count_is_inclusive_of_both_ends() {
        let clip = AnimationClip {
            id: ClipId(0),
            name: "idle".to_owned(),
            start: 0,
            end: 7,
            fps: 12,
            loop_mode: LoopMode::Loop,
        };
        assert_eq!(clip.frame_count(), 8);
    }

    #[test]
    fn single_frame_clip_counts_one() {
        let clip = AnimationClip {
            id: ClipId(1),
            name: "pose".to_owned(),
            start: 3,
            end: 3,
            fps: 1,
            loop_mode: LoopMode::Once,
        };
        assert_eq!(clip.frame_count(), 1);
    }

    #[test]
    fn loop_mode_defaults_to_loop() {
        assert_eq!(LoopMode::default(), LoopMode::Loop);
    }
}
