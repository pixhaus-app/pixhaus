# Work journal — the foundation layer for the Generate workspace

> **Why this file is in `docs/`.** `docs/CLAUDE.md` normally reserves this tree for
> durable, read-only design references and says transient task notes don't belong
> here. This file is a deliberate, user-requested exception: a running build journal
> for the foundational-layer work, kept here (not at the repo root) because the
> conclaude `preventRootAdditions` hook blocks new top-level files. It records what
> landed, the decisions made, and deviations from the plan, stage by stage.

Plan: `~/.claude/plans/sorted-gliding-acorn.md`. Branch: `feat/generate-foundation`.

## Goal

Build the shared spine the Generate workspace stands on — `core` (model, commands,
composite), `services` (undo history, job system, provider registry, result store),
`render` (sprite texture path), a mock provider — then wire the Generate panels so
the loop runs end-to-end: prompt → mock job → result tray → insert as new sprite →
composited on the canvas → undo removes it. No GPU or API key required. Follows
`docs/pixhaus_architecture_bible.md` (9, 12, 13, 14, 16, 22, 31) and the per-crate
`CLAUDE.md` boundaries; adapts v2's proven patterns under v3's strict layering.

## Stage checklist

- [x] Stage 0 — journal (this file)
- [x] Stage 1 — `crates/core`: model + commands + composite
- [x] Stage 2 — `crates/services`: command execution + undo history
- [x] Stage 3 — `crates/services` jobs/providers + `modules/providers` mock
- [x] Stage 4 — `crates/render`: sprite texture path
- [x] Stage 5 — `crates/ui`: EditSession, CanvasCallback upload, Intent + drain
- [x] Stage 6 — `modules/generation` + `canvas_stage` wiring
- [x] Stage 7 — end-to-end verification (headless loop green + full session gate; interactive run pending user)

**FOUNDATION COMPLETE** (branch `feat/generate-foundation`, not committed/pushed). The
spine the Generate workspace stands on is built and verified: `core` model + commands +
composite, `services` undo history + job system + provider registry + result store, the
mock provider, the `render` sprite texture path, and the `ui` `EditSession` + command/job
intent path with the Generate panels wired. Full session gate green (fmt, workspace
clippy `-D warnings`, 166 nextest, doctests, `cargo doc` warning-free, `cargo deny`). The
`render_workspaces` snapshot regenerates cleanly. Only the interactive on-screen check
(run the app, click through) remains for a human.

## Key fixed decisions (from planning)

- One non-generic `Command` trait; its single target is `core::Document`, which
  bundles structural data **and** the pixel-buffer store, so one command type covers
  structural + pixel edits. Pixel bytes are decoupled from metadata via
  `PixelBufferId` handles into a separate store (v2's key lesson) — structural undo
  stays cheap.
- Monotonic `u32` newtype ids minted from a counter, tombstone-on-delete. No
  `slotmap`/`uuid` (not in the catalog).
- Linear, memory-capped undo history; branching is a later refinement.
- RGBA8 buffers only; indexed/pixel-art surfaces deferred to the Pixel Art module.
- Capability-based `Provider` trait, object-safe via a boxed-future return (no
  `async-trait` dep). The `MockProvider` runs as a real tokio task with a small
  delay and draws a deterministic sprite from the prompt.
- `JobMsg` notifications are lightweight; the `GeneratedAsset` is pulled from an
  `Arc<ResultStore>` (the one sanctioned `Arc<Mutex>` — shared job-task ↔ UI lane).
- `services` never depends on egui: it owns a UI-toolkit-free `JobMsg`; the shell
  bridges it. The live `Document`/`History`/services bundle lives in an `EditSession`
  on the `Host`, mutated only in `apply_intent`/`drain_jobs`/`canvas_stage` — never
  borrowed into a `&self` panel.

## Log

### Stage 0 — journal — DONE

Created this file and the feature branch `feat/generate-foundation` off `v3`.

### Stage 1 — `crates/core` — DONE

Built the domain model and the pure operations. Files: `ids.rs` (monotonic `u32`
newtypes `SpriteId`/`LayerId`/`PixelBufferId` + `IdCounter`), `pixel.rs` (`Rgba`,
`BlendMode`, `PixelBuffer` RGBA8 + stride, `PixelError`), `buffer_store.rs`
(`PixelBufferStore`), `document.rs` (`Document` = sprites + buffer store + active +
revision; `Sprite`, `Layer`), `command.rs` (`Command` trait + `CommandError`),
`composite.rs` (`composite_sprite`/`composite_active` + `CompositeError`), `commands/`
(`AddSprite` + `SpriteProto`, `ApplyGeneratedAsset`), and the `undo_round_trip`
integration test. Cargo: added `serde` + `thiserror` (deps), `rstest` + `proptest`
(dev). **Green:** 20 tests pass (incl. a proptest that an opaque sprite composites to
itself), `cargo clippy -p pixhaus-core --all-targets -- -D warnings` clean, doctests ok.

Decisions / notes:
- `Command` is one non-generic trait over `Document`; `apply` captures undo state,
  `undo` reclaims pixels (`ApplyGeneratedAsset` pulls bytes back from the removed
  buffer) so apply/undo/redo cycles. `estimated_size_bytes` returns ~0 while applied
  (pixels live in the doc, not the command) — correct for the memory-capped history.
- `Document` fields are `pub(crate)`; mutation is `pub(crate)` mutators that bump
  `revision`. Public getters (`sprites`/`sprite`/`buffers`/`active_sprite`/
  `active_sprite_size`/`revision`) are what services/render/ui read.
- **Lint gotcha (carry forward):** the repo `clippy.toml` `disallowed-methods` bans
  `.unwrap()`/`.expect()` even in tests, and `cfg_attr(test, allow(unwrap_used, …))`
  does NOT cover it. Add `clippy::disallowed_methods` to the crate-level
  `cfg_attr(test, allow(...))`, and a file-level `#![allow(clippy::disallowed_methods,
  clippy::unwrap_used, clippy::expect_used)]` to every `tests/*.rs` integration file
  (separate crate — the lib's cfg_attr doesn't reach it). Matches
  `crates/ui/tests/resolve_layout_snapshot.rs`.
- Float→u8 in `composite` uses one `#[allow(clippy::cast_possible_truncation,
  clippy::cast_sign_loss)]` helper (`to_u8`, clamped 0..=255, justified). Loop bodies
  use `chunks_exact_mut` + usize offsets to avoid `u32` cast lints.

### Stage 2 — `crates/services` (command + undo) — DONE

Files: `error.rs` (`ServiceError`, `#[from] CommandError`), `history.rs` (`History`:
`execute`/`undo`/`redo`, `can_undo`/`can_redo`, `next_undo_label_key`, memory cap +
eviction), `transaction.rs` (`Transaction` — itself a `core::Command`, children apply
in order / undo in reverse, rollback on mid-group failure), `history_round_trip`
integration test. lib.rs: added the `cfg_attr(test, allow(...))` (the i18n service +
`i18n!` macro left untouched) and `pub mod`/re-exports. Cargo: added `pixhaus-core` +
`thiserror` (deps), `rstest` (dev). **Green:** 16 tests (8 pre-existing i18n + 8 new),
clippy clean, doctests ok.

Decisions / notes:
- `History` is linear; memory cap counts the `done` stack's held bytes and evicts
  oldest while keeping ≥1. `execute`/`undo` account for size symmetrically while the
  command is in its applied shape. The cap is tested in isolation with a no-op
  `Heavy` test command (real commands like `ApplyGeneratedAsset` hold ~0 bytes while
  applied, by design — pixels live in the doc).
- `execute` is `#[instrument(skip_all, fields(command = cmd.label_key()))]` per the
  services tracing rule.

### Stage 3 — `services` jobs/providers + `modules/providers` mock — DONE

services files: `generated.rs` (`GeneratedAsset` + `GenerationProvenance`),
`provider.rs` (`Provider` trait — object-safe via `GenerateFuture` boxed-future type
alias; `ProviderCapability`, `ProviderId`, `ProviderRegistry`, `ProviderError`),
`job.rs` (`JobManager::submit` spawns a tokio task, `select!`s the provider future
against a cancel token, `put`s the asset + emits `JobMsg`; `GenerationJobInput`,
`GenerationContext`, `JobId`, `JobStatus`, `JobMsg`), `result_store.rs` (`ResultStore`,
the one `Arc<Mutex>` via `parking_lot`). providers: `mock.rs` (`MockProvider` — real
tokio task w/ delay, deterministic FNV-hash → palette → centred diamond), `lib.rs`
`register`. Locales: new `commands.yaml` (`command.add_sprite`,
`command.apply_generated_asset`) and `providers.yaml` (`provider.mock.label`). Cargo:
services += tokio/tokio-util/serde/parking_lot; providers += core/services/tokio/
tokio-util/tracing (dev rstest). **Green:** services 23 tests (incl. 3 async job
lifecycle tests), providers 4 tests (incl. the **full headless loop** prompt → mock →
result → `ApplyGeneratedAsset` → composite-matches → undo). `cargo build --workspace`
clean.

Decisions / notes:
- `Provider::generate` returns `Pin<Box<dyn Future + Send + 'a>>` (the `GenerateFuture`
  alias), keeping `dyn Provider` object-safe with NO `async-trait` dep. The spawned
  task owns the input, so the borrowed-`'a` future is fine inside a `'static` task.
- `JobMsg` is lightweight (Status/Completed/Failed); the asset is pulled from the
  `Arc<ResultStore>` by job id — big buffers never ride the channel (bible 31.5).
- The async job tests poll `try_recv` with a short `tokio::time::sleep` (so the
  current-thread test runtime can schedule the spawned task) rather than blocking on
  `recv()`. Carry this pattern into the Stage 7 headless UI test.
- `services` stays egui-free: it owns `JobMsg`; the shell will bridge it onto its
  `BackgroundChannel` (Stage 5).

### Stage 4 — `crates/render` (sprite texture path) — DONE

Rewrote `ViewportRenderer` from a flat-fill into a textured-quad blitter: a Nearest
sampler (mag/min `FilterMode::Nearest`, mipmap `MipmapFilterMode::Nearest`), a
texture+sampler bind group, a retained `CanvasTexture` recreated only on size change,
and `upload_frame(device, queue, rgba, w, h)` (full-frame `queue.write_texture`).
New shader `viewport_blit.wgsl` (fullscreen triangle, UV-from-clip, premultiply to
match egui's premultiplied-alpha target); deleted `viewport_fill.wgsl`. **Green:**
clippy clean, the GPU-gated upload+realloc test passes on the dev box.

Decisions / notes:
- No camera uniform and no new deps. egui-wgpu sets the pass VIEWPORT to the artboard
  rect, so clip→UV maps the sprite across it; zoom/scroll via a camera is the
  follow-up. Kept the public API on raw `&[u8]` so `render` stays spine-agnostic
  (no `core`/`bytemuck` edge needed yet).
- Canvas texture format tracks `target_format.is_srgb()` (`Rgba8UnormSrgb` vs
  `Rgba8Unorm`) so authored bytes round-trip through the GPU encode/decode.
- **wgpu 29 API gotchas (carry forward):** `mipmap_filter` is `MipmapFilterMode`, not
  `FilterMode`; `PipelineLayoutDescriptor.bind_group_layouts` is `&[Some(&bgl)]` +
  `immediate_size: 0`; copy types are `TexelCopyTextureInfo`/`TexelCopyBufferLayout`.
  `paint` is a no-op until a frame is uploaded (empty canvas → egui checkerboard shows).

### Stage 5 — `crates/ui` (EditSession + CanvasCallback + Intent + drain) — DONE

New `state/edit_session.rs` (`EditSession`: document + history + results +
jobs + providers + job_rx + last_uploaded_revision; `Default` creates the job channel
and an empty provider registry). `Host` gains `edit: EditSession`; `SessionState`
gains read-mirror `result_count` + `selected_result`. `Intent` gained `Command(Box<dyn
core::Command>)`, `Undo`, `Redo`, `SubmitGenerateJob{prompt}`,
`InsertSelectedResultAsSprite`, `SelectResult(usize)`; `apply_intent` routes commands
through `history.execute` and submits jobs via the capability lookup (the two big arms
extracted to `insert_selected_result`/`submit_generate_job` to clear the
`too_many_lines` lint). `drain_background` now also drains `edit.job_rx` (collect-then-
process to avoid the channel/field borrow overlap), refreshing the mirror + AI status +
repaint. `CanvasCallback` is no longer zero-sized: it carries `Option<CanvasFrame>` and
uploads in `prepare` (`get_mut::<ViewportRenderer>()` + `upload_frame`); `paint` only
draws. `canvas_stage` reads the real `active_sprite_size()`, composites
`composite_active` only when `revision != last_uploaded_revision`, and the HUD shows the
real size. Cargo: ui += `pixhaus-core`. **Green:** clippy clean, 81 tests pass (new
command/undo/redo intent tests + the callback test), `cargo build --workspace` clean.

Decisions / notes:
- The live `EditSession` is on `Host`, NOT in `SessionState` — panels read the
  read-only `SessionState` mirror through `ContribCtx`; only `apply_intent`/
  `drain_background`/`canvas_stage` touch `EditSession`. Keeps the deferred-intent
  invariant intact.
- `SubmitGenerateJob` calls `jobs.submit` → `tokio::spawn`, so it needs the ambient
  runtime (fine in the app; the Stage 7 headless test uses `#[tokio::test]`). The
  Stage 5 unit tests exercise only the non-spawning arms (Command/Undo/Redo).
- egui-wgpu `prepare` is where the `queue.write_texture` upload belongs (`&mut`
  resources); `paint` is read-only (`&`). The `CanvasFrame` rides an `Arc<Vec<u8>>` so
  the per-frame callback move is a refcount bump, not a pixel copy.

### Stage 6 — `modules/generation` + `canvas_stage` wiring — DONE

`PromptPanel`'s Generate button now pushes `Intent::SubmitGenerateJob { prompt:
scratch.clone() }` (was `RunAction`). `ResultsPanel` reads the real
`session.result_count`/`selected_result` mirror: renders clickable cards (`result_card`
now returns its `Response` with `Sense::click()`) that push `SelectResult(i)`, an
empty-state hint when there are no results, "Insert as new sprite" →
`InsertSelectedResultAsSprite`, and "Generate more" → `SubmitGenerateJob` reusing a new
`session.last_prompt` mirror (set in `submit_generate_job` via `clone_from`). `app`'s
`build_host` registers the mock provider (`pixhaus_mod_providers::register(&mut
host.edit.providers)`) after the module loop; `app/Cargo.toml` += `pixhaus-mod-providers`.
No new deps in `modules/generation` (panels only use `pixhaus_ui::Intent` + the
`SessionState` mirror). **Green:** clippy clean (ui/generation/app), generation + app
tests pass.

Decisions / notes:
- "Use selected" / "Create variations" stay mock (`RunAction`) — not part of the apply
  loop. The palette-registered `gen.*` actions are unchanged (still mock when invoked
  from the palette); the panel buttons are the real path.
- Per-result thumbnail textures (`register_native_texture`) are a documented follow-up;
  cards use the tinted placeholder keyed by index this round.

### Stage 7 — end-to-end verification — HEADLESS GREEN

Added `crates/ui/tests/generate_loop.rs` (`#[tokio::test]`): registers the mock
provider, submits a prompt, polls `drain_background` until the result lands, inserts it
as a sprite via `InsertSelectedResultAsSprite`, asserts the revision advanced + the
sprite composites to 64x64, then undoes it. ui dev-deps += `tokio` +
`pixhaus-mod-providers`. **Full session gate green:** `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace`
(166 tests), `cargo test --doc --workspace`, `cargo deny check` all pass.

Remaining (user): the interactive visual check — `cargo run -p pixhaus-app`, Cmd+4,
type a prompt, Generate, see the result card, Insert as new sprite, see the sprite on
the canvas, undo. The headless test proves the data/command/job path; the GPU upload
path is proven by the render crate's upload test; only the on-screen pixels need a human
eye (the agent can't drive the GUI window).

**Post-verification fix (from the first interactive run):** the document-mutating
intent arms (`Command`/`Undo`/`Redo`/`InsertSelectedResultAsSprite`) now call
`ctx.request_repaint()`. Without it, the mutation happens in the post-frame intent drain
but egui goes idle (no pending input), so the canvas didn't recomposite the new sprite
until the next mouse move. `SubmitGenerateJob` also repaints so the "Working" status
shows immediately. Two UX notes surfaced: (1) the Results cards are **placeholder**
thumbnails (tinted blobs), not the real generated image — the inserted canvas sprite is
the mock's deterministic shape (real per-result thumbnails are the documented follow-up);
(2) selecting a card only highlights it — "Insert as new sprite" is what draws it
(results are transient until applied, per the bible).
