//! AI orchestration: builds the verb runtime, registers the FAL backend, and
//! drives the reference-sheet verb on the shell's tokio runtime, streaming
//! progress and results back over the UI channel.
//!
//! This is the real `ai/` path — the runtime selects an `IMAGE_GENERATION`
//! backend (FAL) and runs `pixhaus.builtin.generate_reference_sheet`.

use std::collections::BTreeMap;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use base64::Engine as _;
use eframe::egui;
use pixhaus_ai::backends::fal::FalBackend;
use pixhaus_ai::backends::openai::OpenAiBackend;
use pixhaus_ai::backends::{
    ApiKeyStore, BackendProxy, BackgroundRemovalRequest, ImageGenRequest, ImageToVideoRequest,
    InferenceRequest, InferenceResponse,
};
use pixhaus_ai::plugin::{
    BackendCapabilities, VerbContext, VerbEffect, VerbId, VerbInputs, VerbProgress,
    VerbProgressEvent, VerbRuntime,
};
use pixhaus_ai::verbs::reference_sheet::{
    GenerateReferenceSheetInputs, GenerateReferenceSheetVerb, GenerateSheetPayload,
    GENERATE_REFERENCE_SHEET_VERB_ID, GENERATE_SHEET_EFFECT_NAME,
};
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::library::composition::StructureId;
use pixhaus_core::project::{EntityId, ProjectMetadata};
use pixhaus_core::transforms::normalize::{normalize_frames, NormalizeOptions};
use tokio::runtime::Handle;
use tokio_util::sync::CancellationToken;

use crate::anim::{self, VideoFrame};
use crate::app::ShellMsg;

/// Backend id FAL keys are stored under (keychain service `pixhaus.fal`).
pub const FAL_BACKEND_ID: &str = "fal";
/// Backend id `OpenAI` keys are stored under (keychain service `pixhaus.openai`).
pub const OPENAI_BACKEND_ID: &str = "openai";

/// Default reference-sheet prompt — Bit, the Pixhaus mascot. Ported from
/// `ui/src/sheet/sheet-editor-state.ts::DEFAULT_SHEET_PROMPT`. Pre-fills a fresh
/// editor so Generate works in one click and shows a worked example.
pub const DEFAULT_SHEET_PROMPT: &str = "Bit, the Pixhaus mascot — a small retro robot with a boxy CRT/floppy-disk head, a glowing pixel-face screen showing its expression, a stubby antenna with a blinking pixel, chunky rounded limbs, friendly proportions, crisp 8-bit palette.";

/// Reference-sheet templates the inspector offers, paired with their built-in
/// composition Structure ids.
pub const TEMPLATES: [(&str, &str); 4] = [
    ("Character", "pixhaus.builtin.structure.character"),
    ("Item", "pixhaus.builtin.structure.item"),
    ("Tileset", "pixhaus.builtin.structure.tileset"),
    ("Custom", "pixhaus.builtin.structure.custom"),
];

/// Builds the verb runtime with the reference-sheet verb registered, and tries
/// to register the `OpenAI` and FAL backends from the OS keychain. Returns the
/// runtime and whether at least one image-generation backend is ready.
///
/// `OpenAI` (gpt-image-2) is registered at higher priority for image generation
/// (reference sheets); FAL is the only backend that also covers image-to-video
/// and background removal (the animation pipeline).
#[must_use]
pub fn build_runtime() -> (Arc<VerbRuntime>, bool) {
    let runtime = VerbRuntime::new();
    if let Err(err) = runtime.register(GenerateReferenceSheetVerb::new()) {
        tracing::error!(%err, "failed to register reference-sheet verb");
    }
    let openai = try_register_openai(&runtime);
    let fal = try_register_fal(&runtime);
    (Arc::new(runtime), openai || fal)
}

/// Tries to register the `OpenAI` backend from the keychain (priority 0 — preferred
/// for image generation). Returns true on success.
pub fn try_register_openai(runtime: &VerbRuntime) -> bool {
    match OpenAiBackend::from_keychain() {
        Ok(openai) => match runtime.register_backend(BackendProxy::new(openai), 0) {
            Ok(()) => true,
            Err(err) => {
                tracing::error!(%err, "failed to register OpenAI backend");
                false
            }
        },
        Err(err) => {
            tracing::info!(%err, "OpenAI backend unavailable (no API key configured)");
            false
        }
    }
}

/// Tries to register the FAL backend from the keychain (priority 10). Returns
/// true on success. Safe to call again after the user stores a key.
pub fn try_register_fal(runtime: &VerbRuntime) -> bool {
    match FalBackend::from_keychain() {
        Ok(fal) => match runtime.register_backend(BackendProxy::new(fal), 10) {
            Ok(()) => true,
            Err(err) => {
                tracing::error!(%err, "failed to register FAL backend");
                false
            }
        },
        Err(err) => {
            tracing::info!(%err, "FAL backend unavailable (no API key configured)");
            false
        }
    }
}

/// Stores an API key for `backend` ("fal" or "openai") in the OS keychain.
/// Blocking I/O — call off the UI thread.
///
/// # Errors
/// Propagates keychain write failures as a string.
pub fn store_key(backend: &str, key: &str) -> Result<(), String> {
    ApiKeyStore::set(backend, key).map_err(|e| e.to_string())
}

/// Parameters for a reference-sheet generation.
pub struct SheetJob {
    /// Project metadata for the verb context.
    pub meta: ProjectMetadata,
    /// Target sprite entity.
    pub entity_id: EntityId,
    /// Built-in composition Structure id (see [`TEMPLATES`]).
    pub structure_id: String,
    /// Free-typed subject description.
    pub prompt: String,
    /// Candidate count (clamped 1-4).
    pub num_variants: u32,
}

/// Runs the reference-sheet verb to completion and returns the candidate PNGs.
/// `progress` is called for each progress event. Used by both the GUI spawn and
/// the headless runner.
pub async fn run_reference_sheet(
    runtime: &VerbRuntime,
    job: SheetJob,
    progress: &(dyn Fn(&str) + Sync),
) -> Result<Vec<Vec<u8>>, String> {
    let inputs = GenerateReferenceSheetInputs {
        entity_id: job.entity_id,
        structure_id: StructureId(job.structure_id),
        style_id: None,
        prompt_id: None,
        variable_values: BTreeMap::new(),
        inline_text: job.prompt,
        inline_negatives: String::new(),
        num_variants: job.num_variants.clamp(1, 4),
        quality: None,
        seed: None,
    };

    let verb_inputs = VerbInputs::from_struct(&inputs).map_err(|e| e.to_string())?;
    let vctx = VerbContext::empty(job.meta);
    let mut invocation = runtime
        .invoke(
            &VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID),
            vctx,
            verb_inputs,
        )
        .map_err(|e| e.to_string())?;

    while let Some(event) = invocation.next_progress().await {
        match event {
            VerbProgressEvent::Started { backend } => {
                progress(&format!(
                    "started on {}",
                    backend.unwrap_or_else(|| "backend".into())
                ));
            }
            VerbProgressEvent::Step { message, .. } => progress(&message),
            _ => {}
        }
    }

    let preview = invocation.finish().await.map_err(|e| e.to_string())?;
    let variants = extract_variants(&preview.output);
    if variants.is_empty() {
        return Err("backend returned no variants".into());
    }
    Ok(variants)
}

/// Spawns a reference-sheet generation on the tokio runtime. Progress and the
/// final variants (decoded PNG bytes) arrive over `tx`; `ctx` is woken after
/// each message so the idle UI repaints.
pub fn spawn_reference_sheet(
    handle: &Handle,
    runtime: Arc<VerbRuntime>,
    ctx: egui::Context,
    tx: Sender<ShellMsg>,
    job: SheetJob,
) {
    handle.spawn(async move {
        let progress_tx = tx.clone();
        let progress_ctx = ctx.clone();
        let progress = move |msg: &str| {
            let _ = progress_tx.send(ShellMsg::SheetProgress {
                fraction: None,
                message: msg.to_owned(),
            });
            progress_ctx.request_repaint();
        };
        let result = run_reference_sheet(&runtime, job, &progress).await;
        match result {
            Ok(variants) => {
                let _ = tx.send(ShellMsg::SheetDone { variants });
            }
            Err(err) => {
                let _ = tx.send(ShellMsg::SheetFailed(err));
            }
        }
        ctx.request_repaint();
    });
}

/// Parameters for an animation generation.
pub struct AnimJob {
    /// Target canvas size (width, height) the frames are normalized to.
    pub canvas: (u32, u32),
    /// Approved reference-sheet PNG used as the character anchor.
    pub anchor_png: Vec<u8>,
    /// Motion description (e.g. "walk cycle, side view").
    pub motion_prompt: String,
    /// Number of loop frames to land in the timeline.
    pub target_frames: u32,
    /// Playback frames per second.
    pub fps: u32,
}

/// Spawns the animation pipeline on the tokio runtime: first-frame generation
/// (FAL image-gen conditioned on the anchor) -> image-to-video clip (FAL) ->
/// clip decode -> loop frame pick -> background removal -> normalize. The final
/// frames (or an error) arrive over `tx`.
pub fn spawn_animation(
    handle: &Handle,
    runtime: Arc<VerbRuntime>,
    ctx: egui::Context,
    tx: Sender<ShellMsg>,
    job: AnimJob,
) {
    handle.spawn(async move {
        let progress_tx = tx.clone();
        let progress_ctx = ctx.clone();
        let progress = move |msg: &str| {
            let _ = progress_tx.send(ShellMsg::AnimProgress {
                message: msg.to_owned(),
            });
            progress_ctx.request_repaint();
        };
        match run_animation(&runtime, &job, &progress).await {
            Ok((frames, frame_duration_ms)) => {
                let _ = tx.send(ShellMsg::AnimDone {
                    frames,
                    frame_duration_ms,
                });
            }
            Err(err) => {
                let _ = tx.send(ShellMsg::AnimFailed(err));
            }
        }
        ctx.request_repaint();
    });
}

/// The animation pipeline body. Calls `progress` at each stage; returns the
/// normalized loop frames and per-frame duration on success. Used by both the
/// GUI spawn and the headless runner.
// A linear state machine: first frame -> clip -> decode -> pick -> bg-remove ->
// normalize, with a progress checkpoint at each stage. Splitting it would
// scatter the await contract across helpers.
#[allow(clippy::too_many_lines)]
pub async fn run_animation(
    runtime: &VerbRuntime,
    job: &AnimJob,
    progress: &(dyn Fn(&str) + Sync),
) -> Result<(Vec<PixelBuffer>, u32), String> {
    let (width, height) = job.canvas;
    let prog = |message: &str| progress(message);

    prog("generating first frame");
    let first_frame = {
        let req = ImageGenRequest {
            model: None,
            prompt: format!(
                "{}, single sprite frame, side view, transparent background",
                job.motion_prompt
            ),
            negative_prompt: Some("background, particles, glow, motion blur".into()),
            width,
            height,
            steps: None,
            seed: None,
            num_images: 1,
            quality: None,
            style_image: None,
            reference_images: vec![job.anchor_png.clone()],
        };
        match invoke_fat(
            runtime,
            BackendCapabilities::IMAGE_GENERATION,
            InferenceRequest::ImageGeneration(req),
        )
        .await?
        {
            InferenceResponse::Image(r) => r
                .images
                .into_iter()
                .next()
                .ok_or_else(|| "first-frame backend returned no image".to_owned())?,
            _ => return Err("unexpected response for first-frame generation".into()),
        }
    };

    prog("generating clip (image-to-video)");
    let (clip, mime) = {
        let req = ImageToVideoRequest {
            model: None,
            image: first_frame,
            prompt: job.motion_prompt.clone(),
            negative_prompt: Some("pivots, quarter-turns, background, particles, glow".into()),
            num_frames: (job.target_frames * 4).max(16),
            fps: job.fps,
            seed: None,
        };
        match invoke_fat(
            runtime,
            BackendCapabilities::IMAGE_TO_VIDEO,
            InferenceRequest::ImageToVideo(req),
        )
        .await?
        {
            InferenceResponse::Video(v) => (v.clip, v.mime),
            _ => return Err("unexpected response for image-to-video".into()),
        }
    };

    prog("decoding clip");
    let decode_fps = job.fps;
    // Clip decode (ffmpeg shell-out / image decode) is blocking — keep it off
    // the async worker.
    let frames = tokio::task::spawn_blocking(move || anim::decode_clip(&clip, &mime, decode_fps))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    if frames.is_empty() {
        return Err("clip decoded to zero frames".into());
    }

    let markers = anim::auto_loop_markers(&frames);
    let picks = anim::pick_loop_frames(&frames, markers, job.target_frames as usize);
    if picks.is_empty() {
        return Err("no loop frames picked".into());
    }

    prog("removing backgrounds");
    let mut picked: Vec<VideoFrame> = Vec::with_capacity(picks.len());
    for &idx in &picks {
        let frame = frames[idx].clone();
        // Best-effort: keep the original frame if background removal fails.
        let stripped = strip_background(runtime, &frame).await.unwrap_or(frame);
        picked.push(stripped);
    }

    prog("normalizing");
    let buffers: Vec<PixelBuffer> = picked
        .iter()
        .filter_map(|f| PixelBuffer::from_raw(f.width, f.height, f.width * 4, f.pixels.clone()).ok())
        .collect();
    if buffers.is_empty() {
        return Err("no decodable loop frames".into());
    }
    let opts = NormalizeOptions {
        canvas_width: width,
        canvas_height: height,
        alpha_threshold: 8,
        chroma: None,
        reference_height: None,
        bottom_margin: 0,
    };
    let result = normalize_frames(&buffers, &opts).map_err(|e| e.to_string())?;
    let frame_duration_ms = (1000 / job.fps.max(1)).max(1);
    Ok((result.frames, frame_duration_ms))
}

/// Background-removes a single frame via FAL. Returns `None` on any failure so
/// the caller can fall back to the original frame.
async fn strip_background(runtime: &VerbRuntime, frame: &VideoFrame) -> Option<VideoFrame> {
    let png = anim::encode_png(frame)?;
    let req = BackgroundRemovalRequest {
        model: None,
        image: png,
    };
    match invoke_fat(
        runtime,
        BackendCapabilities::BACKGROUND_REMOVAL,
        InferenceRequest::BackgroundRemoval(req),
    )
    .await
    .ok()?
    {
        InferenceResponse::Image(r) => {
            let out = r.images.into_iter().next()?;
            anim::decode_png(&out, frame.timestamp_ms)
        }
        _ => None,
    }
}

/// Selects a backend by capability and invokes a typed inference request
/// through the fat backend behind the [`BackendProxy`].
async fn invoke_fat(
    runtime: &VerbRuntime,
    capability: BackendCapabilities,
    request: InferenceRequest,
) -> Result<InferenceResponse, String> {
    let thin = runtime
        .select_backend(capability, &VerbId::new("shell.animation"))
        .map_err(|e| e.to_string())?;
    let proxy = thin
        .as_any()
        .downcast_ref::<BackendProxy>()
        .ok_or_else(|| "registered backend is not a BackendProxy".to_owned())?;
    proxy
        .fat()
        .invoke(request, VerbProgress::discard(), CancellationToken::new())
        .await
        .map_err(|e| e.to_string())
}

/// Pulls decoded PNG bytes for each sheet variant out of the verb output.
fn extract_variants(output: &pixhaus_ai::plugin::VerbOutput) -> Vec<Vec<u8>> {
    for effect in &output.effects {
        let VerbEffect::Custom { name, payload } = effect else {
            continue;
        };
        if name != GENERATE_SHEET_EFFECT_NAME {
            continue;
        }
        let Ok(parsed) = serde_json::from_value::<GenerateSheetPayload>(payload.clone()) else {
            continue;
        };
        return parsed
            .variants
            .iter()
            .filter_map(|v| {
                base64::engine::general_purpose::STANDARD
                    .decode(&v.image_b64)
                    .ok()
            })
            .collect();
    }
    Vec::new()
}
