# Vertical slice — implementation plan and progress log

Living plan for implementing `docs/native-ui-vertical-slice.md` in the v2 clean-slate
worktree. Update the checkboxes and the "Current state" note as work lands. This file
is the durable record across sessions — read it first when resuming.

## The goal (verbatim intent)

One sprite, one loop, working against a real backend, in the native `shell` binary:
create a sprite -> generate a reference sheet, approve a variant -> generate one
animation -> frames land in the timeline -> press play, watch it loop on the canvas.
Real `ai/` code against a configured backend. Not a mock, not a screenshot.

## Ground truth about v2

v2 is a deliberate clean slate: `Cargo.toml` had `members = []`, no `core`/`io`/`ai`.
Directive: port only the code the slice needs, adapted to the new crates — do NOT copy
whole crates. Reference source lives in the main checkout at
`C:\Users\luism\Documents\GitHub\pixhaus` (`core/`, `io/`, `ai/`, `app/`).

## Crate topology (target)

- `core/`   — minimal port: project model (sprite/layer/cel/frame/animation/tag),
  `PixelBuffer`, compositor + blend math, `normalize_frames`. No drawing/undo/selection.
- `ai/`     — minimal port: plugin verb runtime, `reference_sheet` verb, `compose`,
  the FAL backend, backend keys. Depends on `pixhaus-core`.
- `render/` — new: wgpu `ViewportRenderer` (SPRITE program only), viewport math, tile
  cache. egui-independent (depends on `core` + wgpu). Embedded via `egui_wgpu::Callback`.
- `shell/`  — new: eframe + egui binary. Owns `DocumentStore` (plain field, `&mut self`),
  a tokio runtime + results channel, the animation orchestration (ported from
  `app/src/commands/animation.rs`, IPC stripped), and a small `ffmpeg` video-decode module.

## Backend decision

Target = **FAL**. It is the only backend that covers the whole slice in one adapter:
- `IMAGE_GENERATION` — reference sheet + animation first frame
- `IMAGE_TO_VIDEO`   — the long-running clip job
- `BACKGROUND_REMOVAL` — per-frame bg strip
OpenAI cannot do i2v or bg-removal; Replicate cannot do bg-removal. If the user only has
an OpenAI key, reference-sheet generation still runs for real; the i2v animation step
needs a FAL key. Flag at verification.

`decode_video` shells out to `ffmpeg` on PATH (io/src/animated/decode.rs) — port as a
small shell module, no LGPL linking.

## The animation pipeline is app orchestration, not ai verbs

Important: there are NO `animation_*` verbs in `ai/`. The clip pipeline lives in
`app/src/commands/animation.rs` as orchestration calling backend capabilities directly:
anchor (build reference image) -> first frame (IMAGE_GENERATION) -> clip
(IMAGE_TO_VIDEO, long-running, polled) -> decode_video (ffmpeg) -> pick_loop_frames /
auto_loop_markers (ai/src/verbs/motion_from_video/frame_pick.rs) -> background removal
(BACKGROUND_REMOVAL) -> normalize_frames (core/src/transforms/normalize.rs) -> integrate
(add layer + cels + frames + FrameTag + Animation to the sprite). Re-implement this
orchestration in `shell/`, driven by the verb runtime for the generation calls.

## Reference source map (main checkout)

- core model: `core/src/project/{mod,sprite,layer,cel,frame,animation,id,color,blend,geometry,user_data,schema}.rs`
- core canvas: `core/src/canvas/{buffer,composite,blend,error,mod}.rs`
- core normalize: `core/src/transforms/normalize.rs`
- ai plugin: `ai/src/plugin/{verb,descriptor,inputs,context,output,progress,error,backend,anchor,preview}.rs`, `ai/src/plugin/runtime/{mod,registry,invocation}.rs`
- ai reference sheet: `ai/src/verbs/reference_sheet/mod.rs`, `ai/src/compose/{mod,builtins,variables}.rs`, `ai/src/verbs/mod.rs` (ctx_fat_backend, call helpers)
- ai backend: `ai/src/backends/{mod,error,keys,fal}.rs`
- frame pick: `ai/src/verbs/motion_from_video/frame_pick.rs`
- video decode: `io/src/animated/decode.rs`
- orchestration reference: `app/src/commands/{project.rs::sprite_add, animation.rs}`, `app/src/state.rs` (AppState/DocumentStore/runtime setup), `app/src/commands/library/reference_sheets.rs`

## Build order (each step independently demoable)

- [x] **P0 Scaffold.** Workspace members core/ai/render/shell. Empty egui window
  (four-panel layout), tokio runtime, results channel pump. Builds + clippy `-D warnings`
  clean. core ported (227 tests pass); ai is a stub until P4.
- [x] **P1 Display.** `render/`: SPRITE shader (WGSL) + viewport math (ported from
  viewport.ts, unit-tested). Hard-coded test frame through the egui-wgpu paint callback.
  Pan (drag) + zoom (wheel, cursor-anchored) wired. Headless wgpu smoke test passes
  (offscreen render -> readback). Shell launches on Vulkan with no startup panic.
- [x] **P2 Sprites.** `shell/`: `DocumentStore` holds a core `Project` + pixel-buffer
  store + monotonic id allocator. Library panel creates/selects sprites (mirrors
  `sprite_add`); selection composites + uploads + fits. Frame compositor over full-canvas
  cels. Tests pass.
- [x] **P3 Playback.** Timeline panel (transport, frame strip, playhead); playback driven
  by `request_repaint_after(Frame::effective_duration_ms)` in `App::logic`, following the
  tag's `loop_direction` via `play_order`. `integrate_frames` (the real P5 function) +
  a demo-animation button prove the loop. Tests cover integrate + play order. App runs
  with no panic.
- [x] **P4 Reference sheet.** ai crate ported (plugin runtime + reference_sheet verb +
  compose + FAL backend; 167 tests pass). Shell `ai.rs` builds the `VerbRuntime`, registers
  the verb + FAL (via `BackendProxy`), and drives `generate_reference_sheet` on the tokio
  runtime, streaming progress + variants over the channel. Inspector "Reference sheet" tab:
  prompt, template, variant count, Generate, candidate strip, Approve-as-anchor. FAL key
  entry (keychain `pixhaus.fal`) shown when no backend is configured. Runtime confirmed
  live (logs "no API key configured" until a key is set). NEEDS a real FAL key to generate.
- [x] **P5 Animation.** `shell/anim.rs` ports the frame-pick logic (auto_loop_markers,
  pick_loop_frames, motion_magnitude, seam_similarity) self-contained over RGBA frames,
  plus a clip decoder with NO external binary: GIF/APNG via the image crate, and mp4
  (H.264) demuxed with the `mp4` crate + decoded with `openh264` (AVCC->Annex-B conversion
  unit-tested). `shell/ai.rs::run_animation` orchestrates
  the real pipeline: first frame (FAL IMAGE_GENERATION conditioned on the anchor) -> clip
  (FAL IMAGE_TO_VIDEO) -> decode (spawn_blocking) -> auto loop markers + pick -> background
  removal (FAL, best-effort per frame) -> normalize_frames (core) -> integrate. Animation
  tab: motion prompt, loop-frame count, fps, Generate, progress; integrate -> frames in
  timeline -> Play loops. Tests cover frame-pick + decode. Full-workspace clippy
  `-D warnings` clean; app launches with no panic.
- [~] **P6 Jobs tray.** Partial: each inspector tab shows a live spinner + progress message
  for its async job, both pumped over the one `ShellMsg` channel. The *unified* tray with
  cancel buttons is a deliberate follow-up (not required for the creation loop).

## Verification (Done)

Launch `shell`; create sprite; generate reference sheet (real backend), approve variant;
generate one animation, integrate; confirm frames in timeline and play loops on canvas.
Plus: `cargo nextest run` headless wgpu smoke test for `render/`; off-UI-thread state
tests for `shell/`; everything builds with clippy `-D warnings`.

## Decisions (2026-05-26)

- User has a FAL key with i2v access; will paste it in the app's key field at runtime.
- Clip decode is pure-Rust (no ffmpeg): mp4/H.264 via `mp4` + `openh264` (BSD-2, builds
  from bundled source with the MSVC toolchain; nasm optional for speed), GIF/APNG via the
  `image` crate. VP9/AV1/HEVC are not decoded — use an mp4/H.264 or GIF i2v model. This
  removed the ffmpeg-on-PATH requirement entirely at the user's request.

## Backends (update)

Two image-generation backends are now ported and registered:
- **OpenAI** (`gpt-image-2`, keychain `pixhaus.openai`) — `IMAGE_GENERATION`/`IMAGE_EDIT`/
  `IMAGE_INPAINT`. Registered at priority 0 (preferred for reference sheets).
- **FAL** (keychain `pixhaus.fal`) — also `IMAGE_TO_VIDEO` + `BACKGROUND_REMOVAL`, so the
  animation pipeline needs FAL. Priority 10.
A reference sheet runs against whichever image-gen backend is configured (OpenAI alone is
enough); animation still needs FAL. The default reference-sheet prompt ("Bit, the Pixhaus
mascot") is ported from `ui/src/sheet/sheet-editor-state.ts` and prefills the UI + headless.

## Headless runner (so the loop can be demonstrated/inspected without the GUI)

`shell/src/headless.rs` adds CLI subcommands (dispatched before the GUI in `main`):
- `shell set-key <FAL_API_KEY>` — store the key in the keychain without the GUI.
- `shell demo [--out DIR]` — synthetic multi-frame sprite (no backend) -> writes frames
  + an infinite-looping `loop.gif`. VERIFIED: produced 8 distinct frames (hue sweep,
  red..cyan..) + `loop.gif`; proves the composite/integrate/loop/GIF-encode path end-to-end.
- `shell gen [--prompt TEXT] [--motion TEXT] [--out DIR]` — the REAL pipeline: FAL
  reference sheet -> approve variant 0 as anchor -> FAL animation (i2v + openh264 decode)
  -> integrate -> writes the looping sprite. One command; needs a stored FAL key.
The orchestration (`ai::run_reference_sheet`, `ai::run_animation`) was refactored to take a
progress callback so the same code path serves both the GUI and the headless runner.

## Reference-sheet generation DEMONSTRATED (2026-05-26)

`shell sheet` ran the real `generate_reference_sheet` verb against real OpenAI gpt-image-2
(key in keychain) and produced two real sprite reference sheets (Bit the mascot:
neutral/happy/angry screens, poses, palette row, labeled callouts) — 1.4-1.5 MB PNGs,
inspected. This is "a sprite you made" via the real ai verb + real backend, in the shell
binary.

Bug fixed along the way: the OpenAI client timeout was 120s, but gpt-image-2 generation
took 165s -> "operation timed out". Raised to 300s (matches the verb's advertised
max_latency). rustls-tls was never the problem (a transient red herring during diagnosis);
reqwest stays on rustls-tls. FAL uses a polling model (180s per request, 600s queue wait),
so the long i2v job is fine without change.

Remaining for the full "animated, looping" sprite: store a FAL key
(`shell set-key fal <key>`) and run `shell gen` — reference sheet (OpenAI) -> anchor ->
FAL image-to-video -> openh264 decode -> integrate -> looping GIF.

## Current state

P0-P5 code-complete. Workspace builds; `cargo clippy --workspace --all-targets -- -D warnings`
clean; `cargo test --workspace` green (core 227, ai 167, render 5, shell 9). The full
creation loop is wired front-to-back in the `shell` binary against the real ai verb runtime
and real FAL backend calls. P6 (unified jobs tray) is the remaining polish.

To run it live (the only things between here and a looping sprite):
1. Launch `shell`; in the inspector "Reference sheet" tab paste a FAL API key (stored in the
   OS keychain under `pixhaus.fal`). The runtime then registers FAL.
2. Generate a reference sheet, approve a variant (becomes the sprite's anchor).
3. Animation tab -> Generate. Clip decode is pure-Rust (mp4/H.264 or GIF) — no ffmpeg.
4. Frames land in the timeline; press Play to loop on the canvas.
