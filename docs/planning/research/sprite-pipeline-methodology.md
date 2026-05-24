# Sprite pipeline methodology: anchor-first generation and normalization

**Research date:** May 2026
**Status:** Reference notes — feeds verb-stream briefs (S23-S36), the project data model (B2/B9), and AI backend adapters (S22).

> Image generation is roughly 20% of the work. The other 80% is the normalization pipeline that turns raw model output into game-ready frames.

This document captures an external methodology for producing consistent character spritesheets with AI image generation and image-to-video, then mapping it to Pixhaus streams. The methodology is disciplined enough to be worth adopting almost as-is; the value sits in the post-processing rules and the workflow choreography, not in any single prompt.

## 1. Source and attribution

- **Project:** *AI Game Spritesheets* — prompt templates and reference grids for generating game-ready character spritesheets.
- **Author:** Chong-U Lim ([@chongdashu](https://x.com/chongdashu)).
- **License:** MIT, `Copyright (c) 2026 Chong-U Lim`.
- **Linked materials:** tutorial video [How I Turn AI Art Into A Playable Game Character](https://www.youtube.com/watch?v=ftWQpHyWcVQ); origin write-up [Cass Attack Spritesheet Pipeline](https://x.com/chongdashu/status/2047271308166078951); reference cheat sheet [AI Game Spritesheets Pipeline (Excalidraw)](https://aiod.dev/vgd-05-cheatsheet).
- **Pixhaus license posture:** Pixhaus is MIT. The upstream is MIT. The two are compatible without relicensing. Integration paths and the obligations each one triggers are listed in §6.

## 2. Headline framing

Three claims do most of the work in the upstream methodology, and each one survives unchanged when applied to Pixhaus.

1. **One canonical pose, everything else derives.** A single south-facing idle sprite is the anchor. Directions, idle, walk, and attack all flow from it. Re-deriving the anchor cascades to every downstream sheet.
2. **Image generation cannot do walk cycles reliably; image-to-video can.** Generate one anchor, animate it via i2v for ~4-5 seconds, then frame-pick 8-12 frames that form a clean loop. Every attempt to generate a walk sheet from scratch with image-gen alone produces inconsistent feet, drifting bodies, or off-baseline frames.
3. **Normalization is not optional.** Even with disciplined prompts, raw output drifts inside cells, varies in foreground scale between idle/walk/attack, and uses inconsistent foot baselines. The seven-step normalization is what separates a prototype-quality export from a drop-in atlas.

## 3. The eight-stage pipeline

Stage IDs map one-to-one to the upstream prompt files. Each stage names what goes in, what comes out, and the discipline that holds the stage together.

### Stage 1 — Concept / box art (`01-box-art.md`)

- **Input:** text description of the character (silhouette, palette, props, era).
- **Output:** a single high-resolution concept portrait. Painterly is fine. Detail is fine.
- **Rule:** the concept image is for human reference only. It is never passed as an i2i seed to the sprite stages. Passing concept art into sprite generation skews every downstream frame toward painterly rendering.

### Stage 2 — Canonical south anchor (`02-south-anchor.md`)

- **Input:** the text from stage 1, restated for sprite output; a `1024×1024` reference image of a black-and-white pixel grid (alternating blocks).
- **Output:** a single south-facing idle pose at `1024×1024`, framed for downstream resize to `256×256` logical.
- **Rule:** the pixel-grid reference is passed as a *visual discipline anchor*, not as content. Its job is to force chunky, readable pixel-art blocks and centred composition. The model never copies its content.
- **Why this is "the most important image you generate":** every direction and animation roots from this frame. A weak anchor compounds through ten downstream sheets.

### Stage 3 — Neutral anchor reset (`03-neutral-anchor.md`)

- **Input:** the south anchor.
- **Output:** the south anchor stripped of baked-in effects — no fireball, no glow, no charged weapon, no spell trail.
- **Rule:** directional anchors and animations are derived from the *neutral* anchor, not the "hero shot." Otherwise the effect bleeds into every direction and animation frame, and you can't add per-attack effects cleanly later.

### Stage 4 — Directional anchors (`04-directional-anchors.md`)

- **Input:** the neutral south anchor.
- **Output:** west-facing and north-facing anchors. East is **not** regenerated.
- **Rule (directional economy):** east is a horizontal flip of west. Regenerating east always drifts (different prop placement, different shading). The flip is free and consistent.

### Stage 5 — Walk cycle via image-to-video (`05-walk-cycle-i2v.md`)

- **Input:** the directional anchor (one i2v run per direction: S, W, N).
- **Output:** a 4-5 second clip, ~80-120 frames, of an in-place walk loop facing that direction.
- **Workflow:** scrub the clip. Find a "neutral stance" frame (both feet roughly together). Step forward until that pose recurs — that defines one full cycle. Pick 8-12 evenly-spaced frames between those two markers.
- **Rule:** never pass the spritesheet layout guide as a second i2v input. It blends into the output and produces visually corrupted frames.
- **Constraints baked into the prompt:** character must face the requested direction for the entire clip; no pivots, no turns, no rooms or floors materializing as backgrounds; no magic / particles / glow.

### Stage 6 — Attack spritesheet (`06-attack-spritesheet.md`)

- **Input:** the directional anchor; a `1280×512` reference image of a 5×2 grid (each cell `256×256`).
- **Output:** a 10-frame attack sheet laid out left-to-right, top row then bottom row.
- **Rule:** dynamic effects (sparks, projectile, recoil) only appear in this stage and are scripted frame-by-frame in the prompt — neutral ready → anticipation → charge spark → projectile forms → release → follow-through → settle. Effects never leak into idle or walk.

### Stage 7 — Idle spritesheet (`07-idle-spritesheet.md`)

- **Input:** the directional anchor; the 5×2 layout guide.
- **Output:** a 10-frame idle sheet with subtle motion — breathing, weight shift, equipment sway. No walking, no turning, no effects.
- **Rule (loop seam):** frame 10 must visually match frame 1 closely enough that the loop is invisible. If the seam jumps, re-roll.

### Stage 8 — Normalization (`08-normalization.md`)

The seven-step post-processing pass that produces the actual game-ready atlas.

1. **Split frames using the alpha bounding box, not the nominal grid.** AI output drifts characters inside cells; trusting the grid produces clipped feet and off-centre bodies.
2. **Remove background.** The model is asked to output `#FF00FF` (magenta) or `#00FF00` (green); both Bria / remove.bg and manual chroma key produce a clean alpha channel from there.
3. **Measure per frame:** visible width, visible height, centre X of the alpha bbox, foot baseline (the bottom Y of opaque pixels).
4. **Correct scale across sheets.** Attack frames frequently come back smaller than idle/walk. Scale them to match the idle/walk reference height before packing.
5. **Re-pad each frame to a fixed canvas** (`256×256`) with a consistent centre X and foot baseline Y.
6. **Rebuild the atlas.** Pack frames back into the original layout in the original order. Preserve atlas dimensions and frame count.
7. **Verify visually.** Generate a contact sheet (all frames side-by-side) and a GIF preview at runtime scale. No drift, no scale jumps, no missing frames — only then integrate into game code.

## 4. Cross-cutting design principles

These principles repeat across every stage and are the actual deliverable — the prompts are illustrations of the principles, not the other way around.

- **Anchor-first.** A single canonical pose drives every direction and animation. Cascading re-rolls are cheap; re-rolling each frame independently produces drift.
- **Directional economy.** Store and generate S, W, N. East is a free horizontal flip of W. Three i2v runs per character, not four.
- **Reference images as discipline, not content.** Pixel grids enforce chunkiness; sheet guides enforce layout. Neither is a content seed. Passing concept art as a content seed *breaks* the anchor.
- **Chroma-key backgrounds at generation time.** Magenta or green produces clean alpha downstream. Cheap insurance against partial transparency artefacts.
- **Negative prompting as a hard rail.** Direction-locked clips and effect-stripped neutral frames lean heavily on "do not pivot," "do not show a quarter turn," "no spells / smoke / particles." The verb runtime should adopt this pattern.
- **Loop seam and foot baseline are non-negotiable.** A spritesheet that loops cleanly but drifts vertically frame-to-frame is unusable in-game. A frame-perfect baseline lock is the line between prototype and shipping.

## 5. Mapping to Pixhaus streams

Concrete proposals against the verbs and infrastructure already scoped in `docs/planning/work/streams.md`. Each one identifies what the verb gets from the methodology and what it should adopt.

### S25 — Verb: Extend (multi-direction)

The methodology supplies a 4-direction default with built-in directional economy: generate west and north from the south anchor, derive east as a horizontal flip of west. Bake the flip-shortcut into the verb's default config — both as the cheaper i2v option and as the recommended UX. Configurable angle lists still apply (8-direction, custom), but 4-direction with E=flip(W) is the right zero-config default for top-down 2D.

### S26 — Verb: Variant

Surface the **neutral anchor reset** as a first-class variant type. The verb already covers palette swaps, equipment overlays, and expression sets — add "strip baked-in effects" as a named operation that produces a neutral anchor from a hero-shot anchor. This is the prerequisite for clean directional and animation derivation downstream, and it has no analogue in the current variant categories.

### S27 — Verb: Cleanup

This is the verb with the most to gain. The current scope ("snap to project palette, remove sub-pixel AA, fix pivot drift") aligns with steps 5 and 6 of the normalization pass. Expand the verb to cover the full seven-step pass:

1. Alpha-bbox frame split
2. Background removal (real alpha or chroma key with fringe removal)
3. Per-frame metrics (width, height, centre X, foot Y)
4. Cross-sheet scale match
5. Re-pad to a fixed canvas, locked centre + baseline
6. Atlas rebuild preserving order and dimensions
7. Contact sheet + GIF preview generation

Surface each step as an independently runnable sub-operation so artists can run "rebaseline only" or "rescale only" without redoing the whole pass. Pivot drift and palette snap remain the headline features; baseline lock and cross-sheet scale match are the new additions.

### S32 — Verb: Motion-from-video

The existing brief is broader (extract pose timing from a reference video). The methodology gives a narrower, immediately-shippable workflow:

- Drop a 4-5 second i2v output into the verb.
- Auto-detect candidate "neutral stance" frames using alpha-bbox foot proximity (a feet-together heuristic on the lower silhouette).
- Surface loop-start / loop-end markers the artist can nudge.
- Auto-extract N evenly-spaced frames between markers (default 8, configurable 8-12).

This is a useful zero-step workflow even before the broader pose-extraction work lands, and the frame-picker UI it requires is reusable for any future i2v integration.

### S35 — Verb: Tileset-from-description

Borrow the reference-grid-as-anchor trick. Pass a tile-dimension grid (e.g., a `16×16` or `32×32` checkerboard, or the project's own tile size projected as a guide) as a visual discipline reference, not a content seed. Enforces consistent tile dimensions in the output without pinning content.

### S36 — Verb: Sketch finishing

The pixel-grid reference image is the anti-painterly anchor that makes the difference between "AI sprite that looks like a render" and "AI sprite that looks like pixel art." Surface this as a default style anchor inside the verb — alongside the project's learned style (S30), the pixel-grid reference forces chunky block discipline.

### S29 — Verb: Critique

Add three checks to the critique pass, all derivable from the alpha-bbox math the normalization pipeline already implies:

- **Baseline drift:** foot-Y delta between adjacent frames exceeds a threshold.
- **Scale jump:** visible-height ratio between sheets (idle vs walk vs attack) is outside a tolerance.
- **Halo / fringe:** non-opaque pixels along the silhouette edge from sloppy chroma keying.

Each finding maps to a frame and a fix — the existing click-to-jump-to-frame UX already covers the UI.

### S23 — Verb: Inbetween

The frame-picker workflow from stage 5 generalizes. When a user runs Inbetween between two key frames, offer the option to source intermediates from an i2v clip rather than a frame-interpolation model. Same UI, different backend; produces smoother organic motion at the cost of generation time.

### S22 — Backend adapters

The methodology requires a **video-capable adapter path** for the i2v stage. None of the current adapter targets (Anthropic, OpenAI, Replicate, Ollama, ComfyUI, Stability) are explicitly framed as video — but fal.ai (SeedDance-class), ComfyUI workflows, and Replicate all support i2v. Call out the video-output adapter shape *before* the adapter trait freezes, so we don't have to retrofit it after S22 ships.

### S19 — Timeline panel

Contact-sheet generation and runtime-scale GIF preview belong in the timeline panel, not in Cleanup. The artist wants to see "all frames at once" and "the loop playing at game scale" as a verification step regardless of whether they ran a verb. Cleanup can *call* the timeline's preview features; it shouldn't own them.

### B2 / B9 — Data model and project library

Propose a `character_anchor` first-class entity in the project model:

```text
CharacterAnchor
├── canonical_pose            (image — the south hero shot from stage 2)
├── neutral_variant           (image — the effect-stripped anchor from stage 3)
├── directional_anchors
│   ├── south
│   ├── west
│   ├── north
│   └── east                  (derived: horizontal flip of west)
└── derived_sheets
    ├── walk_{s,w,n,e}        (10-frame, baseline-locked)
    ├── idle_{s,w,n,e}        (10-frame, loop-seamed)
    └── attack_{s,w,n,e}      (10-frame, scale-matched)
```

Each derived sheet carries a `derived_from` edge back to the directional anchor it was animated from, and each directional anchor carries a `derived_from` edge back to the neutral variant. Re-rolling the canonical pose cascades automatically: every dependent gets marked stale, the artist gets a "regenerate downstream" action with diff preview.

This is a B9 (project library) concern, but the underlying cels and edges live in B2.

## 6. Integration paths and license obligations

Three concrete options for landing this work, ordered by commitment level.

### Option A — Reference only

This document cites the upstream from the planning directory. Nothing is copied. No MIT-triggered obligation; polite attribution (this doc, §1) is sufficient.

- **Effort:** zero beyond this writeup.
- **What we get:** the methodology recorded in our planning corpus and mapped to streams.
- **What we don't get:** any actual prompts or reference images. The verb-stream agents have to re-derive them.

### Option B — Vendor the upstream as a sample / seed

Drop the upstream tree under `vendor/ai-game-spritesheets/` (eight prompt markdowns, reference grid PNGs, the original `README.md` and `LICENSE`). Add a top-level `THIRD_PARTY_LICENSES.md` listing the dependency.

- **Effort:** one PR to vendor, one to wire the entry.
- **MIT obligation triggered:** preserve `LICENSE` and copyright. Satisfied by keeping the file alongside the vendored material and listing it in `THIRD_PARTY_LICENSES.md`.
- **What we get:** verb-stream agents have a known-good reference to iterate against. The contact-sheet GIFs become useful regression fixtures.

### Option C — Adopt as built-in verb prompts

Move the prompt templates (modified) into `ai/src/verbs/<verb>/prompts.md` as the starting body of the relevant verbs. Each derivative file carries a header comment naming the upstream commit and author. `THIRD_PARTY_LICENSES.md` carries the copyright + permission notice.

- **Effort:** lands inside the verb streams (S25, S26, S27, S32, S36) when each is dispatched.
- **MIT obligation triggered:** preserve copyright and permission notice in `THIRD_PARTY_LICENSES.md`, attribute in each derived file's header. Modification is fine.
- **What we get:** working verb prompts on day one of each verb stream, instead of re-deriving them.

### Recommendation

Start with **A** in this PR (nothing copied; this document exists). Plan for **C** when each verb stream is dispatched. **B** is optional middle ground if the verb agents want a known-good test fixture corpus before any verb is built.

## 7. Open questions

Decisions deferred to the relevant stream's design phase, not to this document:

- **Anchor cascading: data-model edges or UI-driven re-runs? → Resolved: data-model edges.** The `CharacterAnchor` embedded on `ReferenceSheet` carries `derived_from` edges (`SheetVariant::parent_variant_id` for canonical→neutral→directional, `DerivedSheet::derived_from` for directional→sheet). Re-rolling the canonical marks dependents stale structurally (`CharacterAnchor::is_sheet_stale`), surfaced by `animation_set` / `animation_reroll_dependents`.
- **Cleanup auto-apply vs preview-then-confirm.** The normalization pass produces large changes (re-pad every frame, atlas rebuild). The verb plugin protocol's preview-then-commit flow (B5) makes a confirm step natural, but the artist may not want to confirm every cleanup step in isolation. Probably: preview the *combined* result, allow per-step re-runs from the preview.
- **i2v source video persistence.** Does the project file store the 4-5 second source clip, or just the 8-12 picked frames? The clip is useful for re-picking with different markers; it's also a meaningful storage cost. Likely an optional persistence flag per character anchor.
- **Where do the prompt templates live under option C?** Inside `ai/src/verbs/<verb>/` as built-in resources, or as a Lua/WASM plugin pack the user can swap out? Built-in is simpler; plugin-pack lets the community ship alternative prompt sets.

## 8. References

- Upstream repository (`AI Game Spritesheets` by Chong-U Lim, MIT) — see §1 for links.
- `docs/planning/work/streams.md` — verb stream definitions (S23-S36, S19, S22).
- `docs/planning/work/bedrock.md` — verb plugin protocol (B5), core data model (B2), project library (B9).
- `docs/planning/research/project-library-research.md` — adjacent research on multi-asset organization, the natural home for the B9 entity sketch in §5.
