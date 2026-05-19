# Prior-art synthesis — what the seven dossiers tell us to do

This is the homogeneous digest for the deep prior-art dossiers in `../research/`. Each pattern, decision, and port unit below appears in at least one dossier — usually several. Read this before opening a dossier; open the dossier only when you're implementing a specific unit and need the pseudocode, source-tree walk, or per-technique rationale.

The dossiers stay as the canonical deep reference. This file aggregates and points; it never re-derives.

Update this digest when a new dossier lands in `../research/`, when an open decision below resolves, or when a port-roadmap unit ships and changes status.

## Sources

Seven dossiers in `../research/`, landed May 14–19 in PRs #213–#219:

- [`aseprite-prior-art.md`](../research/aseprite-prior-art.md) — Aseprite document model, I/O, rendering pipeline; MIT levels 0–3 portable, EULA levels 4+ inspire-only.
- [`opentoonz-comparison.md`](../research/opentoonz-comparison.md) — production-tested algorithms (22 blend modes, morphological AA, gap-closing wand, centerline vectorization, palette pages); BSD-3.
- [`pixelorama-adoption.md`](../research/pixelorama-adoption.md) — ZIP+JSON format, sparse palette, indexed mode, cel linking, shader ports; MIT, tiered A/S/P/D adoption plan.
- [`falsprite-prior-art.md`](../research/falsprite-prior-art.md) — two-stage LLM prompts for grid sprite-sheet animation via fal.ai; MIT.
- [`grid-snap-quantize-techniques.md`](../research/grid-snap-quantize-techniques.md) — Sprite Fusion's seven-technique pixel-snap pipeline (k-means → gradient profile → step estimate → reconciliation → walker → stabilization → majority-vote downsample); MIT.
- [`sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) — anchor-first generation methodology, directional economy, seven-step normalization; MIT.
- [`project-library-research.md`](../research/project-library-research.md) — comparative survey of library/skin/variant systems across Blender, Spine, Live2D, Unity, Animate, Aseprite, Pixelorama, Procreate Dreams, Krita, Scenario, ComfyUI, Midjourney.

## Recurring patterns

Nine patterns recur across multiple dossiers. They are convergent because more than one mature tool independently chose them. A new tool can break them, but it should know what it is giving up first.

### Anchor-first canonical pose

Pick one canonical pose (typically a south-facing idle) as the single source of truth for a character. Every other direction, animation, and variant derives from that anchor. Regeneration without a fixed anchor produces silhouette drift; cascading from an anchor produces consistent variants and makes re-rolls cheap.

- Seen in: `sprite-pipeline-methodology.md` § 3 stage 2, § 4; `aseprite-prior-art.md` § 1 (linked cels), § 11 (palette); `project-library-research.md` § Spine skins, § Blender collections.
- Lands in: B2 (data model — first-class `character_anchor` entity), B9 (project library), S25 (Verb: Extend), S26 (Verb: Variant), S36 (Verb: Sketch finishing).

### Directional economy: flip before regenerate

East equals horizontally flipped west. North-east equals flipped north-west. Regenerating mirrored views from scratch produces drift; flipping is free and exact. The dossiers agree that AI verbs should consume mirror pairs as a hard rule, not an optional toggle.

- Seen in: `sprite-pipeline-methodology.md` § 4 design principles; `falsprite-prior-art.md` § 5 per-row action selection.
- Lands in: S25 (Verb: Extend — default to flip pairs, generate only the unique half), S26 (Verb: Variant), S32 (Verb: Motion-from-video).

### Sparse, link-set variants over duplication

Variants share data by reference. Pixelorama's cel linking uses link-set IDs (multiple cels point to the same image until one is edited, at which point only that cel detaches). Spine's skins override attachments without copying the skeleton. Aseprite splits Cel and CelData so identical cels share the image. Sprite-pipeline introduces character anchors with `derived_from` edges. The shape varies; the principle does not — never duplicate when you can link.

- Seen in: `pixelorama-adoption.md` § 4 (cel linking via link-set IDs); `aseprite-prior-art.md` § 1 (CelData split); `project-library-research.md` § Spine skins, § Unity prefab variants; `sprite-pipeline-methodology.md` § B2/B9.
- Lands in: B2 (data model — adopt link-set IDs, not pointer-sharing — see D-05), B9 (project library — character anchors), S26 (Verb: Variant).

### Procedural fallbacks for AI verbs

Every dossier that touches AI insists on a deterministic fallback for the same reason: AI is slow, offline-broken, and over-creative for preview work. OpenToonz's procedural inbetween (variance-rejected averaging) predates any AI inbetweening; FalSprite degrades to plain canvas composition if image-gen fails; grid-snap's seven techniques are purely classical and run instantly. Offer the procedural path first, the AI path on demand.

- Seen in: `opentoonz-comparison.md` § Stroke inbetweening; `falsprite-prior-art.md` § 4 (worker-pool GIF), § anti-patterns; `grid-snap-quantize-techniques.md` (whole pipeline).
- Lands in: S23 (Verb: Inbetween — three modes: procedural, AI, hybrid preview), S27 (Verb: Cleanup), S29 (Verb: Critique — VLM with classical preflight).

### Grid as discipline, not content

Pixel-grid reference images discipline the model into chunky blocks without ever appearing in the output. The grid is a constraint, not a content seed. Grid-snap's seven techniques run after generation. FalSprite's row-major math constrains layout without dictating subject matter. Sprite-pipeline keeps the baseline lock as a non-negotiable visual discipline.

- Seen in: `falsprite-prior-art.md` § 2 strict technical-requirements scaffold; `grid-snap-quantize-techniques.md` § pipeline at a glance; `sprite-pipeline-methodology.md` § 4 cross-cutting principles.
- Lands in: S27 (Verb: Cleanup — full normalization, see D-03), S35 (Verb: Tileset-from-description), S22 (Backend adapters — pre/post processing hooks).

### Transparent, inspectable file formats

Pixelorama's ZIP+JSON is diffable, version-control-friendly, and trivially scriptable. Aseprite's binary `.aseprite` survived because every chunk is documented and a community of CLI tools works around it. The cautionary tale in `project-library-research.md` is Blender's opaque `.blend` — powerful, but requires Blender or external tools to inspect. The choice between binary and text is less important than whether the format is *inspectable*.

- Seen in: `pixelorama-adoption.md` § 1 (ZIP container); `aseprite-prior-art.md` § 3 (native format); `project-library-research.md` § Blender, § Aseprite no-library.
- Lands in: B3 (project file format — see D-01), B7 (Aseprite format compatibility), S07 (`.pixhaus` native format).

### Command-tree undo with branching history

Aseprite's command system (level 2, MIT) is a branching command tree with a mixin pattern; every user action is a reversible Cmd. OpenToonz uses the same shape. The verb protocol aligns naturally because every AI verb is already a command-shaped operation with preview-then-commit. The tree (not the linear stack) is what matters — branching lets the user redo down a different path after an undo.

- Seen in: `aseprite-prior-art.md` § 10 (undo command system); `opentoonz-comparison.md` § stroke inbetweening (uses same command shape); `sprite-pipeline-methodology.md` § S21–S22 (verb protocol).
- Lands in: S05 (Undo/redo command pattern), S21 (Verb runtime), B5 (AI verb plugin protocol).

### Color quantization as a shared primitive

Quantization is not just an export step. It runs in cleanup (snap to project palette), in indexed-mode authoring (build the index image), in atlas rebuild (after rescaling), and in generation post-processing (snap AI output). Grid-snap's k-means++ is the same algorithm OpenToonz's `cleanuppalette.cpp` and Pixelorama's indexed-mode shadow buffer use. Build it once as a shared crate primitive; consume from every verb and export.

- Seen in: `grid-snap-quantize-techniques.md` § technique 1, § technique 7; `opentoonz-comparison.md` § color quantization; `pixelorama-adoption.md` § 2, § 3.
- Lands in: S02 (Color and palette ops — `core/src/color/quantize.rs`), S10 (PNG sprite sheet export), S11 (GIF/WebP export), S27 (Verb: Cleanup).

### Palette as first-class animated entity

A palette is not a static lookup table. OpenToonz's palette pages allow per-style keyframed colors (palette cycling, day/night transitions, hit-flash flicker — one cel, many states). Pixelorama's sparse palette permits named slots and reindexing. Aseprite supports per-frame palette arrays. Variants and animations both ride the palette; treat it as an animated, addressable data structure, not a `Vec<Rgba>`.

- Seen in: `opentoonz-comparison.md` § palette model; `pixelorama-adoption.md` § 2 (sparse palette HashMap); `aseprite-prior-art.md` § 11 (color and palette).
- Lands in: S02 (Color and palette ops), S18 (Color and palette panel), B2 (data model — see D-02).

## Open decisions

Five conflicts where the dossiers point to a path the current plan does not yet reflect, or where dossiers disagree. The digest surfaces them; resolution belongs in `../work/bedrock.md` or the named stream brief.

| ID | Topic | Current plan | Counter-evidence | Status | Recommended resolution | Owner |
| --- | --- | --- | --- | --- | --- | --- |
| D-01 | Project file format | B3: MessagePack (`rmp-serde`) + zstd | `pixelorama-adoption.md` § 1 argues ZIP + `manifest.json` + binary payloads — diffable, scriptable, version-control-friendly, supported by every language's stdlib | open | Adopt ZIP+JSON for `.pixhaus`. Cost is small (rmp-serde not yet shipped); user-facing wins (diff, inspect, third-party tools) compound. | B3 (`work/bedrock.md` § B3), S07 |
| D-02 | Palette structure | B2 / S02: assumed `Vec<Rgba8>` | `pixelorama-adoption.md` § 2 sparse `HashMap<u16, PaletteColor>` for gaps and reindex stability; `opentoonz-comparison.md` § palette proposes per-style animation overlay | open | Canonical: sparse `HashMap<u16, PaletteColor>` (Pixelorama). Optional overlay: per-frame palette animation table (OpenToonz). Keep flat-Vec view as a derived accessor. | B2, S02, S18 |
| D-03 | Cleanup verb scope | S27 brief: snap to palette, remove sub-pixel AA, fix pivot drift | `grid-snap-quantize-techniques.md` whole pipeline proposes a seven-step normalization (alpha-bbox split, chroma key, per-frame metrics, cross-sheet scale match, fixed-canvas re-pad, atlas rebuild, visual verify) | open | Expand S27 to cover the full seven-step pipeline as composable sub-steps. User can run any subset (rebaseline only, rescale only, full pass). | S27 |
| D-04 | Inbetween modes | S23 brief: AI frame-interpolation (RIFE-class or video diffusion) | `opentoonz-comparison.md` § stroke inbetweening proposes procedural variance-rejection as the fallback; `sprite-pipeline-methodology.md` § S23 lists i2v as a third option | open | Three modes in one verb: procedural (default, deterministic), AI interpolation (current S23), hybrid preview (procedural draft → AI refine). Backend declares which it supports; UI picks default by availability. | S23 |
| D-05 | Cel linking semantics | (Not yet specified in B2) | `aseprite-prior-art.md` § 1 uses pointer-shared CelData; `pixelorama-adoption.md` § 4 uses explicit link-set IDs (cleaner mutation semantics, easier to serialize) | open | Adopt Pixelorama's link-set ID model. Detaching is a single ID swap; serialization is trivial; the pointer-share variant complicates undo. | B2 |

## Port roadmap

Concrete units of upstream work ready to land, with destination, owner, and license obligation. Rows are the union of "Pixhaus landing" entries the dossiers already drafted — this digest aggregates, it does not invent.

Status legend: `not-started` · `in-flight` · `merged`. Size is per-dossier estimate.

| Source (dossier § section) | Insight / algorithm | Target file or crate | Bedrock / stream | License | Size | Status |
| --- | --- | --- | --- | --- | --- | --- |
| `aseprite-prior-art.md` § 1 | Document model (Sprite / Layer / Cel / CelData / Image / Palette) | `core/src/doc/` | B2 | MIT (Aseprite L0) | L | not-started |
| `aseprite-prior-art.md` § 2 | Pixel buffer with explicit stride | `core/src/doc/image.rs` | B2, S01 | MIT | M | not-started |
| `aseprite-prior-art.md` § 3 | `.aseprite` chunk decoder/encoder | `io/src/aseprite/` | B7, S08 | MIT | L | not-started |
| `aseprite-prior-art.md` § 6 | Pixel-perfect Bresenham line (with Zingl attribution) | `core/src/algorithm/line.rs` | S01 | MIT (+ Zingl) | S | not-started |
| `aseprite-prior-art.md` § 9 | Floodfill | `core/src/algorithm/floodfill.rs` | S03 | MIT | S | not-started |
| `aseprite-prior-art.md` § 10 | Undo command tree + mixin | `core/src/undo/` | S05 | MIT (`src/undo/` only; not `src/app/cmd*`) | M | not-started |
| `aseprite-prior-art.md` § 12 | Color quantization (median cut + k-means) | `core/src/color/quantize.rs` | S02 | MIT | M | not-started |
| `aseprite-prior-art.md` § 13 | Ordered + error-diffusion dithering | `core/src/color/dither.rs` | S02, S10 | MIT | S | not-started |
| `aseprite-prior-art.md` § 14 | RotSprite rotation | `core/src/algorithm/rotsprite.rs` | S04 | MIT | M | not-started |
| `aseprite-prior-art.md` § 15 | Onion skinning compositor | `core/src/render/onion.rs` | S19 | MIT | S | not-started |
| `aseprite-prior-art.md` § 16 | Tilemap and tileset model | `core/src/tilemap/` | B2, S06 | MIT | M | not-started |
| `aseprite-prior-art.md` § 21 | Rendering pipeline composition | `core/src/render/` | S14 | MIT | M | not-started |
| `opentoonz-comparison.md` § blend modes | 8 additional blend modes (vs Pixhaus's 19) | `core/src/blend/extra.rs` | S01 | BSD-3 | S | not-started |
| `opentoonz-comparison.md` § morphological AA | Edge-preserving anti-aliasing | `core/src/algorithm/morphological_aa.rs` | S55 (post-v1) | BSD-3 | M | deferred |
| `opentoonz-comparison.md` § gap-closing | Skeleton-LUT flood fill | `core/src/algorithm/floodfill_gapclose.rs` | S56 (post-v1) | BSD-3 | M | deferred |
| `opentoonz-comparison.md` § inbetween | Variance-rejection inbetween (procedural mode for D-04) | `ai/src/verbs/inbetween/procedural.rs` | S23 | BSD-3 | M | not-started |
| `opentoonz-comparison.md` § palette pages | Palette animation overlay (per-style keyframes) | `core/src/color/palette_animation.rs` | S53 (post-v1) | BSD-3 | L | deferred |
| `opentoonz-comparison.md` § centerline | Centerline vectorization | external crate | S58 (post-v1) | BSD-3 | XL | deferred |
| `pixelorama-adoption.md` § 1 | ZIP + `manifest.json` container (resolves D-01) | `io/src/pixhaus/` | B3, S07 | MIT (design adoption) | M | not-started |
| `pixelorama-adoption.md` § 2 | Sparse palette `HashMap<u16, PaletteColor>` (resolves D-02) | `core/src/color/palette.rs` | B2, S02 | MIT (design adoption) | S | not-started |
| `pixelorama-adoption.md` § 3 | Indexed mode with shadow RGBA buffer | `core/src/doc/image_indexed.rs` | B2, S01 | MIT | M | not-started |
| `pixelorama-adoption.md` § 4 | Cel linking via link-set IDs (resolves D-05) | `core/src/doc/cel.rs` | B2 | MIT | S | not-started |
| `pixelorama-adoption.md` § 9 | Allegro scanline flood fill | `core/src/algorithm/floodfill.rs` | S03 | MIT (P-tier port) | S | not-started |
| `pixelorama-adoption.md` § 14 | Seven pixel-art rotation algorithms (RotSprite, Scale2/3X, etc.) | `ui/src/shaders/rotate/` | S04 | MIT (S-tier shader port) | M | not-started |
| `pixelorama-adoption.md` § 17 | Single-pass multi-layer compositor (sampler2DArray) | `ui/src/shaders/compositor.glsl` | S14 | MIT (S-tier) | L | not-started |
| `pixelorama-adoption.md` § 18 | 24 blend modes by formula table | `ui/src/shaders/blend.glsl` + `core/src/blend/table.rs` | S01 | MIT (P-tier) | M | not-started |
| `pixelorama-adoption.md` § 23 | Onion-skin opacity = `base / frame_distance` with red/blue tinting | `ui/src/shaders/onion.glsl` | S19 | MIT (P-tier) | S | not-started |
| `pixelorama-adoption.md` § 24–26 | Tile cell shape + autotile bitmask | `core/src/tilemap/` | B2, S06 | MIT | M | not-started |
| `pixelorama-adoption.md` § 28 | Aseprite parser as chunk state machine | `io/src/aseprite/parser.rs` | B7, S08 | MIT (P-tier) | L | not-started |
| `pixelorama-adoption.md` § 35 | Theme engine with named token presets | `ui/src/theme/` | S13 | MIT (D-tier) | S | not-started |
| `falsprite-prior-art.md` § 1 | CHARACTER × CHOREOGRAPHY two-stage system prompts | `ai/verbs/sprite_sheet/prompts/` (verbatim assets) | S-NEW.1 (new verb) | MIT (asset vendor) | S | not-started |
| `falsprite-prior-art.md` § 2 | Strict technical-requirements scaffold for grid-shaped output | `ai/verbs/sprite_sheet/prompts/technical.txt` | S-NEW.1 | MIT | S | not-started |
| `falsprite-prior-art.md` § 3 | Row-major frame-grid math + RAF playback with FPS gating | `ui/src/components/timeline/SpriteSheetPlayback.tsx` | S19 | MIT | S | not-started |
| `falsprite-prior-art.md` § 4 | Worker-pool GIF export with per-frame canvas composition | `ui/src/export/gif.ts` | S11 | MIT | M | not-started |
| `grid-snap-quantize-techniques.md` § 1 | K-means++ color quantization | `core/src/color/quantize.rs` | S02, S27 | MIT (SpriteFusion) | S | not-started |
| `grid-snap-quantize-techniques.md` § 2 | Sobel gradient profiling for grid detection | `core/src/grid/profile.rs` | S27, S35 | MIT | S | not-started |
| `grid-snap-quantize-techniques.md` § 3 | Median-of-peak-spacings step estimate | `core/src/grid/step.rs` | S27 | MIT | S | not-started |
| `grid-snap-quantize-techniques.md` § 4 | Two-axis step reconciliation | `core/src/grid/step.rs` | S27 | MIT | S | not-started |
| `grid-snap-quantize-techniques.md` § 5 | Elastic walker for cut placement | `core/src/grid/walk.rs` | S27 | MIT | M | not-started |
| `grid-snap-quantize-techniques.md` § 6 | Two-pass cross-axis stabilization | `core/src/grid/stabilize.rs` | S27 | MIT | S | not-started |
| `grid-snap-quantize-techniques.md` § 7 | Majority-vote downsampling | `core/src/scale/majority_vote.rs` | S27, S12 | MIT | S | not-started |
| `sprite-pipeline-methodology.md` § stage 2–4 | Anchor-first cascade with directional economy | `ai/src/verbs/extend/` (verb prompts + flip-pair short-circuit) | S25 | MIT (reference / option C) | M | not-started |
| `sprite-pipeline-methodology.md` § stage 3 | Neutral anchor reset (effect stripping) | `ai/src/verbs/variant/neutral_reset.rs` | S26 | MIT | S | not-started |
| `sprite-pipeline-methodology.md` § stage 5 | Image-to-video walk-cycle frame-picker | `ai/src/verbs/motion_from_video/` | S32 | MIT | M | not-started |
| `sprite-pipeline-methodology.md` § stage 8 | Seven-step normalization (folds into D-03) | `ai/src/verbs/cleanup/normalize.rs` | S27 | MIT | L | not-started |
| `sprite-pipeline-methodology.md` § B2/B9 | Character anchor as first-class entity with `derived_from` edges | `core/src/doc/character_anchor.rs` | B2, B9 | MIT | M | not-started |
| `project-library-research.md` § Spine skins | Skin-style outfit/equipment variants without duplication | `core/src/doc/skin.rs` | B9 | reference-only (no code lift) | M | not-started |
| `project-library-research.md` § game studio taxonomies | Default entity taxonomy (characters / enemies / props / tilesets / VFX) | `docs/planning/work/b9-project-library.md` (already exists) | B9 | reference-only | S | merged |

## Attribution discipline

The seven dossiers each spell out a near-identical attribution pattern. Canonical statement, once:

- **Per-file header** on every ported file: name the upstream repo with commit pin or version, the upstream copyright line, the SPDX license identifier, and the path to the license text in this repo. The aseprite dossier § "Header template for ports" shows the exact shape; use it for every dossier source.
- **License file copy** in `licenses/<upstream-shortname>-<spdx>.txt` for each upstream we port from (e.g., `licenses/aseprite-MIT.txt`, `licenses/spritefusion-MIT.txt`, `licenses/opentoonz-BSD-3.txt`, `licenses/pixelorama-MIT.txt`, `licenses/falsprite-MIT.txt`).
- **Repo-level `THIRD_PARTY_LICENSES.md`** at the repo root, updated with each port PR. One section per upstream, listing the files we ported and the license-file pointer.
- **Vendor assets** (sample seeds, default palettes, theme tokens) get a sibling `LICENSE` file in the directory where they live (e.g., `assets/third-party/pixelorama/LICENSE`).
- **No code ships without attribution.** A clippy lint or a `scripts/check-attribution.sh` pre-commit hook is on the bedrock backlog to enforce this mechanically. Until then, treat it as a PR-review gate.

All four MIT-class licenses (Aseprite, Sprite Fusion, Pixelorama, FalSprite, sprite-pipeline) and the BSD-3 license (OpenToonz) are compatible with Pixhaus's MIT license. No GPL, LGPL, or AGPL entanglement.

## What this digest intentionally does not consolidate

The dossiers are still the place to go for:

- Per-tool source-tree walks (e.g., aseprite § "How it's decomposed" subsections, opentoonz § repo map).
- Pseudocode and algorithm walkthroughs (e.g., grid-snap's seven techniques each have their own pseudocode block; aseprite § floodfill walks the routine line by line).
- Level-by-level dependency maps (aseprite L0–L5, pixelorama A/S/P/D tiers, opentoonz S53–S58 stream proposals).
- License-audit matrices (aseprite § "License audit matrix").
- Detailed prompt text (falsprite § 1, § 2).
- Open questions specific to one dossier (most dossiers carry their own "Open questions" section — leave those there).

This digest answers "what should I do?" Open a dossier when you need "how exactly does it work?"
