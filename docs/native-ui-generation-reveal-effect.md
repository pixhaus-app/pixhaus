# AI generation reveal effect — design spec

Design spec for the studio's generation loading/reveal animation. Not started.
Read this before implementing; it pairs with `native-ui-ai-studio-redesign.md`.

## Context

When you Generate in the AI studio, the center viewport sits on a static "results
appear in the thread" hint until the image lands. We want that wait to be the
show: a field of scattered "pixels" that drift while the request is in flight,
begin to cohere as the first streamed partial arrives, and snap into the crisp
final image when it lands. It turns dead time into feedback about what's being
made, sized to the actual output.

The inputs already exist. We know the target size before the first byte (the
request carries width/height). The anchor path already streams coarse partial
frames (`ShellMsg::SheetPartial { pixels: PixelData }`, produced from the verb's
`VerbProgressEvent::PartialPixels`). And the center is already a wgpu region (the
canvas paint-callback pattern), so a custom effect pass is incremental, not new
infrastructure.

## Decisions (locked with the user)

- **Add a resolution / aspect-ratio control** to the generation inputs that feeds
  both the request and the effect's layout (Feature A).
- **Fixed decorative particle grid** — a capped grid (~128 on the long side, or
  the sprite's native count when smaller), laid out to the target aspect,
  independent of the output resolution. Honors the 8K perf bound.
- **Both stages** — Anchor and First-frame. The anchor streams partials so it
  plays all three phases; the first-frame has no streaming today, so it plays
  phase 1 → phase 3 (scatter → snap), skipping the mid-transition.

## Goals / non-goals

- Goal: a GPU effect in the center viewport, driven by the existing generation
  states, that reads as "pixels assembling the image".
- Goal: a resolution/ratio control so the user picks the output shape.
- Non-goal: changing the backends or the generation pipeline. Non-goal: a
  full-screen post-process over the whole egui UI (the effect is region-scoped).

## Feature A — Target resolution / aspect-ratio control (prerequisite)

A control in the Anchor composition inputs (`cockpit_inputs`, near the Output /
variants / seed row in `shell/src/cockpit.rs`) to pick the output shape:

- An aspect-ratio dropdown (1:1, 3:4, 4:3, 16:9, 9:16, Custom) plus, for Custom,
  width/height drags. Store the chosen target on `ShellApp` (e.g.
  `ck_target: TargetSize { w, h }`), defaulting to the active sprite's canvas.
- Feeds generation:
  - First-frame path is trivial — `FirstFrameJob::Generate { canvas, .. }`
    already takes explicit dims (`ai.rs`); pass the chosen size instead of the
    raw sprite canvas.
  - Anchor path needs the dims threaded into the reference-sheet request:
    `SheetJob` gains a target size, passed through to the verb's
    `ImageGenRequest { width, height }` (today the verb derives dims from the
    structure). This is the one non-trivial wire — touch points are `SheetJob`
    (`ai.rs`), `into_inputs`, and the reference-sheet verb's dimension logic in
    the `ai/` crate.
- Feeds the effect: the same `TargetSize` sets the particle grid's aspect.

Feature A is independently shippable and useful on its own; the effect depends on
knowing the target shape but the control can land first.

## Feature B — The reveal effect

### The three phases (a small state machine)

A `RevealState` per stage tracks `{ active, phase, target: TargetSize, source:
Option<TextureHandle>, started_at, assembly: f32, reveal: f32 }`, advanced each
frame from the generation signals already on `ShellApp`:

- **Phase 1 — Scatter (status Running, no partial yet).** Particles drift in a
  scattered cloud over the image rect with placeholder colour (sampled from the
  active palette, or low-amplitude noise) and a time-driven shimmer. `assembly`
  hovers low; `reveal` 0.
- **Phase 2 — Cohere (a partial arrived; anchor only).** The streamed
  `SheetPartial` pixels become `source`; raise `assembly` toward a partial-settle
  level (~0.6) and `reveal` so the partial shows through, but keep enough scatter
  that it still reads as forming.
- **Phase 3 — Snap (final landed).** `source` becomes the final image; drive
  `assembly` → 1 and `reveal` → 1 on a fast ease (~0.35 s); particles converge to
  their cells and form the crisp image, then the effect deactivates and the
  static `refine_surface` viewport takes over showing the landed result.

First-frame: no `SheetPartial`, so on `FirstFrameDone` it goes phase 1 → phase 3.

### Where it renders (integration)

In `studio_anchor_surface` / `studio_first_frame_surface`: when the stage's
`RevealState` is `active` (a generation is in flight, or a just-finished phase-3
snap is still animating), render the reveal effect in the center instead of the
static viewport / hint. While active, call `ui.ctx().request_repaint()` so the
animation runs (egui repaints on demand). On deactivate, fall through to the
existing `refine_surface` on the now-selected result.

### The GPU effect (particles + shader)

A self-contained effect module in the shell (e.g. `shell/src/reveal.rs`) with its
own `egui_wgpu::CallbackTrait` impl and WGSL — mirroring the canvas callback
split, but kept in the shell since it is decorative (no need to widen the
UI-agnostic `render/` crate's surface; revisit if it grows).

- One instanced draw of a fixed grid (~128 × N to the target aspect; ≈16k
  instances). Each instance derives a stable scattered start from a hash of its
  index; the target is its cell centre in the image rect.
- Vertex stage: `pos = lerp(scattered_start, cell_target, ease(assembly))` plus a
  per-instance jitter that decays as `assembly → 1`.
- Fragment stage: colour = `mix(placeholder, sample(source, cell_uv), reveal)`,
  where `source` is the partial or final texture (or a 1×1 fallback in phase 1).

### Inputs / uniforms

A small uniform block: target/grid dims (vec2), image rect within the viewport
(from `PaintCallbackInfo` / `ViewportInPixels`), `time`, `assembly`, `reveal`, and
a `has_source` flag. The `source` texture is registered from the partial/final
PNG (decode → upload, reuse the existing texture-upload path). Per-instance
randomness is computed in-shader from the instance index — no per-instance
buffer churn.

### Perf

The grid is fixed and independent of output resolution, so an 8K request still
animates ~16k instances in one draw call — well within the 8K bound. The only
per-generation upload is the small partial/final image as a texture. Repaint only
while a `RevealState` is active.

### Fallbacks

- No partial streaming (first-frame, or a backend like OpenAI that returns only
  the final): skip phase 2 — phase 1 holds until `Done`, then phase 3 snaps.
- Decode failure on a partial/final: keep the current phase's placeholder; the
  static viewport still shows the real result once it lands.

### Settings toggle

Gate the effect behind a preference (default on), persisted with the other UI
prefs (`settings`/`Storage`). Off → the center keeps today's static hint during
generation. Keeps it opt-out for anyone who finds it distracting or slow.

## State and data flow

```
Generate clicked ──> RevealState.active = true, phase = Scatter
SheetProgress/Running ──> phase stays Scatter
SheetPartial { pixels } ──(anchor)── source = partial tex; phase = Cohere
SheetDone / FirstFrameDone ──> source = final tex; phase = Snap (assembly,reveal -> 1)
snap finished ──> RevealState.active = false ──> refine_surface shows the result
```

Two `RevealState`s live on `StudioState` (one per stage), reset on sprite switch
alongside the rest of the session (`sync_studio_owner`).

## Files

- `shell/src/cockpit.rs` — the resolution/ratio control in `cockpit_inputs`.
- `shell/src/ai.rs` — `SheetJob`/`FirstFrameJob` carry the target size; thread it
  into the requests; the partial pixels already flow via `SheetPartial`.
- `ai/` reference-sheet verb — accept explicit target dims (Feature A's one
  non-trivial wire).
- `shell/src/reveal.rs` (new) — the `CallbackTrait` effect, WGSL, and `RevealState`.
- `shell/src/studio.rs` — `RevealState` on `StudioState`; drive it from the gen
  states in the anchor/first-frame surfaces; render the callback while active.
- `shell/src/app.rs` — feed `SheetPartial` / `*Done` into the active `RevealState`
  (instead of, or alongside, the retired canvas preview); the settings toggle.

## Verification

- Gates: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo nextest run --workspace`, `cargo test --doc`, `cargo doc`.
- Unit tests: the `RevealState` phase transitions for the event sequences (with
  and without a partial); the `assembly`/`reveal` easing math; the grid layout for
  a non-square target aspect.
- Manual (`cargo run -p pixhaus-shell`): pick a non-square ratio, Generate on the
  Anchor stage, and watch scatter → cohere (on the streamed partial) → snap to the
  final, which then sits in the static pan/zoom viewport. Repeat on First-frame
  (scatter → snap, no mid-phase). Toggle the setting off and confirm the static
  hint returns.

## Build order / decomposition

1. **Feature A** — resolution/ratio control (first-frame dims, then the anchor
   verb dims). Useful on its own; gives the effect its target shape.
2. **Feature B, skeleton** — `RevealState` + the phase state machine + a plain
   (non-shader) placeholder fill in the center, wired to the gen states. Proves
   the integration without the GPU work.
3. **Feature B, shader** — the instanced particle callback + WGSL, then the
   partial/final texture feed and the snap.
4. Settings toggle + tests.
