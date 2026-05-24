# Animation generation pipeline

Branch: `docs/animation-generation-pipeline` (spec only — no code lands with this doc).

This spec defines how Pixhaus turns an approved **anchor reference sheet** into
generated animations. It is a cross-stream implementation plan, not a new bedrock
id: it threads existing AI-verb streams and the B10 anchor mechanic into one
ordered build. The methodology is taken from two research dossiers, and the parts
lifted from prior art are attributed:

- `docs/planning/research/sprite-pipeline-methodology.md` — the eight-stage,
  anchor-first pipeline this spec implements.
- `docs/planning/research/falsprite-prior-art.md` — the CHARACTER x CHOREOGRAPHY
  two-prompt split and row-major grid math, adapted from FalSprite (MIT, lovisdotio).
  The existing `AnimatedSpriteSheetVerb` already ships this; we extend it, with the
  attribution headers it already carries.

Streams advanced: S23 (inbetween), S25 (multi-direction), S27 (cleanup /
normalization), S32 (motion-from-video). Anchor mechanic: B10.

## Why now

B10 gave us the anchor: a `ReferenceSheet` with a canonical `SheetVariant` that
every downstream generation conditions on for character consistency
(`core/src/project/library/reference_sheets.rs`, `ai/src/plugin/anchor.rs`). The
anchor is the input the animation pipeline was waiting on. With it in place, the
next step is the cascade from one canonical pose to a full set of looping
animations.

## The mental model

One canonical pose, everything else derives. The methodology's first claim is the
whole design:

> A single south-facing idle sprite is the anchor. Directions, idle, walk, and
> attack all flow from it. Re-deriving the anchor cascades to every downstream
> sheet.

Two rules carry through every stage:

- **Reference images are discipline anchors, not content seeds.** A pixel grid or a
  layout guide is passed to force chunky blocks or fixed cell placement — never as
  content to copy. The anchor sheet itself is the content reference; the grids are
  discipline. Passing concept art as a content seed breaks the anchor.
- **Loop seam and foot baseline are non-negotiable.** A sheet that loops but drifts
  vertically is unusable in-game. Frame 10 must match frame 1; the foot baseline
  must hold across every frame. This is what normalization enforces.

## Current state versus target

| Area | Current state | Verdict |
|---|---|---|
| Grid sheet generation | `AnimatedSpriteSheetVerb` generates one g×g image and slices it row-major (`slice_grid()`), with the CHARACTER x CHOREOGRAPHY two-prompt split, palette snapping, and `VerbEffect::AddLayer`. | Keep and extend. |
| Anchor conditioning | The verb falls back to `anchor_style_image_bytes(ctx)` for a style image, but is not anchor-first: no directional target, no neutral reset, no cascade. | Extend. |
| Walk cycles | None. The methodology requires image-to-video; Pixhaus has no i2v capability, backend, or video decode. | Build (phase 2). |
| Directional anchors | None. | Build (phase 3). |
| Normalization | Partial: alpha-bbox detection, atlas packing, scaling, palette extraction exist. Background removal and fixed-canvas re-padding do not. | Assemble + fill gaps. |
| Animation output | The verb emits a layer with frames only — no `FrameTag` or `Animation`. | Extend to emit a real animation. |

### Reuse inventory (exists today)

- Anchor payload and helper: `AnchorPayload` (`ai/src/plugin/anchor.rs`),
  `anchor_style_image_bytes()` (`ai/src/verbs/mod.rs`).
- Image-gen request carries references: `ImageGenRequest.style_image: Option<Vec<u8>>`
  and `reference_images: Vec<Vec<u8>>` (`ai/src/backends/mod.rs`).
- Grid slicing and prompt scaffold: `ai/src/verbs/animated_sprite_sheet/{grid.rs,prompts.rs}`.
- Alpha bounding box: `smart_slice::detect_frames()` (`core/src/import/smart_slice.rs`)
  and `compute_trim()` (`io/src/png/mod.rs`).
- Atlas packing: `pack_frames()` with grid and skyline strategies (`io/src/png/pack.rs`).
- Scaling: `scale_nearest` / `scale_integer` / `scale_integer_down`
  (`core/src/transforms/scale.rs`).
- Palette extraction: `extract_palette_from_image_bytes()` (`core/src/color/extraction.rs`).
- Motion analysis: `motion_from_video` does pixel-diff keyframe detection on a
  `VideoFrame` sequence (`ai/src/verbs/motion_from_video/mod.rs`).
- Frame model: `Sprite.{frames,cels,frame_tags,animations}`, `LoopDirection`,
  `FrameTag`, `Animation` (`core/src/project/{frame.rs,animation.rs,sprite.rs}`).
- Animated preview/export: GIF, WebP, MP4 (export only) in `io/src/animated/`.

### Gaps to build (net-new)

- `IMAGE_TO_VIDEO` capability bit (next free bit is `1 << 13` in
  `ai/src/plugin/descriptor.rs`; flags currently end at `VIEW_SYNTHESIS = 1 << 12`).
- An `ImageToVideoRequest` and a backend that serves it (Replicate first — it
  already declares `FRAME_INTERPOLATION`).
- Video decode: clip bytes to an RGBA `VideoFrame` sequence.
- Chroma-key background removal (magenta `#FF00FF` or green `#00FF00` to alpha).
- Fixed-canvas re-padding with a chosen centre-x and foot-baseline-y.

## Pipeline stages mapped to Pixhaus

Each methodology stage maps to one ordered Pixhaus work item. Stages 1-3 are upstream
of this spec (concept and anchor production live in B10); animation starts at the
directional anchors.

1. **Neutral anchor reset** — derive an effect-stripped variant from the B10
   canonical `SheetVariant`. Animations root from the neutral anchor, not the hero
   shot, so per-attack effects can be added cleanly later. Surfaces as a named
   "strip baked-in effects" operation (advances S26).
2. **Directional anchors** — generate south, west, north from the neutral anchor.
   East is the horizontal flip of west, not a regeneration — regenerating east
   drifts; the flip is free and consistent. Reuse the existing flip transform.
3. **Idle and attack grid sheets** — extend `AnimatedSpriteSheetVerb` to condition
   on the directional anchor as the content reference plus a layout-guide reference
   for cell placement. Idle carries a loop-seam constraint; attack scripts effects
   frame-by-frame in the prompt (anticipation, charge, release, follow-through,
   settle) and confines them to the attack sheet only.
4. **Walk cycles via image-to-video** — image generation cannot do walk cycles
   reliably; i2v can. Generate one anchor, animate it to a short clip, frame-pick a
   clean loop. This is the phase-2 infrastructure track below.
5. **Normalization** — a reusable post-pass that locks baseline and scale and
   rebuilds the atlas (phase-1 deliverable, detailed below).

## i2v infrastructure plan (phase 2)

Walk cycles are the reason to add video. The methodology is explicit: every attempt
to generate a walk sheet from image generation alone produces inconsistent feet and
drifting bodies. The build:

- **Capability.** Add `IMAGE_TO_VIDEO: Self = Self(1 << 13)` to
  `BackendCapabilities` and to the human-readable flag table
  (`ai/src/plugin/descriptor.rs`).
- **Request and result.** Add an `ImageToVideoRequest` (source image bytes, prompt,
  negative prompt, target duration / frame count, seed) and a clip result type in
  `ai/src/backends/mod.rs`. Implement on Replicate first (it already declares
  `FRAME_INTERPOLATION` and reaches marketplace models); FAL is the alternative.
- **Prompt rails.** The i2v prompt bakes in the methodology's hard negatives:
  character faces the requested direction for the whole clip, no pivots or quarter
  turns, no backgrounds materializing, no particles or glow. The layout guide is
  never passed as a second i2v input — it blends into the output and corrupts frames.
- **Decode.** Decode the returned clip to an RGBA `VideoFrame` sequence. Decision
  flagged below: `io/src/animated/mp4.rs` already shells out to `ffmpeg` for export,
  so reusing ffmpeg-on-PATH for decode is the low-dependency path; a pure-Rust
  decode crate is the alternative but must clear the MIT-only license rule.
- **Frame pick.** Reuse and extend `motion_from_video`: detect the neutral-stance
  pose, find where it recurs to bound one cycle, and pick 8-12 evenly spaced frames
  between the markers. This advances S32, which already targets motion timing from
  video.

## Extending AnimatedSpriteSheetVerb

Keep the verb's identity and back-compat with current inputs; add anchor-first
behaviour as optional fields so existing callers are unaffected.

- **Anchor wiring.** Accept the reference entity so the verb pulls the canonical
  anchor via the existing `AnchorPayload` path rather than relying on an explicit
  `style_reference`. Explicit `style_reference` still wins when supplied.
- **Direction.** Add a directional target (south / west / north / east) so the verb
  conditions on the matching directional anchor and labels the output.
- **Layout guide.** Pass a generated grid guide as a discipline reference for cell
  placement (not content).
- **Mode.** Add an i2v walk mode that routes through the phase-2 infrastructure
  instead of single-image grid generation.
- **Real animation output.** Emit a `FrameTag` (and engine-side `Animation`) with the
  right `LoopDirection` — `PingPong` for idle, `Forward` for walk — so the result is
  a playable animation, not a stack of frames. Today the verb only adds a layer with
  frames.

## Normalization pass (phase 1)

Sequence the methodology's seven steps onto existing utilities plus the two gaps.
This is a reusable pass applied to any generated sheet, not verb-specific.

1. Split frames by the alpha bounding box, not the nominal grid — reuse
   `smart_slice::detect_frames()`. Trusting the grid clips feet and off-centres bodies.
2. Remove background — new chroma-key util (magenta or green to alpha).
3. Measure per frame: visible width, visible height, centre-x of the alpha bbox,
   foot baseline (bottom y of opaque pixels) — reuse `compute_trim()` extents.
4. Correct scale across sheets — attack frames often come back smaller than idle and
   walk; scale to the idle/walk reference height with `transforms/scale`.
5. Re-pad each frame to a fixed canvas with a consistent centre-x and foot-baseline-y
   — new padding util.
6. Rebuild the atlas in the original order and dimensions — reuse `png/pack`.
7. Verify — emit a contact sheet and a GIF preview at runtime scale via
   `io/src/animated`; no drift, no scale jumps, no missing frames.

## Phasing

**Phase 1 — anchor-first grid sheets and normalization.** Mostly reuse. Extend
`AnimatedSpriteSheetVerb` for anchor conditioning and real animation output; build
the normalization pass (chroma-key and padding are the only net-new pieces).
Advances S25, S27. Acceptance: an idle and an attack sheet generated from an anchor,
normalized, looping cleanly with a locked baseline, landing as a tagged animation.

**Phase 2 — i2v and walk cycles.** Net-new infrastructure. Capability, request,
Replicate implementation, decode, frame pick. Advances S32. Acceptance: a
direction-locked walk clip frame-picked into an 8-12 frame `Forward` loop, normalized
against the idle/walk reference height.

**Phase 3 — full directional cascade.** Neutral reset, directional anchors with
east-as-flip, and staleness edges so re-rolling the canonical pose marks dependents
stale. Advances S25, S26. Acceptance: south/west/north/east idle, walk, and attack
derived from one neutral anchor, with re-roll cascading.

## Resolved decisions

- **Video decode dependency → ffmpeg-on-PATH.** Decode reuses the ffmpeg shell-out
  already used for MP4 export (`io/src/animated/decode.rs`), sidestepping the MIT-only
  license constraint at the cost of a runtime prerequisite.
- **Where directional anchors live → `CharacterAnchor`.** Stage 1's neutral anchor and
  the directional/derived layers live in a `CharacterAnchor` embedded on
  `ReferenceSheet` (`core/src/project/library/reference_sheets.rs`): `neutral`,
  `directional` (south/west/north + east-as-flip), and `derived_sheets`. Staleness is
  structural — `SheetVariant::parent_variant_id` and `DerivedSheet::derived_from` form
  the `derived_from` edges, so re-rolling the canonical cascades staleness down the
  chain (`CharacterAnchor::is_sheet_stale`). The neutral reset is the
  `StripBakedEffects` mode of the Variant verb (S26), stored via
  `animation_derive_neutral`; animations auto-prefer the neutral/directional anchor via
  `AnchorKind` on `verb_invoke`.
- **i2v cost and latency.** Video runs report a higher `CostEstimate`
  (`ai/src/backends/replicate.rs`) and the studio surfaces elapsed time / spend on the
  i2v request row.

## Cross-references

- Methodology: `docs/planning/research/sprite-pipeline-methodology.md`.
- Prior art and attribution: `docs/planning/research/falsprite-prior-art.md`.
- Anchor mechanic: `docs/planning/work/bedrock.md` (B10), `docs/planning/work/b10-reference-sheets.md`.
- Streams: `docs/planning/work/streams.md` (S23, S25, S26, S27, S32).
- Existing verb: `ai/src/verbs/animated_sprite_sheet/{mod.rs,grid.rs,prompts.rs}`,
  tests at `ai/tests/animated_sprite_sheet_lifecycle.rs`.
- Backends and capabilities: `ai/src/plugin/descriptor.rs`, `ai/src/backends/{mod.rs,replicate.rs}`.
