# Pixhaus integration PRD

This is the product-requirements view of everything we integrate from prior art. Each capability below traces back to a specific dossier in [`../research/`](../research/) or a gap in [`../synthesis/gaps.md`](../synthesis/gaps.md). Nothing here is invented from scratch — every requirement either ports a working pattern, closes a documented gap, or adapts a design we already know works.

The companion docs answer different questions. [`scope.md`](scope.md) says *what Pixhaus is*. [`../synthesis/prior-art.md`](../synthesis/prior-art.md) is the dossier-organized digest of *what to port and where it lands in the codebase*. [`../work/streams.md`](../work/streams.md) is the execution view: 52 parallel work streams. This file is the product view: *what the artist gets, why we integrate it, what done looks like*.

Open conflicts (D-01…D-05) carry forward unresolved from [`../synthesis/prior-art.md`](../synthesis/prior-art.md). They are surfaced again at the bottom for product alignment.

## How to read a capability row

Each row uses the same compact shape:

- **One paragraph**: the artist outcome — what they can do, why it matters.
- **Sources**: dossier(s) and/or gap(s) the requirement comes from.
- **Requirements**: 2–4 acceptance criteria. Pass means the capability ships.
- **Lands in**: the bedrock spec (B*) or stream (S*) that delivers it.

Priority is encoded by section:

- **P0** — required for v1. Without it the [`scope.md`](scope.md) thesis fails.
- **P1** — should ship v1. Without it the product is weaker but viable.
- **P2** — post-v1. Strong leverage once the core is shipped.

---

## P0 — Pixel-perfect editing core

If an Aseprite user opens Pixhaus and feels lost, this block has failed. These capabilities are ground-truth-equivalent to Aseprite before anything AI-native ships on top.

### Document model: Sprite / Layer / Cel / CelData / Image / Palette

The project model that two decades of Aseprite refined: a sprite owns layers, layers own cels (frames), cels point to image data via a separate `CelData` so identical cels can share without copying. Palette is its own object referenced by frames.

- **Sources**: [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 1; [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 4 (cel linking via link-set IDs).
- **Requirements**: linked cels share image data until edited; image stride is explicit; layer tree supports groups; palette is addressable per-frame.
- **Lands in**: B2, S01.

### Project file format

A diffable, scriptable project file. Pixelorama's ZIP + `manifest.json` + binary payloads (each layer as PNG, palette as JSON) is the recommended shape — see open decision D-01 below.

- **Sources**: [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 1; [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 3 (`.aseprite` binary, as the contrasting transparent-binary example).
- **Requirements**: project file is inspectable without launching Pixhaus; round-trip with no data loss; backward compatible across minor versions.
- **Lands in**: B3, S07.

### Aseprite read and write compatibility

`.aseprite` / `.ase` files import and export with full fidelity for the documented chunk set. This is the migration bridge for the Aseprite-installed-base audience that [`scope.md`](scope.md) names as primary.

- **Sources**: [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 3; [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 28 (chunk-state-machine parser as the proven approach).
- **Requirements**: read every chunk type from a v1.3 file; write a v1.3 file Aseprite opens without warning; preserve layer ordering, frame durations, tags, palette, tilemaps.
- **Lands in**: B7, S08.

### Indexed color and palette discipline

Every pixel addresses a palette slot, not an RGB triplet. The palette is a sparse `HashMap<u16, PaletteColor>` (Pixelorama's shape — see D-02) with optional per-frame animation overlay (OpenToonz palette pages).

- **Sources**: [`../synthesis/patterns.md`](../synthesis/patterns.md) § "Indexed color is non-negotiable for pixel art"; [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 2–3; [`../research/opentoonz-comparison.md`](../research/opentoonz-comparison.md) § palette model; [`../synthesis/prior-art.md`](../synthesis/prior-art.md) § "Palette as a first-class animated entity".
- **Requirements**: indexed mode authoring is first-class (not a checkbox); palette slots can be sparse / reindexed without breaking image data; per-frame palette swaps are supported.
- **Lands in**: B2, S02, S18.

### Drawing primitives: pixel-perfect lines, floodfill, RotSprite

The atomic operations that every brush ultimately calls. Aseprite's Bresenham line drawing (with Zingl attribution), its floodfill, and RotSprite for rotation are MIT and field-proven over two decades.

- **Sources**: [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 6 (line), § 9 (floodfill), § 14 (RotSprite); [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 9 (Allegro scanline floodfill — alternative), § 14 (seven pixel-art rotation algorithms).
- **Requirements**: line drawing is pixel-perfect (no double-pixel staircases); floodfill handles gaps and selection masks; RotSprite produces clean rotations at 90° and arbitrary angles.
- **Lands in**: S01, S03, S04.

### Selection, transform, symmetry

Selection mask as an LA8 image (Pixelorama's shape), transform with floating overlay until commit, axis and diagonal symmetry inspired by Aseprite without lifting the EULA-licensed UI code.

- **Sources**: [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 5 (symmetry, inspire-only), § 8 (selection mask); [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 5 (SelectionMap), § 16 (transformation handles).
- **Requirements**: marching ants render at 60fps; transforms are non-destructive until commit; symmetry mirrors brush strokes live across configurable axes.
- **Lands in**: S03, S04, S16.

### Layer composition with working blend modes

The Aseprite gap [`scope.md`](scope.md) calls out by name: group layers with blend modes that actually work. Pixelorama's 24-formula blend table (P-tier port) and OpenToonz's eight additional modes round out the set.

- **Sources**: [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 17–19 (compositor, blend table, group layers); [`../research/opentoonz-comparison.md`](../research/opentoonz-comparison.md) § blend modes; [`../synthesis/gaps.md`](../synthesis/gaps.md) § "Layer group blend modes don't work" (Aseprite-specific gap).
- **Requirements**: group opacity and blend modes compose correctly; ≥24 blend modes ship in v1; single-pass compositor runs on WebGL2 sampler2DArray.
- **Lands in**: S01, S14, S17.

### Animation timeline with onion skin and frame tags

Onion skin with `opacity = base / frame_distance` and red/blue tinting (Pixelorama's shape, Aseprite's defaults). Frame tags as `{name, from, to, direction}` named ranges within one timeline.

- **Sources**: [`../synthesis/patterns.md`](../synthesis/patterns.md) § "Onion skin is sacred", § "Frame tags organize multi-animation files"; [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 15, § 17; [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 7, § 23.
- **Requirements**: onion skin tinting is configurable; frame tags round-trip through `.aseprite`; sprite-sheet export consumes tags as named animations.
- **Lands in**: S05 (undo), S10 (export), S19 (timeline UI).

### Tilemap as a first-class layer type

Closes the Aseprite-Tiled split that [`scope.md`](scope.md) names as the second-largest non-AI bet. Tile cell `{ index, flip_h, flip_v, transpose }` (Pixelorama's shape), autotile via per-tile peering bitmask, animated tiles supported.

- **Sources**: [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 24–26; [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 16; [`../synthesis/gaps.md`](../synthesis/gaps.md) § "Tile autotile generation is still tedious".
- **Requirements**: tilemap layers coexist with sprite layers in one project; autotile rules configure in-tool (no Tiled hop); animated tiles play in the timeline independently.
- **Lands in**: B2, S06, S20.

### Undo with branching history

A command tree, not a linear stack. The artist undoes, tries a different branch, and the redo path down the original branch survives. Aseprite's MIT `src/undo/` is the reference port; OpenToonz uses the same shape.

- **Sources**: [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 10 (`src/undo/` only — MIT, not `src/app/cmd*`); [`../synthesis/prior-art.md`](../synthesis/prior-art.md) § "Command-tree undo with branching history".
- **Requirements**: undo crosses tools, AI verbs, and file operations uniformly; branching survives a session; persistent across save (stretch).
- **Lands in**: S05, S21.

### Sprite sheet + JSON export

Aseprite-compatible JSON metadata so existing Unity / Godot / Phaser importers work day one. PNG sprite sheet packing with configurable algorithms.

- **Sources**: [`../synthesis/patterns.md`](../synthesis/patterns.md) § "Sprite sheet + JSON metadata is the engine handoff"; [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 17, § 21.
- **Requirements**: emitted JSON parses with existing Aseprite importer packages without modification; frame rectangles, durations, tags, pivots all included.
- **Lands in**: S10.

---

## P0 — AI verbs (the differentiator)

The verbs that make Pixhaus AI-native rather than AI-flavored. Each one runs the three-mode architecture from [`../synthesis/ai-opportunity.md`](../synthesis/ai-opportunity.md) § "Three-mode AI verb architecture": procedural default, AI on demand, hybrid preview.

### Inbetween

Generate intermediate frames between two key frames. Procedural mode uses OpenToonz's variance-rejection averaging; AI mode uses RIFE-class interpolation or video diffusion; hybrid shows procedural preview, commits with AI.

- **Sources**: [`../research/opentoonz-comparison.md`](../research/opentoonz-comparison.md) § stroke inbetweening (procedural baseline); [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § stage 5 (i2v); [`../synthesis/ai-opportunity.md`](../synthesis/ai-opportunity.md) § frame interpolation. Open decision D-04.
- **Requirements**: three modes selectable; output snaps to project palette; AI mode declares backend requirements.
- **Lands in**: S23.

### Cleanup

Snap to palette, remove sub-pixel AA, fix pivot drift, and run the full seven-step normalization from grid-snap. ML refinement is the optional layer on top — not the core. See open decision D-03.

- **Sources**: [`../research/grid-snap-quantize-techniques.md`](../research/grid-snap-quantize-techniques.md) (whole pipeline); [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § stage 8; [`../synthesis/gaps.md`](../synthesis/gaps.md) § "Cleanup pipelines exist but stay outside the editor".
- **Requirements**: sub-steps are composable (rebaseline only, rescale only, full pass); deterministic mode produces identical output across runs; one-click pass on any imported sprite.
- **Lands in**: S27.

### Extend (multi-direction)

Generate alternate views — 4-direction, 8-direction, custom angles — from a single canonical pose. Anchor-first cascading (sprite-pipeline) and directional economy (east = flip(west)) cut the generation budget by roughly half and improve consistency.

- **Sources**: [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § stages 2–4; [`../synthesis/prior-art.md`](../synthesis/prior-art.md) § "Anchor-first canonical pose", § "Directional economy"; [`../synthesis/gaps.md`](../synthesis/gaps.md) § "Multi-angle / 8-directional character generation is rare".
- **Requirements**: south anchor is mandatory; mirrored views are generated by flip, not regeneration; output layers attach to a character anchor entity.
- **Lands in**: S25, B2 (`character_anchor`), B9.

### Variant

Palette swaps, equipment overlays, expression sets as derived layers. The variant-storage architecture (link-set IDs) is solved across the field — this verb adds AI-assisted generation that respects style.

- **Sources**: [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § stage 3 (neutral anchor reset); [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 4 (link-set IDs); [`../synthesis/gaps.md`](../synthesis/gaps.md) § "Asset variations: storage is solved, generation is not".
- **Requirements**: variants are derived layers via link-set, not copies; palette-swap path is mostly rule-based with ML refinement at edges; equipment overlays compose without manual masking.
- **Lands in**: S26.

### Critique

VLM analysis of a sprite or animation that surfaces pose continuity errors, palette violations, missing frames, pivot drift, style inconsistency. Closes the asset-QA gap [`../synthesis/gaps.md`](../synthesis/gaps.md) names.

- **Sources**: [`../synthesis/gaps.md`](../synthesis/gaps.md) § "Sprite asset QA is manual"; [`../synthesis/ai-opportunity.md`](../synthesis/ai-opportunity.md) § asset library QA.
- **Requirements**: classical preflight runs before VLM (cheap palette/pivot checks); findings link to the offending frame; categories are configurable.
- **Lands in**: S29.

---

## P1 — Extended capabilities

Capabilities that strengthen the v1 surface but aren't gate-keepers.

### Continue

Predict the next 1–3 frames given the last N. Backend: image-gen with palette + reference conditioning.

- **Sources**: [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § stages 6–7.
- **Requirements**: continuation conditions on the last 3–5 frames; output snaps to palette.
- **Lands in**: S24.

### Tile (autotile generation)

Generate the 47-tile blob set from 1–3 example transitions. The geometric layout is solved (Tilesetter); the AI value is generating the missing pixel transitions in style.

- **Sources**: [`../synthesis/ai-opportunity.md`](../synthesis/ai-opportunity.md) § tile autotile generation; [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 26 (peering bitmask).
- **Requirements**: 47 transitions tile correctly with neighbors; autotile rules pre-configured on output; style matches the example transitions.
- **Lands in**: S28.

### Tileset-from-description

Generate a full autotile-compatible tileset from a prompt. Uses grid-snap's gradient profiling to enforce tile geometry, then runs the Tile verb's transition generation.

- **Sources**: [`../research/grid-snap-quantize-techniques.md`](../research/grid-snap-quantize-techniques.md) § techniques 2–6; [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § S35.
- **Requirements**: output tileset has consistent step size on both axes; autotile rules attach automatically; style is set by reference, not prompt alone.
- **Lands in**: S35.

### Sketch finishing

Artist sketches rough silhouettes or stick figures; AI refines to finished sprites in the project's learned style. Anchor-first principle keeps the result consistent with existing characters.

- **Sources**: [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § S36; [`../synthesis/ai-opportunity.md`](../synthesis/ai-opportunity.md) § style transfer.
- **Requirements**: artist accepts or rejects per frame; output respects palette; project style LoRA is used as default style reference.
- **Lands in**: S36.

### Project style learning (LoRA)

Per-project LoRA trained from existing layers. Trained in 15–30 minutes, registered as the default style reference for subsequent verbs.

- **Sources**: [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § S30 mapping; [`../synthesis/ai-opportunity.md`](../synthesis/ai-opportunity.md) § style transfer.
- **Requirements**: training runs against project data without manual labeling; LoRA persists in the project file; subsequent verbs pick it up as default.
- **Lands in**: S30.

### Conversational editing

Natural language drives multi-step editor commands ("make this enemy look angrier, add a scar over the left eye, slow the walk to 8fps"). VLM plans the command sequence using the existing command vocabulary.

- **Sources**: [`../synthesis/ai-opportunity.md`](../synthesis/ai-opportunity.md) § three-mode architecture; existing scope.md S31.
- **Requirements**: planned commands surface as a preview before execution; full plan undoes as a single entry; no command runs without artist accept.
- **Lands in**: S31.

### Motion-from-video

Extract motion timing and key poses from a reference video into the timeline. Pose estimation (DensePose / MediaPipe) + VLM for keyframe identification.

- **Sources**: [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § stage 5 (i2v frame-picker); [`../synthesis/ai-opportunity.md`](../synthesis/ai-opportunity.md) § reference matching; [`../synthesis/gaps.md`](../synthesis/gaps.md) § "Animation reference matching is manual".
- **Requirements**: artist owns every frame; AI delivers timing skeleton + rough silhouettes; reference video plays scrubbable next to the timeline.
- **Lands in**: S32.

---

## P1 — Project library and variants

### Character anchors as first-class entities

A character anchor is a project-level entity with `derived_from` edges to all its variants, animations, and directional views. This is what makes cascading re-rolls cheap and consistent.

- **Sources**: [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § B2/B9; [`../research/project-library-research.md`](../research/project-library-research.md) § Spine skins.
- **Requirements**: anchor is addressable from any verb; deleting an anchor cascades to derivatives with confirmation; library panel lists anchors as the top-level entry.
- **Lands in**: B2, B9.

### Reference sheets

Reference images as monochrome/overlay/clamp shader overlays on the canvas. Persists across sessions; multiple references per project.

- **Sources**: [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 37; existing `work/b10-reference-sheets.md`.
- **Requirements**: reference images load from drag-and-drop; opacity and blend modes apply; references don't enter the export pipeline.
- **Lands in**: B10.

---

## P1 — Output and ecosystem

### Unity importer

UPM package that imports `.pixhaus` files directly. Sprite sheets, animation tags, palette, tilemap all consume on the Unity side without sidecar JSON.

- **Sources**: [`scope.md`](scope.md) § engine target; [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 30 (plugin-registered formats).
- **Requirements**: importer installs via UPM URL; opens `.pixhaus` and `.aseprite`; round-trip preserves animation tags and palette.
- **Lands in**: B6, S39, S40.

### Plugin system: Lua + WASM

Lua for scripting (mlua), WASM (extism) for cross-language plugins. Plugin manifests with crash-detect-then-disable.

- **Sources**: [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 29–30; [`scope.md`](scope.md) § plugins.
- **Requirements**: plugins register file formats, verbs, and panels; crashes auto-disable the offending plugin; manifest declares permissions.
- **Lands in**: S37, S38.

### Animated GIF and WebP export

Worker-pool composition (FalSprite shape), per-frame palette quantization (grid-snap majority-vote), looped or once playback.

- **Sources**: [`../research/falsprite-prior-art.md`](../research/falsprite-prior-art.md) § 4; [`../research/grid-snap-quantize-techniques.md`](../research/grid-snap-quantize-techniques.md) § technique 7.
- **Requirements**: GIF export uses project palette; WebP export supports both lossless and lossy; frame durations honor timeline.
- **Lands in**: S11.

---

## P2 — Post-v1 candidates

Capabilities the dossiers document but that don't gate v1.

### OpenToonz advanced (S53–S58)

Palette pages with animation, eight additional blend modes, morphological AA, gap-closing wand, procedural inbetween, centerline vectorization.

- **Sources**: [`../research/opentoonz-comparison.md`](../research/opentoonz-comparison.md) § proposed follow-up streams.
- **Lands in**: S53–S58.

### Audio-driven timing

Beat detection / lip sync from audio. Audio analysis is mostly classical; VLM handles lip-sync intent.

- **Sources**: [`scope.md`](scope.md) § verb set; existing S34 in `work/streams.md`.
- **Lands in**: S34.

### Auto-mesh deformation

No-bones rigging via segmentation + view synthesis. Live2D-style without explicit bones — segment the sprite, derive a deformation rig automatically.

- **Sources**: [`scope.md`](scope.md) § verb set; [`../research/project-library-research.md`](../research/project-library-research.md) § Live2D parameters.
- **Lands in**: S33.

### FalSprite-shaped animated sprite sheet from prompt

Two-stage LLM choreography (CHARACTER × CHOREOGRAPHY) producing a grid sprite sheet via fal.ai. Lifts the FalSprite prompts verbatim as MIT-attributed assets.

- **Sources**: [`../research/falsprite-prior-art.md`](../research/falsprite-prior-art.md) § 1–3.
- **Lands in**: S-NEW.1 (new verb in `work/streams.md` revision).

---

## Open decisions

Five conflicts the dossiers surface that block specific capabilities. Resolution belongs in [`../work/bedrock.md`](../work/bedrock.md) or the named stream brief — not here. Listed for product alignment.

| ID | Decision | Blocks |
| --- | --- | --- |
| D-01 | Project file format: MessagePack+zstd vs ZIP+JSON | Project file format capability above |
| D-02 | Palette shape: flat Vec vs sparse HashMap vs animated overlay | Indexed color and palette discipline |
| D-03 | Cleanup verb scope: narrow vs full seven-step | Cleanup verb |
| D-04 | Inbetween modes: AI-only vs procedural vs hybrid | Inbetween verb |
| D-05 | Cel linking: pointer-share vs link-set IDs | Document model, Variant verb |

Full counter-evidence and recommended resolution per row: [`../synthesis/prior-art.md`](../synthesis/prior-art.md) § "Open decisions".

## What we explicitly do not integrate

The dossiers document plenty we deliberately leave on the table:

- **Aseprite UI framework** ([`aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 20) — MIT but skip. Solid + WebGL2 is our target; lifting `src/ui/` adds nothing.
- **Aseprite EULA-licensed application layer** — tool system, brush dynamics, scripting API, plugin model. Inspire-only; we reimplement.
- **OpenToonz centerline vectorization** ([`opentoonz-comparison.md`](../research/opentoonz-comparison.md) § centerline) — vector output is out of scope for a raster editor. Defer indefinitely.
- **Pixelorama splash dialog** ([`pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 36) — D-tier but unnecessary for v1.
- **Krita and PSD parsers** — deferred reference only. PSD import is a P2 nice-to-have; Krita is even further out.
- **Multi-user collaboration** — [`../synthesis/gaps.md`](../synthesis/gaps.md) names this as a real gap, but it's a CRDT engineering problem, not a prior-art port. Out of scope for v1; revisit only with a clear use case.

## Success criteria

The integration is successful when:

1. Every P0 capability has a passing acceptance test traceable to its source dossier or gap.
2. An Aseprite-installed-base artist can open Pixhaus, import a `.aseprite` file, edit it with their existing muscle memory, run at least one AI verb (Cleanup or Inbetween), and export a sprite sheet that drops into their Unity project — all without consulting documentation for basic operations.
3. The five open decisions (D-01…D-05) are resolved in [`../work/bedrock.md`](../work/bedrock.md) before the streams that depend on them start.
4. No P0 capability ships without its attribution discipline ([`../synthesis/prior-art.md`](../synthesis/prior-art.md) § "Attribution discipline") in place.
5. Every gap closed in [`../synthesis/gaps.md`](../synthesis/gaps.md) by the integrations above is removed from the gaps file in the same PR that ships the closing capability.
