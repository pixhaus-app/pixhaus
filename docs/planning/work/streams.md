# Parallel work streams

Every item below is a stream of work that can be dispatched to its own agent and developed in parallel with the others, assuming the bedrock specs in `bedrock.md` are in place. Streams are sized to be roughly 1-3 weeks of agent work each, including review cycles.

The total stream count is high (40+) deliberately. Maximum parallelism means more, smaller streams with sharper interfaces, not fewer big ones.

## How to read this

Each stream has:
- **ID and name**
- **Scope** — one paragraph of what gets built
- **Depends on** — bedrock specs and (where unavoidable) other streams
- **Interfaces** — what other streams consume from this one
- **Agent brief** — the prompt to dispatch

Streams marked with **★** are on the critical path — they unblock other streams and should be staffed first if compute is constrained. Most streams have no critical-path role and can run whenever.

## Stream index

### Rust core (S01-S06)
| ID | Name | Critical |
|---|---|---|
| S01 | Pixel buffer and blend modes | ★ |
| S02 | Color and palette ops | ★ |
| S03 | Selection algorithms |  |
| S04 | Transform operations |  |
| S05 | Undo/redo command pattern | ★ |
| S06 | Tilemap data structures and autotile rules | ★ |

### File I/O (S07-S12)
| ID | Name | Critical |
|---|---|---|
| S07 | `.pixhaus` native format | ★ |
| S08 | `.aseprite` read/write | ★ |
| S09 | `.psd` import |  |
| S10 | PNG sprite sheet + JSON export | ★ |
| S11 | Animated GIF + WebP export |  |
| S12 | TMX tilemap export |  |

### Editor UI (S13-S20)
| ID | Name | Critical |
|---|---|---|
| S13 | Application shell and command palette | ★ |
| S14 | Canvas viewport (WebGL2) | ★ |
| S15 | Brush engine UI |  |
| S16 | Selection and transform UI |  |
| S17 | Layer panel |  |
| S18 | Color and palette panel |  |
| S19 | Timeline panel |  |
| S20 | Tilemap UI (tileset, autotile rule editor) |  |

### AI infrastructure (S21-S22)
| ID | Name | Critical |
|---|---|---|
| S21 | Verb runtime (dispatch, streaming, cancellation, context injection) | ★ |
| S22 | Backend adapters (Anthropic, OpenAI, Replicate, Ollama, ComfyUI, Stability) | ★ |

### AI verbs (S23-S36)
| ID | Name | Critical |
|---|---|---|
| S23 | Verb: Inbetween |  |
| S24 | Verb: Continue |  |
| S25 | Verb: Extend (multi-direction) |  |
| S26 | Verb: Variant |  |
| S27 | Verb: Cleanup |  |
| S28 | Verb: Tile (autotile generation) |  |
| S29 | Verb: Critique |  |
| S30 | Verb: Project style learning |  |
| S31 | Verb: Conversational editing |  |
| S32 | Verb: Motion-from-video |  |
| S33 | Verb: Auto-mesh-deformation |  |
| S34 | Verb: Audio-driven timing |  |
| S35 | Verb: Tileset-from-description |  |
| S36 | Verb: Sketch finishing |  |

### Plugins and scripting (S37-S38)
| ID | Name | Critical |
|---|---|---|
| S37 | Plugin loader and public API surface |  |
| S38 | Lua scripting bindings |  |

### Unity integration (S39-S40)
| ID | Name | Critical |
|---|---|---|
| S39 | Unity importer package | ★ |
| S40 | Unity sample project |  |

### Documentation and content (S41-S45)
| ID | Name | Critical |
|---|---|---|
| S41 | User documentation site |  |
| S42 | Migration guide from Aseprite |  |
| S43 | Plugin developer guide |  |
| S44 | Tutorial content (videos and walkthroughs) |  |
| S45 | Sample projects and fixtures |  |

### Brand and launch (S46-S48)
| ID | Name | Critical |
|---|---|---|
| S46 | Logo, visual identity, design tokens |  |
| S47 | Website (pixhaus.app landing) |  |
| S48 | Discord and community setup |  |

### Build, release, ops (S49-S52)
| ID | Name | Critical |
|---|---|---|
| S49 | CI/CD pipelines | ★ |
| S50 | Release packaging (installers, signing, auto-update) |  |
| S51 | Crash reporting (opt-in) |  |
| S52 | Visual regression test harness |  |

---

## Stream details

### S01. Pixel buffer and blend modes ★

**Scope:** The foundation of every visual operation. A `PixelBuffer<P: Pixel>` type that holds a 2D array of pixels, supports indexed (palette index) and RGBA modes, and implements all standard blend modes (Normal, Multiply, Screen, Overlay, Darken, Lighten, Color Dodge, Color Burn, Hard Light, Soft Light, Difference, Exclusion, Hue, Saturation, Color, Luminosity, Add, Subtract, Divide). Pixel-perfect blend math that matches Aseprite reference output exactly. SIMD where it pays off. Tile-based memory layout for cache friendliness.

**Depends on:** B2 (data model)

**Interfaces:** S02, S03, S04, S05, S06, S08 (Aseprite blend mode parity), S14 (canvas rendering)

**Agent brief:**
> Implement `core/src/canvas/` with the `PixelBuffer` type and full blend mode set. Match Aseprite's blend math byte-for-byte — Aseprite's `src/doc/blend_funcs.cpp` is the reference. Support both indexed and RGBA modes. Use `image::ImageBuffer` as the underlying storage where it fits, custom storage where it doesn't. Add criterion benchmarks for each blend mode showing throughput on 256x256 buffers. Include round-trip tests against fixture inputs/outputs from a known-good Aseprite render. Use `rayon` for parallel composite operations on multi-layer stacks. The crate should expose blend mode operations as standalone functions (not bound to a particular buffer type) so other modules can compose them.

---

### S02. Color and palette ops ★

**Scope:** The palette is sacred for pixel art. This stream owns palette types, indexed-color discipline, palette swap operations, color ramps, harmony tools (split-complement, triad, tetrad, analogous), color cycling, and palette I/O for `.gpl`, `.pal` (Microsoft and JASC variants), `.aco` (Photoshop), `.hex` (Lospec), and the Lospec API.

**Depends on:** B2 (data model), S01 (pixel buffer for palette swap operations)

**Interfaces:** S07 (.pixhaus stores palettes), S08 (Aseprite palettes), S18 (palette panel UI)

**Agent brief:**
> Implement `core/src/color/` with palette types, color math, indexed-mode operations, and palette I/O. Required: indexed↔RGBA conversion that respects transparent index, palette swap (replace one color with another across all uses), color cycling animation (cycle palette indices over time), color ramp generation (interpolate between two colors with N stops), harmony tool generation (split-complement, triad, tetrad, analogous, monochromatic). I/O formats: `.gpl` (GIMP), `.pal` (Microsoft RIFF), `.pal` (JASC), `.aco` (Photoshop), `.hex` (Lospec text format). Add a Lospec API client (https://lospec.com/palette-list) for browsing and importing community palettes. Include unit tests for every conversion and round-trip tests for every I/O format. Reference: Aseprite's `src/doc/palette.cpp` and `src/doc/file/`.

---

### S03. Selection algorithms

**Scope:** Selection masks and the algorithms that produce them. Rectangular and elliptical marquee, freehand lasso, magic wand (4-connected and 8-connected flood fill with tolerance), color range (select all pixels matching a color or color range), invert, expand, contract, feather (less critical for pixel art but supported), boolean operations between selections (union, intersect, subtract, xor).

**Depends on:** B2, S01

**Interfaces:** S04 (transforms operate on selections), S16 (selection UI)

**Agent brief:**
> Implement `core/src/selection/` with selection mask types and all selection algorithms. Selection is a 1-bit-per-pixel boolean mask plus a "soft" alpha channel for feathered selections. Required algorithms: rectangle, ellipse, freehand polygon (with closing), magic wand (configurable connectivity and tolerance), color range (selects all pixels matching a color +/- tolerance), invert, expand by N pixels, contract by N pixels, feather by radius. Boolean operators: union, intersect, subtract, xor. Pixel-art bias: feathering should be off by default; selections should snap to pixel boundaries. Include exhaustive unit tests with fixture before/after states. Reference: any classical image processing text or `image-rs` selection ops, but verify against Aseprite behavior on edge cases.

---

### S04. Transform operations

**Scope:** Move, scale, rotate, flip horizontal, flip vertical, skew, perspective. Pixel-art bias: integer-pixel rotations use RotSprite or similar pixel-aware algorithms (not bilinear interpolation). Scaling supports nearest-neighbor (default for pixel art) and integer scaling. Skew and perspective are supported but documented as non-pixel-perfect.

**Depends on:** B2, S01, S03

**Interfaces:** S16 (transform UI)

**Agent brief:**
> Implement `core/src/transforms/` with all standard image transforms. Required: translate (integer pixels), scale (with nearest-neighbor and integer-multiple variants), rotate (with RotSprite for pixel art and bilinear/bicubic as opt-in for non-pixel use), flip H, flip V, skew, perspective. RotSprite reference: https://en.wikipedia.org/wiki/Pixel_art_scaling_algorithms#RotSprite. Transforms operate on the active selection if one exists, else on the active layer. Each transform is reversible (recorded in the undo stack — the actual undo system is S05; this stream just produces the operations). Include benchmark tests; rotate-RotSprite on 256x256 should run in single-digit milliseconds.

---

### S05. Undo/redo command pattern ★

**Scope:** Every editor mutation goes through a command. Commands are reversible — they implement `apply` and `undo`. The undo stack supports unlimited depth (memory-bounded), branching (if user undoes then makes a new edit, the redo branch is preserved as a tree), and named history entries. This is the spine of editor reliability.

**Depends on:** B2, B4 (commands map to IPC entries)

**Interfaces:** All editing streams (S15, S16, S17, S19, S20) emit commands; AI verbs (S23-S36) commit through commands.

**Agent brief:**
> Implement `core/src/undo/` with the command pattern and undo/redo stack. Every mutation is a command implementing a `Command` trait with `apply(&mut Project)` and `undo(&mut Project)`. The history is a tree, not a flat stack — branching on edits-after-undo preserves the redo branch as a new tree node. Memory bound: configurable cap (default 500 commands or 500MB whichever first), oldest entries dropped beyond the cap. Coalescing: stroke commands within a single brush stroke coalesce into one history entry; configurable timeout for grouping. Naming: every command has a human-readable label for the history panel ("Brush stroke", "Add layer 'Foreground'"). Include test cases for branching, coalescing, memory eviction, and round-trip apply→undo→apply→undo invariance.

---

### S06. Tilemap data structures and autotile rules ★

**Scope:** Tilemap layer type, tileset definition, per-cell tile data, tile flags (rotation 90/180/270, flip H, flip V), animated tiles, and the autotile rule engine. Autotile supports Wang corner-blob (16 tiles), Wang edge-blob (47 tiles, the standard "blob set"), and rule-based custom tiles.

**Depends on:** B2 (data model)

**Interfaces:** S07 (storage), S10 (sprite sheet export of tilemaps), S12 (TMX export), S20 (tilemap UI)

**Agent brief:**
> Implement `core/src/tilemap/` with the tilemap layer type and autotile rule engine. A `Tileset` defines tile dimensions, source image (or atlas), and per-tile metadata (collision, animation timing, rotation/flip flags). A `TilemapLayer` is a 2D grid of tile references with per-cell flags (rotation, flip H, flip V). Autotile types to support: 16-tile Wang corner-blob, 47-tile Wang edge-blob (the de facto standard), 4-tile minimal blob, and rule-based (user-defined matching rules à la Tiled's "rule tiles"). The autotile engine takes a tilemap, a brush position, and emits the correct tile based on neighbor matching. Animated tiles cycle through a frame list with configurable timing per tile. Reference: Tiled's autotile docs and Godot 4's TileSet system. Include exhaustive tests with fixture inputs.

---

### S07. `.pixhaus` native format ★

**Scope:** Implementation of the format spec from B3. Read, write, and migrate between schema versions.

**Depends on:** B3 (spec)

**Interfaces:** S08-S12 (sister I/O streams), S13 (open/save UI), S39 (Unity importer reads exported sprite sheets, not the native format directly).

**Agent brief:**
> Implement `io/src/pixhaus/` per the spec in `docs/file-format.md`. Read and write the binary format: magic bytes, version header, MessagePack-encoded core data model, zstd-compressed pixel buffer payloads. Forward-compatible reader: skip unknown optional chunks with a warning; refuse to load unknown required chunks. Schema migration: when the format version changes in a backward-incompatible way, ship a migration function that converts old files to the new format. Round-trip tests for every supported feature. Performance test: reading and writing a 256x256x100-frame project should take under 200ms on a recent laptop.

---

### S08. `.aseprite` read/write ★

**Scope:** Implementation of B7 — the Aseprite file format support. Read every chunk type at the support level the spec defines; write at the documented write-side compatibility level.

**Depends on:** B7 (spec), S01 (blend modes), S02 (palettes), S05 (undo not needed for I/O), S06 (for tilemap chunks)

**Interfaces:** S13 (file open/save), S42 (migration guide validation)

**Agent brief:**
> Implement `io/src/aseprite/` per the spec in `docs/aseprite-compat.md`. Reference: https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md. Implement readers and writers for every chunk type at the support level documented in the spec. Validate against a fixture set of real-world `.aseprite` files (test by collecting permissively-licensed examples from itch.io / GitHub or by hand-crafting fixtures with Aseprite). Write-side: when Pixhaus saves a `.aseprite` file, opening it in Aseprite should produce a file that Aseprite considers valid and renders correctly. Cross-check with the LibreSprite reader implementation as a sanity check. Edge cases to test: linked cels, layer groups with blend modes, custom blend modes, tilemap chunks (Aseprite 1.3+), color profiles.

---

### S09. `.psd` import

**Scope:** Read Photoshop `.psd` files: layers, layer groups, blend modes (PSD's superset), basic transforms, basic masks. Write side: not supported (PSD writing is a tar pit). Used for migration from Photoshop sprite workflows.

**Depends on:** B2, S01

**Interfaces:** S13 (file open)

**Agent brief:**
> Implement `io/src/psd/` for reading Photoshop `.psd` files. Use the `psd` crate as the foundation if it covers the needed features; if not, implement the spec directly per Adobe's Photoshop File Format Specification. Required: layer hierarchy with groups, blend modes (mapping PSD's full set onto Pixhaus's blend mode set), opacity and visibility, layer masks (vector masks ignored), basic transforms baked into layer data. Skip: smart objects, layer effects, adjustment layers, text layers (rendered as raster), 16-bit and 32-bit per channel modes (downsample to 8-bit with warning). Test against a corpus of PSD files saved by Photoshop CC and Affinity Photo to catch divergent dialect choices. Write side is out of scope.

---

### S10. PNG sprite sheet + JSON export ★

**Scope:** The engine handoff format. Pack frames into a sprite sheet PNG with one of several layout strategies (grid, packed, by-row), emit Aseprite-compatible JSON metadata. Per spec B6.

**Depends on:** B6 (Unity handoff spec), S01

**Interfaces:** S39 (Unity importer consumes this)

**Agent brief:**
> Implement `io/src/png/` with sprite sheet packing + JSON metadata export, per `docs/unity-handoff.md`. Layout strategies: grid (uniform cells), packed (rectangle bin-packing for tightest output), by-row (one frame per row). Use the `texture-packer` crate or a simple Skyline algorithm for packed layout. JSON output schema is Aseprite-JSON-compatible: `frames` array with frame rectangles + durations, `meta` object with size + scale + frame tags + slices. Include round-trip tests where the JSON+PNG can be parsed back into a frame sequence. Test fixtures should cover: small grid (4x4), large packed sheet (100+ sprites), mixed sizes, animated tilemap frames.

---

### S11. Animated GIF + WebP export

**Scope:** Animated GIF export with palette quantization and dithering options. WebP animated export. MP4 export for sharing.

**Depends on:** S01, S02

**Interfaces:** S13 (export menu)

**Agent brief:**
> Implement `io/src/animated/` with GIF, animated WebP, and MP4 export. Use the `image` crate's GIF encoder, `webp` crate, and `ffmpeg-next` (or shelling out to ffmpeg if licensing forbids static linking) for MP4. GIF export options: palette mode (use existing palette, quantize to 256 colors, quantize per frame), dithering (off, Floyd-Steinberg, ordered Bayer 8x8), loop count, frame timing. Per-format quality knobs documented in `docs/export-formats.md`. Test by exporting reference animations and verifying byte-level decode-ability with `image-rs` and a few external decoders.

---

### S12. TMX tilemap export

**Scope:** Export tilemap layers as Tiled-compatible `.tmx` (XML) plus a tileset PNG. Required because Unity's SuperTiled2Unity importer is the path of least resistance for tilemap pipelines.

**Depends on:** B6, S06 (tilemap data), S10 (sprite sheet packing for tileset)

**Interfaces:** S39 (Unity importer optionally consumes TMX)

**Agent brief:**
> Implement `io/src/tiled/` with TMX export per the Tiled format documented at https://doc.mapeditor.org/en/stable/reference/tmx-map-format/. Export needs: layer hierarchy, tileset definition with per-tile properties (collision, animation), object layers if Pixhaus has them (defer if not in scope), per-cell flip/rotate flags. Test the output by importing into Tiled and into Unity's SuperTiled2Unity importer; both should render the map correctly. Reference exports in `examples/tiled-export/`. Skip: Tiled's object templates, per-tile terrain definitions (we use our own autotile system), Tiled's wangsets format (export as plain tilemap with autotile already applied).

---

### S13. Application shell and command palette ★

**Scope:** The Tauri app, window chrome, menus, command palette (Ctrl/Cmd+K), keybind manager, theming (light/dark/custom themes), preferences UI, project switcher. The skeleton everything else hangs from.

**Depends on:** B1 (scaffold), B4 (commands)

**Interfaces:** Every UI stream consumes the shell.

**Agent brief:**
> Build the Pixhaus application shell in `ui/src/shell/`. Stack: Solid.js + Vite + Tauri 2. Required components: window chrome (custom title bar with menu, minimize/maximize/close), main menu (File, Edit, Sprite, Frame, Layer, Select, View, AI, Window, Help — names from Aseprite for familiarity), command palette (Ctrl/Cmd+K opens a fuzzy-searchable list of every command from B4), keybind manager (configurable, with Aseprite-compatible defaults as one option and Photoshop-compatible defaults as another), theme system using CSS custom properties with light/dark/Pixhaus-default themes, preferences UI for keybinds, themes, and AI backend config, project switcher (recent projects + open file). Use a small reactive state store (Solid stores or a tiny custom one — no Redux). Performance: shell startup under 200ms after the Tauri process is up. The window background, menus, and panel chrome should feel native on each OS, not look like a web app pretending.

---

### S14. Canvas viewport (WebGL2) ★

**Scope:** The pixel rendering surface. WebGL2 viewport that displays sprites, frames, layers composited by the Rust core. Pan, zoom (fit-to-window, 100%, 200%, custom), grid overlay (configurable spacing), pixel grid at high zoom, onion skin overlay, selection marching ants, brush cursor preview, transform handles. Tile-based dirty rendering — only re-render regions that changed.

**Depends on:** S01 (compositing), S05 (renders post-undo state), S13 (lives inside shell)

**Interfaces:** S15 (brush draws into canvas), S16 (selection visible), S19 (timeline scrub updates canvas), S20 (tile painting)

**Agent brief:**
> Build the canvas viewport in `ui/src/canvas/` using WebGL2 and Solid. The viewport receives composited tile textures from the Rust side via Tauri events (Rust composites layers into 256x256 tiles, ships textures over IPC). The WebGL2 layer renders these tiles + UI overlays (grid, marching ants, brush preview, transform handles). Pan via spacebar+drag and middle-mouse-drag. Zoom via mouse wheel (anchored at cursor) and +/- keys; snap zoom levels at 100%, 200%, 400%, 800%, 1600%, plus continuous in-between. Pixel grid auto-shows above 800% zoom (configurable). Onion skin overlay: previous N frames at configurable opacity tinted red, next N at configurable opacity tinted blue. Marching ants for selections: animated with a shader, frame-rate-stable. Performance target: 60fps pan/zoom on 4096x4096 sprites with 50 layers. Use `regl` or write WebGL directly — no React-Three-Fiber overhead. The Rust side uses `wgpu` for tile compositing where useful but doesn't ship pixel data through IPC each frame; uses shared GPU textures.

---

### S15. Brush engine UI

**Scope:** All drawing tools — pencil, eraser, line, rectangle, ellipse, polygon, fill bucket, gradient, dither brush, pattern stamp, smudge (limited for pixel art), spray. Tool options panel (size, opacity, hardness for non-pixel modes, dither pattern, pixel-perfect stroke smoothing). Custom brush definitions (load PNG as brush stamp).

**Depends on:** S01 (the actual pixel ops happen in Rust), S14 (canvas hosts tools)

**Interfaces:** S05 (every stroke is a command)

**Agent brief:**
> Implement the brush engine UI in `ui/src/canvas/tools/` plus the underlying stroke ops in `core/src/canvas/tools/`. Tools: pencil (1-pixel by default, configurable size, pixel-perfect mode), eraser, line, rectangle, ellipse, polygon, fill bucket (with contiguous and global modes, tolerance), gradient (linear, radial), dither brush (50%-50% checker, 25%-75%, configurable patterns), pattern stamp (load PNG as stamp), smudge (warn user it's not standard for pixel art), spray (random pixel placement within a circle). Tool options panel shows tool-specific controls. Pixel-perfect stroke mode: post-process strokes to remove "doubled" pixels at corners (Aseprite's "Pixel-perfect strokes" feature). Custom brush: load any image as a brush stamp; brush rotation, mirroring, color tint. Each tool's stroke is committed as a single coalesced undo command via S05. Reference: Aseprite tool source for behavior parity. Performance: a 1000-pixel stroke with custom brush should run at 60fps.

---

### S16. Selection and transform UI

**Scope:** UI for the selection algorithms (S03) and transform operations (S04). Marquee tools (rect, ellipse, freehand), magic wand, color range, transform handles for the active selection.

**Depends on:** S03, S04, S14

**Interfaces:** S05

**Agent brief:**
> Implement the selection and transform UI in `ui/src/canvas/select/` and `ui/src/canvas/transform/`. Selection tools: rectangular marquee (with shift-add, alt-subtract modifiers), elliptical marquee, freehand lasso (click-and-drag or click-points), magic wand (click pixel, configurable tolerance + connectivity), color range (color picker → tolerance slider → live preview). Transform UI: when a selection is active, show transform handles (corner + edge handles) for scale, plus a rotation handle, plus a free-transform mode. Skew with shift-modifier on edge handles. Numeric input fields for precise transform. Transforms preview live; commit on Enter or click outside. Each commit is one undo command. Marching ants visualization for selection borders, animated.

---

### S17. Layer panel

**Scope:** Layer hierarchy UI. List view with drag-to-reorder, group/ungroup, blend mode dropdown per layer, opacity slider per layer, visibility toggle, lock toggle, layer thumbnail (live-updating). Right-click menu: rename, duplicate, delete, merge down, flatten visible, convert to group, convert to tilemap layer.

**Depends on:** S05 (every layer op is a command), S13

**Interfaces:** S14 (canvas reflects active layer), S19 (timeline shows per-layer cels)

**Agent brief:**
> Implement the layer panel in `ui/src/layers/`. Tree view with drag-to-reorder using a virtual list for performance with hundreds of layers. Each layer row: thumbnail (live-rendered 32x32 preview from the Rust side), name (double-click to rename), blend mode dropdown, opacity slider (0-255), visibility toggle, lock toggle. Group layers expand/collapse. Right-click context menu: rename, duplicate, delete, merge down, merge selected, flatten visible, convert to group, convert to tilemap layer. Multi-select with shift+click and ctrl+click. Drag-and-drop into and out of groups with visual indicators. Every operation goes through S05 commands. Performance: panel with 500 layers should scroll at 60fps; thumbnail updates batched at most every 100ms.

---

### S18. Color and palette panel

**Scope:** The color picker and palette UI. Foreground/background colors, palette grid (with reorder, lock, name colors), gradient/ramp generator, harmony picker, palette I/O menu (load/save in supported formats), Lospec browser.

**Depends on:** S02

**Interfaces:** S15 (current color), S05 (palette edits as commands)

**Agent brief:**
> Implement the color and palette panel in `ui/src/palette/`. Components: color picker with HSV/HSL/RGB/HEX/OKLCH input modes, foreground/background swatches (X to swap, D to reset to black/white per Photoshop convention), palette grid with click-to-pick + drag-to-reorder + right-click-to-edit + lock toggle per color, indexed-mode palette indicator (when project is in indexed mode, palette is the source of truth and changing a color updates all uses), harmony picker showing complement / triad / tetrad / analogous suggestions for the active color, ramp generator (pick two colors + N steps → adds the ramp to palette), palette I/O menu (load/save .gpl/.pal/.aco/.hex), Lospec browser modal that pulls from https://lospec.com/palette-list and lets the user browse/preview/import. Every palette edit is a command via S05.

---

### S19. Timeline panel

**Scope:** The animation timeline. Frame/layer grid (frames horizontal, layers vertical), per-cel thumbnails, frame duration editing, frame tags (named ranges with loop direction), onion skin controls, playback controls (play/pause/loop/scrub).

**Depends on:** S05, S13, S17 (shares layer hierarchy)

**Interfaces:** S14 (timeline scrub updates canvas)

**Agent brief:**
> Implement the timeline panel in `ui/src/timeline/`. Layout: layers on the Y axis (mirrors the layer panel), frames on the X axis, each intersection is a cel cell showing a thumbnail. Frame duration shown above the frame column, editable per frame in milliseconds. Frame tags (named ranges with loop direction: forward / reverse / pingpong / once) shown as bars above the timeline; click+drag to create, click to edit. Onion skin controls: enable, range (frames before/after), opacity. Playback controls: play, pause, stop, loop toggle, scrub bar that snaps to frames. Frame operations via right-click: insert, delete, duplicate, copy, paste, reverse selected, reorder selected. Multi-select frames and cels with shift/ctrl. Every operation is a command. Reference: Aseprite's timeline UI, Pixelorama's timeline. Performance: timeline with 200 frames + 50 layers should scroll at 60fps.

---

### S20. Tilemap UI

**Scope:** Tile editor (paint tiles into tilemap layers), tileset panel (browse and select tiles, define per-tile properties), autotile rule editor (visual rule definition for custom autotile sets).

**Depends on:** S06, S13, S14

**Interfaces:** S05

**Agent brief:**
> Implement the tilemap UI in `ui/src/tilemap/`. Three sub-panels: (a) Tileset panel — grid view of tiles in the active tileset, click to select, right-click for tile properties (collision shape, animation timing, autotile membership); (b) Autotile rule editor — visual editor for defining custom autotile rules. For Wang corner-blob (16-tile) and Wang edge-blob (47-tile) standard sets, the rule is implicit. For custom rule sets, the user defines per-tile neighbor matching patterns (à la Tiled's rule tiles) with a visual grid editor. (c) Tile-paint mode for the canvas viewport (S14): when a tilemap layer is active, the brush places tile indices instead of pixels, with optional autotile-aware mode that picks the right tile based on neighbors. Animated tiles cycle in the live preview. Reference: Tiled's autotile editor and Godot 4's TileSet editor.

---

### S21. Verb runtime ★

**Scope:** Implementation of the verb plugin protocol from B5. Dispatch, async invocation, streaming outputs, cancellation, context injection, preview-then-commit flow, cost/latency tracking.

**Depends on:** B5 (protocol spec), S05 (verb commits via undo commands)

**Interfaces:** S22 (consumes backend adapters), S23-S36 (verbs implement against this), S29 (Critique uses the runtime to read project)

**Agent brief:**
> Implement the verb runtime in `ai/src/runtime/` per the protocol in `docs/verb-protocol.md`. Required: verb registration + lookup, async invocation with tokio, streaming outputs as `tokio::sync::mpsc` channels or async streams, cancellation tokens, context builder (collects palette, active layers, frame history, references and packages them as the input payload), preview model (verb produces a preview without committing; user accepts → commit as undo command), cost/latency tracking surfaced to UI, verb error types. Configuration: per-verb backend selection, BYO-API-key from preferences, fallback chains (try local first, fall back to cloud). Tests: a mock backend + an `echo` verb should round-trip through the runtime. Performance: verb dispatch overhead under 1ms.

---

### S22. Backend adapters ★

**Scope:** Inference backend adapters. Anthropic (Claude), OpenAI (GPT-5 + DALL-E + image edit), Replicate (model marketplace), Ollama (local LLMs), ComfyUI (workflow execution), Stability (SD images). Each adapter implements the `InferenceBackend` trait. API key management. Cost estimation per call.

**Depends on:** B5 (which declares backend capability requirements)

**Interfaces:** S21 (runtime consumes adapters), S23-S36 (verbs request capabilities, runtime resolves to adapter)

**Agent brief:**
> Implement backend adapters in `ai/src/backends/`. Each adapter implements the `InferenceBackend` trait with: capabilities list (text generation, vision-language, image generation, image editing, etc.), invoke methods, cost estimation, latency estimation, streaming support flag. Adapters: (1) Anthropic — Claude 4.6 family for VLM and text; (2) OpenAI — GPT-5 class for text/VLM, DALL-E 4 for image gen, image edit endpoints; (3) Replicate — generic adapter that takes a model ID and runs it (lets users plug in Flux, SDXL, custom LoRAs without us shipping every one); (4) Ollama — local LLM via Ollama's HTTP API; (5) ComfyUI — submits a workflow JSON to a local or remote ComfyUI server, polls for results; (6) Stability — Stable Diffusion 3 family + image edit endpoints. API keys stored in the OS keychain via `keyring` crate, never written to disk in plaintext. Configuration UI is part of S13 (preferences). Each adapter has integration tests that hit the real APIs (gated behind env vars so CI can skip them).

---

### S23-S36. AI verbs (shared brief structure)

Each verb is its own stream. They share the same shape:
- Implement against the verb plugin protocol (B5, S21)
- Live in `ai/src/verbs/<verb_name>/`
- Backend requirements declared upfront
- Preview-then-commit flow
- UI invocation via command palette and AI menu
- Documentation in `docs/verbs/<verb_name>.md`

#### S23. Verb: Inbetween

> Generate intermediate frames between two key frames. Backend requirements: image-gen with palette conditioning. Approach: use a frame-interpolation model (RIFE-class or video diffusion) conditioned on the two source frames + the project palette. Snap output to palette. Output: N new cels in the active layer between frames A and B. Reference: Retro Diffusion's chained-frame consistency techniques. Test fixtures: walk cycle key frames + expected intermediate frames.

#### S24. Verb: Continue

> Predict the next 1-3 frames given the last N frames. Backend: image-gen with palette + reference conditioning. Approach: feed the last 3-5 frames to a video-diffusion model as conditioning, request the next frames, snap to palette. Output: N new cels appended to the timeline in the active layer.

#### S25. Verb: Extend (multi-direction)

> Generate multi-direction views from a single sprite. Backend: image-gen + view synthesis. Approach: use a single-image-to-3D model (TripoSR-class) to estimate geometry, render from N camera angles, apply style transfer to match the source style, snap to palette. Output: N new cels (one per direction) in a new layer or layer group. Configurable: 4-direction, 8-direction, custom angle list.

#### S26. Verb: Variant

> Generate variants of a base sprite — palette swaps, equipment overlays, expression sets. Backend: image-edit + reference. Approach: take a base sprite + variant description (text or palette change), produce a new layer with the variant. Palette-swap mode is mostly classical (substitute palette indices) with AI refinement for cases where straight substitution looks wrong. Equipment/expressions use image-edit conditioning. Output: new derived layer or layer group.

#### S27. Verb: Cleanup

> Snap a generated or imported sprite to the project palette, remove sub-pixel anti-aliasing, fix pivot drift across animation frames. Backend: classical image processing + lightweight VLM for ambiguous decisions. Output: applied to active layer (with undo).

#### S28. Verb: Tile (autotile generation)

> Generate a 47-tile blob autotile set from 1-3 example transitions. Backend: image-gen with strong style conditioning. Approach: extract style from examples, generate the missing transitions, validate they tile correctly with neighbors. Output: complete autotile set added to the active tileset, with autotile rules pre-configured.

#### S29. Verb: Critique

> Vision-language analysis of a sprite or animation. Backend: VLM (Claude/GPT-5). Approach: feed the sprite sheet + animation playback + project palette to the VLM with a structured prompt, surface findings as a list. Categories: pose continuity errors, palette violations, missing frames, pivot drift, style inconsistency. Output: a critique panel listing issues with click-to-jump-to-frame.

#### S30. Verb: Project style learning

> Train a per-project style model. Backend: LoRA training (Replicate or local via Diffusers). Approach: ingest every layer in the project as training data, fine-tune a small LoRA (15-30 minutes), register it as the default style reference for subsequent verbs. Output: a trained model file stored in the project directory; subsequent verb invocations include it as a style reference automatically.

#### S31. Verb: Conversational editing

> Free-form natural language → multi-step editor commands. Backend: VLM with tool-use. Approach: user types "make this enemy look angrier, add a scar over the left eye, slow the walk to 8fps", VLM plans command sequence (using S05's command vocabulary), surfaces plan as preview, user accepts → executes. Output: multi-command undo entry.

#### S32. Verb: Motion-from-video

> Extract motion from a reference video into the timeline. Backend: pose extraction (DensePose / MediaPipe) + VLM for keyframe identification. Approach: user drops a video, AI extracts pose timing, populates the timeline with keyframe markers and rough silhouette poses. The artist owns the actual frames; AI provides the timing skeleton. Output: keyframe markers added to the active animation tag with rough pose layers.

#### S33. Verb: Auto-mesh-deformation

> No-bones rigging. Backend: segmentation + view synthesis. Approach: take a single sprite, segment into deformation regions, derive a mesh deformation rig automatically (Live2D-style without explicit bones), produce a parameterized animation surface. Output: the sprite gains a "deformation rig" property; subsequent animations can use parameter sliders to deform without redrawing. Stretch goal — may need to land partially.

#### S34. Verb: Audio-driven timing

> Beat detection / lip sync from audio. Backend: audio analysis (lightweight, can be classical) + VLM for lip-sync intent. Approach: drop in audio, AI detects beats or syllables, places frame markers at beat times. Lip-sync mode generates mouth shapes from voice clips. Output: frame timing markers + (optional) mouth-shape cel sequence.

#### S35. Verb: Tileset-from-description

> Generate a full autotile-compatible tileset from a description. Backend: image-gen with strong consistency. Approach: prompt-to-tileset, generate tile primitives, run S28's autotile generation logic for transitions. Output: complete tileset added to the project.

#### S36. Verb: Sketch finishing

> Finish rough sketches in project style. Backend: image-edit with project style reference. Approach: artist draws rough silhouettes / stick figures / gesture poses, AI refines to finished sprites in the project's learned style. Output: refined cel layer (artist accepts/rejects per frame).

---

### S37. Plugin loader and public API surface

**Scope:** The plugin system. Plugins are bundled as folders with a `plugin.toml` manifest and either Lua scripts or pre-compiled WASM. Hot-load at runtime. Plugins can register: custom verbs, custom tools (brushes, selection algorithms), custom panels, custom file format readers/writers, custom commands.

**Depends on:** B5, S05, S13, S21

**Interfaces:** S38 (Lua bindings used by Lua plugins)

**Agent brief:**
> Implement the plugin loader in `core/src/plugins/`. Plugins live in `~/.pixhaus/plugins/<plugin-name>/` with a `plugin.toml` manifest declaring name, version, author, description, entry point (Lua script path or WASM module path), permissions (can register verbs, tools, panels, commands, format readers/writers). On editor startup, scan plugin directory, load manifests, instantiate plugin instances with restricted capabilities. Plugins register via a host API exposed to Lua/WASM. Hot-reload: editing a plugin file triggers reload without editor restart. Sandbox: WASM plugins can't access filesystem outside the project; Lua plugins use a restricted Lua environment. Document the plugin format in `docs/plugin-system.md` with a worked example.

---

### S38. Lua scripting bindings

**Scope:** Lua bindings for the editor's public API. Aseprite-compatible API surface where possible (so Aseprite scripts have a migration path). Custom UI panel API beyond Aseprite's.

**Depends on:** S37, S05 (commands the script can invoke)

**Interfaces:** S37, S43 (developer guide)

**Agent brief:**
> Implement Lua scripting in `scripting/` using `mlua` crate (Lua 5.4). Expose a Pixhaus host API that parallels Aseprite's `app` global where possible — `app.activeSprite`, `app.activeLayer`, `app.activeFrame`, `app.fgColor`, etc. Aseprite's API reference is at https://github.com/aseprite/api. Add Pixhaus-specific extensions: custom panels (`app.ui.panel { ... }`), custom verbs (`app.ai.registerVerb { ... }`), command palette entries (`app.commands.register { ... }`). A goal: any non-trivial Aseprite script (e.g., Color Reduction, Sprite Sheet Generator, palette tools from the community) ports to Pixhaus with under 20 lines of changes. Ship a sample plugin in Lua demonstrating each major API surface.

---

### S39. Unity importer package ★

**Scope:** A Unity package (UPM, publishable to OpenUPM) that imports Pixhaus exports — sprite sheet PNG + JSON, plus optional TMX tilemaps. Auto-slices, generates animation clips, sets pivots, builds tilemap palettes. Lives in its own repo for separate Unity-cycle release management.

**Depends on:** B6, S10, S12

**Interfaces:** S40 (sample project consumes this), S42 (migration guide references)

**Agent brief:**
> Build the Pixhaus Unity importer in the `unity/` folder of the main repo. Target: Unity 2022.3 LTS minimum, Unity 6 primary. Importer reads sprite sheet PNG + JSON metadata (per `docs/unity-handoff.md`), produces a Sprite asset with sub-sprites for each frame, generates AnimationClip assets per frame tag, sets pivots from slice data, builds a SpriteAtlas if multiple sheets are imported together. Optional TMX import path uses Unity's Tilemap system with auto-generated TileBase assets. Editor scripts go in `unity/Editor/`, runtime helpers (e.g., a PixhausAnimator component for cleaner scripted playback) in `unity/Runtime/`. Reference: the Aseprite Importer for Unity package as the gold standard for what users expect. Publish format: UPM-compatible package.json, ready for OpenUPM submission.

---

### S40. Unity sample project

**Scope:** A small Unity sample project demonstrating Pixhaus → Unity end-to-end. A single character with idle/walk/run/attack animations, a small tilemap level, and a few interactive props. Showcases what the importer gives you.

**Depends on:** S39, S45 (sample art assets)

**Interfaces:** S44 (tutorial content references this)

**Agent brief:**
> Build a Unity sample project in `examples/unity-sample/`. Content: a player character (sprite from S45 sample assets) with idle, walk, run, jump, attack animations (8-direction); a small tilemap level (16x32 tiles, 2 layers — terrain and decoration) using a Pixhaus-exported tileset; a couple of interactive props (animated tiles for water, lava, conveyor belt). Player movement and animation state machine wired up with the simplest possible scripts so the project demonstrates the import pipeline rather than gameplay. README explains how to import a Pixhaus project, what the importer creates, and how to wire it into a scene. Target Unity 6.

---

### S41. User documentation site

**Scope:** The user-facing documentation. Built with mdbook or Astro Starlight. Covers installation, basic editing, animation, tilemaps, AI verbs, scripting, and FAQ. Hosted at docs.pixhaus.app (or pixhaus.app/docs).

**Depends on:** Everything (docs follow features). Can start in parallel with stub pages.

**Interfaces:** S42-S44 consume this site.

**Agent brief:**
> Build the user docs site in `docs/`. Stack: Astro Starlight for richer interactivity, or mdbook for simpler maintenance — make the call based on whether interactive embeds (live editor demos) are needed. Sections: Getting Started (install, first sprite), Editor (tools, layers, palette, selection, transforms), Animation (timeline, tags, onion skin, export), Tilemaps (autotile, animated tiles, tileset editor), AI verbs (one page per verb with examples), Scripting (Lua API reference), Plugins (developing plugins), Reference (keybinds, file formats, IPC commands), FAQ. Initially every page can be a stub, populated as features land. Build pipeline: docs deploy on push to main via GitHub Actions to docs.pixhaus.app.

---

### S42. Migration guide from Aseprite

**Scope:** A first-class document targeting Aseprite users. What's the same, what's different, what `.aseprite` features round-trip cleanly, what doesn't, keybind comparison, scripting porting.

**Depends on:** S08, S38, S41

**Interfaces:** Marketing references this heavily.

**Agent brief:**
> Write the Aseprite migration guide as a chapter in the docs site (S41). Sections: (1) What's the same (timeline, frame tags, onion skin, palette workflow, layer system); (2) What's different (tilemap as a first-class layer type, AI verbs, plugin system extends to UI not just data); (3) File compatibility (what's in `docs/aseprite-compat.md` distilled for users — what round-trips cleanly, what gets warnings); (4) Keybind comparison table (Aseprite default vs Pixhaus default, with a note that Aseprite-compatible mode is one click); (5) Scripting (porting common Aseprite scripts: walk through Color Reduction, Outline, Sprite Sheet Generator, with diff annotations); (6) Tips for the first hour (the 5-10 things that work differently and what to do instead). Keep the tone "you've used Aseprite, here's what's familiar and where to look." Reference: the Aseprite docs at https://www.aseprite.org/docs/.

---

### S43. Plugin developer guide

**Scope:** Documentation for developers building plugins. Covers the manifest, registering verbs/tools/panels, the Lua API, the WASM API, packaging and distribution.

**Depends on:** S37, S38

**Interfaces:** Plugin authors.

**Agent brief:**
> Write the plugin developer guide as a section in the docs site (S41). Sections: (1) Plugin manifest format (`plugin.toml`); (2) Lua plugin tutorial — build a simple custom verb step by step; (3) WASM plugin tutorial — same but in Rust→WASM; (4) UI extension API (registering panels, tools, commands); (5) AI verb authoring (deeper dive — how to write a verb that uses an inference backend, handles streaming, manages context, produces previews); (6) Packaging and distribution (folder layout, signing, publishing to a future plugin registry); (7) Reference for every host API call. Sample plugins live in `examples/plugins/`. Each tutorial should result in a working plugin the reader can run.

---

### S44. Tutorial content

**Scope:** Video and written tutorials for getting started. "Your first sprite", "Your first animation", "Your first tilemap", "Using AI verbs", "Scripting basics". Distributed via the docs site and YouTube.

**Depends on:** S41 + the features being tutorialized

**Interfaces:** Marketing.

**Agent brief:**
> Produce a starter tutorial set: 5-7 short tutorials (5-10 minutes each), in both video form (recorded in Pixhaus with voice-over) and written walkthroughs. Topics: (1) Install Pixhaus, draw your first sprite; (2) Build a 4-frame walk cycle with onion skin; (3) Make a tileset and paint a level with autotile; (4) Use AI verbs to inbetween a walk cycle; (5) Export to Unity; (6) Customize keybinds and themes; (7) Write your first Lua script. Each tutorial has a starter file in `examples/tutorials/<topic>-start.pixhaus` and a finished file. Video production: handled by the user or contracted out; written walkthroughs by an agent. Tutorials should land in the docs (S41) under "Getting started."

---

### S45. Sample projects and fixtures

**Scope:** Sample Pixhaus projects used as test fixtures, marketing demos, and tutorial starting points. A character (idle/walk/run/attack/hurt/death), a tileset (forest, dungeon, city), an enemy, a UI sprite sheet, a level scene assembled from these.

**Depends on:** Editor functional enough to produce these.

**Interfaces:** S40, S44, S47.

**Agent brief:**
> Produce a set of sample Pixhaus projects in `examples/samples/`. Required: (1) `character-knight.pixhaus` — 32x32 knight with idle (4 frames), walk (8 frames × 8 directions), run (8 frames × 8 directions), attack-slash (6 frames × 4 directions), hurt (3 frames), death (8 frames); (2) `tileset-forest.pixhaus` — 16x16 forest tileset with autotile rules for grass→dirt→stone transitions, animated water tiles, decorations; (3) `enemy-slime.pixhaus` — 16x16 slime with idle, hop, hit, split animations; (4) `ui-sprites.pixhaus` — health bar, mana bar, button states, dialogue box; (5) `level-forest.pixhaus` — a 32x16-tile level scene composed from the forest tileset. Art created by the user or contracted out — but the project files themselves (organized layers, tagged animations, palette discipline) are part of the agent deliverable. License the art permissively (CC0 or CC-BY).

---

### S46. Logo, visual identity, design tokens

**Scope:** The Pixhaus brand. Logo (a wordmark + a symbol), color palette, typography, design tokens used by the website and the editor's default theme.

**Depends on:** Naming locked (Pixhaus).

**Interfaces:** S47, the Pixhaus default theme.

**Agent brief:**
> Develop the Pixhaus brand identity. Deliverables: (1) Logo — a wordmark using a typeface that nods to Bauhaus modernism (consider Geist Sans, Apercu, or a custom mark; alternative: a custom geometric wordmark that reads at small sizes), plus a symbol (a single mark that could work as a favicon and app icon — perhaps a stylized pixel arrangement evoking the "house" of "haus"); (2) Color palette — a primary brand color, neutrals, and accents, with consideration for both light and dark editor themes; (3) Typography — primary and secondary typefaces, with a system fallback stack; (4) Design tokens — exported as both CSS custom properties (for the website and editor UI) and a Tailwind config; (5) Logo files in SVG (vector source), PNG at multiple sizes, ICO for Windows. Brand brief: open-source, design-pedigree, modernist functionalism, taken-seriously-by-pros. Avoid: cute pixel-art-themed logos (the tool is for pixel art, but the brand isn't a pixel-art mascot).

---

### S47. Website (pixhaus.app landing)

**Scope:** The marketing site at pixhaus.app. Hero, feature overview, AI verb showcase, comparison-against-Aseprite, download/install, link to docs and GitHub, blog. Built with Astro for static performance.

**Depends on:** S46 (brand), S41 (docs link target)

**Interfaces:** Outside world.

**Agent brief:**
> Build pixhaus.app as a static Astro site. Pages: (1) Home — hero with one-line value prop and demo video, three-feature highlight, AI verb showcase with animated examples, install CTA; (2) Download — installers for Windows / macOS / Linux, build-from-source link, system requirements; (3) Features — deep tour of editor, animation, tilemap, AI capabilities; (4) Compare — feature matrix vs Aseprite, Pixelorama, Photoshop; (5) Docs — redirect to docs.pixhaus.app; (6) Blog — devlog posts; (7) Community — Discord link, GitHub link, contributing CTA. Use S46's design tokens. Self-hosted on Cloudflare Pages or Vercel (free tier). Open Graph images per page. RSS feed for the blog. Lighthouse score 95+ on every page.

---

### S48. Discord and community setup

**Scope:** Discord server with channels for support, showcase, dev, plugins, suggestions. GitHub Discussions enabled. Code of conduct. Issue templates. PR template. Release announcement automation.

**Depends on:** Project public-launch ready.

**Interfaces:** Outside world.

**Agent brief:**
> Set up the Pixhaus community infrastructure. Discord: create server, configure channels (#welcome, #announcements, #general, #showcase, #help, #plugins, #dev, #suggestions, #off-topic), set up roles (newcomer, member, contributor, maintainer), bot integrations (release announcements via webhook, GitHub PR notifications, message moderation). GitHub: enable Discussions, set up issue templates (bug, feature, plugin idea), PR template, code of conduct (Contributor Covenant), Contributing guide referencing `CONTRIBUTING.md` from B8, release-drafter for changelogs. Optional: a Mastodon and Bluesky presence for release announcements. Keep moderation overhead low — the goal is a place users can ask questions and contributors can coordinate.

---

### S49. CI/CD pipelines ★

**Scope:** GitHub Actions for cargo + pnpm linting, testing, building. Per-platform release builds. Auto-publish to GitHub Releases on tag.

**Depends on:** B1.

**Interfaces:** S50.

**Agent brief:**
> Implement CI/CD in `.github/workflows/`. Required workflows: (1) `ci.yml` — runs on every PR, lints (cargo clippy, pnpm lint), tests (cargo test, pnpm test), typecheck (pnpm typecheck), build (cargo build, pnpm build); (2) `release.yml` — triggered on git tag matching `v*`, builds installers for Windows (.msi), macOS (.dmg, both Intel and Apple Silicon), and Linux (.deb, .rpm, AppImage), uploads to GitHub Releases with auto-generated changelog; (3) `docs.yml` — deploys docs on push to main; (4) `unity.yml` — runs Unity tests for the importer package using a Unity License Activation step. Cache: cargo registry, target directory, pnpm store. Speed target: PR CI should complete in under 8 minutes for a feature branch.

---

### S50. Release packaging

**Scope:** Native installers per OS. Code signing (Authenticode for Windows, Apple Developer for macOS, optional GPG for Linux). Auto-update via Tauri's updater plugin. Release notes generation.

**Depends on:** S49.

**Interfaces:** S48 (announcement automation).

**Agent brief:**
> Implement release packaging on top of S49. Use `tauri-action` for cross-platform packaging. Required artifacts per release: Windows MSI signed with Authenticode (acquire and configure cert), macOS DMG signed and notarized with Apple Developer cert (acquire), Linux .deb / .rpm / AppImage. Auto-update: configure Tauri's `updater` plugin pointing at a GitHub Releases endpoint; sign update artifacts with a project-owned key. Document the release process in `docs/release.md`: version bump → tag → CI builds → review and publish. Crash reporting hooks (S51) wired into the release builds, opt-in only.

---

### S51. Crash reporting (opt-in)

**Scope:** Sentry or self-hosted GlitchTip instance. Opt-in by user — first-launch dialog, never enabled by default. Captures Rust panics and JS errors. Strips PII.

**Depends on:** S13 (preferences UI for opt-in toggle).

**Interfaces:** Maintainers.

**Agent brief:**
> Implement crash reporting with `sentry-rust` and `@sentry/browser` for the JS layer. Self-host on GlitchTip (open-source Sentry-compatible) on a small VPS, or use Sentry Free tier — make the call based on long-term cost. Behavior: opt-in only. First-launch dialog asks "Help improve Pixhaus by sending anonymous crash reports?" with prominent No button. Setting persisted in user prefs. When enabled, crashes capture: stack trace, OS, Pixhaus version, anonymized user ID. Stripped: file paths (replaced with `<user>`), filenames of opened projects, palette contents, anything that could identify content. Toggle in preferences. Document the policy in `docs/privacy.md`.

---

### S52. Visual regression test harness

**Scope:** Screenshot-based regression testing for the canvas and UI. Runs via Tauri WebDriver or Playwright. CI runs the full suite on PRs. Diff visualization for failures.

**Depends on:** B1, S14.

**Interfaces:** Every UI stream consumes this for tests.

**Agent brief:**
> Build a visual regression test harness in `tests/visual/`. Stack: Playwright with `@tauri-apps/playwright-driver` (or WebDriver if Playwright doesn't support Tauri 2 yet — confirm). Tests render specific scenes (a canvas with a known sprite, a layer panel state, a timeline with frames) and compare screenshots against committed baselines using `pixelmatch` for pixel-diff with a small tolerance (1-2% to absorb font rendering variance). Failures upload diff images to the CI artifact store. Tests run in headless mode on CI, headed mode locally. Sample tests cover: canvas with a 32x32 sprite at 100% zoom, canvas at 800% zoom showing pixel grid, layer panel with 5 layers + 1 group, timeline with 10 frames + 3 tags. Each UI stream contributes its own visual tests; this stream just ships the harness.

---

## How to dispatch

The recommended order:

1. **Day 0:** Bedrock B1 (scaffold) and B8 (handbook). Single agent, sequential. ~2 days.
2. **Day 2:** Bedrock B2 (data model) on its own — high stakes, single agent, careful review. ~3 days.
3. **Day 5:** Bedrock B3, B4, B5, B6, B7 in parallel. 5 agents. ~3 days each.
4. **Day 8:** Critical-path streams in parallel — S01, S02, S05, S06, S07, S08, S10, S13, S14, S21, S22, S39, S49. ~13 agents. 1-3 weeks each.
5. **Day 22:** Remaining streams in parallel. As many agents as you can manage. 1-3 weeks each.

The dependency graph means that even if every stream takes 3 weeks, the project's wall-clock is ~6-8 weeks of agent time, not the additive 80+ weeks of agent-hours total. That's the parallelism paying off.

Don't dispatch all streams the same day. The bottleneck is review, not execution — humans (you) reviewing agent output is the rate limiter. Plan to review one to two streams per day.

## What's not in here

- A dedicated stream for AI safety / responsible AI considerations. This should arguably be a stream of its own — content policy for verbs, watermarking AI-generated assets, attribution requirements. Add as S53 if you want it explicit.
- Localization (i18n / l10n). Defer until launch.
- Mobile (iPad). Defer entirely; this is a desktop tool.
- Web build of the editor itself. Tauri 2's mobile/web targets aren't mature enough; defer.

If a feature you want isn't in this list, it goes in a new stream with its own brief. The list isn't a limit; it's a starting set.
