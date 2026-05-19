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

No phases. No priority tiers. Every capability listed below is in scope. The order is reading order — what an artist encounters first sits earlier in the document. The execution order is governed by [`../work/streams.md`](../work/streams.md), not by this file.

---

## Document model and file format

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

### Undo with branching history

A command tree, not a linear stack. The artist undoes, tries a different branch, and the redo path down the original branch survives. Aseprite's MIT `src/undo/` is the reference port; OpenToonz uses the same shape.

- **Sources**: [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 10 (`src/undo/` only — MIT, not `src/app/cmd*`); [`../synthesis/prior-art.md`](../synthesis/prior-art.md) § "Command-tree undo with branching history".
- **Requirements**: undo crosses tools, AI verbs, and file operations uniformly; branching survives a session; persistent across save (stretch).
- **Lands in**: S05, S21.

---

## Drawing primitives

The atomic operations every brush, verb, and tool calls into. Six dossier-derived primitives, each a separate row so they can be ported independently.

### Pixel-perfect line drawing

Aseprite's Bresenham line (with Zingl attribution) draws clean diagonals without double-pixel staircases. MIT, field-proven for two decades.

- **Sources**: [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 6.
- **Requirements**: no double-pixel runs at any slope; brush size scales the line without breaking pixel-perfection; integrates with symmetry without artifacts.
- **Lands in**: S01.

### Floodfill (with gap-closing variant)

Aseprite's classical floodfill handles the common case. OpenToonz's gap-closing floodfill (skeleton-LUT closure) handles hand-drawn line art where the outline has small gaps that ruin a naive fill.

- **Sources**: [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 9; [`../research/opentoonz-comparison.md`](../research/opentoonz-comparison.md) § "Gap-closing flood fill"; [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 9 (Allegro scanline floodfill — alternative).
- **Requirements**: respects selection mask; gap-closing mode is opt-in with a tolerance slider; both modes share the same target-color matching predicate.
- **Lands in**: S01, S03.

### RotSprite plus Pixelorama's seven rotation algorithms

RotSprite for high-quality rotation at arbitrary angles. Pixelorama's seven pixel-art-specific rotation algorithms (Scale2X, Scale3X, RotSprite, and four variants) give the artist a per-context choice.

- **Sources**: [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 14; [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 14, § 15 (Scale3X upscale).
- **Requirements**: rotation at 90° is bit-exact; arbitrary-angle rotations have configurable algorithm; all algorithms preserve indexed color when source is indexed.
- **Lands in**: S04.

### Morphological anti-aliasing

OpenToonz's edge-preserving AA that classical bilinear smears would destroy. The right tool when scaling or rotating pixel art that has antialiased edges from a different source pipeline.

- **Sources**: [`../research/opentoonz-comparison.md`](../research/opentoonz-comparison.md) § "Morphological anti-aliasing".
- **Requirements**: preserves edge geometry across rotation and scale; opt-in per operation; falls back to nearest-neighbor for pure pixel art.
- **Lands in**: S04.

### Brush stroke rasterization

OpenToonz's production-tested stroke pipeline (`rasterstrokegenerator.cpp`). The rasterization layer every brush ultimately calls — pressure, spacing, stamp shape, stroke smoothing all live here.

- **Sources**: [`../research/opentoonz-comparison.md`](../research/opentoonz-comparison.md) § "Brush stroke rasterization".
- **Requirements**: pressure-sensitive when a tablet is present; spacing configurable; stamp shapes plug in without touching the rasterizer core.
- **Lands in**: S01, S15.

### Fast raster operations (16-bit fixed point)

The performance primitive every pixel-level operation lives on top of. OpenToonz's `quickput.cpp` uses 16-bit fixed-point composition for speed and accuracy that floating-point and 8-bit alternatives both miss.

- **Sources**: [`../research/opentoonz-comparison.md`](../research/opentoonz-comparison.md) § "Fast raster operations".
- **Requirements**: per-pixel composition runs at 60fps for 4K layers on commodity hardware; SIMD where the platform supports it; matches reference output bit-for-bit.
- **Lands in**: S01, S14.

---

## Indexed color and palette

### Sparse palette as `HashMap<u16, PaletteColor>`

Every pixel addresses a palette slot, not an RGB triplet. Pixelorama's sparse `HashMap<u16, PaletteColor>` permits named slots, reindexing without breaking image data, and gaps for future expansion. This is the shape D-02 leans toward.

- **Sources**: [`../synthesis/patterns.md`](../synthesis/patterns.md) § "Indexed color is non-negotiable for pixel art"; [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 2; [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 11.
- **Requirements**: indexed mode authoring is first-class (not a checkbox); palette slots can be sparse / reindexed without breaking image data; named slots persist across save.
- **Lands in**: B2, S02, S18.

### Palette pages with animation overlay

OpenToonz's palette pages keyframe colors across styles — one cel, many states. Palette cycling, day/night transitions, hit-flash flicker all ride this overlay. Sits on top of the sparse palette as an optional per-style animation table.

- **Sources**: [`../research/opentoonz-comparison.md`](../research/opentoonz-comparison.md) § "Palette model"; [`../synthesis/prior-art.md`](../synthesis/prior-art.md) § "Palette as a first-class animated entity".
- **Requirements**: palette pages are addressable from the timeline; per-style animation keyframes interpolate; cel rendering picks the right page automatically.
- **Lands in**: B2, S02, S18, S19.

### `cleanuppalette` quantization knobs

OpenToonz's `cleanuppalette.cpp` exposes per-target-color knobs the generic k-means quantization in grid-snap lacks. The right tool when the artist wants to lock specific palette entries while quantizing the rest.

- **Sources**: [`../research/opentoonz-comparison.md`](../research/opentoonz-comparison.md) § "Color quantization"; [`../research/grid-snap-quantize-techniques.md`](../research/grid-snap-quantize-techniques.md) § technique 1 (k-means as the baseline path).
- **Requirements**: targeted-color mode preserves locked palette entries exactly; tolerance-per-color is configurable; output is identical across runs with the same input.
- **Lands in**: S02, S27.

---

## Selection and transform

### Selection mask

Selection mask as an LA8 image (Pixelorama's shape). Marching ants render at 60fps; non-rectangular and feathered selections both fit.

- **Sources**: [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 8; [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 5.
- **Requirements**: marching ants render at 60fps; transforms operate on selection; mask survives layer changes.
- **Lands in**: S03, S16.

### Transform overlay

Non-destructive transform with a floating overlay until commit (Pixelorama's transformation handles).

- **Sources**: [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 16.
- **Requirements**: transforms are non-destructive until commit; rotate, scale, skew compose into a single matrix; cancel restores the original.
- **Lands in**: S04, S16.

### Symmetry

Axis and diagonal symmetry inspired by Aseprite without lifting the EULA-licensed UI code. Mirror brush strokes live across configurable axes.

- **Sources**: [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 5 (inspire-only).
- **Requirements**: live mirroring during stroke; configurable axes (vertical, horizontal, diagonal, point); compatible with selection.
- **Lands in**: S15, S16.

---

## Layer composition and blend modes

### Unified blend-mode set

OpenToonz's 22+ blend modes and Pixelorama's 24-formula table merged into one canonical set (~30 modes after dedup). The Aseprite gap [`scope.md`](scope.md) calls out by name — group layers with blend modes that actually work — closes here.

- **Sources**: [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 18 (24-formula table); [`../research/opentoonz-comparison.md`](../research/opentoonz-comparison.md) § "Blend modes" (22+ modes); [`../synthesis/gaps.md`](../synthesis/gaps.md) § "Aseprite's specific gaps" (group blend modes don't work).
- **Requirements**: ≥30 blend modes ship; group opacity and blend modes compose correctly; single-pass compositor runs on WebGL2 sampler2DArray.
- **Lands in**: S01, S14, S17.

### Group layers and pass-through

Group layers with optional Pass-Through blending (Pixelorama). Groups respect opacity and blend modes (the Aseprite gap).

- **Sources**: [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 17, § 19.
- **Requirements**: group opacity composes; pass-through mode is opt-in per group; nested groups blend correctly.
- **Lands in**: S17.

### Clipping masks

Clipping mask: the layer below acts as the alpha mask for the layer above (Pixelorama's shape).

- **Sources**: [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 20.
- **Requirements**: toggle per layer; chains correctly when the masking layer is itself in a group; selection mask compatibility.
- **Lands in**: S17.

### Non-destructive layer effects

Per-layer effects as `Vec<LayerEffect>` (Pixelorama). Stackable, reorderable, and removable without committing to pixels.

- **Sources**: [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 8.
- **Requirements**: effects render via shader (no pre-baking); reorder swaps order without re-compositing the layer; effects survive `.aseprite` round-trip via a Pixhaus-specific extension chunk.
- **Lands in**: S01, S17.

---

## Animation timeline

### Onion skin

Onion skin with `opacity = base / frame_distance` and red/blue tinting (Pixelorama's shape, Aseprite's defaults). Configurable range and tinting.

- **Sources**: [`../synthesis/patterns.md`](../synthesis/patterns.md) § "Onion skin is sacred"; [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 15; [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 23.
- **Requirements**: range and tinting are configurable; renders at 60fps for the active frame; toggles via shortcut.
- **Lands in**: S19.

### Frame tags

Named ranges within one timeline (`idle: 0-3`, `walk: 4-11`) with per-tag loop direction. Storing all character animations in one file with tags beats one file per animation — the convergent pattern.

- **Sources**: [`../synthesis/patterns.md`](../synthesis/patterns.md) § "Frame tags organize multi-animation files"; [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 17; [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 7.
- **Requirements**: tags round-trip through `.aseprite`; sprite-sheet export consumes tags as named animations; per-tag direction (forward / reverse / ping-pong) survives.
- **Lands in**: S10, S19.

### Palette-animation timeline overlay

When palette pages are in use, the timeline shows palette-animation keyframes as a row alongside the cel rows. Surface the palette-cycling discipline OpenToonz pioneered.

- **Sources**: [`../research/opentoonz-comparison.md`](../research/opentoonz-comparison.md) § "Palette model".
- **Requirements**: palette keyframes are scrubbable in the timeline; keyframe drag updates the page; tag-scoped palette animations are supported.
- **Lands in**: S19.

### Animated tiles

Tiles in a tilemap layer hold their own animation tag and play independently of the sprite timeline. Closes a long-standing gap [`scope.md`](scope.md) names: "tile animation is currently a workaround in every tool surveyed."

- **Sources**: [`scope.md`](scope.md) § Animation timeline; [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 24–26 (tile cell shape).
- **Requirements**: animated tile plays in-tilemap; multiple animated tiles in one tilemap play at independent rates; animation survives `.aseprite` and Unity round-trips.
- **Lands in**: B2, S06, S19.

---

## Tilemap and autotile

### Tilemap as a first-class layer type

Closes the Aseprite-Tiled split that [`scope.md`](scope.md) names as the second-largest non-AI bet. Tile cell `{ index, flip_h, flip_v, transpose }` (Pixelorama's shape), tilemap layers coexist with sprite layers in one project.

- **Sources**: [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 24; [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 16; [`../synthesis/gaps.md`](../synthesis/gaps.md) § "Tile autotile generation is still tedious".
- **Requirements**: tilemap layers and sprite layers compose in one project; tile cells encode flip and transpose; tileset is shared across tilemap layers.
- **Lands in**: B2, S06, S20.

### Autotile via per-tile peering bitmask

Pixelorama's per-tile peering bitmask configures Wang-style autotile rules in-tool, no Tiled hop required.

- **Sources**: [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 26.
- **Requirements**: 47-blob and Wang corner sets configure via UI; rules survive save; new tilesets get a default rule template.
- **Lands in**: S06, S20.

### TileSetCustom: shape + offset axis

Custom tile shapes (hexagonal, isometric, triangular) with configurable offset axis. Pixhaus's tilemap is not square-tile-only.

- **Sources**: [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 25.
- **Requirements**: square, hex, and iso tile shapes supported; offset axis configurable per tileset; autotile rules respect the shape.
- **Lands in**: S06, S20.

---

## AI verbs

Every verb below runs the three-mode architecture from [`../synthesis/ai-opportunity.md`](../synthesis/ai-opportunity.md) § "Three-mode AI verb architecture": procedural default, AI on demand, hybrid preview. Each verb runs against a reference sheet when the target entity has one (see "Reference sheets — the anchor primitive" below).

### Inbetween

Generate intermediate frames between two key frames. Procedural mode uses OpenToonz's variance-rejection averaging; AI mode uses RIFE-class interpolation or video diffusion; hybrid shows procedural preview, commits with AI.

- **Sources**: [`../research/opentoonz-comparison.md`](../research/opentoonz-comparison.md) § "Stroke inbetweening" (procedural baseline); [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § stage 5 (i2v); [`../synthesis/ai-opportunity.md`](../synthesis/ai-opportunity.md) § frame interpolation. Open decision D-04.
- **Requirements**: three modes selectable; output snaps to project palette; AI mode declares backend requirements.
- **Lands in**: S23.

### Cleanup

The full grid-snap seven-step normalization (k-means → gradient profile → step estimate → reconciliation → walker → stabilization → majority-vote downsample) plus OpenToonz `cleanuppalette` for targeted-color quantization. ML refinement is the optional layer for ambiguous artifacts — not the core. Open decision D-03.

- **Sources**: [`../research/grid-snap-quantize-techniques.md`](../research/grid-snap-quantize-techniques.md) (whole pipeline); [`../research/opentoonz-comparison.md`](../research/opentoonz-comparison.md) § "Color quantization"; [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § stage 8; [`../synthesis/gaps.md`](../synthesis/gaps.md) § "Cleanup pipelines exist but stay outside the editor".
- **Requirements**: sub-steps are composable (rebaseline only, rescale only, full pass); deterministic mode produces identical output across runs; targeted-color quantization preserves locked palette entries.
- **Lands in**: S27.

### Extend (multi-direction)

Generate alternate views — 4-direction, 8-direction, custom angles — from a single canonical pose. Anchor-first cascading and directional economy (east = flip(west)) cut the generation budget by roughly half and improve consistency.

- **Sources**: [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § stages 2–4; [`../synthesis/prior-art.md`](../synthesis/prior-art.md) § "Anchor-first canonical pose", § "Directional economy"; [`../synthesis/gaps.md`](../synthesis/gaps.md) § "Multi-angle / 8-directional character generation is rare".
- **Requirements**: south anchor is mandatory; mirrored views are generated by flip, not regeneration; output layers attach to the entity's reference sheet.
- **Lands in**: S25, B10.

### Variant

Palette swaps, equipment overlays, expression sets as derived layers. The variant-storage architecture (link-set IDs) is solved across the field — this verb adds AI-assisted generation that respects style.

- **Sources**: [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § stage 3 (neutral anchor reset); [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 4 (link-set IDs); [`../synthesis/gaps.md`](../synthesis/gaps.md) § "Asset variations: storage is solved, generation is not".
- **Requirements**: variants are derived layers via link-set, not copies; palette-swap path is mostly rule-based with ML refinement at edges; equipment overlays compose without manual masking.
- **Lands in**: S26.

### Critique

VLM analysis that surfaces pose continuity errors, palette violations, missing frames, pivot drift, style inconsistency. Classical preflight runs first (cheap palette/pivot checks); VLM handles the ambiguous cases.

- **Sources**: [`../synthesis/gaps.md`](../synthesis/gaps.md) § "Sprite asset QA is manual"; [`../synthesis/ai-opportunity.md`](../synthesis/ai-opportunity.md) § asset library QA.
- **Requirements**: classical preflight runs before VLM; findings link to the offending frame; categories are configurable.
- **Lands in**: S29.

### Continue

Predict the next 1–3 frames given the last N. Backend: image-gen with palette + reference conditioning.

- **Sources**: [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § stages 6–7.
- **Requirements**: continuation conditions on the last 3–5 frames; output snaps to palette; respects the entity's reference sheet.
- **Lands in**: S24.

### Tile (autotile generation)

Generate the 47-tile blob set from 1–3 example transitions. Geometric layout is solved (autotile bitmask above); the AI value is generating the missing pixel transitions in style.

- **Sources**: [`../synthesis/ai-opportunity.md`](../synthesis/ai-opportunity.md) § tile autotile generation; [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 26.
- **Requirements**: 47 transitions tile correctly with neighbors; autotile rules pre-configured on output; style matches the example transitions.
- **Lands in**: S28.

### Tileset-from-description

Generate a full autotile-compatible tileset from a prompt. Uses grid-snap's gradient profiling to enforce tile geometry, then runs the Tile verb's transition generation.

- **Sources**: [`../research/grid-snap-quantize-techniques.md`](../research/grid-snap-quantize-techniques.md) § techniques 2–6; [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § S35.
- **Requirements**: output tileset has consistent step size on both axes; autotile rules attach automatically; style is set by reference sheet, not prompt alone.
- **Lands in**: S35.

### Sketch finishing

Artist sketches rough silhouettes or stick figures; AI refines to finished sprites in the project's learned style. Anchor-first principle keeps the result consistent with existing characters.

- **Sources**: [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § S36; [`../synthesis/ai-opportunity.md`](../synthesis/ai-opportunity.md) § style transfer.
- **Requirements**: artist accepts or rejects per frame; output respects palette; project style LoRA is used as default style reference.
- **Lands in**: S36.

### Project style learning (LoRA)

Per-project LoRA trained from existing layers. Trained in 15–30 minutes, registered as the default style reference for subsequent verbs. Extends to per-entity LoRA training when paired with a reference sheet.

- **Sources**: [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § S30 mapping; [`../synthesis/ai-opportunity.md`](../synthesis/ai-opportunity.md) § style transfer.
- **Requirements**: training runs against project data without manual labeling; LoRA persists in the project file; subsequent verbs pick it up as default.
- **Lands in**: S30, B10.

### Conversational editing

Natural language drives multi-step editor commands ("make this enemy look angrier, add a scar over the left eye, slow the walk to 8fps"). VLM plans the command sequence using the existing command vocabulary.

- **Sources**: [`../synthesis/ai-opportunity.md`](../synthesis/ai-opportunity.md) § three-mode architecture; [`scope.md`](scope.md) § verb set.
- **Requirements**: planned commands surface as a preview before execution; full plan undoes as a single entry; no command runs without artist accept.
- **Lands in**: S31.

### Motion-from-video

Extract motion timing and key poses from a reference video into the timeline. Pose estimation (DensePose / MediaPipe) + VLM for keyframe identification.

- **Sources**: [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § stage 5; [`../synthesis/ai-opportunity.md`](../synthesis/ai-opportunity.md) § reference matching; [`../synthesis/gaps.md`](../synthesis/gaps.md) § "Animation reference matching is manual".
- **Requirements**: artist owns every frame; AI delivers timing skeleton + rough silhouettes; reference video plays scrubbable next to the timeline.
- **Lands in**: S32.

### Audio-driven timing

Beat detection / lip sync from audio. Audio analysis is mostly classical; VLM handles lip-sync intent.

- **Sources**: [`scope.md`](scope.md) § verb set.
- **Requirements**: beat detection places frame markers on the active animation tag; lip-sync mode generates a mouth-shape cel sequence.
- **Lands in**: S34.

### Auto-mesh deformation

No-bones rigging via segmentation + view synthesis. Live2D-style without explicit bones — segment the sprite, derive a deformation rig automatically, expose parameter sliders.

- **Sources**: [`scope.md`](scope.md) § verb set; [`../research/project-library-research.md`](../research/project-library-research.md) § Live2D parameters.
- **Requirements**: deformation rig generates from a single sprite; parameter sliders drive deformation in real time; rig survives `.pixhaus` save.
- **Lands in**: S33.

### Animated sprite sheet from prompt

Two-stage LLM choreography (CHARACTER × CHOREOGRAPHY) producing a grid sprite sheet via fal.ai. Lifts the FalSprite prompts verbatim as MIT-attributed assets. A reference-sheet-aware variant of this verb uses the active sheet as the CHARACTER half.

- **Sources**: [`../research/falsprite-prior-art.md`](../research/falsprite-prior-art.md) § 1–3.
- **Requirements**: CHARACTER and CHOREOGRAPHY prompts are user-editable; grid math (cell-size floor division, pixel-perfect rendering) is deterministic; output snaps to palette.
- **Lands in**: S53. Verb at `ai/src/verbs/animated_sprite_sheet/`; UI form at `ui/src/verbs/animated-sprite-sheet/`; frame-grid playback primitives at `ui/src/timeline/frame-grid.ts` + `ui/src/timeline/use-animation-loop.ts`. FalSprite attribution at `LICENSES/falsprite-MIT.txt` and `LICENSES/NOTICE.txt`.

---

## Reference sheets — the anchor primitive

The unifying primitive that makes Pixhaus AI-native and consistent across every verb invocation. The reference sheet *is* the anchor mechanic — they are the same concept. Applies to characters, items, props, tilesets, environments, and any Custom entity that can hold an `anchor_reference_id`. Every AI verb invocation against an anchored entity passes the sheet's image as a backend reference, the sheet's extracted palette as a generation constraint, and (where backends support it) the per-entity LoRA. Consistency becomes mechanical, not hopeful.

A reference sheet is an authoritative document about an asset. For a Character it shows turnaround views, expressions, palette swatches, detail callouts, outfit variants. For an Item or Prop it shows multi-angle views, callouts, palette. For a Tileset it shows tile primitives, autotile preview, palette. The composition differs by kind; the underlying data structure is the same.

The user generates a sheet through an AI workflow (composition template, prompt, backend, 1–4 candidates), iterates with panel-scoped inpainting, approves a canonical variant, and from that point forward every verb invocation against the entity inherits consistency. Cross-entity reuse means a Goblin sheet can derive from the Hero's sheet ("like the Hero but green and shorter") with the Variant verb anchored on the source sheet for style continuity.

- **Sources**: [`../work/b10-reference-sheets.md`](../work/b10-reference-sheets.md) (the full spec); [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) § stages 2–4 (anchor-first methodology as the prior-art ground); [`../research/project-library-research.md`](../research/project-library-research.md) § Spine skins (variant-sharing precedent); [`../synthesis/prior-art.md`](../synthesis/prior-art.md) § "Anchor-first canonical pose".
- **Requirements**:
  - Sheet generation verb (`generate-reference-sheet`) produces 1–4 candidates per invocation.
  - Sheet iteration verb (`iterate-reference-sheet`) supports panel-scoped inpainting.
  - Approval flow promotes a variant to canonical; the previous canonical demotes to history.
  - All 14+ AI verbs accept an optional `anchor: Option<AnchorPayload>` and resolve it automatically from `Entity.anchor_reference_id`.
  - Composition templates ship for Character / Item / Tileset / Custom.
  - Sheet view UI renders the canonical sheet with panel overlay, asset info side panel, history strip, generate/refine controls.
  - Per-entity LoRA training is optional and extends Project Style Learning.
  - Apply-to scope: characters, items, props, tilesets, environments, custom — anything that can hold an anchor.
- **Lands in**: B9 (data model dependency), B10 (the full spec).

---

## Reference image overlays

A separate, simpler capability from the reference sheets above. The artist drags an arbitrary image onto the canvas (a photo, a sketch, a reference from elsewhere) and the editor renders it as a translucent overlay using Pixelorama's monochrome/overlay/clamp shader. Reference image overlays do not enter the export pipeline and do not drive AI generation; they exist only to help the artist eyeball proportions and composition while drawing.

- **Sources**: [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 37 (reference images with monochrome/overlay/clamp shader).
- **Requirements**: drag-and-drop loads any common image format; opacity and blend mode apply; multiple overlays per project; overlays toggle visibility per-canvas without affecting export.
- **Lands in**: S14.

---

## Output and engine handoff

### Unity importer

UPM package that imports `.pixhaus` files directly. Sprite sheets, animation tags, palette, tilemap all consume on the Unity side without sidecar JSON.

- **Sources**: [`scope.md`](scope.md) § engine target; [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 30 (plugin-registered formats).
- **Requirements**: importer installs via UPM URL; opens `.pixhaus` and `.aseprite`; round-trip preserves animation tags and palette.
- **Lands in**: B6, S39, S40.

### Sprite sheet plus JSON export

Aseprite-compatible JSON metadata so existing Unity / Godot / Phaser importers work day one. PNG sprite sheet packing with configurable algorithms.

- **Sources**: [`../synthesis/patterns.md`](../synthesis/patterns.md) § "Sprite sheet + JSON metadata is the engine handoff"; [`../research/aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 17, § 21.
- **Requirements**: emitted JSON parses with existing Aseprite importer packages without modification; frame rectangles, durations, tags, pivots all included.
- **Lands in**: S10.

### Animated GIF and WebP export

Worker-pool composition (FalSprite shape), per-frame palette quantization (grid-snap majority-vote downsampling), looped or once playback.

- **Sources**: [`../research/falsprite-prior-art.md`](../research/falsprite-prior-art.md) § 4; [`../research/grid-snap-quantize-techniques.md`](../research/grid-snap-quantize-techniques.md) § technique 7.
- **Requirements**: GIF export uses project palette; WebP export supports both lossless and lossy; frame durations honor timeline.
- **Lands in**: S11.

### TMX tilemap export

Tiled-compatible TMX export so existing Tiled-driven Unity / Godot importers keep working.

- **Sources**: [`scope.md`](scope.md) § engine target; [`../synthesis/gaps.md`](../synthesis/gaps.md) § "Tile autotile generation is still tedious".
- **Requirements**: TMX includes tile flips, autotile rule references, animated tile metadata; round-trips through Tiled without loss.
- **Lands in**: S12.

---

## Plugin and ecosystem

### Lua scripting

`mlua` bindings for in-editor scripting. Lua is the indie scripting language across Aseprite, Tiled, GameMaker, Defold, Roblox, LÖVE.

- **Sources**: [`../synthesis/patterns.md`](../synthesis/patterns.md) § "Lua is the indie scripting language"; [`scope.md`](scope.md) § scripting.
- **Requirements**: scripts have access to project data; scripts can register commands and panels; sandboxed file system access.
- **Lands in**: S38.

### WASM plugins

`extism` cross-language WASM plugins for verbs, file formats, and panels. Per Pixelorama's plugin manifest pattern with crash-detect-then-disable.

- **Sources**: [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 29 (manifest + crash-detect); [`scope.md`](scope.md) § plugins.
- **Requirements**: plugin manifest declares permissions; crashes auto-disable the offending plugin; plugins survive editor updates.
- **Lands in**: S37.

### Plugin-registered file formats

Plugins can register new file formats for the importer/exporter pipeline (Pixelorama's pattern).

- **Sources**: [`../research/pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 30.
- **Requirements**: registered formats appear in the open/save dialog; plugin failure does not crash the editor; per-format options pass through to the plugin.
- **Lands in**: S37.

---

## Open decisions

Five conflicts the dossiers surface that shape specific capabilities. Resolution belongs in [`../work/bedrock.md`](../work/bedrock.md) or the named stream brief — not here. Listed for product alignment.

| ID | Decision | Shapes |
| --- | --- | --- |
| D-01 | Project file format: MessagePack+zstd vs ZIP+JSON | Project file format above |
| D-02 | Palette shape: flat Vec vs sparse HashMap vs animated overlay | Sparse palette + Palette pages above (this PRD leans into both — sparse HashMap as the base with palette pages as the animated overlay) |
| D-03 | Cleanup verb scope: narrow vs full seven-step | Cleanup verb above (this PRD leans into the full seven-step) |
| D-04 | Inbetween modes: AI-only vs procedural vs hybrid | Inbetween verb above (this PRD leans into all three modes) |
| D-05 | Cel linking: pointer-share vs link-set IDs | Document model, Variant verb (this PRD leans into link-set IDs) |

Full counter-evidence and recommended resolution per row: [`../synthesis/prior-art.md`](../synthesis/prior-art.md) § "Open decisions".

## What we explicitly do not integrate

The dossiers document plenty we deliberately leave on the table:

- **Aseprite UI framework** ([`aseprite-prior-art.md`](../research/aseprite-prior-art.md) § 20) — MIT but skip. Solid + WebGL2 is our target; lifting `src/ui/` adds nothing.
- **Aseprite EULA-licensed application layer** — tool system, brush dynamics, scripting API, plugin model. Inspire-only; we reimplement.
- **OpenToonz centerline vectorization** ([`opentoonz-comparison.md`](../research/opentoonz-comparison.md) § "Centerline vectorization") — vector output is out of scope for a raster editor. Pixhaus is raster-only; vectorization would force a parallel data model we do not want.
- **Pixelorama splash dialog** ([`pixelorama-adoption.md`](../research/pixelorama-adoption.md) § 36) — D-tier but unnecessary.
- **Krita and PSD parsers** — deferred reference only. PSD import is nice-to-have but the chunk format is heavy; Krita is even further out.
- **Multi-user collaboration** — [`../synthesis/gaps.md`](../synthesis/gaps.md) names this as a real gap, but it is a CRDT engineering problem, not a prior-art port. Out of scope; revisit only with a clear use case.

## Success criteria

The integration is successful when:

1. Every capability above has a passing acceptance test traceable to its source dossier or gap.
2. An Aseprite-installed-base artist can open Pixhaus, import a `.aseprite` file, edit it with their existing muscle memory, run at least one AI verb (Cleanup or Inbetween), and export a sprite sheet that drops into their Unity project — all without consulting documentation for basic operations.
3. The five open decisions (D-01…D-05) are resolved in [`../work/bedrock.md`](../work/bedrock.md) before the streams that depend on them start.
4. No capability ships without its attribution discipline ([`../synthesis/prior-art.md`](../synthesis/prior-art.md) § "Attribution discipline") in place.
5. Every gap closed in [`../synthesis/gaps.md`](../synthesis/gaps.md) by an integration above is removed from the gaps file in the same PR that ships the closing capability.
