# Plan: staged, inspectable animation pipeline (reference → first frame → video → frames)

Status: proposal, for review. No code changed yet.

## Why

The current animation studio is a single opaque shot: pick a type/direction, hit Generate, and
the reference anchor goes straight into image-to-video, gets frame-picked, normalized, and
landed — with no way to see or fix the intermediate artifacts. When a run looks wrong (the
recent magenta-cropped clip, the blank normalized frames) there is nothing to inspect and
nothing to tune. Two model choices are also baked in: the anchor is fed to i2v raw (Wan 2.1),
which handles a multi-panel reference sheet badly.

This plan rebuilds the studio as an explicit, inspectable, four-stage pipeline. Each stage
produces a reviewable artifact you can see, regenerate, and refine before committing to the
next — so the run can be debugged and the output optimized at every step.

> Reference sheet → **first frame** (gpt-image-2 edit, refine with reroll + inpaint) →
> **raw video** (fal Seedance i2v, inspect and regenerate) → **extract frames** (loop picker
> → normalize → integrate).

### Decisions taken (user-approved)

1. **First frame = image edit.** Create the first frame with the OpenAI `/images/edits`
   endpoint (`gpt-image-2`), passing the entity's reference sheet as the input image and a
   prompt that isolates a single direction-facing pose on a flat magenta background. Not a
   from-scratch generation, and not the raw multi-panel sheet fed to i2v.
2. **Refine = reroll + inpaint.** Edit the prompt and reroll for fresh candidates, and add
   mask-based inpaint edits (paint a region on the first frame, describe the fix) via the same
   edits endpoint.
3. **Seedance is the default video model, selectable.** Default the i2v step to fal Seedance,
   with a dropdown to switch (Wan 2.1 stays as fallback). Exact fal endpoint slug and its
   parameter shape to be confirmed against fal docs (see open items).
4. **Keep the loop picker.** Extraction stays the auto-loop-detect + pick-N picker, with the
   raw video visible so markers can be tuned. Not "dump every frame".

## What already exists (reused, not rebuilt)

- **OpenAI image edit** — `ai/src/backends/openai.rs::edit_image` posts `image` + optional
  `mask` + `prompt` to `/images/edits` with `gpt-image-2`; the backend advertises
  `IMAGE_EDIT` and `IMAGE_INPAINT`. `ImageEditRequest` (`ai/src/backends/mod.rs`) already
  carries `image`, `mask`, `prompt`, `num_images`, `reference_images`.
- **fal i2v** — `ai/src/backends/fal.rs::call_video_endpoint` + `build_fal_i2v_body`;
  `ImageToVideoRequest.model` is an overridable endpoint slug (defaults to `FAL_I2V` =
  `fal-ai/wan-i2v`). Queue polling was fixed this session.
- **Pick / normalize / integrate** — `app/src/commands/animation.rs`:
  `animation_pick_frames`, `animation_normalize` (reference-height fit fixed this session),
  `animation_integrate`, plus `animation_derive_neutral` / `animation_set` /
  `animation_reroll_dependents`. `motion_from_video` (loop markers) and `io::animated::decode`
  unchanged.
- **UI** — `FramePicker`, `CandidateReview`, `NormalizationReview`, `FramePreview`
  (`fit` mode added this session), `AnimationSet`. The studio shell, request strip, and
  staleness cascade stay.
- **Anchor resolution** — `resolve_anchor_kind(project, entity, AnchorKind, cache)` in
  `app/src/commands/verbs.rs` returns the reference image bytes for a direction.

## Target flow

```text
Stage 0  Reference     show resolved anchor for the chosen direction
Stage 1  First frame   gpt-image-2 edit(reference sheet, prompt) -> candidates
                        reroll / pick / inpaint-edit -> approved first frame (on magenta)
Stage 2  Video         Seedance i2v(approved first frame, motion prompt) -> raw clip
                        inspect raw video / regenerate / switch model
Stage 3  Extract       loop picker over the clip -> normalize -> candidate -> integrate
```

Persistent visibility: a stage strip across the top shows the **reference**, the **approved
first frame**, and the **raw video** thumbnails at all times; clicking one expands it. The
center pane hosts the active stage. The stage advances only when its artifact is approved, and
any earlier stage can be reopened (which invalidates downstream artifacts).

## Part 1 — Backend

All in `app/src/commands/animation.rs` unless noted.

### 1a. Fetch the reference anchor for display — `animation_get_anchor`

New command: `animation_get_anchor(entity_id, direction) -> AnchorImageDto { png_base64,
width, height }`. Resolves `AnchorKind::Directional(dir)` via `resolve_anchor_kind`, returns
the PNG so Stage 0 can show what the first frame is derived from. Errors clearly when the
entity has no approved anchor.

### 1b. First-frame edit — `animation_generate_first_frame`

New command:

```text
animation_generate_first_frame(
    entity_id, direction, animation_kind,
    prompt: Option<String>,        // user choreography / pose description
    base_image_base64: Option<String>,  // for inpaint iterations; default = reference anchor
    mask_base64: Option<String>,   // white = edit, transparent/black = keep
    num_images, seed,
) -> FirstFrameResult { images: Vec<{ png_base64, width, height }> }
```

- Source image: `base_image_base64` when present (an inpaint pass over the current first
  frame), otherwise the resolved reference anchor.
- Select an `IMAGE_EDIT` backend (`select_backend(IMAGE_EDIT, ...)`); error "no image-edit
  backend configured — add an OpenAI key" if none.
- Build `ImageEditRequest { image, mask, prompt: first_frame_prompt(kind, dir, user_prompt),
  num_images, .. }` and invoke via the existing `invoke_selected_backend`.
- `fn first_frame_prompt(kind, dir, user) -> String`: "A single <kind> pose of the character,
  facing <dir>, full body centered in frame, on a flat solid magenta (#FF00FF) background. No
  other poses, no panels, no text, no ground shadow." + the user's description when present.
  The magenta background is what the downstream chroma key removes, so it is requested
  explicitly. (If `gpt-image-2` ignores the background instruction often, add an explicit
  `background: "opaque"` form field + a post-pass composite onto magenta — see open items.)
- Edit endpoint sizes are constrained to `1024x1024` / `1536x1024` / `1024x1536` / `auto`;
  pick by direction aspect (default `1024x1024`). The frame is downscaled later by normalize,
  so a large edit canvas is fine.
- Returns the candidate image(s); the UI keeps the approved one in memory.

### 1c. Video from the approved first frame — extend `animation_generate_clip`

The clip command stops resolving the anchor itself and instead animates the **approved first
frame** the UI passes in:

- Add `first_frame_base64: String` (the approved Stage 1 output); drop the internal
  `resolve_anchor_kind` call for the source image.
- Add `model: Option<String>` (the fal endpoint slug). Default to a new `FAL_SEEDANCE` const.
- `I2V_CLIP_FRAMES` becomes model-aware: Wan needs ≥81 frames; Seedance is typically
  driven by `duration` + `resolution` rather than a frame count. Introduce per-model body
  mapping in `fal.rs` (see 1d).
- Keep returning `GenerateClipResult { clip_base64, mime, fps, target_count }`.

### 1d. Seedance model wiring — `ai/src/backends/fal.rs`

- Add `pub const FAL_SEEDANCE: &str = "fal-ai/bytedance/seedance/..."` (confirm exact slug).
- `build_fal_i2v_body` becomes model-aware: Wan takes `num_frames` + `frames_per_second`;
  Seedance takes its own params (likely `duration`, `resolution`, `prompt`, `image_url`).
  Branch on the endpoint slug, or add a small `FalI2vParams` mapping per model. Keep the
  existing Wan body for the fallback path and its test.
- Confirm Seedance's output shape flows through `decode_fal_video` (it already falls back to
  the first `url`-like field).

### 1e. Capability + selection

`IMAGE_EDIT` and `IMAGE_TO_VIDEO` selection both go through `verb_runtime.select_backend(cap,
&VerbId)` as `animation_generate_clip` already does. Register the two new commands in
`app/src/lib.rs`.

## Part 2 — UI state machine (`ui/src/animation/animation-studio-state.ts`)

Add an explicit stage and the per-stage artifacts:

```ts
export type Stage = "reference" | "first_frame" | "video" | "extract";
export const [stage, setStage] = createSignal<Stage>("reference");

// Stage 0
export const [referenceImage, setReferenceImage] = createSignal<AnchorImage | null>(null);
// Stage 1
export const [firstFrameCandidates, setFirstFrameCandidates] = createSignal<FirstFrame[]>([]);
export const [approvedFirstFrame, setApprovedFirstFrame] = createSignal<FirstFrame | null>(null);
export const [firstFramePrompt, setFirstFramePrompt] = createSignal("");
// Stage 2
export const [videoModel, setVideoModel] = createSignal<"seedance" | "wan">("seedance");
// pendingClip stays as the raw clip artifact; reuse it as the Stage 2 output.
```

`resetStudio()` clears all of these and sets `stage = "reference"`. Approving an artifact
advances the stage; reopening an earlier stage invalidates downstream (`approvedFirstFrame`
change clears `pendingClip` and candidates). Keep `candidates`, `selectedCandidate`,
`requests`, transport, `loopDirectionFor`.

## Part 3 — UI components (`ui/src/animation/`)

- **`StageStrip.tsx`** (new) — the persistent reference / first-frame / video thumbnail row
  with the active-stage indicator; clicking a thumb reopens that stage.
- **`AnimationStudio.tsx`** — becomes the stage host: renders Stage 0/1/2/3 by `stage()`, keeps
  the request strip and footer `AnimationSet`. `generate()` is replaced by per-stage actions.
- **Stage 0 — `ReferenceStage.tsx`** (new, small) — loads `animation_get_anchor` on entry and
  shows the anchor with the existing `fit` preview; a "Use as base" button advances to Stage 1.
- **Stage 1 — `FirstFrameStage.tsx`** (new) — prompt box + Generate (calls
  `animationGenerateFirstFrame`), a candidate strip (reuse the candidate-thumb styles), pick to
  approve, and an **inpaint sub-mode**: a mask canvas overlay (brush paints white onto a
  transparent mask the size of the first frame), a fix prompt, and Apply (re-calls the command
  with `base_image_base64` = current first frame + `mask_base64`). Approve → Stage 2.
- **Stage 2 — `VideoStage.tsx`** (new) — motion prompt + model dropdown (Seedance / Wan) +
  fps/seed, Generate (calls `animationGenerateClip` with the approved first frame), and a raw
  `<video>` element playing the returned clip (object URL from the base64 bytes) so the raw
  output is fully inspectable. Regenerate in place; "Extract frames" advances to Stage 3.
- **Stage 3 — extract** — the existing `FramePicker` → `usePickedFrames` → `animationNormalize`
  → `CandidateReview` → `NormalizationReview` → `animationIntegrate` flow, unchanged except it
  reads the clip from the Stage 2 artifact.

### IPC wrappers (`ui/src/lib/commands/animation.ts`)

- `animationGetAnchor(args) -> AnchorImage`
- `animationGenerateFirstFrame(args) -> { images: FirstFrame[] }`
- extend `animationGenerateClip` args with `first_frame_base64` + `model`.

## Critical files

- New (Rust): commands `animation_get_anchor`, `animation_generate_first_frame` in
  `app/src/commands/animation.rs`; `FAL_SEEDANCE` + model-aware `build_fal_i2v_body` in
  `ai/src/backends/fal.rs`; register commands in `app/src/lib.rs`.
- Edit (Rust): `animation_generate_clip` (first-frame input + model arg) in
  `app/src/commands/animation.rs`.
- New (UI): `StageStrip.tsx`, `ReferenceStage.tsx`, `FirstFrameStage.tsx`, `VideoStage.tsx` in
  `ui/src/animation/`.
- Edit (UI): `animation-studio-state.ts` (stage machine + artifacts), `AnimationStudio.tsx`
  (stage host), `AnimationControls.tsx` (fold per-stage controls or retire),
  `ui/src/lib/commands/animation.ts` (wrappers).

## Verification

- `cargo build -p pixhaus-app`; `cargo clippy -p pixhaus-ai -p pixhaus-app --tests
  -- -D warnings`; `cargo nextest run -p pixhaus-ai -p pixhaus-app`;
  `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --document-private-items`
  (Stop-hook gate). `cd ui && npx tsc --noEmit && npx eslint src && npx vitest run`.
- Unit tests: `first_frame_prompt` (kind/direction/magenta wording), model-aware
  `build_fal_i2v_body` (Wan body unchanged; Seedance body carries duration/resolution).
- End-to-end (`pnpm dev`): open the studio for an entity with an approved anchor →
  Stage 0 shows the reference → Stage 1 generates a single magenta-background pose, reroll and
  one inpaint fix work, approve → Stage 2 animates it with Seedance and the raw video plays,
  regenerate works → Stage 3 picks a loop, normalizes (subject fits the cell), integrates a
  tagged animation that plays on the canvas. The stage strip shows all three artifacts
  throughout.

## Open items / risks

- **Seedance slug + params.** The exact fal endpoint and whether it takes `duration` vs
  `num_frames` must be confirmed against fal docs; the model-aware body builder isolates this
  to one place. Until confirmed, Wan stays the working default.
- **Magenta background from gpt-image edit.** If the model ignores the "flat magenta
  background" instruction, add an explicit `background: "opaque"` field to the edit form and/or
  composite the (possibly transparent) edit output onto a magenta canvas server-side before
  i2v. Either keeps the chroma key downstream honest.
- **Inpaint mask UX.** The mask canvas is the heaviest new piece. Start minimal: one brush,
  white-on-transparent, full-frame mask, no feather. Reuse brush-size UI patterns but a
  dedicated overlay canvas — do not entangle with the main editor tool state.
- **Cost/latency.** Each stage is a paid call (gpt-image edit, Seedance clip). The request
  strip already surfaces elapsed time; keep per-stage Generate explicit (no auto-chaining) so
  the user controls spend.
- **Downscale quality.** Normalize still uses nearest-neighbor to the cell size; a future pass
  could area-average + palette-snap. Out of scope here.
