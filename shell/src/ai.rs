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
pub use pixhaus_ai::backends::ImageQuality;
use pixhaus_ai::backends::fal::FalBackend;
use pixhaus_ai::backends::openai::OpenAiBackend;
use pixhaus_ai::backends::{
    ApiKeyStore, BackendError, BackendProxy, BackgroundRemovalRequest, ImageEditRequest, ImageGenRequest, ImageToVideoRequest, InferenceRequest,
    InferenceResponse,
};
use pixhaus_ai::compose::builtins::{BuiltinLibrary, STRUCTURE_SINGLE_ID, baseline_for};
use pixhaus_ai::compose::{ComposeRequest, VariableAxis, compose, identity_lock_clause};
use pixhaus_ai::plugin::{
    BackendCapabilities, CompositionLibraryView, PixelData, ProjectCompositionLibrary, VerbContext, VerbEffect, VerbId, VerbInputs, VerbProgress,
    VerbProgressEvent, VerbRuntime,
};
use pixhaus_ai::verbs::reference_sheet::{
    GENERATE_REFERENCE_SHEET_VERB_ID, GENERATE_SHEET_EFFECT_NAME, GenerateReferenceSheetInputs, GenerateReferenceSheetVerb, GenerateSheetPayload,
    ReferenceInput, SheetVariantOutput,
};
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::library::ai::ProjectAi;
use pixhaus_core::project::library::composition::{ArtStyleKind, PromptId, PromptTemplate, PromptVariable, Structure, StructureId, Style, StyleId};
use pixhaus_core::project::{AnchorDirection, EntityId, PixelBufferId, ProjectMetadata, SheetVariantId};
use pixhaus_core::transforms::normalize::{NormalizeOptions, normalize_frames};
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

/// The built-in free-form structure id — the cockpit's default output.
pub const SINGLE_STRUCTURE_ID: &str = STRUCTURE_SINGLE_ID;

/// The (id, display-name) of every resolvable composition structure, with the
/// free-form `Single` first so it reads as the natural default in the picker.
/// Project structures shadow built-ins of the same id.
#[must_use]
pub fn structure_options(project: &ProjectAi) -> Vec<(String, String)> {
    let lib = BuiltinLibrary::load();
    let mut map: BTreeMap<String, String> = lib.structures.values().map(|s| (s.id.0.clone(), s.name.clone())).collect();
    for s in &project.structures {
        map.insert(s.id.0.clone(), s.name.clone());
    }
    let mut out: Vec<(String, String)> = map.into_iter().collect();
    out.sort_by_key(|(id, _)| u8::from(id != STRUCTURE_SINGLE_ID));
    out
}

/// The variables of a saved/built-in prompt template, for the cockpit dials. A
/// project template shadows the built-in of the same id. Empty for an unknown id
/// or a template with no variables.
#[must_use]
pub fn prompt_variables(project: &ProjectAi, prompt_id: &str) -> Vec<PromptVariable> {
    if let Some(p) = project.prompts.iter().find(|p| p.id.0 == prompt_id) {
        return p.variables.clone();
    }
    BuiltinLibrary::load()
        .prompts
        .get(&PromptId(prompt_id.to_owned()))
        .map(|p| p.variables.clone())
        .unwrap_or_default()
}

/// The (id, name) of every saved/built-in prompt template, name-sorted, for the
/// cockpit template picker. Project templates shadow built-ins of the same id.
#[must_use]
pub fn prompt_options(project: &ProjectAi) -> Vec<(String, String)> {
    let lib = BuiltinLibrary::load();
    let mut map: BTreeMap<String, String> = lib.prompts.values().map(|p| (p.id.0.clone(), p.name.clone())).collect();
    for p in &project.prompts {
        map.insert(p.id.0.clone(), p.name.clone());
    }
    let mut out: Vec<(String, String)> = map.into_iter().collect();
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

/// Composes the live positive/negative prompt preview for the cockpit, on the
/// UI thread, with no backend call. Mirrors what the verb sends: the project
/// `style_notes` (or the built-in default) lead, then the picked Style's
/// modifiers, the structure's layout prose, the picked template (with variables
/// substituted), then the subject. The picked Style also folds its look
/// negatives in. Returns empty strings for an unknown structure.
#[must_use]
pub fn compose_preview(
    project: &ProjectAi,
    structure_id: &str,
    subject: &str,
    style_notes: &str,
    prompt_id: Option<&str>,
    style_id: Option<&str>,
    variables: &BTreeMap<String, String>,
) -> (String, String) {
    let builtins = BuiltinLibrary::load();
    let view = CompositionLibraryView::new(&project.structures, &project.styles, &project.prompts, builtins);
    let Some(structure) = view.structure(&StructureId(structure_id.to_owned())) else {
        return (String::new(), String::new());
    };
    let prompt = prompt_id.and_then(|id| view.prompt(&PromptId(id.to_owned())));
    let style = style_id.and_then(|id| view.style(&StyleId(id.to_owned())));
    // Mirror the verb: house style_notes lead when set, else the baseline
    // derives from the picked style's art-style kind (PixelArt by default).
    let resolved_kind = style.map_or(ArtStyleKind::default(), |s| s.kind);
    let baseline = if style_notes.trim().is_empty() {
        baseline_for(resolved_kind)
    } else {
        style_notes
    };
    let empty = BTreeMap::new();
    let req = ComposeRequest {
        baseline,
        structure,
        style,
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

/// The (id, name) of every saved/built-in look Style, name-sorted, for the
/// cockpit and library style pickers. Project Styles shadow built-ins of the
/// same id.
#[must_use]
pub fn style_options(project: &ProjectAi) -> Vec<(String, String)> {
    let lib = BuiltinLibrary::load();
    let mut map: BTreeMap<String, String> = lib.styles.values().map(|s| (s.id.0.clone(), s.name.clone())).collect();
    for s in &project.styles {
        map.insert(s.id.0.clone(), s.name.clone());
    }
    let mut out: Vec<(String, String)> = map.into_iter().collect();
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
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
    /// Project composition structures, so a saved structure resolves in the verb.
    pub structures: Vec<Structure>,
    /// Project composition styles, so a saved style resolves in the verb.
    pub styles: Vec<Style>,
    /// Project prompt templates, so a saved template resolves in the verb.
    pub prompts: Vec<PromptTemplate>,
    /// Drag-in conditioning references (subject/style anchors).
    pub references: Vec<ReferenceInput>,
    /// Verbatim positive prompt, sent in place of the composed text.
    pub prompt_override: Option<String>,
    /// Verbatim negative prompt.
    pub negative_override: Option<String>,
    /// Fixed RNG seed, or `None` for a fresh random seed each run.
    pub seed: Option<u64>,
    /// Explicit output size `(width, height)` for free-form anchors. Ignored by
    /// Paneled structures, which keep their declared canvas.
    pub target_size: Option<(u32, u32)>,
    /// Quality tier for the run. `None` lets the router/provider choose; a
    /// promotion sets [`ImageQuality::High`] for the final render.
    pub quality: Option<ImageQuality>,
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
            structures: Vec::new(),
            styles: Vec::new(),
            prompts: Vec::new(),
            references: Vec::new(),
            prompt_override: None,
            negative_override: None,
            seed: None,
            target_size: None,
            quality: None,
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
            quality: self.quality,
            seed: self.seed,
            references: self.references,
            prompt_override: self.prompt_override,
            negative_override: self.negative_override,
            target_size: self.target_size,
        };
        let ctx = VerbContext::builder(self.meta)
            .with_composition_library(ProjectCompositionLibrary {
                structures: self.structures,
                styles: self.styles,
                prompts: self.prompts,
                style_notes: self.style_notes,
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
/// `job_id` is the durable-queue id the terminal messages carry so the shell
/// marks the right record `Done`/`Error`.
pub fn spawn_clip(
    handle: &Handle,
    runtime: Arc<VerbRuntime>,
    ctx: egui::Context,
    tx: Sender<ShellMsg>,
    job: AnimJob,
    cancel: CancellationToken,
    epoch: u64,
    job_id: u64,
) {
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
                let _ = tx.send(ShellMsg::ClipReady {
                    epoch,
                    job_id,
                    clip,
                    mime,
                    frames,
                });
            }
            Err(error) => {
                let _ = tx.send(ShellMsg::ClipFailed { epoch, job_id, error });
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

/// Assembles a derivation prompt: the base subject, the framing `suffix`, and —
/// only when the pass conditions on an anchor (`conditioned`) — the Pose
/// identity-lock clause naming the reference and freeing only the pose. An
/// IP-Adapter weight pulls globally toward the anchor but cannot say "keep the
/// face and palette, change the pose", so the prose states it; from-scratch
/// passes (`conditioned` false) skip it. The clause rides at the tail, mirroring
/// where `compose` appends `operation_hint`.
///
/// Used only by `generate_clip`'s self-seed, whose `motion_prompt` carries no
/// lock, so the single append here is correct. `run_first_frame`'s Generate arm
/// does NOT use this: it appends the suffix via [`generate_frame_prompt`] and
/// the Pose lock via [`lock_when_conditioned`], which together avoid
/// double-appending or contradicting the free axis.
fn derive_prompt(base: &str, suffix: &str, conditioned: bool) -> String {
    let lock = if conditioned {
        format!(", {}", identity_lock_clause(VariableAxis::Pose))
    } else {
        String::new()
    };
    format!("{base}, {suffix}{lock}")
}

/// Appends only the single-sprite framing suffix to a Generate-arm `base` prompt.
///
/// This adds no identity-lock clause — the Generate arm appends that separately
/// via [`lock_when_conditioned`] so it can key off `reference_images` and the
/// clause the base already carries. The two callers differ: the studio
/// neutral/directional passes arrive with the axis-appropriate lock baked into
/// `base` (Pose for neutral, Facing for directional), while the studio seed-pose
/// path (`studio::start_gen`) arrives lock-free.
pub(crate) fn generate_frame_prompt(base: &str) -> String {
    format!("{base}, single sprite frame, side view")
}

/// The substring that marks a prompt as already carrying an identity-lock
/// clause from [`identity_lock_clause`]. Used to decide whether the Generate arm
/// should add the Pose lock or leave the existing axis lock untouched.
const IDENTITY_LOCK_MARKER: &str = "keep the same character from the reference image";

/// Appends the Pose identity-lock clause to a Generate-arm prompt when the pass
/// conditions on an anchor and the prompt does not already carry a lock.
///
/// The Generate arm has three callers, and only the prompt-plus-references pair
/// distinguishes them. The studio neutral/directional cascade passes arrive
/// already locked (Pose for neutral, Facing for directional), so the marker
/// guard leaves them alone — re-appending a Pose lock would double it on neutral
/// and contradict the Facing axis on directional turns. The studio seed-pose
/// path (`studio::start_gen`) always conditions on the anchor yet arrives
/// lock-free, so it gets the Pose lock here. The from-scratch headless path
/// passes no reference, so `has_reference` is false and it stays lock-free.
pub(crate) fn lock_when_conditioned(prompt: String, has_reference: bool) -> String {
    if has_reference && !prompt.contains(IDENTITY_LOCK_MARKER) {
        format!("{prompt}, {}", identity_lock_clause(VariableAxis::Pose))
    } else {
        prompt
    }
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
            // The background is requested in the positive prompt (a flat key
            // colour, built in the studio); keep "background" out of the
            // negatives so the model actually paints the solid fill.
            let has_reference = !reference_images.is_empty();
            let req = ImageGenRequest {
                model: None,
                // Framing suffix, then the Pose lock only when this conditions on
                // an anchor and the prompt is not already locked. The cascade
                // passes arrive pre-locked and are left alone; the studio
                // seed-pose path arrives lock-free and gets the Pose lock.
                prompt: lock_when_conditioned(generate_frame_prompt(&prompt), has_reference),
                negative_prompt: Some("scenery, environment, particles, glow, motion blur, gradient, drop shadow".into()),
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
            // Refining a region against an anchor locks identity too — the repaint
            // must keep the character. Refining the anchor itself (no references)
            // carries no lock. The inpaint instruction has no framing suffix, so
            // append the clause directly rather than through `derive_prompt`.
            let prompt = if reference_images.is_empty() {
                prompt
            } else {
                format!("{prompt}, {}", identity_lock_clause(VariableAxis::Pose))
            };
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
) {
    handle.spawn(async move {
        let _ = tx.send(ShellMsg::FirstFrameProgress {
            epoch,
            message: "generating".to_owned(),
        });
        ctx.request_repaint();
        match run_first_frame(&runtime, job, &cancel).await {
            Ok(images) => {
                let _ = tx.send(ShellMsg::FirstFrameDone { epoch, images, parent, append });
            }
            Err(error) => {
                let _ = tx.send(ShellMsg::FirstFrameFailed { epoch, error });
            }
        }
        ctx.request_repaint();
    });
}

/// Spawns an anchor inpaint-refine (`job` must be a [`FirstFrameJob::Inpaint`]).
/// The single repainted image arrives over `tx` as a [`ShellMsg::AnchorRefineDone`]
/// tagged with `epoch`; `parent` is the gallery index it descends from.
pub fn spawn_anchor_refine(
    handle: &Handle,
    runtime: Arc<VerbRuntime>,
    ctx: egui::Context,
    tx: Sender<ShellMsg>,
    job: FirstFrameJob,
    epoch: u64,
    parent: usize,
) {
    handle.spawn(async move {
        let cancel = CancellationToken::new();
        let msg = match run_first_frame(&runtime, job, &cancel).await {
            Ok(images) => match images.into_iter().next() {
                Some(image) => ShellMsg::AnchorRefineDone { epoch, parent, image },
                None => ShellMsg::AnchorRefineFailed {
                    epoch,
                    error: "the backend returned no image".to_owned(),
                },
            },
            Err(error) => ShellMsg::AnchorRefineFailed { epoch, error },
        };
        let _ = tx.send(msg);
        ctx.request_repaint();
    });
}

/// Spawns the Tier-1 neutral-anchor derivation: a text-to-image pass that
/// regenerates the canonical pose as a clean, effect-free neutral, conditioned
/// on the canonical sheet so the result stays on-model. `job` must be a
/// [`FirstFrameJob::Generate`] built with the neutral-pose prompt and the
/// canonical anchor as its reference image. The single derived image arrives
/// over `tx` as a [`ShellMsg::NeutralDerived`] tagged with `epoch`, carrying the
/// `entity_id` and `canonical_id` the UI thread needs to stamp and store it; a
/// failure (or empty result) arrives as [`ShellMsg::NeutralFailed`].
pub fn spawn_neutral(
    handle: &Handle,
    runtime: Arc<VerbRuntime>,
    ctx: egui::Context,
    tx: Sender<ShellMsg>,
    job: FirstFrameJob,
    cancel: CancellationToken,
    epoch: u64,
    entity_id: EntityId,
    canonical_id: SheetVariantId,
) {
    handle.spawn(async move {
        let msg = match run_first_frame(&runtime, job, &cancel).await {
            Ok(images) => match images.into_iter().next() {
                Some(image) => ShellMsg::NeutralDerived {
                    epoch,
                    entity_id,
                    canonical_id,
                    image,
                },
                None => ShellMsg::NeutralFailed {
                    epoch,
                    error: "the backend returned no image".to_owned(),
                },
            },
            Err(error) => ShellMsg::NeutralFailed { epoch, error },
        };
        let _ = tx.send(msg);
        ctx.request_repaint();
    });
}

/// Spawns a directional-anchor derivation: a text-to-image pass that regenerates
/// the neutral pose seen from `direction`, conditioned on the neutral anchor so
/// the result stays on-model. `job` must be a [`FirstFrameJob::Generate`] built
/// with the directional-view prompt and the neutral image as its reference. The
/// single derived image arrives over `tx` as a [`ShellMsg::DirectionalDerived`]
/// tagged with `epoch`, carrying the `entity_id`, `direction`, and `neutral_id`
/// the UI thread needs to stamp the `parent_variant_id` edge and store it; a
/// failure (or empty result) arrives as [`ShellMsg::DirectionalFailed`]. East is
/// never passed here — it is the flip of west, enabled, not generated.
#[allow(clippy::too_many_arguments)]
pub fn spawn_directional(
    handle: &Handle,
    runtime: Arc<VerbRuntime>,
    ctx: egui::Context,
    tx: Sender<ShellMsg>,
    job: FirstFrameJob,
    cancel: CancellationToken,
    epoch: u64,
    entity_id: EntityId,
    direction: AnchorDirection,
    neutral_id: SheetVariantId,
) {
    handle.spawn(async move {
        let msg = match run_first_frame(&runtime, job, &cancel).await {
            Ok(images) => match images.into_iter().next() {
                Some(image) => ShellMsg::DirectionalDerived {
                    epoch,
                    entity_id,
                    direction,
                    neutral_id,
                    image,
                },
                None => ShellMsg::DirectionalFailed {
                    epoch,
                    error: "the backend returned no image".to_owned(),
                },
            },
            Err(error) => ShellMsg::DirectionalFailed { epoch, error },
        };
        let _ = tx.send(msg);
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
        // The self-seed always conditions on the approved anchor, so it carries
        // the identity-lock clause to hold the character across the phase change.
        let req = ImageGenRequest {
            model: None,
            prompt: derive_prompt(&job.motion_prompt, "single sprite frame, side view, transparent background", true),
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use pixhaus_core::project::library::ai::ProjectAi;
    use pixhaus_core::project::library::composition::{ArtStyleKind, Structure, StructureId, StructureOutput, Style, StyleId};

    use pixhaus_ai::compose::{VariableAxis, identity_lock_clause};

    use super::{STRUCTURE_SINGLE_ID, compose_preview, derive_prompt, generate_frame_prompt, lock_when_conditioned, structure_options, style_options};

    #[test]
    fn lock_when_conditioned_adds_the_pose_lock_once_on_an_unlocked_conditioned_prompt() {
        let lock = identity_lock_clause(VariableAxis::Pose);
        let marker = "keep the same character from the reference image";

        // Conditioned and unlocked: the seed-pose case. The Pose lock rides the tail.
        let seed = lock_when_conditioned(generate_frame_prompt("character seed pose, side view, facing left"), true);
        assert!(seed.ends_with(&lock), "the Pose lock rides the tail: {seed}");
        assert_eq!(seed.matches(marker).count(), 1, "exactly one identity-lock clause: {seed}");

        // Conditioned but already locked: the cascade case. The marker guard leaves it alone.
        let already = format!("a hero, {lock}");
        let cascade = lock_when_conditioned(generate_frame_prompt(&already), true);
        assert_eq!(cascade.matches(marker).count(), 1, "a pre-locked prompt is not double-locked: {cascade}");

        // No reference: the from-scratch headless case. No lock.
        let scratch = lock_when_conditioned(generate_frame_prompt("a hero"), false);
        assert!(!scratch.contains(marker), "an unconditioned prompt carries no lock: {scratch}");
    }

    #[test]
    fn derive_prompt_appends_lock_only_when_conditioned() {
        let lock = identity_lock_clause(VariableAxis::Pose);

        // Conditioned: base, suffix, then the identity-lock clause at the tail.
        let conditioned = derive_prompt("a hero", "single sprite frame, side view", true);
        assert_eq!(conditioned, format!("a hero, single sprite frame, side view, {lock}"));
        assert!(conditioned.contains("keep the same character from the reference image"), "the lock clause rides the tail: {conditioned}");

        // From scratch: base and suffix only, no lock.
        let unconditioned = derive_prompt("a hero", "single sprite frame, side view", false);
        assert_eq!(unconditioned, "a hero, single sprite frame, side view");
        assert!(!unconditioned.contains("keep the same character"), "an unconditioned prompt carries no lock: {unconditioned}");
    }

    #[test]
    fn structure_options_let_project_records_shadow_builtins() {
        let mut project = ProjectAi::default();
        // A project structure that reuses a built-in id overrides its name.
        project.structures.push(Structure {
            id: StructureId(STRUCTURE_SINGLE_ID.to_owned()),
            name: "My single".to_owned(),
            output: StructureOutput::Single,
            layout_negatives: String::new(),
        });
        let opts = structure_options(&project);
        let single = opts.iter().find(|(id, _)| id == STRUCTURE_SINGLE_ID).expect("single structure present");
        assert_eq!(single.1, "My single");
        // The free-form structure still sorts first.
        assert_eq!(opts.first().map(|(id, _)| id.as_str()), Some(STRUCTURE_SINGLE_ID));
    }

    #[test]
    fn style_options_include_project_styles() {
        let mut project = ProjectAi::default();
        project.styles.push(Style {
            id: StyleId("project.style.1".to_owned()),
            name: "Zzz late".to_owned(),
            kind: ArtStyleKind::default(),
            modifiers: String::new(),
            look_negatives: String::new(),
            model_pref: None,
            quality: None,
        });
        let opts = style_options(&project);
        assert!(opts.iter().any(|(id, name)| id == "project.style.1" && name == "Zzz late"));
        // Name-sorted: a "Zzz" style lands last.
        assert_eq!(opts.last().map(|(_, name)| name.as_str()), Some("Zzz late"));
    }

    #[test]
    fn style_options_include_the_builtin_art_styles_name_sorted() {
        let project = ProjectAi::default();
        let opts = style_options(&project);
        // The built-in art-style taxonomy surfaces in the picker.
        for name in ["Pixel art", "Retro pixel", "Pixel inspired", "Clean HD", "Map style", "Default"] {
            assert!(opts.iter().any(|(_, n)| n == name), "built-in style `{name}` must surface, got {opts:?}");
        }
        // The list is name-sorted, so display names are non-decreasing.
        let names: Vec<&str> = opts.iter().map(|(_, n)| n.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted, "style options must be name-sorted");
    }

    #[test]
    fn compose_preview_folds_subject_and_style_into_the_prompt() {
        let mut project = ProjectAi::default();
        project.styles.push(Style {
            id: StyleId("project.style.snes".to_owned()),
            name: "SNES".to_owned(),
            kind: ArtStyleKind::default(),
            modifiers: "16-bit jrpg palette".to_owned(),
            look_negatives: "photoreal".to_owned(),
            model_pref: None,
            quality: None,
        });
        let vars = BTreeMap::new();

        let (pos, neg) = compose_preview(
            &project,
            STRUCTURE_SINGLE_ID,
            "a knight",
            "house style: chunky outlines",
            None,
            Some("project.style.snes"),
            &vars,
        );
        assert!(!pos.is_empty(), "a known structure yields a non-empty positive prompt");
        assert!(pos.contains("a knight"), "the subject is folded in: {pos}");
        assert!(pos.contains("16-bit jrpg palette"), "the picked style's modifiers fold in: {pos}");
        assert!(neg.contains("photoreal"), "the picked style's look negatives fold in: {neg}");

        // Editing the subject changes the output.
        let (pos2, _) = compose_preview(&project, STRUCTURE_SINGLE_ID, "a dragon", "", None, None, &vars);
        assert_ne!(pos, pos2, "a different subject yields a different prompt");
        assert!(pos2.contains("a dragon"));
    }

    #[test]
    fn compose_preview_returns_empty_for_an_unknown_structure() {
        let project = ProjectAi::default();
        let vars = BTreeMap::new();
        let (pos, neg) = compose_preview(&project, "does.not.exist", "a knight", "", None, None, &vars);
        assert!(pos.is_empty());
        assert!(neg.is_empty());
    }
}
