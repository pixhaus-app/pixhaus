//! Animated tile stepping: resolve which source tile to display at a given
//! playback position.
//!
//! Animated tiles are defined in [`TileAnimation`] on the tileset's per-tile
//! [`TileProperties`]. The renderer calls [`step_animation`] each frame with
//! the elapsed time; the function returns the source [`TileIndex`] to render.

use crate::project::id::TileIndex;
use crate::project::tileset::{AnimLoopMode, TileAnimation};

/// Returns the source [`TileIndex`] for an animated tile at `elapsed_ms`.
///
/// `elapsed_ms` is the total accumulated playback time in milliseconds since
/// the animation started (or since the last loop reset for the caller).
///
/// Returns `None` when `anim.frames` is empty (degenerate animation with no
/// content). Returns the first frame's tile index when only one frame is
/// defined.
///
/// Loop semantics:
/// - [`AnimLoopMode::Loop`] — wraps `elapsed_ms` modulo the total duration.
/// - [`AnimLoopMode::Once`] — clamps to the last frame after the total
///   duration is reached.
/// - [`AnimLoopMode::PingPong`] — bounces back and forth; the first and last
///   frames are each played once per period (not doubled).
#[must_use]
pub fn step_animation(anim: &TileAnimation, elapsed_ms: u64) -> Option<TileIndex> {
    if anim.frames.is_empty() {
        return None;
    }
    if anim.frames.len() == 1 {
        return Some(anim.frames[0].tile_index);
    }

    let total = anim.total_duration_ms();
    if total == 0 {
        return Some(anim.frames[0].tile_index);
    }

    let frame_idx = match anim.loop_mode {
        AnimLoopMode::Loop => frame_at_time(&anim.frames, elapsed_ms % total),
        AnimLoopMode::Once => frame_at_time(&anim.frames, elapsed_ms.min(total - 1)),
        AnimLoopMode::PingPong => {
            // One full period = forward pass + backward pass.
            // Forward: frame 0 → frame N-1 (total ms)
            // Backward: frame N-2 → frame 0 (total - first_frame - last_frame ms)
            // But the canonical ping-pong just reverses the frame sequence:
            // build a virtual doubled list: [0..N-1, N-2..1] and pick from it.
            let n = anim.frames.len();
            // Duration of the backward pass (excludes first and last frames
            // to avoid doubling the endpoints).
            let back_duration: u64 = anim.frames[1..n - 1]
                .iter()
                .map(|f| u64::from(f.duration_ms))
                .sum();
            let period = total + back_duration;
            let t = if period == 0 { 0 } else { elapsed_ms % period };

            if t < total {
                // Forward pass.
                frame_at_time(&anim.frames, t)
            } else {
                // Backward pass over frames [N-2..1] (reversed inner frames).
                let back_t = t - total;
                let inner: Vec<_> = anim.frames[1..n - 1].iter().rev().collect();
                let mut acc = 0u64;
                let mut result = n - 2;
                for (i, frame) in inner.iter().enumerate() {
                    acc += u64::from(frame.duration_ms);
                    if back_t < acc {
                        // Map back to original index: inner[i] is frames[n-2-i]
                        result = n - 2 - i;
                        break;
                    }
                }
                result
            }
        }
    };

    Some(anim.frames[frame_idx].tile_index)
}

/// Returns the 0-based frame index for time `t` within `[0, total_duration)`.
///
/// Scans frames left to right and returns the index of the first frame whose
/// cumulative end time exceeds `t`.
fn frame_at_time(frames: &[crate::project::tileset::TileAnimationFrame], t: u64) -> usize {
    let mut acc = 0u64;
    for (i, frame) in frames.iter().enumerate() {
        acc += u64::from(frame.duration_ms);
        if t < acc {
            return i;
        }
    }
    // Should not be reached if `t < total_duration`.
    frames.len() - 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::tileset::{AnimLoopMode, TileAnimation, TileAnimationFrame};
    use rstest::rstest;

    fn make_anim(durations_ms: &[u32], loop_mode: AnimLoopMode) -> TileAnimation {
        TileAnimation {
            frames: durations_ms
                .iter()
                .enumerate()
                .map(|(i, &d)| TileAnimationFrame {
                    tile_index: TileIndex::new(u32::try_from(i).unwrap() + 1),
                    duration_ms: d,
                })
                .collect(),
            loop_mode,
        }
    }

    // --- Empty / degenerate -------------------------------------------------

    #[test]
    fn empty_animation_returns_none() {
        let anim = TileAnimation {
            frames: vec![],
            loop_mode: AnimLoopMode::Loop,
        };
        assert_eq!(step_animation(&anim, 0), None);
        assert_eq!(step_animation(&anim, 9999), None);
    }

    #[test]
    fn single_frame_always_returns_that_frame() {
        let anim = make_anim(&[100], AnimLoopMode::Loop);
        for t in [0, 50, 100, 500, 9999] {
            assert_eq!(
                step_animation(&anim, t),
                Some(TileIndex::new(1)),
                "at t={t}"
            );
        }
    }

    #[test]
    fn zero_duration_frames_return_first_frame() {
        let anim = TileAnimation {
            frames: vec![
                TileAnimationFrame {
                    tile_index: TileIndex::new(1),
                    duration_ms: 0,
                },
                TileAnimationFrame {
                    tile_index: TileIndex::new(2),
                    duration_ms: 0,
                },
            ],
            loop_mode: AnimLoopMode::Loop,
        };
        // total_duration_ms == 0 → always first frame
        assert_eq!(step_animation(&anim, 9999), Some(TileIndex::new(1)));
    }

    // --- Loop mode ----------------------------------------------------------

    #[rstest]
    #[case(0, TileIndex::new(1))] // t=0: frame1 [0..100)
    #[case(99, TileIndex::new(1))] // t=99: still frame1
    #[case(100, TileIndex::new(2))] // t=100: frame2 starts [100..200)
    #[case(199, TileIndex::new(2))] // t=199: still frame2
    #[case(200, TileIndex::new(1))] // t=200: wraps → 200%200=0 → frame1
    #[case(201, TileIndex::new(1))] // t=201: 201%200=1 → frame1
    #[case(300, TileIndex::new(2))] // t=300: 300%200=100 → frame2 [100..200)
    fn loop_two_equal_frames(#[case] elapsed: u64, #[case] expected: TileIndex) {
        // Two frames of 100ms each, total=200ms.
        let anim = make_anim(&[100, 100], AnimLoopMode::Loop);
        assert_eq!(step_animation(&anim, elapsed), Some(expected));
    }

    #[test]
    fn loop_wraps_at_total_duration() {
        let anim = make_anim(&[100, 100, 100], AnimLoopMode::Loop); // total=300
        assert_eq!(step_animation(&anim, 0), Some(TileIndex::new(1)));
        assert_eq!(step_animation(&anim, 299), Some(TileIndex::new(3)));
        assert_eq!(step_animation(&anim, 300), Some(TileIndex::new(1))); // 300%300=0 → frame1
        assert_eq!(step_animation(&anim, 500), Some(TileIndex::new(3))); // 500%300=200 → frame3
        assert_eq!(step_animation(&anim, 601), Some(TileIndex::new(1))); // 601%300=1 → frame1
    }

    // --- Once mode ----------------------------------------------------------

    #[test]
    fn once_clamps_after_total_duration() {
        let anim = make_anim(&[100, 100], AnimLoopMode::Once); // total=200
        assert_eq!(step_animation(&anim, 0), Some(TileIndex::new(1)));
        assert_eq!(step_animation(&anim, 100), Some(TileIndex::new(2)));
        assert_eq!(step_animation(&anim, 199), Some(TileIndex::new(2)));
        assert_eq!(step_animation(&anim, 200), Some(TileIndex::new(2))); // clamped
        assert_eq!(step_animation(&anim, 9999), Some(TileIndex::new(2))); // still clamped
    }

    // --- PingPong mode ------------------------------------------------------

    #[test]
    fn pingpong_three_equal_frames() {
        // Frames: 1(100ms), 2(100ms), 3(100ms) → total forward=300ms.
        // PingPong period = forward(300) + backward middle(100 for frame 2) = 400ms.
        // Forward pass:  0-99→frame1, 100-199→frame2, 200-299→frame3.
        // Backward pass (t-300): 0-99→frame2.
        // t=300: back_t=0 → frame2.
        // t=399: back_t=99 → frame2.
        // t=400: wraps, same as t=0 → frame1.
        let anim = make_anim(&[100, 100, 100], AnimLoopMode::PingPong);
        assert_eq!(step_animation(&anim, 0), Some(TileIndex::new(1)));
        assert_eq!(step_animation(&anim, 99), Some(TileIndex::new(1)));
        assert_eq!(step_animation(&anim, 100), Some(TileIndex::new(2)));
        assert_eq!(step_animation(&anim, 200), Some(TileIndex::new(3)));
        assert_eq!(step_animation(&anim, 299), Some(TileIndex::new(3)));
        assert_eq!(step_animation(&anim, 300), Some(TileIndex::new(2)));
        assert_eq!(step_animation(&anim, 399), Some(TileIndex::new(2)));
        assert_eq!(step_animation(&anim, 400), Some(TileIndex::new(1))); // new period
    }

    #[test]
    fn pingpong_two_frames_period_matches_forward() {
        // 2 frames: [1(100ms), 2(100ms)]. Backward inner = empty → period = 200ms.
        // t=0→frame1, t=100→frame2, t=200→frame1 (new period).
        let anim = make_anim(&[100, 100], AnimLoopMode::PingPong);
        assert_eq!(step_animation(&anim, 0), Some(TileIndex::new(1)));
        assert_eq!(step_animation(&anim, 100), Some(TileIndex::new(2)));
        assert_eq!(step_animation(&anim, 200), Some(TileIndex::new(1)));
    }

    // --- Unequal frame durations -------------------------------------------

    #[test]
    fn loop_unequal_durations() {
        // Frame 1: 50ms, Frame 2: 150ms → total = 200ms.
        let anim = make_anim(&[50, 150], AnimLoopMode::Loop);
        assert_eq!(step_animation(&anim, 0), Some(TileIndex::new(1)));
        assert_eq!(step_animation(&anim, 49), Some(TileIndex::new(1)));
        assert_eq!(step_animation(&anim, 50), Some(TileIndex::new(2)));
        assert_eq!(step_animation(&anim, 199), Some(TileIndex::new(2)));
        assert_eq!(step_animation(&anim, 200), Some(TileIndex::new(1))); // wrap
    }
}
