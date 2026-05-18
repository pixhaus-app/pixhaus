# Pixelorama adoption plan

## Context

Pixelorama (Orama Interactive, MIT, Godot 4.6) is a feature-rich pixel art editor with ~200 GDScript files, 33 shaders, and a 1.5k-line changelog shipping on a ~3-week cadence. It is on disk at `/Users/luismorales/project/pixhaus-app/Pixelorama`. Pixhaus is also MIT, so the two are license-compatible: we can adopt ideas, port algorithms to Rust, port shaders to WebGL2 GLSL ES 3.00, and vendor selected assets verbatim — provided we comply with MIT by preserving the upstream copyright, attributing derivative work, and shipping the upstream LICENSE text via `docs/THIRD_PARTY_LICENSES.md`.

A product-level survey of Pixelorama already lives at `docs/planning/pixel-art-editors/06-pixelorama.md`. This doc is the implementation-level companion: a triaged catalog naming each idea, classifying it by level of borrow (direct asset reuse, shader port, algorithm port, design adoption), and tying every item to a bedrock spec and stream.

Upstream commit pinned in `docs/THIRD_PARTY_LICENSES.md`: `b6dbb2b0bf8a8b04ed4a49d525cfec287ff9706b`. All Pixelorama file paths and line numbers in this doc are valid against that commit; re-pin and re-cite if we resync against a newer upstream.

## MIT-compliance mechanics

Future port PRs will follow these rules. They are codified once here so they do not get reinvented per stream.

1. **`docs/THIRD_PARTY_LICENSES.md` is the single source of attribution truth.** Each upstream project gets one section: URL, pinned commit, license text verbatim (including copyright), and the running list of vendored/ported/adopted items. Append to the list as PRs land; do not move the file silently.
2. **Header comments on every ported source file.** Standardized 4-line block at the top of any Rust, TS, or GLSL file whose logic traces to Pixelorama:
   ```rust
   // Ported from Pixelorama (MIT-licensed, Orama Interactive 2019-present).
   // Upstream: https://github.com/Orama-Interactive/Pixelorama/blob/<commit>/<path>
   // Original: <one-line summary of what the upstream file does>.
   // See docs/THIRD_PARTY_LICENSES.md for the full upstream copyright notice.
   ```
3. **Verbatim-copied assets** land in `assets/third-party/pixelorama/<group>/` with a sibling `LICENSE` file containing the upstream copyright and the full MIT text. No surprise vendoring elsewhere.
4. **Commit-message trailer on port PRs**: `Source: https://github.com/Orama-Interactive/Pixelorama/blob/<commit>/<path>`. Keeps `git log` self-explanatory.
5. **No license cross-contamination.** Pixhaus stays MIT. Pixelorama's MIT permits adoption. No CLA, no dual-licensing.
6. **No GDScript dependency.** Pixhaus does not link Godot. Every borrow is language- and runtime-translated; even GLSL → GLSL ES 3.00 requires editing. The borrow stays at the "MIT source code I read and used" level — the level MIT is designed for.

## How to read this

### The four borrow tiers

| Tier                    | Meaning                                                                            | Attribution rules                                                              |
| ----------------------- | ---------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| **A** — Asset reuse     | Vendor the file verbatim (PNG, theme tokens, palette).                             | Sibling `LICENSE` in vendored dir plus a `docs/THIRD_PARTY_LICENSES.md` entry. |
| **S** — Shader port     | Translate `.gdshader` → GLSL ES 3.00. Semantics preserved, syntax adjusted.        | Header comment on the ported file plus a `docs/THIRD_PARTY_LICENSES.md` entry. |
| **P** — Algorithm port  | Reimplement the logic in Rust or TS. Same algorithm, different language and types. | Header comment on the ported file plus a `docs/THIRD_PARTY_LICENSES.md` entry. |
| **D** — Design adoption | Take only the design idea or data shape; write fresh code.                         | Single line in `docs/THIRD_PARTY_LICENSES.md` under "adopted designs".         |

Every catalog entry below is tagged with one or more tiers in the heading.

### Per-entry template

Each entry follows the same beats:

- **Upstream.** Where in Pixelorama the idea lives.
- **What we adopt.** The Pixhaus-side design.
- **Why it matters.** The user or architecture win.
- **Data shape / pseudocode.** Rust types, GLSL uniforms, or algorithm sketch where non-obvious.
- **Integration.** Bedrock specs and streams that own delivery, plus prerequisites.
- **Attribution.** Concrete: which file gets the header comment, which line in `docs/THIRD_PARTY_LICENSES.md`.
- **Non-goals.** What we explicitly will not do here.

Skip beats that don't apply. Keep entries scannable.

## Group 1 — Project file format and core data model

Touches **B2** (core data model), **B3** (`.pixhaus` format), **S07** (`.pixhaus` native format).

### 1. ZIP container with `manifest.json` and binary payloads _(Tier D)_

- **Upstream.** `src/Autoload/OpenSave.gd` lines 292-640 (`open_pxo_file`, `save_pxo_file`). Pixelorama's `.pxo` is a ZIP archive with `data.json` (project metadata), `mimetype`, and per-cel image data at `image/<layer>/<frame>.png`, plus `tilesets/<i>/`, `audio/<i>/`, `scene/<i>.tscn`, `brushes/<i>` paths.
- **What we adopt.** Replace the planned MessagePack + zstd format with a ZIP archive of:
  ```
  pixhaus.json                  // manifest: version, fps, layer/frame/cel index
  image/<layer-id>/<frame-id>.png
  tilesets/<id>.json + tilesets/<id>/<tile-id>.png
  palettes/<id>.json
  brushes/<id>.png
  references/<id>.png
  audio/<id>.opus
  ```
- **Why it matters.** Cheap inspection (`unzip -p file.pixhaus pixhaus.json`), forward-extensible (new folders don't break old readers), and zstd-equivalent compression is available per-entry via deflate or store. The user-facing wins (diffable, portable, recoverable) are large; the engineering cost is tiny next to a bespoke binary format.
- **Data shape.**
  ```rust
  // pixhaus.json
  struct ProjectManifest {
      pixhaus_version: u32,        // schema version; increment on breaking changes
      app_version: String,         // informational
      size: [u32; 2],              // (w, h) in pixels
      fps: f32,
      color_mode: ColorMode,       // Rgba8 | Indexed
      layers: Vec<LayerEntry>,
      frames: Vec<FrameEntry>,
      animation_tags: Vec<AnimationTag>,
      palette_ids: Vec<PaletteId>,
      reference_image_ids: Vec<ReferenceImageId>,
      cameras: Vec<CameraState>,
      meta: BTreeMap<String, JsonValue>,
  }
  ```
- **Algorithm / pseudocode (read).**
  ```rust
  let reader = ZipReader::open(path)?;
  let manifest: ProjectManifest = serde_json::from_slice(&reader.read("pixhaus.json")?)?;
  ensure!(manifest.pixhaus_version <= CURRENT_PIXHAUS_VERSION, MigrationError);
  for entry in &manifest.frames { /* read image/<layer>/<frame>.png */ }
  // refuse to load any executable / script payload; whitelist known paths
  ```
- **Integration.** B3 (revise the format choice from MessagePack to ZIP), S07 (implementation). Prerequisite: agreement on the manifest schema. Migration: if any `.pixhaus` files exist from earlier iterations, write a one-shot converter before flipping the default writer.
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry: ZIP container layout informed by Pixelorama's `.pxo`.
- **Non-goals.** Round-tripping `.pxo` files. We can read them via the Aseprite/Krita/PSD-style parser path (see entry 31), not by adopting their schema.

### 2. Sparse palette as `HashMap<u16, PaletteColor>` _(Tier D)_

- **Upstream.** `src/Palette/Palette.gd:31-82`. Pixelorama palettes are dictionaries keyed by grid index; the grid can have gaps, and resize reindexes occupied slots into the new shape.
- **What we adopt.** Same shape in Rust:
  ```rust
  struct Palette {
      name: String,
      grid: (u16, u16),                       // (width, height) in slots
      colors: HashMap<u16, PaletteColor>,     // slot index → color
      comment: String,
      project_local: bool,
  }
  struct PaletteColor { rgba: [u8; 4], name: Option<String> }
  ```
- **Why it matters.** Pixel artists routinely leave gaps in palette grids (e.g., 8×8 grid with 47 colors arranged for visual grouping). A dense `Vec<Color>` forces gap-filling with sentinel transparent slots; the sparse map is simpler to serialize, simpler to reindex, and matches user mental model.
- **Integration.** B2, S02, S18.
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.

### 3. Indexed color mode via shadow index image alongside RGBA8 _(Tier D)_

- **Upstream.** `src/Classes/Project.gd:16, 235-236, 609-614`. Pixelorama stores indexed-mode cels as both an `indices_image` (single-channel) for canonical state and an RGBA8 image for display. Tools that don't need to know about palette indexing operate on the RGBA8 path.
- **What we adopt.**
  ```rust
  struct PixelCel {
      rgba: Image<Rgba8>,              // display buffer; always present
      indices: Option<Image<U8>>,      // present iff project.color_mode == Indexed
      palette: Option<PaletteId>,
  }
  ```
- **Why it matters.** Retro game workflows (NES/SNES/GameBoy palettes, demoscene constraints) need true indexed output: an export that reduces RGBA to indices via nearest-color quantization is lossy each time. Keeping indices canonical and deriving RGBA on the side preserves intent and lets tools that don't care (selection, transform, blur) operate on RGBA without bridging logic.
- **Edits.** Drawing in indexed mode writes the index buffer first; RGBA is regenerated from the palette. Palette edits invalidate RGBA but not indices.
- **Integration.** B2, S02. Prerequisite: palette identity stable across the project (palette IDs, not pointer-shared).
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.

### 4. Cel linking via link-set IDs _(Tier D)_

- **Upstream.** `src/Classes/Cels/BaseCel.gd:94-99`, `src/Classes/Project.gd:745+`. Pixelorama groups cels that share content into "link sets" so editing any member updates all of them, without pointer-sharing the underlying image.
- **What we adopt.**
  ```rust
  struct FrameEntry {
      duration_mul: f32,
      cels_by_layer: BTreeMap<LayerId, CelRef>,
  }
  enum CelRef {
      Owned(CelId),
      Linked { set_id: LinkSetId, hue_rotate: f32 },
  }
  ```
  An edit that writes to a `Linked` cel walks the link set and applies the same op to every member, captured as a single undo group.
- **Why it matters.** Idle animations reuse a single drawing across many frames; without cel linking, users either duplicate the image (and forget to update all copies) or refactor to a single-frame layer (losing animation context). The link-set ID approach round-trips through serialization cleanly — no `Arc<RwLock<Image>>` and no pointer-fix-up on load.
- **Integration.** B2, S05 (undo/redo command pattern), S19 (timeline panel).
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.
- **Non-goals.** Cross-project link sets. Link sets are project-local; copy/paste across projects breaks the link.

### 5. `SelectionMap` as an LA8 mask image _(Tier D)_

- **Upstream.** `src/Classes/SelectionMap.gd`, used at `src/Classes/Project.gd:97-104`. Selection state is a same-size 1-channel image; `alpha > 0` is selected.
- **What we adopt.**
  ```rust
  struct SelectionMap {
      pixels: Vec<u8>,                  // size = project.size.x * project.size.y
      size: (u32, u32),
      cached_bbox: Option<URect>,       // recomputed lazily
  }
  impl SelectionMap {
      fn union(&self, other: &Self) -> Self      { per_pixel(|a, b| a.max(b)) }
      fn intersect(&self, other: &Self) -> Self  { per_pixel(|a, b| a.min(b)) }
      fn subtract(&self, other: &Self) -> Self   { per_pixel(|a, b| a.saturating_sub(b)) }
      fn feather(&self, radius: u32) -> Self     { gaussian_blur(self, radius) }
  }
  ```
- **Why it matters.** A polygon-based selection forces every tool to deal with hit-testing against arbitrary geometry. A raster mask is constant-cost regardless of selection complexity, GPU-friendly (feeds compositing as one extra texture), and unifies hand-painted selection ("paint to add/subtract") with shape-based selection — they all write to the same mask.
- **Integration.** B2, S03 (selection algorithms), S16 (selection UI).
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.

### 6. Frame duration as float multiplier of project FPS _(Tier D)_

- **Upstream.** `src/Classes/Frame.gd:16-17`. Pixelorama stores `duration: float`; playback time per frame is `duration * (1.0 / fps)`.
- **What we adopt.** `duration_mul: f32` on each frame entry. Effective time = `duration_mul / project.fps` seconds. Default 1.0.
- **Why it matters.** Letting users specify absolute milliseconds per frame creates a re-timing trap: doubling the playback speed requires editing every frame. The multiplier-of-FPS form lets users set "this frame holds 4× as long" once and re-time the whole animation by adjusting project FPS.
- **Integration.** B2, S19.
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.

### 7. Animation tags as `{name, from, to}`; direction picked at export time _(Tier D)_

- **Upstream.** `src/Classes/AnimationTag.gd`, `src/Autoload/Export.gd:5` (`AnimationDirection` enum). Tags carry name, color, and frame range; direction (forward, reverse, ping-pong) is a per-export setting, not part of the tag.
- **What we adopt.**
  ```rust
  struct AnimationTag {
      name: String,
      range: RangeInclusive<u32>,       // frame indices
      ui_color: Rgba8,
      user_data: String,
  }
  enum PlayDirection { Forward, Reverse, PingPong }
  ```
- **Why it matters.** The same `walk` tag may export as forward for a side-view walk cycle and ping-pong for a hover idle. Forcing direction into the tag pushes users into duplicating tags. Keeping it at export time is one less knob in the timeline UI and one less migration when a tag's purpose changes.
- **Integration.** B2, S19, S11 (GIF/WebP export).
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.

### 8. Non-destructive layer effects as `Vec<LayerEffect>` per layer _(Tier D)_

- **Upstream.** `src/Classes/Layers/BaseLayer.gd:effects`, applied at render time in `src/Autoload/DrawingAlgos.gd:84`. Each layer carries a stack of effects (outline, drop shadow, gradient map, palettize, …), composed top-down without writing back to the cel.
- **What we adopt.**
  ```rust
  enum LayerEffect {
      Outline   { color: Rgba8, thickness: u8, conn: Connectivity },
      DropShadow { color: Rgba8, offset: (i32, i32), blur: u32 },
      GradientMap { gradient: GradientId, dither: DitherMode },
      Palettize { palette: PaletteId, dither: DitherMode },
      Brightness { delta: f32 },
      // …
  }
  struct LayerEntry {
      // …
      effects: Vec<LayerEffect>,        // applied in order at composite time
  }
  ```
- **Why it matters.** Lets users iterate on effects without losing the source pixels, and gives Pixhaus a clean home for AI verbs that produce overlay-style results (e.g., a generated outline, a generated palette mapping). The verb runtime can register new `LayerEffect` variants.
- **Integration.** B2, S17 (layer panel), S21 (verb runtime — AI verbs can produce overlay effects).
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.
- **Non-goals.** A general adjustment-layer object (Photoshop-style). Layer effects belong to a layer; they are not standalone layers.

## Group 2 — Drawing, selection, transform algorithms

Touches **S01** (pixel buffer / blend modes), **S02** (color/palette), **S03** (selection), **S04** (transforms).

### 9. Allegro scanline flood fill _(Tier P)_

- **Upstream.** `src/Tools/DesignTools/Bucket.gd` lines 230-490 (cf. the `_compute_segments_for_image` and `_flood_line_around_point` functions); same algorithm reused in `src/Tools/SelectionTools/MagicWand.gd:67-90`. Originally from Allegro 4.2.1 by Shawn Hargreaves.
- **What we adopt.** Single `flood_fill` function in Rust used by bucket fill, magic-wand selection, and tilemap bucket fill (operating on tile indices).
- **Why it matters.** Scanline flood fill is O(n) pixel visits, non-recursive (no stack overflow on large fills), and friendly to bounds-checking and cancellation. The recursive 4-way variant is the canonical naive answer; the scanline form is the canonical correct answer.
- **Data shape / pseudocode.**

  ```rust
  struct Segment {
      flooding: bool,
      todo_above: bool,
      todo_below: bool,
      left: u32,
      right: u32,
      y: u32,
  }

  fn flood_fill(
      image: &Image,
      seed: (u32, u32),
      match_fn: impl Fn(Rgba8) -> bool,
      mut visit: impl FnMut(u32, u32),
  ) {
      let mut stack: Vec<Segment> = vec![seed_segment(image, seed, &match_fn)];
      while let Some(seg) = stack.pop() {
          for x in seg.left..=seg.right { visit(x, seg.y); }
          if seg.todo_above && seg.y > 0 {
              extend_above(image, &seg, &match_fn, &mut stack);
          }
          if seg.todo_below && seg.y + 1 < image.height() {
              extend_below(image, &seg, &match_fn, &mut stack);
          }
      }
  }
  ```

  `match_fn` is `|c| similar_colors(c, target, tol)` for fill, `|c| c == target` for exact-match selection, or `|t| t == target_tile` for tilemap fill.

- **Integration.** S01 (paint ops), S03 (magic wand), S06 (tilemap bucket fill). Lives in `core/src/flood.rs`.
- **Attribution.** Header comment in `core/src/flood.rs`; "ported algorithms" line in `docs/THIRD_PARTY_LICENSES.md`.

### 10. Color similarity via squared distance _(Tier P)_

- **Upstream.** `src/Autoload/DrawingAlgos.gd:667`:
  ```gdscript
  func similar_colors(c1: Color, c2: Color, tol := 0.392157) -> bool:
      return c1.distance_squared_to(c2) < tol * tol
  ```
- **What we adopt.**
  ```rust
  pub fn similar_colors(a: Rgba8, b: Rgba8, tol: f32) -> bool {
      let to_f = |c: u8| (c as f32) / 255.0;
      let dr = to_f(a[0]) - to_f(b[0]);
      let dg = to_f(a[1]) - to_f(b[1]);
      let db = to_f(a[2]) - to_f(b[2]);
      let da = to_f(a[3]) - to_f(b[3]);
      dr*dr + dg*dg + db*db + da*da < tol * tol
  }
  ```
- **Why it matters.** Used by flood fill, magic wand, color select, color replace, and palette quantize. One helper, one tolerance slider, consistent UX across tools. Default `tol = 0.0015` (~0.04 distance, ~10/255 per channel) matches the upstream default.
- **Integration.** S01, S02, S03. Lives in `core/src/color.rs`.
- **Attribution.** Header comment in `core/src/color.rs`; "ported algorithms" line in `docs/THIRD_PARTY_LICENSES.md`.

### 11. Three-mode bucket fill: AREA / SAME_COLOR / SELECTION_ONLY _(Tier D)_

- **Upstream.** `src/Tools/DesignTools/Bucket.gd:241, 317`. Same tool, three modes; SAME_COLOR sweeps the whole image via a shader, SELECTION_ONLY masks to the active selection, AREA runs scanline flood fill.
- **What we adopt.** Identical three-mode toggle in the brush-engine UI for the bucket tool.
- **Why it matters.** Pixel artists use SAME_COLOR for global color-replace tasks (changing a character's shirt across all frames in one click). Forcing them to use "Edit → Replace color" hides a frequent operation behind a menu.
- **Integration.** S15 (brush engine UI), S01 (paint ops). Algorithm reuse: entry 9 for AREA, entry 10 for SAME_COLOR predicate.
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.

### 12. Pattern fill as a tiling shader _(Tier S)_

- **Upstream.** `src/Shaders/PatternFill.gdshader`. Bucket fill can paint a tiled image pattern instead of a solid color; offset `(x, y)` controls tiling origin.
- **What we adopt.** Port the shader to GLSL ES 3.00:
  ```glsl
  #version 300 es
  precision highp float;
  uniform sampler2D u_pattern;
  uniform vec2 u_pattern_size;
  uniform vec2 u_offset;
  uniform sampler2D u_target_mask;     // result of flood fill: 1 = fill, 0 = skip
  in vec2 v_uv;
  out vec4 o_color;
  void main() {
      float m = texture(u_target_mask, v_uv).r;
      if (m < 0.5) discard;
      vec2 pat_uv = mod(v_uv * u_target_size + u_offset, u_pattern_size) / u_pattern_size;
      o_color = texture(u_pattern, pat_uv);
  }
  ```
- **Why it matters.** GPU-side tiling is one draw call regardless of fill size; a CPU sampling loop scales with pixel count.
- **Integration.** S01, S14 (canvas viewport). Lives in `ui/src/shaders/pattern-fill.glsl`.
- **Attribution.** Header comment in the shader file; "ported shaders" line in `docs/THIRD_PARTY_LICENSES.md`.

### 13. Midpoint ellipse rasterizer _(Tier P)_

- **Upstream.** `src/Autoload/DrawingAlgos.gd:147-208`. Generates 4-way symmetric ellipse points in `O(a + b)` time (Bresenham-style midpoint algorithm).
- **What we adopt.** A `rasterize_ellipse(bounds: URect, thickness: u8) -> Vec<(u32, u32)>` function. For `thickness > 1`, draw inner and outer ellipses and fill between them.
- **Why it matters.** Naive ellipse drawing (sample `cos`/`sin`) produces gaps or duplicates at low resolutions. Midpoint integer math produces exactly one pixel per row/column edge — the pixel-perfect result.
- **Pseudocode (sketch).**
  ```rust
  // For ellipse with semi-axes (a, b) centered at (cx, cy):
  // d1 = b² - a²·b + 0.25·a²
  // Step in x until 2·b²·x ≥ 2·a²·y, then step in y.
  // Mirror across both axes.
  ```
- **Decision: thickness > 1 means center-of-stroke, not edge-of-stroke.** A 3-pixel-thick ellipse drawn inside a 16×16 box places the stroke center on the box edge, so the visible width is centered on the intended boundary. Document the decision in the UI.
- **Integration.** S01, S15. Lives in `core/src/rasterize.rs`.
- **Attribution.** Header comment in `core/src/rasterize.rs`; "ported algorithms" line in `docs/THIRD_PARTY_LICENSES.md`.

### 14. Seven pixel-art rotation algorithms _(Tier S)_

- **Upstream.** `src/Shaders/Effects/Rotation/{SmearRotxel,cleanEdge,OmniScale,NearestNeighbour}.gdshader`, with CPU-side dispatch in `src/Autoload/DrawingAlgos.gd:297+`. Enum: ROTXEL_SMEAR, CLEANEDGE, OMNISCALE, NNS, NN, ROTXEL, URD.
- **What we adopt.** Port the four primary algorithms (RotxelSmear, CleanEdge, OmniScale, NN) as GLSL ES 3.00 fragment shaders; CPU-side dispatch picks one based on a user setting on the transform tool.
- **Why it matters.** Naive nearest-neighbor rotation produces visible diagonal artifacts at non-90° angles — exactly the situation pixel artists rotate sprites in (28°, 45°, 60°). RotSprite-family algorithms preserve diagonals, anti-alias minimally, and keep silhouettes intact. Pixhaus needs at least three options because none of them is universally best (RotxelSmear is great at small angles; CleanEdge wins at 45°; OmniScale handles odd sizes).
- **Integration.** S04 (transform operations), S14 (canvas viewport for live preview). Lives in `ui/src/shaders/rotate-{rotxel,cleanedge,omniscale,nn}.glsl` with a Rust-side dispatcher in `core/src/transform/rotate.rs`.
- **Attribution.** Header comment on each shader file; "ported shaders" line in `docs/THIRD_PARTY_LICENSES.md`.
- **Non-goals.** The NNS, ROTXEL, and URD variants. NNS is a smoothed NN; smoothing pixel art usually loses the point. ROTXEL is a precursor to RotxelSmear. URD is undocumented in upstream. Skip until requested.

### 15. Scale3X upscale path _(Tier P)_

- **Upstream.** `src/Autoload/DrawingAlgos.gd:242+`. 8-neighbor edge-pattern lookup; outputs a 3× region per source pixel based on the local edge pattern.
- **What we adopt.** A `scale3x(src: &Image) -> Image` function emitting a 3×-resolution image. Used for export-time scale previews ("how does this look at 3×?") and as an option in the Unity importer's auto-scale step.
- **Why it matters.** NN doubling produces blocky pixels (correct but coarse); bilinear produces blurry pixels (wrong). Scale3X (and Eagle, hq2x, etc.) preserve edges while smoothing diagonals — a third option between blocky and blurry.
- **Pseudocode.**
  ```rust
  // For each source pixel P at (x, y), examine 4-neighbors A, B, C, D (top/right/bottom/left).
  // Output 3×3 tile based on diagonal-edge patterns; see DrawingAlgos.gd:242 for the exact lookup.
  ```
- **Integration.** S04, S10 (sprite-sheet export). Lives in `core/src/scale.rs`.
- **Attribution.** Header comment in `core/src/scale.rs`; "ported algorithms" line in `docs/THIRD_PARTY_LICENSES.md`.

### 16. Transformation handles as a non-destructive floating overlay _(Tier D)_

- **Upstream.** `src/UI/Canvas/TransformationHandles.gd`. The selected pixels become a "floating" overlay image with a `Mat3` transform; gizmos drive the matrix; on confirm, the result is composited back; on cancel, it is discarded.
- **What we adopt.** Same approach for the transform tool. The floating overlay is a separate texture rendered on top of the canvas; the underlying cel is not mutated until confirm.
- **Why it matters.** Live-preview transforms without rasterize-on-every-handle-drag. Avoids quality loss from repeated rasterize/sample cycles. Undo is one operation (confirm), not one-per-drag.
- **Data shape.**
  ```rust
  struct FloatingSelection {
      source_cel_id: CelId,
      source_bbox: URect,
      pixels: Image<Rgba8>,             // detached copy at start of transform
      transform: Mat3,                  // affine
      interpolation: TransformInterpolation,
  }
  ```
- **Integration.** S04, S14, S16 (selection / transform UI).
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.

## Group 3 — Layer compositing and effects pipeline

Touches **S01**, **S14**, **S17**.

### 17. Single-pass multi-layer compositor with `sampler2DArray` + metadata texture _(Tier S, D)_

- **Upstream.** `src/Shaders/BlendLayers.gdshader` + `src/Shaders/CanvasCommon.gdshaderinc`. Pixelorama packs all layer images into a `Texture2DArray`, per-layer state (blend mode index, opacity, clipping-mask flag, origin) into a small metadata texture, and runs a single fragment shader that loops over layers.
- **What we adopt.** Same approach in WebGL2. WebGL2 supports `sampler2DArray` (GLSL ES 3.00, `#version 300 es`) and integer texture sampling via `texelFetch`.
- **Why it matters.** N draw calls (one per layer) scales poorly past ~20 layers. One draw call with a texture array stays cheap up to the array layer count limit (≥256 on real-world devices). The metadata texture is a 4×N or 8×N RGBA texture, cheap to update on layer changes.
- **Uniform packing.**
  ```glsl
  // Per-layer metadata, packed as RGBA8 rows in u_layer_meta (texelFetch by layer index):
  // texel.r = blend_mode_index (0..23)
  // texel.g = opacity (0..255)
  // texel.b = flags (bit0 = clipping_mask, bit1 = pass_through_group, bit2 = visible)
  // texel.a = z_index (signed offset, biased by 128)
  ```
- **GLSL ES 3.00 differences from Godot's GDShader to call out in the port:**
  - `uniform sampler2DArray u_layers;` — supported but requires `#version 300 es` and `precision highp sampler2DArray;`.
  - `texelFetch(u_layer_meta, ivec2(0, layer_idx), 0)` — supported.
  - No `hint_color_no_alpha` or similar Godot-specific hints.
  - Loops over `int` layer counts only when bounded by a uniform with constant max; WebGL2 won't allow truly dynamic loop counts on all drivers.
- **Integration.** S01 (blend mode formulas; see entry 18), S14 (viewport plumbing). Lives in `ui/src/shaders/compose-layers.glsl`.
- **Attribution.** Header comment on the shader file; "ported shaders" line in `docs/THIRD_PARTY_LICENSES.md`.

### 18. 24 blend modes by formula table, not by implementation file _(Tier P)_

- **Upstream.** Enum at `src/Classes/Layers/BaseLayer.gd:15-37`, implementations in `src/Shaders/CanvasCommon.gdshaderinc`. List: Normal, Erase, Darken, Multiply, Color Burn, Linear Burn, Lighten, Screen, Color Dodge, Add, Overlay, Soft Light, Hard Light, Difference, Exclusion, Subtract, Divide, Hue, Saturation, Color, Luminosity, plus Pass-Through (group-only).
- **What we adopt.** The same 22 source-over modes plus Pass-Through, but cite the **PDF/SVG color-component specs** as the source-of-truth rather than the Godot shader code. Pixelorama's implementations follow those specs.
- **Why it matters.** Blend-mode bugs are a real interop hazard with Aseprite, Krita, PSD. Citing the spec (W3C SVG Compositing module, PDF 1.7 spec section 7.2.5) means our blend modes round-trip cleanly. The Pixelorama shaders are a useful reference for the integer-math edge cases (HSL modes use a luminosity-preserving algorithm with three helper functions: `lum`, `set_lum`, `clip_color`).
- **HSL-mode helpers (paraphrased from PDF/SVG, matching the Pixelorama implementations):**
  ```glsl
  float lum(vec3 c) { return dot(c, vec3(0.3, 0.59, 0.11)); }
  vec3 set_lum(vec3 c, float l) {
      vec3 d = c + (l - lum(c));
      return clip_color(d);
  }
  vec3 clip_color(vec3 c) {
      float l = lum(c), mn = min3(c), mx = max3(c);
      if (mn < 0.0) c = l + ((c - l) * l) / (l - mn);
      if (mx > 1.0) c = l + ((c - l) * (1.0 - l)) / (mx - l);
      return c;
  }
  ```
- **Integration.** S01 (blend mode core), S14 (shader). Lives alongside entry 17.
- **Attribution.** Header comment in the compositor shader; "ported algorithms" line in `docs/THIRD_PARTY_LICENSES.md`.

### 19. Group layers with optional Pass-Through blending _(Tier D)_

- **Upstream.** `src/Classes/Layers/GroupLayer.gd`. Groups normally composite children-then-blend; with Pass-Through, children blend directly onto the parent canvas (so adjustment-like effects on children apply to the parent).
- **What we adopt.** A `pass_through: bool` flag on group layers. Default `false` (composite-then-blend). Single bit in layer metadata.
- **Why it matters.** Lets users organize layers without paying the "blend mode is now sandwiched" cost. Photoshop ships pass-through as the default for groups; Pixelorama makes it opt-in. We follow Pixelorama because pixel-art folders are usually content-organization, not adjustment-stacks.
- **Integration.** B2, S17.
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.

### 20. Clipping masks as "layer below acts as alpha mask" _(Tier D)_

- **Upstream.** `src/Classes/Layers/BaseLayer.gd:clipping_mask`. A flag on the upper layer says "multiply my alpha by the lower layer's alpha."
- **What we adopt.** Same: one bit in layer metadata. The compositor multiplies the layer's output alpha by the alpha of the next-lower visible layer when the bit is set.
- **Why it matters.** No separate mask asset, no extra data path, no UX confusion about which layer "owns" the mask. Trivial to explain to new users: "this layer is clipped to the one below."
- **Integration.** B2, S17.
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.
- **Non-goals.** True layer masks (a separate per-layer alpha asset). Worth doing as a follow-up; not in initial scope.

### 21. All filters as fragment shaders against the cel image _(Tier S)_

- **Upstream.** `src/Shaders/Effects/` — 15 files: BrightnessContrast, ColorCurves, ConvolutionMatrix, Desaturate, DropShadow, GaussianBlur, Gradient, GradientMap, HSV, IndexMap, Invert, OffsetPixels, OutlineInline, Palettize, Pixelize, Posterize.
- **What we adopt.** Port each to GLSL ES 3.00. One shader per effect. Each gets a small uniform block; the host code parses tool-options UI and uploads the uniforms.
- **Mapping (upstream → Pixhaus path):**
  | Upstream | Pixhaus |
  |---|---|
  | `BrightnessContrast.gdshader` | `ui/src/shaders/effects/brightness-contrast.glsl` |
  | `ColorCurves.gdshader` | `ui/src/shaders/effects/color-curves.glsl` |
  | `ConvolutionMatrix.gdshader` | `ui/src/shaders/effects/convolution-matrix.glsl` |
  | `Desaturate.gdshader` | `ui/src/shaders/effects/desaturate.glsl` |
  | `DropShadow.gdshader` | `ui/src/shaders/effects/drop-shadow.glsl` |
  | `GaussianBlur.gdshader` | `ui/src/shaders/effects/gaussian-blur.glsl` |
  | `Gradient.gdshader` | `ui/src/shaders/effects/gradient.glsl` |
  | `GradientMap.gdshader` | `ui/src/shaders/effects/gradient-map.glsl` |
  | `HSV.gdshader` | `ui/src/shaders/effects/hsv.glsl` |
  | `IndexMap.gdshader` | `ui/src/shaders/effects/index-map.glsl` |
  | `Invert.gdshader` | `ui/src/shaders/effects/invert.glsl` |
  | `OffsetPixels.gdshader` | `ui/src/shaders/effects/offset.glsl` |
  | `OutlineInline.gdshader` | `ui/src/shaders/effects/outline.glsl` |
  | `Palettize.gdshader` | `ui/src/shaders/effects/palettize.glsl` |
  | `Pixelize.gdshader` | `ui/src/shaders/effects/pixelize.glsl` |
  | `Posterize.gdshader` | `ui/src/shaders/effects/posterize.glsl` |
- **Why it matters.** GPU effects on a 4k×4k canvas run at interactive rates; CPU loops do not. Effect parameter tweaking with a slider needs <16ms-per-frame turnaround.
- **Tests.** Each effect ships an `image-compare` snapshot test against a fixed input PNG and a known-good output PNG. Threshold tight (≤1% per-pixel delta) because pixel art has no anti-aliasing slack.
- **Integration.** S01, S14, S17. Lives in `ui/src/shaders/effects/`.
- **Attribution.** Header comment on each shader file; "ported shaders" line in `docs/THIRD_PARTY_LICENSES.md` (one line per shader is fine; a single grouped line is also acceptable).

### 22. Bayer dither matrices precomputed as PNG assets _(Tier A)_

- **Upstream.** `assets/dither-matrices/bayer{2,4,8,16}.png`. 1-channel PNGs encoding the standard 2×2, 4×4, 8×8, 16×16 Bayer threshold matrices. Used by gradient and posterize shaders for ordered dithering.
- **What we adopt.** **Direct verbatim vendoring** — copy the four PNGs into `assets/third-party/pixelorama/dither/` along with a sibling `LICENSE` file containing the Pixelorama copyright and MIT text.
- **Why it matters.** The matrices are mathematical constants (Bayer's 1973 paper); regenerating them produces the same bits. Vendoring saves a code-side asset-generation step and makes the dither encoding immediately inspectable. The user-customization angle is a bonus: dropping additional PNGs alongside means designers can experiment with non-Bayer ordered dithering without code changes.
- **Integration.** S01 (effect shaders that sample the matrices), prerequisite for entries in 21 that use dithering (gradient, posterize, palettize).
- **Attribution.** Sibling `LICENSE` file in `assets/third-party/pixelorama/dither/LICENSE`; "vendored assets" line in `docs/THIRD_PARTY_LICENSES.md`.
- **Non-goals.** Vendoring this in _this_ PR. The adoption plan flags it; the actual copy lands in the PR that introduces the gradient/posterize shaders.

## Group 4 — Animation, timeline, tilemap

Touches **B2**, **S06**, **S19**, **S20**.

### 23. Onion skin with `opacity = base / frame_distance` and red/blue tinting _(Tier P)_

- **Upstream.** `src/UI/Canvas/OnionSkinning.gd:29-41`. Past frames tinted blue, future frames tinted red; per-frame opacity falls off as `base / frame_distance`.
- **What we adopt.**
  ```rust
  struct OnionSkinConfig {
      past_count: u8,
      future_count: u8,
      base_opacity: f32,           // 0..1, e.g. 0.5
      tint: OnionSkinTint,         // None | RedBlue { past: Rgba8, future: Rgba8 }
  }
  // Render frame F at distance d (d > 0 past, d < 0 future, abs(d) <= past/future_count)
  // with opacity = base_opacity / abs(d).
  ```
  Per-layer `ignore_onion: bool` flag excludes HUD-style layers from the preview.
- **Why it matters.** Linear falloff (`base / d`) is gentler than exponential (`base * 0.5^d`) at high frame counts — the 3rd frame back is still visible enough to read. Tinting (Pixelorama uses red for future, blue for past; we follow) makes direction obvious at a glance.
- **Integration.** S19. Lives in `ui/src/timeline/onion-skin.ts` for state and a shader pass for render.
- **Attribution.** Header comment in the onion-skin module; "ported algorithms" line in `docs/THIRD_PARTY_LICENSES.md`.

### 24. Tile cell = `{ index, flip_h, flip_v, transpose }` _(Tier D)_

- **Upstream.** `src/Classes/Cels/CelTileMap.gd:86` — `class Cell { index: int; flip_h, flip_v: bool; transpose: bool }`.
- **What we adopt.**
  ```rust
  #[derive(Copy, Clone)]
  struct TileCell(u32);
  impl TileCell {
      const FLIP_H:    u32 = 1 << 29;
      const FLIP_V:    u32 = 1 << 30;
      const TRANSPOSE: u32 = 1 << 31;
      pub fn new(index: u32, flip_h: bool, flip_v: bool, transpose: bool) -> Self {
          let mut v = index & 0x1FFFFFFF;
          if flip_h    { v |= Self::FLIP_H; }
          if flip_v    { v |= Self::FLIP_V; }
          if transpose { v |= Self::TRANSPOSE; }
          Self(v)
      }
      pub fn index(self) -> u32 { self.0 & 0x1FFFFFFF }
      pub fn flip_h(self) -> bool { self.0 & Self::FLIP_H != 0 }
      pub fn flip_v(self) -> bool { self.0 & Self::FLIP_V != 0 }
      pub fn transpose(self) -> bool { self.0 & Self::TRANSPOSE != 0 }
  }
  ```
  Three orientation bits + 29-bit index = all 8 orientations of up to 512M tiles in one `u32`.
- **Why it matters.** Tilemaps with even modest tile counts (256 tiles × 256×256 grid = 65k cells) become large fast. Packing into a `u32` per cell keeps the serialized footprint tight and the in-memory layout cache-friendly.
- **Integration.** B2, S06, S20.
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.

### 25. TileSetCustom: tile-shape parameter + offset axis _(Tier D)_

- **Upstream.** `src/Classes/Cels/CelTileMap.gd:44-66, 208-241`. One `TileSetCustom` handles square, isometric, hex-pointy-top, and hex-flat-top layouts via a `tile_shape` parameter and a `tile_offset_axis` (which row/column gets the half-step offset for hex).
- **What we adopt.**

  ```rust
  enum TileShape { Square, Isometric, HexPointy, HexFlat }
  enum HexOffsetAxis { OddRow, EvenRow, OddCol, EvenCol }

  struct TileSet {
      tile_size: (u32, u32),
      shape: TileShape,
      hex_offset: Option<HexOffsetAxis>,  // Some(...) iff shape ∈ {HexPointy, HexFlat}
      tiles: Vec<TileEntry>,
  }
  ```

- **Why it matters.** Removes the future "we need a separate isometric editor" problem before it appears. Cell-to-pixel math switches on `shape`; the rest of the tilemap stack (TileCell, autotile, bucket fill) is shape-agnostic.
- **Integration.** B2, S06, S20.
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.

### 26. Autotile via per-tile peering bitmask _(Tier P)_

- **Upstream.** `src/Classes/Cels/CelTileMap.gd:14-31, 225`. Each tileset tile carries a neighbor signature; placement looks up neighbors and picks the matching tile variant.
- **What we adopt.** A 4-bit (rook) or 8-bit (queen) peering field per tile.
  ```rust
  struct TileEntry {
      image: TileImageId,
      peering: u8,            // bits set = "this tile expects a matching neighbor in this direction"
      family: AutotileFamily, // groups variants of the same logical tile
  }
  // Rook (4 bits): N=0x1, E=0x2, S=0x4, W=0x8
  // Queen (8 bits): adds NE=0x10, SE=0x20, SW=0x40, NW=0x80
  fn pick_autotile(family: AutotileFamily, neighbor_mask: u8, set: &TileSet) -> TileId {
      // exact match first; fall back to highest-bit-count match if no exact tile exists
  }
  ```
- **Why it matters.** Manual tile placement is fine for sparse decoration; auto-tile is the only sane workflow for terrain. Forty-seven-tile rook autotile sets are standard among pixel-art tilesets; making the format speak the standard avoids requiring tileset authors to do bit-magic.
- **Integration.** S06, S20. Lives in `core/src/tilemap/autotile.rs`.
- **Attribution.** Header comment in `core/src/tilemap/autotile.rs`; "ported algorithms" line in `docs/THIRD_PARTY_LICENSES.md`.

### 27. Smart-slice spritesheet import _(Tier P)_

- **Upstream.** `src/UI/Dialogs/ImportPreviewDialog.gd:smart_slice`. Detects sprite-sheet frame boundaries from transparency: flood-fill the alpha=0 background, label connected non-transparent regions, snap detected bboxes to a grid if they align.
- **What we adopt.** A `detect_frames(image: &Image, opts: SmartSliceOpts) -> Vec<URect>` function.
- **Why it matters.** Importing sprite sheets is the entry point for half of all pixel-art projects; forcing the user to manually enter grid dimensions is hostile when the answer is visible in the alpha channel. Smart slice also feeds tileset import (entry 25) and Aseprite slice round-trip (entry 28).
- **Algorithm.**
  1. Flood-fill the alpha=0 region from any corner (entry 9). Any non-flooded pixel is foreground.
  2. Connected-component label the foreground (4-connectivity).
  3. Compute the bounding box of each component.
  4. If component bboxes are similar-sized and arranged in a grid, snap to grid: extract `cols × rows`, `frame_w × frame_h`, `gap_x`, `gap_y`.
  5. Otherwise return the bboxes as-is (variable-size frames).
- **Integration.** S08 (`.aseprite` slice support), tileset import (S06), generic sprite-sheet import (S10-related). Lives in `core/src/import/smart_slice.rs`.
- **Attribution.** Header comment in `core/src/import/smart_slice.rs`; "ported algorithms" line in `docs/THIRD_PARTY_LICENSES.md`.

## Group 5 — Plugin and file-format compatibility

Touches **B5** (verb plugin protocol), **B7** (Aseprite format spec), **S08** (`.aseprite` I/O), **S37** (plugin loader), **S38** (Lua bindings).

### 28. Aseprite parser as a chunk-by-chunk state machine _(Tier P)_

- **Upstream.** `src/Classes/SoftwareParsers/AsepriteParser.gd` (578 lines). Reads `.ase`/`.aseprite`. Validates magic number (0xA5E0), iterates frames, iterates chunks within each frame, dispatches on chunk type.
- **What we adopt.** Same chunk-dispatch shape in Rust, located at `io/src/aseprite/mod.rs`. Chunk types we handle initially:
  | Chunk type | ID | Notes |
  |---|---|---|
  | Palette (old, 256) | 0x0004 | Rare in modern files; read for back-compat. |
  | Palette (old, 64) | 0x0011 | Same. |
  | Layer | 0x2004 | Name, flags, blend mode, opacity, user data. |
  | Cel | 0x2005 | Compressed image data; per-frame, per-layer. |
  | Cel extra | 0x2006 | Precise position/size (rare). |
  | Color profile | 0x2007 | sRGB / ICC; track for color-accurate import. |
  | External files | 0x2008 | Reference assets — we may not honor these in v1. |
  | Mask | 0x2016 | Deprecated; skip. |
  | Path | 0x2017 | Deprecated; skip. |
  | Tags | 0x2018 | Animation tags. |
  | Palette | 0x2019 | Modern palette format. |
  | User data | 0x2020 | Arbitrary key/value. |
  | Slice | 0x2022 | Sprite-sheet slices; feeds smart-slice (entry 27). |
  | Tileset | 0x2023 | Tileset tiles. |
- **Blend-mode mapping (Aseprite 18 modes → Pixhaus 24 modes).** Most map directly; document the three that don't round-trip:
  - Aseprite `Addition` → Pixhaus `Add`. Round-trips.
  - Aseprite `Subtract` → Pixhaus `Subtract`. Round-trips.
  - Aseprite `Divide` → Pixhaus `Divide`. Round-trips.
  - Pixhaus `Color Burn` / `Linear Burn`: Aseprite has no exact equivalent; export as `Multiply` and note the loss in a non-fatal warning.
  - Pixhaus `Erase`: Aseprite has no equivalent; export as `Normal` with alpha 0 and warn.
  - Pixhaus `Pass-Through`: group-only; Aseprite groups always pass-through. Export as Aseprite group, no flag needed.
- **Why it matters.** Aseprite interop is the single biggest user request for a competing pixel art editor. A clean port of the chunk parser is one of the highest-leverage borrows in this catalog.
- **Integration.** B7 (Aseprite format compatibility spec — this is the reference implementation we measure against), S08 (`.aseprite` read/write). Lives in `io/src/aseprite/`.
- **Attribution.** Header comment in `io/src/aseprite/mod.rs`; "ported algorithms" line in `docs/THIRD_PARTY_LICENSES.md`.
- **Non-goals.** Write-side compatibility with every Aseprite feature. v1 ships read-side first; write-side follows after Aseprite users sanity-check the export.

### 29. Plugin manifest + crash-detect-then-disable _(Tier D)_

- **Upstream.** `src/HandleExtensions.gd:21, 55, 97-150`. Each extension carries a manifest with name, version, description, author, license, and a list of nodes to instantiate. On load, the loader sets a "loading X" session flag; on successful boot, it clears the flag. Next session boot, if a stale flag is found, the offending extension is auto-disabled and the user is notified.
- **What we adopt.**
  ```toml
  # plugin.toml
  name = "outline-pro"
  version = "0.3.1"
  pixhaus_version_min = "1.0.0"
  author = "..."
  license = "MIT"
  entry = "outline-pro.wasm"          # for WASM plugins (extism)
  # or
  entry = "outline-pro.lua"           # for Lua plugins (mlua)
  permissions = ["canvas:read", "canvas:write", "fs:read:./assets"]
  ```
  Loader semantics: write `~/.config/pixhaus/.loading-plugin` containing the plugin name immediately before loading; delete it on successful init. On startup, read the file (if present) and disable that plugin with a UI banner.
- **Why it matters.** A pixel art editor with plugins will inevitably have a plugin that crashes on load. Without crash-detect, the user is stuck in a boot loop. With it, the next launch comes up clean with a clear notice.
- **Integration.** B5 (verb plugin protocol — same manifest schema for verbs and general plugins), S37 (plugin loader).
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.

### 30. Plugin-registered file formats _(Tier D)_

- **Upstream.** `src/Autoload/Export.gd:104-128` — `add_custom_file_format(format_name, format_description, exporter, file_info)`. Same registration shape as the built-in exporters; both internal and external code use it.
- **What we adopt.**
  ```rust
  pub trait FileFormat {
      fn id(&self) -> &str;
      fn extensions(&self) -> &[&str];
      fn read(&self, bytes: &[u8]) -> Result<Project>;
      fn write(&self, project: &Project) -> Result<Vec<u8>>;
  }
  pub fn register_file_format(format: Arc<dyn FileFormat>);
  ```
  Built-in `.aseprite`, `.psd`, `.pixhaus`, PNG/sprite-sheet I/O all register through `register_file_format`. Plugins do the same.
- **Why it matters.** Avoids a separate "external format" code path that has to keep up with the built-in one. Verb adapters can register their own export formats (e.g., a verb that emits Spine JSON) without forking the I/O stack.
- **Integration.** B5, S07-S12, S37.
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.

### 31. Krita and PSD parsers — deferred reference _(Tier P, deferred)_

- **Upstream.** `src/Classes/SoftwareParsers/KritaParser.gd` (509 lines), `src/Classes/SoftwareParsers/PhotoshopParser.gd` (782 lines). Substantial; both handle layer hierarchies, blend modes, palettes.
- **What we adopt.** Flagged as a reference for future streams. Pixhaus's current scope (per `docs/planning/work/streams.md`) includes `.psd` import (S09) but not `.kra`. When S09 opens, the upstream parser is the highest-quality reference available; for `.kra`, opening a follow-up stream becomes attractive once Krita interop is requested.
- **Integration.** S09 (PSD), future Krita stream. Lives in `io/src/psd/` and `io/src/krita/` when scheduled.
- **Attribution.** Header comments on the ported files; "ported algorithms" lines in `docs/THIRD_PARTY_LICENSES.md`.
- **Non-goals.** Porting `.kra` or `.psd` parsers in the adoption-plan PR. The plan flags the references; the ports land per-stream.

## Group 6 — UX, theming, shell

Touches **S13** (app shell), **S15** (brush UI), **S16** (selection/transform UI), **S41** (documentation site / localization).

### 32. Action-and-profile keyboard system _(Tier D)_

- **Upstream.** `addons/keychain/Keychain.gd` (222 lines) + `src/Autoload/Global.gd` (defines 150+ `Keychain.InputAction` objects). Every menu item, every tool, every modifier-while-painting is a named action with default and user-bound triggers.
- **What we adopt.**
  ```rust
  struct ActionDef {
      id: &'static str,                  // e.g. "draw.bucket-fill"
      label: &'static str,                // localizable; default English
      group: ActionGroup,                 // File, Edit, View, Tools, Timeline, ...
      default_keybind: Option<Keybind>,
      default_mouse_motion: Option<MouseMotionBinding>,
      scope: ActionScope,                 // Global | CanvasFocused | TimelineFocused | ...
  }
  ```
  Profiles saved as TOML at `~/.config/pixhaus/shortcut-profiles/<name>.toml`. Ship "Default", "Aseprite", and "Photoshop" profiles as first-class deliverables.
- **Why it matters.** Pixel artists migrate from Aseprite or Photoshop with a decade of muscle memory. Forcing them to relearn keybinds is the single biggest friction point. Action-and-profile means the migration is "switch profile in preferences," not "edit your shortcuts manually for 30 minutes."
- **Integration.** S13 (app shell command system), S41 (Aseprite migration guide cites the profile).
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.

### 33. Mouse-motion modifier shortcuts _(Tier D)_

- **Upstream.** `addons/keychain/Keychain.MouseMovementInputAction`. Hold modifier + drag mouse → continuously adjusts brush size, hue, saturation, value, or alpha. Configurable axis (X or Y), direction, and sensitivity per binding.
- **What we adopt.**
  ```rust
  struct MouseMotionBinding {
      modifier: KeyModifiers,
      axis: Axis,                   // X | Y
      direction: Direction,         // Positive | Negative
      sensitivity: f32,             // units per pixel of motion
      target: ContinuousTarget,     // BrushSize | Hue | Saturation | Value | Alpha
  }
  ```
- **Why it matters.** Bracket-keys for brush size is the bare minimum; modifier-drag is the smooth-and-pleasant version. Pixelorama users routinely cite this as the feature they miss most when switching to other editors.
- **Integration.** S15 (brush engine UI), S16. Wires into the same action system as entry 32.
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.

### 34. Dockable panel system with floating windows and saved layouts _(Tier D)_

- **Upstream.** `addons/dockable_container/`. Dock/undock, drag-rearrange, save/restore named layouts, multi-monitor float, splittable regions.
- **What we adopt.** Pick a web-native equivalent (Dockview, golden-layout, or a custom Solid implementation) and require the same feature set:
  - Drag a panel by its title bar to dock at any edge or split.
  - Undock a panel into a floating window (multi-monitor).
  - Save the current layout under a name.
  - Restore a saved layout.
  - Reset to default layout.
  - "Zen mode" hotkey to hide all panels.
- **Why it matters.** Pixel art workflows vary wildly (tilemap artists want big tile preview, sprite artists want big timeline). Forcing a fixed layout fights every workflow. Saved layouts let users build per-task setups (e.g., "Animation", "Tilemap", "Reference Heavy").
- **Integration.** S13 (app shell). Web-stack choice gets its own ADR.
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.

### 35. Theme engine with named token-file presets _(Tier D)_

- **Upstream.** `src/Autoload/Themes.gd` + `assets/themes/{dark,gray,blue,caramel,light,purple,rose}/theme.tres`. 7 themes shipped; users pick from preferences.
- **What we adopt.** Redefine the same 7 named themes as TOML or JSON design-token files consumed by Solid. Godot `.tres` files are engine-specific and do not port; the _idea_ of seven named themes plus an extension-can-add-more API is what we adopt.
  ```toml
  # themes/dark.toml
  [color]
  background = "#1a1a1a"
  surface    = "#252525"
  primary    = "#7fb3ff"
  on-surface = "#e6e6e6"
  border     = "#3a3a3a"
  # …
  [type]
  base-size = "13px"
  ```
- **Why it matters.** Pixel artists work for long sessions; theme choice is comfort. A 7-theme starter set is large enough that most users find one they like and stop tweaking.
- **Integration.** S13.
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.

### 36. Splash dialog with rotating contributor artwork _(Tier D)_

- **Upstream.** `src/UI/Dialogs/SplashDialog.gd`. Random artwork on each launch, with arrow-keys to browse and an artist credit; "Open Last Project" button; "Show on Startup" checkbox to suppress.
- **What we adopt.** Same shape. Artwork lives in `assets/splash/<artist>/<piece>.png` with sibling `<piece>.txt` credit files. Optional but high-impact onboarding win.
- **Why it matters.** Sets tone (Pixhaus respects the artistic side of the tool) and gives the editor a personality at the moment users care most (first launch). Almost zero engineering cost.
- **Integration.** S13.
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.

### 37. Reference images with monochrome/overlay/clamp shader _(Tier S)_

- **Upstream.** `src/Shaders/ReferenceImageShader.gdshader` + `src/UI/ReferenceImages/ReferenceImage.gd`. Drop-in references over/under canvas with opacity, rotation, scale, monochrome toggle, color overlay tint, alpha clamping, linear/nearest filter toggle.
- **What we adopt.** Port the shader; reproduce the data model.
  ```rust
  struct ReferenceImage {
      id: ReferenceImageId,
      image: Image<Rgba8>,
      position: Vec2,
      scale: Vec2,
      rotation: f32,
      opacity: f32,
      monochrome: bool,
      color_overlay: Option<Rgba8>,
      filter: TextureFilter,           // Linear | Nearest
      z_order: ReferenceZOrder,         // AboveCanvas | BelowCanvas
      locked: bool,
  }
  ```
- **Why it matters.** Pixel artists work from references constantly (sprite-from-3D-render, sprite-from-photo, sprite-from-sketch). Native reference-image support means the user doesn't alt-tab to a second window.
- **Integration.** S14 (viewport), S17 (layer-adjacent panel UI). Shader at `ui/src/shaders/reference-image.glsl`.
- **Attribution.** Header comment in the shader file; "ported shaders" line in `docs/THIRD_PARTY_LICENSES.md`.

### 38. Crowdin + `.po` localization pipeline _(Tier D)_

- **Upstream.** `crowdin.yml` at the upstream repo root; 65 `.po` files in `Translations/`. Gettext-style string extraction; translations crowd-sourced via Crowdin.
- **What we adopt.** Same toolchain shape, scoped initially to a smaller language set (top 10-15 by user count). String extraction via a `tr!` macro in Rust + a `t()` helper in TypeScript that lints to a `.pot` template. Crowdin or a self-hosted alternative (e.g., Weblate) handles the rest.
- **Why it matters.** Sets up S41 (documentation/localization) with a known-working toolchain rather than reinventing.
- **Integration.** S41. Establishes a per-PR rule: any new user-facing string must go through `tr!` / `t()`.
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.

### 39. Drag-from-browser image import _(Tier D)_

- **Upstream.** `src/Main.gd` — HTTPRequest path triggered when the user drops an image URL or blob payload from a browser tab onto the canvas. Pixelorama downloads the image, inserts it as a new layer or reference image.
- **What we adopt.** Wire Tauri's `tauri::Window::on_event` to `WindowEvent::FileDrop` and, for URL payloads, fall back to a single Reqwest GET. Result: drag an image from any browser tab and it lands as a new layer.
- **Why it matters.** Removes a friction step that every pixel artist hits: "save the reference, find it in the file dialog, open it." One drag is enough.
- **Integration.** S13.
- **Attribution.** `docs/THIRD_PARTY_LICENSES.md` "adopted designs" entry.

## Conspicuous absences

Things Pixelorama explicitly **does not** have. Listing here so the team can see where Pixhaus's roadmap is meaningfully ahead:

- **No AI / ML anywhere.** Pixhaus's verb runtime (S21-S36) is the differentiation.
- **No clone / healing brush.** AI verbs cover the same ground (and more).
- **No vector or Bezier primitives that survive past rasterize.** Pixhaus is raster-only by scope, so this is alignment, not a gap.
- **No content-aware fill or generative inpainting.** AI verb.
- **No mesh warp / liquify / cage deform.** Pixhaus delivers this via the auto-mesh-deformation verb.
- **No true layer masks.** Only clipping masks (entry 20). Adding layer masks is a small follow-up UX win.
- **No keyframe interpolation / animation easing.** Frame-by-frame only, like Pixhaus's planned scope. A potential future differentiator if a tween-curve UX is added.
- **No artboards / multi-canvas per project.** Pixhaus's scope is single canvas per project; alignment.
- **No batch / CLI / headless scripting.** Extensions run in-process only. Pixhaus may want a CLI for CI / batch export — open question.
- **No collaboration / multiplayer.** Pixhaus also doesn't, per scope.

## Open questions for the team

These came up during cataloguing and need a decision before the corresponding ports land:

1. **File format swap (entry 1).** B3 currently specifies MessagePack + zstd; entry 1 proposes ZIP + JSON. Revise B3, or keep MessagePack and treat this as a non-binding suggestion? Recommendation: revise B3.
2. **Theme files (entry 35).** Solid design-tokens via TOML vs CSS custom properties? No strong preference from the cataloguing — defer to the S13 owner.
3. **Plugin entry-point format (entry 29).** WASM via extism for general plugins, Lua via mlua for scripting. Are these one plugin system or two? Both are in scope per B5; clarify which manifest field selects the runtime.
4. **Dither matrix vendoring (entry 22).** Vendor the four Bayer PNGs in this PR or in the gradient-shader PR? Recommendation: gradient-shader PR, to keep this PR documentation-only.
5. **Aseprite write-side scope (entry 28).** Read-side only for v1, or read+write together? Aseprite's docs note that some chunk types are read-only (deprecated mask/path); whichever we pick, the doc captures it.

## What this PR delivers vs. what comes next

This PR adds two files:

- `docs/planning/research/pixelorama-adoption.md` — this file.
- `docs/THIRD_PARTY_LICENSES.md` — the attribution scaffolding, pre-filled with the Pixelorama entry.

This PR **does not**:

- Implement any of the 39 catalog entries. Each lives in its owning stream.
- Vendor any assets. Entry 22 is flagged but the actual PNG copy lands with the gradient/posterize shaders.
- Update `docs/planning/work/bedrock.md` or `streams.md` to cross-link back. Do that in a follow-up once adoption is approved.
- Move `docs/THIRD_PARTY_LICENSES.md` to the repo root. Requires a separate planning-doc revision; flagged inside that file.
- Replace or revise `docs/planning/pixel-art-editors/06-pixelorama.md`. That doc keeps its current product-level framing.
