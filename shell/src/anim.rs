//! Animation clip processing: decode an image-to-video clip into frames, find
//! a clean loop, and pick evenly spaced frames.
//!
//! The frame-pick logic is ported from
//! `ai/src/verbs/motion_from_video/frame_pick.rs`, made self-contained over
//! tightly-packed RGBA8 [`VideoFrame`]s (no dependency on the ai `PixelData`).
//!
//! Clip decoding is pluggable: [`decode_clip`] handles GIF/APNG with the
//! `image` crate (no external binary). MP4/WebM decode needs `ffmpeg` and is
//! the isolated follow-up — it returns [`DecodeError::NeedsFfmpeg`] for now.

use std::io::Cursor;

use image::AnimationDecoder;
use mp4::{MediaType, Mp4Reader};
use openh264::decoder::Decoder;
use openh264::formats::YUVSource;

/// One decoded clip frame: tightly-packed RGBA8 plus its timestamp.
#[derive(Clone, Debug)]
pub struct VideoFrame {
    /// RGBA8 pixels, tightly packed (`width * height * 4` bytes).
    pub pixels: Vec<u8>,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Presentation timestamp in milliseconds.
    pub timestamp_ms: u32,
}

/// A pair of frame indices bounding one motion cycle.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct LoopMarkers {
    /// Inclusive start of the cycle (first neutral-stance frame).
    pub start: usize,
    /// Where the neutral stance recurs.
    pub end: usize,
}

/// Why a clip could not be decoded.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    /// The clip carries a codec this build cannot decode (only H.264 in mp4 is
    /// supported natively; for VP9/AV1/HEVC use a model that returns mp4/H.264
    /// or GIF).
    #[error("unsupported clip codec/format: {0}")]
    Unsupported(String),
    /// The clip bytes could not be decoded.
    #[error("clip decode failed: {0}")]
    Decode(String),
}

/// Decodes a clip into RGBA frames, with no external binary. GIF/APNG are
/// handled by the `image` crate; mp4 (H.264) is demuxed with `mp4` and decoded
/// with `openh264`. `fps` is used for mp4 timestamps; GIF/APNG carry their own
/// per-frame delays.
///
/// # Errors
/// [`DecodeError::Unsupported`] for a codec/container we cannot decode;
/// [`DecodeError::Decode`] on malformed input.
pub fn decode_clip(clip: &[u8], mime: &str, fps: u32) -> Result<Vec<VideoFrame>, DecodeError> {
    match mime {
        "image/gif" => decode_gif(clip),
        "image/apng" | "image/png" => decode_apng(clip),
        "video/mp4" | "video/quicktime" | "application/mp4" => decode_mp4_h264(clip, fps.max(1)),
        other => Err(DecodeError::Unsupported(other.to_owned())),
    }
}

/// Prepends an Annex-B start code to a raw NAL and appends it to `out`.
fn push_annexb(out: &mut Vec<u8>, nal: &[u8]) {
    out.extend_from_slice(&[0, 0, 0, 1]);
    out.extend_from_slice(nal);
}

/// Converts AVCC length-prefixed NAL units (4-byte big-endian lengths, the mp4
/// in-band format) to Annex-B start-code-delimited units, appending to `out`.
fn avcc_to_annexb(avcc: &[u8], out: &mut Vec<u8>) {
    let mut i = 0;
    while i + 4 <= avcc.len() {
        let len = u32::from_be_bytes([avcc[i], avcc[i + 1], avcc[i + 2], avcc[i + 3]]) as usize;
        i += 4;
        if i + len > avcc.len() {
            break;
        }
        push_annexb(out, &avcc[i..i + len]);
        i += len;
    }
}

/// Demuxes an mp4 with the `mp4` crate and decodes its H.264 track to RGBA
/// frames with `openh264` — no ffmpeg, no external binary.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn decode_mp4_h264(clip: &[u8], fps: u32) -> Result<Vec<VideoFrame>, DecodeError> {
    if clip.is_empty() {
        return Err(DecodeError::Decode("clip is empty".into()));
    }
    let size = clip.len() as u64;
    let mut reader = Mp4Reader::read_header(Cursor::new(clip), size).map_err(|e| DecodeError::Decode(e.to_string()))?;

    // Find the first H.264 video track and pull its SPS/PPS.
    let mut h264_track: Option<(u32, Vec<u8>, Vec<u8>)> = None;
    for track in reader.tracks().values() {
        if matches!(track.media_type(), Ok(MediaType::H264)) {
            let sps = track.sequence_parameter_set().map_err(|e| DecodeError::Decode(e.to_string()))?.to_vec();
            let pps = track.picture_parameter_set().map_err(|e| DecodeError::Decode(e.to_string()))?.to_vec();
            h264_track = Some((track.track_id(), sps, pps));
            break;
        }
    }
    let Some((track_id, sps, pps)) = h264_track else {
        return Err(DecodeError::Unsupported("no H.264 video track in clip".into()));
    };

    let mut decoder = Decoder::new().map_err(|e| DecodeError::Decode(e.to_string()))?;
    // Prime the decoder with SPS/PPS before any frame.
    let mut header = Vec::new();
    push_annexb(&mut header, &sps);
    push_annexb(&mut header, &pps);
    let _ = decoder.decode(&header);

    let sample_count = reader.sample_count(track_id).map_err(|e| DecodeError::Decode(e.to_string()))?;

    let mut frames = Vec::with_capacity(sample_count as usize);
    for sample_id in 1..=sample_count {
        let Some(sample) = reader.read_sample(track_id, sample_id).map_err(|e| DecodeError::Decode(e.to_string()))? else {
            continue;
        };
        // Re-send SPS/PPS ahead of each keyframe so a mid-stream decode stays valid.
        let mut au = Vec::new();
        if sample.is_sync {
            push_annexb(&mut au, &sps);
            push_annexb(&mut au, &pps);
        }
        avcc_to_annexb(&sample.bytes, &mut au);

        if let Ok(Some(yuv)) = decoder.decode(&au) {
            let (width, height) = yuv.dimensions();
            let mut rgba = vec![0u8; yuv.rgba8_len()];
            yuv.write_rgba8(&mut rgba);
            let ts = (u64::from(frames.len() as u32) * 1000 / u64::from(fps)) as u32;
            frames.push(VideoFrame {
                pixels: rgba,
                width: width as u32,
                height: height as u32,
                timestamp_ms: ts,
            });
        }
    }

    if frames.is_empty() {
        return Err(DecodeError::Decode("H.264 decode produced no frames".into()));
    }
    Ok(frames)
}

fn collect_anim<'a, D: AnimationDecoder<'a>>(decoder: D) -> Result<Vec<VideoFrame>, DecodeError> {
    let frames = decoder.into_frames().collect_frames().map_err(|e| DecodeError::Decode(e.to_string()))?;
    let mut out = Vec::with_capacity(frames.len());
    let mut ts = 0u32;
    for frame in frames {
        let (num, den) = frame.delay().numer_denom_ms();
        let delay = num.checked_div(den).unwrap_or(0);
        let buffer = frame.into_buffer();
        let (width, height) = (buffer.width(), buffer.height());
        out.push(VideoFrame {
            pixels: buffer.into_raw(),
            width,
            height,
            timestamp_ms: ts,
        });
        ts = ts.saturating_add(delay);
    }
    Ok(out)
}

fn decode_gif(clip: &[u8]) -> Result<Vec<VideoFrame>, DecodeError> {
    let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(clip)).map_err(|e| DecodeError::Decode(e.to_string()))?;
    collect_anim(decoder)
}

fn decode_apng(clip: &[u8]) -> Result<Vec<VideoFrame>, DecodeError> {
    let decoder = image::codecs::png::PngDecoder::new(Cursor::new(clip))
        .map_err(|e| DecodeError::Decode(e.to_string()))?
        .apng()
        .map_err(|e| DecodeError::Decode(e.to_string()))?;
    collect_anim(decoder)
}

/// Encodes a frame to PNG bytes (for backend round-trips like background
/// removal). Returns `None` if the buffer is malformed.
#[must_use]
pub fn encode_png(frame: &VideoFrame) -> Option<Vec<u8>> {
    let img: image::RgbaImage = image::ImageBuffer::from_raw(frame.width, frame.height, frame.pixels.clone())?;
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

/// Decodes PNG bytes into a [`VideoFrame`] at `timestamp_ms`.
#[must_use]
pub fn decode_png(png: &[u8], timestamp_ms: u32) -> Option<VideoFrame> {
    let rgba = image::load_from_memory(png).ok()?.to_rgba8();
    let (width, height) = (rgba.width(), rgba.height());
    Some(VideoFrame {
        pixels: rgba.into_raw(),
        width,
        height,
        timestamp_ms,
    })
}

/// Mean per-pixel RGB difference between two equally-sized RGBA frames,
/// normalized to `0.0..=1.0`. Pixels where both alphas are near-zero are
/// skipped so transparent background does not dominate. Ported from the ai
/// `motion_magnitude`.
#[must_use]
pub fn motion_magnitude(a: &VideoFrame, b: &VideoFrame) -> f32 {
    if a.width != b.width || a.height != b.height || a.width == 0 || a.pixels.len() != b.pixels.len() {
        return 0.0;
    }
    let mut sum = 0.0_f64;
    let mut counted = 0.0_f64;
    for (pa, pb) in a.pixels.chunks_exact(4).zip(b.pixels.chunks_exact(4)) {
        if u32::from(pa[3]) + u32::from(pb[3]) < 64 {
            continue;
        }
        sum += (f64::from(pa[0]) - f64::from(pb[0])).abs() + (f64::from(pa[1]) - f64::from(pb[1])).abs() + (f64::from(pa[2]) - f64::from(pb[2])).abs();
        counted += 1.0;
    }
    if counted == 0.0 {
        return 0.0;
    }
    #[allow(clippy::cast_possible_truncation)]
    let mag = (sum / (counted * 3.0 * 255.0)) as f32;
    mag.clamp(0.0, 1.0)
}

/// Pixel similarity between two frames in `0.0..=1.0` (`1.0` = identical).
/// Part of the frame-pick API (loop-seam quality); surfaced by the jobs tray
/// in a later phase.
#[must_use]
#[allow(dead_code)]
pub fn seam_similarity(frames: &[VideoFrame], a: usize, b: usize) -> f32 {
    match (frames.get(a), frames.get(b)) {
        (Some(fa), Some(fb)) => 1.0 - motion_magnitude(fa, fb),
        _ => 0.0,
    }
}

/// Auto-detects loop markers: marker A is the lowest-motion frame in the first
/// half (the neutral stance); marker B is the later frame, at least a minimum
/// cycle past A, most similar to A. Ported from the ai frame picker.
#[must_use]
pub fn auto_loop_markers(frames: &[VideoFrame]) -> LoopMarkers {
    let n = frames.len();
    if n < 4 {
        return LoopMarkers {
            start: 0,
            end: n.saturating_sub(1),
        };
    }

    let motion: Vec<f32> = (0..n)
        .map(|i| if i == 0 { f32::MAX } else { motion_magnitude(&frames[i - 1], &frames[i]) })
        .collect();

    let half = (n / 2).max(2);
    let mut start = 1;
    let mut min_motion = f32::MAX;
    for (i, &m) in motion.iter().enumerate().take(half).skip(1) {
        if m < min_motion {
            min_motion = m;
            start = i;
        }
    }

    let min_cycle = (n / 4).max(2);
    let search_start = (start + min_cycle).min(n - 1);
    let mut end = n - 1;
    let mut best_sim = -1.0_f32;
    for k in search_start..n {
        let sim = 1.0 - motion_magnitude(&frames[start], &frames[k]);
        if sim > best_sim {
            best_sim = sim;
            end = k;
        }
    }

    LoopMarkers { start, end }
}

/// Picks exactly `target_count` evenly spaced, distinct frame indices to host
/// the loop. Samples a half-open window `[lo, hi)` (`hi` excluded so a forward
/// loop wraps to it without duplicating a frame). The window prefers the
/// detected loop `[start, end)`; when that is too short to supply `target_count`
/// distinct frames it widens to the clip tail, then to the whole clip. Only when
/// even the whole clip is shorter than `target_count` does it return fewer.
#[must_use]
pub fn pick_loop_frames(frames: &[VideoFrame], markers: LoopMarkers, target_count: usize) -> Vec<usize> {
    let n = frames.len();
    if n == 0 {
        return Vec::new();
    }
    let target = target_count.clamp(1, n);

    // Start from the detected loop, then widen until the window can supply
    // `target` distinct frames.
    let mut lo = markers.start.min(n - 1);
    let mut hi = if markers.end > lo { markers.end.min(n) } else { n };
    if hi - lo < target {
        hi = n; // widen to the clip tail
    }
    if hi - lo < target {
        lo = 0; // widen to the whole clip
        hi = n;
    }

    let span = hi - lo;
    let count = target.min(span);
    // With `span >= count` the offsets strictly increase, so the picks are
    // distinct without a dedup pass.
    (0..count).map(|i| lo + (i * span) / count).collect()
}

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
mod tests {
    use super::*;

    fn frame(value: u8, ts: u32) -> VideoFrame {
        VideoFrame {
            pixels: [value, value, value, 255].repeat(4),
            width: 2,
            height: 2,
            timestamp_ms: ts,
        }
    }

    #[test]
    fn seam_similarity_identical_is_one() {
        let frames = vec![frame(100, 0), frame(100, 100)];
        assert!((seam_similarity(&frames, 0, 1) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn motion_magnitude_identical_is_zero() {
        assert!(motion_magnitude(&frame(128, 0), &frame(128, 0)) < f32::EPSILON);
    }

    #[test]
    fn auto_markers_short_clip_spans_all() {
        let frames = vec![frame(0, 0), frame(50, 100)];
        let m = auto_loop_markers(&frames);
        assert_eq!(m.start, 0);
        assert_eq!(m.end, 1);
    }

    #[test]
    fn pick_loop_frames_evenly_spaced() {
        let frames: Vec<VideoFrame> = (0..10).map(|i| frame((i * 20) as u8, i * 100)).collect();
        let picks = pick_loop_frames(&frames, LoopMarkers { start: 0, end: 8 }, 4);
        assert_eq!(picks, vec![0, 2, 4, 6]);
    }

    #[test]
    fn pick_loop_frames_widens_a_short_loop_to_hit_the_target() {
        // The detected loop spans only 3 frames [2, 5); a 6-frame request widens
        // the window so it still yields exactly 6 distinct, ordered picks.
        let frames: Vec<VideoFrame> = (0..10).map(|i| frame((i * 20) as u8, i * 100)).collect();
        let picks = pick_loop_frames(&frames, LoopMarkers { start: 2, end: 5 }, 6);
        assert_eq!(picks.len(), 6);
        assert!(picks.windows(2).all(|w| w[0] < w[1]), "picks are strictly increasing: {picks:?}");
        assert!(picks.iter().all(|&i| i < frames.len()));
    }

    #[test]
    fn pick_loop_frames_clamps_target_to_the_clip_length() {
        // Asking for more frames than the clip holds returns one distinct pick
        // per available frame, not duplicates.
        let frames: Vec<VideoFrame> = (0..4).map(|i| frame((i * 20) as u8, i * 100)).collect();
        let picks = pick_loop_frames(&frames, LoopMarkers { start: 0, end: 3 }, 6);
        assert_eq!(picks, vec![0, 1, 2, 3]);
    }

    #[test]
    fn garbage_mp4_fails_cleanly() {
        // Not a valid mp4: the demuxer rejects it with a clean error, no panic.
        let err = decode_clip(&[0, 1, 2, 3, 4, 5, 6, 7], "video/mp4", 12).unwrap_err();
        assert!(matches!(err, DecodeError::Decode(_) | DecodeError::Unsupported(_)));
    }

    #[test]
    fn unknown_mime_is_unsupported() {
        let err = decode_clip(&[0, 1, 2], "video/webm", 12).unwrap_err();
        assert!(matches!(err, DecodeError::Unsupported(_)));
    }

    #[test]
    fn avcc_to_annexb_converts_length_prefixed_nals() {
        // Two NALs: a 2-byte and a 3-byte unit, each with a 4-byte BE length.
        let avcc = [
            0, 0, 0, 2, 0xAA, 0xBB, // NAL 1
            0, 0, 0, 3, 0xCC, 0xDD, 0xEE, // NAL 2
        ];
        let mut out = Vec::new();
        avcc_to_annexb(&avcc, &mut out);
        assert_eq!(out, vec![0, 0, 0, 1, 0xAA, 0xBB, 0, 0, 0, 1, 0xCC, 0xDD, 0xEE]);
    }

    #[test]
    fn avcc_to_annexb_ignores_truncated_tail() {
        // Declares a 4-byte NAL but only 2 bytes follow — must not panic or
        // emit a partial unit.
        let avcc = [0, 0, 0, 4, 0xAA, 0xBB];
        let mut out = Vec::new();
        avcc_to_annexb(&avcc, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn push_annexb_prepends_start_code() {
        let mut out = Vec::new();
        push_annexb(&mut out, &[0x67, 0x42]);
        assert_eq!(out, vec![0, 0, 0, 1, 0x67, 0x42]);
    }
}
