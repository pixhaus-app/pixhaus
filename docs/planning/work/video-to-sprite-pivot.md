# Plan: pivot the animation studio to video-to-sprite (remove grid)

Status: proposal, for review. No code changed yet.

## Why

Grid image-generation (the FalSprite-derived `AnimatedSpriteSheetVerb`) produces unreliable
sprite animations: a model asked for a g×g grid of distinct poses often returns a single
character, frames come back cropped/inconsistent, and the path needs heavy normalization to
be usable. Image-to-video (i2v) is the better mechanic and is already mostly built in this
branch.

This plan **removes the grid path entirely** and makes the animation studio a single i2v
flow:

> Use the entity's **reference anchor image directly** as the first frame → generate the
> motion with an image-to-video backend → **extract frames** with the loop frame-picker →
> normalize → land as a tagged animation on the timeline.

### Decisions taken (for this plan)

1. **Fully delete** the grid verb, its tests, the standalone "Animated Sprite Sheet" form,
   and its command-palette entry. The studio becomes i2v-only.
2. **Feed the anchor image directly** into i2v as the first frame — no separate
   still-generation step. (Tradeoff: the anchor is often a multi-panel reference sheet,
   which i2v handles imperfectly; accepted for simplicity/cost. Future: derive a clean
   single-pose directional anchor first.)
3. **Keep the loop frame-picker** (auto-detect loop markers + pick N evenly-spaced frames,
   user-adjustable) — best for idle/walk loops.

## What already exists (reused, not rebuilt)

The i2v pipeline does **not** depend on the grid verb, so removal is clean:

- `app/src/commands/animation.rs`
  - `animation_generate_walk_clip` (+ `WalkClipArgs`/`WalkClipResult`, `generate_walk_still`,
    `invoke_selected_backend`) — walk-specific today; to be generalized.
  - `animation_pick_frames` — decode clip → auto loop markers → pick frames. Animation-agnostic.
  - `animation_normalize` — chroma-key + baseline lock + scale + re-pad.
  - `animation_integrate` — lands layer + cels + frames + tag + engine animation; already
    accepts `animation_kind` + `direction` for the staleness cascade.
  - `animation_derive_neutral`, `animation_set`, `animation_reroll_dependents` — unchanged.
- `ai/src/verbs/motion_from_video/frame_pick.rs` — `auto_loop_markers`, `pick_loop_frames`,
  `seam_similarity`. Reused.
- `io/src/animated/decode.rs` — `decode_video` (ffmpeg). Reused.
- `ui/src/animation/` — `FramePicker.tsx`, `CandidateReview.tsx`, `NormalizationReview.tsx`,
  `FramePreview.tsx`, `AnimationSet.tsx`. Reused.

## Part 1 — Delete the grid path

**Delete:**

- `ai/src/verbs/animated_sprite_sheet/` (whole module: `mod.rs`, `grid.rs`, `prompts.rs`,
  `prompts/`). ~1860 + grid/prompts lines.
- `ai/tests/animated_sprite_sheet_lifecycle.rs`.
- `ui/src/verbs/animated-sprite-sheet/` (whole dir: `AnimatedSpriteSheetForm.tsx`,
  `AnimatedSpriteSheetHost.tsx`, `form-logic.ts`, `form-logic.test.ts`, `state.ts`,
  `types.ts`).

**Edit references:**

| File | Change |
|---|---|
| `ai/src/verbs/mod.rs` | Remove `pub mod animated_sprite_sheet;` and `pub use animated_sprite_sheet::AnimatedSpriteSheetVerb;` (+ any other re-export of its symbols). |
| `app/src/state.rs` | Drop `AnimatedSpriteSheetVerb` from the `use pixhaus_ai::verbs::{…}` import; remove `register_builtin(&runtime, AnimatedSpriteSheetVerb::new());`; remove `"pixhaus.builtin.animated_sprite_sheet"` from the registered-verb-id test expectation. |
| `ui/src/command-palette/command-registry.ts` | Remove the `ai:animated-sprite-sheet` entry and the `openAnimatedSpriteSheetForm` import. Keep `ai:animations` (the studio). |
| `ui/src/shell/Shell.tsx` (+ hosts) | Remove the `AnimatedSpriteSheetHost` mount/import if present. |

Confirm nothing else imports `pixhaus_ai::verbs::animated_sprite_sheet`. `app/src/commands/
animation.rs` uses `verbs::variant` (neutral reset) and `verbs::motion_from_video` (frame
pick) — neither is deleted. The compiler will name any straggler imports.

## Part 2 — Generalize the i2v command (`app/src/commands/animation.rs`)

The command is walk-specific and generates a still first. Rework it to feed the anchor
directly and support any animation type:

- Rename `WalkClipArgs` → `GenerateClipArgs`; add `animation_kind: Option<String>`
  (`idle`/`walk`/`attack`/`custom`) and `choreography: Option<String>`. Keep `entity_id`,
  `direction`, `num_frames`, `fps`, `seed`. Rename `WalkClipResult` → `GenerateClipResult`
  (same fields: `clip_base64`, `mime`, `fps`, `target_count`).
- Rename `animation_generate_walk_clip` → `animation_generate_clip`. New body:
  1. Resolve the source frame from the anchor: `resolve_anchor_kind(project, Some(entity),
     AnchorKind::Directional(dir), cache)` → `image_bytes`. Error clearly if none
     ("approve a reference-sheet anchor first").
  2. `select_backend(IMAGE_TO_VIDEO, …)`; error "no image-to-video backend configured — add
     a fal.ai or Replicate key" if none.
  3. Build the motion prompt from `animation_kind` + `choreography` + facing via a new
     `fn motion_prompt(kind, dir_label, choreography) -> String`:
     - idle → "stands in place, a looping idle — subtle breathing and weight shift; no
       walking, no turning"
     - walk → "walks in place, a clean looping walk cycle; feet on a constant ground line"
     - attack → "performs an attack — anticipation, strike, follow-through"
     - custom → the `choreography` text (neutral fallback if empty)
     all suffixed with the facing + shared negatives (no pivots / quarter-turns / background
     / particles / camera motion).
  4. `ImageToVideoRequest { image: anchor_bytes, prompt, negative_prompt, num_frames, fps,
     seed }` → `invoke_selected_backend` → return `GenerateClipResult`.
- **Delete** `generate_walk_still` and the `IMAGE_GENERATION` backend selection (no
  still-gen).
- `app/src/lib.rs`: rename the registered command to `animation_generate_clip`.
- Add a unit test for `motion_prompt` (each kind contains its keyword; custom uses the
  choreography text).

## Part 3 — Studio UI becomes i2v-only (`ui/src/animation/`)

- `animation-studio-state.ts`: delete `GenMode`, `genMode`, `gridSize`, `layoutGuide` (+
  setters). Keep `animType`, `direction`, `i2vFrameCount` ("frames"), `fps`, `choreography`,
  `seed`, `cellSize` (output sprite-frame size = normalize canvas). Keep `loopDirectionFor`,
  `Candidate`, `PendingClip`.
- `AnimationControls.tsx`: remove the MODE toggle, the grid-size slider, and the layout-guide
  checkbox (+ imports). Keep TYPE (drop the "i2v" badge — everything is i2v now), DIRECTION,
  a frames slider (8–24), CHOREOGRAPHY, Advanced (fps, frame size, seed).
- `AnimationStudio.tsx`: `generate()` always runs i2v — call `generateClip(entityId)` which
  calls `animationGenerateClip({ entity_id, direction, animation_kind: animType()==="custom"
  ? null : animType(), choreography: choreography() || null, num_frames: i2vFrameCount(),
  fps: fps(), seed: seed() })` then `setPendingClip`. Delete `handleGridOutput`, the
  `verbInvoke` grid branch, the `ANIMATED_SPRITE_SHEET_VERB` const, and
  `VerbOutputDto`/`pixelDataToRgbaFrame` (drop the now-unused `verbInvoke` import). The
  FramePicker → `usePickedFrames` (chroma "magenta") → candidate → integrate flow stays.
- `lib/commands/animation.ts`: rename `animationGenerateWalkClip` → `animationGenerateClip`;
  rename/extend args to `GenerateClipArgs` with `animation_kind?` and `choreography?`.

## End-to-end flow after the change

1. Studio opens for an entity that has an approved anchor (no Mode/grid controls).
2. User picks Type (Idle/Walk/Attack/Custom) + Direction, optionally edits Choreography,
   sets frames/fps.
3. Generate → `animation_generate_clip` resolves the anchor image, picks an i2v backend
   (fal/Replicate), animates the anchor with the type's motion prompt, returns the clip.
4. The FramePicker decodes the clip, auto-detects a loop, and previews N picked frames; the
   user can nudge markers/count, then "Use these".
5. Frames are normalized (magenta keyed, baseline-locked, centered) → a candidate.
6. Accept → Integrate lands a layer + cels + frames + frame tag + engine animation with the
   right loop direction (idle → ping-pong; walk/attack → forward) and records the cascade
   edge for staleness.

## Critical files

- Delete: `ai/src/verbs/animated_sprite_sheet/`, `ai/tests/animated_sprite_sheet_lifecycle.rs`,
  `ui/src/verbs/animated-sprite-sheet/`.
- Edit (Rust): `ai/src/verbs/mod.rs`, `app/src/state.rs`, `app/src/commands/animation.rs`,
  `app/src/lib.rs`.
- Edit (UI): `ui/src/animation/{animation-studio-state.ts, AnimationControls.tsx,
  AnimationStudio.tsx}`, `ui/src/lib/commands/animation.ts`,
  `ui/src/command-palette/command-registry.ts`, `ui/src/shell/Shell.tsx`.

## Verification

- `cargo build -p pixhaus-app`
- `cargo clippy -p pixhaus-ai -p pixhaus-app --tests -- -D warnings`
- `cargo nextest run -p pixhaus-ai -p pixhaus-app`
- `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --document-private-items`
  (Stop-hook gate)
- `cd ui && npx tsc --noEmit && npx eslint src`
- Manual: studio shows no Mode/grid controls; Generate animates the anchor via i2v; frame
  picker → normalize → integrate lands a tagged animation; the standalone "Animated Sprite
  Sheet from Prompt" command is gone.

## Risks / open items

- Feeding a multi-panel reference sheet to i2v can yield poor motion. Mitigation later:
  derive a single-pose directional anchor (the cascade already supports directional
  anchors).
- Removing the grid verb also removes the only consumer of the FalSprite-derived prompt
  scaffolds; the attribution in `NOTICES.md`/`THIRD_PARTY_NOTICES.md` can be trimmed when the
  code is deleted (optional follow-up; not required for the build).
- Streams doc S53 (animated sprite sheet) and any references in `docs/planning/work/` become
  historical; update or annotate as a separate docs pass if desired.
