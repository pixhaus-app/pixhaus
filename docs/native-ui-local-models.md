# Native UI local models: on-device FLUX.2 klein generation in v2

Status: proposed. Part of the native-UI set, after [`native-ui.md`](./native-ui.md)
(the migration ADR), [`native-ui-vertical-slice.md`](./native-ui-vertical-slice.md)
(the AI creation loop), [`native-ui-settings.md`](./native-ui-settings.md) (the
settings window), and [`native-ui-sprite-generation-upgrades.md`](./native-ui-sprite-generation-upgrades.md)
(the sprite-pipeline direction). It plugs a local inference backend into the same
`InferenceBackend` registry those docs already lean on.

## Status and scope

v2 generates sprites only through cloud backends today — OpenAI `gpt-image` and FAL,
both reached through the `InferenceBackend` registry in `ai/src/backends/`. Every image
costs money and a network round-trip, and nothing works offline.

This doc plans an on-device option: run **FLUX.2 [klein] 4B distilled** locally via the
[Candle](https://github.com/huggingface/candle) Rust ML framework, behind a Cargo feature,
with the weights downloaded on demand and a settings surface to configure it. Distilled
settings are fixed — 4 sampling steps, `guidance_scale = 1.0` (CFG disabled). The local
runtime registers as one more backend, so the existing Create studio and three new
editor-mode actions reach it through the same plumbing as the cloud backends.

In scope:

| Area | What lands |
|---|---|
| `pixhaus-flux` crate | A new workspace crate that ports the FLUX.2 transformer to Candle and exposes load + t2i + i2i. |
| Local backend | `LocalFluxBackend` implementing `InferenceBackend`, registered behind a feature + weights-present gate. |
| Model management | hf-hub download with a byte-progress UI, an app-data cache, a device selector, an optional HF token. |
| Settings | A renamed "AI" tab with a local-model section (device, cache dir, download/manage card). |
| Editor actions | Text-to-image → new layer, image-to-image edit, inpaint selection, and backing the Create studio. |

Deferred, with reasons:

- **fp8 / quantized weights** — the published `FLUX.2-klein-4B` repo is bf16; fp8 lives in
  a separate repo with its own terms. Ship bf16 first; revisit fp8 as a smaller-download
  option once its license is cleared.
- **Models beyond klein-4B** — the store renders a list so a second model slots in, but only
  klein-4B is wired. No multi-model routing UI yet.
- **Training / LoRA / fine-tunes** — out. This is inference only.
- **The 9B (and 9b-kv)** — non-commercial. Reference only for the i2i call pattern; never
  shipped or downloadable.

## What v2 already has — reuse, do not rebuild

- **Backend abstraction.** `ai/src/backends/mod.rs` defines the fat trait
  `InferenceBackend { backend_id, capabilities, supports_streaming, estimate_cost, async invoke(...) }`,
  the `InferenceRequest` enum (`ImageGeneration`, `ImageEdit`, `ImageInpaint`, …), the
  `InferenceResponse::Image(ImageGenResponse { images: Vec<Vec<u8>>, model })` reply, the
  `BackendCapabilities` bitflags, and `BackendRegistry::find_for(caps)`. A new backend is one
  more `impl` of this trait.
- **Registration + priority.** `shell/src/ai.rs::register_backends_blocking` registers each
  backend with `runtime.register_backend(BackendProxy::new(b), priority)` — OpenAI at 0, FAL
  at 10 (lower wins). `try_register_fal` is the template for a conditional registration.
- **Async + progress.** One tokio runtime, created in `shell/src/main.rs`, owned by the app.
  Work runs on its handle; CPU-bound work uses `spawn_blocking`. Progress flows over the bounded
  `VerbProgress` mpsc (`VerbProgressEvent::Step { .. }`); the shell drains a `ShellMsg` channel
  each frame and calls `ctx.request_repaint()`. `spawn_reference_sheet` / `spawn_clip` in
  `shell/src/ai.rs` are the spawn templates.
- **Secrets.** `ai/src/backends/keys.rs::ApiKeyStore` (`get`/`set`/`delete`) over the OS keychain,
  service name `pixhaus.<id>`, written off-thread via `ai::spawn_backend_key_op`.
- **Settings + persistence.** `shell/src/settings.rs` (`SettingsTab`, `backend_row`), with prefs
  persisted through eframe `Storage` — loaded in `ShellApp::new` via `eframe::get_value`, saved in
  `fn save` via `eframe::set_value` (see `GridPrefs`).
- **Undoable landing.** `shell/src/commands.rs` lands generated pixels on the canvas with one undo
  entry: `push_sprite_edit_with_buffers` (add a layer + buffer), `PixelRegionEdit` (replace a region),
  `integrate_frames_undoable` (insert an animation). `editor.selection: Option<SelectionMask>` is the
  inpaint mask source.
- **The Create studio.** `shell/src/cockpit.rs` / `studio.rs` already drive a generation loop through
  the registry; the local backend satisfies it with no studio rewrite.

## Architecture — the `pixhaus-flux` crate

The FLUX.2 port lives in a **new workspace crate `flux/` (`pixhaus-flux`)**, not a module inside `ai/`.
Two reasons decide it. First, **lint isolation**: the workspace floor is `unsafe_code = "forbid"` plus
`unwrap_used`/`expect_used`/`panic = "deny"`, inherited by every crate that writes `[lints] workspace = true`.
Tensor code wants the occasional `unwrap` on a shape it just built; a separate crate sets its own `[lints]`
(keep `unsafe_code = "forbid"` — Candle's API is safe, so the port never needs `unsafe` — but relax
`unwrap_used`/`expect_used` to `warn`) without loosening the floor for `core`/`ai`/`shell`. Second,
**compile-time and dep-tree isolation**: Candle + cudarc + gemm + tokenizers is the heaviest subtree in the
project; behind an optional dep it never compiles in the default build and never poisons incremental rebuilds
of the other crates.

`flux/Cargo.toml` (versions current as of early 2026):

```toml
[package]
name = "pixhaus-flux"
edition.workspace = true          # 2024 — per-crate editions interoperate with Candle's 2021
rust-version.workspace = true     # 1.85

[dependencies]
# Published crates.io releases — no fork, no git pin. There is no `flux2` module
# (only FLUX.1's `flux`), so the FLUX.2 topology is assembled on candle's base APIs
# (candle-core Tensor + candle-nn VarBuilder/layers); the flow-matching sampler, the
# VAE, the Qwen3 encoder, and the public DiT building-block structs are imported from
# candle-transformers. F8E4M3 weight loading is present in candle-core 0.10.
candle-core         = { version = "0.10", default-features = false }
candle-nn           = { version = "0.10", default-features = false }
candle-transformers = { version = "0.10", default-features = false }
tokenizers = { version = "0.22", default-features = false, features = ["onig"] }
hf-hub     = { version = "0.5", default-features = false }
safetensors = "0.7"
image.workspace = true
directories = "6"                 # add to [workspace.dependencies] too
serde.workspace = true
thiserror.workspace = true
tracing.workspace = true
parking_lot.workspace = true

[features]
default    = ["cpu"]
cpu        = []
cuda       = ["candle-core/cuda",  "candle-nn/cuda",  "candle-transformers/cuda"]
metal      = ["candle-core/metal", "candle-nn/metal", "candle-transformers/metal"]
flash-attn = ["cuda", "candle-transformers/flash-attn"]
download   = ["hf-hub/tokio", "hf-hub/rustls-tls"]

[lints.rust]
unsafe_code = "forbid"
[lints.clippy]
pedantic    = { level = "warn", priority = -1 }
unwrap_used = "warn"              # relaxed from the workspace "deny"
expect_used = "warn"
```

Opt-in wiring:

- `ai/Cargo.toml`: `pixhaus-flux = { path = "../flux", optional = true }` plus features
  `local-flux = ["dep:pixhaus-flux"]`, `local-flux-cuda`, `local-flux-metal`, `local-flux-cpu`
  (each forwards `pixhaus-flux/<backend>` + `pixhaus-flux/download`).
- `shell/Cargo.toml`: forward `local-flux-cuda` / `local-flux-metal` / `local-flux-cpu` to `pixhaus-ai`.
  The Windows packaged build compiles with `local-flux-cuda`, macOS with `local-flux-metal`; CI's default
  `cargo build` / `nextest` leaves the feature **off**, so Candle never compiles in the standard gate.
- Root `Cargo.toml`: append `"flux"` to `members`; add `pixhaus-flux` and `directories` to
  `[workspace.dependencies]`.

### Port vs reuse

| Component | Decision | Source |
|---|---|---|
| Tensor + nn base APIs (`Tensor`, `Device`, `VarBuilder`, `linear`, `conv2d`, `layer_norm`, `group_norm`, `rms_norm`, `ops`, `Module`) | **Use** — the foundation the adapter is built on | `candle-core`, `candle-nn` 0.10 (all public) |
| DiT building-block structs (`Config`, `Flux`, `DoubleStreamBlock`, `SingleStreamBlock`, `SelfAttention`, `EmbedNd`, `QkNorm`, `MlpEmbedder`, `LastLayer`) | **Import** where the FLUX.1 shape fits | `candle_transformers::models::flux::model` (public) |
| Helper fns (`apply_rope`, `attention`, `scaled_dot_product_attention`, `timestep_embedding`, `Modulation1/2`) | **Reimplement** on candle-nn — these are module-private, not `pub` | local, ~tens of lines each |
| Flow-matching sampler (`State`, `get_noise`, `get_schedule`, `denoise`, `unpack`) | **Import**, adjusted for 4-step / guidance=1 | `candle_transformers::models::flux::sampling` (public) |
| VAE (`Encoder`, `Decoder`, `DiagonalGaussian`, `AutoEncoder`) | **Import** structure, re-derive config | `candle_transformers::models::flux::autoencoder` (public) |
| fp8 / bf16 safetensors loading (`DType::F8E4M3`, `VarBuilder`) | **Use** | `candle-core` 0.10 (`F8E4M3` confirmed present) |
| Qwen3 text encoder | **Import**, adapt to expose hidden states (not the LM head) | `candle_transformers::models::qwen3` (public) |
| CUDA + Metal backends | **Use** | `candle-core` |
| FLUX.2 transformer topology + `Config` | **Build** on the base API — block counts, hidden sizes, head config differ from FLUX.1 | `black-forest-labs/flux2`, diffusers `Flux2` |
| Conditioning rewrite | **Build** — Qwen3 hidden states only, no CLIP pooled vector; new `State` + forward signature; position ids extended for multi-reference packing | reference repos |
| img2img wiring | **Build** — VAE-encode reference → `DiagonalGaussian` sample → patchify → add noise to strength → enter `denoise` at the partial schedule index | 9b-kv README, diffusers i2i |

There is **no `flux2` module** in candle (only FLUX.1's `flux` plus `qwen3`), so we do **not** fork candle.
We depend on the published `candle-core` / `candle-nn` / `candle-transformers` 0.10 crates and build the
FLUX.2 adapter on candle's base inference APIs: import the public sampler, VAE, Qwen3 encoder, and DiT
building-block structs; reimplement the handful of module-private helpers locally; and assemble the new
FLUX.2 block topology and forward where it diverges from FLUX.1. Net-new code requires a one-line
justification pointing at this table or the verify list below.

### Verify, do not assume

Each is a checklist item gated before the milestone that depends on it:

- **VAE config.** FLUX.1 uses `z_channels=16`, `scale_factor=0.3611`, `shift_factor=0.1159`. FLUX.2 changed
  its latent space — re-read channels / scale / shift / downsample from the diffusers `Flux2` VAE config.
  Reuse the encoder/decoder if the family is identical; port if it differs.
- **Qwen3 usage.** Confirm the exact Qwen3 variant, which hidden layer is consumed, and any projection
  applied before the tokens enter the transformer.
- **Position-id / RoPE scheme.** Confirm the axes and id layout for image tokens and multi-reference
  packing; FLUX.1's `EmbedNd` axes may not transfer unchanged.
- **fp8 vs upcast.** The bf16 repo we target needs no fp8 matmul path. `DType::F8E4M3` is present in the
  published candle-core 0.10, so fp8 weights load without a fork; if fp8 weights are added later, confirm
  whether the target backend matmuls in fp8 or upcasts on load to bf16.

## The local backend — `LocalFluxBackend`

A new `ai/src/backends/local_flux.rs`, entirely `#[cfg(feature = "local-flux")]`, implementing
`InferenceBackend` like `fal.rs` / `openai.rs`:

- `backend_id() = "flux-local"`.
- `capabilities() = IMAGE_GENERATION | IMAGE_EDIT | IMAGE_INPAINT`.
- `supports_streaming() = false` (no token stream; it emits `Step` events).
- `estimate_cost()` → `$0`. Latency is the honest signal — seconds on GPU, minutes on CPU — reported
  through the estimate's latency field.
- `invoke` maps `ImageGeneration` → text-to-image, `ImageEdit` / `ImageInpaint` → image-to-image; every
  other arm returns `BackendError::UnsupportedCapability` (the registry never routes them here because the
  capability bits don't match). The backend pins the distilled settings (4 steps, guidance 1.0) and ignores
  `steps` / guidance unless the settings advanced-override toggle is on.

### The blocking / progress / cancel bridge

Candle inference is synchronous and CPU/GPU-bound, so `invoke` offloads to `spawn_blocking` and bridges
progress back to the async `VerbProgress` channel:

```rust
// model: Arc<OnceCell<Mutex<LoadedModel>>> on the backend — multi-GB load runs ONCE, lazily.
let (tick_tx, tick_rx) = std::sync::mpsc::channel::<FluxTick>();
let drain = tokio::spawn(forward_ticks(tick_rx, progress.clone())); // Step events, async send
let images = tokio::task::spawn_blocking(move || -> Result<Vec<Vec<u8>>> {
    let cell = model.get_or_try_init(|| {
        tick_tx.send(FluxTick::Loading).ok();
        Ok::<_, BackendError>(Mutex::new(LoadedModel::load(&cache, device)?))
    })?;
    let mut m = cell.lock();                       // parking_lot; serializes the GPU, no .await held
    m.text_to_image(&req, |step, total| {          // callback before each of the 4 steps
        tick_tx.send(FluxTick::Step { step, total }).ok();
        !cancel.is_cancelled()                      // false => stop the sampling loop early
    })
}).await??;
drain.await.ok();
if cancel.is_cancelled() { return Err(BackendError::Cancelled); }
Ok(InferenceResponse::Image(ImageGenResponse { images, model: "flux2-klein-4b".into() }))
```

Three points the implementer must hold:

- **Load once.** `Arc<OnceCell<Mutex<LoadedModel>>>` keeps the ~8 GB load to the first `invoke`, not per
  request. The `parking_lot::Mutex` serializes GPU access (one denoise at a time — a consumer GPU can't run
  two FLUX jobs without OOM) and is locked **only inside** `spawn_blocking`, never across an `.await`.
- **Progress without async-in-blocking.** `VerbProgress::send` is async; the blocking closure can't call it.
  A std mpsc carries 4 ticks to a small async `forward_ticks` task that owns the real `VerbProgress` and does
  the `send(Step { fraction, message }).await`. The shell already drains this channel and repaints — no UI-side
  change.
- **Cancellation.** Candle's sampler has no built-in cancel. The callback runs before each of the 4 steps and
  returns `false` to break; worst-case latency to honor a cancel is one step. The VAE decode tail is a single
  non-cancellable op, which is acceptable.

### Registration

`shell/src/ai.rs` gains `try_register_local_flux(runtime, &LocalModelSettings)`, mirroring `try_register_fal`,
gated on two independent conditions:

```rust
#[cfg(feature = "local-flux")]
pub fn try_register_local_flux(runtime: &VerbRuntime, settings: &LocalModelSettings) -> bool {
    let store = pixhaus_flux::ModelStore::from_settings(settings);
    if !store.is_downloaded() { return false; }    // UI shows a Download affordance instead
    let backend = LocalFluxBackend::new(store, pixhaus_flux::DeviceChoice::resolve(settings));
    runtime.register_backend(BackendProxy::new(backend), 5).is_ok() // below OpenAI(0), above FAL(10)
}
#[cfg(not(feature = "local-flux"))]
pub fn try_register_local_flux(_: &VerbRuntime, _: &LocalModelSettings) -> bool { false }
```

Priority **5** keeps premium cloud (`gpt-image`) the default but ranks local above generic cloud FLUX, so an
offline or no-key user still gets a working image backend. The model is **not** loaded at registration — only
presence-checked — so registration stays instant and never blocks the UI thread. Re-run registration when a
download completes or the device changes.

## Model management — download, cache, device

`flux/src/store.rs` owns presence + download; `flux/src/loader.rs` owns `LoadedModel::load`.

- **Repo + files.** `ModelStore` targets `black-forest-labs/FLUX.2-klein-4B` at a pinned revision (a commit
  hash before shipping, for reproducibility). It fetches the diffusers-layout components —
  `model_index.json`, `transformer/`, `vae/`, `text_encoder/` (Qwen3), `tokenizer/`, `scheduler/` — and skips
  the example images. **Confirm the exact safetensors filenames and whether the transformer is sharded**
  (`*-00001-of-0000N.safetensors` + an index) against the live HF tree; the published repo is **bf16**, total
  footprint **~24 GB**.
- **Cache dir.** Default to app-data via `directories` (`ProjectDirs::from("app", "Pixhaus", "Pixhaus")` →
  `data_dir().join("models/flux2-klein-4b")`), so Pixhaus owns the lifecycle and uninstall can clean it.
  Pass it as `ApiBuilder::with_cache_dir`. A settings override (persisted) wins.
- **Download with byte progress.** hf-hub 0.5's `get` has no programmatic byte callback (it drives an
  `indicatif` CLI bar), so stream each file with `reqwest` (already a workspace dep, rustls) via
  `bytes_stream()` against `Content-Length`, summing bytes and pushing `(downloaded, total)` onto an mpsc
  every few MB. Add an `Authorization: Bearer <token>` header for gated repos (klein-4B is public, token
  optional). Runs off-thread on the tokio handle.
- **HF token.** Stored in the OS keychain under service `pixhaus.huggingface`, reusing `ApiKeyStore` — no new
  keychain code, one new id constant.
- **Integrity.** `is_downloaded()` checks every required path exists and is non-empty for the registration
  gate; add a SHA check against the HF LFS pointer to guard against truncated downloads.
- **Device.** `DeviceChoice { Cuda(usize), Metal, Cpu }` with `auto()` resolving CUDA → Metal → CPU; settings
  can force one. CPU posture, stated plainly: the `cpu` feature compiles and runs (it is the CI / correctness /
  last-resort path), but a 4B transformer on CPU is **minutes per image** — the UI warns when the device
  resolves to CPU and surfaces it in the latency estimate.

## Settings — the "AI" tab

Rename `SettingsTab::AiBackends` → `Ai` (label "AI"). `ai_tab` keeps the cloud `backend_row`s under a
"Cloud backends" heading, a `ui.separator()`, then `local_model_section(ui)`. The section is
`#[cfg(feature = "local-flux")]`; the `#[cfg(not)]` stub renders one weak line — "This build was compiled
without on-device generation."

The section, top to bottom, built from the existing primitives (`ui.heading`, weak `RichText` captions,
`ui.horizontal`, `Palette` status chips, deferred-mutation like `backend_row`):

1. Heading "Local model (FLUX.2 klein)" + caption "Runs on this machine. No API key, no per-image cost.
   One-time ~24 GB download."
2. **Device** radios (`Auto` / `CUDA` / `Metal` / `CPU`) — show only the variants the build and platform
   support; under `Auto`, a caption names the resolved device.
3. **Cache directory** row — current path + a "Choose…" button that spawns `rfd::FileDialog::pick_folder`
   **off-thread** (it blocks; follow the `export.rs` pattern) and returns over `ShellMsg::ModelCacheDirChosen`,
   plus "Reset to default".
4. **Model card** (an `egui::Frame::group`, rendered as a list for future models) keyed off status:
   `NotDownloaded` → Download button; `Downloading { fraction }` → `egui::ProgressBar::new(fraction)` +
   "X.X / 24.0 GB" + Cancel; `Ready` → success chip + Delete; `Failed(msg)` → error chip + Retry. Status
   comes from `self.local_ai_status`, set by the download messages and a startup probe.
5. **Total disk usage** caption + cache path.
6. **HF token** — a password `TextEdit` + Save/Clear, routed through `ai::spawn_backend_key_op` with id
   `"huggingface"`. Labelled optional, only for gated downloads.
7. **Distilled params** — read-only "Steps: 4 · Guidance: 1.0 (distilled defaults)" + an "Advanced override"
   checkbox that reveals step (1..=8) and guidance (0.0..=4.0) sliders.

Persisted struct, next to `GridPrefs`:

```rust
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LocalModelSettings {
    pub device: DevicePref,                     // Auto | Cuda | Metal | Cpu
    pub cache_dir: Option<PathBuf>,             // None = app-data default
    pub default_model: String,                  // "flux2-klein"
    pub overrides: Option<DistilledOverrides>,  // None = distilled 4 / 1.0
}
```

Loaded in `ShellApp::new` via `eframe::get_value(s, "local_ai")` (next to the `grid_prefs` load), saved in
`fn save` via `eframe::set_value(storage, "local_ai", &self.local_ai)`. A transient `advanced_open: bool`
stays a non-persisted `ShellApp` field. The HF-token-configured flag rides the existing off-thread backend
refresh.

New `ShellMsg` variants and off-thread spawns (in a new `shell/src/local_ai.rs`, modeled on
`spawn_backend_key_op` for the blocking shape and `spawn_clip` for the cancel-token + progress-closure shape):

```rust
ModelDownloadProgress { model: String, fraction: Option<f32>, bytes: u64, total: u64, message: String },
ModelDownloadDone     { model: String },
ModelDownloadFailed   { model: String, error: String },
ModelCacheDirChosen(Option<PathBuf>),
LocalModelProbed      { model: String, ready: bool },
```

The download cancel token is owned by `ShellApp` (`local_ai_cancel: Option<CancellationToken>`), created at
download start and `.cancel()`-ed by the Cancel button — the lifecycle the clip path already uses.

## Editor-mode actions

Generation is a command, not a pixel tool — so the entry point is one `Generate` menu in the menu bar (not a
new tools-rail entry, not a new dock), with three items, plus a small `egui::Window` popover that gathers the
prompt and strength. All three live in `shell/src/local_ai.rs` as an impl block on `ShellApp`, build their
request with the `ai/src/backends` types, and land through `shell/src/commands.rs`. Each menu item is
`add_enabled(self.local_ai_ready(), …)` with an `on_disabled_hover_text` explaining why.

Popover state:

```rust
local_gen: Option<LocalGenPanel>,   // None = closed
struct LocalGenPanel { mode: LocalGenMode, prompt: String, strength: f32, as_new_layer: bool }
enum LocalGenMode { TextToImage, ImageToImage, Inpaint }
local_gen_status: JobStatus,        // reuses the existing enum + cockpit spinner block
```

A single `spawn_local_image` (mirroring `spawn_reference_sheet`) runs all three; one drain block keyed by
`LocalGenMode` chooses the landing path. Each landing pushes exactly one history entry, and a stale-result
epoch guard (as `ClipReady` uses) drops a late result from a cancelled run.

- **(a) Text-to-image → new layer.** Prompt only; output sized to the active sprite canvas. Build an
  `ImageGenRequest` with `model: Some("flux2-klein")` to pin the local backend even when a cloud backend is
  registered. Land with `commands::push_sprite_edit_with_buffers`: a new `Layer::raster` + `Cel::raster` +
  the decoded buffer in one undo entry.
- **(b) Image-to-image edit.** Init image is the active layer's cel buffer (or the cached `display_frame`
  composite), encoded to PNG. Build an `ImageEditRequest` with a strength slider. This requires adding
  `strength: Option<f32>` to `ImageEditRequest` (defaulted `None`; cloud backends destructure-and-ignore it,
  exactly as they do `style_image` today). Land as a full-canvas `PixelRegionEdit` (Replace, the default for
  i2i) or as a new layer (the toggle).
- **(c) Inpaint selection.** Gated on `editor.selection.is_some()`. Rasterize the `SelectionMask` to a
  white-on-black PNG (white = repaint, per the `ImageEditRequest::mask` contract), pass the active
  layer/composite as the init image, request `IMAGE_INPAINT`. Land as a region `PixelRegionEdit` over the
  selection bounds — cheaper for the 8K-canvas constraint than a full-canvas replace.
- **(d) Back the Create studio.** The reference-sheet path already routes through the registry; pinning the
  local backend (model id / backend selector) lets the existing studio run offline with no studio rewrite.

## Progress, errors, and gating

- **4-step generation** → spinner + "step n/4" label (not a bar; four fast steps don't warrant a bar that
  barely moves), reusing the `cockpit_generate_row` status block. On done, land; on failure, set
  `JobStatus::Failed(err)` and a `set_status` toast so the error survives the popover closing.
- **Multi-GB download** → a real `egui::ProgressBar` driven by `ModelDownloadProgress.fraction` with a
  `bytes/total` caption. This is the one place a bar is right.
- **Not downloaded when invoked** → the popover opens with Generate disabled and an
  `on_disabled_hover_text` plus an "Open download settings" button (`open_settings(SettingsTab::Ai)`),
  turning a dead end into a one-click path to the fix.
- **Three gates.** Compiled-in (`#[cfg(feature = "local-flux")]` on registration, the settings section, the
  Generate menu, and the `ShellMsg` variants — mirror the `LospecDone` gating); weights-present (probed
  off-thread at startup into `local_ai_status`); device-usable (the radios offer only detected variants;
  switching device shows a restart/fallback advisory). What the artist sees: no Generate menu when not
  compiled; a Download card and a CTA when absent; everything live when ready, folded into the status-bar
  `backend_ready` chip.

## Milestones — parity gates

Each milestone is gated by a parity test against the PyTorch reference. Generate fixtures once by running the
reference pipeline at a fixed seed and dumping intermediate tensors to `.safetensors`; each gate compares the
Rust output to the dump. Tolerances are bf16-compute tolerances — a wide miss means an architecture/layout bug,
not precision.

| # | Gate | Test kind | Tolerance |
|---|---|---|---|
| 1 | VAE round-trip | encode→decode a fixture, `image-compare` | decoded ≥ 0.99; latent mean/std/shape within 1e-2 |
| 2 | Text-encoder parity | Qwen3 hidden states vs reference, `insta` stats | cosine > 0.999, max abs within 1e-2 |
| 3 | Single forward pass | one DiT step on fixed inputs; shape + finiteness, `proptest` seeds | velocity within tolerance — highest-risk gate |
| 4 | Full 4-step t2i | fixed seed, compare in latent space then `image-compare` the PNG | ≥ 0.999 vs golden |
| 5 | Image-to-image | single-reference edit at fixed strength, `image-compare` | ≥ 0.999 vs golden |
| 6 | Backend + ergonomics | **always-on, mocked** (`mockall` on a `FluxRunner` trait): caps, `backend_id`, unsupported arms → `UnsupportedCapability`, cancel → `Cancelled`, the std-channel → `VerbProgress` bridge emits 4 `Step` events | exact |

Fixtures live in `flux/tests/fixtures/` (inputs) and `flux/tests/snapshots/` (golden PNGs + insta `.snap`).
Gates 1–5 are triple-gated so GPU-less CI stays green: the crate compiles only under a `cpu`/`cuda`/`metal`
feature, the tests carry `#[ignore = "requires downloaded FLUX.2 weights and a GPU"]`, and an early
`if std::env::var_os("PIXHAUS_FLUX_WEIGHTS").is_none() { return; }` guard. Gate 6 is the only one that runs in
normal CI.

## Risks and guardrails

- **cargo-deny license sweep (highest friction).** Candle's subtree adds dozens of crates — `cudarc`, `metal`,
  `gemm`, `tokenizers` (which pulls `onig`/`onig_sys`, `esaxx-rs`), and `*-sys` crates with vendored C. The repo
  already carries known cargo-deny pain. Budget a review pass after adding Candle, and expect to add a few
  deliberate `exceptions` entries with justifications. Run `cargo deny check` and record the result.
- **No wgpu conflict, but check duplicates.** Candle talks to CUDA via `cudarc` and to Metal via the `metal`
  crate — it does **not** use wgpu, so there is no clash with the exact `wgpu = "=29.0.1"` pin. The residual
  risk is duplicate transitive versions (`half`, `gemm`, `rayon`); run `cargo tree -d` and record any.
- **Download + binary size.** A ~24 GB one-time download (user-initiated, with the progress UI) and a much
  larger linked binary when CUDA kernels are statically included. Ship GPU features only in the platform
  packaged builds; keep them off in the fast nextest loop.
- **Compile-time blowup.** Candle codegen is the largest compile cost in the tree; crate isolation keeps it
  out of `core`/`ai`/`shell` incremental rebuilds and out of features-off CI.
- **Edition mismatch is benign.** `pixhaus-flux` is edition 2024 depending on Candle's edition 2021; per-crate
  editions interoperate on rustc 1.85. Confirm with `cargo build -p pixhaus-flux --features cpu`.
- **Provenance (recorded decision).** Omit the reference repo's pixel-layer watermarking — it would corrupt
  exact pixel output, which is antithetical to a pixel-art editor. Instead record prompt / backend / model /
  seed in the project's AI lineage, as the Create studio already does for cloud generations. State the omission
  in code comments so it is a deliberate choice, not an accident.
- **Licensing.** The 4B weights are Apache-2.0 and safe to vendor/redistribute. The 9B (and 9b-kv) are
  non-commercial — reference only for the i2i call pattern; the download module hard-codes the 4B repo id and
  never offers the 9B.

## Verification

- `cargo build -p pixhaus-flux --features cpu` — the crate compiles on the always-buildable path.
- `cargo nextest run --workspace` (features off) — gate 6 and every existing test pass; Candle does not compile.
- `cargo deny check` and `cargo tree -d` — run and record license exceptions + duplicate versions.
- With a GPU and `PIXHAUS_FLUX_WEIGHTS` set, `cargo nextest run -p pixhaus-flux --run-ignored all` — gates 1–5.
- By hand: Settings → AI → Download model (watch the progress bar to 100%); restart the app and confirm the
  device / cache / model settings persist; in Draw mode run each Generate action (text-to-image, edit, inpaint
  a selection) and confirm a single undo fully reverts each result and a redo reinstates it; cancel a run
  mid-flight and confirm the document is untouched.

## Out of scope

Multi-model management beyond klein-4B; fp8 / quantized variants (future, license-gated); training / LoRA;
the 9B; video. Suggested build order: scaffold the crate → `ModelStore` + download → VAE (gate 1) → Qwen3
conditioning (gate 2) → DiT topology + 4-step t2i (gates 3–4) → img2img (gate 5) → `LocalFluxBackend` + the
spawn_blocking/progress/cancel bridge (gate 6) → settings + the Generate menu and editor actions → the
cargo-deny license sweep.
