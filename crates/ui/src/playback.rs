//! Pure animation-playback math: which frame the playhead lands on.
//!
//! Egui-free so it is unit-testable without a UI. The canvas and the Animate
//! timeline both derive the displayed frame from one scalar (elapsed seconds) via
//! these functions, so they cannot drift. Playback is a transient view concern: the
//! document is never mutated here (bible rules 3/4 - durable mutation is a `Command`,
//! and a per-frame command would pollute undo).

use pixhaus_core::{ClipId, LoopMode, Sprite};

/// Default playback rate for a sprite that has frames but no clip to name an fps.
pub const DEFAULT_PLAYBACK_FPS: u16 = 12;

/// The resolved frame range the playhead runs over, plus its timing.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PlayRange {
    /// First frame index (into [`Sprite::frames`]) of the range.
    pub start: u32,
    /// Number of frames in the range (always `>= 1`).
    pub frame_count: u32,
    /// Playback rate in frames per second.
    pub fps: u16,
    /// How the range behaves at its end.
    pub loop_mode: LoopMode,
}

/// Resolve which frame range plays for `sprite`, given an optionally-selected clip.
///
/// The selected clip if it is still valid; else the sprite's first clip; else the
/// implicit "all frames" range at [`DEFAULT_PLAYBACK_FPS`]. Always clamped to the
/// sprite's real frames, so an out-of-date clip range can never index out of bounds.
#[must_use]
pub fn resolve_range(sprite: &Sprite, clip: Option<ClipId>) -> PlayRange {
    let frames_len = u32::try_from(sprite.frames().len()).unwrap_or(u32::MAX);
    let chosen = clip
        .and_then(|id| sprite.clips().iter().find(|c| c.id == id))
        .or_else(|| sprite.clips().first());
    if let Some(clip) = chosen {
        let start = clip.start.min(frames_len.saturating_sub(1));
        let frame_count = clip.frame_count().min(frames_len.saturating_sub(start)).max(1);
        PlayRange {
            start,
            frame_count,
            fps: clip.fps,
            loop_mode: clip.loop_mode,
        }
    } else {
        PlayRange {
            start: 0,
            frame_count: frames_len.max(1),
            fps: DEFAULT_PLAYBACK_FPS,
            loop_mode: LoopMode::Loop,
        }
    }
}

/// The 0-based frame offset within a range at `seconds` elapsed, honoring `loop_mode`.
///
/// `Loop` wraps, `Once` holds the last frame, `PingPong` reflects at the endpoints.
/// A range of one frame (or zero fps clamped to one) is always offset 0.
#[must_use]
pub fn playhead_index(seconds: f32, fps: u16, frame_count: u32, loop_mode: LoopMode) -> u32 {
    if frame_count <= 1 {
        return 0;
    }
    let fps = f32::from(fps.max(1));
    // `seconds` and `fps` are finite and non-negative; the result is reduced modulo
    // the frame count immediately, so going through u64 ticks first cannot overflow.
    let raw = (seconds * fps).floor().max(0.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let ticks = raw as u64;
    let count = u64::from(frame_count);
    let offset = match loop_mode {
        LoopMode::Loop => ticks % count,
        LoopMode::Once => ticks.min(count - 1),
        LoopMode::PingPong => {
            // forward then back, the two endpoints shared: period is 2*(count-1).
            let period = (count - 1) * 2;
            let position = ticks % period;
            if position < count - 1 { position } else { period - position }
        }
    };
    // `offset < frame_count <= u32::MAX`, so this cast neither truncates nor wraps.
    #[allow(clippy::cast_possible_truncation)]
    let offset = offset as u32;
    offset
}

/// The absolute frame the playhead sits on: `range_start + offset`, clamped into the
/// sprite's real frames. The Animate timeline ruler highlights this frame.
#[must_use]
pub fn playhead_frame(range_start: u32, playhead_offset: u32, frames: u32) -> u32 {
    range_start.saturating_add(playhead_offset).min(frames.max(1) - 1)
}

/// The range-relative frame offset for a click at `pos_x` on a frame ruler that starts
/// at `left` with `frame_w` pixels per frame: the absolute frame under the cursor,
/// clamped into the real frames, then made relative to `range_start`. Scrubs by click.
#[must_use]
pub fn click_to_offset(pos_x: f32, left: f32, frame_w: f32, frames: u32, range_start: u32) -> u32 {
    let frames = frames.max(1);
    // A click left of the ruler floors to 0; a NaN from a zero `frame_w` also maxes to
    // 0. The result is clamped into the real frames before the cast, so it cannot wrap.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let frame = (((pos_x - left) / frame_w).floor().max(0.0) as u32).min(frames - 1);
    frame.saturating_sub(range_start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::commands::{ApplyGeneratedAnimation, GeneratedFrameData};
    use pixhaus_core::{Command, Document};

    #[test]
    fn loop_wraps_at_the_frame_count() {
        // 8 frames at 8 fps: one second is exactly one full loop -> back to 0.
        assert_eq!(playhead_index(0.0, 8, 8, LoopMode::Loop), 0);
        assert_eq!(playhead_index(0.5, 8, 8, LoopMode::Loop), 4);
        assert_eq!(playhead_index(1.0, 8, 8, LoopMode::Loop), 0);
        assert_eq!(playhead_index(1.125, 8, 8, LoopMode::Loop), 1);
    }

    #[test]
    fn once_holds_the_last_frame() {
        assert_eq!(playhead_index(0.25, 8, 8, LoopMode::Once), 2);
        assert_eq!(playhead_index(100.0, 8, 8, LoopMode::Once), 7, "Once clamps at the last frame");
    }

    #[test]
    fn pingpong_reflects_at_the_endpoints() {
        // 4 frames at 4 fps -> one tick per 0.25s. Sequence: 0,1,2,3,2,1,0,1,2,3,...
        let seq: Vec<u32> = (0..8u16).map(|t| playhead_index(f32::from(t) * 0.25, 4, 4, LoopMode::PingPong)).collect();
        assert_eq!(seq, vec![0, 1, 2, 3, 2, 1, 0, 1]);
    }

    #[test]
    fn single_frame_range_is_always_zero() {
        assert_eq!(playhead_index(99.0, 12, 1, LoopMode::Loop), 0);
        assert_eq!(playhead_index(99.0, 0, 0, LoopMode::Loop), 0, "zero frames clamps to 0, no panic");
    }

    // Build a document with one animated sprite (one clip spanning all frames).
    fn anim_doc(frames: usize, fps: u16) -> Document {
        let data: Vec<GeneratedFrameData> = (0..frames)
            .map(|_| GeneratedFrameData {
                stride: 4,
                rgba: vec![0, 0, 0, 255],
            })
            .collect();
        let mut doc = Document::new();
        let mut cmd = ApplyGeneratedAnimation::new("s".to_owned(), "idle".to_owned(), 1, 1, fps, LoopMode::Loop, data);
        assert!(cmd.apply(&mut doc).is_ok(), "the test animation applies");
        doc
    }

    #[test]
    fn playhead_frame_clamps_into_real_frames() {
        assert_eq!(playhead_frame(0, 0, 8), 0);
        assert_eq!(playhead_frame(2, 3, 8), 5);
        assert_eq!(playhead_frame(0, 99, 8), 7, "an offset past the end clamps to the last frame");
        assert_eq!(playhead_frame(0, 0, 0), 0, "zero frames clamps to 0 with no underflow");
    }

    #[test]
    fn click_to_offset_maps_x_to_a_range_relative_frame() {
        // 8 frames, ruler from x=0, 10px per frame.
        assert_eq!(click_to_offset(0.0, 0.0, 10.0, 8, 0), 0, "the left edge is frame 0");
        assert_eq!(click_to_offset(25.0, 0.0, 10.0, 8, 0), 2, "x=25 lands in frame 2");
        assert_eq!(click_to_offset(-5.0, 0.0, 10.0, 8, 0), 0, "a click left of the ruler clamps to 0");
        assert_eq!(click_to_offset(1000.0, 0.0, 10.0, 8, 0), 7, "far right clamps to the last frame");
        assert_eq!(click_to_offset(25.0, 0.0, 10.0, 8, 2), 0, "range_start 2 makes absolute frame 2 a zero offset");
    }

    #[test]
    fn resolve_range_uses_the_sprites_clip() {
        let doc = anim_doc(8, 10);
        let range = doc.active_sprite().and_then(|id| doc.sprite(id)).map(|s| resolve_range(s, None));
        assert_eq!(
            range,
            Some(PlayRange {
                start: 0,
                frame_count: 8,
                fps: 10,
                loop_mode: LoopMode::Loop,
            }),
        );
    }

    #[test]
    fn resolve_range_clamps_frame_count_to_real_frames() {
        let doc = anim_doc(3, 12);
        let range = doc.active_sprite().and_then(|id| doc.sprite(id)).map(|s| resolve_range(s, None));
        assert_eq!(range.map(|r| r.frame_count), Some(3), "the range never exceeds the real frame count");
    }
}
