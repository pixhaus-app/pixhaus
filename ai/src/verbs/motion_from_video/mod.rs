//! Verb: Motion-from-video (S32).
//!
//! Extracts a timing skeleton and pose reference layers from a
//! reference video supplied as a sequence of RGBA8 frames.
//!
//! # What the verb produces
//!
//! - **N new frames** whose durations come from the video timestamps.
//!   Only keyframes are added — frames where motion crosses the
//!   sensitivity threshold — so a 30fps, 2-second clip yields far
//!   fewer than 60 frames.
//! - **One frame tag** (`"Motion"` by default) covering the new frames.
//! - **One "Pose reference" layer** containing a silhouette thumbnail
//!   for each keyframe. The artist traces the silhouettes when drawing
//!   the actual pixel art frames.
//!
//! The verb runs entirely on the local CPU; no inference backend is
//! required. Pose estimation via a `POSE_ESTIMATION` backend (when
//! available from S22) is the planned upgrade path and will replace the
//! classical pixel-difference analysis in a future stream.
//!
//! # Protocol notes
//!
//! The output emits three effects in this order:
//!
//! 1. `AddLayer` — creates the "Pose reference" layer with placeholder
//!    `LayerId(0)`. No cels here.
//! 2. `AddFrames` — inserts the keyframe slots. Cels on the pose layer
//!    use `LayerId(0)` as a cross-effect placeholder; the host rewrites
//!    it to the real `LayerId` assigned by effect 1 on commit.
//!    Frame indices in cels are relative (0 = first new frame).
//! 3. `AddTag` — names the inserted range. Uses absolute frame indices
//!    computed from `VerbContext::active_frame` (or the last frame of
//!    the sprite when none is active).

use std::time::{Duration, Instant};

use async_trait::async_trait;
use pixhaus_core::project::{
    Cel, Frame, FrameIndex, FrameRange, FrameTag, Layer, LayerId, LoopDirection, PixelBufferId,
    Size, UserData,
};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::plugin::{
    context::{PixelData, VerbContext},
    descriptor::{BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId},
    error::{Result, VerbError},
    inputs::VerbInputs,
    output::{ActualCost, NewPixelBuffer, VerbEffect, VerbOutput},
    progress::{VerbProgress, VerbProgressEvent},
    verb::Verb,
};

/// Stable identifier for the built-in motion-from-video verb.
pub const MOTION_FROM_VIDEO_VERB_ID: &str = "pixhaus.builtin.motion_from_video";

/// Placeholder layer ID used across `AddLayer` and `AddFrames` effects.
/// The host rewrites this to the real `LayerId` on commit.
const POSE_LAYER_PLACEHOLDER: u32 = 0;

/// Default keyframe sensitivity: keeps roughly 25 % of the most
/// motion-heavy frames plus the first and last.
const DEFAULT_SENSITIVITY: f32 = 0.25;

/// One frame of the reference video.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VideoFrame {
    /// RGBA8 pixel data. All frames in a single input must share the
    /// same `width`, `height`, and `bytes_per_pixel`.
    pub pixels: PixelData,
    /// Presentation timestamp in milliseconds from the video start.
    /// Must be strictly increasing within the sequence.
    pub timestamp_ms: u32,
}

/// Inputs for [`MotionFromVideoVerb`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MotionFromVideoInputs {
    /// The video as a sequence of frames. Minimum 2.
    pub frames: Vec<VideoFrame>,
    /// Name for the resulting frame tag. Defaults to `"Motion"`.
    #[serde(default)]
    pub tag_name: Option<String>,
    /// Keyframe detection sensitivity in `0.0..=1.0`.
    ///
    /// Controls the threshold at which a frame is considered a
    /// keyframe based on its motion magnitude relative to the maximum
    /// motion observed in the sequence:
    ///
    /// - `0.0` — every frame is a keyframe.
    /// - `0.25` (default) — frames with at least 25 % of the maximum
    ///   observed motion are kept.
    /// - `1.0` — only the frame with the highest motion is kept
    ///   (plus the mandatory first and last frames).
    #[serde(default)]
    pub keyframe_sensitivity: Option<f32>,
}

/// Verb: extract a motion timing skeleton from a reference video.
///
/// See the module-level documentation for the full description of what
/// the verb produces and how the output effects interact.
#[derive(Debug)]
pub struct MotionFromVideoVerb {
    descriptor: VerbDescriptor,
}

impl MotionFromVideoVerb {
    /// Constructs the verb with its descriptor.
    #[must_use]
    // `serde_json::json!` calls `Result::unwrap` on infallible internal
    // builders; the workspace forbids `unwrap` globally, so we exempt
    // the constructor here rather than open-coding the schema by hand.
    #[allow(clippy::disallowed_methods)]
    pub fn new() -> Self {
        let input_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "frames": {
                    "type": "array",
                    "minItems": 2,
                    "items": {
                        "type": "object",
                        "properties": {
                            "pixels": {
                                "type": "object",
                                "properties": {
                                    "width":  { "type": "integer", "minimum": 1 },
                                    "height": { "type": "integer", "minimum": 1 },
                                    "bytes_per_pixel": { "type": "integer" },
                                    "stride": { "type": "integer" },
                                    "bytes": {
                                        "type": "array",
                                        "items": { "type": "integer" }
                                    }
                                },
                                "required": [
                                    "width", "height",
                                    "bytes_per_pixel", "stride", "bytes"
                                ]
                            },
                            "timestamp_ms": {
                                "type": "integer",
                                "minimum": 0
                            }
                        },
                        "required": ["pixels", "timestamp_ms"]
                    }
                },
                "tag_name": { "type": ["string", "null"] },
                "keyframe_sensitivity": {
                    "type": ["number", "null"],
                    "minimum": 0.0,
                    "maximum": 1.0
                }
            },
            "required": ["frames"]
        });

        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(MOTION_FROM_VIDEO_VERB_ID),
                display_name: "Motion from video".into(),
                description: concat!(
                    "Extract a timing skeleton and pose reference layers ",
                    "from a reference video"
                )
                .into(),
                version: env!("CARGO_PKG_VERSION").into(),
                required_capabilities: BackendCapabilities::empty(),
                input_schema,
                output_schema: None,
                output_kinds: vec![
                    EffectKind::AddLayer,
                    EffectKind::AddFrames,
                    EffectKind::AddTag,
                ],
                cost_estimate: CostEstimate::free(),
                streaming: true,
                cancellable: true,
                documentation_url: Some("docs/verbs/motion_from_video.md".into()),
            },
        }
    }
}

impl Default for MotionFromVideoVerb {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::too_many_lines)]
#[async_trait]
impl Verb for MotionFromVideoVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        let parsed: MotionFromVideoInputs = inputs.deserialize()?;

        if parsed.frames.len() < 2 {
            return Err(VerbError::Schema(
                "motion_from_video: at least 2 frames are required".into(),
            ));
        }

        let first = &parsed.frames[0].pixels;
        if !first.is_well_formed() {
            return Err(VerbError::Schema(
                "motion_from_video: frame 0 pixel data is malformed".into(),
            ));
        }

        for (i, vf) in parsed.frames.iter().enumerate().skip(1) {
            if !vf.pixels.is_well_formed() {
                return Err(VerbError::Schema(format!(
                    "motion_from_video: frame {i} pixel data is malformed"
                )));
            }
            if vf.pixels.width != first.width || vf.pixels.height != first.height {
                return Err(VerbError::Schema(format!(
                    "motion_from_video: frame {i} dimensions \
                     ({}x{}) differ from frame 0 ({}x{})",
                    vf.pixels.width, vf.pixels.height, first.width, first.height,
                )));
            }
            if vf.timestamp_ms <= parsed.frames[i - 1].timestamp_ms {
                return Err(VerbError::Schema(format!(
                    "motion_from_video: timestamps must be strictly increasing \
                     (frames {} and {} have timestamps {} and {})",
                    i - 1,
                    i,
                    parsed.frames[i - 1].timestamp_ms,
                    vf.timestamp_ms,
                )));
            }
        }

        if let Some(s) = parsed.keyframe_sensitivity {
            if !(0.0..=1.0).contains(&s) {
                return Err(VerbError::Schema(
                    "motion_from_video: keyframe_sensitivity must be in 0.0..=1.0".into(),
                ));
            }
        }

        Ok(())
    }

    async fn invoke(
        &self,
        ctx: VerbContext,
        inputs: VerbInputs,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<VerbOutput> {
        let started = Instant::now();
        // Consume the payload: frames carry large pixel byte vectors that
        // would otherwise be cloned by the borrowed deserialiser.
        let inputs: MotionFromVideoInputs = inputs.deserialize_owned()?;
        let sprite_id = ctx.require_sprite_id()?;

        let sensitivity = inputs.keyframe_sensitivity.unwrap_or(DEFAULT_SENSITIVITY);
        let tag_name = inputs.tag_name.clone().unwrap_or_else(|| "Motion".into());
        let n_source = inputs.frames.len();

        progress
            .send(VerbProgressEvent::Started { backend: None })
            .await;
        progress.step(Some(0.1), "analysing motion").await;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        let keyframe_indices = detect_keyframes(&inputs.frames, sensitivity);
        let n_keyframes = keyframe_indices.len();
        let n_keyframes_u32 = u32::try_from(n_keyframes)
            .map_err(|_| VerbError::Schema("keyframe count exceeds u32 maximum".into()))?;

        progress.step(Some(0.4), "extracting silhouettes").await;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        let frame_w = inputs.frames[0].pixels.width;
        let frame_h = inputs.frames[0].pixels.height;
        let cel_size = Size::new(frame_w, frame_h);
        let pose_layer_id = LayerId::new(POSE_LAYER_PLACEHOLDER);

        let mut pixel_buffers: Vec<NewPixelBuffer> = Vec::with_capacity(n_keyframes);
        let mut cels: Vec<Cel> = Vec::with_capacity(n_keyframes);

        for (slot, &src_idx) in keyframe_indices.iter().enumerate() {
            let slot_u32 = u32::try_from(slot)
                .map_err(|_| VerbError::Schema("keyframe index exceeds u32 maximum".into()))?;
            let silhouette = to_silhouette(&inputs.frames[src_idx].pixels);
            let buf_id = PixelBufferId::new(slot_u32);
            // frame_index is relative to the first new frame (0 = first new
            // frame); the host renumbers to absolute on commit.
            cels.push(Cel::raster(
                pose_layer_id,
                FrameIndex::new(slot_u32),
                buf_id,
                cel_size,
            ));
            pixel_buffers.push(NewPixelBuffer {
                placeholder: buf_id,
                pixels: silhouette,
            });
        }

        progress.step(Some(0.7), "building timeline frames").await;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        let frames = keyframe_durations(&inputs.frames, &keyframe_indices);

        // Determine the insertion point. Use the active frame when available,
        // else append after the sprite's last existing frame.
        let insert_after = ctx.active_frame.unwrap_or_else(|| {
            let last = ctx
                .sprite
                .as_ref()
                .map_or(0, |s| s.frames.len().saturating_sub(1));
            FrameIndex::new(
                // Sprite frame counts are bounded by the project model, which
                // stores them in a Vec whose length fits in usize. On all
                // supported 64-bit targets usize ≥ u32, so capping at u32::MAX
                // is an unreachable safety valve rather than real logic.
                u32::try_from(last).unwrap_or(u32::MAX),
            )
        });

        let tag_start = FrameIndex::new(insert_after.get() + 1);
        let tag_end = FrameIndex::new(insert_after.get() + n_keyframes_u32);

        progress.step(Some(1.0), "done").await;

        let elapsed = started.elapsed();
        let summary = format!(
            "Add {n_keyframes} keyframe{} tagged \"{tag_name}\" \
             ({n_source} source frame{} analysed)",
            if n_keyframes == 1 { "" } else { "s" },
            if n_source == 1 { "" } else { "s" },
        );

        Ok(VerbOutput {
            summary,
            effects: vec![
                // Effect 1: create the pose reference layer.
                // No cels here — cels live in AddFrames so they are
                // created alongside the frames they reference.
                VerbEffect::AddLayer {
                    sprite: sprite_id,
                    layer: Layer::raster(pose_layer_id, "Pose reference"),
                    cels: vec![],
                    pixel_buffers: vec![],
                },
                // Effect 2: insert keyframe slots and attach silhouette
                // cels. LayerId(POSE_LAYER_PLACEHOLDER) in each cel is a
                // cross-effect reference to the layer created above; the
                // host rewrites it to the real LayerId on commit.
                VerbEffect::AddFrames {
                    sprite: sprite_id,
                    after: Some(insert_after),
                    frames,
                    cels,
                    pixel_buffers,
                },
                // Effect 3: tag the new frame range.
                VerbEffect::AddTag {
                    sprite: sprite_id,
                    tag: FrameTag {
                        name: tag_name,
                        range: FrameRange::new(tag_start, tag_end),
                        loop_direction: LoopDirection::Forward,
                        repeat: 0,
                        user_data: UserData::default(),
                    },
                },
            ],
            thumbnail: None,
            actual_cost: ActualCost::free(elapsed.max(Duration::from_micros(1))),
            notes: vec![
                "Pose reference layer contains silhouette thumbnails to trace; \
                 add pixel art for each frame manually."
                    .into(),
            ],
        })
    }
}

// ---------------------------------------------------------------------------
// Analysis helpers
// ---------------------------------------------------------------------------

/// Returns the normalised mean absolute pixel difference between two RGBA8
/// frames, in the range `0.0..=1.0`.
///
/// Pixels where both frames are substantially transparent (combined alpha
/// < 64) are excluded from the average so off-canvas padding does not
/// inflate the motion reading.
///
/// Returns `0.0` on dimension mismatch, zero-area images, or non-RGBA8
/// input (any `bytes_per_pixel != 4`).
fn motion_magnitude(a: &PixelData, b: &PixelData) -> f32 {
    if a.width != b.width
        || a.height != b.height
        || a.bytes_per_pixel != 4
        || b.bytes_per_pixel != 4
        || a.width == 0
    {
        return 0.0;
    }

    let row_pixels = a.width as usize * 4;
    let stride_a = a.stride as usize;
    let stride_b = b.stride as usize;

    let mut sum = 0.0_f64;
    let mut counted = 0.0_f64;

    for (row_a, row_b) in a
        .bytes
        .chunks_exact(stride_a)
        .zip(b.bytes.chunks_exact(stride_b))
    {
        for (pa, pb) in row_a[..row_pixels]
            .chunks_exact(4)
            .zip(row_b[..row_pixels].chunks_exact(4))
        {
            if (u32::from(pa[3]) + u32::from(pb[3])) < 64 {
                continue;
            }
            sum += (f64::from(pa[0]) - f64::from(pb[0])).abs()
                + (f64::from(pa[1]) - f64::from(pb[1])).abs()
                + (f64::from(pa[2]) - f64::from(pb[2])).abs();
            counted += 1.0;
        }
    }

    if counted == 0.0 {
        return 0.0;
    }

    // The result is always in 0.0..=1.0; f32 precision is sufficient
    // for a motion magnitude ratio. Truncation from f64 to f32 is
    // intentional: the value is a ratio in [0.0, 1.0] and single
    // precision is more than adequate.
    #[allow(clippy::cast_possible_truncation)]
    let result = (sum / (counted * 765.0)) as f32;
    result
}

/// Selects keyframe indices from a video frame sequence.
///
/// Always includes index `0` and index `frames.len() - 1`. Additional
/// frames are included when their motion magnitude (compared to the
/// previous frame) meets or exceeds `sensitivity * max_magnitude`.
///
/// A `sensitivity` of `0.0` keeps every frame; `1.0` keeps only the
/// frame(s) with the highest motion (plus the mandatory endpoints).
/// When all frames are identical (`max_magnitude == 0`), only the
/// endpoints are returned.
fn detect_keyframes(frames: &[VideoFrame], sensitivity: f32) -> Vec<usize> {
    let n = frames.len();
    if n <= 2 {
        return (0..n).collect();
    }

    let magnitudes: Vec<f32> = (1..n)
        .map(|i| motion_magnitude(&frames[i - 1].pixels, &frames[i].pixels))
        .collect();

    let max_mag = magnitudes.iter().copied().fold(0.0_f32, f32::max);

    // When there is no motion at all, the timing skeleton is trivial:
    // only the first and last frames matter.
    if max_mag < f32::EPSILON {
        return vec![0, n - 1];
    }

    let threshold = max_mag * sensitivity.clamp(0.0, 1.0);

    let mut keyframes = vec![0usize];
    for i in 1..n {
        if magnitudes[i - 1] >= threshold {
            keyframes.push(i);
        }
    }
    let last = n - 1;
    if keyframes.last().copied() != Some(last) {
        keyframes.push(last);
    }
    keyframes
}

/// Computes per-keyframe durations from the source video timestamps.
///
/// Each keyframe's duration is the elapsed time until the next keyframe.
/// The last keyframe receives the median of all other durations (or
/// 100 ms when there is only one keyframe).
fn keyframe_durations(frames: &[VideoFrame], keyframe_indices: &[usize]) -> Vec<Frame> {
    let n = keyframe_indices.len();
    if n == 0 {
        return vec![];
    }

    let mut result: Vec<Frame> = Vec::with_capacity(n);

    for i in 0..n.saturating_sub(1) {
        let cur = frames[keyframe_indices[i]].timestamp_ms;
        let next = frames[keyframe_indices[i + 1]].timestamp_ms;
        let dur = next.saturating_sub(cur).max(1);
        result.push(Frame {
            duration_ms: dur,
            user_data: UserData::default(),
        });
    }

    // Last keyframe: use the median of all preceding durations, or 100 ms.
    let last_dur = if result.is_empty() {
        100
    } else {
        let mut ds: Vec<u32> = result.iter().map(|f| f.duration_ms).collect();
        ds.sort_unstable();
        ds[ds.len() / 2]
    };
    result.push(Frame {
        duration_ms: last_dur,
        user_data: UserData::default(),
    });

    result
}

/// Converts an RGBA8 frame to a monochrome silhouette for use as a
/// tracing reference.
///
/// Non-transparent pixels are darkened and given a faint blue tint so
/// the reference layer is visually distinct from the artist's own work.
/// Transparent pixels are preserved as transparent. Non-RGBA8 input is
/// returned unchanged.
fn to_silhouette(pixels: &PixelData) -> PixelData {
    if pixels.bytes_per_pixel != 4 {
        return pixels.clone();
    }

    let w = pixels.width as usize;
    let row_pixels = w * 4;
    let stride = pixels.stride as usize;
    let mut out = vec![0u8; w * 4 * pixels.height as usize];

    for (row_in, row_out) in pixels
        .bytes
        .chunks_exact(stride)
        .zip(out.chunks_exact_mut(w * 4))
    {
        for (p_in, p_out) in row_in[..row_pixels]
            .chunks_exact(4)
            .zip(row_out.chunks_exact_mut(4))
        {
            if p_in[3] < 32 {
                p_out.copy_from_slice(&[0, 0, 0, 0]);
            } else {
                // Rec. 601 luminance. The maximum possible value is
                // (255 * 299 + 255 * 587 + 255 * 114) / 1000 = 255,
                // so the u32 → u8 cast is safe.
                #[allow(clippy::cast_possible_truncation)]
                let lum = ((u32::from(p_in[0]) * 299
                    + u32::from(p_in[1]) * 587
                    + u32::from(p_in[2]) * 114)
                    / 1000) as u8;
                // Darken to a quarter and add a slight blue tint to
                // distinguish the reference layer from normal layers.
                let v = lum / 4;
                // 255 * 3 / 4 = 191, which fits in u8.
                #[allow(clippy::cast_possible_truncation)]
                let alpha = (u32::from(p_in[3]) * 3 / 4) as u8;
                p_out[0] = v;
                p_out[1] = v;
                p_out[2] = v.saturating_add(20);
                p_out[3] = alpha;
            }
        }
    }

    PixelData::rgba8(pixels.width, pixels.height, out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::runtime::VerbRuntime;
    use pixhaus_core::project::{ProjectMetadata, SpriteId};

    fn metadata() -> ProjectMetadata {
        ProjectMetadata {
            name: "mfv-test".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0".into(),
        }
    }

    fn ctx_with_sprite() -> VerbContext {
        let mut ctx = VerbContext::empty(metadata());
        ctx.active_sprite = Some(SpriteId::new(1));
        ctx.active_frame = Some(FrameIndex::new(0));
        ctx
    }

    fn solid_frame(w: u32, h: u32, rgba: [u8; 4], ts: u32) -> VideoFrame {
        let bytes = rgba
            .iter()
            .copied()
            .cycle()
            .take((w * h * 4) as usize)
            .collect();
        VideoFrame {
            pixels: PixelData::rgba8(w, h, bytes),
            timestamp_ms: ts,
        }
    }

    fn two_frames() -> Vec<VideoFrame> {
        vec![
            solid_frame(4, 4, [200, 100, 50, 255], 0),
            solid_frame(4, 4, [50, 200, 100, 255], 100),
        ]
    }

    // --- descriptor ---

    #[test]
    fn descriptor_id_is_stable() {
        let v = MotionFromVideoVerb::new();
        assert_eq!(v.descriptor().id.as_str(), MOTION_FROM_VIDEO_VERB_ID);
    }

    #[test]
    fn descriptor_requires_no_backend() {
        let v = MotionFromVideoVerb::new();
        assert!(v.descriptor().required_capabilities.is_empty());
    }

    #[test]
    fn descriptor_output_kinds_correct() {
        let v = MotionFromVideoVerb::new();
        let kinds = &v.descriptor().output_kinds;
        assert!(kinds.contains(&EffectKind::AddLayer));
        assert!(kinds.contains(&EffectKind::AddFrames));
        assert!(kinds.contains(&EffectKind::AddTag));
    }

    // --- validation ---

    #[test]
    fn validate_rejects_single_frame() {
        let v = MotionFromVideoVerb::new();
        let inputs = VerbInputs::from_struct(&MotionFromVideoInputs {
            frames: vec![solid_frame(4, 4, [0, 0, 0, 255], 0)],
            tag_name: None,
            keyframe_sensitivity: None,
        })
        .unwrap();
        assert!(v.validate(&inputs).is_err());
    }

    #[test]
    fn validate_rejects_mismatched_dimensions() {
        let v = MotionFromVideoVerb::new();
        let inputs = VerbInputs::from_struct(&MotionFromVideoInputs {
            frames: vec![
                solid_frame(4, 4, [0, 0, 0, 255], 0),
                solid_frame(8, 4, [0, 0, 0, 255], 100),
            ],
            tag_name: None,
            keyframe_sensitivity: None,
        })
        .unwrap();
        assert!(v.validate(&inputs).is_err());
    }

    #[test]
    fn validate_rejects_non_monotonic_timestamps() {
        let v = MotionFromVideoVerb::new();
        let inputs = VerbInputs::from_struct(&MotionFromVideoInputs {
            frames: vec![
                solid_frame(4, 4, [0, 0, 0, 255], 100),
                solid_frame(4, 4, [0, 0, 0, 255], 50),
            ],
            tag_name: None,
            keyframe_sensitivity: None,
        })
        .unwrap();
        assert!(v.validate(&inputs).is_err());
    }

    #[test]
    fn validate_rejects_equal_timestamps() {
        let v = MotionFromVideoVerb::new();
        let inputs = VerbInputs::from_struct(&MotionFromVideoInputs {
            frames: vec![
                solid_frame(4, 4, [0, 0, 0, 255], 0),
                solid_frame(4, 4, [0, 0, 0, 255], 0),
            ],
            tag_name: None,
            keyframe_sensitivity: None,
        })
        .unwrap();
        assert!(v.validate(&inputs).is_err());
    }

    #[test]
    fn validate_rejects_sensitivity_out_of_range() {
        let v = MotionFromVideoVerb::new();
        let inputs = VerbInputs::from_struct(&MotionFromVideoInputs {
            frames: two_frames(),
            tag_name: None,
            keyframe_sensitivity: Some(1.5),
        })
        .unwrap();
        assert!(v.validate(&inputs).is_err());
    }

    #[test]
    fn validate_accepts_valid_input() {
        let v = MotionFromVideoVerb::new();
        let inputs = VerbInputs::from_struct(&MotionFromVideoInputs {
            frames: two_frames(),
            tag_name: Some("walk".into()),
            keyframe_sensitivity: Some(0.5),
        })
        .unwrap();
        assert!(v.validate(&inputs).is_ok());
    }

    // --- keyframe detection ---

    #[test]
    fn detect_keyframes_always_includes_endpoints() {
        // i: u8 so i.saturating_mul(25) never overflows (max 9 × 25 = 225).
        let frames: Vec<VideoFrame> = (0u8..10)
            .map(|i| solid_frame(4, 4, [i.saturating_mul(25), 0, 0, 255], u32::from(i) * 100))
            .collect();
        let kf = detect_keyframes(&frames, 0.5);
        assert_eq!(kf[0], 0);
        assert_eq!(*kf.last().unwrap(), 9);
    }

    #[test]
    fn detect_keyframes_two_frames_returns_both() {
        let frames = two_frames();
        let kf = detect_keyframes(&frames, 0.5);
        assert_eq!(kf, vec![0, 1]);
    }

    #[test]
    fn detect_keyframes_identical_frames_returns_endpoints_only() {
        let frames: Vec<VideoFrame> = (0u32..5)
            .map(|i| solid_frame(4, 4, [100, 100, 100, 255], i * 100))
            .collect();
        let kf = detect_keyframes(&frames, 0.5);
        assert_eq!(kf, vec![0, 4]);
    }

    #[test]
    fn detect_keyframes_zero_sensitivity_keeps_all() {
        // i: u8, i.saturating_mul(50) max is 4 × 50 = 200 — fits in u8.
        let frames: Vec<VideoFrame> = (0u8..5)
            .map(|i| solid_frame(4, 4, [i.saturating_mul(50), 0, 0, 255], u32::from(i) * 100))
            .collect();
        let kf = detect_keyframes(&frames, 0.0);
        assert_eq!(kf.len(), 5);
    }

    // --- motion magnitude ---

    #[test]
    fn motion_magnitude_identical_frames_is_zero() {
        let a = solid_frame(4, 4, [128, 64, 32, 255], 0);
        let b = solid_frame(4, 4, [128, 64, 32, 255], 0);
        assert!(motion_magnitude(&a.pixels, &b.pixels) < f32::EPSILON);
    }

    #[test]
    fn motion_magnitude_different_frames_nonzero() {
        let a = solid_frame(4, 4, [0, 0, 0, 255], 0);
        let b = solid_frame(4, 4, [255, 255, 255, 255], 0);
        let mag = motion_magnitude(&a.pixels, &b.pixels);
        assert!(mag > 0.9);
    }

    #[test]
    fn motion_magnitude_both_transparent_skipped() {
        let a = solid_frame(4, 4, [0, 0, 0, 0], 0);
        let b = solid_frame(4, 4, [255, 255, 255, 0], 0);
        // Both frames fully transparent — no pixels counted, result is 0.
        assert!(motion_magnitude(&a.pixels, &b.pixels) < f32::EPSILON);
    }

    #[test]
    fn motion_magnitude_dimension_mismatch_is_zero() {
        let a = solid_frame(4, 4, [0, 0, 0, 255], 0);
        let b = solid_frame(8, 4, [255, 0, 0, 255], 0);
        assert!(motion_magnitude(&a.pixels, &b.pixels) < f32::EPSILON);
    }

    // --- silhouette conversion ---

    #[test]
    fn silhouette_preserves_transparency() {
        let frame = solid_frame(2, 2, [255, 0, 0, 0], 0); // fully transparent
        let result = to_silhouette(&frame.pixels);
        assert!(result.bytes.iter().all(|&b| b == 0));
    }

    #[test]
    fn silhouette_darkens_opaque_pixels() {
        let frame = solid_frame(2, 2, [200, 200, 200, 255], 0);
        let result = to_silhouette(&frame.pixels);
        // Output alpha should be lower than input alpha (3/4 of 255 ≈ 191).
        for p in result.bytes.chunks_exact(4) {
            assert!(p[3] < 255);
            // RGB values should be much lower than input 200.
            assert!(p[0] < 100);
        }
    }

    #[test]
    fn silhouette_output_is_tightly_packed() {
        let frame = solid_frame(4, 4, [128, 64, 32, 255], 0);
        let result = to_silhouette(&frame.pixels);
        assert_eq!(result.stride, frame.pixels.width.saturating_mul(4));
        assert!(result.is_well_formed());
    }

    // --- keyframe durations ---

    #[test]
    fn keyframe_durations_two_frames() {
        let frames = vec![
            solid_frame(2, 2, [0, 0, 0, 255], 0),
            solid_frame(2, 2, [0, 0, 0, 255], 200),
        ];
        let indices = vec![0, 1];
        let durations = keyframe_durations(&frames, &indices);
        assert_eq!(durations.len(), 2);
        assert_eq!(durations[0].duration_ms, 200);
        // Last frame gets median duration (200 here).
        assert_eq!(durations[1].duration_ms, 200);
    }

    #[test]
    fn keyframe_durations_empty_returns_empty() {
        let frames: Vec<VideoFrame> = vec![];
        let durations = keyframe_durations(&frames, &[]);
        assert!(durations.is_empty());
    }

    // --- end-to-end ---

    #[tokio::test]
    async fn invoke_produces_three_effects() {
        let runtime = VerbRuntime::new();
        runtime.register(MotionFromVideoVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&MotionFromVideoInputs {
            frames: two_frames(),
            tag_name: Some("walk".into()),
            keyframe_sensitivity: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(MOTION_FROM_VIDEO_VERB_ID),
                ctx_with_sprite(),
                inputs,
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();
        assert_eq!(preview.output.effects.len(), 3);
    }

    #[tokio::test]
    async fn invoke_add_layer_is_first_effect() {
        let runtime = VerbRuntime::new();
        runtime.register(MotionFromVideoVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&MotionFromVideoInputs {
            frames: two_frames(),
            tag_name: None,
            keyframe_sensitivity: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(MOTION_FROM_VIDEO_VERB_ID),
                ctx_with_sprite(),
                inputs,
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();

        match &preview.output.effects[0] {
            VerbEffect::AddLayer { layer, cels, .. } => {
                assert_eq!(layer.name, "Pose reference");
                assert!(cels.is_empty(), "pose layer cels belong in AddFrames");
            }
            other => panic!("expected AddLayer, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn invoke_add_frames_carries_silhouette_cels() {
        let runtime = VerbRuntime::new();
        runtime.register(MotionFromVideoVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&MotionFromVideoInputs {
            frames: two_frames(),
            tag_name: None,
            keyframe_sensitivity: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(MOTION_FROM_VIDEO_VERB_ID),
                ctx_with_sprite(),
                inputs,
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();

        match &preview.output.effects[1] {
            VerbEffect::AddFrames {
                frames,
                cels,
                pixel_buffers,
                ..
            } => {
                assert_eq!(frames.len(), cels.len());
                assert_eq!(frames.len(), pixel_buffers.len());
                assert!(!frames.is_empty());
            }
            other => panic!("expected AddFrames, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn invoke_add_tag_names_motion_range() {
        let runtime = VerbRuntime::new();
        runtime.register(MotionFromVideoVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&MotionFromVideoInputs {
            frames: two_frames(),
            tag_name: Some("run".into()),
            keyframe_sensitivity: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(MOTION_FROM_VIDEO_VERB_ID),
                ctx_with_sprite(),
                inputs,
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();

        match &preview.output.effects[2] {
            VerbEffect::AddTag { tag, .. } => {
                assert_eq!(tag.name, "run");
                assert_eq!(tag.loop_direction, LoopDirection::Forward);
            }
            other => panic!("expected AddTag, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn invoke_requires_active_sprite() {
        let runtime = VerbRuntime::new();
        runtime.register(MotionFromVideoVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&MotionFromVideoInputs {
            frames: two_frames(),
            tag_name: None,
            keyframe_sensitivity: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(MOTION_FROM_VIDEO_VERB_ID),
                VerbContext::empty(metadata()),
                inputs,
            )
            .unwrap();
        let res = inv.finish().await;
        assert!(matches!(
            res,
            Err(VerbError::MissingContext("active sprite"))
        ));
    }

    #[tokio::test]
    async fn invoke_honours_cancellation() {
        let runtime = VerbRuntime::new();
        runtime.register(MotionFromVideoVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&MotionFromVideoInputs {
            frames: two_frames(),
            tag_name: None,
            keyframe_sensitivity: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(MOTION_FROM_VIDEO_VERB_ID),
                ctx_with_sprite(),
                inputs,
            )
            .unwrap();

        // Cancel immediately before the task picks it up.
        inv.cancel();
        let res = inv.finish().await;
        assert!(matches!(res, Err(VerbError::Cancelled)));
    }
}
