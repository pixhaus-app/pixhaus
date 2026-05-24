//! Walk-cycle loop frame picking (S32, i2v path).
//!
//! The image-to-video backend returns a clip of a character walking. To
//! land a clean looping walk we need to: find where the neutral stance
//! (feet together) recurs to bound exactly one cycle, then pick a fixed
//! number of evenly spaced frames between those markers.
//!
//! The neutral stance is the lowest-motion pose in the first part of the
//! clip — a walk passes through "both feet planted" once per stride, and a
//! stride bookends the cycle. The recurrence is the later frame most
//! similar to that neutral pose. The studio surfaces both markers so the
//! user can nudge them; this module supplies the auto-detect and the
//! even-spacing pick.

use super::{VideoFrame, motion_magnitude};

/// A pair of frame indices bounding one motion cycle.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct LoopMarkers {
    /// Inclusive start of the cycle — the first neutral-stance frame.
    pub start: usize,
    /// Inclusive end of the cycle — where the neutral stance recurs.
    pub end: usize,
}

/// Pixel similarity between two frames in `0.0..=1.0`. `1.0` is identical.
///
/// Defined as the complement of the internal motion-magnitude measure so
/// callers reason in "how alike" terms when judging a loop seam.
#[must_use]
pub fn seam_similarity(frames: &[VideoFrame], a: usize, b: usize) -> f32 {
    match (frames.get(a), frames.get(b)) {
        (Some(fa), Some(fb)) => 1.0 - motion_magnitude(&fa.pixels, &fb.pixels),
        _ => 0.0,
    }
}

/// Auto-detects the loop markers in a clip.
///
/// Marker A is the lowest-motion frame in the first half (the neutral
/// stance). Marker B is the later frame — at least a minimum cycle length
/// past A — most similar to A (the recurrence of that stance). Falls back
/// to the whole clip when it is too short to analyse.
#[must_use]
pub fn auto_loop_markers(frames: &[VideoFrame]) -> LoopMarkers {
    let n = frames.len();
    if n < 4 {
        return LoopMarkers {
            start: 0,
            end: n.saturating_sub(1),
        };
    }

    // Per-frame motion against the previous frame. A neutral stance sits at
    // a local minimum: the body momentarily plants before pushing off.
    let motion: Vec<f32> = (0..n)
        .map(|i| {
            if i == 0 {
                f32::MAX
            } else {
                motion_magnitude(&frames[i - 1].pixels, &frames[i].pixels)
            }
        })
        .collect();

    // Marker A: lowest-motion frame in the first half, skipping the very
    // first frame (often a static lead-in that is not a true stride beat).
    let half = (n / 2).max(2);
    let mut start = 1;
    let mut min_motion = f32::MAX;
    for (i, &m) in motion.iter().enumerate().take(half).skip(1) {
        if m < min_motion {
            min_motion = m;
            start = i;
        }
    }

    // Marker B: most similar frame to A, at least `min_cycle` frames later.
    let min_cycle = (n / 4).max(2);
    let search_start = (start + min_cycle).min(n - 1);
    let mut end = n - 1;
    let mut best_sim = -1.0_f32;
    for k in search_start..n {
        let sim = 1.0 - motion_magnitude(&frames[start].pixels, &frames[k].pixels);
        if sim > best_sim {
            best_sim = sim;
            end = k;
        }
    }

    LoopMarkers { start, end }
}

/// Picks `target_count` evenly spaced frame indices in `[markers.start,
/// markers.end)`.
///
/// The endpoint `markers.end` is excluded so the picked loop does not
/// duplicate the neutral stance — the first picked frame already is it, and
/// a forward loop wraps back to it. `target_count` is clamped to the
/// available span and to at least two frames.
#[must_use]
pub fn pick_loop_frames(
    frames: &[VideoFrame],
    markers: LoopMarkers,
    target_count: usize,
) -> Vec<usize> {
    let n = frames.len();
    if n == 0 {
        return Vec::new();
    }
    let start = markers.start.min(n - 1);
    let end = markers.end.min(n - 1);
    if end <= start {
        return vec![start];
    }
    // Span is half-open [start, end): the recurrence frame is the loop's
    // wrap target, not a distinct picked frame.
    let span = end - start;
    let count = target_count.clamp(2, span.max(2)).min(span);
    if count <= 1 {
        return vec![start];
    }
    let mut picks = Vec::with_capacity(count);
    for i in 0..count {
        // Spread i across [0, span) so the last pick stops just before end.
        let offset = (i * span) / count;
        picks.push(start + offset);
    }
    picks.dedup();
    picks
}

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
mod tests {
    use super::*;
    use crate::plugin::context::PixelData;

    fn frame(value: u8, ts: u32) -> VideoFrame {
        // 2x2 solid RGBA frame; `value` drives the colour so motion between
        // frames is a function of the value delta.
        let bytes = [value, value, value, 255].repeat(4);
        VideoFrame {
            pixels: PixelData::rgba8(2, 2, bytes),
            timestamp_ms: ts,
        }
    }

    #[test]
    fn seam_similarity_identical_is_one() {
        let frames = vec![frame(100, 0), frame(100, 100)];
        assert!((seam_similarity(&frames, 0, 1) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn auto_markers_short_clip_spans_all() {
        let frames = vec![frame(0, 0), frame(50, 100)];
        let m = auto_loop_markers(&frames);
        assert_eq!(m.start, 0);
        assert_eq!(m.end, 1);
    }

    #[test]
    fn auto_markers_finds_recurring_neutral() {
        // A neutral pose (value 0) recurs at indices 0 and 6; the swing
        // peaks in between. Markers should bound one cycle to the recurrence.
        let pattern = [0u8, 80, 160, 200, 160, 80, 0, 80];
        let frames: Vec<VideoFrame> = pattern
            .iter()
            .enumerate()
            .map(|(i, &v)| frame(v, i as u32 * 100))
            .collect();
        let m = auto_loop_markers(&frames);
        // Start is a low-motion neutral frame; end recurs to a similar pose.
        assert!(m.end > m.start);
        assert!(seam_similarity(&frames, m.start, m.end) > 0.5);
    }

    #[test]
    fn pick_evenly_spaced_count() {
        let frames: Vec<VideoFrame> = (0..12).map(|i| frame((i * 20) as u8, i * 100)).collect();
        let picks = pick_loop_frames(&frames, LoopMarkers { start: 0, end: 10 }, 5);
        assert_eq!(picks.len(), 5);
        assert_eq!(picks[0], 0);
        assert!(*picks.last().unwrap() < 10, "endpoint excluded");
        // Strictly increasing.
        assert!(picks.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn pick_clamps_to_span() {
        let frames: Vec<VideoFrame> = (0..6).map(|i| frame(i as u8, i * 100)).collect();
        let picks = pick_loop_frames(&frames, LoopMarkers { start: 1, end: 4 }, 10);
        // Span is 3, so at most 3 distinct picks.
        assert!(picks.len() <= 3);
        assert_eq!(picks[0], 1);
    }

    #[test]
    fn pick_empty_frames() {
        assert!(pick_loop_frames(&[], LoopMarkers { start: 0, end: 0 }, 5).is_empty());
    }
}
