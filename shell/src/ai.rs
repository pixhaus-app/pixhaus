//! AI orchestration: builds the verb runtime, registers the FAL backend, and
//! drives the reference-sheet verb on the shell's tokio runtime, streaming
//! progress and results back over the UI channel.
//!
//! This is the real `ai/` path — the runtime selects an `IMAGE_GENERATION`
//! backend (FAL) and runs `pixhaus.builtin.generate_reference_sheet`.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use base64::Engine as _;
use eframe::egui;
use pixhaus_ai::backends::fal::FalBackend;
use pixhaus_ai::backends::openai::OpenAiBackend;
use pixhaus_ai::backends::{
    ApiKeyStore, BackendError, BackendProxy, BackgroundRemovalRequest, ImageEditRequest, ImageGenRequest, ImageToVideoRequest, InferenceRequest,
    InferenceResponse,
};
use pixhaus_ai::compose::builtins::{BUILTIN_DEFAULT_BASELINE, BuiltinLibrary, STRUCTURE_SINGLE_ID};
use pixhaus_ai::compose::{ComposeRequest, compose};
use pixhaus_ai::plugin::{
    BackendCapabilities, CompositionLibraryView, PixelData, ProjectCompositionLibrary, VerbContext, VerbEffect, VerbId, VerbInputs, VerbProgress,
    VerbProgressEvent, VerbRuntime,
};
use pixhaus_ai::verbs::reference_sheet::{
    GENERATE_REFERENCE_SHEET_VERB_ID, GENERATE_SHEET_EFFECT_NAME, GenerateReferenceSheetInputs, GenerateReferenceSheetVerb, GenerateSheetPayload,
    ReferenceInput, SheetVariantOutput,
};
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::library::composition::{PromptId, PromptVariable, StructureId};
use pixhaus_core::project::{EntityId, PixelBufferId, ProjectMetadata};
use pixhaus_core::transforms::normalize::{NormalizeOptions, normalize_frames};
use tokio::runtime::Handle;
use tokio_util::sync::CancellationToken;

use crate::anim::{self, VideoFrame};
use crate::app::ShellMsg;
use crate::studio::GenTarget;

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

/// The built-in free-form structure id — the cockpit's default output.
pub const SINGLE_STRUCTURE_ID: &str = STRUCTURE_SINGLE_ID;

/// The (id, display-name) of every resolvable composition structure, with the
/// free-form `Single` first so it reads as the natural default in the picker.
#[must_use]
pub fn structure_options() -> Vec<(String, String)> {
    let lib = BuiltinLibrary::load();
    let mut out: Vec<(String, String)> = lib.structures.values().map(|s| (s.id.0.clone(), s.name.clone())).collect();
    out.sort_by_key(|(id, _)| u8::from(id != STRUCTURE_SINGLE_ID));
    out
}

/// The variables of a saved/built-in prompt template, for the cockpit dials.
/// Empty for an unknown id or a template with no variables.
#[must_use]
pub fn prompt_variables(prompt_id: &str) -> Vec<PromptVariable> {
    let lib = BuiltinLibrary::load();
    lib.prompts
        .get(&PromptId(prompt_id.to_owned()))
        .map(|p| p.variables.clone())
        .unwrap_or_default()
}

/// The (id, name) of every saved/built-in prompt template, name-sorted, for the
/// cockpit template picker.
#[must_use]
pub fn prompt_options() -> Vec<(String, String)> {
    let lib = BuiltinLibrary::load();
    let mut out: Vec<(String, String)> = lib.prompts.values().map(|p| (p.id.0.clone(), p.name.clone())).collect();
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

/// Composes the live positive/negative prompt preview for the cockpit, on the
/// UI thread, with no backend call. Mirrors what the verb sends: the project
/// `style_notes` (or the built-in default) lead, then the structure's layout
/// prose, the picked template (with variables substituted), then the subject.
/// Returns empty strings for an unknown structure.
#[must_use]
pub fn compose_preview(
    structure_id: &str,
    subject: &str,
    style_notes: &str,
    prompt_id: Option<&str>,
    variables: &BTreeMap<String, String>,
) -> (String, String) {
    let builtins = BuiltinLibrary::load();
    let view = CompositionLibraryView::new(&[], &[], &[], builtins);
    let Some(structure) = view.structure(&StructureId(structure_id.to_owned())) else {
        return (String::new(), String::new());
    };
    let prompt = prompt_id.and_then(|id| view.prompt(&PromptId(id.to_owned())));
    let baseline = if style_notes.trim().is_empty() {
        BUILTIN_DEFAULT_BASELINE
    } else {
        style_notes
    };
    let empty = BTreeMap::new();
    let req = ComposeRequest {
        baseline,
        structure,
        style: None,
        prompt,
        variable_values: variables,
        entity_info: &empty,
        inline_text: subject,
        inline_negatives: "",
        operation_hint: None,
        context_fragments: &[],
    };
    compose(&req).map_or_else(|_| (String::new(), String::new()), |c| (c.positive, c.negative))
}

/// Builds the verb runtime with the reference-sheet verb registered. Backend
/// registration from the keychain is deferred to a [`spawn_backend_key_op`]
/// with [`KeyOp::RegisterFromKeychain`] so the blocking keychain reads do not
/// stall the first paint; readiness arrives later over the channel.
///
/// `OpenAI` (gpt-image-2) registers at higher priority for image generation
/// (reference sheets); FAL is the only backend that also covers image-to-video
/// and background removal (the animation pipeline).
#[must_use]
pub fn build_runtime() -> Arc<VerbRuntime> {
    let runtime = VerbRuntime::new();
    if let Err(err) = runtime.register(GenerateReferenceSheetVerb::new()) {
        tracing::error!(%err, "failed to register reference-sheet verb");
    }
    Arc::new(runtime)
}

/// Registers backends from the keychain synchronously, returning whether at
/// least one is ready. For the headless CLI runner, which has no UI thread to
/// protect and must have its backends ready before it proceeds; the GUI defers
/// registration via [`spawn_backend_key_op`] instead.
#[must_use]
pub fn register_backends_blocking(runtime: &VerbRuntime) -> bool {
    let openai = try_register_openai(runtime);
    let fal = try_register_fal(runtime);
    openai || fal
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

/// Removes the stored API key for `backend` and unregisters its backend from
/// the runtime. A key that was never stored is treated as success. Blocking
/// I/O — call off the UI thread.
///
/// # Errors
/// Propagates keychain delete failures (other than "no key stored") as a
/// string.
pub fn clear_key(runtime: &VerbRuntime, backend: &str) -> Result<(), String> {
    match ApiKeyStore::delete(backend) {
        Ok(()) | Err(BackendError::ApiKeyNotFound(_)) => {}
        Err(err) => return Err(err.to_string()),
    }
    // Best-effort: the backend may have never registered (e.g. the key was
    // stored but registration failed), so a missing backend is not an error.
    let _ = runtime.unregister_backend(backend);
    Ok(())
}

/// Whether an API key is stored for `backend` in the OS keychain.
#[must_use]
pub fn key_configured(backend: &str) -> bool {
    ApiKeyStore::get(backend).is_ok()
}

/// Whether `backend` is currently registered in the verb runtime.
#[must_use]
pub fn backend_registered(runtime: &VerbRuntime, backend: &str) -> bool {
    runtime.list_backends().iter().any(|b| b.id == backend)
}

/// A keychain mutation to run off the UI thread, followed by backend
/// re-registration and a [`ShellMsg::BackendsRefreshed`] report.
pub enum KeyOp {
    /// Store `key` for `backend`, then register backends from the keychain.
    Save {
        /// Backend id ("openai" or "fal").
        backend: String,
        /// API key to store.
        key: String,
    },
    /// Clear `backend`'s stored key and unregister it.
    Clear {
        /// Backend id to clear.
        backend: String,
    },
    /// Register whatever keys already sit in the keychain (startup).
    RegisterFromKeychain,
}

/// Runs a keychain [`KeyOp`] on a blocking thread, then reports the resulting
/// per-backend configured state and readiness over `tx`. Keychain I/O blocks
/// (on Linux it can even pop a system unlock dialog), so it must never run on
/// the egui update thread; this keeps the window responsive and the first paint
/// from waiting on the credential store.
pub fn spawn_backend_key_op(handle: &Handle, runtime: Arc<VerbRuntime>, ctx: egui::Context, tx: Sender<ShellMsg>, op: KeyOp) {
    handle.spawn_blocking(move || {
        let error = match op {
            KeyOp::Save { backend, key } => match store_key(&backend, &key) {
                Ok(()) => {
                    try_register_openai(&runtime);
                    try_register_fal(&runtime);
                    None
                }
                Err(err) => Some(err),
            },
            KeyOp::Clear { backend } => clear_key(&runtime, &backend).err(),
            KeyOp::RegisterFromKeychain => {
                try_register_openai(&runtime);
                try_register_fal(&runtime);
                None
            }
        };
        let _ = tx.send(ShellMsg::BackendsRefreshed {
            openai_configured: key_configured(OPENAI_BACKEND_ID),
            fal_configured: key_configured(FAL_BACKEND_ID),
            ready: backend_registered(&runtime, OPENAI_BACKEND_ID) || backend_registered(&runtime, FAL_BACKEND_ID),
            error,
        });
        ctx.request_repaint();
    });
}

/// Parameters for a reference-sheet generation from the cockpit.
///
/// Beyond the minimal subject + structure, this carries the cockpit's context
/// and overrides: the project `style_notes` (folded into the prompt baseline),
/// drag-in conditioning references, a verbatim prompt/negative override when the
/// artist edited the composed text, and an optional fixed seed.
pub struct SheetJob {
    /// Project metadata for the verb context.
    pub meta: ProjectMetadata,
    /// Target sprite entity.
    pub entity_id: EntityId,
    /// Built-in composition Structure id (see [`TEMPLATES`]).
    pub structure_id: String,
    /// Free-typed subject description.
    pub prompt: String,
    /// Optional saved-prompt template id whose text (with variables substituted)
    /// composes into the request.
    pub prompt_id: Option<String>,
    /// Values for the picked template's `{variables}`.
    pub variable_values: BTreeMap<String, String>,
    /// Candidate count (clamped 1-4).
    pub num_variants: u32,
    /// Project `style_notes`, used as the prompt baseline when non-empty.
    pub style_notes: String,
    /// Drag-in conditioning references (subject/style anchors).
    pub references: Vec<ReferenceInput>,
    /// Verbatim positive prompt, sent in place of the composed text.
    pub prompt_override: Option<String>,
    /// Verbatim negative prompt.
    pub negative_override: Option<String>,
    /// Fixed RNG seed, or `None` for a fresh random seed each run.
    pub seed: Option<u64>,
}

impl SheetJob {
    /// Builds the minimal job — subject + structure, no overrides or references.
    pub fn minimal(meta: ProjectMetadata, entity_id: EntityId, structure_id: String, prompt: String, num_variants: u32) -> Self {
        Self {
            meta,
            entity_id,
            structure_id,
            prompt,
            prompt_id: None,
            variable_values: BTreeMap::new(),
            num_variants,
            style_notes: String::new(),
            references: Vec::new(),
            prompt_override: None,
            negative_override: None,
            seed: None,
        }
    }

    fn into_inputs(self) -> (GenerateReferenceSheetInputs, VerbContext) {
        let inputs = GenerateReferenceSheetInputs {
            entity_id: self.entity_id,
            structure_id: StructureId(self.structure_id),
            style_id: None,
            prompt_id: self.prompt_id.map(PromptId),
            variable_values: self.variable_values,
            inline_text: self.prompt,
            inline_negatives: String::new(),
            num_variants: self.num_variants.clamp(1, 4),
            quality: None,
            seed: self.seed,
            references: self.references,
            prompt_override: self.prompt_override,
            negative_override: self.negative_override,
        };
        let ctx = VerbContext::builder(self.meta)
            .with_composition_library(ProjectCompositionLibrary {
                style_notes: self.style_notes,
                ..Default::default()
            })
            .build();
        (inputs, ctx)
    }
}

/// One generated candidate plus its decoded PNG, ready for the gallery.
#[derive(Debug)]
pub struct GeneratedVariant {
    /// Full verb output: provenance, composition, the base64 image.
    pub output: SheetVariantOutput,
    /// Decoded PNG bytes (for texture upload and canvas preview).
    pub png: Vec<u8>,
}

/// The result of a cockpit generation: the candidates plus the run's reported cost.
pub struct SheetGeneration {
    /// Generated candidates with full provenance.
    pub variants: Vec<GeneratedVariant>,
    /// Reported run cost in USD, if the backend surfaced one.
    pub cost_usd: Option<f64>,
}

/// A progress update from a running reference-sheet generation, delivered to
/// the caller's callback as the verb streams. Status text drives the spinner;
/// partial frames are progressively sharper previews for the canvas.
pub enum SheetUpdate {
    /// One-line status text.
    Status(String),
    /// A streamed partial preview frame, decoded to RGBA.
    Partial(PixelData),
}

/// Runs the reference-sheet verb to completion and returns the candidate PNGs.
/// The headless runner uses this; the GUI uses [`run_sheet_rich`] for provenance.
pub async fn run_reference_sheet(runtime: &VerbRuntime, job: SheetJob, progress: &(dyn Fn(&str) + Sync)) -> Result<Vec<Vec<u8>>, String> {
    // The headless path only wants status text; drop streamed partial frames.
    let adapter = |update: SheetUpdate| {
        if let SheetUpdate::Status(message) = update {
            progress(&message);
        }
    };
    let generated = run_sheet_rich(runtime, job, &adapter).await?;
    Ok(generated.variants.into_iter().map(|v| v.png).collect())
}

/// Runs the reference-sheet verb and returns each candidate with full provenance
/// (composed prompt, user prompt, backend, model, seed) plus the run cost.
pub async fn run_sheet_rich(runtime: &VerbRuntime, job: SheetJob, progress: &(dyn Fn(SheetUpdate) + Sync)) -> Result<SheetGeneration, String> {
    let (inputs, vctx) = job.into_inputs();
    let verb_inputs = VerbInputs::from_struct(&inputs).map_err(|e| e.to_string())?;
    let mut invocation = runtime
        .invoke(&VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID), vctx, verb_inputs)
        .map_err(|e| e.to_string())?;

    while let Some(event) = invocation.next_progress().await {
        match event {
            VerbProgressEvent::Started { backend } => {
                progress(SheetUpdate::Status(format!("started on {}", backend.unwrap_or_else(|| "backend".into()))));
            }
            VerbProgressEvent::Step { message, .. } => progress(SheetUpdate::Status(message)),
            VerbProgressEvent::PartialPixels { pixels, .. } => progress(SheetUpdate::Partial(pixels)),
            _ => {}
        }
    }

    let preview = invocation.finish().await.map_err(|e| e.to_string())?;
    let cost_usd = (preview.output.actual_cost.usd_cents > 0.0).then(|| f64::from(preview.output.actual_cost.usd_cents) / 100.0);
    let variants = extract_rich(&preview.output);
    if variants.is_empty() {
        return Err("backend returned no variants".into());
    }
    Ok(SheetGeneration { variants, cost_usd })
}

/// Spawns a reference-sheet generation on the tokio runtime. Progress and the
/// final candidates (with provenance) arrive over `tx`; `ctx` is woken after
/// each message so the idle UI repaints.
pub fn spawn_reference_sheet(handle: &Handle, runtime: Arc<VerbRuntime>, ctx: egui::Context, tx: Sender<ShellMsg>, job: SheetJob) {
    handle.spawn(async move {
        let progress_tx = tx.clone();
        let progress_ctx = ctx.clone();
        let progress = move |update: SheetUpdate| {
            let msg = match update {
                SheetUpdate::Status(message) => ShellMsg::SheetProgress { fraction: None, message },
                SheetUpdate::Partial(pixels) => ShellMsg::SheetPartial { pixels },
            };
            let _ = progress_tx.send(msg);
            progress_ctx.request_repaint();
        };
        match run_sheet_rich(&runtime, job, &progress).await {
            Ok(generated) => {
                let _ = tx.send(ShellMsg::SheetDone {
                    variants: generated.variants,
                    cost_usd: generated.cost_usd,
                });
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
    /// Approved seed pose driving the image-to-video call. When `Some`, the
    /// studio's first-frame stage already produced and locked it, so
    /// [`generate_clip`] skips its own text-to-image step and animates this
    /// frame directly. When `None`, the clip path generates a seed itself.
    pub first_frame_png: Option<Vec<u8>>,
    /// Motion description (e.g. "walk cycle, side view").
    pub motion_prompt: String,
    /// Image-to-video model override (e.g. Seedance, Wan), or `None` for the
    /// backend default.
    pub i2v_model: Option<String>,
    /// Number of loop frames to land in the timeline.
    pub target_frames: u32,
    /// Playback frames per second.
    pub fps: u32,
    /// RNG seed for reproducibility. `None` uses a random seed each run.
    pub seed: Option<u64>,
}

/// The raw output of the Generate stage: the exact Seedance clip plus its
/// decoded frames. Held by the wizard so Review, Mark loop, and Pick frames all
/// run on the UI thread with no further backend calls.
pub struct ClipResult {
    /// Raw clip bytes, exactly as the backend returned them.
    pub clip: Vec<u8>,
    /// MIME type of the clip (drives the decode path).
    pub mime: String,
    /// Decoded RGBA frames, in order.
    pub frames: Vec<VideoFrame>,
}

/// Spawns only the Generate stage on the tokio runtime: first-frame generation
/// (FAL image-gen conditioned on the anchor) -> image-to-video clip (FAL) ->
/// decode. It retains what the old monolith discarded — the raw clip bytes, the
/// mime, and the decoded frames — and sends them as [`ShellMsg::ClipReady`].
/// Loop detection, frame picking, background removal, and normalize do not run
/// here; the shell drives them. `cancel` aborts a long i2v job. `epoch` tags
/// every message so the shell can drop a canceled or superseded run's results.
pub fn spawn_clip(handle: &Handle, runtime: Arc<VerbRuntime>, ctx: egui::Context, tx: Sender<ShellMsg>, job: AnimJob, cancel: CancellationToken, epoch: u64) {
    handle.spawn(async move {
        let progress_tx = tx.clone();
        let progress_ctx = ctx.clone();
        let progress = move |msg: &str| {
            let _ = progress_tx.send(ShellMsg::ClipProgress {
                epoch,
                message: msg.to_owned(),
            });
            progress_ctx.request_repaint();
        };
        match generate_clip(&runtime, &job, &cancel, &progress).await {
            Ok(ClipResult { clip, mime, frames }) => {
                let _ = tx.send(ShellMsg::ClipReady { epoch, clip, mime, frames });
            }
            Err(error) => {
                let _ = tx.send(ShellMsg::ClipFailed { epoch, error });
            }
        }
        ctx.request_repaint();
    });
}

/// Spawns an AI background-removal of one timeline cel on the tokio runtime.
/// `buffer_id` identifies the cel so the shell can apply the result to the
/// right buffer; the stripped PNG (or an error) arrives over `tx`.
pub fn spawn_bg_removal(
    handle: &Handle,
    runtime: Arc<VerbRuntime>,
    ctx: egui::Context,
    tx: Sender<ShellMsg>,
    buffer_id: PixelBufferId,
    png: Vec<u8>,
    cancel: CancellationToken,
) {
    handle.spawn(async move {
        let msg = match remove_background_ai(&runtime, png, &cancel).await {
            Ok(png) => ShellMsg::BgRemovalDone { buffer_id, png },
            Err(error) => ShellMsg::BgRemovalFailed { buffer_id, error },
        };
        let _ = tx.send(msg);
        ctx.request_repaint();
    });
}

/// A studio generation request: generate (text-to-image) or refine (inpaint a
/// masked region of a base image). Used by both the anchor and first-frame
/// threads — the anchor thread passes no reference images (it makes the anchor),
/// the first-frame thread passes the approved anchor.
pub enum FirstFrameJob {
    /// Generate `num_variants` candidates via text-to-image.
    Generate {
        /// Images the generation conditions on (the anchor for first frames,
        /// empty when generating the anchor itself).
        reference_images: Vec<Vec<u8>>,
        /// Target canvas size (width, height).
        canvas: (u32, u32),
        /// Positive prompt describing the desired image.
        prompt: String,
        /// Candidate count (clamped 1-4).
        num_variants: u32,
        /// Fixed RNG seed, or `None` for a fresh random seed.
        seed: Option<u64>,
    },
    /// Repaint a masked region of `base` via inpaint.
    Inpaint {
        /// The candidate being corrected, as PNG bytes.
        base: Vec<u8>,
        /// Edit mask as PNG bytes — white marks the region to repaint.
        mask: Vec<u8>,
        /// Images the refinement conditions on (the anchor for first frames,
        /// empty when refining the anchor itself).
        reference_images: Vec<Vec<u8>>,
        /// Instruction describing the fix.
        prompt: String,
        /// Candidate count (clamped 1-4).
        num_variants: u32,
    },
}

/// Runs a [`FirstFrameJob`] to completion, returning the candidate PNGs. The
/// generate arm needs `IMAGE_GENERATION`; the inpaint arm needs `IMAGE_INPAINT`.
/// Both condition on the anchor so the seed pose stays on-model.
pub async fn run_first_frame(runtime: &VerbRuntime, job: FirstFrameJob, cancel: &CancellationToken) -> Result<Vec<Vec<u8>>, String> {
    let (capability, request) = match job {
        FirstFrameJob::Generate {
            reference_images,
            canvas,
            prompt,
            num_variants,
            seed,
        } => {
            let (width, height) = canvas;
            let req = ImageGenRequest {
                model: None,
                prompt: format!("{prompt}, single sprite frame, side view, transparent background"),
                negative_prompt: Some("background, particles, glow, motion blur".into()),
                width,
                height,
                steps: None,
                seed,
                num_images: num_variants.clamp(1, 4),
                quality: None,
                style_image: None,
                reference_images,
            };
            (BackendCapabilities::IMAGE_GENERATION, InferenceRequest::ImageGeneration(req))
        }
        FirstFrameJob::Inpaint {
            base,
            mask,
            reference_images,
            prompt,
            num_variants,
        } => {
            let req = ImageEditRequest {
                model: None,
                image: base,
                mask: Some(mask),
                prompt,
                negative_prompt: Some("background, particles, glow, motion blur".into()),
                num_images: num_variants.clamp(1, 4),
                style_image: None,
                reference_images,
            };
            (BackendCapabilities::IMAGE_INPAINT, InferenceRequest::ImageInpaint(req))
        }
    };
    match invoke_fat(runtime, capability, request, cancel).await? {
        InferenceResponse::Image(r) if !r.images.is_empty() => Ok(r.images),
        InferenceResponse::Image(_) => Err("first-frame backend returned no image".into()),
        _ => Err("unexpected response for first-frame generation".into()),
    }
}

/// Spawns a [`FirstFrameJob`] on the tokio runtime. Progress and the final
/// candidate PNGs arrive over `tx`, tagged with `epoch` so a superseded or
/// canceled run's results can be dropped. `parent` and `append` thread the
/// lineage back so the studio gallery records where each candidate came from.
pub fn spawn_first_frame(
    handle: &Handle,
    runtime: Arc<VerbRuntime>,
    ctx: egui::Context,
    tx: Sender<ShellMsg>,
    job: FirstFrameJob,
    cancel: CancellationToken,
    epoch: u64,
    parent: Option<usize>,
    append: bool,
    target: GenTarget,
) {
    handle.spawn(async move {
        let _ = tx.send(ShellMsg::FirstFrameProgress {
            target,
            epoch,
            message: "generating".to_owned(),
        });
        ctx.request_repaint();
        match run_first_frame(&runtime, job, &cancel).await {
            Ok(images) => {
                let _ = tx.send(ShellMsg::FirstFrameDone {
                    target,
                    epoch,
                    images,
                    parent,
                    append,
                });
            }
            Err(error) => {
                let _ = tx.send(ShellMsg::FirstFrameFailed { target, epoch, error });
            }
        }
        ctx.request_repaint();
    });
}

/// The Generate stage in isolation: first frame -> image-to-video clip ->
/// decode, retaining the raw clip bytes, the mime, and the decoded frames. It
/// does **not** detect a loop, pick frames, remove backgrounds, or normalize —
/// the shell drives those on the UI thread. `cancel` aborts the long i2v await.
pub async fn generate_clip(runtime: &VerbRuntime, job: &AnimJob, cancel: &CancellationToken, progress: &(dyn Fn(&str) + Sync)) -> Result<ClipResult, String> {
    let (width, height) = job.canvas;

    // The studio's first-frame stage hands an approved seed pose down; honour
    // it and skip the text-to-image step. Only when none was supplied (the
    // headless path) does the clip generate its own seed frame.
    let first_frame = if let Some(approved) = job.first_frame_png.clone() {
        approved
    } else {
        progress("generating first frame");
        let req = ImageGenRequest {
            model: None,
            prompt: format!("{}, single sprite frame, side view, transparent background", job.motion_prompt),
            negative_prompt: Some("background, particles, glow, motion blur".into()),
            width,
            height,
            steps: None,
            seed: job.seed,
            num_images: 1,
            quality: None,
            style_image: None,
            reference_images: vec![job.anchor_png.clone()],
        };
        match invoke_fat(runtime, BackendCapabilities::IMAGE_GENERATION, InferenceRequest::ImageGeneration(req), cancel).await? {
            InferenceResponse::Image(r) => r.images.into_iter().next().ok_or_else(|| "first-frame backend returned no image".to_owned())?,
            _ => return Err("unexpected response for first-frame generation".into()),
        }
    };

    progress("generating clip (image-to-video)");
    let (clip, mime) = {
        let req = ImageToVideoRequest {
            model: job.i2v_model.clone(),
            image: first_frame,
            prompt: job.motion_prompt.clone(),
            negative_prompt: Some("pivots, quarter-turns, background, particles, glow".into()),
            num_frames: (job.target_frames * 4).max(16),
            fps: job.fps,
            seed: job.seed,
        };
        match invoke_fat(runtime, BackendCapabilities::IMAGE_TO_VIDEO, InferenceRequest::ImageToVideo(req), cancel).await? {
            InferenceResponse::Video(v) => (v.clip, v.mime),
            _ => return Err("unexpected response for image-to-video".into()),
        }
    };

    progress("decoding clip");
    let decode_fps = job.fps;
    // Clip decode (mp4/H.264, gif, apng) is blocking — keep it off the async
    // worker. Clone the bytes into the task so the raw clip survives to return.
    let clip_for_decode = clip.clone();
    let mime_for_decode = mime.clone();
    let frames = tokio::task::spawn_blocking(move || anim::decode_clip(&clip_for_decode, &mime_for_decode, decode_fps))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    if frames.is_empty() {
        return Err("clip decoded to zero frames".into());
    }
    Ok(ClipResult { clip, mime, frames })
}

/// The full pipeline for the headless runner: generate the clip, auto-pick the
/// loop, strip backgrounds, and normalize, returning the finished loop frames
/// and per-frame duration. The GUI wizard runs these stages by hand instead.
pub async fn run_animation(runtime: &VerbRuntime, job: &AnimJob, progress: &(dyn Fn(&str) + Sync)) -> Result<(Vec<PixelBuffer>, u32), String> {
    let (width, height) = job.canvas;
    let cancel = CancellationToken::new();
    let ClipResult { frames, .. } = generate_clip(runtime, job, &cancel, progress).await?;

    let markers = anim::auto_loop_markers(&frames);
    let picks = anim::pick_loop_frames(&frames, markers, job.target_frames as usize);
    if picks.is_empty() {
        return Err("no loop frames picked".into());
    }

    progress("removing backgrounds");
    let mut picked: Vec<VideoFrame> = Vec::with_capacity(picks.len());
    for &idx in &picks {
        let frame = frames[idx].clone();
        // Best-effort: keep the original frame if background removal fails.
        let stripped = strip_background(runtime, &frame, &cancel).await.unwrap_or(frame);
        picked.push(stripped);
    }

    progress("normalizing");
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

/// Removes the background of one PNG via the `BACKGROUND_REMOVAL` capability
/// (FAL Bria), returning the stripped PNG bytes. The user-invoked fallback the
/// timeline background-removal op falls back to when keying misses.
///
/// # Errors
/// Propagates backend errors and an unexpected response shape as a string.
pub async fn remove_background_ai(runtime: &VerbRuntime, png: Vec<u8>, cancel: &CancellationToken) -> Result<Vec<u8>, String> {
    let req = BackgroundRemovalRequest { model: None, image: png };
    match invoke_fat(
        runtime,
        BackendCapabilities::BACKGROUND_REMOVAL,
        InferenceRequest::BackgroundRemoval(req),
        cancel,
    )
    .await?
    {
        InferenceResponse::Image(r) => r.images.into_iter().next().ok_or_else(|| "background removal returned no image".to_owned()),
        _ => Err("unexpected response for background removal".into()),
    }
}

/// Background-removes a single decoded frame via FAL. Returns `None` on any
/// failure so the caller can fall back to the original frame.
async fn strip_background(runtime: &VerbRuntime, frame: &VideoFrame, cancel: &CancellationToken) -> Option<VideoFrame> {
    let png = anim::encode_png(frame)?;
    let out = remove_background_ai(runtime, png, cancel).await.ok()?;
    anim::decode_png(&out, frame.timestamp_ms)
}

/// Selects a backend by capability and invokes a typed inference request
/// through the fat backend behind the [`BackendProxy`], honouring `cancel`.
async fn invoke_fat(
    runtime: &VerbRuntime,
    capability: BackendCapabilities,
    request: InferenceRequest,
    cancel: &CancellationToken,
) -> Result<InferenceResponse, String> {
    let thin = runtime.select_backend(capability, &VerbId::new("shell.animation")).map_err(|e| e.to_string())?;
    let proxy = thin
        .as_any()
        .downcast_ref::<BackendProxy>()
        .ok_or_else(|| "registered backend is not a BackendProxy".to_owned())?;
    proxy
        .fat()
        .invoke(request, VerbProgress::discard(), cancel.clone())
        .await
        .map_err(|e| e.to_string())
}

/// Pulls each sheet variant — full provenance plus decoded PNG — out of the
/// verb output. Variants whose base64 image fails to decode are dropped.
fn extract_rich(output: &pixhaus_ai::plugin::VerbOutput) -> Vec<GeneratedVariant> {
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
            .into_iter()
            .filter_map(|output| {
                let png = base64::engine::general_purpose::STANDARD.decode(&output.image_b64).ok()?;
                Some(GeneratedVariant { output, png })
            })
            .collect();
    }
    Vec::new()
}
