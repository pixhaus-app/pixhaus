//! Verb: Audio-driven timing — stream S34.
//!
//! Detects beats or syllables in an audio clip and places frame timing
//! markers at the detected times. In beat mode the verb produces frames
//! whose durations match the detected inter-beat intervals and adds a
//! frame tag covering the range. In lip-sync mode it additionally emits
//! a `"Mouth"` layer with placeholder 1×1 cels annotated with
//! `user_data.text = "open" | "closed"` so the artist can filter by
//! mouth state and fill in the real art.
//!
//! # Beat detection algorithm
//!
//! A classical energy-envelope onset detector — no inference backend
//! required:
//!
//! 1. Parse PCM samples from a WAV input (RIFF/WAVE, PCM sub-format 1).
//! 2. Downsample to mono by averaging channels.
//! 3. Split the mono stream into 10 ms windows and compute RMS energy
//!    per window.
//! 4. Compute the first-order energy difference (positive half only).
//! 5. Collect peaks that exceed `sensitivity × max_difference` as onset
//!    candidates.
//! 6. Enforce a `50 ms` minimum gap between accepted onsets.
//! 7. Snap each accepted onset time to the nearest frame boundary at
//!    `fps`.
//!
//! # Lip-sync mode
//!
//! Uses the same onset detector to find syllable boundaries, then adds
//! a mouth layer. Each cel carries `user_data.text` of `"open"` (onset
//! frame) or `"closed"` (gap frame). A 1×1 pixel placeholder buffer is
//! produced for every cel — white for open, black for closed — so the
//! layer renders visibly before the artist replaces the art.
//!
//! # Input format
//!
//! PCM WAV only (RIFF container, audio-format code `1`). The header is
//! parsed inline; no external audio crate is needed. 8-bit, 16-bit,
//! 24-bit, and 32-bit signed PCM are all supported. Passing MP3, OGG,
//! or FLAC returns [`super::error::VerbError::Schema`] with instructions
//! to convert to PCM WAV first.

use std::collections::HashSet;
use std::time::Duration;

use async_trait::async_trait;
use pixhaus_core::project::{
    Cel, Frame, FrameIndex, FrameRange, FrameTag, Layer, LayerId, LoopDirection, PixelBufferId,
    Size, UserData,
};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::plugin::context::{PixelData, VerbContext};
use crate::plugin::descriptor::{
    BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId,
};
use crate::plugin::error::{Result, VerbError};
use crate::plugin::inputs::VerbInputs;
use crate::plugin::output::{ActualCost, NewPixelBuffer, VerbEffect, VerbOutput};
use crate::plugin::progress::{VerbProgress, VerbProgressEvent};
use crate::plugin::verb::Verb;

/// Stable identifier for the built-in audio-timing verb.
pub const AUDIO_TIMING_VERB_ID: &str = "pixhaus.builtin.audio_timing";

/// Placeholder layer ID used for the mouth-shape layer in lip-sync mode.
/// The host rewrites it to a real `LayerId` on commit.
const MOUTH_LAYER_PLACEHOLDER: u32 = 0;

/// Base value for placeholder pixel-buffer IDs in lip-sync mode.
/// Buffer for frame N gets placeholder `MOUTH_BUF_BASE + N`.
const MOUTH_BUF_BASE: u32 = 100;

/// Minimum gap between accepted onset candidates (milliseconds).
///
/// Prevents the detector from double-firing on the energy envelope's
/// steep attack edge around a single beat.
const MIN_ONSET_GAP_MS: f32 = 50.0;

/// Analysis window size in milliseconds.
const WINDOW_MS: f32 = 10.0;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Audio container format hint.
///
/// The verb only processes WAV (PCM sub-format 1). All other variants
/// return [`VerbError::Schema`] with a convert-to-WAV message. Pass
/// `Unknown` to let the verb sniff the magic bytes.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioFormat {
    /// RIFF/WAVE PCM — supported natively.
    #[default]
    Wav,
    /// MPEG audio — not supported; convert to PCM WAV first.
    Mp3,
    /// OGG Vorbis — not supported; convert to PCM WAV first.
    Ogg,
    /// FLAC lossless — not supported; convert to PCM WAV first.
    Flac,
    /// Unknown container — the verb will attempt magic-byte detection.
    Unknown,
}

/// Operating mode.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioTimingMode {
    /// Detect beats (energy onsets) and produce beat-aligned frames plus
    /// a frame tag. No extra layer is added.
    #[default]
    Beat,
    /// Same as `Beat` but additionally emits a `"Mouth"` layer whose cels
    /// are annotated with the detected open/closed state.
    LipSync,
}

/// Inputs for [`AudioTimingVerb`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioTimingInputs {
    /// Raw audio bytes. Must be PCM WAV (RIFF/WAVE, format code 1) unless
    /// `format` is `Unknown` and magic bytes identify a WAV file.
    pub audio_bytes: Vec<u8>,
    /// Container format hint. Defaults to `wav`.
    #[serde(default)]
    pub format: AudioFormat,
    /// Operating mode. Defaults to `beat`.
    #[serde(default)]
    pub mode: AudioTimingMode,
    /// Target animation frame rate in frames per second. Beat times are
    /// snapped to the nearest frame boundary at this rate.
    /// Valid range: 1–120. Defaults to `12.0`.
    #[serde(default = "default_fps")]
    pub fps: f32,
    /// First beat frame will be inserted *after* this 0-based frame index.
    /// `0` means prepend (before frame 0). Defaults to `0`.
    #[serde(default)]
    pub start_frame: u32,
    /// Beat-detection sensitivity in `[0.0, 1.0]`. Higher values detect
    /// more beats; lower values pick only the strongest onsets.
    /// Defaults to `0.5`.
    #[serde(default = "default_sensitivity")]
    pub sensitivity: f32,
    /// Name for the generated frame tag. Defaults to `"Beat"` (beat mode)
    /// or `"Lip sync"` (lip-sync mode).
    #[serde(default)]
    pub tag_name: Option<String>,
    /// Name for the mouth-shape layer generated in lip-sync mode. Ignored
    /// in beat mode. Defaults to `"Mouth"`.
    #[serde(default)]
    pub layer_name: Option<String>,
}

fn default_fps() -> f32 {
    12.0
}
fn default_sensitivity() -> f32 {
    0.5
}

// ---------------------------------------------------------------------------
// Verb
// ---------------------------------------------------------------------------

/// Audio-driven timing verb — beat detection and lip sync from audio.
///
/// See module documentation for the algorithm and input format.
#[derive(Debug)]
pub struct AudioTimingVerb {
    descriptor: VerbDescriptor,
}

impl AudioTimingVerb {
    /// Constructs a fresh audio-timing verb.
    #[must_use]
    // `serde_json::json!` expands to internal helpers that call
    // `Result::unwrap` on infallible builders. The workspace disallows
    // `unwrap` everywhere, so we exempt this constructor.
    #[allow(clippy::disallowed_methods)]
    pub fn new() -> Self {
        let input_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "audio_bytes": {
                    "type": "array",
                    "items": {"type": "integer"},
                    "description": "PCM WAV bytes"
                },
                "format": {
                    "type": "string",
                    "enum": ["wav", "mp3", "ogg", "flac", "unknown"],
                    "default": "wav"
                },
                "mode": {
                    "type": "string",
                    "enum": ["beat", "lip_sync"],
                    "default": "beat"
                },
                "fps": {
                    "type": "number",
                    "minimum": 1.0,
                    "maximum": 120.0,
                    "default": 12.0
                },
                "start_frame": {
                    "type": "integer",
                    "minimum": 0,
                    "default": 0
                },
                "sensitivity": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.5
                },
                "tag_name": {"type": ["string", "null"]},
                "layer_name": {"type": ["string", "null"]}
            },
            "required": ["audio_bytes"]
        });

        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(AUDIO_TIMING_VERB_ID),
                display_name: "Audio-driven timing".into(),
                description: concat!(
                    "Detect beats or syllables in audio and place frame ",
                    "timing markers at the detected times"
                )
                .into(),
                version: env!("CARGO_PKG_VERSION").into(),
                // Beat detection runs entirely on the local CPU; no backend
                // capability is required. A registered AUDIO_ANALYSIS backend
                // would improve phoneme classification in lip-sync mode, but
                // the classical fallback covers both modes without one.
                required_capabilities: BackendCapabilities::empty(),
                input_schema,
                output_schema: None,
                output_kinds: vec![
                    EffectKind::AddFrames,
                    EffectKind::AddTag,
                    EffectKind::AddLayer,
                ],
                cost_estimate: CostEstimate {
                    typical_latency: Duration::from_millis(200),
                    max_latency: Duration::from_secs(2),
                    typical_usd_cents: 0.0,
                    max_usd_cents: 0.0,
                },
                streaming: true,
                cancellable: true,
                documentation_url: Some("docs/verbs/audio_timing.md".into()),
            },
        }
    }
}

impl Default for AudioTimingVerb {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Verb for AudioTimingVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        let parsed: AudioTimingInputs = inputs.deserialize()?;
        if parsed.audio_bytes.is_empty() {
            return Err(VerbError::Schema(
                "audio_timing: audio_bytes must not be empty".into(),
            ));
        }
        #[allow(clippy::float_cmp)]
        if !(1.0..=120.0).contains(&parsed.fps) {
            return Err(VerbError::Schema(format!(
                "audio_timing: fps must be in 1–120, got {}",
                parsed.fps
            )));
        }
        if !(0.0..=1.0).contains(&parsed.sensitivity) {
            return Err(VerbError::Schema(format!(
                "audio_timing: sensitivity must be in 0.0–1.0, got {}",
                parsed.sensitivity
            )));
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
    async fn invoke(
        &self,
        ctx: VerbContext,
        inputs: VerbInputs,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<VerbOutput> {
        let started = std::time::Instant::now();
        let inputs: AudioTimingInputs = inputs.deserialize_owned()?;
        let sprite_id = ctx.require_sprite_id()?;

        progress
            .send(VerbProgressEvent::Started { backend: None })
            .await;
        progress.step(Some(0.1), "parsing audio").await;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        // Resolve format — sniff magic bytes when the caller said Unknown.
        let format = match inputs.format {
            AudioFormat::Unknown => detect_format(&inputs.audio_bytes),
            other => other,
        };
        if !matches!(format, AudioFormat::Wav) {
            return Err(VerbError::Schema(format!(
                "audio_timing: unsupported format {format:?}; convert to PCM WAV before invoking"
            )));
        }

        let audio_bytes = inputs.audio_bytes.clone();
        let sensitivity = inputs.sensitivity;
        let fps = inputs.fps;

        progress.step(Some(0.2), "decoding PCM").await;
        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        // CPU-bound WAV decode + onset detection runs on the blocking pool.
        let onset_times_ms = tokio::task::spawn_blocking(move || {
            let pcm = parse_wav_mono_f32(&audio_bytes)?;
            Ok::<Vec<f32>, VerbError>(detect_onsets(&pcm.samples, pcm.sample_rate, sensitivity))
        })
        .await
        .map_err(|e| VerbError::Backend(format!("audio analysis task panicked: {e}")))??;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        progress.step(Some(0.6), "building frame effects").await;

        if onset_times_ms.is_empty() {
            return Err(VerbError::Backend(
                "no beats detected; try raising sensitivity or checking the audio content".into(),
            ));
        }

        let tag_name = inputs.tag_name.unwrap_or_else(|| match inputs.mode {
            AudioTimingMode::Beat => "Beat".into(),
            AudioTimingMode::LipSync => "Lip sync".into(),
        });

        let frame_durations_ms = onset_times_to_frame_durations(&onset_times_ms, fps);
        if frame_durations_ms.is_empty() {
            return Err(VerbError::Backend(
                "onsets all collapsed to the same frame; try a higher fps or different audio"
                    .into(),
            ));
        }

        let frame_count = frame_durations_ms.len() as u32;
        let after = if inputs.start_frame == 0 {
            None
        } else {
            Some(FrameIndex::new(inputs.start_frame - 1))
        };

        let frames: Vec<Frame> = frame_durations_ms
            .iter()
            .map(|&ms| Frame {
                duration_ms: ms.max(1),
                user_data: UserData::default(),
            })
            .collect();

        let tag_start = FrameIndex::new(inputs.start_frame);
        let tag_end = FrameIndex::new(inputs.start_frame + frame_count.saturating_sub(1));

        let mut effects = vec![
            VerbEffect::AddFrames {
                sprite: sprite_id,
                after,
                frames,
                cels: vec![],
                pixel_buffers: vec![],
            },
            VerbEffect::AddTag {
                sprite: sprite_id,
                tag: FrameTag {
                    name: tag_name.clone(),
                    range: FrameRange::new(tag_start, tag_end),
                    loop_direction: LoopDirection::Forward,
                    repeat: 0,
                    user_data: UserData::default(),
                },
            },
        ];

        if matches!(inputs.mode, AudioTimingMode::LipSync) {
            progress
                .step(Some(0.8), "generating mouth-shape layer")
                .await;
            if cancel.is_cancelled() {
                return Err(VerbError::Cancelled);
            }

            let layer_name = inputs.layer_name.unwrap_or_else(|| "Mouth".into());
            let layer = Layer::raster(LayerId::new(MOUTH_LAYER_PLACEHOLDER), &layer_name);
            let (cels, pixel_buffers) = build_mouth_cels(
                &onset_times_ms,
                fps,
                inputs.start_frame,
                frame_count,
                LayerId::new(MOUTH_LAYER_PLACEHOLDER),
            );
            effects.push(VerbEffect::AddLayer {
                sprite: sprite_id,
                layer,
                cels,
                pixel_buffers,
            });
        }

        progress.step(Some(1.0), "done").await;

        let elapsed = started.elapsed();
        let summary = match inputs.mode {
            AudioTimingMode::Beat => {
                format!("Add {frame_count} beat-timed frames tagged \"{tag_name}\"")
            }
            AudioTimingMode::LipSync => format!(
                "Add {frame_count} lip-sync frames tagged \"{tag_name}\" with mouth-shape layer"
            ),
        };

        Ok(VerbOutput {
            summary,
            effects,
            thumbnail: None,
            actual_cost: ActualCost::free(elapsed.max(Duration::from_micros(1))),
            notes: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// Audio format detection
// ---------------------------------------------------------------------------

/// Sniffs the audio format from magic bytes.
fn detect_format(bytes: &[u8]) -> AudioFormat {
    if bytes.len() >= 12 && bytes.get(0..4) == Some(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") {
        return AudioFormat::Wav;
    }
    if bytes.get(0..3) == Some(b"ID3")
        || bytes.get(0..2) == Some(b"\xff\xfb")
        || bytes.get(0..2) == Some(b"\xff\xf3")
    {
        return AudioFormat::Mp3;
    }
    if bytes.get(0..4) == Some(b"OggS") {
        return AudioFormat::Ogg;
    }
    if bytes.get(0..4) == Some(b"fLaC") {
        return AudioFormat::Flac;
    }
    AudioFormat::Unknown
}

// ---------------------------------------------------------------------------
// WAV parsing
// ---------------------------------------------------------------------------

/// Mono f32 PCM result from the WAV parser.
struct MonoPcm {
    samples: Vec<f32>,
    sample_rate: u32,
}

/// Reads a `u16` little-endian from `data` at `offset`.
fn read_u16_le(data: &[u8], offset: usize) -> Result<u16> {
    let b: [u8; 2] = data
        .get(offset..offset + 2)
        .ok_or_else(|| VerbError::Schema("WAV: unexpected end of header".into()))?
        .try_into()
        .map_err(|_| VerbError::Schema("WAV: header byte conversion failed".into()))?;
    Ok(u16::from_le_bytes(b))
}

/// Reads a `u32` little-endian from `data` at `offset`.
fn read_u32_le(data: &[u8], offset: usize) -> Result<u32> {
    let b: [u8; 4] = data
        .get(offset..offset + 4)
        .ok_or_else(|| VerbError::Schema("WAV: unexpected end of header".into()))?
        .try_into()
        .map_err(|_| VerbError::Schema("WAV: header byte conversion failed".into()))?;
    Ok(u32::from_le_bytes(b))
}

/// Parses a RIFF/WAVE PCM file and returns a mono f32 sample stream.
///
/// Walks the RIFF chunk list looking for `"fmt "` and `"data"` chunks.
/// Multi-channel audio is downmixed to mono by averaging channels.
/// Supported bit depths: 8, 16, 24, 32 (all signed PCM, format code 1).
fn parse_wav_mono_f32(bytes: &[u8]) -> Result<MonoPcm> {
    if bytes.len() < 12 {
        return Err(VerbError::Schema("WAV: file too short".into()));
    }
    if bytes.get(0..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WAVE") {
        return Err(VerbError::Schema(
            "WAV: not a RIFF/WAVE file; ensure audio is PCM WAV".into(),
        ));
    }

    let mut pos: usize = 12;
    let mut sample_rate: u32 = 0;
    let mut channels: u16 = 0;
    let mut bits_per_sample: u16 = 0;
    let mut audio_format: u16 = 0;
    let mut pcm_data: &[u8] = &[];

    while pos + 8 <= bytes.len() {
        let chunk_id = bytes
            .get(pos..pos + 4)
            .ok_or_else(|| VerbError::Schema("WAV: truncated chunk header".into()))?;
        let chunk_size = read_u32_le(bytes, pos + 4)? as usize;
        let data_start = pos + 8;

        if chunk_id == b"fmt " {
            if chunk_size < 16 {
                return Err(VerbError::Schema("WAV: fmt chunk too short".into()));
            }
            audio_format = read_u16_le(bytes, data_start)?;
            channels = read_u16_le(bytes, data_start + 2)?;
            sample_rate = read_u32_le(bytes, data_start + 4)?;
            bits_per_sample = read_u16_le(bytes, data_start + 14)?;
        } else if chunk_id == b"data" {
            let data_end = (data_start + chunk_size).min(bytes.len());
            pcm_data = bytes
                .get(data_start..data_end)
                .ok_or_else(|| VerbError::Schema("WAV: data chunk out of range".into()))?;
        }

        // RIFF chunks are padded to even byte boundaries.
        pos = data_start + chunk_size + (chunk_size & 1);
    }

    if sample_rate == 0 || channels == 0 {
        return Err(VerbError::Schema(
            "WAV: fmt chunk missing or malformed".into(),
        ));
    }
    if pcm_data.is_empty() {
        return Err(VerbError::Schema("WAV: data chunk missing or empty".into()));
    }
    if audio_format != 1 {
        return Err(VerbError::Backend(format!(
            "WAV: unsupported audio format code {audio_format}; only PCM (code 1) is supported"
        )));
    }

    let samples = decode_pcm_to_mono_f32(pcm_data, channels, bits_per_sample)?;
    Ok(MonoPcm {
        samples,
        sample_rate,
    })
}

/// Decodes interleaved multi-channel PCM bytes to a mono f32 stream.
///
/// Channels are averaged per frame. Normalised to `[-1.0, 1.0]`.
#[allow(clippy::cast_precision_loss)]
fn decode_pcm_to_mono_f32(data: &[u8], channels: u16, bits_per_sample: u16) -> Result<Vec<f32>> {
    let channels = usize::from(channels);
    let bytes_per_sample = usize::from(bits_per_sample / 8);
    if bytes_per_sample == 0 {
        return Err(VerbError::Schema(
            "WAV: bits_per_sample must be a multiple of 8 and >= 8".into(),
        ));
    }
    if channels == 0 {
        return Err(VerbError::Schema("WAV: channel count must be >= 1".into()));
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
            let sample_bytes = frame_bytes
                .get(start..start + bytes_per_sample)
                .ok_or_else(|| VerbError::Schema("WAV: truncated PCM frame".into()))?;
            let s = decode_sample(sample_bytes, bits_per_sample)?;
            sum += s;
        }
        mono.push(sum / channels as f32);
    }

    Ok(mono)
}

/// Decodes a single PCM sample from raw bytes, normalised to `[-1.0, 1.0]`.
#[allow(clippy::cast_precision_loss)]
fn decode_sample(sample_bytes: &[u8], bits_per_sample: u16) -> Result<f32> {
    match bits_per_sample {
        8 => {
            // 8-bit PCM is unsigned; 128 is the centre.
            let b = sample_bytes
                .first()
                .ok_or_else(|| VerbError::Schema("WAV: truncated 8-bit sample".into()))?;
            Ok((f32::from(*b) - 128.0) / 128.0)
        }
        16 => {
            let arr: [u8; 2] = sample_bytes
                .try_into()
                .map_err(|_| VerbError::Schema("WAV: malformed 16-bit sample".into()))?;
            Ok(f32::from(i16::from_le_bytes(arr)) / 32_768.0)
        }
        24 => {
            let arr: [u8; 3] = sample_bytes
                .try_into()
                .map_err(|_| VerbError::Schema("WAV: malformed 24-bit sample".into()))?;
            let [b0, b1, b2] = arr;
            let raw = i32::from_le_bytes([b0, b1, b2, 0]) >> 8;
            Ok(raw as f32 / 8_388_608.0)
        }
        32 => {
            let arr: [u8; 4] = sample_bytes
                .try_into()
                .map_err(|_| VerbError::Schema("WAV: malformed 32-bit sample".into()))?;
            Ok(i32::from_le_bytes(arr) as f32 / 2_147_483_648.0)
        }
        bps => Err(VerbError::Backend(format!(
            "WAV: unsupported bits-per-sample {bps}; supported: 8, 16, 24, 32"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Beat / onset detection
// ---------------------------------------------------------------------------

/// Detects onset times (in milliseconds) using an energy-envelope detector.
///
/// Returns an empty `Vec` for silence or constant-amplitude signals.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn detect_onsets(samples: &[f32], sample_rate: u32, sensitivity: f32) -> Vec<f32> {
    if samples.is_empty() || sample_rate == 0 {
        return Vec::new();
    }

    let window_size =
        ((f64::from(sample_rate) * f64::from(WINDOW_MS) / 1000.0).round() as usize).max(1);

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

    // Prepend a silence window so the very first window's energy appears as
    // a rising edge from zero. Without this, an onset at sample 0 is missed
    // because there is no prior window to compute a positive difference from.
    let mut energies_with_prior = Vec::with_capacity(energies.len() + 1);
    energies_with_prior.push(0.0f32);
    energies_with_prior.extend_from_slice(&energies);

    // First-order energy difference — positive half only (rising edges).
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

    let mut onsets: Vec<f32> = Vec::new();
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

// ---------------------------------------------------------------------------
// Frame timing
// ---------------------------------------------------------------------------

/// Converts onset times (milliseconds) to per-frame durations (milliseconds).
///
/// Each accepted onset becomes one frame. The duration of frame N is the
/// gap to the next onset; the last frame reuses the previous duration
/// (or one frame-period if there is only one beat).
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn onset_times_to_frame_durations(onset_times_ms: &[f32], fps: f32) -> Vec<u32> {
    if onset_times_ms.is_empty() {
        return Vec::new();
    }

    let frame_period_ms = 1000.0 / fps;

    // Snap each onset to the nearest frame boundary.
    let snapped: Vec<u32> = onset_times_ms
        .iter()
        .map(|&t| (t / frame_period_ms).round() as u32)
        .collect();

    // Deduplicate: two onsets that snap to the same frame index merge into
    // one. Doing this before computing durations avoids zero-duration frames.
    let mut unique: Vec<u32> = Vec::with_capacity(snapped.len());
    for f in snapped {
        if unique.last() != Some(&f) {
            unique.push(f);
        }
    }

    if unique.is_empty() {
        return Vec::new();
    }

    // Convert absolute frame indices to inter-frame durations in ms.
    let n = unique.len();
    let mut durations: Vec<u32> = Vec::with_capacity(n);
    for i in 0..n {
        let dur_ms = if i + 1 < n {
            let gap = unique
                .get(i + 1)
                .copied()
                .unwrap_or(0)
                .saturating_sub(unique.get(i).copied().unwrap_or(0));
            (gap as f32 * frame_period_ms).round() as u32
        } else if i > 0 {
            durations
                .get(i - 1)
                .copied()
                .unwrap_or_else(|| frame_period_ms.round() as u32)
        } else {
            frame_period_ms.round() as u32
        };
        durations.push(dur_ms.max(1));
    }

    durations
}

// ---------------------------------------------------------------------------
// Mouth-shape cel generation (lip-sync mode)
// ---------------------------------------------------------------------------

/// Builds placeholder mouth-shape cels for lip-sync mode.
///
/// Frames that coincide with a detected onset get `mouth_state = "open"`;
/// all other frames get `mouth_state = "closed"`. Each cel uses a 1×1
/// RGBA8 pixel buffer: white (0xff) for open, black (0x00) for closed.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn build_mouth_cels(
    onset_times_ms: &[f32],
    fps: f32,
    start_frame: u32,
    frame_count: u32,
    layer_id: LayerId,
) -> (Vec<Cel>, Vec<NewPixelBuffer>) {
    let frame_period_ms = 1000.0 / fps;

    // Onset frames (absolute frame indices).
    let open_set: HashSet<u32> = onset_times_ms
        .iter()
        .map(|&t| start_frame + (t / frame_period_ms).round() as u32)
        .collect();

    let mut cels = Vec::with_capacity(frame_count as usize);
    let mut pixel_buffers = Vec::with_capacity(frame_count as usize);

    for i in 0..frame_count {
        let abs_frame = start_frame + i;
        let is_open = open_set.contains(&abs_frame);
        let buffer_id = PixelBufferId::new(MOUTH_BUF_BASE + i);
        let luma: u8 = if is_open { 0xff } else { 0x00 };

        pixel_buffers.push(NewPixelBuffer {
            placeholder: buffer_id,
            pixels: PixelData::rgba8(1, 1, vec![luma, luma, luma, 0xff]),
        });

        let mut cel = Cel::raster(
            layer_id,
            FrameIndex::new(abs_frame),
            buffer_id,
            Size::new(1, 1),
        );
        cel.user_data.text = Some(if is_open {
            "open".into()
        } else {
            "closed".into()
        });
        cels.push(cel);
    }

    (cels, pixel_buffers)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::runtime::VerbRuntime;
    use pixhaus_core::project::{ProjectMetadata, SpriteId};

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn metadata() -> ProjectMetadata {
        ProjectMetadata {
            name: "audio-timing-test".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0".into(),
        }
    }

    fn ctx_with_sprite() -> crate::plugin::context::VerbContext {
        let mut ctx = crate::plugin::context::VerbContext::empty(metadata());
        ctx.active_sprite = Some(SpriteId::new(1));
        ctx
    }

    /// Builds a minimal 44-byte PCM WAV with `frames` stereo 16-bit samples
    /// at `sample_rate` Hz. Each sample pair alternates between a loud peak
    /// and silence to produce a detectable onset at the very beginning.
    fn minimal_wav_16bit(sample_rate: u32, samples_per_channel: u32) -> Vec<u8> {
        let channels: u16 = 1;
        let bits: u16 = 16;
        let bytes_per_sample = u32::from(bits / 8);
        let data_size = samples_per_channel * u32::from(channels) * bytes_per_sample;
        let file_size = 36 + data_size;

        let mut wav = Vec::with_capacity((file_size + 8) as usize);
        // RIFF header
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&file_size.to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        // fmt chunk
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes()); // chunk size
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        let byte_rate = sample_rate * u32::from(channels) * bytes_per_sample;
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        let block_align = channels * bits / 8;
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&bits.to_le_bytes());
        // data chunk
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());

        // Generate samples: loud at the start, silence after a small gap.
        for i in 0..samples_per_channel {
            let s: i16 = if i < 10 {
                i16::MAX // loud onset
            } else {
                0 // silence
            };
            wav.extend_from_slice(&s.to_le_bytes());
        }
        wav
    }

    // -----------------------------------------------------------------------
    // Validation
    // -----------------------------------------------------------------------

    #[test]
    fn validate_rejects_empty_audio() {
        let verb = AudioTimingVerb::new();
        let inputs = VerbInputs::from_struct(&AudioTimingInputs {
            audio_bytes: vec![],
            format: AudioFormat::Wav,
            mode: AudioTimingMode::Beat,
            fps: 12.0,
            start_frame: 0,
            sensitivity: 0.5,
            tag_name: None,
            layer_name: None,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_rejects_fps_out_of_range() {
        let verb = AudioTimingVerb::new();
        let base = AudioTimingInputs {
            audio_bytes: vec![0u8; 8],
            format: AudioFormat::Wav,
            mode: AudioTimingMode::Beat,
            fps: 0.5,
            start_frame: 0,
            sensitivity: 0.5,
            tag_name: None,
            layer_name: None,
        };
        assert!(
            verb.validate(&VerbInputs::from_struct(&base).unwrap())
                .is_err()
        );
        let over = AudioTimingInputs { fps: 200.0, ..base };
        assert!(
            verb.validate(&VerbInputs::from_struct(&over).unwrap())
                .is_err()
        );
    }

    #[test]
    fn validate_rejects_sensitivity_out_of_range() {
        let verb = AudioTimingVerb::new();
        let inputs = AudioTimingInputs {
            audio_bytes: vec![0u8; 8],
            format: AudioFormat::Wav,
            mode: AudioTimingMode::Beat,
            fps: 12.0,
            start_frame: 0,
            sensitivity: 1.5,
            tag_name: None,
            layer_name: None,
        };
        assert!(
            verb.validate(&VerbInputs::from_struct(&inputs).unwrap())
                .is_err()
        );
    }

    // -----------------------------------------------------------------------
    // Format detection
    // -----------------------------------------------------------------------

    #[test]
    fn detect_format_identifies_wav() {
        let mut bytes = vec![0u8; 16];
        bytes[0..4].copy_from_slice(b"RIFF");
        bytes[8..12].copy_from_slice(b"WAVE");
        assert!(matches!(detect_format(&bytes), AudioFormat::Wav));
    }

    #[test]
    fn detect_format_identifies_mp3_id3() {
        assert!(matches!(detect_format(b"ID3\x00"), AudioFormat::Mp3));
    }

    #[test]
    fn detect_format_identifies_ogg() {
        assert!(matches!(detect_format(b"OggS\x00"), AudioFormat::Ogg));
    }

    #[test]
    fn detect_format_identifies_flac() {
        assert!(matches!(detect_format(b"fLaC\x00"), AudioFormat::Flac));
    }

    #[test]
    fn detect_format_unknown_for_garbage() {
        assert!(matches!(
            detect_format(b"\x00\x01\x02\x03"),
            AudioFormat::Unknown
        ));
    }

    // -----------------------------------------------------------------------
    // WAV parsing
    // -----------------------------------------------------------------------

    #[test]
    fn parse_wav_rejects_too_short() {
        assert!(parse_wav_mono_f32(&[0u8; 8]).is_err());
    }

    #[test]
    fn parse_wav_rejects_wrong_magic() {
        let mut bytes = vec![0u8; 44];
        bytes[0..4].copy_from_slice(b"XXXX");
        assert!(parse_wav_mono_f32(&bytes).is_err());
    }

    #[test]
    fn parse_wav_rejects_non_pcm_format() {
        // Build a WAV with audio format = 3 (IEEE float) instead of 1 (PCM).
        let mut wav = minimal_wav_16bit(44100, 100);
        // audio format field is at offset 20 (after RIFF+size+WAVE+fmt +size = 20).
        wav[20] = 3;
        wav[21] = 0;
        assert!(parse_wav_mono_f32(&wav).is_err());
    }

    #[test]
    fn parse_wav_decodes_mono_16bit() {
        let wav = minimal_wav_16bit(44100, 100);
        let pcm = parse_wav_mono_f32(&wav).unwrap();
        assert_eq!(pcm.sample_rate, 44100);
        assert_eq!(pcm.samples.len(), 100);
        // First 10 samples should be loud (near 1.0 for i16::MAX).
        assert!(pcm.samples[0] > 0.9);
        // Remaining samples should be silent.
        assert_eq!(pcm.samples[10], 0.0);
    }

    // -----------------------------------------------------------------------
    // Beat detection
    // -----------------------------------------------------------------------

    #[test]
    fn detect_onsets_empty_returns_empty() {
        assert!(detect_onsets(&[], 44100, 0.5).is_empty());
    }

    #[test]
    fn detect_onsets_silence_returns_empty() {
        let silence = vec![0.0f32; 44100];
        assert!(detect_onsets(&silence, 44100, 0.5).is_empty());
    }

    #[test]
    fn detect_onsets_single_impulse_returns_one_onset() {
        // One loud sample at position 0, then silence.
        let mut samples = vec![0.0f32; 44100];
        samples[0] = 1.0;
        let onsets = detect_onsets(&samples, 44100, 0.1);
        assert!(!onsets.is_empty(), "expected at least one onset");
        assert!(
            onsets[0] < 100.0,
            "onset should be near start, got {}",
            onsets[0]
        );
    }

    // -----------------------------------------------------------------------
    // Frame timing conversion
    // -----------------------------------------------------------------------

    #[test]
    fn frame_durations_empty_for_empty_onsets() {
        assert!(onset_times_to_frame_durations(&[], 12.0).is_empty());
    }

    #[test]
    fn frame_durations_single_onset() {
        let durations = onset_times_to_frame_durations(&[0.0], 12.0);
        assert_eq!(durations.len(), 1);
        assert!(durations[0] >= 1);
    }

    #[test]
    fn frame_durations_two_onsets_produces_two_frames() {
        // 83 ms apart at 12 fps ≈ 1 frame gap.
        let durations = onset_times_to_frame_durations(&[0.0, 500.0, 1000.0], 12.0);
        assert_eq!(durations.len(), 3);
    }

    #[test]
    fn frame_durations_deduplicates_same_snapped_frame() {
        // Two onsets that both snap to frame 0 at 12 fps should merge.
        let durations = onset_times_to_frame_durations(&[0.0, 1.0], 12.0);
        assert_eq!(durations.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Full verb invocation
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn beat_mode_produces_frames_and_tag() {
        let runtime = VerbRuntime::new();
        runtime.register(AudioTimingVerb::new()).unwrap();

        let wav = minimal_wav_16bit(44100, 44100); // 1 second
        let inputs = VerbInputs::from_struct(&AudioTimingInputs {
            audio_bytes: wav,
            format: AudioFormat::Wav,
            mode: AudioTimingMode::Beat,
            fps: 12.0,
            start_frame: 0,
            sensitivity: 0.1,
            tag_name: Some("Beats".into()),
            layer_name: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(AUDIO_TIMING_VERB_ID),
                ctx_with_sprite(),
                inputs,
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();

        // Must produce at least AddFrames + AddTag.
        assert!(
            preview.output.effects.len() >= 2,
            "expected >= 2 effects, got {}",
            preview.output.effects.len()
        );
        assert!(matches!(
            preview.output.effects.first(),
            Some(VerbEffect::AddFrames { .. })
        ));
        assert!(
            preview
                .output
                .effects
                .iter()
                .any(|e| matches!(e, VerbEffect::AddTag { .. }))
        );
    }

    #[tokio::test]
    async fn lipsync_mode_adds_mouth_layer() {
        let runtime = VerbRuntime::new();
        runtime.register(AudioTimingVerb::new()).unwrap();

        let wav = minimal_wav_16bit(44100, 44100);
        let inputs = VerbInputs::from_struct(&AudioTimingInputs {
            audio_bytes: wav,
            format: AudioFormat::Wav,
            mode: AudioTimingMode::LipSync,
            fps: 12.0,
            start_frame: 0,
            sensitivity: 0.1,
            tag_name: None,
            layer_name: Some("Mouth".into()),
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(AUDIO_TIMING_VERB_ID),
                ctx_with_sprite(),
                inputs,
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();

        let has_mouth_layer = preview
            .output
            .effects
            .iter()
            .any(|e| matches!(e, VerbEffect::AddLayer { layer, .. } if layer.name == "Mouth"));
        assert!(
            has_mouth_layer,
            "expected AddLayer for mouth in LipSync mode"
        );
    }

    #[tokio::test]
    async fn verb_requires_active_sprite() {
        let runtime = VerbRuntime::new();
        runtime.register(AudioTimingVerb::new()).unwrap();

        let wav = minimal_wav_16bit(44100, 100);
        let inputs = VerbInputs::from_struct(&AudioTimingInputs {
            audio_bytes: wav,
            format: AudioFormat::Wav,
            mode: AudioTimingMode::Beat,
            fps: 12.0,
            start_frame: 0,
            sensitivity: 0.5,
            tag_name: None,
            layer_name: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(AUDIO_TIMING_VERB_ID),
                // No active sprite in context.
                crate::plugin::context::VerbContext::empty(metadata()),
                inputs,
            )
            .unwrap();
        let res = inv.finish().await;
        assert!(
            matches!(res, Err(VerbError::MissingContext(_))),
            "expected MissingContext, got {res:?}"
        );
    }

    #[tokio::test]
    async fn verb_rejects_non_wav_format() {
        let runtime = VerbRuntime::new();
        runtime.register(AudioTimingVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&AudioTimingInputs {
            audio_bytes: b"OggS\x00some ogg bytes".to_vec(),
            format: AudioFormat::Unknown, // will be detected as Ogg
            mode: AudioTimingMode::Beat,
            fps: 12.0,
            start_frame: 0,
            sensitivity: 0.5,
            tag_name: None,
            layer_name: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(AUDIO_TIMING_VERB_ID),
                ctx_with_sprite(),
                inputs,
            )
            .unwrap();
        let res = inv.finish().await;
        assert!(
            matches!(res, Err(VerbError::Schema(_))),
            "expected Schema error for Ogg input, got {res:?}"
        );
    }

    #[test]
    fn descriptor_advertises_empty_capabilities() {
        let verb = AudioTimingVerb::new();
        assert!(verb.descriptor().required_capabilities.is_empty());
        assert!(verb.descriptor().cancellable);
        assert!(verb.descriptor().streaming);
    }
}
