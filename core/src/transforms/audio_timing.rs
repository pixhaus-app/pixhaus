//! Beat-driven frame timing from audio.
//!
//! Ported from the Tauri `ai/src/verbs/audio_timing.rs` energy-envelope
//! onset detector. Beat mode only — the old verb's lip-sync mouth-layer
//! mode is out of scope here. Audio is read for timing, never embedded.
//!
//! # No audio crate
//!
//! WAV is parsed inline: RIFF container, PCM sub-format 1, with 8/16/24/32-bit
//! signed PCM all supported. There is no external decoder dependency, matching
//! the plan's constraint. Any other container (MP3, OGG, FLAC, or a non-PCM
//! WAV) returns [`AudioError::Unsupported`] with a convert-to-WAV message; a
//! truncated or malformed RIFF header returns [`AudioError::Malformed`].
//!
//! # Detection algorithm
//!
//! 1. Parse PCM samples from the WAV input.
//! 2. Downsample to mono by averaging channels.
//! 3. Split the mono stream into 10 ms windows and compute RMS energy per
//!    window.
//! 4. Take the positive first-order energy difference (rising edges only).
//! 5. Collect peaks above `sensitivity * max_difference` as onset candidates.
//! 6. Enforce a 50 ms minimum gap between accepted onsets.
//! 7. [`onset_frame_durations`] snaps each onset to the nearest frame boundary
//!    at `fps` and converts the inter-onset intervals to per-frame durations.

use thiserror::Error;

/// Minimum gap between accepted onset candidates, in milliseconds.
///
/// Stops the detector from double-firing on the steep attack edge of a single
/// beat's energy envelope.
const MIN_ONSET_GAP_MS: f32 = 50.0;

/// Analysis window size, in milliseconds.
const WINDOW_MS: f32 = 10.0;

/// Errors raised when reading audio for timing.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AudioError {
    /// The input is not PCM WAV. MP3, OGG, FLAC, and non-PCM (e.g. IEEE-float)
    /// WAV all land here. The message names the convert-to-WAV remedy.
    #[error("{0}")]
    Unsupported(String),

    /// The input claims to be RIFF/WAVE but its header is truncated or
    /// internally inconsistent.
    #[error("malformed WAV: {0}")]
    Malformed(String),
}

/// A detected energy onset, in milliseconds from the start of the clip.
pub type OnsetMs = f32;

/// Detects beat onsets in a PCM WAV byte stream and returns their times in
/// milliseconds from the start.
///
/// `sensitivity` is clamped to `[0.0, 1.0]`: higher detects more beats, lower
/// keeps only the strongest. The returned vector is empty for silence or a
/// constant-amplitude signal — that is a valid "no beats" result, not an error.
///
/// # Errors
///
/// - [`AudioError::Unsupported`] if the bytes are not PCM WAV (MP3/OGG/FLAC
///   magic, or a non-PCM WAV format code).
/// - [`AudioError::Malformed`] if a RIFF/WAVE header is truncated or its `fmt`
///   or `data` chunk is missing or inconsistent.
pub fn detect_onsets_wav(bytes: &[u8], sensitivity: f32) -> Result<Vec<OnsetMs>, AudioError> {
    if !is_wav_magic(bytes) {
        return Err(AudioError::Unsupported(format!(
            "{}; convert to PCM WAV before syncing",
            describe_foreign_format(bytes)
        )));
    }
    let pcm = parse_wav_mono_f32(bytes)?;
    Ok(detect_onsets(&pcm.samples, pcm.sample_rate, sensitivity.clamp(0.0, 1.0)))
}

/// Converts onset times (milliseconds) to per-frame durations (milliseconds) at
/// `fps`.
///
/// Each onset that lands on a distinct frame boundary becomes one frame whose
/// duration is the gap to the next onset. Two onsets that snap to the same
/// frame index merge, so no zero-length frame is produced. The final frame
/// reuses the previous duration (or one frame-period if there is a single
/// beat). Every duration is floored at `1` ms.
///
/// `fps` is clamped to `[1.0, 240.0]`. An empty onset list yields an empty
/// result.
#[must_use]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn onset_frame_durations(onset_times_ms: &[OnsetMs], fps: f32) -> Vec<u32> {
    if onset_times_ms.is_empty() {
        return Vec::new();
    }

    let fps = fps.clamp(1.0, 240.0);
    let frame_period_ms = 1000.0 / fps;

    // Snap each onset to the nearest frame boundary.
    let snapped = onset_times_ms.iter().map(|&t| (t / frame_period_ms).round().max(0.0) as u32);

    // Deduplicate adjacent identical frame indices before computing durations,
    // so two onsets landing on the same frame do not yield a zero-length frame.
    let mut unique: Vec<u32> = Vec::with_capacity(onset_times_ms.len());
    for f in snapped {
        if unique.last() != Some(&f) {
            unique.push(f);
        }
    }
    if unique.is_empty() {
        return Vec::new();
    }

    let n = unique.len();
    let mut durations: Vec<u32> = Vec::with_capacity(n);
    for i in 0..n {
        let dur_ms = if i + 1 < n {
            let here = unique.get(i).copied().unwrap_or(0);
            let next = unique.get(i + 1).copied().unwrap_or(here);
            let gap = next.saturating_sub(here);
            (gap as f32 * frame_period_ms).round() as u32
        } else if i > 0 {
            durations.get(i - 1).copied().unwrap_or_else(|| frame_period_ms.round() as u32)
        } else {
            frame_period_ms.round() as u32
        };
        durations.push(dur_ms.max(1));
    }

    durations
}

// ---------------------------------------------------------------------------
// Format sniffing
// ---------------------------------------------------------------------------

/// Whether the bytes begin with a RIFF/WAVE container header.
fn is_wav_magic(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && bytes.get(0..4) == Some(b"RIFF") && bytes.get(8..12) == Some(b"WAVE")
}

/// Describes a non-WAV container from its magic bytes for the error message.
fn describe_foreign_format(bytes: &[u8]) -> &'static str {
    if bytes.get(0..3) == Some(b"ID3") || bytes.get(0..2) == Some(b"\xff\xfb") || bytes.get(0..2) == Some(b"\xff\xf3") {
        "input looks like MP3 audio, not PCM WAV"
    } else if bytes.get(0..4) == Some(b"OggS") {
        "input looks like OGG audio, not PCM WAV"
    } else if bytes.get(0..4) == Some(b"fLaC") {
        "input looks like FLAC audio, not PCM WAV"
    } else {
        "input is not a PCM WAV file"
    }
}

// ---------------------------------------------------------------------------
// WAV parsing
// ---------------------------------------------------------------------------

/// Mono f32 PCM result from the WAV parser.
struct MonoPcm {
    samples: Vec<f32>,
    sample_rate: u32,
}

/// Reads a little-endian `u16` from `data` at `offset`.
fn read_u16_le(data: &[u8], offset: usize) -> Result<u16, AudioError> {
    let b: [u8; 2] = data
        .get(offset..offset + 2)
        .ok_or_else(|| AudioError::Malformed("unexpected end of header".into()))?
        .try_into()
        .map_err(|_| AudioError::Malformed("header byte conversion failed".into()))?;
    Ok(u16::from_le_bytes(b))
}

/// Reads a little-endian `u32` from `data` at `offset`.
fn read_u32_le(data: &[u8], offset: usize) -> Result<u32, AudioError> {
    let b: [u8; 4] = data
        .get(offset..offset + 4)
        .ok_or_else(|| AudioError::Malformed("unexpected end of header".into()))?
        .try_into()
        .map_err(|_| AudioError::Malformed("header byte conversion failed".into()))?;
    Ok(u32::from_le_bytes(b))
}

/// Parses a RIFF/WAVE PCM file into a mono f32 sample stream.
///
/// Walks the chunk list for `fmt ` and `data`. Multi-channel audio is downmixed
/// to mono by averaging channels. Supported bit depths: 8, 16, 24, 32 (signed
/// PCM, format code 1).
fn parse_wav_mono_f32(bytes: &[u8]) -> Result<MonoPcm, AudioError> {
    // The caller already checked the RIFF/WAVE magic; re-check defensively in
    // case this is reached directly.
    if !is_wav_magic(bytes) {
        return Err(AudioError::Malformed("not a RIFF/WAVE file".into()));
    }

    let mut pos: usize = 12;
    let mut sample_rate: u32 = 0;
    let mut channels: u16 = 0;
    let mut bits_per_sample: u16 = 0;
    let mut audio_format: u16 = 0;
    let mut pcm_data: &[u8] = &[];

    while pos + 8 <= bytes.len() {
        let chunk_id = bytes.get(pos..pos + 4).ok_or_else(|| AudioError::Malformed("truncated chunk header".into()))?;
        let chunk_size = read_u32_le(bytes, pos + 4)? as usize;
        let data_start = pos + 8;

        if chunk_id == b"fmt " {
            if chunk_size < 16 {
                return Err(AudioError::Malformed("fmt chunk too short".into()));
            }
            audio_format = read_u16_le(bytes, data_start)?;
            channels = read_u16_le(bytes, data_start + 2)?;
            sample_rate = read_u32_le(bytes, data_start + 4)?;
            bits_per_sample = read_u16_le(bytes, data_start + 14)?;
        } else if chunk_id == b"data" {
            let data_end = (data_start + chunk_size).min(bytes.len());
            pcm_data = bytes.get(data_start..data_end).ok_or_else(|| AudioError::Malformed("data chunk out of range".into()))?;
        }

        // RIFF chunks are padded to even byte boundaries.
        pos = data_start + chunk_size + (chunk_size & 1);
    }

    if sample_rate == 0 || channels == 0 {
        return Err(AudioError::Malformed("fmt chunk missing or zeroed".into()));
    }
    if pcm_data.is_empty() {
        return Err(AudioError::Malformed("data chunk missing or empty".into()));
    }
    if audio_format != 1 {
        return Err(AudioError::Unsupported(format!(
            "WAV audio format code {audio_format} is not PCM (code 1); convert to PCM WAV before syncing"
        )));
    }

    let samples = decode_pcm_to_mono_f32(pcm_data, channels, bits_per_sample)?;
    Ok(MonoPcm { samples, sample_rate })
}

/// Decodes interleaved multi-channel PCM bytes to a mono f32 stream, averaging
/// channels per frame and normalising to `[-1.0, 1.0]`.
#[allow(clippy::cast_precision_loss)]
fn decode_pcm_to_mono_f32(data: &[u8], channels: u16, bits_per_sample: u16) -> Result<Vec<f32>, AudioError> {
    let channels = usize::from(channels);
    let bytes_per_sample = usize::from(bits_per_sample / 8);
    if bytes_per_sample == 0 {
        return Err(AudioError::Malformed("bits_per_sample must be a multiple of 8 and >= 8".into()));
    }
    if channels == 0 {
        return Err(AudioError::Malformed("channel count must be >= 1".into()));
    }

    let bytes_per_frame = bytes_per_sample * channels;
    let mut mono = Vec::with_capacity(data.len() / bytes_per_frame.max(1));

    for frame_bytes in data.chunks(bytes_per_frame) {
        if frame_bytes.len() < bytes_per_frame {
            break; // incomplete trailing frame — skip
        }
        let mut sum = 0.0f32;
        for ch in 0..channels {
            let start = ch * bytes_per_sample;
            let sample_bytes = frame_bytes.get(start..start + bytes_per_sample).ok_or_else(|| AudioError::Malformed("truncated PCM frame".into()))?;
            sum += decode_sample(sample_bytes, bits_per_sample)?;
        }
        mono.push(sum / channels as f32);
    }

    Ok(mono)
}

/// Decodes one PCM sample from raw bytes, normalised to `[-1.0, 1.0]`.
#[allow(clippy::cast_precision_loss)]
fn decode_sample(sample_bytes: &[u8], bits_per_sample: u16) -> Result<f32, AudioError> {
    match bits_per_sample {
        8 => {
            // 8-bit PCM is unsigned; 128 is the centre.
            let b = sample_bytes.first().ok_or_else(|| AudioError::Malformed("truncated 8-bit sample".into()))?;
            Ok((f32::from(*b) - 128.0) / 128.0)
        }
        16 => {
            let arr: [u8; 2] = sample_bytes.try_into().map_err(|_| AudioError::Malformed("malformed 16-bit sample".into()))?;
            Ok(f32::from(i16::from_le_bytes(arr)) / 32_768.0)
        }
        24 => {
            let arr: [u8; 3] = sample_bytes.try_into().map_err(|_| AudioError::Malformed("malformed 24-bit sample".into()))?;
            let [b0, b1, b2] = arr;
            let raw = i32::from_le_bytes([b0, b1, b2, 0]) >> 8;
            Ok(raw as f32 / 8_388_608.0)
        }
        32 => {
            let arr: [u8; 4] = sample_bytes.try_into().map_err(|_| AudioError::Malformed("malformed 32-bit sample".into()))?;
            Ok(i32::from_le_bytes(arr) as f32 / 2_147_483_648.0)
        }
        bps => Err(AudioError::Unsupported(format!(
            "WAV bits-per-sample {bps} is not supported; use 8, 16, 24, or 32-bit PCM WAV"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Onset detection
// ---------------------------------------------------------------------------

/// Detects onset times (milliseconds) with an energy-envelope detector.
///
/// Returns an empty `Vec` for silence or a constant-amplitude signal.
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn detect_onsets(samples: &[f32], sample_rate: u32, sensitivity: f32) -> Vec<OnsetMs> {
    if samples.is_empty() || sample_rate == 0 {
        return Vec::new();
    }

    let window_size = ((f64::from(sample_rate) * f64::from(WINDOW_MS) / 1000.0).round() as usize).max(1);

    // Per-window RMS energy.
    let energies: Vec<f32> = samples
        .chunks(window_size)
        .map(|chunk| {
            let sum_sq: f32 = chunk.iter().map(|s| s * s).sum();
            (sum_sq / chunk.len() as f32).sqrt()
        })
        .collect();

    if energies.is_empty() {
        return Vec::new();
    }

    // Prepend a silence window so an onset at sample 0 reads as a rising edge
    // from zero rather than being missed for lack of a prior window.
    let mut energies_with_prior = Vec::with_capacity(energies.len() + 1);
    energies_with_prior.push(0.0f32);
    energies_with_prior.extend_from_slice(&energies);

    // Positive first-order energy difference (rising edges only).
    let diffs: Vec<f32> = energies_with_prior
        .windows(2)
        .map(|w| {
            let prev = w.first().copied().unwrap_or(0.0);
            let next = w.get(1).copied().unwrap_or(0.0);
            (next - prev).max(0.0)
        })
        .collect();

    let max_diff = diffs.iter().copied().fold(0.0f32, f32::max);
    if max_diff == 0.0 {
        return Vec::new();
    }

    let threshold = sensitivity * max_diff;
    let min_gap_windows = (MIN_ONSET_GAP_MS / WINDOW_MS).ceil() as usize;

    let mut onsets: Vec<OnsetMs> = Vec::new();
    let mut last_onset_window: Option<usize> = None;

    for (i, &d) in diffs.iter().enumerate() {
        if d >= threshold {
            let gap = last_onset_window.map_or(usize::MAX, |lw| i.saturating_sub(lw));
            if gap >= min_gap_windows {
                onsets.push(i as f32 * WINDOW_MS);
                last_onset_window = Some(i);
            }
        }
    }

    onsets
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a minimal mono 16-bit PCM WAV at `sample_rate` Hz. The sample
    /// generator decides amplitude per sample, so a test can place loud bursts
    /// at known offsets to produce onsets at predictable times.
    fn mono_wav_16bit(sample_rate: u32, samples: u32, sample_at: impl Fn(u32) -> i16) -> Vec<u8> {
        let channels: u16 = 1;
        let bits: u16 = 16;
        let bytes_per_sample = u32::from(bits / 8);
        let data_size = samples * u32::from(channels) * bytes_per_sample;
        let file_size = 36 + data_size;

        let mut wav = Vec::with_capacity((file_size + 8) as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&file_size.to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        let byte_rate = sample_rate * u32::from(channels) * bytes_per_sample;
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        let block_align = channels * bits / 8;
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&bits.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        for i in 0..samples {
            wav.extend_from_slice(&sample_at(i).to_le_bytes());
        }
        wav
    }

    // ── format / header errors ────────────────────────────────────────────

    #[test]
    fn mp3_magic_is_unsupported() {
        let bytes = b"ID3\x04\x00\x00garbage payload bytes that are long enough".to_vec();
        let err = detect_onsets_wav(&bytes, 0.5).unwrap_err();
        assert!(matches!(err, AudioError::Unsupported(_)), "got {err:?}");
        assert!(err.to_string().contains("WAV"), "message should mention WAV: {err}");
    }

    #[test]
    fn ogg_magic_is_unsupported() {
        let bytes = b"OggS\x00 some ogg container bytes here".to_vec();
        assert!(matches!(detect_onsets_wav(&bytes, 0.5), Err(AudioError::Unsupported(_))));
    }

    #[test]
    fn non_pcm_wav_is_unsupported() {
        // A RIFF/WAVE whose fmt audio-format code is 3 (IEEE float), not 1 (PCM).
        let mut wav = mono_wav_16bit(44_100, 100, |_| 0);
        wav[20] = 3;
        wav[21] = 0;
        let err = detect_onsets_wav(&wav, 0.5).unwrap_err();
        assert!(matches!(err, AudioError::Unsupported(_)), "got {err:?}");
    }

    #[test]
    fn truncated_riff_header_is_malformed() {
        // RIFF/WAVE magic present, but no fmt/data chunks and too short to hold
        // any — the parser cannot find a valid fmt chunk.
        let mut bytes = vec![0u8; 12];
        bytes[0..4].copy_from_slice(b"RIFF");
        bytes[8..12].copy_from_slice(b"WAVE");
        let err = detect_onsets_wav(&bytes, 0.5).unwrap_err();
        assert!(matches!(err, AudioError::Malformed(_)), "got {err:?}");
    }

    #[test]
    fn empty_input_is_unsupported_not_panic() {
        assert!(matches!(detect_onsets_wav(&[], 0.5), Err(AudioError::Unsupported(_))));
    }

    // ── synthetic beat spacing → onset frames ─────────────────────────────

    #[test]
    fn synthetic_beats_land_on_expected_frames() {
        // 1 kHz sample rate keeps the math readable: a 10 ms window is 10
        // samples, so a frame index at 12 fps (≈ 83 ms per frame) maps cleanly.
        // Place a loud 30-sample burst every 500 ms (at 0, 500, 1000 ms), with
        // silence between. The detector should fire one onset per burst.
        let sample_rate = 1_000u32;
        let total = 1_500u32; // 1.5 s
        let wav = mono_wav_16bit(sample_rate, total, |i| {
            let ms = i; // 1 sample == 1 ms at 1 kHz
            let near_beat = ms < 30 || (500..530).contains(&ms) || (1000..1030).contains(&ms);
            if near_beat { i16::MAX } else { 0 }
        });

        let onsets = detect_onsets_wav(&wav, 0.3).expect("detect");
        assert_eq!(onsets.len(), 3, "expected three beats, got {onsets:?}");
        // Onsets snap to the 10 ms window grid: 0, 500, 1000 ms.
        assert!((onsets[0] - 0.0).abs() < WINDOW_MS, "first onset {} should be near 0", onsets[0]);
        assert!((onsets[1] - 500.0).abs() < WINDOW_MS, "second onset {} should be near 500", onsets[1]);
        assert!((onsets[2] - 1000.0).abs() < WINDOW_MS, "third onset {} should be near 1000", onsets[2]);

        // At 12 fps (≈ 83.33 ms/frame), 500 ms is 6 frames apart. The per-frame
        // durations between beats should reflect that spacing.
        let durations = onset_frame_durations(&onsets, 12.0);
        assert_eq!(durations.len(), 3);
        // Frame 0 and frame 1 each span the 500 ms gap to the next beat,
        // rounding to 6 frame-periods ≈ 500 ms.
        assert!((480..=520).contains(&durations[0]), "gap 0 was {}", durations[0]);
        assert!((480..=520).contains(&durations[1]), "gap 1 was {}", durations[1]);
        // The final frame reuses the previous duration.
        assert_eq!(durations[2], durations[1]);
    }

    #[test]
    fn silence_yields_no_onsets() {
        let wav = mono_wav_16bit(8_000, 8_000, |_| 0);
        assert!(detect_onsets_wav(&wav, 0.5).expect("detect").is_empty());
    }

    #[test]
    fn sensitivity_is_clamped() {
        // Out-of-range sensitivity must not panic; it clamps into [0, 1].
        let wav = mono_wav_16bit(8_000, 800, |i| if i < 50 { i16::MAX } else { 0 });
        assert!(detect_onsets_wav(&wav, 5.0).is_ok());
        assert!(detect_onsets_wav(&wav, -1.0).is_ok());
    }

    // ── onset → frame-duration conversion ─────────────────────────────────

    #[test]
    fn durations_empty_for_empty_onsets() {
        assert!(onset_frame_durations(&[], 12.0).is_empty());
    }

    #[test]
    fn durations_single_onset_is_one_frame() {
        let durations = onset_frame_durations(&[0.0], 12.0);
        assert_eq!(durations.len(), 1);
        assert!(durations[0] >= 1);
    }

    #[test]
    fn durations_dedup_onsets_snapping_to_same_frame() {
        // Two onsets 1 ms apart both snap to frame 0 at 12 fps and merge.
        assert_eq!(onset_frame_durations(&[0.0, 1.0], 12.0).len(), 1);
    }

    #[test]
    fn durations_floor_at_one_ms() {
        // A pathological fps clamp still produces a >= 1 ms duration per frame.
        let durations = onset_frame_durations(&[0.0, 100.0, 200.0], 240.0);
        assert!(durations.iter().all(|&d| d >= 1));
    }

    #[test]
    fn durations_match_inter_onset_interval() {
        // Three evenly spaced beats 250 ms apart at 8 fps (125 ms/frame): each
        // gap is 2 frames ≈ 250 ms.
        let durations = onset_frame_durations(&[0.0, 250.0, 500.0], 8.0);
        assert_eq!(durations.len(), 3);
        assert!((240..=260).contains(&durations[0]), "{}", durations[0]);
        assert!((240..=260).contains(&durations[1]), "{}", durations[1]);
    }
}
