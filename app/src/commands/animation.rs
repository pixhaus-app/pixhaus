//! Animation studio IPC commands.
//!
//! The studio (see `docs/planning/work/animation-generation-ui.md`) drives
//! AI animation generation: grid generation runs through the existing
//! `verb_invoke` path; the steps that need host-side orchestration live here:
//!
//! - [`animation_normalize`] — runs the core normalization pass over a set of
//!   frames and returns the locked frames plus the drift / scale / seam
//!   report the normalization review surfaces.
//! - [`animation_pick_frames`] — decodes an i2v clip (ffmpeg, via the `io`
//!   crate), auto-detects loop markers, and frame-picks an even loop for the
//!   walk-cycle picker.
//! - [`animation_integrate`] — lands a set of frames as a layer, cels, frames,
//!   a tag, and an engine animation in one undoable step. Handles east-as-flip
//!   and stamps the anchor hash for staleness tracking.
//! - [`animation_set`] — the per-direction / per-type coverage and staleness
//!   view for the directional cascade.

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio_util::sync::CancellationToken;

use pixhaus_ai::backends::{
    BackendProxy, BackgroundRemovalRequest, ImageEditRequest, ImageGenRequest, ImageQuality,
    ImageToVideoRequest, InferenceRequest, InferenceResponse,
};
use pixhaus_ai::plugin::context::PixelData;
use pixhaus_ai::plugin::descriptor::{BackendCapabilities, VerbId};
use pixhaus_ai::plugin::inputs::VerbInputs;
use pixhaus_ai::plugin::output::VerbEffect;
use pixhaus_ai::plugin::progress::VerbProgress;
use pixhaus_ai::verbs::motion_from_video::{
    LoopMarkers, VideoFrame, auto_loop_markers, pick_loop_frames, seam_similarity,
};
use pixhaus_ai::verbs::variant::{VARIANT_VERB_ID, VariantInputs, VariantMode};
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::{
    Animation, AnimationId, Cel, EntityContent, EntityId, Frame, FrameIndex, FrameRange, FrameTag,
    Layer, LayerId, LoopDirection, PixelBufferId, ReferenceImage, SheetVariant, SheetVariantId,
    SpriteId, UserData, VariantOrigin,
};
use pixhaus_core::transforms::flip::flip_horizontal;
use pixhaus_core::transforms::normalize::{
    ChromaKey, NormalizeOptions, NormalizeReport, normalize_frames,
};
use pixhaus_io::animated::{DecodeOptions, decode_video};
use pixhaus_io::pixhaus::PixelBufferEntry;

use crate::animation_jobs::AnimationJob;
use crate::commands::verbs::{AnchorKind, build_verb_context, resolve_anchor_kind};
use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

/// A tightly-packed RGBA8 frame crossing the IPC boundary.
///
/// The UI sends candidate frames in and receives normalized / picked frames
/// out in this shape. `pixels.len() == width * height * 4`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RgbaFrame {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Tightly-packed RGBA8 bytes.
    pub pixels: Vec<u8>,
}

impl RgbaFrame {
    fn to_buffer(&self) -> CommandResult<PixelBuffer> {
        PixelBuffer::from_raw(self.width, self.height, self.width * 4, self.pixels.clone()).map_err(
            |e| AppCommandError::Validation {
                detail: format!("invalid frame: {e}"),
            },
        )
    }

    fn from_buffer(buf: &PixelBuffer) -> Self {
        // Repack to a tight stride so the UI can drop the bytes straight into
        // an ImageData / texture without stride math.
        let w = buf.width();
        let h = buf.height();
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            if let Some(row) = buf.row(y) {
                pixels.extend_from_slice(row);
            } else {
                pixels.extend(std::iter::repeat_n(0u8, (w * 4) as usize));
            }
        }
        Self {
            width: w,
            height: h,
            pixels,
        }
    }
}

// ── Normalize ────────────────────────────────────────────────────────────────

/// Options the studio passes to [`animation_normalize`].
#[derive(Clone, Debug, Deserialize)]
pub struct NormalizeArgs {
    /// Frames to normalize, in order.
    pub frames: Vec<RgbaFrame>,
    /// Output canvas side (square). Defaults to the first frame's width.
    #[serde(default)]
    pub canvas_size: Option<u32>,
    /// Optional chroma key: `"magenta"`, `"green"`, or `"none"`.
    #[serde(default)]
    pub chroma: Option<String>,
    /// Reference visible height; `None` uses the tallest subject.
    #[serde(default)]
    pub reference_height: Option<u32>,
}

/// Result of [`animation_normalize`]: the locked frames plus the report.
#[derive(Clone, Debug, Serialize)]
pub struct NormalizeResultDto {
    /// Normalized frames at the requested canvas size.
    pub frames: Vec<RgbaFrame>,
    /// Drift / scale / seam report the review surfaces.
    pub report: NormalizeReport,
}

/// Runs the normalization pass over a set of candidate frames.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn animation_normalize(args: NormalizeArgs) -> CommandResult<NormalizeResultDto> {
    if args.frames.is_empty() {
        return Err(AppCommandError::Validation {
            detail: "no frames to normalize".into(),
        });
    }
    let canvas = args.canvas_size.unwrap_or(args.frames[0].width).max(1);
    let chroma = match args.chroma.as_deref() {
        Some("magenta") => Some(ChromaKey::magenta()),
        Some("green") => Some(ChromaKey::green()),
        _ => None,
    };
    let buffers: Vec<PixelBuffer> = args
        .frames
        .iter()
        .map(RgbaFrame::to_buffer)
        .collect::<CommandResult<_>>()?;
    let opts = NormalizeOptions {
        chroma,
        reference_height: args.reference_height,
        ..NormalizeOptions::square(canvas)
    };
    let result = normalize_frames(&buffers, &opts).map_err(|e| AppCommandError::Validation {
        detail: format!("normalization failed: {e}"),
    })?;
    Ok(NormalizeResultDto {
        frames: result.frames.iter().map(RgbaFrame::from_buffer).collect(),
        report: result.report,
    })
}

// ── Background removal (extract stage) ───────────────────────────────────────────

/// Cuts the background from each extracted frame via the AI background-removal
/// backend (fal Bria), returning alpha-cut frames the studio then normalizes
/// with `chroma: "none"`.
///
/// Replaces the magenta chroma key: Seedance dithers the flat magenta into
/// noise a fixed-tolerance key leaves speckled. Each frame is an independent
/// call fired concurrently; any failure fails the whole extract (no chroma
/// fallback). Selecting by the `BACKGROUND_REMOVAL` capability guarantees an
/// fal backend, so a missing key errors before any call.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn animation_remove_background(
    frames: Vec<RgbaFrame>,
    state: State<'_, AppState>,
) -> CommandResult<Vec<RgbaFrame>> {
    if frames.is_empty() {
        return Err(AppCommandError::Validation {
            detail: "no frames to remove background from".into(),
        });
    }
    let backend = state
        .verb_runtime
        .select_backend(
            BackendCapabilities::BACKGROUND_REMOVAL,
            &VerbId::new("pixhaus.animation.remove_bg"),
        )
        .map_err(|_| AppCommandError::Validation {
            detail: "no background-removal backend configured — add a fal.ai key".into(),
        })?;

    // One call per frame, all in flight at once; await in order to keep the
    // loop sequenced. Cloning the backend Arc into each task keeps it 'static.
    let mut handles = Vec::with_capacity(frames.len());
    for frame in frames {
        let png = encode_pixeldata_to_png(&frame.into_pixeldata())?;
        let backend = backend.clone();
        handles.push(tokio::spawn(async move {
            invoke_selected_backend(
                &backend,
                InferenceRequest::BackgroundRemoval(BackgroundRemovalRequest {
                    model: None,
                    image: png,
                }),
                CancellationToken::new(),
            )
            .await
        }));
    }

    // Await in order. On the first failure, abort the still-running tasks
    // before returning so we don't leave detached jobs racking up paid calls.
    let mut out = Vec::with_capacity(handles.len());
    for idx in 0..handles.len() {
        let frame = async {
            let resp = (&mut handles[idx])
                .await
                .map_err(|e| AppCommandError::VerbError {
                    message: format!("background-removal task failed: {e}"),
                })??;
            let InferenceResponse::Image(image) = resp else {
                return Err(AppCommandError::VerbError {
                    message: "background-removal backend returned a non-image response".into(),
                });
            };
            let png =
                image
                    .images
                    .into_iter()
                    .next()
                    .ok_or_else(|| AppCommandError::VerbError {
                        message: "background-removal backend returned no image".into(),
                    })?;
            let pd = decode_png_to_pixeldata(&png)?;
            Ok(RgbaFrame {
                width: pd.width,
                height: pd.height,
                pixels: pd.bytes,
            })
        }
        .await;
        match frame {
            Ok(frame) => out.push(frame),
            Err(err) => {
                for handle in &handles[idx + 1..] {
                    handle.abort();
                }
                return Err(err);
            }
        }
    }
    Ok(out)
}

// ── Frame pick (i2v) ───────────────────────────────────────────────────────────

/// Arguments for [`animation_pick_frames`].
#[derive(Clone, Debug, Deserialize)]
pub struct PickFramesArgs {
    /// The i2v clip, base64-encoded (the verb's walk-clip custom effect
    /// carries it this way).
    pub clip_base64: String,
    /// Clip MIME type (e.g. `video/mp4`).
    pub mime: String,
    /// Decode / spacing fps.
    #[serde(default)]
    pub fps: Option<u32>,
    /// Target number of picked frames (8-12 typical).
    #[serde(default)]
    pub target_count: Option<u32>,
    /// Explicit loop markers `[start, end]`. `None` auto-detects.
    #[serde(default)]
    pub markers: Option<[usize; 2]>,
}

/// Result of [`animation_pick_frames`].
#[derive(Clone, Debug, Serialize)]
pub struct PickFramesResult {
    /// The picked loop frames, in order.
    pub frames: Vec<RgbaFrame>,
    /// The (auto-detected or supplied) loop markers used.
    pub markers: [usize; 2],
    /// All decoded frames (for the scrubber preview).
    pub total_frames: usize,
    /// Loop-seam similarity between the first and last picked frame, `0..=1`.
    pub seam_similarity: f32,
}

/// Decodes an i2v clip, auto-detects (or accepts) loop markers, and picks an
/// even loop. Backs the walk-cycle frame picker.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn animation_pick_frames(args: PickFramesArgs) -> CommandResult<PickFramesResult> {
    let clip = base64::engine::general_purpose::STANDARD
        .decode(args.clip_base64.as_bytes())
        .map_err(|e| AppCommandError::Validation {
            detail: format!("clip is not valid base64: {e}"),
        })?;
    let fps = args.fps.unwrap_or(12).clamp(1, 60);
    let target = args.target_count.unwrap_or(10).clamp(2, 24) as usize;

    // Decode is CPU + process bound; run it off the async worker.
    let decoded = tokio::task::spawn_blocking(move || {
        decode_video(
            &clip,
            &args.mime,
            &DecodeOptions {
                fps,
                max_frames: Some(240),
            },
        )
    })
    .await
    .map_err(|e| AppCommandError::Validation {
        detail: format!("decode task join failed: {e}"),
    })?
    .map_err(|e| AppCommandError::Validation {
        detail: format!("clip decode failed: {e}"),
    })?;

    // Convert decoded buffers to VideoFrames for the picker.
    let video: Vec<VideoFrame> = decoded
        .iter()
        .map(|d| VideoFrame {
            pixels: RgbaFrame::from_buffer(&d.buffer).into_pixeldata(),
            timestamp_ms: d.timestamp_ms,
        })
        .collect();

    let markers = match args.markers {
        Some([a, b]) => LoopMarkers { start: a, end: b },
        None => auto_loop_markers(&video),
    };
    let picked = pick_loop_frames(&video, markers, target);
    let seam = if picked.len() >= 2 {
        seam_similarity(&video, picked[0], *picked.last().unwrap_or(&picked[0]))
    } else {
        1.0
    };
    let frames = picked
        .iter()
        .filter_map(|&i| decoded.get(i))
        .map(|d| RgbaFrame::from_buffer(&d.buffer))
        .collect();

    Ok(PickFramesResult {
        frames,
        markers: [markers.start, markers.end],
        total_frames: decoded.len(),
        seam_similarity: seam,
    })
}

// ── Reference anchor (stage 0) ──────────────────────────────────────────────────

/// The resolved reference anchor image the studio animates, for the reference
/// stage and the persistent stage strip.
#[derive(Clone, Debug, Serialize)]
pub struct AnchorImageDto {
    /// Anchor image as base64-encoded PNG.
    pub png_base64: String,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
}

/// Returns the entity's resolved directional reference anchor as PNG bytes.
///
/// Backs the studio's reference stage — the image the first frame is derived
/// from. Errors clearly when the entity has no approved anchor.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn animation_get_anchor(
    entity_id: u32,
    direction: String,
    state: State<'_, AppState>,
) -> CommandResult<AnchorImageDto> {
    let dir = parse_direction(&direction).ok_or_else(|| AppCommandError::Validation {
        detail: format!("unknown direction: {direction}"),
    })?;
    let bytes = resolve_directional_anchor(&state, entity_id, dir).await?;
    let (width, height) = png_dimensions(&bytes)?;
    Ok(AnchorImageDto {
        png_base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
        width,
        height,
    })
}

// ── First frame (stage 1, image edit) ────────────────────────────────────────────

/// Arguments for [`animation_generate_first_frame`].
#[derive(Clone, Debug, Deserialize)]
pub struct GenerateFirstFrameArgs {
    /// Entity whose reference sheet is edited into the first frame.
    pub entity_id: u32,
    /// Direction the character should face.
    pub direction: String,
    /// Animation kind (`idle` / `walk` / `attack` / `custom`); scaffolds the
    /// pose phrasing. `None` is treated as a generic single pose.
    #[serde(default)]
    pub animation_kind: Option<String>,
    /// User pose / choreography description appended to the scaffold.
    #[serde(default)]
    pub prompt: Option<String>,
    /// Base image to edit (base64 PNG). Defaults to the reference anchor; set
    /// to the current first frame for an inpaint pass.
    #[serde(default)]
    pub base_image_base64: Option<String>,
    /// Inpaint mask (base64 PNG): white pixels are edited, the rest kept.
    #[serde(default)]
    pub mask_base64: Option<String>,
    /// Number of candidate frames to return.
    #[serde(default)]
    pub num_images: Option<u32>,
}

/// One first-frame candidate.
#[derive(Clone, Debug, Serialize)]
pub struct FirstFrameImageDto {
    /// Candidate as base64-encoded PNG.
    pub png_base64: String,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
}

/// Result of [`animation_generate_first_frame`].
#[derive(Clone, Debug, Serialize)]
pub struct GenerateFirstFrameResult {
    /// Candidate first frames, in backend order.
    pub images: Vec<FirstFrameImageDto>,
}

/// Produces the studio's first-frame candidates with `gpt-image-2`.
///
/// The first pass recomposes a single full-body pose from the reference sheet
/// via generation-with-references (`IMAGE_GENERATION`) — the edits endpoint
/// preserves the sheet's multi-panel layout, so it can't isolate one pose.
/// An inpaint pass (a painted mask + a fix instruction) refines the picked
/// frame in place via `IMAGE_EDIT`.
///
/// Backend selection happens here rather than in a verb: the capable backend
/// comes from the runtime registry, which a verb's single injected backend
/// can't provide.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn animation_generate_first_frame(
    args: GenerateFirstFrameArgs,
    state: State<'_, AppState>,
) -> CommandResult<GenerateFirstFrameResult> {
    let dir = parse_direction(&args.direction).ok_or_else(|| AppCommandError::Validation {
        detail: format!("unknown direction: {}", args.direction),
    })?;
    let num_images = args.num_images.unwrap_or(1).clamp(1, 4);

    // Source: a supplied base image (an inpaint pass over the current frame)
    // or the entity's reference anchor for the first pass.
    let source = match args.base_image_base64.as_deref() {
        Some(b64) => decode_b64_png(b64)?,
        None => resolve_directional_anchor(&state, args.entity_id, dir).await?,
    };

    let resp = if let Some(mask_b64) = args.mask_base64.as_deref() {
        // Inpaint: edit the painted region of the current frame in place.
        let backend =
            select_image_backend(&state, BackendCapabilities::IMAGE_INPAINT, "image-inpaint")?;
        let edit = ImageEditRequest {
            model: None,
            image: source,
            mask: Some(decode_b64_png(mask_b64)?),
            prompt: args
                .prompt
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("Blend the edited region naturally into the character.")
                .to_owned(),
            negative_prompt: None,
            num_images,
            style_image: None,
            reference_images: Vec::new(),
        };
        invoke_selected_backend(
            &backend,
            InferenceRequest::ImageInpaint(edit),
            CancellationToken::new(),
        )
        .await?
    } else {
        // First pass: recompose a single full-body pose from the reference
        // sheet, conditioned on it as a generation reference.
        let backend = select_image_backend(
            &state,
            BackendCapabilities::IMAGE_GENERATION,
            "image-generation",
        )?;
        let request = ImageGenRequest {
            model: None,
            prompt: first_frame_prompt(
                args.animation_kind.as_deref(),
                args.direction.as_str(),
                args.prompt.as_deref(),
            ),
            negative_prompt: None,
            width: 1024,
            height: 1024,
            steps: None,
            seed: None,
            num_images,
            quality: Some(ImageQuality::Medium),
            style_image: None,
            reference_images: vec![source],
        };
        invoke_selected_backend(
            &backend,
            InferenceRequest::ImageGeneration(request),
            CancellationToken::new(),
        )
        .await?
    };

    let InferenceResponse::Image(resp) = resp else {
        return Err(AppCommandError::VerbError {
            message: "expected an image response from the image backend".into(),
        });
    };
    let images = resp
        .images
        .iter()
        .map(|png| {
            let (width, height) = png_dimensions(png)?;
            Ok(FirstFrameImageDto {
                png_base64: base64::engine::general_purpose::STANDARD.encode(png),
                width,
                height,
            })
        })
        .collect::<CommandResult<Vec<_>>>()?;
    Ok(GenerateFirstFrameResult { images })
}

/// Selects a runtime backend advertising `cap`, with a configuration hint in
/// the error. `kind` names the capability for the message (e.g. `image-edit`).
fn select_image_backend(
    state: &State<'_, AppState>,
    cap: BackendCapabilities,
    kind: &str,
) -> CommandResult<std::sync::Arc<dyn pixhaus_ai::plugin::InferenceBackend>> {
    state
        .verb_runtime
        .select_backend(cap, &VerbId::new("pixhaus.animation.first_frame"))
        .map_err(|_| AppCommandError::Validation {
            detail: format!("no {kind} backend configured — add an OpenAI key"),
        })
}

/// Builds the first-frame generation prompt: recompose one full-body pose on
/// magenta. The magenta background is what the downstream chroma key removes.
fn first_frame_prompt(kind: Option<&str>, dir_label: &str, user: Option<&str>) -> String {
    let pose = match kind {
        Some("idle") => "a neutral standing idle pose",
        Some("walk") => "a mid-stride walking pose",
        Some("attack") => "a mid-attack pose",
        _ => "a single clear pose",
    };
    let mut prompt = format!(
        "Render the referenced character as a single full-body sprite in {pose}, facing \
         {dir_label}, centered with margin around it, on a flat solid magenta (#FF00FF) \
         background. One character only — no other poses, no panels, no text, no ground shadow."
    );
    if let Some(extra) = user.map(str::trim).filter(|s| !s.is_empty()) {
        prompt.push(' ');
        prompt.push_str(extra);
    }
    prompt
}

/// Resolves an entity's directional reference anchor to PNG bytes, erroring
/// clearly when none is approved.
async fn resolve_directional_anchor(
    state: &State<'_, AppState>,
    entity_id: u32,
    dir: pixhaus_core::project::AnchorDirection,
) -> CommandResult<Vec<u8>> {
    let doc = state.doc.read().await;
    doc.project
        .as_ref()
        .and_then(|p| {
            resolve_anchor_kind(
                p,
                Some(EntityId::new(entity_id)),
                AnchorKind::Directional(dir),
                &state.anchor_cache,
            )
        })
        .map(|a| a.image_bytes)
        .ok_or_else(|| AppCommandError::Validation {
            detail: "this entity has no approved reference anchor — create one first".into(),
        })
}

/// Decodes base64 PNG bytes, erroring clearly on malformed input.
fn decode_b64_png(s: &str) -> CommandResult<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(s.as_bytes())
        .map_err(|e| AppCommandError::Validation {
            detail: format!("invalid base64 image: {e}"),
        })
}

/// Reads the pixel dimensions of a PNG without keeping the decoded image.
fn png_dimensions(png: &[u8]) -> CommandResult<(u32, u32)> {
    let img = image::load_from_memory(png).map_err(|e| AppCommandError::Validation {
        detail: format!("invalid image: {e}"),
    })?;
    Ok((img.width(), img.height()))
}

// ── Clip generation (image-to-video) ────────────────────────────────────────────

/// Number of frames the i2v model generates per clip. Wan 2.1 (the fallback
/// i2v endpoint) rejects anything below 81; this is its minimum, which keeps
/// generation time down while giving the picker a dense source to sample.
/// Seedance ignores it (it is duration-driven).
const I2V_CLIP_FRAMES: u32 = 81;

/// Arguments for [`animation_generate_clip`].
#[derive(Clone, Debug, Deserialize)]
pub struct GenerateClipArgs {
    /// Entity the clip belongs to (used to track and list its jobs).
    pub entity_id: u32,
    /// The approved first frame the motion derives from (base64 PNG).
    pub first_frame_base64: String,
    /// Direction the character faces (`south` / `west` / `north` / `east`).
    pub direction: String,
    /// Animation kind (`idle` / `walk` / `attack` / `custom`). Drives the
    /// motion prompt. `None` is treated as a generic loop.
    #[serde(default)]
    pub animation_kind: Option<String>,
    /// Free-text choreography. Used as the motion prompt for `custom`, and
    /// appended to the built-in prompt for other kinds.
    #[serde(default)]
    pub choreography: Option<String>,
    /// Video model: `seedance` (default) or `wan`.
    #[serde(default)]
    pub model: Option<String>,
    /// Target picked-frame count (8-12 typical).
    #[serde(default)]
    pub num_frames: Option<u32>,
    /// Target clip fps.
    #[serde(default)]
    pub fps: Option<u32>,
    /// RNG seed for reproducibility.
    #[serde(default)]
    pub seed: Option<u64>,
}

/// Maps a UI model key to a fal i2v endpoint slug. Defaults to Seedance.
fn i2v_endpoint(model: Option<&str>) -> &'static str {
    match model {
        Some("wan") => pixhaus_ai::backends::fal::FAL_I2V,
        _ => pixhaus_ai::backends::fal::FAL_SEEDANCE,
    }
}

/// The clip the studio hands to the frame picker.
#[derive(Clone, Debug, Serialize)]
pub struct GenerateClipResult {
    /// The video clip, base64-encoded.
    pub clip_base64: String,
    /// MIME type (e.g. `video/mp4`).
    pub mime: String,
    /// Clip fps (decode / spacing rate).
    pub fps: u32,
    /// Target picked-frame count.
    pub target_count: u32,
}

/// Invokes a runtime-selected backend. The registry hands back the
/// lightweight plugin trait; the fat execution surface (`invoke`) lives behind
/// the `BackendProxy` wrapper every registered backend uses.
async fn invoke_selected_backend(
    backend: &std::sync::Arc<dyn pixhaus_ai::plugin::InferenceBackend>,
    request: InferenceRequest,
    cancel: CancellationToken,
) -> CommandResult<InferenceResponse> {
    let proxy = backend
        .as_any()
        .downcast_ref::<BackendProxy>()
        .ok_or_else(|| AppCommandError::VerbError {
            message: "selected backend does not expose the execution surface".into(),
        })?;
    let (progress, _rx) = VerbProgress::channel();
    proxy
        .fat()
        .invoke(request, progress, cancel)
        .await
        .map_err(|e| AppCommandError::VerbError {
            message: e.to_string(),
        })
}

/// Builds the image-to-video motion prompt for an animation kind, facing, and
/// optional free-text choreography. The hard negatives (no pivots, no
/// background, no particles) are applied separately at the call site.
fn motion_prompt(kind: Option<&str>, dir_label: &str, choreography: Option<&str>) -> String {
    let choreo = choreography.map(str::trim).filter(|c| !c.is_empty());
    let base = match kind {
        Some("idle") => {
            "The character stands in place in a looping idle — subtle breathing and weight \
             shift. No walking, no turning."
        }
        Some("walk") => {
            "The character walks in place, a clean looping walk cycle. The feet stay on a \
             constant ground line."
        }
        Some("attack") => {
            "The character performs one attack — anticipation, strike, then settle back to \
             the resting pose."
        }
        // `custom` (or unknown) leans entirely on the user's choreography.
        _ => "The character performs a short looping animation.",
    };
    let mut prompt = format!("{base} The character faces {dir_label} the whole time.");
    if let Some(choreo) = choreo {
        prompt.push(' ');
        prompt.push_str(choreo);
    }
    prompt
}

/// Tauri event name for image-to-video job state changes. The payload is an
/// [`AnimationJob`] (no clip bytes — the UI fetches those via
/// [`animation_job_clip`]).
const ANIMATION_JOB_EVENT: &str = "AnimationJobUpdate";

/// Emits an `AnimationJobUpdate` for `job`, logging on failure.
fn emit_job_update(app: &AppHandle, job: &AnimationJob) {
    if let Err(err) = app.emit(ANIMATION_JOB_EVENT, job) {
        tracing::warn!(error = %err, "failed to emit animation job update");
    }
}

/// Starts an image-to-video generation as a tracked, durable background job and
/// returns its id immediately.
///
/// The first frame comes from the studio's first-frame stage (a single clean
/// pose on magenta), not the raw reference sheet. Generation runs in a spawned
/// task so navigating away can't lose it; the UI tracks progress by id over
/// `AnimationJobUpdate` events and fetches the finished clip with
/// [`animation_job_clip`]. Backend selection happens here — picking an
/// `IMAGE_TO_VIDEO` backend needs the runtime's registry, which a verb's single
/// injected backend can't provide. Errors before creating a job when no i2v
/// backend is configured.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn animation_generate_clip(
    args: GenerateClipArgs,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<u64> {
    parse_direction(&args.direction).ok_or_else(|| AppCommandError::Validation {
        detail: format!("unknown direction: {}", args.direction),
    })?;
    // The UI count is the sprite-frame pick target, not the clip length. The
    // i2v model generates a full clip (Wan 2.1 requires at least 81 frames);
    // the picker then extracts `target_count` even frames from the decoded clip.
    let target_count = args.num_frames.unwrap_or(12).clamp(8, 24);
    let fps = args.fps.unwrap_or(12).clamp(1, 60);
    let first_frame = decode_b64_png(&args.first_frame_base64)?;
    let endpoint = i2v_endpoint(args.model.as_deref());
    let model_label = args.model.clone().unwrap_or_else(|| "seedance".into());

    // Select the backend up front so a missing key fails before a job exists.
    let backend = state
        .verb_runtime
        .select_backend(
            BackendCapabilities::IMAGE_TO_VIDEO,
            &VerbId::new("pixhaus.animation.i2v"),
        )
        .map_err(|_| AppCommandError::Validation {
            detail: "no image-to-video backend configured — add a fal.ai or Replicate key".into(),
        })?;
    let i2v_req = ImageToVideoRequest {
        model: Some(endpoint.to_owned()),
        image: first_frame,
        prompt: motion_prompt(
            args.animation_kind.as_deref(),
            args.direction.as_str(),
            args.choreography.as_deref(),
        ),
        negative_prompt: Some(
            "pivots, quarter turns, turning around, changing direction, background, scenery, \
             particles, glow, sparks, camera motion, zoom"
                .into(),
        ),
        num_frames: I2V_CLIP_FRAMES,
        fps,
        seed: args.seed,
    };

    // Register the job, then run generation in the background. The command
    // returns the id immediately; the task drives the job to a terminal state
    // and emits an update the UI reflects.
    let (job_id, token) = state.animation_jobs.start(
        args.entity_id,
        args.direction.clone(),
        args.animation_kind.clone(),
        model_label,
        target_count,
        fps,
    );
    let jobs = state.animation_jobs.clone();
    if let Some(job) = jobs.get(job_id) {
        emit_job_update(&app, &job);
    }

    tauri::async_runtime::spawn(async move {
        let result = invoke_selected_backend(
            &backend,
            InferenceRequest::ImageToVideo(i2v_req),
            token.clone(),
        )
        .await;
        match result {
            Ok(InferenceResponse::Video(clip)) => {
                jobs.finish_done(job_id, &clip.mime, &clip.clip);
            }
            Ok(_) => {
                jobs.finish_error(job_id, "expected a video response from the i2v backend");
            }
            Err(_) if token.is_cancelled() => jobs.finish_cancelled(job_id),
            Err(err) => jobs.finish_error(job_id, &err.to_string()),
        }
        if let Some(job) = jobs.get(job_id) {
            emit_job_update(&app, &job);
        }
    });

    Ok(job_id)
}

/// Lists the entity's i2v jobs, newest first, so the studio can reflect any
/// running, finished, or interrupted generation on (re)mount.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn animation_jobs_list(
    entity_id: u32,
    state: State<'_, AppState>,
) -> CommandResult<Vec<AnimationJob>> {
    Ok(state.animation_jobs.list(entity_id))
}

/// Returns one job's current state, or `None` if unknown. Backs the studio's
/// status polling, which is the authoritative source for in-flight progress
/// and cancellation (events are a best-effort fast-path on top).
#[tauri::command(async, rename_all = "snake_case")]
pub async fn animation_job_get(
    job_id: u64,
    state: State<'_, AppState>,
) -> CommandResult<Option<AnimationJob>> {
    Ok(state.animation_jobs.get(job_id))
}

/// Returns a finished job's clip as base64 for the frame picker.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn animation_job_clip(
    job_id: u64,
    state: State<'_, AppState>,
) -> CommandResult<GenerateClipResult> {
    let job = state
        .animation_jobs
        .get(job_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "animation job".into(),
            id: job_id,
        })?;
    let (bytes, mime) =
        state
            .animation_jobs
            .read_clip(job_id)
            .ok_or_else(|| AppCommandError::Validation {
                detail: "clip not available — the job has not finished".into(),
            })?;
    Ok(GenerateClipResult {
        clip_base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
        mime,
        fps: job.fps,
        target_count: job.target_count,
    })
}

/// Cancels an in-flight i2v job; the spawned task then marks it cancelled.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn animation_cancel_clip_job(
    job_id: u64,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    state.animation_jobs.cancel(job_id);
    Ok(())
}

// ── Integrate ──────────────────────────────────────────────────────────────────

/// Arguments for [`animation_integrate`].
#[derive(Clone, Debug, Deserialize)]
pub struct IntegrateArgs {
    /// Sprite to land the animation on.
    pub sprite_id: SpriteId,
    /// Tag / animation / layer name (e.g. `"walk-south"`).
    pub name: String,
    /// Frames to land, in display order.
    pub frames: Vec<RgbaFrame>,
    /// Loop direction for the tag and animation.
    pub loop_direction: LoopDirection,
    /// Tag repeat count; `0` loops forever.
    #[serde(default)]
    pub repeat: u16,
    /// Per-frame duration in milliseconds.
    pub frame_duration_ms: u32,
    /// Mirror each frame horizontally — used for east-as-flip-of-west, so the
    /// studio never regenerates east (the flip is free and consistent).
    #[serde(default)]
    pub flip_horizontal: bool,
    /// Entity whose `CharacterAnchor` records this as a derived sheet. When
    /// present together with `animation_kind` and `direction`, integrate
    /// appends a `DerivedSheet` edge so the cascade can detect staleness.
    #[serde(default)]
    pub entity_id: Option<u32>,
    /// Animation kind for the cascade record (`idle` / `walk` / `attack`).
    #[serde(default)]
    pub animation_kind: Option<String>,
    /// Direction for the cascade record (`south` / `west` / `north` / `east`).
    #[serde(default)]
    pub direction: Option<String>,
}

/// Result of a successful integrate.
#[derive(Clone, Debug, Serialize)]
pub struct IntegrateResult {
    /// The layer id assigned to the new animation layer.
    pub layer_id: LayerId,
    /// Frame index of the first new frame.
    pub start_frame: u32,
    /// Frame index of the last new frame.
    pub end_frame: u32,
}

/// Lands a set of frames as a layer + cels + frames + tag + engine animation,
/// in one undoable step. The timeline selects the new tag afterwards.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn animation_integrate(
    args: IntegrateArgs,
    state: State<'_, AppState>,
) -> CommandResult<IntegrateResult> {
    if args.frames.is_empty() {
        return Err(AppCommandError::Validation {
            detail: "no frames to integrate".into(),
        });
    }
    if args.frame_duration_ms == 0 {
        return Err(AppCommandError::Validation {
            detail: "frame_duration_ms must be at least 1".into(),
        });
    }

    // Prepare buffers (apply the east flip up front, off the lock).
    let buffers = prepare_integrate_buffers(&args.frames, args.flip_horizontal)?;

    let mut doc = state.doc.write().await;

    // Mint a layer id and one buffer id per frame.
    let layer_id = LayerId::new(doc.next_id);
    doc.next_id += 1;

    let mut new_entries: Vec<PixelBufferEntry> = Vec::with_capacity(buffers.len());
    let mut buffer_ids: Vec<PixelBufferId> = Vec::with_capacity(buffers.len());
    for buf in &buffers {
        let id = doc.next_id;
        doc.next_id += 1;
        buffer_ids.push(PixelBufferId::new(id));
        new_entries.push(PixelBufferEntry {
            id,
            width: buf.width(),
            height: buf.height(),
            stride: buf.stride(),
            pixels: buf.as_bytes().to_vec(),
        });
    }

    let result = {
        let sprite = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?
            .sprite_mut(args.sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(args.sprite_id.get()),
            })?;

        // Append new frames at the end of the timeline.
        let start =
            u32::try_from(sprite.frames.len()).map_err(|_| AppCommandError::Validation {
                detail: "sprite has too many frames".into(),
            })?;
        for _ in &buffers {
            sprite.frames.push(Frame {
                duration_ms: args.frame_duration_ms,
                duration_mul: 1.0,
                user_data: UserData::default(),
            });
        }
        let end = start + u32::try_from(buffers.len()).unwrap_or(0) - 1;

        // New layer + one cel per new frame.
        sprite.layers.push(Layer::raster(layer_id, &args.name));
        for (i, (buf, buf_id)) in buffers.iter().zip(&buffer_ids).enumerate() {
            let frame_index = FrameIndex::new(start + u32::try_from(i).unwrap_or(0));
            sprite.cels.push(Cel::raster(
                layer_id,
                frame_index,
                *buf_id,
                pixhaus_core::project::Size::new(buf.width(), buf.height()),
            ));
        }

        let range = FrameRange::new(FrameIndex::new(start), FrameIndex::new(end));

        // Frame tag (editor) + engine animation.
        sprite.frame_tags.push(FrameTag {
            name: args.name.clone(),
            range,
            loop_direction: args.loop_direction,
            repeat: args.repeat,
            user_data: UserData::default(),
        });
        sprite.animations.push(Animation {
            id: AnimationId::new(doc_anim_next_id(sprite)),
            name: args.name.clone(),
            range,
            loop_direction: args.loop_direction,
            speed_multiplier: 1.0,
            user_data: UserData::default(),
        });

        IntegrateResult {
            layer_id,
            start_frame: start,
            end_frame: end,
        }
    };

    // Record the cascade edge: a DerivedSheet pointing at the directional
    // anchor (or neutral) the frames were animated from. This is what
    // animation_set walks to detect staleness when the canonical re-rolls.
    if let (Some(eid), Some(kind), Some(dir)) = (
        args.entity_id,
        args.animation_kind.as_deref(),
        args.direction.as_deref(),
    ) && let (Some(kind), Some(dir)) = (parse_animation_kind(kind), parse_direction(dir))
    {
        record_derived_sheet(&mut doc, EntityId::new(eid), kind, dir, &args.name);
    }

    doc.pixel_buffers.extend(new_entries);
    doc.dirty = true;
    Ok(result)
}

/// Appends (or replaces) a `DerivedSheet` edge on the entity's character
/// anchor, pointing `derived_from` at the directional anchor (or the neutral
/// variant when no directional anchor exists). No-op if the entity has no
/// reference sheet.
fn record_derived_sheet(
    doc: &mut crate::state::DocumentStore,
    entity_id: EntityId,
    kind: pixhaus_core::project::AnimationKind,
    dir: pixhaus_core::project::AnchorDirection,
    tag_name: &str,
) {
    let Some(project) = doc.project.as_mut() else {
        return;
    };
    let Some(entity) = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == entity_id)
    else {
        return;
    };
    let EntityContent::Sprites {
        reference_sheet: Some(sheet),
        ..
    } = &mut entity.content
    else {
        return;
    };
    // The source the sheet derives from: directional anchor if present, else
    // the neutral variant, else the canonical (best-effort edge).
    let derived_from = sheet
        .anchor
        .directional
        .get(dir)
        .map(|v| v.id)
        .or_else(|| sheet.anchor.neutral.as_ref().map(|v| v.id))
        .or_else(|| sheet.canonical.as_ref().map(|v| v.id));
    let Some(derived_from) = derived_from else {
        return;
    };
    let new = pixhaus_core::project::DerivedSheet {
        animation_kind: kind,
        direction: dir,
        tag_name: tag_name.to_owned(),
        derived_from,
    };
    // Replace any existing sheet for the same (kind, direction).
    sheet
        .anchor
        .derived_sheets
        .retain(|s| !(s.animation_kind == kind && s.direction == dir));
    sheet.anchor.derived_sheets.push(new);
}

/// Parses the wire animation-kind string.
fn parse_animation_kind(s: &str) -> Option<pixhaus_core::project::AnimationKind> {
    use pixhaus_core::project::AnimationKind::{Attack, Idle, Walk};
    match s {
        "idle" => Some(Idle),
        "walk" => Some(Walk),
        "attack" => Some(Attack),
        _ => None,
    }
}

/// Parses the wire direction string.
fn parse_direction(s: &str) -> Option<pixhaus_core::project::AnchorDirection> {
    use pixhaus_core::project::AnchorDirection::{East, North, South, West};
    match s {
        "south" => Some(South),
        "west" => Some(West),
        "north" => Some(North),
        "east" => Some(East),
        _ => None,
    }
}

/// Builds the pixel buffers for an integrate, applying the east flip when
/// requested. Kept separate so the buffers are ready before the doc lock.
fn prepare_integrate_buffers(frames: &[RgbaFrame], flip: bool) -> CommandResult<Vec<PixelBuffer>> {
    frames
        .iter()
        .map(|f| {
            let buf = f.to_buffer()?;
            if flip {
                flip_horizontal(&buf, None).map_err(|e| AppCommandError::Validation {
                    detail: format!("east flip failed: {e}"),
                })
            } else {
                Ok(buf)
            }
        })
        .collect()
}

/// Mints the next free animation id within `sprite` (max existing + 1).
fn doc_anim_next_id(sprite: &pixhaus_core::project::Sprite) -> u32 {
    sprite
        .animations
        .iter()
        .map(|a| a.id.get())
        .max()
        .map_or(1, |m| m + 1)
}

// ── Animation set (directional cascade) ─────────────────────────────────────────

/// Per-cell status in the animation-set grid.
#[derive(Copy, Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CellStatus {
    /// No animation of this type/direction exists.
    Missing,
    /// An animation exists.
    Present,
    /// An animation exists but the anchor changed after it was made.
    Stale,
}

/// One cell of the cascade grid.
#[derive(Clone, Debug, Serialize)]
pub struct AnimationSetCell {
    /// Animation type stem (`idle`, `walk`, `attack`).
    pub animation_type: String,
    /// Direction (`south`, `west`, `north`, `east`).
    pub direction: String,
    /// Coverage / staleness status.
    pub status: CellStatus,
}

/// The per-entity animation set view.
#[derive(Clone, Debug, Serialize)]
pub struct AnimationSet {
    /// Whether the entity has an approved canonical anchor.
    pub anchor_approved: bool,
    /// One cell per (type, direction).
    pub cells: Vec<AnimationSetCell>,
    /// `true` when any cell is stale (anchor changed after derivation).
    pub has_stale: bool,
}

/// Computes the directional-cascade coverage for an entity.
///
/// Status comes from the entity's `CharacterAnchor.derived_sheets` and the
/// structural `derived_from` edges: a cell is `present` when a derived sheet
/// exists for `(kind, direction)`, and `stale` when the cascade
/// (`CharacterAnchor::is_sheet_stale`) detects an upstream change after the
/// canonical re-rolled.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn animation_set(
    entity_id: u32,
    state: State<'_, AppState>,
) -> CommandResult<AnimationSet> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    let cascade = entity_cascade(project, EntityId::new(entity_id));

    let types = ["idle", "walk", "attack"];
    let directions = ["south", "west", "north", "east"];
    let mut cells = Vec::with_capacity(types.len() * directions.len());
    let mut has_stale = false;
    for ty in types {
        for dir in directions {
            let status = cascade
                .as_ref()
                .map_or(CellStatus::Missing, |c| c.cell_status(ty, dir));
            if status == CellStatus::Stale {
                has_stale = true;
            }
            cells.push(AnimationSetCell {
                animation_type: ty.to_owned(),
                direction: dir.to_owned(),
                status,
            });
        }
    }

    Ok(AnimationSet {
        anchor_approved: cascade.is_some(),
        cells,
        has_stale,
    })
}

/// Returns the stale `(animation_kind, direction)` cells the studio should
/// regenerate. Backs the "re-roll dependents" action.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn animation_reroll_dependents(
    entity_id: u32,
    state: State<'_, AppState>,
) -> CommandResult<Vec<AnimationSetCell>> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    let Some(cascade) = entity_cascade(project, EntityId::new(entity_id)) else {
        return Ok(Vec::new());
    };
    let mut dependents = Vec::new();
    for ty in ["idle", "walk", "attack"] {
        for dir in ["south", "west", "north", "east"] {
            if cascade.cell_status(ty, dir) == CellStatus::Stale {
                dependents.push(AnimationSetCell {
                    animation_type: ty.to_owned(),
                    direction: dir.to_owned(),
                    status: CellStatus::Stale,
                });
            }
        }
    }
    Ok(dependents)
}

/// A borrowed view of an entity's canonical + character anchor, used to
/// compute per-cell cascade status.
struct EntityCascade<'a> {
    canonical: &'a SheetVariant,
    anchor: &'a pixhaus_core::project::CharacterAnchor,
}

impl EntityCascade<'_> {
    fn cell_status(&self, kind: &str, dir: &str) -> CellStatus {
        let (Some(kind), Some(dir)) = (parse_animation_kind(kind), parse_direction(dir)) else {
            return CellStatus::Missing;
        };
        match self
            .anchor
            .derived_sheets
            .iter()
            .find(|s| s.animation_kind == kind && s.direction == dir)
        {
            None => CellStatus::Missing,
            Some(sheet) => {
                if self.anchor.is_sheet_stale(sheet, self.canonical) {
                    CellStatus::Stale
                } else {
                    CellStatus::Present
                }
            }
        }
    }
}

/// Borrows an entity's canonical variant + character anchor, if it has an
/// approved canonical sheet.
fn entity_cascade(
    project: &pixhaus_core::project::Project,
    entity_id: EntityId,
) -> Option<EntityCascade<'_>> {
    let entity = project
        .library
        .entities
        .iter()
        .find(|e| e.id == entity_id)?;
    let EntityContent::Sprites {
        reference_sheet: Some(sheet),
        ..
    } = &entity.content
    else {
        return None;
    };
    let canonical = sheet.canonical.as_ref()?;
    Some(EntityCascade {
        canonical,
        anchor: &sheet.anchor,
    })
}

// ── Neutral anchor reset ────────────────────────────────────────────────────

/// Derives the effect-stripped neutral anchor for an entity.
///
/// Runs the Variant verb in `StripBakedEffects` mode against the entity's
/// canonical sheet, then stores the result as the `CharacterAnchor.neutral`
/// variant (origin `NeutralReset`, `parent_variant_id == canonical.id` so the
/// staleness cascade can walk the edge). Animations auto-prefer this neutral
/// anchor, so effects don't bleed into derived directions and frames.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn animation_derive_neutral(
    entity_id: u32,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let eid = EntityId::new(entity_id);

    // Read the canonical image + build the verb context, then release the lock
    // before awaiting the backend (never hold a lock across `.await`).
    let (ctx, canonical_id, canonical_pixels, dims) = {
        let doc = state.doc.read().await;
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let canonical =
            canonical_variant(project, eid).ok_or_else(|| AppCommandError::Validation {
                detail: "entity has no approved canonical anchor to strip".into(),
            })?;
        let pixels = decode_png_to_pixeldata(&canonical.image.bytes)?;
        let dims = (canonical.width, canonical.height);
        let ctx = build_verb_context(
            Some(project),
            Some(eid),
            AnchorKind::Canonical,
            &state.anchor_cache,
        );
        (ctx, canonical.id, pixels, dims)
    };

    // Invoke the strip-baked-effects variant operation.
    let inputs = VerbInputs::from_struct(&VariantInputs {
        pixels: canonical_pixels,
        mode: VariantMode::StripBakedEffects {
            extra_negatives: None,
        },
        layer_name: Some("Neutral".into()),
    })
    .map_err(|e| AppCommandError::VerbError {
        message: e.to_string(),
    })?;

    let invocation = state
        .verb_runtime
        .invoke(&VerbId::new(VARIANT_VERB_ID), ctx, inputs)
        .map_err(|e| AppCommandError::VerbError {
            message: e.to_string(),
        })?;
    let preview = invocation
        .finish()
        .await
        .map_err(|e| AppCommandError::VerbError {
            message: e.to_string(),
        })?;

    // Extract the stripped image from the verb's AddLayer effect.
    let stripped = preview
        .output
        .effects
        .iter()
        .find_map(|e| match e {
            VerbEffect::AddLayer { pixel_buffers, .. } => pixel_buffers.first(),
            _ => None,
        })
        .ok_or_else(|| AppCommandError::VerbError {
            message: "strip-baked-effects produced no image".into(),
        })?;
    let png = encode_pixeldata_to_png(&stripped.pixels)?;

    // Build the neutral SheetVariant and store it on the entity's anchor.
    let mut doc = state.doc.write().await;
    store_neutral_variant(&mut doc, eid, png, dims, canonical_id)?;
    doc.dirty = true;
    drop(doc);
    state.anchor_cache.remove(&entity_id);
    Ok(())
}

/// Stores a freshly-stripped neutral variant on the entity's character anchor,
/// stamped with `parent_variant_id == canonical_id` so the cascade can detect
/// staleness when the canonical re-rolls.
fn store_neutral_variant(
    doc: &mut crate::state::DocumentStore,
    eid: EntityId,
    png: Vec<u8>,
    dims: (u32, u32),
    canonical_id: SheetVariantId,
) -> CommandResult<()> {
    let ts = now_secs();
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == eid)
        .ok_or(AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(eid.get()),
        })?;
    let EntityContent::Sprites {
        reference_sheet: Some(sheet),
        ..
    } = &mut entity.content
    else {
        return Err(AppCommandError::Validation {
            detail: "entity has no reference sheet".into(),
        });
    };
    let new_id = next_sheet_variant_id(sheet);
    let mut neutral = SheetVariant::from_image(
        new_id,
        ts,
        ReferenceImage {
            bytes: png,
            mime: "image/png".into(),
        },
    );
    neutral.width = dims.0;
    neutral.height = dims.1;
    neutral.origin = VariantOrigin::NeutralReset;
    neutral.parent_variant_id = Some(canonical_id);
    sheet.anchor.neutral = Some(neutral);
    entity.updated_at = ts;
    Ok(())
}

/// Returns a clone of the entity's canonical sheet variant, if any.
fn canonical_variant(
    project: &pixhaus_core::project::Project,
    entity_id: EntityId,
) -> Option<SheetVariant> {
    let entity = project
        .library
        .entities
        .iter()
        .find(|e| e.id == entity_id)?;
    match &entity.content {
        EntityContent::Sprites {
            reference_sheet: Some(sheet),
            ..
        } => sheet.canonical.clone(),
        _ => None,
    }
}

/// Mints the next free `SheetVariantId` across canonical, history, and the
/// anchor cascade so a derived variant never collides.
fn next_sheet_variant_id(sheet: &pixhaus_core::project::ReferenceSheet) -> SheetVariantId {
    let mut max = 0u32;
    let mut bump = |v: &SheetVariant| max = max.max(v.id.get());
    if let Some(c) = &sheet.canonical {
        bump(c);
    }
    sheet.variants.iter().for_each(&mut bump);
    if let Some(n) = &sheet.anchor.neutral {
        bump(n);
    }
    for d in [
        &sheet.anchor.directional.south,
        &sheet.anchor.directional.west,
        &sheet.anchor.directional.north,
    ]
    .into_iter()
    .flatten()
    {
        bump(d);
    }
    SheetVariantId::new(max + 1)
}

/// Decodes PNG bytes into a tightly-packed RGBA8 [`PixelData`].
fn decode_png_to_pixeldata(png: &[u8]) -> CommandResult<PixelData> {
    let img = image::load_from_memory(png)
        .map_err(|e| AppCommandError::Validation {
            detail: format!("canonical image decode failed: {e}"),
        })?
        .to_rgba8();
    let (w, h) = img.dimensions();
    Ok(PixelData::rgba8(w, h, img.into_raw()))
}

/// Encodes a (possibly strided) RGBA8 [`PixelData`] to PNG bytes.
fn encode_pixeldata_to_png(pd: &PixelData) -> CommandResult<Vec<u8>> {
    let row = (pd.width * 4) as usize;
    let stride = pd.stride as usize;
    let mut tight = Vec::with_capacity(row * pd.height as usize);
    for y in 0..pd.height as usize {
        let start = y * stride;
        let end = start.saturating_add(row);
        let slice = pd
            .bytes
            .get(start..end)
            .ok_or_else(|| AppCommandError::Validation {
                detail: "stripped image has inconsistent stride / byte length".into(),
            })?;
        tight.extend_from_slice(slice);
    }
    let img = image::RgbaImage::from_raw(pd.width, pd.height, tight).ok_or_else(|| {
        AppCommandError::Validation {
            detail: "stripped image has inconsistent dimensions".into(),
        }
    })?;
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| AppCommandError::Validation {
            detail: format!("PNG encode failed: {e}"),
        })?;
    Ok(png)
}

/// Current UTC seconds since the Unix epoch.
fn now_secs() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

// ── PixelData bridge ─────────────────────────────────────────────────────────

impl RgbaFrame {
    /// Converts to the ai-crate `PixelData` the frame picker consumes.
    fn into_pixeldata(self) -> PixelData {
        PixelData::rgba8(self.width, self.height, self.pixels)
    }
}

// ── Studio state persistence ─────────────────────────────────────────────────

/// Persists the animation-studio working state for `entity_id` into the
/// project so it is written to the `.pixhaus` file on save. The blob is a
/// UI-owned JSON string (the studio store snapshot); `None` clears it. Mirrors
/// the `canvas_set_viewport` "push editor state into the project" pattern.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn animation_studio_set_state(
    entity_id: u32,
    state_json: Option<String>,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let id = EntityId::new(entity_id);
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let entity = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == id)
        .ok_or(AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id),
        })?;
    entity.ai.animation_studio_state = state_json;
    doc.dirty = true;
    Ok(())
}

/// Reads the persisted animation-studio working state for `entity_id`, or
/// `None` when the studio has never been used for it.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn animation_studio_get_state(
    entity_id: u32,
    state: State<'_, AppState>,
) -> CommandResult<Option<String>> {
    let id = EntityId::new(entity_id);
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    let blob = project
        .library
        .entities
        .iter()
        .find(|e| e.id == id)
        .ok_or(AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id),
        })?
        .ai
        .animation_studio_state
        .clone();
    Ok(blob)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, rgba: [u8; 4]) -> RgbaFrame {
        RgbaFrame {
            width: w,
            height: h,
            pixels: rgba.repeat((w * h) as usize),
        }
    }

    #[tokio::test]
    async fn normalize_returns_frames_and_report() {
        let args = NormalizeArgs {
            frames: vec![solid(8, 8, [0, 0, 0, 255]), solid(8, 8, [0, 0, 0, 255])],
            canvas_size: Some(8),
            chroma: Some("none".into()),
            reference_height: None,
        };
        let res = animation_normalize(args).await.unwrap();
        assert_eq!(res.frames.len(), 2);
        assert_eq!(res.frames[0].width, 8);
    }

    #[tokio::test]
    async fn normalize_rejects_empty() {
        let args = NormalizeArgs {
            frames: vec![],
            canvas_size: None,
            chroma: None,
            reference_height: None,
        };
        assert!(animation_normalize(args).await.is_err());
    }

    #[test]
    fn rgba_frame_round_trips_through_buffer() {
        let f = solid(4, 3, [10, 20, 30, 255]);
        let buf = f.to_buffer().unwrap();
        let back = RgbaFrame::from_buffer(&buf);
        assert_eq!(back.width, 4);
        assert_eq!(back.height, 3);
        assert_eq!(back.pixels, f.pixels);
    }

    #[test]
    fn parse_helpers_round_trip() {
        use pixhaus_core::project::{AnchorDirection, AnimationKind};
        assert_eq!(parse_animation_kind("walk"), Some(AnimationKind::Walk));
        assert_eq!(parse_direction("east"), Some(AnchorDirection::East));
        assert_eq!(parse_animation_kind("bogus"), None);
        assert_eq!(parse_direction("bogus"), None);
    }

    #[test]
    fn motion_prompt_varies_by_kind() {
        assert!(
            motion_prompt(Some("idle"), "west", None)
                .to_lowercase()
                .contains("idle")
        );
        assert!(
            motion_prompt(Some("walk"), "south", None)
                .to_lowercase()
                .contains("walk")
        );
        assert!(
            motion_prompt(Some("attack"), "north", None)
                .to_lowercase()
                .contains("attack")
        );
        // All include the facing.
        assert!(motion_prompt(Some("walk"), "south", None).contains("south"));
        // Custom (or unknown) leans on the supplied choreography.
        let custom = motion_prompt(None, "east", Some("does a backflip"));
        assert!(custom.contains("does a backflip"));
        // Empty choreography is ignored.
        assert!(!motion_prompt(Some("idle"), "west", Some("  ")).ends_with("  "));
    }

    #[test]
    fn first_frame_prompt_requests_single_pose_on_magenta() {
        let p = first_frame_prompt(Some("walk"), "south", None);
        let lower = p.to_lowercase();
        assert!(lower.contains("magenta"));
        assert!(lower.contains("#ff00ff"));
        assert!(lower.contains("walking"));
        assert!(p.contains("south"));
        // User detail is appended; blank detail is ignored.
        assert!(
            first_frame_prompt(None, "east", Some("holding a sword")).contains("holding a sword")
        );
        assert!(!first_frame_prompt(Some("idle"), "west", Some("   ")).ends_with("   "));
    }

    #[test]
    fn i2v_endpoint_defaults_to_seedance() {
        assert_eq!(i2v_endpoint(None), pixhaus_ai::backends::fal::FAL_SEEDANCE);
        assert_eq!(
            i2v_endpoint(Some("seedance")),
            pixhaus_ai::backends::fal::FAL_SEEDANCE
        );
        assert_eq!(
            i2v_endpoint(Some("wan")),
            pixhaus_ai::backends::fal::FAL_I2V
        );
    }

    #[test]
    fn prepare_buffers_flips_when_requested() {
        // 2x1: left red, right green. After horizontal flip, swapped.
        let f = RgbaFrame {
            width: 2,
            height: 1,
            pixels: vec![255, 0, 0, 255, 0, 255, 0, 255],
        };
        let flipped = prepare_integrate_buffers(std::slice::from_ref(&f), true).unwrap();
        assert_eq!(flipped[0].pixel(0, 0).unwrap().g, 255, "left now green");
        let same = prepare_integrate_buffers(&[f], false).unwrap();
        assert_eq!(same[0].pixel(0, 0).unwrap().r, 255, "left stays red");
    }
}
