//! On-device FLUX.2 backend.
//!
//! [`LocalFluxBackend`] implements [`super::InferenceBackend`] over a loaded
//! `pixhaus_flux::LoadedModel`: text-to-image ([`InferenceRequest::ImageGeneration`])
//! and image-to-image edit / inpaint ([`InferenceRequest::ImageEdit`] /
//! [`InferenceRequest::ImageInpaint`]). It runs locally with no API key and no
//! per-image cost. Every other request arm returns
//! [`BackendError::UnsupportedCapability`] — the registry never routes them here
//! because the capability bits don't match, but the backend guards anyway.
//!
//! # The blocking / progress / cancel bridge
//!
//! Candle inference is synchronous and CPU/GPU-bound, so `invoke` offloads the
//! load-and-denoise to [`tokio::task::spawn_blocking`] and bridges progress back
//! to the async [`VerbProgress`] channel:
//!
//! - **Load once.** The multi-GB model is cached behind
//!   `Arc<OnceCell<Mutex<LoadedModel>>>` inside the [`FluxRunner`], so the load
//!   runs on the first `invoke` only. The `parking_lot::Mutex` serializes GPU
//!   access — a consumer GPU can't run two FLUX jobs without OOM — and is locked
//!   **only inside** the blocking closure, never across an `.await`.
//! - **Progress without async-in-blocking.** [`VerbProgress::send`] is async; the
//!   blocking closure can't call it. A `std::sync::mpsc` carries [`FluxTick`]s to
//!   a small async task ([`forward_ticks`]) that owns the real `VerbProgress` and
//!   does the `send(Step { fraction, message }).await`.
//! - **Cancellation.** Candle's sampler has no built-in cancel. The per-step
//!   callback checks the [`CancellationToken`] before each step and returns
//!   `false` to break; worst-case latency to honour a cancel is one step. The
//!   backend also re-checks after the blocking join and maps to
//!   [`BackendError::Cancelled`].
//!
//! # Provenance
//!
//! The reference pipeline's pixel-layer watermarking is deliberately omitted — it
//! would corrupt exact pixel output, which is antithetical to a pixel-art editor.
//! Lineage (prompt / backend / model / seed) is recorded instead: cloud Create
//! generations go through the studio's reference-sheet provenance, and the
//! editor-mode local actions (text-to-image, image-to-image, inpaint) that land
//! directly on the canvas record into the project's AI lineage at the landing
//! site (`shell::local_ai::ShellApp::record_local_gen_lineage`). The backend
//! itself stays stateless — it owns no project, so it cannot write lineage.

#![cfg(feature = "local-flux")]

use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::mpsc;
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::Mutex;
use pixhaus_flux::{AdvancedSampling, FluxError, LoadedModel};
use tokio_util::sync::CancellationToken;

use super::{
    BackendError, ImageEditRequest, ImageGenRequest, ImageGenResponse, InferenceBackend, InferenceRequest, InferenceResponse, Result, VerbProgress,
};
use crate::plugin::descriptor::{BackendCapabilities, CostEstimate};
use crate::plugin::progress::VerbProgressEvent;

// Re-export the load-side types the shell registration path names, so callers
// reach them through `pixhaus_ai::backends::local_flux` without a direct
// `pixhaus_flux` dependency.
pub use pixhaus_flux::{DeviceChoice, DevicePref, FLUX2_KLEIN_MODEL_ID, FluxRequest, ModelStore};

/// Stable model id reported in [`ImageGenResponse::model`] and used as the
/// `model` pin so a request can target the local backend even when a cloud
/// backend is also registered.
pub const LOCAL_FLUX_MODEL_ID: &str = "flux2-klein-4b";

/// The two run modes the runner dispatches on. Mapped from the request arm by
/// [`LocalFluxBackend::invoke`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FluxMode {
    /// Text-to-image (no init image).
    TextToImage,
    /// Image-to-image edit or masked inpaint (init image present).
    ImageToImage,
}

/// A progress tick from the blocking sampler, forwarded to the async progress
/// channel by [`forward_ticks`]. The `std::sync::mpsc` carrying these is the only
/// channel a blocking closure can write — [`VerbProgress::send`] is async.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FluxTick {
    /// The model is being loaded (the one-time multi-GB step on first invoke).
    Loading,
    /// A sampling step is about to run: `step` of `total` (1-based for display).
    Step {
        /// 1-based index of the step about to run.
        step: usize,
        /// Total step count for the schedule.
        total: usize,
    },
}

/// The boundary trait the backend runs against, so the spawn/progress/cancel
/// bridge is unit-testable without weights or a GPU.
///
/// The real implementation ([`LoadedFluxRunner`]) lazily loads and caches a
/// `LoadedModel`; a `mockall` double stands in for the tests. `run` is a single
/// blocking call: it owns the lazy-load, the per-step cancel callback, and the
/// decode to PNG bytes.
pub trait FluxRunner: Send + Sync + 'static {
    /// Run one generation. `tick` is sent a [`FluxTick`] before load and before
    /// each sampling step; `should_continue` is polled before each step and
    /// returning `false` cancels the run (the sampler breaks early). Returns one
    /// PNG buffer per requested image.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] on a load, tensor, encode, or decode failure.
    fn run(&self, mode: FluxMode, request: FluxRequest, tick: &mpsc::Sender<FluxTick>, should_continue: &(dyn Fn() -> bool + Send + Sync)) -> Result<Vec<Vec<u8>>>;
}

/// The production [`FluxRunner`]: a lazily-loaded, GPU-serialized `LoadedModel`.
///
/// The model is loaded once behind `OnceLock<Mutex<LoadedModel>>` (the std
/// `OnceLock` initialises exactly once even under concurrent first calls), then
/// every run takes the `parking_lot::Mutex` so only one denoise touches the GPU
/// at a time. The lock is taken only inside [`FluxRunner::run`], which the backend
/// invokes inside `spawn_blocking` — never across an `.await`.
pub struct LoadedFluxRunner {
    store: ModelStore,
    device: DeviceChoice,
    model: OnceLock<Mutex<LoadedModel>>,
}

impl LoadedFluxRunner {
    /// Build a runner over a presence-checked store and a resolved device. The
    /// model is **not** loaded here — the first [`FluxRunner::run`] loads it.
    #[must_use]
    pub fn new(store: ModelStore, device: DeviceChoice) -> Self {
        Self {
            store,
            device,
            model: OnceLock::new(),
        }
    }

    /// Get the cached model, loading it once on first use. Sends
    /// [`FluxTick::Loading`] before the load so the UI can show a one-time
    /// "loading model" status.
    fn model_or_load(&self, tick: &mpsc::Sender<FluxTick>) -> Result<&Mutex<LoadedModel>> {
        if self.model.get().is_none() {
            // Report the load before it starts. A racing initialiser may also
            // send this; a duplicate Loading tick is harmless.
            let _ = tick.send(FluxTick::Loading);
        }
        // OnceLock has no get_or_try_init on stable, so initialise eagerly and
        // store, tolerating the race where two threads both load (the first to
        // `set` wins; the loser's model is dropped). In practice the GPU mutex
        // and single-invoke-at-a-time usage make the race vanishingly rare.
        if let Some(model) = self.model.get() {
            return Ok(model);
        }
        let loaded = LoadedModel::load(&self.store, self.device).map_err(map_flux_error)?;
        // Ignore the Err(value) from a lost race; the stored value is valid.
        let _ = self.model.set(Mutex::new(loaded));
        self.model.get().ok_or_else(|| BackendError::Other("flux model cell was empty after init".to_owned()))
    }
}

impl FluxRunner for LoadedFluxRunner {
    fn run(&self, mode: FluxMode, request: FluxRequest, tick: &mpsc::Sender<FluxTick>, should_continue: &(dyn Fn() -> bool + Send + Sync)) -> Result<Vec<Vec<u8>>> {
        let cell = self.model_or_load(tick)?;
        // parking_lot Mutex: serializes the GPU, held only inside this blocking
        // call, never across an .await.
        let mut model = cell.lock();
        // The per-step callback the sampler invokes before each step. It forwards
        // a Step tick and honours cancellation: returning false breaks the loop.
        // The pipeline passes a 0-based step index; bump to 1-based for display.
        let on_step = |step: usize, step_total: usize| -> bool {
            let _ = tick.send(FluxTick::Step {
                step: step.saturating_add(1),
                total: step_total,
            });
            should_continue()
        };
        let images = match mode {
            FluxMode::TextToImage => model.text_to_image(&request, on_step),
            FluxMode::ImageToImage => model.image_to_image(&request, on_step),
        };
        images.map_err(map_flux_error)
    }
}

/// The distilled step count klein pins. The advanced override replaces it.
const DISTILLED_STEPS: usize = 4;

/// Map a `pixhaus_flux::FluxError` onto the backend's closed error set.
fn map_flux_error(err: FluxError) -> BackendError {
    match err {
        FluxError::NotDownloaded => BackendError::Other("FLUX.2 weights are not downloaded".to_owned()),
        other => BackendError::Other(other.to_string()),
    }
}

/// On-device FLUX.2 backend over a [`FluxRunner`].
///
/// Construct with [`LocalFluxBackend::new`] (production, lazily-loaded weights)
/// or [`LocalFluxBackend::with_runner`] (tests, a mocked runner). The runner is
/// `Arc`-shared so `invoke` can move a handle into `spawn_blocking`.
pub struct LocalFluxBackend {
    runner: Arc<dyn FluxRunner>,
}

impl std::fmt::Debug for LocalFluxBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalFluxBackend").finish_non_exhaustive()
    }
}

impl LocalFluxBackend {
    /// Build the production backend over a presence-checked store and a resolved
    /// device. The weights load lazily on the first `invoke`.
    #[must_use]
    pub fn new(store: ModelStore, device: DeviceChoice) -> Self {
        Self::with_runner(Arc::new(LoadedFluxRunner::new(store, device)))
    }

    /// Build the backend over an explicit [`FluxRunner`]. The unit tests pass a
    /// `mockall` double so the spawn/progress/cancel bridge runs without weights.
    #[must_use]
    pub fn with_runner(runner: Arc<dyn FluxRunner>) -> Self {
        Self { runner }
    }

    /// Drive the runner inside `spawn_blocking` and bridge progress + cancel.
    ///
    /// The std mpsc carries [`FluxTick`]s out of the blocking closure; a small
    /// async task forwards them as `Step` events. The closure checks `cancel`
    /// before each step; the backend re-checks after the join and maps a fired
    /// token to [`BackendError::Cancelled`].
    async fn run_bridged(&self, mode: FluxMode, request: FluxRequest, progress: VerbProgress, cancel: CancellationToken) -> Result<Vec<Vec<u8>>> {
        let total_steps = request.advanced.map_or(DISTILLED_STEPS, |adv| adv.steps.max(1));
        let (tick_tx, tick_rx) = mpsc::channel::<FluxTick>();

        // The forwarder owns the real VerbProgress and turns each tick into an
        // async Step send. It exits when the blocking closure drops tick_tx.
        let drain = tokio::spawn(forward_ticks(tick_rx, progress, total_steps));

        let runner = Arc::clone(&self.runner);
        let cancel_for_closure = cancel.clone();
        let join = tokio::task::spawn_blocking(move || -> Result<Vec<Vec<u8>>> {
            let should_continue = move || !cancel_for_closure.is_cancelled();
            runner.run(mode, request, &tick_tx, &should_continue)
            // tick_tx drops here, closing the channel so the forwarder ends.
        })
        .await;

        // Let the forwarder finish draining whatever the closure sent.
        let _ = drain.await;

        // A cancelled run may still return Ok (the sampler decodes the partial
        // latent); honour the token over the partial result.
        if cancel.is_cancelled() {
            return Err(BackendError::Cancelled);
        }

        match join {
            Ok(result) => result,
            Err(join_err) => Err(BackendError::Other(format!("flux worker panicked or was aborted: {join_err}"))),
        }
    }
}

/// Forward [`FluxTick`]s from the blocking closure to the async progress channel.
///
/// Runs as its own tokio task so `VerbProgress::send` (async) is reachable. Ends
/// when the sender half is dropped at the end of the blocking closure. `Loading`
/// becomes an indeterminate step; each `Step` carries a `0.0..=1.0` fraction.
async fn forward_ticks(rx: mpsc::Receiver<FluxTick>, progress: VerbProgress, total_steps: usize) {
    // A blocking std `recv()` here would park this async task's executor thread,
    // so poll with `try_recv` and yield between empties. The sampler emits at
    // most a handful of ticks (load + ~4 steps), so the short sleep is cheap and
    // the loop exits as soon as the blocking closure drops the sender.
    loop {
        match rx.try_recv() {
            Ok(tick) => {
                // The bool return reports whether the receiver is still up; we
                // forward best-effort and let the loop end on Disconnected.
                let _ = progress.send(tick_to_event(tick, total_steps)).await;
            }
            Err(mpsc::TryRecvError::Empty) => tokio::time::sleep(Duration::from_millis(8)).await,
            Err(mpsc::TryRecvError::Disconnected) => break,
        }
    }
}

/// Turn one tick into the [`VerbProgressEvent::Step`] the UI renders.
fn tick_to_event(tick: FluxTick, total_steps: usize) -> VerbProgressEvent {
    match tick {
        FluxTick::Loading => VerbProgressEvent::Step {
            fraction: None,
            message: "loading model".to_owned(),
        },
        FluxTick::Step { step, total } => {
            let denom = if total == 0 { total_steps.max(1) } else { total };
            #[allow(clippy::cast_precision_loss)]
            let fraction = Some((step as f32) / (denom as f32));
            VerbProgressEvent::Step {
                fraction,
                message: format!("step {step}/{denom}"),
            }
        }
    }
}

/// Build a [`FluxRequest`] for the text-to-image arm from an [`ImageGenRequest`].
///
/// The distilled defaults (4 steps, guidance 1.0) are pinned unless the caller
/// passes a `steps` override, which lifts the advanced sampling toggle. Guidance
/// is a no-op for klein, so the override only carries a step count.
fn build_t2i_request(req: &ImageGenRequest) -> FluxRequest {
    FluxRequest {
        prompt: req.prompt.clone(),
        width: req.width,
        height: req.height,
        seed: req.seed.unwrap_or_default(),
        num_images: req.num_images,
        init_image: None,
        mask: None,
        strength: 1.0,
        advanced: advanced_from_steps(req.steps),
    }
}

/// Build a [`FluxRequest`] for the image-to-image / inpaint arm from an
/// [`ImageEditRequest`]. The init image is the source; the mask, when present,
/// scopes the repaint to its white pixels. Width/height come from the source PNG
/// dimensions, decoded here so the latent grid matches the reference.
///
/// # Errors
///
/// Returns [`BackendError::InvalidResponse`] if the source image cannot be
/// decoded to read its dimensions.
fn build_i2i_request(req: &ImageEditRequest) -> Result<FluxRequest> {
    let (width, height) = image::load_from_memory(&req.image)
        .map(|img| (img.width(), img.height()))
        .map_err(|err| BackendError::InvalidResponse(format!("could not decode edit source image: {err}")))?;
    // Inpaint and edit share the i2i path; a present mask scopes the repaint. The
    // editor action threads a user strength through `req.strength`; clamp it to
    // the valid noise-schedule range and fall back to the edit default when the
    // request carries none, so a whole-image edit at 0.8 keeps most of the
    // reference while letting the prompt steer.
    let strength = req.strength.map_or(DEFAULT_EDIT_STRENGTH, |s| s.clamp(0.0, 1.0));
    Ok(FluxRequest {
        prompt: req.prompt.clone(),
        width,
        height,
        seed: 0,
        num_images: req.num_images,
        init_image: Some(req.image.clone()),
        mask: req.mask.clone(),
        strength,
        advanced: None,
    })
}

/// Default image-to-image strength when the request carries no explicit one.
const DEFAULT_EDIT_STRENGTH: f32 = 0.8;

/// Lift a `steps` override into [`AdvancedSampling`]. `None` keeps the pinned
/// distilled defaults; `Some(n)` overrides the step count (guidance stays 1.0,
/// a no-op for klein).
fn advanced_from_steps(steps: Option<u32>) -> Option<AdvancedSampling> {
    steps.map(|s| AdvancedSampling {
        steps: (s as usize).max(1),
        guidance: 1.0,
    })
}

#[async_trait]
impl InferenceBackend for LocalFluxBackend {
    fn backend_id(&self) -> &'static str {
        "flux-local"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::IMAGE_GENERATION
            .union(BackendCapabilities::IMAGE_EDIT)
            .union(BackendCapabilities::IMAGE_INPAINT)
    }

    fn supports_streaming(&self) -> bool {
        // No token stream. Progress is emitted as Step events from the sampler.
        false
    }

    fn estimate_cost(&self, _request: &InferenceRequest) -> CostEstimate {
        // Free — runs on this machine. Latency is the honest signal: seconds on a
        // GPU, minutes on CPU. We can't see the device here without holding a
        // load, so report a wide GPU-typical / CPU-worst envelope.
        CostEstimate {
            typical_latency: Duration::from_secs(10),
            max_latency: Duration::from_secs(600),
            typical_usd_cents: 0.0,
            max_usd_cents: 0.0,
        }
    }

    async fn invoke(&self, request: InferenceRequest, progress: VerbProgress, cancel: CancellationToken) -> Result<InferenceResponse> {
        progress.send(VerbProgressEvent::Started { backend: Some("flux-local".to_owned()) }).await;
        // Honour a token that fired before we even started.
        if cancel.is_cancelled() {
            return Err(BackendError::Cancelled);
        }

        let (mode, flux_request) = match request {
            InferenceRequest::ImageGeneration(req) => (FluxMode::TextToImage, build_t2i_request(&req)),
            InferenceRequest::ImageEdit(req) | InferenceRequest::ImageInpaint(req) => (FluxMode::ImageToImage, build_i2i_request(&req)?),
            // The registry never routes these here (the caps don't match), but the
            // backend guards anyway so a direct caller gets the right error.
            _ => return Err(BackendError::UnsupportedCapability),
        };

        let images = self.run_bridged(mode, flux_request, progress, cancel).await?;
        Ok(InferenceResponse::Image(ImageGenResponse {
            images,
            model: LOCAL_FLUX_MODEL_ID.to_owned(),
        }))
    }
}
