# Sprite generation upgrades: learnings from agent-sprite-forge

agent-sprite-forge is a Codex skill that produces game-ready sprite sheets and animations by enforcing one rule above all others: the creative model paints raster art on a known chroma-key background, and a deterministic processor does every geometry decision. That separation is what lets it ship clean, atlas-ready sprites instead of soft AI renders — magenta keying, connected-component isolation, edge-touch QC, shared-scale normalization, hard-masked GIF export, and an asset-plan layer that picks the right sheet shape before a single pixel is drawn. Pixhaus v2 already has the harder half built: a real verb runtime, a deterministic prompt composer, a rich reference-sheet/anchor data model, a mature animation studio, and a `normalize_frames` pass that does chroma-key + scale-match + baseline-lock + drift/seam QC.

Read the rest of this doc through two reframings the project owner just locked in.

First, **the pipeline is style-parametric, not a pixel-art enforcement machine**. Pixel art is the *default* style — because the mascot Bit is pixel art and most users arrive wanting pixel art — but it is one style among several first-class ones. Clean-HD, retro-pixel, pixel-inspired, and map-style ship alongside it. Pixel-only steps (the palette-snap/downscale finisher, the pixel-art prompt adjectives) are *gated* behind the selected style; the universal disciplines (containment, no-border, identity/scale-lock) apply to every style. The selected art style drives both gates.

Second, **i2v is one of two animation paths, not the only one**. The FAL image-to-video model is now optional. A static, image-only path — generate one grid sheet, slice the cells, normalize the frames, land them as an Animation — is first-class. It reuses almost all of the existing clip plumbing because the i2v-specific code is a single function (`generate_clip`, `shell/src/ai.rs:868`) and everything downstream of it consumes a plain `Vec<VideoFrame>` with no knowledge of the source. The user-video import path already proves this seam works.

The single highest-leverage win is still **GUI sprite export with a hard-masked looping GIF and packed atlas** — v2 can author sprites end to end but cannot deliver one from the UI, which is the difference between an editor that makes sprites and one that ships them. The two reframings do not displace that win; they widen it. Export closes the author→ship loop for everyone. The static path widens *who* can produce an animation at all (no video model, no video API cost). Style selection is the small enabler the other quality work hangs off — it should land early.

## How agent-sprite-forge works

The pipeline in one breath: the agent infers a minimal asset plan (type, action, view, sheet shape, bundle), hand-writes a containment-heavy prompt forcing a solid `#FF00FF` magenta background and an exact grid with identical bounding box and scale per cell, calls the built-in `image_gen` for the raw sheet, then runs a deterministic Python/Pillow processor that chroma-keys the magenta by Euclidean distance, trims a border, flood-fills edge bleed, splits the grid, runs connected-component analysis to keep the largest body or all detached FX, re-centers and anchors frames at a single shared scale, and emits per-frame PNGs plus a re-composited transparent sheet, a hard-masked looping GIF, and a `pipeline-meta.json` carrying per-frame QC (edge-touch flags, component areas). The agent visually QCs the result and either re-runs the processor with different primitives or regenerates — reprocess before regenerate, because deterministic cleanup is cheap and a new generation is not.

The techniques, grouped by category and with the why behind each guardrail:

**Asset-plan** — Decide the sheet shape before generating. `smallest-useful-output`: infer the minimal plan from natural language so you never generate a 16-frame atlas when a 2x2 idle suffices, which shrinks the failure surface and cost. `multirow-grid-over-singlerow`: never generate a raw 1xN body strip (map 4→2x2, 6→2x3, 9→3x3, 16→4x4) because a single row gives the model no vertical anchor and drifts horizontally. `prop-pack-classification`: classify compact vs wide vs tall before picking a grid, because a wide vehicle clipped into a square cell is a guaranteed bad crop. `body-only-fx-split` and `no-mixed-action-raw-atlas`: keep hero bodies on body-only sheets and assemble multi-action atlases deterministically from individually-QC'd grids, because a wide FX bbox silently shrinks the body and unrelated action rows share no scale or anchor.

**Prompt** — `containment-prompt-rules`: restate that the entire subject fits inside each cell with equal margin on all four sides and nothing crosses an edge, because models cheat boundaries and the splitter cannot recover bleed. `identity-scale-lock`: require the same identity, same bbox, same pixel scale per cell, because frame-to-frame scale changes defeat shared-scale post-processing. `no-text-no-ui-rule`: forbid text, labels, and drawn cell borders, because stray marks contaminate sprites and break the "cells connected only by background" assumption. `style-selection-policy` and `stable-seed`: pick the art style up front and derive a reproducible seed so regeneration is deterministic.

**Containment / contracts** — `chroma-key-magenta-bg`: a 100% solid `#FF00FF` background is the single contract that decouples the creative model from the deterministic processor; a known far-from-sprite key makes removal reliable where gradients would leave halos. `imagegen-only-source`: raw art must come from the model, never code-drawn placeholders, so quality is auditable. `low-level-processor-philosophy`: the script makes no aesthetic decisions; the agent chooses every primitive flag, so the same flags reproduce the same geometry.

**Postprocess** — `magenta-distance-removal`: two-pass removal — a tight interior Euclidean threshold, then a border-seeded flood-fill at a looser distance — kills the bulk background and the connected AA fringe without eating interior pixels of a similar hue. `edge-clean-depth` and `trim-border`: a thin edge band and a fixed border trim remove the dark/halo rim and cell-seam bleed. `connected-components` + `component-mode-largest-vs-all`: label the alpha mask so a stray speck cannot inflate the bbox; keep the largest body or all detached FX. `shared-scale` + `fit-scale-padding` + `align-anchor`: one common scale from the largest frame plus an 85% safe area plus feet/center anchoring keeps an animation from visibly growing, shrinking, or bobbing. `lanczos-resampling` and `single-sprite-center` round out the normalization.

**QC** — `edge-touch-qc`: flag any frame whose bbox reaches the cell edge and optionally hard-reject, because a clipped subject ships broken otherwise. `body-shrink-qc`: reject a body action more than ~10–15% smaller than idle/run even when no edge touches, the defect edge-touch alone misses. `pipeline-meta-json` + `reprocess-then-regenerate-loop`: a machine-readable per-frame record makes runs reproducible and gates the cheap-reprocess-before-expensive-regenerate decision.

**Export / bundle** — `transparent-gif-export`: hard-mask alpha at ≥128 and reserve one palette index as transparent, because GIF has 1-bit alpha and a naive encode leaves a soft-fringe halo. `recompose-transparent-sheet`: paste processed frames back into a uniform grid for a clean atlas. `frame-label-contract`, `direction-strip-and-gif-split`, and `output-bundle-shape`: deterministic semantic filenames, per-direction strips/GIFs, and a predictable artifact set make downstream import mechanical.

**Reference / layout-guide** — `reference-visible-first`: make the reference actually visible to the model and state what stays fixed vs may change, because a path string does not condition generation. `layout-guide-geometry` and the non-repro/selectivity contracts: an invisible numbered-cell guide pins slot count and spacing for hard grids, used selectively so it doesn't over-constrain expressive poses.

## Where v2 stands today

v2 is native Rust (eframe + egui + wgpu, no Tauri/TS, no Python), and the AI pipeline is real, not stubbed.

**Verb runtime** — Mature. `ai/src/plugin/runtime/{mod,registry,invocation}.rs` does priority-ordered capability-matched backend selection, streamed progress, cancellation, and preview/commit. But exactly one generative verb is registered: `generate_reference_sheet` (`shell/src/ai.rs:176` `build_runtime`). The `auto_tag` verb exists as a backend-gated stub and is not registered.

**Compose** — Deterministic and well-tested. `ai/src/compose/{mod,builtins,variables}.rs` folds a project `style_notes` baseline, a picked Style's modifiers and look-negatives, structure layout prose, a templated prompt with `{var}` substitution, then the subject; negatives merge. Five built-in structures (single/character/item/tileset/custom) carry hardcoded panel geometry and prose. The invariant hooks `operation_hint` and `context_fragments` exist and are tested (`compose/mod.rs:52-56`, `:162-169`), but the derivation paths bypass them. One important detail for art-style work: pixel art is hardcoded in two places, not driven by the picked Style — `BUILTIN_DEFAULT_BASELINE = "pixel art reference sheet"` (`builtins.rs:17`) and pixel-art adjectives baked into structure prose (`builtins.rs:132`, `:230`).

**Backends** — Only FAL (`ai/src/backends/fal.rs`, IMAGE_GENERATION/INPAINT/IMAGE_TO_VIDEO/BACKGROUND_REMOVAL) and OpenAI (`openai.rs`, IMAGE_GENERATION/INPAINT, gpt-image-2) are wired; keys come from the OS keychain. Anthropic/Replicate/Stability/ComfyUI/Ollama/Google are enumerated but dropped (`backends/mod.rs:33`). No text or vision backend ships, so `auto_tag` and any vision QC cannot run. gpt-image-2 clamps every dimension to [1024,2048] rounded to /16 (`openai.rs:510-529`), capping aspect ratio at 2:1; FAL Flux passes `image_size {width,height}` verbatim with no aspect clamp (`fal.rs:462-480`) — which matters for wide grid sheets.

**Data model** — Rich and serializable. `core/src/project/library/reference_sheets.rs` holds `ReferenceSheet`, `SheetVariant` (composed prompt, references, provenance, origin, refinement, chat transcript, `extracted_palette`), `CharacterAnchor`/`DirectionalAnchors` with staleness predicates, and `SheetComposition` panel `Rect`s (`:580`). `AnimationKind` is `Idle`/`Walk`/`Attack` (`:270-277`); the character structure is a single-row turnaround. Approval (`core/src/project/approval.rs`) promotes a variant to canonical and extracts a palette. `Style` (`composition/style.rs:14`) carries id/name/modifiers/look_negatives/model_pref/quality — and no art-style discriminant.

**Animation studio** — The strongest area. The 8-stage wizard (`shell/src/studio.rs`) derives neutral/directional anchors (`shell/src/anim_set.rs`, east = horizontal flip of west), generates a first frame, runs FAL i2v, decodes the clip (`shell/src/anim.rs`: GIF/APNG via `image`, MP4/H.264 via `mp4`+`openh264`; no WebM/VP9/AV1/HEVC), auto-detects a loop, picks evenly-spaced frames, strips background (chroma key first, FAL Bria fallback, `shell/src/bg_removal.rs`), normalizes (`core/src/transforms/normalize.rs` `normalize_frames` — chroma, scale-to-reference-height, foot-baseline repad, drift/scale/seam report), and lands frames as a layer + tag + Animation. A durable i2v job queue (`shell/src/anim_jobs.rs`) survives restart. The i2v seam is narrow: `generate_clip` (`ai.rs:868`) is the only function that issues an image-to-video request and decodes a clip; downstream, `push_clip_candidate` (`app.rs:2697`) and `compute_normalize`/`integrate_picked` consume a plain `Vec<VideoFrame>`. The user-video import path (`import_video_clip`/`on_video_imported`, `app.rs:2736-2795`) feeds non-i2v frames into the identical Clip→Pick→Normalize→Land pipeline.

**Demo / boot** — The app boots one empty sprite; the File menu offers only New-sprite/Settings/Quit (`app.rs:1034`, `:3099-3113`). There is no demo project, no `io` crate, no `.pixhaus` save/load (`Cargo.toml:6`; `shell/src/document.rs:9-13`). The mascot exists in built-in *prompts* as Bit (a CRT-head robot), Byte, and Floppy (`builtins.rs:305-394`) — but there is no demo project that uses any of them.

**Known absences** — No paneled-sheet slicing (panel rects stored, never cropped). No grid-sheet slicer (`core` has the public `transforms::crop` at `resize.rs:121`, re-exported at `transforms/mod.rs:51`, but no `slice_grid`/`slice_rects`). No GUI export (only the headless CLI `write_outputs` writes per-frame PNGs + a looping GIF). No connected-component analysis anywhere. No edge-touch QC. No palette-snap/pixel-grid finisher on generated frames. No art-style discriminant. No static (i2v-optional) animation path. No Bit demo project.

## What to port, and what to leave behind

v2's stack is locked: native Rust, egui + wgpu, FAL + OpenAI backends, MessagePack + zstd project format (when the `io` crate lands). The forge's value is in its **ideas**, not its mechanics. Drop the Python: Pillow, NumPy, the `image_gen` built-in, `make_layout_guide.py`, the CLI flag surface, and the chroma-key constants baked into Python constants.

Reframe the forge's two strongest assumptions before porting them:

- **Pixel art is the default, not the law.** The forge bakes pixel art into every prompt. v2 makes the pipeline style-parametric: a serde-default `ArtStyleKind` on `Style` selects pixel-art (default), clean-HD, or another shipped style. Pixel-only steps gate on that kind; universal disciplines do not. This is a small enabler that unlocks the rest — land it early.
- **The `#FF00FF` magenta contract is per-style, not global.** v2 keys per-variant via `SheetVariant.chroma_color` and i2v clips arrive on whatever background the model produced, so v2 detects the key color rather than dictating it. The *static* sheet path is the one place v2 leans into the forge contract: it asks the model for a solid magenta grid so the slice→normalize tail keys cleanly.
- **i2v is optional.** The forge has no video model at all — it slices a static sheet. v2 keeps i2v as the high-motion path and adds the forge's static-sheet path as the no-video-model alternative, converging both on the same Clip→Pick→Normalize→Land tail.

Port the **concepts** to core Rust:
- Post-processing math — two-pass Euclidean chroma key, connected-component labeling, edge-band cleanup, shared-scale + safe-area normalization, hard-alpha-mask GIF encoding, atlas packing, grid slicing — all pure `PixelBuffer` work that belongs in `core/src/transforms`.
- Style-gated pixel finisher and style-gated pixel prose — the finisher in `core/src/transforms/finisher.rs`, the prose in `ai/src/compose/builtins.rs`, both gated on `ArtStyleKind`.
- Containment / no-border / identity-scale-lock prompt clauses — universal prose in `ai/src/compose/builtins.rs`, applied to every style.
- Asset-plan inference — frame-count→grid mapping and prop classification in `core` + `ai`, surfaced as an advisory hint in the cockpit.
- The static generate-sheet → slice → normalize → land path that makes the FAL video model optional.
- Bundle structure and QC loop — a predictable export set and a persisted per-frame QC record that gates reprocess-vs-regenerate.
- A showcase: a Bit demo project, built in code (no `io` crate yet), with a prompt pack covering every movement and action, composed in the pixel-art default style.

**Debunked gaps — do not relitigate.** v2 already covers more than a surface read suggests, so the following are NOT recommendations: the studio's clip-review player, key-color eyedrop, and keyed preview are built (`studio.rs:1794+`), not "proposed"; the GPU generation reveal effect is built (`shell/src/reveal.rs`), not "not started"; conversational/inpaint refinement of the anchor and first frame is wired through IMAGE_INPAINT with a brush + box gizmo; project records ARE sent to the verb; and the normalize pass genuinely computes drift/scale/seam and surfaces it in the studio inspector. The real gaps are the style/postprocess/QC/export/asset-plan/static-path items below.

## Gap analysis

| Capability | Forge technique | v2 status | Target crate/files | Payoff | Effort | Priority |
|---|---|---|---|---|---|---|
| Art-style selection: pixel-art default, multi-style first-class | style-selection-policy | missing (pixel art hardcoded in baseline + prose; no `ArtStyleKind`) | `core/.../composition/style.rs`, `ai/src/compose/{builtins,mod}.rs`, `ai/src/verbs/reference_sheet/mod.rs`, `shell/src/ai.rs` | Unlocks clean-HD/other styles; gates the finisher; small enabler | S | 9 |
| GUI export: transparent PNG + hard-masked GIF + atlas | transparent-gif-export, recompose-transparent-sheet, output-bundle-shape | partial (headless only, no hard mask, no atlas) | `core/src/export/`, `shell/src/export.rs`, `headless.rs` | Ships sprites from the UI; kills GIF halo | M | 8 |
| Static grid-sheet animation (i2v-optional) | chroma-key-magenta-bg + grid slice + shared-scale | missing (only i2v produces frames; no grid slicer) | `core/src/transforms/sheet.rs`, `ai/src/compose/builtins.rs`, `shell/src/{ai,app,studio}.rs` | Makes FAL video model optional; widens who can animate | M | 8 |
| Slice paneled sheet → directional frames | recompose (inverse), frame-label-contract, direction-strip | missing (rects stored, never cropped) | `core/src/transforms/normalize.rs`, `shell/src/anim_set.rs`, `cockpit.rs`, `studio.rs` | Consistent directional sprites from one generation | L | 8 |
| Two-pass Euclidean chroma key + edge-band + trim | magenta-distance-removal, edge-clean-depth, trim-border | partial (per-channel abs_diff ≤16, single pass) | `core/src/transforms/normalize.rs`, `shell/src/bg_removal.rs` | Removes magenta/AA halo on dark backgrounds | M | 7 |
| Connected-component isolation (largest vs all) | connected-components, component-mode-largest-vs-all | missing (bbox over whole alpha) | `core/src/transforms/normalize.rs`, `shell/src/app.rs`, `studio.rs` | Stray speck no longer shrinks the body | M | 7 |
| Edge-touch / safe-margin QC + scale-parity delta | edge-touch-qc, body-shrink-qc | missing (silent clip, pre-scale spread only) | `core/src/transforms/normalize.rs`, `shell/src/studio.rs`, `app.rs`, `ai.rs` | Flags clipped/shrunken frames before Land | M | 7 |
| Containment / no-border / scale-lock prose (universal) | containment-prompt-rules, identity-scale-lock, no-text-no-ui-rule | partial (no containment, no no-border) | `ai/src/compose/builtins.rs`, `mod.rs`, `reference_sheet/mod.rs` | Makes deterministic slicing safe; lifts every style's quality | S | 7 |
| Style-gated palette-snap + downscale finisher | single-sprite-center spirit + LANCZOS (as nearest finisher) | missing (primitives exist, never composed/run on gen) | `core/src/transforms/finisher.rs`, `shell/src/{ai,app,cockpit}.rs` | "Make it pixel art" step — for pixel-class styles only | M | 7 |
| Explicit fixed/variable invariant clauses | reference-visible-first, identity-scale-lock | partial (only numeric IP-Adapter strength) | `ai/src/compose/invariants.rs`, `reference_sheet/mod.rs`, `shell/src/{ai,anim_set}.rs` | Less identity/scale drift across derivations | S | 7 |
| Per-frame QC + provenance record on landed loops | pipeline-meta-json, reprocess-then-regenerate-loop | partial (computed then discarded; no components) | `core/src/transforms/normalize.rs`, `core/src/project/qc.rs`, `shell/src/{anim_jobs,commands,app,studio}.rs` | Reproducible runs; gates reprocess-vs-regenerate | M | 6 |
| Multi-row grids + prop classification | multirow-grid-over-singlerow, prop-pack-classification, smallest-useful-output | partial (character is a raw 1x5 strip) | `core/.../composition/structure.rs`, `ai/src/compose/builtins.rs`, `shell/src/{cockpit,ai}.rs` | Stops horizontal drift; fits subject to sheet | M | 5 |
| Bit demo project + prompt pack (all movements/actions) | smallest-useful-output (showcase) | missing (boots empty; mascot Bit in prompts, no demo) | `shell/src/demo.rs`, `shell/src/app.rs`, `ai/src/compose/builtins.rs` | First-run showcase of generate + style + export + animate | M | 4 |
| Shared fit-scale safe-area padding | shared-scale, fit-scale-padding, align-anchor | partial (fills cell edge-to-edge, no fraction) | `core/src/transforms/normalize.rs` | Breathing room; lets edge-touch QC pass | S | (folds into edge-touch QC) |
| Single-sprite crop-and-center normalization | single-sprite-center | partial (multi-frame only; single lands raw) | `core/src/transforms/normalize.rs`, `shell/src/cockpit.rs` | Consistent centered single assets | S | 5 |

## Prioritized roadmap

**Wave 1 — quick wins and the style enabler (S, land first).**
- Art-style selection: pixel-art default, multi-style first-class — small `ArtStyleKind` enum + built-in styles + a kind-driven baseline. Lands early because the finisher and the pixel prose gate on it.
- Containment + no-border + scale-lock prose into the paneled built-in structures — pure prose edit, universal across styles, makes every slicing/QC/static-path stream safer.
- Explicit fixed/variable invariant clauses on anchor-conditioned derivations — small prompt-assembly change, cuts identity drift across the cascade.

**Wave 2 — core quality (M, the deterministic post-pass spine).**
- Two-pass Euclidean chroma key — removes the halo every keyed sprite currently shows on dark backgrounds.
- Connected-component isolation — the primitive the next two streams reuse; land before edge-touch QC.
- Edge-touch / safe-margin QC + scale-parity delta — turns the Normalize review from advisory into a gate.
- Style-gated palette-snap + downscale finisher — the "make it pixel art" step for pixel-class styles; a no-op for clean-HD.
- GUI sprite export (PNG + hard-masked GIF + atlas) — closes the author→ship loop; the single highest-leverage win.
- Static grid-sheet animation (i2v-optional) — generate-sheet → slice-grid → normalize → land; makes the FAL video model optional. Core-quality; reuses the grid slicer and the chroma/normalize spine.

**Wave 3 — advanced and showcase (M–L, build on the spine).**
- Per-frame QC + provenance record on landed loops — reuses the component/edge-touch primitives from Wave 2.
- Multi-row grids + prop classification — re-lays the character body sheet and adds the advisory classifier; shares the grid engine with the static path.
- Slice paneled sheet → directional frames — the largest multi-view quality win; depends on clean containment prose and the slicing math.
- Bit demo project + prompt pack — the first-run showcase. Depends on generate + style + export work landing first, since it exercises all three.

## Stream briefs

### Brief 1 — Art-style selection: pixel-art default, multi-style first-class

**Summary.** v2 hardcodes pixel art in two places that are independent of the selected Style: the cascading baseline `BUILTIN_DEFAULT_BASELINE = "pixel art reference sheet"` (`ai/src/compose/builtins.rs:17`, chosen at `reference_sheet/mod.rs:340-344` and `shell/src/ai.rs:131-135`) and pixel-art adjectives baked into structure layout prose (`builtins.rs:132` "clean pixel-art lines", `:230` "clean pixel art"). `Style` (`core/.../composition/style.rs:14`) has no art-style discriminant, so picking a non-pixel look still prefixes "pixel art" onto every prompt. Make the pipeline style-parametric: add a serde-default `ArtStyleKind` to `Style` (default `PixelArt`), ship the art-style taxonomy as built-in Styles, replace the hardcoded baseline with a `baseline_for(kind)` lookup, and move the pixel adjectives out of structure prose into the pixel-art Style's modifiers. The selected style's `kind` then drives two gates: the pixel prose (this brief) and the pixel finisher (Brief 8). Universal disciplines — containment, no-border, identity/scale-lock (Brief 7) — stay style-agnostic at the structure level.

**Motivation.** Forge learning (style-selection-policy): pick the art style up front and let it carry the look. v2 only ships one style-agnostic built-in Style (`default_style()`, empty modifiers, shared look-negatives, `builtins.rs:288-297`), and pixel art lives in the baseline and structure prose rather than in a Style — so a clean-HD render is impossible without code surgery. Compose already layers `style.modifiers` and `style.look_negatives` as distinct optional segments that vanish when no style is picked (`compose/mod.rs:158-178`), so pixel prose riding in a pixel-art Style appears only when that style is selected, with zero conditionals in `compose()` itself. The one real ordering gap: in the verb the baseline is computed at `reference_sheet/mod.rs:340` *before* the style resolves at `:356-363`, so a kind-driven baseline needs the style resolved first; in `shell/src/ai.rs` the style already resolves (`:130`) before the baseline (`:131`), so only the verb reorders.

**Design.** Discriminant in `core`; taxonomy + baseline + prose in `ai`; finisher gate read in `shell`. No data migration — a serde-default field is non-breaking, and `Style` is shared by `StylePack`/`ProjectAi`, all of which tolerate an added `#[serde(default)]` field (the `empty_optionals_are_skipped` test at `style.rs:46` still holds).

1. `core/.../composition/style.rs`: add `#[derive(...)] pub enum ArtStyleKind { PixelArt, RetroPixel, PixelInspired, CleanHd, MapStyle }` with `#[serde(rename_all = "snake_case")]`, `Default` = `PixelArt`, and `is_pixel()` (true for PixelArt/RetroPixel/PixelInspired). Add `#[serde(default)] pub kind: ArtStyleKind` to `Style`.
2. `ai/src/compose/builtins.rs`: ship five built-in Styles, one per kind, with distinct sortable names so they all surface in the picker (dedup is by id; two styles sharing a name sort unstably, so names must differ). Each sets `kind` and look-appropriate `modifiers`/`look_negatives`. The pixel-art Style carries the pixel adjectives stripped from structure prose. Replace `BUILTIN_DEFAULT_BASELINE` with `baseline_for(kind) -> &'static str` (`PixelArt` → "pixel art reference sheet"; `CleanHd` → "high-detail reference sheet"; etc.).
3. `ai/src/compose/builtins.rs` prose: strip only the pixel adjective from the structure fragments at `:132`/`:230`, keeping the rest of each fragment intact — those fragments also carry migration-asserted style-agnostic phrases ("Professional sprite sheet format", asserted at `:530`; "White background"; "consistent scale across all views"). No migration test asserts the literal word "pixel", so stripping keeps them green.
4. `ai/src/verbs/reference_sheet/mod.rs`: resolve the style before computing the baseline, then call `baseline_for(style.kind)` (default `PixelArt` when no style). Surface the resolved `kind` and a finisher spec on the payload — `GenerateSheetPayload` (`:163-168`) and `SheetVariantOutput` carry no finisher field today, so this is a small net-new payload extension, not a no-op.
5. `shell/src/ai.rs`: read the resolved `kind` so the Land/finisher path (Brief 8) can gate on `is_pixel()`. `style_options` (`:156-164`) already merges builtin + project and sorts by name (`:163`), so new built-in styles appear in pickers automatically.

**Target files.**
- `core/src/project/library/composition/style.rs`
- `ai/src/compose/builtins.rs`
- `ai/src/compose/mod.rs`
- `ai/src/verbs/reference_sheet/mod.rs`
- `shell/src/ai.rs`

**API sketch.**
```rust
// core/src/project/library/composition/style.rs
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtStyleKind {
    #[default]
    PixelArt,
    RetroPixel,
    PixelInspired,
    CleanHd,
    MapStyle,
}

impl ArtStyleKind {
    /// Pixel-class styles run the pixel prose and the pixel finisher.
    #[must_use]
    pub fn is_pixel(self) -> bool {
        matches!(self, Self::PixelArt | Self::RetroPixel | Self::PixelInspired)
    }
}

pub struct Style {
    pub id: StyleId,
    pub name: String,
    #[serde(default)]
    pub kind: ArtStyleKind, // PixelArt by default; non-breaking
    // ...existing modifiers / look_negatives / model_pref / quality...
}

// ai/src/compose/builtins.rs
#[must_use]
pub fn baseline_for(kind: ArtStyleKind) -> &'static str; // replaces BUILTIN_DEFAULT_BASELINE

fn art_styles() -> Vec<Style>; // five built-ins, distinct sortable names, each sets kind
```

**Test plan.** Core (rstest + serde): `ArtStyleKind` default is `PixelArt`; `is_pixel` truth table; a `Style` deserialized from JSON lacking `kind` defaults to `PixelArt` (forward-compat); round-trip with `kind` set. ai (rstest + insta): `baseline_for` per kind; the five built-in Styles load with distinct names and correct kinds; `loads_structures_and_default_style` extended for the new styles; an insta snapshot proving the pixel-art Style's modifiers carry the adjectives and `compose_layout(character)` no longer contains "pixel" in the structure prose while the migration-asserted phrases survive. Verb (`#[tokio::test]`): `pixel_art_style_gives_pixel_baseline`, `clean_hd_style_gives_non_pixel_baseline` (asserts the prompt does not begin "pixel art"), and the existing `style_notes_become_the_prompt_baseline`/`prompt_override_is_sent_verbatim` still pass. Shell: `style_options` includes the new styles, name-sorted. No image-compare — text/serde only.

**Risks.** Two styles sharing a name sort unstably and both show — give each a distinct sortable name. Stripping the pixel adjective must preserve the rest of the fragment or the migration-equivalence tests break — append/strip surgically, never rewrite the fragment. The verb reorder (style-before-baseline) is a small but real control-flow change at `reference_sheet/mod.rs:340` vs `:356` — cover it with the baseline tests. Surfacing the finisher spec is net-new payload plumbing on `GenerateSheetPayload`/`SheetVariantOutput`, not free; keep it minimal (just the resolved kind plus an optional target grid). Over-scoping: do not change the universal containment prose here — that is Brief 7 and stays style-agnostic.

**Effort.** S.

**Dependencies.** None blocking. Enables Brief 8 (the finisher gates on `is_pixel`) and is the cleanest thing to land first in Wave 1. Built on the existing compose layering and the project-over-builtin style resolution (`ai/src/plugin/context.rs:91`).

### Brief 2 — Explicit fixed/variable invariant clauses on anchor-conditioned derivations

**Summary.** Every v2 generation that conditions on the anchor image (neutral reset, directional anchors, animation first frames) routes the anchor PNG into `reference_images` but never tells the model what about that reference stays fixed. The derivation prompts in `shell/src/anim_set.rs` and `shell/src/ai.rs` state only the variable axis (facing/pose/phase) and lean entirely on a numeric IP-Adapter strength to hold identity. Add an explicit clause — "keep the same character from the reference image: identical silhouette, palette, face, costume, and scale; change only the {pose/facing/phase}" — assembled by one pure helper in the `ai` crate and threaded through the existing `operation_hint`/`context_fragments` hooks in `compose` plus the raw-string derivation paths. No new image plumbing, no backend changes.

**Motivation.** Forge learning (reference-visible-first + identity-scale-lock): naming the reference as an input AND stating which attributes are locked vs free measurably reduces identity/scale drift. An IP-Adapter weight pulls globally toward the reference but cannot distinguish "keep the face and palette" from "let the pose change," so a directional turn or a seed frame can subtly redraw the costume or rescale the character — the exact drift the directional cascade exists to prevent. Verified in-tree: `anim_set.rs:187-195` `neutral_pose_prompt` and `:250-265` `directional_pose_prompt` state the variable axis only; `ai.rs:663-716` `run_first_frame` appends only `, single sprite frame, side view`; `anchor.rs` exposes only numeric `strength`. The invariant hooks already exist and are proven (`compose/mod.rs:52-56`, `:162-169`; the snapshot at `mod.rs:334` passes `operation_hint: Some("Preserve the character identity.")`), but `reference_sheet/mod.rs:382-383` and `shell/src/ai.rs:146-147` pass `None`/`&[]`, and the derivation paths bypass `compose` with raw `format!`. The plumbing is present; the prose is missing.

**Design.** Prose/invariant logic is asset-plan logic, so it lives in `ai/src/compose/`; the shell selects which axis is variable. No post-processing touched — scale stays *stated in prose*, which is the point of identity-scale-lock at the prompt layer.

1. New pure module `ai/src/compose/invariants.rs` (re-exported from `compose/mod.rs`) owning the canonical fixed/variable vocabulary. `identity_lock_clause(Facing)` → "keep the same character from the reference image: identical silhouette, palette, face, costume, and scale; change only the facing direction". The fixed list is a single private const so it cannot drift between call sites. The phrase opens by naming the reference image, satisfying reference-visible-first even when the model sees only text.
2. `ai/src/verbs/reference_sheet/mod.rs`: when the inputs carry any non-Style reference (`decode_references` produced a non-empty `reference_images`), pass `operation_hint: Some(&hint)` with `hint = identity_lock_clause(Pose)` into the existing `ComposeRequest` (lines 373-384). The `prompt_override` path stays verbatim, unchanged.
3. `shell/src/ai.rs`: a small `derive_prompt(base, suffix, conditioned)` helper appends the clause only when `reference_images` is non-empty, used by `run_first_frame`'s Generate arm, `generate_clip`'s self-seed, and the Inpaint arm when conditioning.
4. `shell/src/anim_set.rs`: `neutral_pose_prompt` appends the Pose clause, `directional_pose_prompt` the Facing clause, after the existing axis prose; the empty-subject guards (no leading comma) are preserved.
5. Optional: `compose_preview` passes the same `operation_hint` when references are present so the cockpit preview matches the verb.

**Target files.**
- `ai/src/compose/invariants.rs`
- `ai/src/compose/mod.rs`
- `ai/src/verbs/reference_sheet/mod.rs`
- `shell/src/ai.rs`
- `shell/src/anim_set.rs`

**API sketch.**
```rust
// ai/src/compose/invariants.rs (new)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VariableAxis { Pose, Facing, Phase }

impl VariableAxis {
    fn variable_phrase(self) -> &'static str; // e.g. "the facing direction"
}

/// Names the reference image, lists the locked attributes, then the one free
/// axis. Pure, deterministic, never empty.
#[must_use]
pub fn identity_lock_clause(axis: VariableAxis) -> String;

// ai/src/compose/mod.rs
pub mod invariants;
pub use invariants::{VariableAxis, identity_lock_clause};

// ai/src/verbs/reference_sheet/mod.rs — at the ComposeRequest:
let hint = (!reference_images_pre.is_empty())
    .then(|| identity_lock_clause(VariableAxis::Pose));
let req = ComposeRequest { /* … */ operation_hint: hint.as_deref(), context_fragments: &[] };

// shell/src/ai.rs
fn derive_prompt(base: &str, suffix: &str, conditioned: bool) -> String {
    let lock = conditioned
        .then(|| format!(", {}", identity_lock_clause(VariableAxis::Pose)))
        .unwrap_or_default();
    format!("{base}, {suffix}{lock}")
}

// shell/src/anim_set.rs
fn neutral_pose_prompt(canonical_prompt: &str) -> String;            // append Pose clause
fn directional_pose_prompt(neutral_prompt: &str, dir: AnchorDirection) -> String; // append Facing
```

**Test plan.** `ai` crate (rstest + insta): exhaustive cases over all three `VariableAxis` variants assert the clause names the reference image, contains the full fixed list (silhouette/palette/face/costume/scale), names only that axis's variable phrase, and has no leading comma; `insta::assert_snapshot!` freezes all three clause strings. Verb tests (extend `CapturingStub`): `anchor_reference_adds_identity_lock_to_prompt` passes a Subject-role reference and asserts the captured prompt contains the fixed-attrs list; `no_reference_omits_identity_lock` asserts from-scratch sheets are unaffected; existing `prompt_override_is_sent_verbatim` and `style_notes_become_the_prompt_baseline` still pass. Shell: unit-test `derive_prompt` (conditioned true appends lock + suffix; false appends only suffix); extend the existing `anim_set` prompt-layering tests to assert the clause is present and the empty-subject guard holds. No image-compare — text assembly only.

**Risks.** Prompt bloat — keep the clause to one short sentence; it rides in `operation_hint`, which compose appends near the end. Over-locking — "identical scale" could fight a paneled turnaround's intentional rescale, so the verb adds it only for non-Style references on the single-image conditioning path; paneled geometry stays driven by structure prose, and the clause speaks to the character, not the canvas. Wording regressions are invisible without snapshots — hence the insta freeze. No double-append: `run_first_frame` and the verb are separate paths that never both run on one request. Behavioral, not contractual — no schema or `ReferenceRole` change, no migration.

**Effort.** S.

**Dependencies.** None blocking. Builds on shipped surfaces: `compose`'s `operation_hint`/`context_fragments`, `decode_references` routing, the neutral/directional cascade. No bedrock prerequisite.

### Brief 3 — Bake containment + no-border/no-text + identity/scale-lock clauses into built-in structure layout prose

**Summary.** Extend the prose constants and per-structure `layout_negatives` that the deterministic composer (`ai/src/compose/builtins.rs`) already emits so every paneled built-in structure tells the backend: keep the entire subject inside each cell with equal margin on all four sides, render every cell at an identical bbox and pixel scale, connect cells by background only with no drawn borders/gridlines/gutters, and add no text/labels/captions. These disciplines are universal — they apply to every art style, pixel or clean-HD — so they live in the structure, not in any Style. A pure prose/negatives edit to five existing `fn`s, no new machinery. The payoff is that deterministic fixed-rect slicing downstream (Briefs 8 and 11, and the static-sheet path) can crop clean panels without subject bleed or a drawn border contaminating the cut.

**Motivation.** Forge learning (containment-prompt-rules + identity-scale-lock + no-text-no-ui-rule): diffusion backends let limbs/weapons/FX bleed across panel edges, draw decorative borders, scatter text/watermarks, and render the same subject at inconsistent sizes — each corrupts a fixed-rectangle crop, and none of it is style-specific. Verified against `ai/src/compose` greps: containment has NO matches (only "left-aligned starting at the left edge" — positioning — and "seamless edges" — tile tiling); no-border has NO matches; no-text is only partially covered by the optional Default Style's `look_negatives` ("watermark, text label, logo, cropped", `builtins.rs:293`); scale-lock is partial and inconsistent — "consistent scale across all views" on only 2 panels (`:132`, `:171`), no "identical bbox / pixel scale across cells". The payoff consumer is real: `build_composition` (`mod.rs:112-134`) emits fixed `Rect`s and the verb ships them verbatim (`reference_sheet/mod.rs:475-485`), so any later crop is only as clean as the generated image.

**Design.** `ai` crate only — prompt/contract logic, style-agnostic. Post-processing (the actual slicing/QC) stays out of scope; this makes the prompt safe for it. No data-model change (`Structure`/`StructurePanel` already carry `prose_fragment` and `layout_negatives`). Three coordinated edits in `builtins.rs`:

1. Containment + scale-lock positive clause, added once per paneled structure using the existing "only the first panel of a slot-group carries the shared clause" idiom (`:87-92`). One module-level const `PANEL_DISCIPLINE` appended (with a leading space, preserving the `{panel_w}/{panel_h}` clause) to the first non-empty prose fragment of `character()`, `item()`, `tileset()`, `custom()`. Compose joins fragments with ". ", so appending to the lead emits the clause once per sheet. Do NOT touch `single()` (free composition) or per-panel fragments cleared to "".
2. Leave the two ad-hoc "consistent scale" phrases (`:132`, `:171`) — the migration-equivalence tests assert legacy phrases survive; the shared clause is additive (append, never delete), which is why those tests stay green. This is the same fragment Brief 1 strips the pixel adjective from — coordinate the two edits on the same lines.
3. No-border / no-text negatives on the structures themselves via a shared `PANEL_NEG` const appended to each paneled structure's `layout_negatives` (so the rule holds even when no Style is picked — compose only folds `look_negatives` when `style: Some`). Keep each structure's domain negatives. `single()` keeps empty `layout_negatives`.

`mod.rs` and the verb need no change — appended text flows through the join logic and the verb forwards composed prompts and geometry. The one verb-file action is adding test coverage (none currently assert containment).

**Target files.**
- `ai/src/compose/builtins.rs`
- `ai/src/compose/mod.rs`
- `ai/src/verbs/reference_sheet/mod.rs`

**API sketch.**
```rust
// ai/src/compose/builtins.rs — new module-level consts (no public API change).

/// Containment + identity-scale-lock + no-border + no-text rules appended once
/// to the lead prose fragment of every paneled built-in structure. Universal —
/// applies to every art style.
const PANEL_DISCIPLINE: &str = "keep the entire subject inside its own cell with \
    equal empty margin on all four sides, never letting limbs, weapons, or effects \
    cross into a neighbouring cell; render every cell at an identical bounding box \
    and identical pixel scale; cells are separated by background only — no drawn \
    cell borders, dividing lines, gridlines, or gutters; no text, labels, captions, \
    numbers, or watermarks anywhere on the sheet";

/// Structure-level negatives carrying the same rules, so they hold even when no
/// Style is picked (compose only folds look_negatives when style is Some).
const PANEL_NEG: &str = "drawn cell borders, dividing lines, gridlines, gutters, \
    panel frames, text, labels, captions, page numbers, subject bleeding across \
    cell edges, inconsistent cell scale";

fn lead(base: &str) -> String { format!("{base} {PANEL_DISCIPLINE}") }
// each paneled structure appends PANEL_NEG to its existing layout_negatives;
// single() keeps String::new().
// No signature change to compose(), composition_for(), or the verb's invoke.
```

**Test plan.** In-crate (rstest-style + insta). builtins: `paneled_structures_emit_containment_clause` (rstest over CHARACTER/ITEM/TILESET/CUSTOM via the existing `compose_layout` helper at `:407`, assert "equal empty margin on all four sides", "identical bounding box and identical pixel scale", "no drawn cell borders"); `single_structure_has_no_containment_clause`; `paneled_structures_negatives_forbid_borders_and_text` (compose with NO style, assert "drawn cell borders" and "text, labels"); the four `*_migration_preserves_all_layout_phrases` tests stay unchanged (proves additive). insta: `paneled_positive_prompts_snapshot` per paneled built-in, `cargo insta review` to accept, commit `.snap`. Verb (`#[tokio::test]` with the WhiteStub + VerbRuntime harness): `generated_prompt_carries_panel_discipline` (invoke CHARACTER, assert `variants[0].generation.prompt` contains the margin clause and the negative contains "drawn cell borders"). Negative-path guard: the composed negative does not start with a bare comma after `PANEL_NEG` concatenation (mirrors `reference_sheet/mod.rs:1062`). Run `cargo nextest run -p pixhaus-ai` and `cargo clippy --tests -p pixhaus-ai -- -D warnings`.

**Risks.** Migration-equivalence tests are load-bearing — append, never delete legacy prose, so they stay green; the new clause is additive. Wording regressions need the insta freeze. Over-locking "identical scale" is fine here because `single()` (free composition) is excluded and the clause speaks to the character per cell, not the canvas. Coordinate with Brief 1 — both edit the same lead fragments at `:132`/`:230` (Brief 1 strips the pixel adjective, this appends the discipline clause). Resist adding slicing/QC here — that is core's job.

**Effort.** S.

**Dependencies.** Built on the existing composer and composition data model — landed, no blocker. Prerequisite for (not dependent on) the downstream paneled-sheet slicing work (Brief 11), the static-sheet path (Brief 9), and the finisher (Brief 8): it makes their input clean. Invisible to the verb's I/O contract — no migration.

### Brief 4 — Two-pass Euclidean chroma key: interior threshold + border flood-fill + edge-band cleanup

**Summary.** Replace v2's single-pass, per-channel `abs_diff <= tolerance` chroma keyer with a two-pass Euclidean-distance keyer porting `magenta-distance-removal + edge-clean-depth + trim-border`. Pass 1 keys interior pixels within a tight squared-Euclidean distance of the key color. Pass 2 floods inward from the border at a looser distance so connected AA fringe is removed without eating interior pixels that merely share the key's hue. A final edge-band cleanup softens the residual rim, and an optional fixed border trim removes cell-seam bleed. Lives entirely in `core/src/transforms/normalize.rs` as a pure function; `shell/src/bg_removal.rs` and `shell/src/studio.rs` switch their call sites. No GPU, no async, no new crates.

**Motivation.** Forge learning: the production removal path was never a flat threshold — it was distance-based removal, an edge-clean depth, then a border trim, because a flat key always leaves a colored halo on AA-blended sheets. v2 regressed to the naive version. Verified: `normalize.rs:73-76` `ChromaKey::matches` is `abs_diff <= tolerance` per channel; `:55-69` defaults tolerance 16; `chroma_key` (`:186-199`) is one flat per-pixel pass — no border seeding, flood-fill, edge-band, or trim. A per-channel box test treats `#E040E0` (an AA blend of `#FF00FF` over a dark edge) as keepable, so the partially-desaturated rim survives as a magenta ring that reads as a glow over a dark game background. A squared-Euclidean primitive exists at `color/ops.rs:58-65` (`similar_colors`) but is wired only to flood-fill/wand and unused by the keyer. `judge_key` (`:314`) already flags failures; this fixes the removal itself. It is the single highest-leverage deterministic post-pass — it touches `key_background_now`, the live preview, the clip-review thumbnails, the land-chroma path, and the static-sheet slice path (Brief 9).

**Design.** All algorithm work in `core` (`thiserror`, reuse `transforms::error::Result`); the shell only swaps call sites. Three stages on `&PixelBuffer` with explicit stride.

1. Distance primitive: a squared-Euclidean RGB helper in `normalize.rs` working in `i32` over RGB (max 3·255² = 195075, fits `i32`). Do NOT reuse `color::ops::similar_colors` (it normalizes to f32 and includes alpha). The user-facing tolerance stays a `u8` slider (0..=128) squared internally: `interior_thresh = 3 * (tol as i32).pow(2)`, so existing saved values behave sanely. Border tolerance is `tol` scaled up (~1.5×).
2. Extend `ChromaKey` without breaking serde: add `#[serde(default)] border_tolerance: Option<u8>` and `#[serde(default)] trim_border: u8` so old MessagePack/JSON still loads.
3. Two-pass keyer `chroma_key_two_pass(buf, key)`: Pass 1 (interior) sets alpha 0 where squared distance ≤ `interior_thresh`, preserving RGB. Pass 2 (border flood) is a 4-connected flood from every still-opaque edge pixel via an explicit `VecDeque` queue and a `vec![false; w*h]` visited mask (bounded, not recursion — respects the 8K perf constraint), clearing pixels within `border_thresh`; the flood cannot cross an opaque dissimilar interior pixel, so a same-hue interior region is not eaten. Edge-band cleanup: one dilation ring — any still-opaque pixel 4-adjacent to a now-transparent pixel AND within `border_thresh` gets cleared. Trim: if `trim_border > 0`, force the outer ring transparent. RGB stays on every cleared pixel.
4. Wire-up: keep `chroma_key` as a thin shim calling `chroma_key_two_pass` so `document.rs:937`, the `bg_removal.rs` preview, and `studio.rs:1913` thumbnails upgrade with zero call-site churn; `judge_key` is unchanged.

**Target files.**
- `core/src/transforms/normalize.rs`
- `core/src/transforms/mod.rs`
- `shell/src/bg_removal.rs`
- `shell/src/studio.rs`
- `shell/src/document.rs`

**API sketch.**
```rust
// core/src/transforms/normalize.rs
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChromaKey {
    pub color: Rgba,
    /// Interior squared-distance tolerance as a 0..=128 slider; squared internally.
    pub tolerance: u8,
    /// Looser flood distance. None derives ~1.5x from tolerance. serde-defaulted.
    #[serde(default)]
    pub border_tolerance: Option<u8>,
    /// Outer-border pixels forced transparent after keying. 0 = off.
    #[serde(default)]
    pub trim_border: u8,
}

impl ChromaKey {
    pub const fn magenta() -> Self; // #FF00FF, tolerance 16, border None, trim 0
    pub const fn green() -> Self;
    fn interior_threshold(self) -> i32 { 3 * (self.tolerance as i32).pow(2) }
    fn border_threshold(self) -> i32 {
        let t = self.border_tolerance
            .unwrap_or(self.tolerance.saturating_add(self.tolerance / 2));
        3 * (t as i32).pow(2)
    }
}

/// Squared Euclidean RGB distance, alpha ignored. Max 195075.
fn rgb_dist_sq(a: Rgba, b: Rgba) -> i32;

/// Interior threshold, border-seeded 4-connected flood, edge-band ring,
/// optional border trim. RGB preserved on cleared pixels.
#[must_use]
pub fn chroma_key_two_pass(buf: &PixelBuffer, key: ChromaKey) -> PixelBuffer;

/// Back-compat shim — existing call sites now two-pass.
#[must_use]
pub fn chroma_key(buf: &PixelBuffer, key: ChromaKey) -> PixelBuffer {
    chroma_key_two_pass(buf, key)
}
```

**Test plan.** Unit (rstest, extend the existing `normalize.rs` module): `rgb_dist_sq` exactness and symmetry; interior-pass parity (the existing `chroma_key_removes_background_keeps_subject` still passes); halo removal — a magenta field, a subject block, and a 1px `#E040E0` AA rim → the rim is alpha 0 after two-pass but would survive the old per-channel pass; interior-hue protection — an interior block within `border_tolerance` of the key but surrounded by dissimilar subject pixels stays opaque (proves border-seeding); RGB preserved on cleared pixels; `trim_border` clears the outer ring; serde back-compat — deserialize a `ChromaKey` lacking the new fields via `rmp_serde` and `serde_json`, assert defaults; `judge_key` still returns `Ok`/`Missed`/`TooBroad`. Proptest: `chroma_key_two_pass` never increases opaque count and never clears a pixel beyond `border_threshold` outside the trim ring. Image-compare: a magenta + AA-rim + subject fixture keyed two-pass vs a committed golden. Run `cargo nextest run -p core` then the shell tests for `land_chroma`/studio.

**Risks.** Behavior change — every keyed sprite now floods; verify the interior-protection test holds so same-hue interior detail (a magenta gem on a magenta-keyed sheet) is not eaten; if it is, lower the `border_tolerance` default toward `tolerance`. serde compatibility — `#[serde(default)]` is mandatory for old `.pixhaus` files to load; covered by the back-compat test; `ChromaKey` keeps `Eq`. 8K memory — the visited mask is up to 64 MiB at 8192² but transient and on an explicit action, not the brush hot path; use the explicit queue, never recursion. Edge-band tuning — lock to exactly one dilation ring gated by `border_threshold`. The `studio.rs` thumbnail cache keys on `(bg_key_color, bg_tolerance)`; if `border_tolerance`/`trim_border` become user-adjustable later, add them to the cache signature (out of scope now).

**Effort.** M.

**Dependencies.** None blocking. Self-contained in `core` + three shell call sites. Reuses `PixelBuffer` (stride-explicit) and `transforms::error::Result`. Independent of the AI background-removal backend (still the fallback in `bg_removal.rs`). The static-sheet path (Brief 9) relies on this for clean magenta-grid keying. A future consolidation of `rgb_dist_sq` with `color::ops::similar_colors` is a follow-up, not a prerequisite.

### Brief 5 — Connected-component isolation for alpha measurement: largest-vs-all + min-area filter

**Summary.** Add a 4-connected component-labeling primitive to `core`'s normalize module that labels the opaque mask (alpha > threshold) into components with per-component area / bbox / `touches_edge`, then lets the bbox measurement consume them under two policies: `Largest` (keep only the biggest — the hero body) and `All { min_area }` (keep every component at or above N opaque pixels, drop the rest). Wire it through `NormalizeOptions` so `measure`, `reference_height`, `normalize_one`, and the studio Land pass crop against a speck-cleaned mask. Today a single detached speck after keying widens the bbox, shrinks the body under the shared-height scale, and shifts the foot baseline — this fixes that at its root.

**Motivation.** Forge learning (connected-components + component-mode-largest-vs-all): pairing CC labeling with a largest-vs-all selector after keying isolates the hero for clean cropping while preserving intentional multi-part FX. Verified: `normalize.rs:206 measure()` takes the bbox over the entire alpha channel (`px.a > threshold`, `:213-225`) with no component analysis. That bbox feeds three degrading paths — `reference_height` (`:502-503`, a top speck inflates the shared scale target), per-frame scale in `normalize_one` (`:453-461`, a speck stretches the crop so the body renders smaller — the "body shrinks under noise" failure), and `foot_baseline_y = max_y` (`:245`, a speck below the feet drags the baseline). No CC primitive exists anywhere: `selection/algorithms.rs:220` magic_wand is single-seed BFS, `color_range` is global-by-color, `fill.rs` flood_fill is the bucket. This is greenfield and the foundation the edge-touch QC stream will reuse.

**Design.** Core, in `core/src/transforms/normalize.rs` (~80–120 lines; `measure` is its only consumer today). Three layers.

1. Labeling primitive `label_components(buf, alpha_threshold) -> Vec<Component>`: a flat `Vec<u32>` label array, iterative 4-connected BFS using a reused work stack (no per-component realloc); accumulate `area`, bbox, and `touches_edge` (any member on the 1px border). Iterative, not recursive (8K stack safety; the BFS is bounded by opaque-pixel count, not canvas area). Expose `pub` now — it's the QC stream's foundation.
2. Selection policy `ComponentMode { WholeAlpha, Largest, All { min_area } }`. `measure_components(buf, alpha_threshold, mode) -> FrameMetrics`: `WholeAlpha` delegates to today's `measure` body (refactor `measure` to call it, one bbox code path); `Largest` picks the max-area component (ties by lowest label for determinism); `All` unions bboxes of components ≥ `min_area`, falling back to `empty = true` if the filter drops everything.
3. Wire through options: add `component_mode: ComponentMode` to `NormalizeOptions` defaulting to `WholeAlpha` in `square()` (no behavior change). In `normalize_frames` the two `measure(...)` calls (`:500`, `:522`) become `measure_components(...)` with `opts.component_mode`. Pass-1 (source) uses the mode so `reference_height` and the crop see the cleaned bbox; pass-2 (verify) keeps `WholeAlpha` so the QC report describes the landed pixels — document the asymmetry. Note: `measure` only computes the crop rect; a speck *inside* the kept bbox still survives (acceptable for `Largest`, which already excludes detached specks; true pixel removal is a follow-up).

Shell wiring (`app.rs:2944` `compute_normalize`, not `studio.rs` — the gap's target file is stale): add `component_mode` to the literal, defaulting to `Largest` for the Land pass when `remove_on_land` is set, else `WholeAlpha`. Surface a small studio toggle (Largest vs All) next to the key controls via a `land_component_mode(remove_on_land, all_parts, min_area)` helper. Start with the mode toggle only; default `min_area` to 4–8px. No async, no locks — `compute_normalize` runs inline over owned buffers.

**Target files.**
- `core/src/transforms/normalize.rs`
- `shell/src/app.rs`
- `shell/src/studio.rs`

**API sketch.**
```rust
// core/src/transforms/normalize.rs
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Component {
    pub area: u32,
    pub bbox_x: u32, pub bbox_y: u32,
    pub width: u32, pub height: u32,
    pub touches_edge: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ComponentMode {
    WholeAlpha,
    Largest,
    All { min_area: u32 },
}

/// Iterative 4-connected BFS over the opaque mask; bounded by opaque-pixel count.
#[must_use]
pub fn label_components(buf: &PixelBuffer, alpha_threshold: u8) -> Vec<Component>;

/// Measures the opaque bbox under `mode`. WholeAlpha matches `measure`.
#[must_use]
pub fn measure_components(buf: &PixelBuffer, alpha_threshold: u8, mode: ComponentMode) -> FrameMetrics;

// NormalizeOptions gains: pub component_mode: ComponentMode  (square() defaults WholeAlpha)

// shell/src/studio.rs
pub(crate) fn land_component_mode(remove_on_land: bool, all_parts: bool, min_area: u32) -> ComponentMode;
```

**Test plan.** Unit (rstest, reuse the `solid` helper): `label_components_counts_separate_runs` (a 4px body + 1px speck → two components, areas 4 and 1); `label_components_marks_edge_touch`; `label_components_diagonal_is_two_components` (4-connectivity guard); `measure_largest_ignores_speck` (body + a 1px speck → Largest matches the body, differs from WholeAlpha); `measure_all_keeps_above_min_area`; `measure_all_empty_when_all_filtered`; the headline regression `normalize_largest_keeps_body_height` (two frames, one with a top speck, assert the speck frame's reference_height/scale matches the no-speck baseline); `measure_whole_alpha_matches_legacy` (proves the refactor is behavior-preserving). Proptest: sum of component areas == total opaque count; `Largest` bbox area ≤ `WholeAlpha` bbox area. Numeric metrics, no snapshot. Run `cargo nextest run -p core` and `cargo clippy --tests -p core -- -D warnings`.

**Risks.** Default drift — `component_mode` must default to `WholeAlpha` or every existing test and the live Land pass change silently; covered by the legacy-equality test. Speck inside the kept bbox still lands — Largest excludes detached specks (the common case), not enclosed ones; documented bounded scope. 8K — iterative BFS with a reused stack is mandatory; labeling cost is bounded by opaque-pixel count, not w·h. Determinism — break Largest ties by lowest label. Memory — a `Vec<u32>` of length w·h is 256 MB at 8192²; acceptable for small studio pick buffers, note the caveat if ever called on a full canvas.

**Effort.** M.

**Dependencies.** None hard. Self-contained in normalize plus a one-line options addition in `app.rs:2944` and a studio toggle. It is the foundation the edge-touch QC stream depends on (`Component::touches_edge`) — land this first. No bedrock, ai, or render changes.

### Brief 6 — Edge-touch / safe-margin QC flag plus post-scale parity delta in the normalize report

**Summary.** Extend the core normalize pass to detect two defects it ships silently: subjects whose bbox touches a canvas edge (the artifact of `repad`'s silent out-of-bounds clip), and per-frame body-shrink after scale correction. Add `safe_margin` to `NormalizeOptions`, `edge_touch_frames: Vec<usize>` and `scale_parity_pct: u32` to `NormalizeReport`, and surface both as a new inspector row and a per-frame strip badge in `shell/src/studio.rs`. This turns the Normalize review from advisory into a real gate.

**Motivation.** Forge learning (edge-touch-qc + body-shrink-qc): these caught the two failure modes artists complain about most — a subject too large for the cell ships visibly cropped, and an attack frame the scale pass under-corrected ships noticeably smaller. Both are silent in the artifact. Verified: `repad` (`normalize.rs:379-396`) writes the centered subject and `continue`s past any out-of-bounds row/column, clipping an oversized subject to a blank strip with zero signal (the doc at `:405` admits "re-pad clips it to a blank strip"). `NormalizeOptions` (`:134-153`) enforces only `bottom_margin`, so a left/right/top edge touch is never flagged. `scale_match_pct` (`:534-538`) is computed from SOURCE metrics before correction, so it reports pre-scale spread and structurally cannot flag post-scale body-shrink. A whole-worktree grep for `touches_edge`/`safe_margin`/`scale_parity` returns zero Rust hits. The inspector (`studio.rs:2391-2445`) renders three rows; there is nowhere to see clipping or post-scale parity. The report already carries drift/scale/seam and is already rendered, so this is a low-cost, high-leverage extension.

**Design.** Detection in core (post-processing); the ai crate stays out. In-place extension of `normalize.rs` plus its render surface.

Core:
1. `bbox_touches_edge(metrics, canvas_width, canvas_height, margin)` — a frame touches when its bbox left < margin, top < margin, right > w-1-margin, or bottom > h-1-margin. Operate on OUTPUT metrics (`out_metrics`, `:522`) — the landed bbox pinned to row 0 / col 0 is the clip signature. Skip empty frames.
2. Add `safe_margin: u32` to `NormalizeOptions`, default 0 in `square()` (preserves behavior). Document it as independent of `bottom_margin` (placement vs QC).
3. Do NOT add a `touches_edge` field to `FrameMetrics` (it would force a value `measure` can't fill); derive it in `normalize_frames` from `out_metrics` + `safe_margin`.
4. Add to `NormalizeReport`: `edge_touch_frames: Vec<usize>` (`#[serde(default, skip_serializing_if = "Vec::is_empty")]`) and `scale_parity_pct: u32` (post-correction: `min_corrected_visible_height * 100 / max_corrected_visible_height` over non-empty `out_metrics`, 100 when ≤1 non-empty frame). Keep `scale_match_pct` — it is a legitimate pre-scale signal the seam/scale UI uses; `scale_parity_pct` is additive.
5. In `normalize_frames` after `out_metrics`: compute `edge_touch_frames`, push a warning per frame; compute `scale_parity_pct`, push a warning under the existing `< 60` band.

Shell:
6. Inspector: add two `report_row` calls after Scale-match — an "Edge clear" row (Ok when empty, Error otherwise) and a "Scale parity" row (reuse `scale_status` bands on `scale_parity_pct`); add `edge_status` next to `drift_status`/`scale_status`, all `pub(crate)`.
7. Strip surface: when `report.edge_touch_frames.contains(&idx)`, render a `palette.error` badge (`crate::icons::WARNING` + "clipped") under the existing drift label.
8. Call sites constructing `NormalizeOptions` add `safe_margin`: `app.rs::compute_normalize` (`:2944`) and `ai.rs` loop normalize (`:961`); set `0` initially (AI loop path stays 0). Adding a field to a struct-literal type is a compile break at every site — the compiler catches misses.

No GPU, no async.

**Target files.**
- `core/src/transforms/normalize.rs`
- `shell/src/studio.rs`
- `shell/src/app.rs`
- `shell/src/ai.rs`

**API sketch.**
```rust
// core/src/transforms/normalize.rs
/// True when the landed bbox enters the `margin` band at any of the four edges.
/// Empty frames never touch. Operates on OUTPUT metrics.
#[must_use]
pub fn bbox_touches_edge(m: &FrameMetrics, canvas_width: u32, canvas_height: u32, margin: u32) -> bool;

pub struct NormalizeOptions {
    // ...existing...
    /// Minimum transparent border on all four sides. A landed bbox entering
    /// this band is flagged as edge-touching. 0 disables. Independent of
    /// bottom_margin (placement, not QC).
    pub safe_margin: u32,
}

pub struct NormalizeReport {
    // ...existing...
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edge_touch_frames: Vec<usize>,
    /// Post-correction parity: shortest landed subject over tallest, x100.
    /// 100 = every corrected subject agrees in height. Unlike scale_match_pct
    /// (pre-scale spread), measures the frames that actually land.
    pub scale_parity_pct: u32,
}

// shell/src/studio.rs
#[must_use]
pub(crate) fn edge_status(touched_frames: usize) -> ReportStatus; // Ok at 0, else Error
```

**Test plan.** Core (rstest + proptest): `bbox_touches_edge_flags_each_side` (case per edge at margin 0, interior false, empty false); `safe_margin_widens_the_band`; `oversized_subject_lands_edge_touching` (reproduces the repad clip, asserts `edge_touch_frames` + a warning — the regression for the silent-clip bug); `clean_sequence_has_no_edge_touch`; `scale_parity_measures_post_correction` (~100 even when pre-scale `scale_match_pct` is low); `scale_parity_flags_residual_shrink`; proptest — `scale_parity_pct` always 0..=100 and 100 when all heights equal; update the empty-input path to assert empty `edge_touch_frames` and `scale_parity_pct == 100`. Shell (rstest, alongside the drift/scale/seam tests): `edge_status_is_ok_only_at_zero`; `edge_status_color_is_error`; extend the clean-baseline test to assert both new statuses Ok; a normalize-frames integration test producing an edge-touching frame and asserting the inspector reads Error. Serde round-trip of a report with and without `edge_touch_frames` (empty vec skipped on the wire). No image-compare — numeric/text classification.

**Risks.** Struct-literal break at three sites — `safe_margin` is a compile error until all updated; do not paper over with `#[non_exhaustive]` or `Default`. `scale_parity_pct` is a required field — any external code deserializing an old report without it fails, but v2 does not persist `NormalizeReport` (transient review artifact, not in the B2/B3 schema), so internal-only; confirm no `.pixhaus` fixture embeds a report. Edge-touch on OUTPUT metrics flags a deliberately full-bleed subject as "clipped" — a by-design false positive; `safe_margin` default 0 only flags actual bbox-at-edge, the badge is advisory, document it as advisory-strong not a hard Land block. `scale_parity_pct` vs `scale_match_pct` naming is close — doc comments must spell out pre-scale vs post-scale; keep both. The reference-height cap means some shrink is intentional — reuse the existing `< 60` warning band rather than a stricter one to avoid spamming.

**Effort.** M.

**Dependencies.** Standalone — no bedrock. Touches the normalize pass and its built render surface plus the two option call sites. Pairs with connected-components as the bbox source if/when that lands, but does not require it — `measure` already produces the bbox this consumes.

### Brief 7 — GUI sprite export: transparent-PNG frames, hard-masked looping GIF, packed atlas

**Summary.** A finished v2 sprite can only leave the app through the headless CLI (`shell demo|sheet|gen`). `headless.rs::write_outputs` (`:209-237`) writes per-frame transparent PNGs and an infinitely-looping GIF, but nothing in the GUI calls it, and no atlas writer exists anywhere. The only `rfd` save dialogs are palette export (`palette_panel.rs:594`) and the `.pixstyle` pack (`library.rs:637`). Add a reusable, UI-agnostic sprite exporter in `core` (frame slicing, hard-alpha masking, atlas packing — pure pixel ops, no IO) plus a thin shell layer (`shell/src/export.rs`) that composites the active sprite's frames, runs an off-thread `rfd` dialog and `image`-crate encode on `spawn_blocking`, and reports back over `ShellMsg`. Three shapes: a per-frame transparent PNG sequence, an infinitely-looping GIF with a hard alpha mask (≥128 → opaque, else the single reserved transparent index) to kill soft-fringe halos, and a packed atlas PNG plus a sidecar JSON manifest of frame rects. The headless `write_outputs` path is refactored to call the same core helpers so CLI and GUI share one code path.

**Motivation.** Forge learning (transparent-gif-export + recompose-transparent-sheet + output-bundle-shape): a naive RGBA→GIF encode produces a halo of semi-transparent fringe because GIF has 1-bit alpha and a per-frame quantizer dithers the partially-transparent edge into visible speckle. The fix is two-part — hard-threshold alpha to 0/255 at ≥128 before encoding, and reserve one palette index as transparent so no edge pixel borrows a neighbor's color — plus a predictable bundle (named frames + manifest) so the Unity importer consumes output without guessing the grid. v2 authors sprites well but cannot ship them from the GUI; the encode machinery is walled off in the CLI, and even there `write_outputs` (`:222-225`) feeds RGBA straight to `GifEncoder` with no alpha threshold, so the anti-halo technique exists nowhere in v2. This closes the loop and avoids the halo that would otherwise make every exported GIF look worse than the on-canvas sprite.

**Design.** Per the locked constraint: pure pixel/geometry (slicing, hard mask, atlas packing, manifest math) lives in `core`, NOT `render` (wgpu-only, no `image`). Encoding and disk IO live in the shell — v2 has no `io` crate, so export uses the `image` crate directly (already a shell dep with `png`/`gif` features). Bundle-shape policy stays in shell; the `ai` crate is untouched.

Core (`core/src/export/mod.rs`, new):
- `hard_mask_alpha(buf, threshold)` — clamp every alpha to 0 or 255 at `threshold` (forge ≥128 default), reusing the row/stride iteration from `normalize::chroma_key`.
- `pack_atlas(frames, opts)` — pack equal-size frames into a grid (single row, fixed columns, or near-square auto). All frames share the canvas size, so this is a fixed-cell grid pack, not a bin-packer. Returns the atlas `PixelBuffer` plus `Vec<FrameRect>`; `thiserror`-typed errors (empty input, size mismatch, dimension overflow against the 8K constraint).
- These are what `write_outputs` is refactored to call.

Shell (`shell/src/export.rs`, wired from `studio.rs` Land and the editor File menu):
- `ExportRequest` built on the UI thread from `doc.composite_frame(idx)` over `0..doc.frame_count()` (the loop `headless.rs::composite_all_frames` uses).
- `run_export(req)` on `runtime.handle().spawn_blocking`: opens the `rfd` save dialog OFF the UI thread (rfd sync API blocks — mirror `library.rs::run_export`), then encodes. PngSequence: `image::RgbaImage::from_raw` (drop row padding like `headless.rs::to_rgba_image`) → save `{base}_{i:03}.png`. LoopingGif: `hard_mask_alpha` each frame, encode with `GifEncoder` + `Repeat::Infinite`; the encoder routes alpha-0 to the transparent index, and the hard mask guarantees every edge pixel is 0 or 255. Atlas: `pack_atlas` → one PNG + a `{base}.atlas.json` manifest (`{ cell_w, cell_h, columns, frame_ms, frames: [{index, x, y, w, h}] }`).
- Result returns over a new `ShellMsg::SpriteExported { result: Result<Option<PathBuf>, String> }` (drained in `app.rs::logic`); `on_sprite_exported` sets the status line and `request_repaint`. No lock held across dialog/encode; the document is read into owned `PixelBuffer`s before the worker spawns.
- UI: an "Export sprite…" button on the studio Land stage and a File-menu item, opening a plain egui window to pick kind and (for atlas) column count.

**Target files.**
- `core/src/export/mod.rs`
- `core/src/lib.rs`
- `shell/src/export.rs`
- `shell/src/studio.rs`
- `shell/src/app.rs`
- `shell/src/editor.rs`
- `shell/src/headless.rs`

**API sketch.**
```rust
// core/src/export/mod.rs (pure pixel/geometry; thiserror)
use crate::canvas::PixelBuffer;

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("no frames to export")] NoFrames,
    #[error("frame {index} is {got:?}, expected {expected:?}")]
    SizeMismatch { index: usize, got: (u32, u32), expected: (u32, u32) },
    #[error("atlas dimensions {w}x{h} exceed the {max}px limit")]
    TooLarge { w: u32, h: u32, max: u32 },
}

#[must_use]
pub fn hard_mask_alpha(buf: &PixelBuffer, threshold: u8) -> PixelBuffer;

#[derive(Copy, Clone, Debug)]
pub enum AtlasLayout { SingleRow, Columns(u32), AutoSquare }
#[derive(Copy, Clone, Debug)]
pub struct AtlasOptions { pub layout: AtlasLayout, pub max_dim: u32 } // max_dim default 8192
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FrameRect { pub index: usize, pub x: u32, pub y: u32, pub w: u32, pub h: u32 }
pub struct Atlas { pub image: PixelBuffer, pub columns: u32, pub frames: Vec<FrameRect> }

/// Frames must share dimensions.
pub fn pack_atlas(frames: &[PixelBuffer], opts: AtlasOptions) -> Result<Atlas, ExportError>;

// shell/src/export.rs (encode + IO)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExportKind { PngSequence, LoopingGif, Atlas { columns: u32 } }
pub(crate) struct ExportRequest {
    pub kind: ExportKind, pub frames: Vec<PixelBuffer>,
    pub frame_ms: u32, pub base_name: String,
}
/// Off the UI thread: rfd dialog + image encode. `Ok(None)` = cancelled.
pub(crate) fn run_export(req: ExportRequest) -> Result<Option<std::path::PathBuf>, String>;

// shell/src/app.rs
// ShellMsg::SpriteExported { result: Result<Option<PathBuf>, String> }
```

**Test plan.** Core (unit + proptest + image-compare): `hard_mask_alpha` cases — alpha 127→0, 128→255, 255→255, 0→0, RGB untouched; proptest — every output alpha ∈ {0,255}, idempotent. `pack_atlas` — `NoFrames` on empty, `SizeMismatch` on a differing frame, `Columns(3)` over 7 frames → 3×3 with two transparent cells and correct rects, `AutoSquare` over 4 → 2×2, `TooLarge` past 8192; image-compare a packed 2-frame atlas against a fixture; insta-snapshot the manifest JSON. Shell: LoopingGif — build 3 frames with a semi-transparent edge ring (alpha 64), export, re-decode via `anim::decode_clip`, assert no decoded pixel has 0 < alpha < 255 (halo gone) and frame count round-trips; PngSequence — assert N named files each decode to source size; Atlas — one PNG + one JSON, rect count == frame count. The rfd dialog is not unit-tested; inject a target path behind a `#[cfg(test)]` `dest: Option<PathBuf>` seam. Headless regression — `shell demo` still writes `loop.gif` + frames after the refactor, plus a new assert that the demo GIF is halo-free.

**Risks.** The `image` `GifEncoder` may run its own per-frame quantizer and not honor a reserved index cleanly; if a halo persists after hard-masking, drop to the lower-level `gif` crate and build the palette with index 0 reserved transparent — the round-trip test catches it; budget for the fallback. GIF is 256 colors — a rich sprite plus the reserved index can exceed it and force dithering; quantize to 255 + 1 reserved and document GIF as lossy (PNG and atlas stay lossless; `color_quant` is available transitively). rfd on the UI thread freezes egui — must run inside `spawn_blocking`. 8K constraint — `pack_atlas` guards with `max_dim` and returns `TooLarge` rather than allocating hundreds of MB. Stride — drop row padding (copy `headless.rs::to_rgba_image`) or the encode smears. Scope: PNG/GIF/atlas only, no Aseprite/PSD/TMX (no `io` crate).

**Effort.** M.

**Dependencies.** Reuses `normalize::chroma_key` (pattern), the `PixelBuffer` stride API, `DocumentStore::{composite_frame, frame_count}`, the `ShellMsg` mpsc channel + `spawn_blocking` worker pattern, the off-thread rfd pattern, and the `image` encode already in `headless.rs`. Load `pixhaus-rfd`, `pixhaus-image`, `pixhaus-tokio`, `pixhaus-testing-conventions`; `pixhaus-color-quant` only if the GIF palette fallback is needed. No dependency on the AI pipeline. Touches the shared `headless.rs` write path — coordinate with concurrent CLI work.

### Brief 8 — Style-gated pixel-art finisher: palette-snap + downscale-to-grid post-pass on generated frames

**Summary.** Add a deterministic "make it pixel art" finisher that runs on AI-generated frames after they return at backend resolution — but only for pixel-class art styles. It (1) true-downscales each frame to a target pixel grid with nearest-neighbor, and (2) snaps every opaque pixel to a small palette extracted from the frame (or supplied by the anchor's `extracted_palette`). The selected `ArtStyleKind` (Brief 1) drives whether it runs: `is_pixel()` styles finish; clean-HD and map-style skip it and run only `normalize_frames`. Today the only post-pass on AI output is `normalize_frames` (chroma-key + bbox-crop + scale-to-reference-height, `ai.rs:969`, `app.rs:2954`) — no palette reduction, no pixel grid — so frames land at gpt-image 1024–2048 or Seedance native and look like soft AI art regardless of style. All building blocks exist in core (`scale_integer_down`/`scale_nearest`, `extract_palette_from_rgba8`, `nearest_color_index`) but the snap helper (`quantize_buffer`) is a `pub(crate)` shell helper used only by the manual "Reduce to palette" button. Lift the snap into core, build a `finish_frames` pass beside normalize, and wire it into both AI land paths plus the single-sprite variant land path under the style gate.

**Motivation.** Forge learning (single-sprite-center spirit + LANCZOS): the forge got pixel-art-looking output from ~1024 renders by pairing normalization with a deterministic resample down to a low grid — the resample turns soft gradients into discrete pixels. That step only makes sense for pixel art; a clean-HD sprite must stay at full resolution. v2 ported the normalization spirit but dropped the finisher entirely: no enforced pixel grid, no palette reduction on any AI-result path. Verified: single-sprite variants store the PNG verbatim (`cockpit.rs:1810/1833`) and only *display* crisp via `TextureOptions::NEAREST` (`:1800-1804`) — the underlying pixels are 1024px soft art; animation frames go through `normalize_frames` (scale to a reference *height* via `scale_nearest`) but never to a low grid and never snap color; the one resample on backend output (`resize_png_to` Lanczos3, `openai.rs:554-559`) merely fits a snapped gpt-image size back. The "make it pixel art" step the pixel-art pipeline implicitly relies on the prompt to deliver is missing; gating it on the style means it is on-mission for pixel art and correctly absent for clean-HD. Gating `finish_frame` on `kind` is a clean superset of an always-on finisher and changes only the shell wire-up, not core.

**Design.** Per the boundary: post-processing (grid + palette snap) lives in core; the style gate and prompt/asset-plan stay in shell/ai. The finisher is pure core and style-agnostic by itself; the shell decides whether to call it based on the resolved `ArtStyleKind`.

1. Lift the snap into core: move `quantize_buffer` (`palette_panel.rs:991`) into `core` as `transforms::finisher::snap_to_palette(buf, palette)` (iterates row_mut stride rows, calls `Palette::nearest_index` → `nearest_color_index`, leaves alpha-0 pixels untouched). Re-export it and have `reduce_to_palette` call the core fn so there is one snap implementation.
2. New `core/src/transforms/finisher.rs` beside normalize.rs. It owns `FinishOptions { target_grid: Option<(u32,u32)>, palette: PaletteSource, alpha_threshold: u8 }`; `PaletteSource { Extract { max_colors, quantize_bits }, Fixed(Palette), None }` (Extract uses `extract_palette_from_rgba8` then `Palette::from_colors`); `finish_frame`/`finish_frames` returning `FinishedFrame { buffer, palette }`. Downscale rule: prefer `scale_integer_down` when the target divides evenly (lossless top-left sample), else `scale_nearest`. Do NOT add Lanczos in core — a bilinear/Lanczos downscale would reintroduce the soft edges the finisher exists to remove (a deliberate divergence from the literal forge port; document it). Order: downscale first, then extract+snap on the small buffer. The finisher itself has no concept of art style — it is the caller's gate.
3. The style gate (shell): each land path resolves the `ArtStyleKind` for the generation (from the verb payload surfaced in Brief 1, or the picked Style), and calls the finisher only when `kind.is_pixel()`. For clean-HD/map-style, the frames land straight from `normalize_frames` with no palette reduction and no grid snap. Wire into the three land paths (all shell): animation headless `run_animation` (`ai.rs:953-971`, after `normalize_frames`, behind the gate); studio Land `integrate_picked` (`app.rs:2970`, before frames hit the timeline, seed the sprite palette from the returned Palette when pixel); single-sprite variant land (`cockpit.rs` from_image paths) — decode, gate, `finish_frame` if pixel, re-encode, store finished bytes, populate `SheetVariant.extracted_palette` (`reference_sheets.rs:538`) at land time so `approval.rs:155 ensure_extracted_palette` becomes a no-op.
4. Defaults: `FinishOptions::for_canvas(w,h)` (Extract, 32 colors, 5-bit matching extraction's default) and `with_palette(grid, palette)` for the anchor-palette path. Decode/encode stays in shell. CPU-only over small post-grid buffers (bounded by target_grid, not the 8K canvas); move to `spawn_blocking` if a large studio batch hitches (`studio.rs:2451` already notes this for normalize).

**Target files.**
- `core/src/transforms/finisher.rs`
- `core/src/transforms/mod.rs`
- `core/src/color/ops.rs`
- `core/src/color/extraction.rs`
- `shell/src/palette_panel.rs`
- `shell/src/ai.rs`
- `shell/src/app.rs`
- `shell/src/cockpit.rs`

**API sketch.**
```rust
// core/src/transforms/finisher.rs (new, pure core, style-agnostic)
use crate::canvas::buffer::PixelBuffer;
use crate::project::palette::Palette;
use super::error::Result;

#[derive(Clone, Debug)]
pub enum PaletteSource {
    Extract { max_colors: usize, quantize_bits: u8 },
    Fixed(Palette),
    None,
}

#[derive(Clone, Debug)]
pub struct FinishOptions {
    /// Low pixel grid to downscale to. None = palette-only finish.
    pub target_grid: Option<(u32, u32)>,
    pub palette: PaletteSource,
    pub alpha_threshold: u8,
}

impl FinishOptions {
    #[must_use] pub fn for_canvas(width: u32, height: u32) -> Self; // Extract, 32 colors, 5-bit
    #[must_use] pub fn with_palette(grid: (u32, u32), palette: Palette) -> Self;
}

#[derive(Clone, Debug)]
pub struct FinishedFrame { pub buffer: PixelBuffer, pub palette: Palette }

/// Downscale (integer divisor when exact, else nearest) then snap to palette.
pub fn finish_frame(buf: &PixelBuffer, opts: &FinishOptions) -> Result<FinishedFrame>;
pub fn finish_frames(frames: &[PixelBuffer], opts: &FinishOptions) -> Result<Vec<FinishedFrame>>;

/// Lifted from shell verbatim. Empty palette is a no-op. Bounded by pixel count.
pub fn snap_to_palette(buf: &mut PixelBuffer, palette: &Palette);

// shell — the style gate (Brief 1 surfaces ArtStyleKind on the payload)
// if kind.is_pixel() { finish_frames(...); } else { /* land normalized frames as-is */ }

// core/src/transforms/mod.rs
pub mod finisher;
pub use finisher::{FinishOptions, FinishedFrame, PaletteSource, finish_frame, finish_frames};
```

**Test plan.** Core (rstest): `finish_frame` downscales (256×256 with a 4×4 block pattern, target 64×64 → 64×64, exact-divisor path, a known block color survives); non-integer target falls back to nearest; palette snap reduces a red ramp to ≤8 colors, every opaque pixel ∈ the returned palette; Fixed path snaps to supplied colors only; `None` downscales but leaves the color set unchanged; alpha-0 pixels untouched (ported from the existing quantize test); empty palette / empty frame are no-ops. Proptest: every opaque output pixel's color ∈ the returned palette (snap closure); output dims == target_grid when set. insta: snapshot the returned Palette hex list (lock ordering). image-compare: a soft-gradient fixture through `for_canvas(64,64)` vs a committed expected sprite. Shell (the gate): `pixel_style_runs_finisher` (a pixel-class kind calls `finish_frame`, frames land snapped); `clean_hd_skips_finisher` (a clean-HD kind lands `normalize_frames` output verbatim — same byte count, no palette reduction); keep existing palette_panel tests green after the lift (re-export shim); a cockpit test that pixel-class from_image-fed bytes land with a non-empty `extracted_palette` and clean-HD bytes do not.

**Risks.** Wrong gate — if the gate reads the wrong style (e.g. a stale picker value) a clean-HD sprite gets crushed to a low grid; resolve the kind from the same payload the verb returns (Brief 1), not a UI field that may have changed since generation; covered by the `clean_hd_skips_finisher` test. Double-finishing — single-sprite land + approval both touch `extracted_palette`; mitigated because `approval.rs:155` early-returns when non-empty, so land must populate it and approval becomes a no-op; verify the idempotence test. Over-quantizing anchors — default the anchor/single-sprite pixel path to `Fixed(anchor.extracted_palette)` when present, Extract only as fallback. Divergence from the literal forge port (nearest, not Lanczos) — document the rationale so a reviewer doesn't "fix" it back. Grid choice — for animation, frames are canvas-sized so `target_grid = job.canvas` is safe; single sprites take the grid from the intended canvas, not a hardcoded constant. Perf — extraction builds a HashMap per frame, bounded by post-downscale pixel count; move to `spawn_blocking` if a batch hitches. no-unwrap/thiserror — finisher returns `transforms::error::Result`; shell decode/encode uses the surrounding land paths' error type; the finisher is sync, no lock across await.

**Effort.** M.

**Dependencies.** Depends on Brief 1 (the `ArtStyleKind` gate). Reuses `scale_integer_down`/`scale_nearest` (`transforms/scale.rs`), `extract_palette_from_rgba8` (`color/extraction.rs:107`), `nearest_color_index` (`color/ops.rs:16`), `Palette::from_colors`/`nearest_index`, and the `quantize_buffer` body to lift. Touches the normalize land paths and the `extracted_palette` field already consumed by `ai/src/plugin/anchor.rs:130`. Runs AFTER normalize on the animation/studio paths. No new crate (`color_quant` is a transitive dep but the frequency extractor suffices). Load `pixhaus-palette`, `pixhaus-color-quant`.

### Brief 9 — Static grid-sheet animation (i2v-optional) via gpt-image-2 / FAL Flux

**Summary.** Make the FAL image-to-video model optional. Today the only producer of animation frames is `generate_clip` (`shell/src/ai.rs:868`), the single function that issues an `ImageToVideoRequest` and decodes a clip into `Vec<VideoFrame>`. Everything downstream of it — `push_clip_candidate` (`app.rs:2697`), loop detection, frame picking, `compute_normalize`/`integrate_picked`, cascade-edge recording — consumes a plain `Vec<VideoFrame>` with zero knowledge of the source. The user-video import path (`import_video_clip`/`on_video_imported`, `app.rs:2736-2795`) already proves the tail accepts externally-produced frames. Add a *static* producer that mirrors video import: ask an image backend for one solid-magenta `rows × cols` grid sheet conditioned on the anchor, slice the cells into ordered `VideoFrame`s, and hand them to `push_clip_candidate`. From there Clip→Pick→Normalize→Land run unchanged. The generation-mode choice lives at the Motion stage as a `GenMode` enum (`Animated` = i2v, `Static` = sheet grid), not as a branch inside `generate_clip` — that function carries i2v-only state (model id, `num_frames*4`, the durable job queue, the cancel token) a static run does not have.

**Motivation.** Forge learning (chroma-key-magenta-bg + grid slice + shared-scale): the forge has no video model at all — it generates a static magenta grid sheet and slices it, leaning on the deterministic processor for every geometry decision. That is exactly what v2 can do today with `gpt-image-2` or FAL Flux plus `normalize_frames` (which already does chroma-key + scale-correct + repad on any `Vec<PixelBuffer>`). The payoff: animation without a video model and without per-clip video API cost, and a path that works when no i2v backend key is configured. The one genuinely missing primitive is a grid slicer — `core` has the public `transforms::crop` (`resize.rs:121`, re-exported at `transforms/mod.rs:51`) but no `slice_grid`. The trade-off is honest: i2v interpolates true motion; a sliced sheet is only as good as the model's frame-to-frame consistency (the bulk of the forge's guardrails exist for exactly this), so the existing `NormalizeReport` drift/scale/seam warnings are the only motion-quality signal the static path has — surface them prominently.

**Design.** One thin core primitive; an optional thin `ai` structure/verb; a new shell producer mirroring video import; a mode switch at the Motion stage. The Pick/Normalize/Land tail, `normalize.rs`, `integrate_frames_undoable`, the cascade-edge recording (`record_cascade_edge`, `shell/src/anim_set.rs:926`, called from the shared Land path at `app.rs:3008`), and both backends need zero changes.

1. core (`core/src/transforms/sheet.rs`, new): `slice_grid(sheet, rows, cols) -> Result<Vec<PixelBuffer>>` and `slice_rects(sheet, &[Rect]) -> Result<Vec<PixelBuffer>>`. Implement as a loop calling the existing public `transforms::crop` — net-new code is thin (no need to promote the private `normalize.rs` crop; a public crop already exists). `slice_rects` handles non-uniform cells when a Structure declares gutters or a palette swatch. Unit-test against a constructed multi-cell buffer.
2. ai (`compose/builtins.rs`, optional but cleaner): add a Paneled "animation grid" Structure with ordered View-slot cells on a magenta-friendly canvas, and a thin `generate_animation_sheet` verb wrapping the existing compose → `ImageGenRequest` path (a near-copy of `GenerateReferenceSheetVerb`) that returns the ordered cell rects as its composition. Prefer this over overloading `generate_reference_sheet` so animation landing/cascade semantics stay separate from character-sheet approval semantics.
3. shell (`ai.rs`): `StaticSheetJob { canvas, anchor_png, action_prompt, rows, cols, frames, seed }` and `spawn_static_sheet(...)` mirroring `spawn_clip`/`spawn_reference_sheet`. Internally build one `ImageGenRequest` (anchor as `reference_images`, prompt requiring a solid `#FF00FF` background plus a `rows×cols` grid, sheet-sized canvas), `invoke_fat(IMAGE_GENERATION)`, decode the PNG to a `PixelBuffer`, call `slice_grid`/`slice_rects`, convert each cell to a `VideoFrame` (timestamp `i*1000/fps`), and send `ShellMsg::StaticSheetReady`. Run the decode + slice on `spawn_blocking` like `pick_and_decode_video` already does.
4. shell (`app.rs`): `start_static_sheet` + `on_static_sheet_ready` mirroring `import_video_clip`/`on_video_imported`, both calling `push_clip_candidate(clip_png_or_empty, "image/png", frames, action_prompt, fps, seed, parent)`. Skip the i2v `AnimJobQueue` record (no remote video clip to persist), or add a lightweight `record_import`-style entry tagged `model = "sheet"`. New `ShellMsg::StaticSheetReady`/`StaticSheetFailed`.
5. shell (`studio.rs`): `enum GenMode { Animated, Static }` (serde, persisted in `StudioSession` beside `i2v_model`) and a `selectable_value` in the Motion inspector. The Generate button dispatches `match mode { Animated => start_clip(), Static => start_static_sheet() }`. For Static, hide the I2vModel picker and show rows/cols (or a grid preset) instead; keep fps/frames/seed (they map to cell count and playback). Cancel stays i2v-only.

**Target files.**
- `core/src/transforms/sheet.rs`
- `core/src/transforms/mod.rs`
- `ai/src/compose/builtins.rs`
- `ai/src/verbs/` (new `generate_animation_sheet`, optional)
- `shell/src/ai.rs`
- `shell/src/app.rs`
- `shell/src/studio.rs`

**API sketch.**
```rust
// core/src/transforms/sheet.rs (new, pure core)
use crate::canvas::PixelBuffer;
use super::error::Result;
use super::resize::crop; // the existing public crop — loop over it, no new primitive

/// Cuts a sheet into rows*cols equal cells, row-major. Floor-divided; trailing
/// remainder dropped so cells stay uniform. Deterministic.
/// # Errors
/// `Error::EmptyBuffer` when the sheet is 0x0 or rows/cols is 0.
pub fn slice_grid(sheet: &PixelBuffer, rows: u32, cols: u32) -> Result<Vec<PixelBuffer>>;

/// Cuts a sheet at explicit (clamped) cell rectangles — for Structures with
/// gutters, labels, or a palette swatch the naive grid would mis-cut.
pub fn slice_rects(sheet: &PixelBuffer, rects: &[(u32, u32, u32, u32)]) -> Result<Vec<PixelBuffer>>;

// shell/src/ai.rs
pub struct StaticSheetJob {
    pub canvas: (u32, u32),
    pub anchor_png: Vec<u8>,
    pub action_prompt: String,
    pub rows: u32,
    pub cols: u32,
    pub frames: u32,
    pub seed: Option<u64>,
}
/// Generate one magenta grid sheet, decode, slice to frames. Off the UI thread.
pub fn spawn_static_sheet(/* runtime, job, tx, ctx */);

// shell/src/studio.rs
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum GenMode { Animated, Static } // persisted in StudioSession

// shell/src/app.rs
// ShellMsg::StaticSheetReady { epoch, frames: Vec<VideoFrame>, action: String, fps: u32, seed: Option<u64> }
// ShellMsg::StaticSheetFailed { epoch, error: String }
```

**Test plan.** Core (rstest + proptest + image-compare): `slice_grid` on a synthetic 2×2 magenta sheet with one opaque square per quadrant → four cells in row-major order, each the correct sub-rect; `slice_grid` errors on 0×0 / zero rows; non-divisible dims drop the trailing remainder so cells stay uniform; `slice_rects` cuts non-uniform cells correctly and clamps an overhanging rect; proptest — random sheet sizes and `rows`/`cols` never panic and yield `rows*cols` cells each of the floor-divided size; image-compare a sliced fixture against committed cell goldens. Shell: `static_sheet_frames_reach_clip_candidate` (a `spawn_static_sheet` stub returning a known sheet lands `rows*cols` frames via `push_clip_candidate`, then the existing Pick/Normalize/Land tail runs — assert frame count and that no i2v `AnimJobQueue` record was created); `gen_mode_static_routes_to_static_sheet` (the Generate dispatch picks the static producer); `gen_mode_persists` (StudioSession round-trip). The magenta sheet keys through `NormalizeOptions::square` (`ChromaKey::magenta` default) so frames land pre-stripped — assert with the existing normalize tests.

**Risks.** No true interframe motion — quality depends entirely on the model keeping subject scale, position, and silhouette consistent across cells; lean on the `NormalizeReport` drift/scale/seam warnings (Brief 6) and surface them prominently for static sheets. gpt-image-2 clamps every dimension to [1024,2048] rounded to /16 (`openai.rs:510-529`), capping aspect ratio at 2:1 — a wide single-row N-cell strip (>2:1) is distorted/clamped, NOT rendered natively; constrain the static sheet to a roughly-square `rows × cols` grid on gpt-image-2, OR route wide strips to FAL Flux (`fal.rs:462-480` sets `image_size` verbatim with no aspect clamp); cell math must use the post-fit size (`fit_images_to_request` already downscales). Reusing `start_clip` wholesale would create a misleading i2v job record and a phantom cancel token — use the separate lighter producer. The naive grid mis-cuts when the model adds gutters/labels/a palette swatch — use `slice_rects` with the Structure's cell rects, not `slice_grid`, for paneled animation structures.

**Effort.** M.

**Dependencies.** Built on shipped surfaces — the source-blind Clip→Pick→Normalize→Land tail, `push_clip_candidate`, the video-import shape, `transforms::crop`, `normalize_frames`, and the image backends. Relies on Brief 3 (containment prose) and Brief 4 (two-pass key) for clean cells, and pairs with Brief 11 (multi-row grids) — the `slice_grid`/`slice_rects` primitive and Brief 11's `grid_rects` are the inverse of each other and should share the cell math. No bedrock blocker. The `generate_animation_sheet` verb is optional; the path works by reusing the existing image-gen request shape from `generate_clip`'s self-seed arm (`ai.rs:874-895`).

### Brief 10 — Per-frame QC + provenance record persisted on landed animations

**Summary.** v2 computes a `NormalizeReport` (drift/scale/seam) and per-frame `FrameMetrics` during the studio's Normalize review, then discards both at Land (`app.rs:2970-3013` `integrate_picked` consumes only `result.frames`; `result.report`/`result.metrics`/the cache are dropped). The landed artifact carries no QC payload — neither `FrameTag` nor `Animation` records normalize settings or per-frame QC, and `AnimJobRecord` holds generation provenance only, finalized at decode time (`app.rs:1173`), before normalize runs. There is no connected-component analysis. This stream (1) adds connected-component + edge-touch analysis to core's normalize module and widens `FrameMetrics`/`NormalizeReport`, (2) defines a serializable `AnimationQc` capturing per-frame QC plus the key/threshold/canvas settings used, and (3) persists it at Land — onto the `Animation` entry (durable in the project) and back onto the `AnimJobRecord` sidecar (durable across restart). Every landed loop then carries a machine-readable record an artist or an automated loop reads to decide reprocess-with-tighter-flags vs regenerate.

**Motivation.** Forge learning (pipeline-meta-json + reprocess-then-regenerate-loop): a pipeline that iterates cheaply must persist how each artifact was processed so a follow-up pass can re-run post-processing with adjusted flags before an expensive re-generation. v2 already does the hard measurement (NormalizeReport + FrameMetrics) but the data is one-shot — gone the moment the loop lands. An artist who sees a 6px drift has no record of which key color, tolerance, alpha threshold, or canvas the frames came from, so "rerun with tighter flags" is guesswork and every fix is a full regenerate. Worse, there is no component count, so the most common defect — a stray keyed-out speck or a body that split into two blobs — is invisible. Persisting component count, the largest component's area/bbox, the crop bbox, output size, an edge_touch flag, and the exact settings turns the QC loop from observe-once into iterate. This serves both animation paths: i2v clips and static sheets land through the same Land path.

**Design.** Per the v2 rule: pixel QC (component labeling, edge-touch, metrics) in core's normalize module; persistence shapes in core's project model; capture-at-Land in shell. No ai-crate work.

1. `core/src/transforms/normalize.rs`: `connected_components(buf, alpha_threshold) -> ComponentStats` — single-pass union-find or two-pass scanline over opaque pixels, 4-connectivity (8 behind a const), returning `num_components`, the largest component's area and bbox, and `edge_touch` (any opaque pixel on the 1px border). Iterative, not recursive (8K), bounded by the frame. Widen `FrameMetrics` with `num_components`/`largest_component_area`/`largest_component_bbox`/`edge_touch` (populate in a new `measure_qc` that calls `measure` then `connected_components`, keeping `measure` cheap for the seam path). Widen `NormalizeReport` with `max_components`/`any_edge_touch` and push warnings so the studio warnings list surfaces them. New Vec/Option fields get `#[serde(default)]`.
2. `core/src/project/qc.rs` (new): `AnimationQc { settings: QcSettings, report: NormalizeReportSummary, frames: Vec<FrameQc> }`, all serde with `#[serde(default)]`. `QcSettings` captures the reproducible inputs (key_color, key_tolerance, alpha_threshold, canvas, bottom_margin, reference_height, remove_on_land). `From<&FrameMetrics>`/`From<&NormalizeReport>` for mapping. Attach `#[serde(default, skip_serializing_if = "Option::is_none")] pub qc: Option<AnimationQc>` to `Animation` (`animation.rs:16`) — the durable home in the archive.
3. Shell: `commands.rs::integrate_frames_undoable` (`:379`) gains a `qc: Option<AnimationQc>` param; thread it into `doc.integrate_frames` so it is part of the same `SpriteBufferEdit` undo snapshot (not a post-push mutation). `app.rs::integrate_picked` builds `AnimationQc` from the `result.report`/`result.metrics` it holds plus the `NormalizeOptions` it built (`:2944-2953`) BEFORE dropping them; when `east_from_west` flips frames (`:2995`), mirror the per-frame bbox/center_x (`x' = canvas_w - x - w`). `anim_jobs.rs`: `finalize_qc(id, qc)` sets a new `#[serde(default)] pub qc: Option<AnimationQc>` and re-persists the sidecar at Land (after finish_done — the decode-time finalize is too early); if the originating job is gone, skip the sidecar (the Animation copy is the source of truth). Determinism: `connected_components` must be order-stable for snapshots.

**Target files.**
- `core/src/transforms/normalize.rs`
- `core/src/project/qc.rs`
- `core/src/project/animation.rs`
- `core/src/project/mod.rs`
- `shell/src/anim_jobs.rs`
- `shell/src/commands.rs`
- `shell/src/document.rs`
- `shell/src/app.rs`
- `shell/src/studio.rs`

**API sketch.**
```rust
// core/src/transforms/normalize.rs
pub struct ComponentStats {
    pub num_components: u32,
    pub largest_component_area: u32,
    pub largest_component_bbox: Option<(u32, u32, u32, u32)>, // x, y, w, h
    pub edge_touch: bool,
}
#[must_use]
pub fn connected_components(buf: &PixelBuffer, alpha_threshold: u8) -> ComponentStats;
#[must_use]
pub fn measure_qc(buf: &PixelBuffer, alpha_threshold: u8) -> FrameMetrics;
// FrameMetrics gains num_components/largest_component_area/largest_component_bbox/edge_touch
// NormalizeReport gains max_components/any_edge_touch

// core/src/project/qc.rs
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QcSettings {
    pub key_color: Rgba, pub key_tolerance: u8, pub alpha_threshold: u8,
    pub canvas: (u32, u32), pub bottom_margin: u32,
    pub reference_height: u32, pub remove_on_land: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameQc {
    pub bbox: (u32, u32, u32, u32), pub center_x: u32, pub foot_baseline_y: u32,
    pub empty: bool, pub num_components: u32, pub largest_component_area: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub largest_component_bbox: Option<(u32, u32, u32, u32)>,
    pub edge_touch: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NormalizeReportSummary {
    pub baseline_drift_px: u32, pub scale_match_pct: u32, pub seam: SeamMatch,
    pub reference_height: u32, pub max_components: u32, pub any_edge_touch: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimationQc { pub settings: QcSettings, pub report: NormalizeReportSummary, pub frames: Vec<FrameQc> }

// Animation gains: #[serde(default, skip_serializing_if = "Option::is_none")] pub qc: Option<AnimationQc>

// shell/src/commands.rs — new trailing param (None at non-reviewed sites)
pub fn integrate_frames_undoable(
    editor: &mut EditorState, doc: &mut DocumentStore, frames: Vec<PixelBuffer>,
    frame_duration_ms: u32, name: &str, loop_direction: LoopDirection,
    qc: Option<AnimationQc>,
) -> Option<FrameRange>;

// shell/src/anim_jobs.rs — AnimJobRecord gains #[serde(default)] pub qc: Option<AnimationQc>
pub(crate) fn finalize_qc(&mut self, id: u64, qc: AnimationQc);
```

**Test plan.** Core (rstest + proptest + insta): `connected_components` — single blob → 1 component, area w·h, full bbox, edge_touch true for a frame-filler; two blobs → 2 and the larger picked; a 1px diagonal gap stays 2 under 4-connectivity; transparent → 0/None/false; a centered subject with a border → edge_touch false, flush to row 0 → true. Proptest — `num_components <= opaque_count`, `largest_area <= total`, `largest_area == 0 iff num_components == 0`, edge_touch implies a border opaque pixel. Determinism — same buffer twice → identical stats. `measure_qc` agrees with `measure` on shared fields. NormalizeReport widening — extend the baseline test to assert `max_components`/`any_edge_touch`; a two-blob fixture fires the "disconnected components" warning. qc.rs — insta snapshot of populated JSON, a round_trip test, a forward-compat test deserializing JSON missing the new Option/Vec fields; `Animation::round_trip` with `Some(qc)` and the `None` case still identical. Shell — `integrate_frames_undoable` with `Some(qc)` lands `Animation.qc == Some(qc)`, undo removes it; `finalize_qc` on a Done job re-persists and reloads from the temp sidecar; on a missing id is a no-op; an older sidecar without qc still deserializes; east-flip test — a flipped frame's `FrameQc` bbox/center_x is the horizontal mirror.

**Risks.** Undo correctness — write qc inside the same `SpriteBufferEdit` (thread into `doc.integrate_frames`, not a post-push mutation), or undo leaves a dangling qc. East-flip skew — mirror per-frame coords when `east_flip_on_land()`, covered by a test. Sidecar timing — `finish_done` runs at decode, before normalize; the sidecar qc must be a separate `finalize_qc` at Land and tolerate a pruned job (skip; Animation.qc is the source of truth). CC cost on large picks — union-find is near-linear, bounded by the small pick frames; `measure_qc` must not run in the per-frame seam loop (keep `measure` for `seam_match`). Scope — do NOT build the automated reprocess loop here; this only records the data. 4- vs 8-connectivity is a judgment call — pick 4, document it, gate 8 behind a const.

**Effort.** M.

**Dependencies.** Depends on the existing normalize pass and the studio Land path — all present, no bedrock blocker. Synergistic with and ordered after the component/edge-touch pixel-fix gaps (Briefs 5, 6): the `connected_components` + `edge_touch` primitives are shared — coordinate the signature so it is shared, not duplicated. No `io` crate dependency — the record rides on the in-memory `Animation` now and serializes for free when `.pixhaus` save lands; the `anim_jobs` sidecar is the interim durable-across-restart home.

### Brief 11 — Default body subjects to compact multi-row grids; classify props before picking grid shape

**Summary.** v2's character reference-sheet structure lays its five turnaround views in a raw single horizontal row (`ai/src/compose/builtins.rs:66-144`: views at x=i·200, y=0, prose "horizontal strip across the top") — exactly the raw 1xN body strip the forge warns against, which drives horizontal scale-drift and inconsistent crop. Items already use a 2x2 grid and tilesets stacked rows, so the fix is partial. This stream (a) re-lays the character body views as a compact 3x2/2x3 grid, (b) adds a reusable auto-grid primitive (a `grid_rects` helper plus a frame-count→rows×cols mapping: 4→2x2, 6→2x3, 9→3x3, 16→4x4) so future structures and the static-sheet path (Brief 9) share one grid engine, and (c) adds a prop classification layer (compact/wide/tall) plus a non-blocking guard in the cockpit structure picker so a wide or collision-heavy object is not forced into square cells. The reference_sheet verb stays template-free; geometry continues to derive from the resolved Structure through `build_composition`.

**Motivation.** Forge learning, three techniques. multirow-grid-over-singlerow: a 1xN strip gives the model no vertical anchor, so each cell drifts in scale and the right edge crops; a compact grid gives a 2D anchor. prop-pack-classification: a sheet shape must fit the subject's aspect — a wide vehicle or tall staff clipped into a square cell is a guaranteed bad crop. smallest-useful-output: map the frame count to the smallest compact grid. Verified: `builtins.rs:66-92` places 5 views at x=i·200, y=0 across a 1024×1536 canvas — a single 1000px row, prose at `:81-85` literally "horizontal strip across the top, left-aligned". `item()` (`:146-158`) already grids 2x2 and `tileset()` (`:191-232`) stacks rows, proving compact layout is viable. There is no classification: the cockpit picks via a flat ComboBox (`cockpit.rs:248-263`) backed by `ai::structure_options` (`ai.rs:68-77`), zero compact/wide/tall branching, no frame-count→grid mapping. The geometry path is sound and centralized (Structure → `build_composition` → `SheetComposition` → `SheetVariantOutput.composition`), so the fix lands at the source without touching the verb or QC layer.

**Design.** Prompt/asset-plan/structure-shape logic in `ai`; the data-model primitive in `core`; only the picker guard in `shell`. Nothing touches QC — this only changes how panel rectangles are authored, which flows downstream unchanged.

1. core (`composition/structure.rs`): a pure `grid_rects(region, rows, cols) -> Vec<PanelRect>` tiling a region into equal cells row-major (floor-divided, trailing remainder dropped so cells stay uniform), and `compact_grid_shape(n) -> (rows, cols)` picking the squarest factor pair whose product ≥ n, columns ≥ rows by at most one (4→2x2, 6→2x3, 9→3x3, 16→4x4, 5→2x3, 7→3x3). Both `#[must_use]`, pure, no I/O, clamp n ≥ 1. This is the inverse of Brief 9's `slice_grid` — share the cell math (one helper, used to author rects here and to cut them there).
2. ai (`builtins.rs`): rewrite `character()` to lay the 5 views via `grid_rects` as a 3x2 (6 cells, last empty) over the top region, each cell ~341×480 instead of 200×480, and rewrite the shared prose to "a compact grid of turnaround views, N columns by M rows, each {panel_w}x{panel_h}, consistent scale across all cells". Keep the canvas 1024×1536 and the 12-panel count (5 views + 3 expr + 1 palette + 2 callout + 1 outfit) so `SheetComposition.views.len() == 5` holds (the verb test at `reference_sheet/mod.rs:836`). Add `SubjectClass { Compact, Wide, Tall }` and `classify(subject) -> SubjectClass` (keyword sets: wide/collision → vehicle/car/ship/banner/weapon-rack/vista/landscape; tall → staff/spear/tower/totem/column; default Compact), plus `recommended_structure(class) -> StructureId`. Expose `ai::grid` re-exporting the mapping for the shell.
3. ai: two new built-in structures `wide_object()` (a 2-row grid of 2:1 wide cells) and optionally `tall_object()`, registered in `BuiltinLibrary::load()` alongside the five, so the classifier has somewhere to point.
4. shell (`cockpit.rs`/`ai.rs`): in `cockpit_inputs`, after the subject TextEdit, call `ai::classify` and, when the recommendation differs from `self.ck_structure`, render a non-blocking hint row ("This reads as a wide object — Vehicle sheet fits better [Use it]") via `crate::icons`; clicking sets the structure and `ck_dirty`. Never auto-override the artist — suggest only. The combo stays; the guard sits above it.

`classify` is pure over the subject string (case-folded), so the cockpit preview and the verb stay reproducible. The verb is untouched.

**Target files.**
- `core/src/project/library/composition/structure.rs`
- `ai/src/compose/builtins.rs`
- `ai/src/compose/mod.rs`
- `shell/src/cockpit.rs`
- `shell/src/ai.rs`
- `ai/src/verbs/reference_sheet/mod.rs`

**API sketch.**
```rust
// core/src/project/library/composition/structure.rs
/// Smallest compact (rows, cols) holding `n`, columns >= rows.
/// 4->(2,2), 6->(2,3), 9->(3,3), 16->(4,4), 5->(2,3), 7->(3,3).
#[must_use]
pub fn compact_grid_shape(n: u32) -> (u32, u32);

/// Tiles `region` into rows x cols equal cells, row-major. Floor-divided;
/// trailing remainder dropped so cells stay uniform. Inverse of slice_grid.
#[must_use]
pub fn grid_rects(region: PanelRect, rows: u32, cols: u32) -> Vec<PanelRect>;

// ai/src/compose/builtins.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubjectClass { Compact, Wide, Tall }
#[must_use]
pub fn classify(subject: &str) -> SubjectClass;
#[must_use]
pub fn recommended_structure(class: SubjectClass) -> StructureId;
fn wide_object() -> Structure; // 2-row grid of 2:1 wide cells
fn tall_object() -> Structure;

// shell/src/ai.rs
#[must_use]
pub fn suggested_structure_id(subject: &str) -> String;
```

**Test plan.** Core (rstest + proptest): `compact_grid_shape` cases (1→(1,1), 4→(2,2), 6→(2,3), 9→(3,3), 16→(4,4), 5→(2,3), 7→(3,3)) asserting rows·cols ≥ n and cols-rows ≤ 1; proptest on `grid_rects` for n in 1..=64 and any region — every rect lies fully inside `region` (the core anti-clip guarantee), rects non-overlapping, count == rows·cols; a uniform-cell test (all cells share width/height). ai (rstest + insta + verb harness): `classify` table ("a red sports car"→Wide, "a tall wizard staff"→Tall, "a small knight"→Compact, case-insensitive); `recommended_structure` mapping; extend `character_geometry_matches_legacy` (`builtins.rs:470`) to keep 12 panels and 5 views but assert the 5 view rects span more than one y (multi-row) and every rect is inside 1024×1536; insta snapshot of `compose_layout(character)` locking the new prose and asserting "horizontal strip" is gone; the existing `variants_carry_composition_panels` verb test (`:818-840`) still passes unchanged; update `loads_structures_and_default_style` for the new wide/tall built-ins. shell (rstest): `suggested_structure_id` returns the wide id for "a red car", the character id for "a knight"; a focused test that the guard never mutates `ck_structure` without the [Use it] click (test the extracted state-transition function, not the egui paint).

**Risks.** Migration-equivalence tests are load-bearing — `character_migration_preserves_all_layout_phrases` (`:521-534`) and `reference_sheet/mod.rs:836` assert exact legacy prose ("each 200 pixels wide, 480 pixels tall") and 5-view geometry. The grid rewrite intentionally breaks the prose and the 200×480 dims — update those tests in the same PR with the new compact-grid prose, called out as a deliberate layout migration. Keep view count at 5 so the verb test holds. The keyword classifier will misclassify — advisory-only, never auto-override, easy to dismiss; do not gate generation on it. Adding built-ins shifts the structure count and ComboBox order — update `structure_options` sort and the `loads_structures` test together. Cell dimensions change the `{panel_w}/{panel_h}` and the SheetComposition rects the gallery slices from — confirm the clip-review/gallery overlay still renders (no code change needed). Coordinate with Brief 1 and Brief 3 — all three touch the `character()` prose fragment. Scope discipline — resist adding chroma-key/slice/edge-touch QC here; this only authors rectangles and prose.

**Effort.** M.

**Dependencies.** None hard. Builds on the landed composition data model, the compose resolver, and the reference_sheet verb. Shares the grid cell math with Brief 9. Coordinates with (does not block on) the AI Studio redesign — the cockpit guard should be portable into that layout. No bedrock blocked or blocking.

### Brief 12 — Bit demo project + prompt pack: all movements and actions

**Summary.** v2 boots one empty sprite and offers New-sprite/Settings/Quit in the File menu (`app.rs:1034`, `:3099-3113`). There is no demo project and no `io` crate to save or load a `.pixhaus` file. The mascot Bit — the CRT-head robot, with companions Byte and Floppy — already exists in the built-in *prompts* (`builtins.rs:305-394`). Build a Bit demo project in code — a `Custom("Character")` entity named Bit with sprites for each movement and action — seed it on first run and behind a File-menu action, and ship a prompt pack of one composed prompt per action in the pixel-art default style. The demo is the first-run showcase that exercises generate + style + export + animate end to end. Because there is no `io` crate, the demo is constructed at runtime through the existing `DocumentStore` + `core::Project` model, not loaded from a file.

**Motivation.** Forge learning (smallest-useful-output as a showcase): a worked example that runs the whole pipeline teaches the tool faster than docs. v2 already has the mascot identity (Bit) and the prompt-pack machinery (built-in `PromptTemplate`s, `prompt.rs`; project `pixstyle.rs`) but no demo project. The payoff is a first-run window that already holds a recognizable character with idle/walk/run/attack/jump cells the user can regenerate, restyle (pixel-art default, then try clean-HD), animate (i2v or the static sheet path), and export — the four capabilities the rest of this roadmap builds.

**Design.** Demo construction in a new `shell/src/demo.rs`; seeding in `app.rs`; the prompt pack in `ai/src/compose/builtins.rs`. The demo must seed through the existing model, not a freshly-invented `Project`.

1. The shape already exists. `core::Project` is `EntityKind::Custom("Character")` + `EntityContent::Sprites { states }` + `NamedSprite { state_name }` + `ActiveTarget::State` (`core/src/project/mod.rs:110-216`; the test at `:230-259` builds a Custom "Character" named "Hero" with an "idle" state). The shell wraps it as `DocumentStore { pub project: Project }` (`document.rs:62`). Do not invent a new struct — populate this one.
2. A bare `Project` is insufficient: pixel bytes live in `DocumentStore.pixel_buffers` keyed by `PixelBufferId`, and ids must come from the shared `next_id` allocator (`alloc_id`, `document.rs:127`). So `build_bit_demo` is a `DocumentStore` method (or takes `&mut DocumentStore`) that seeds `pixel_buffers` and respects the allocator — not a free function returning a lone `Project`.
3. Respect the shell's one-state-per-entity grain. Every shell entity-creation path hard-codes a single state named "primary" (`document.rs:170`, `:335`), and `library_rows`/`push_library_level` flatten each state into its own sprite row labelled by `sprite.name`, ignoring `state_name` (`document.rs:727-742`); no UI picks states within an entity. So model each Bit action as either its own entity, or — cleaner — one Bit sprite holding all actions via `frames` + `frame_tags` (`sprite.rs:44`, `:59-62`), which the existing timeline panel already drives. Prefer the frame-tags shape: one Bit sprite, one tag per action, the timeline already renders it.
4. Seed on first run in `App::new` (when no project was restored) and add a File-menu "Open Bit demo" action next to New-sprite (`app.rs:3099-3113`) so it is re-loadable.
5. Prompt pack (`builtins.rs example_prompts`): add one `PromptTemplate` per Bit action, each `default_structure` pointing at the right structure and composing in the pixel-art default style. Per-action grids need no new Rust type — `StructureOutput::Paneled { canvas, panels }` already expresses arbitrary grids via `PanelRect` (the `item()` built-in is a 2x2 at `builtins.rs:146-186`), so the action grids are new *data records*, reusing Brief 11's `grid_rects`/`compact_grid_shape` to author the cells.

The full Bit action/movement set: idle, walk, run, jump, fall, attack (a melee beat), hurt, and a turnaround (south/west/north + east-as-flip). Movements (idle/walk/run/jump/fall) map to looping `frame_tags`; actions (attack/hurt) are one-shot tags; the turnaround feeds the directional anchor cascade. Note `AnimationKind` only has Idle/Walk/Attack (`reference_sheets.rs:270-277`) — the extra movements ride as `frame_tags` on the sprite, not as new `AnimationKind` variants (defer any `AnimationKind` change).

**Target files.**
- `shell/src/demo.rs`
- `shell/src/document.rs`
- `shell/src/app.rs`
- `ai/src/compose/builtins.rs`

**API sketch.**
```rust
// shell/src/demo.rs (new)
use crate::document::DocumentStore;

/// Seeds the running document with the Bit mascot: one Custom("Character")
/// entity named "Bit", one sprite carrying every action as a frame tag, and
/// pixel buffers allocated through the document's id allocator. Built in code —
/// there is no io crate, so the demo is constructed, not loaded.
pub(crate) fn build_bit_demo(doc: &mut DocumentStore);

/// The movement/action set the demo ships. Movements loop; actions are one-shot.
pub(crate) const BIT_ACTIONS: &[(&str, FrameTagKind)] = &[
    ("idle", FrameTagKind::Loop), ("walk", FrameTagKind::Loop),
    ("run", FrameTagKind::Loop), ("jump", FrameTagKind::Loop),
    ("fall", FrameTagKind::Loop), ("attack", FrameTagKind::OneShot),
    ("hurt", FrameTagKind::OneShot),
];

// ai/src/compose/builtins.rs — one prompt per action in the pixel-art default
fn bit_prompts() -> Vec<PromptTemplate>; // merged into example_prompts()
```

**Test plan.** Shell (rstest): `build_bit_demo_seeds_one_bit_entity` (after the call, the document has exactly one `Custom("Character")` entity named "Bit"); `build_bit_demo_allocates_through_id_allocator` (every seeded `PixelBufferId` came from `alloc_id`, no id collisions, `next_id` advanced); `build_bit_demo_tags_every_action` (the sprite's `frame_tags` cover all `BIT_ACTIONS`, loop vs one-shot kinds correct); `build_bit_demo_pixel_buffers_present` (each tagged frame resolves to a non-empty `PixelBuffer`); a re-seed test (calling it twice from the menu replaces cleanly, no duplicate-id panic). ai (rstest + insta): `bit_prompts_cover_all_actions` (one `PromptTemplate` per action, each composes in the pixel-art default — assert the composed positive begins with the pixel baseline); `bit_prompts_point_at_valid_structures` (every `default_structure` resolves in `BuiltinLibrary::load()`); insta snapshot of the composed Bit idle prompt. No image-compare unless committed demo pixel fixtures are added (optional follow-up).

**Risks.** No `io` crate (`Cargo.toml:6`; `document.rs:9-13`) — the demo cannot be a `.pixhaus` file; build it in code and seed at runtime, and accept that it does not persist across restart until save/load lands. The one-state-per-entity grain (`document.rs:170`/`:335`/`:727-742`) fights "many states in one entity" — use the frame-tags shape (one sprite, many tags) the timeline already drives, not multiple states the UI cannot pick. Demo pixel content — either ship tiny hand-authored placeholder frames or generate them lazily; do not block first paint on an AI call. `AnimationKind` is Idle/Walk/Attack only — ride the extra movements as `frame_tags`, do not widen the enum here (defer). Seeding on first run must not clobber a restored project — gate it on "no project restored".

**Effort.** M.

**Dependencies.** Depends on the generate + style + export work it showcases (Briefs 1, 7, 9, 11) — land it last so it exercises a real pipeline rather than placeholders. Built on the existing `DocumentStore` + `core::Project` model, the `frame_tags` timeline, and the built-in prompt machinery. No bedrock blocker, but it improves materially once `.pixhaus` save/load (the `io` crate) lands, at which point the in-code demo can become a shipped project file.

## Next step

Land **Brief 1 — Art-style selection** first as the Wave-1 enabler: it is S effort, non-breaking (a serde-default `ArtStyleKind`), and it is what the finisher and the pixel prose gate on, so it unblocks the rest of the quality work cleanly. Then dispatch **Brief 7 — GUI sprite export (transparent-PNG frames, hard-masked looping GIF, packed atlas)** as the single highest-leverage win: v2 can already generate, refine, animate, normalize, and land a sprite, but it cannot deliver one from the GUI, so every other quality gain is currently locked inside the editor; it is self-contained (M effort, no bedrock dependency, no AI-pipeline coupling), it ports a concrete forge technique that exists nowhere in v2 — the hard-alpha-mask + reserved-index GIF that kills the soft-fringe halo — and it shares one code path with the headless writer so the CLI improves for free.

With the enabler and the loop-closer in place, layer the deterministic post-pass spine (Briefs 3–6, 8) and the static-sheet path (Brief 9) underneath, then the advanced QC/grid work (Briefs 10–11), and finish with the Bit demo (Brief 12) once there is a real generate→style→export→animate pipeline for it to showcase.
