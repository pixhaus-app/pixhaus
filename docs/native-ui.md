# Native UI migration: Tauri webview → egui + wgpu

Status: proposed. Supersedes the locked "UI runtime: Tauri 2.x" decision in `docs/planning/architecture/stack.md`. This document is both the architecture decision record and the full migration playbook.

## Context

### The problem

Drawing and animation playback are rough at 4K and 8K canvases. Both symptoms trace to a single architectural seam, not to any tunable webview setting.

Pixel data lives in Rust. The GPU surface that displays it lives **inside the webview**, in a WebGL2 context owned by JavaScript (`ui/src/canvas/renderer/index.ts`). Every pixel the user sees has to cross the IPC boundary: Rust composites a tile, ships it over a channel, and JS uploads it with `texImage2D`. At 32x32 this is free. At 8192x8192 with a wide brush, one stroke dirties a region spanning many 256px tiles (256 KB each), and that volume is shipped and re-uploaded every frame of the drag. Playback re-streams a whole composited frame's worth of tiles on every frame change.

We have already taken the easy wins on this path:
- `50468e3` moved tile streaming off the base64 JSON event bridge onto a binary IPC channel, because that bridge "makes drawing unusable on WebView2 (Windows)" (`ui/src/canvas/Canvas.tsx:105`).
- `65d2fc0` added incremental stroke rasterization and per-tile compositing.

No encoding trick removes the copy itself. The bandwidth across the seam is the wall.

### Why native rendering fixes it

If Rust owns the GPU surface directly through `wgpu`, the composited texture never leaves the GPU. Rust writes only the dirty sub-rect into a GPU texture once (`queue.write_texture`) and draws. There is zero IPC for pixels, and input flows straight into the drawing code as a function call instead of an `invoke` round trip. That removes both halves of the symptom.

### The honest scope

The perf payoff comes almost entirely from the canvas going native. The rest of the UI — about 30k lines of Solid across 13 feature areas and 155 IPC commands — is not what's slow. Rewriting it is a bet on long-term cohesion (one language, no webview quirks, no IPC seam anywhere), not on speed. Several feature areas are genuinely web-native and carry real porting risk: the animation studio plays multi-MB video clips, the palette embeds a Lospec web browser, the reference sheet is 3,200 lines of canvas editing and variant history. Those risks are addressed per-area below.

## Decision

1. Replace the Tauri webview shell with a native Rust shell built on **eframe + egui + egui-wgpu**.
2. Lift the canvas renderer out of TypeScript/WebGL2 into a UI-agnostic Rust **`render/` crate** on `wgpu`.
3. Collapse the 155-command Tauri IPC layer into direct in-process function calls against the existing logic crates.
4. Migrate with a **strangler pattern** at the binary level: the Tauri app stays shippable until the native shell reaches parity, then `ui/` and most of `app/` are deleted.

### Why egui

The deciding constraint is the MIT license lock (`CLAUDE.md`: no GPL/LGPL/AGPL without explicit approval). That rules out **Slint** (GPL-or-commercial). Also ruled out for now: **Xilem/Vello** (pre-1.0, unstable API — too risky to ship a product on) and **GPUI** (Zed's; fast and proven but not published as a stable semver crate — you would vendor Zed's monorepo).

Among the remaining MIT/Apache options — egui, iced, makepad, floem — egui wins on the make-or-break requirement: embedding a custom `wgpu` render pass inside a UI region is a first-class, well-trodden path (`egui_wgpu::Callback`), proven at pro-tool scale by rerun.io, and backed by the largest contributor pool, which matters for an open-source project. The cost is immediate-mode ergonomics for heavy dialog and text UIs, which is a learning curve, not a blocker.

## Target architecture

### Crate topology

The logic crates do not know what the UI is, so the rewrite is narrower than the line count suggests.

```
Untouched (all logic, all tests):
  core/         pixel ops, blend, undo, project model, selection, transforms, tools
  io/           .pixhaus, .aseprite, .psd, PNG, GIF/WebP, TMX, video decode
  ai/           verb runtime, backend adapters, built-in verbs, job manager
  scripting/    Lua bindings
  (vectorize, plugins as they exist)

New:
  render/       UI-agnostic wgpu viewport renderer. Depends on core + wgpu.
                Knows nothing about egui. The perf-critical code.
  shell/        eframe + egui + egui-wgpu binary. Owns DocumentStore, hosts
                panels, embeds render/ via an egui paint callback. Depends on
                everything.

Deleted at the end of the migration:
  ui/           all Solid/TypeScript
  app/          the Tauri shell + the IPC command layer (most of it; see below)
```

Keeping `render/` independent of egui is deliberate. If egui ever disappoints, the perf-critical renderer survives the divorce. It also stays unit-testable against a headless `wgpu` device.

### Ownership model

Today `app/src/state.rs` holds an `AppState` with a `RwLock<DocumentStore>` because Tauri commands run on arbitrary threads and need shared access. The native shell collapses that. egui's update loop runs on one thread, so the `eframe::App` struct owns `DocumentStore` directly as a plain field and mutates it through `&mut self`. No `RwLock` for UI-thread access. This matches the repo rule "every project has a single owner; avoid `Arc<Mutex<>>` except at the app boundary" better than the current code does.

What still needs sharing is background work: AI verb invocations, file save/open, video decode. Those keep `Arc` (the existing `Arc<VerbRuntime>`, `Arc<PluginRegistry>`) and run on a tokio runtime the shell owns. Results come back over channels that the egui `update()` drains each frame, calling `ctx.request_repaint()` when something lands. This replaces Tauri's async command + event model with an explicit message pump the shell controls.

### Data flow, before and after

Drawing, before:
```
pointer (JS) -> invoke canvas_extend_stroke -> Rust rasterize dirty region
  -> composite affected tiles -> serialize -> binary channel -> JS
  -> texImage2D into WebGL2 -> raf draw
```

Drawing, after:
```
pointer (egui Response) -> core::canvas::tools::draw_stroke(&mut buffer, ...)
  -> render crate composites dirty tiles -> queue.write_texture(dirty sub-rect)
  -> egui paint callback draws the textured quads
```

The serialize / channel / re-upload middle disappears. So does the JS side.

## The `render/` crate (Phase 1 core)

This is where the perf win lives and the first thing built. It is a direct port of `ui/src/canvas/renderer/` to `wgpu`.

### Public API (port of `CanvasRenderer`)

Mirror the existing TS class so the porting is mechanical and reviewable:

```rust
pub struct ViewportRenderer { /* owns wgpu resources */ }

impl ViewportRenderer {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self;
    pub fn set_viewport(&mut self, vp: ViewportConfig);     // scroll_x, scroll_y, zoom, width, height
    pub fn set_sprite(&mut self, sprite: Option<SpriteConfig>);
    pub fn set_selection(&mut self, sel: SelectionConfig);  // rect or per-pixel mask
    pub fn set_onion_skin(&mut self, onion: OnionSkinConfig);
    pub fn set_major_grid(&mut self, grid: MajorGridConfig);
    pub fn upload_tile(&mut self, key: TileKey, device: &wgpu::Device, queue: &wgpu::Queue, data: &TileData);
    pub fn paint(&self, render_pass: &mut wgpu::RenderPass);  // called from egui callback
}
```

The config structs map one-to-one from the TS interfaces (`ViewportConfig`, `SpriteConfig`, `SelectionConfig` with rect+mask, `OnionSkinConfig`, `MajorGridConfig`).

### Shader port (GLSL -> WGSL)

Six programs in `shaders.ts` become WGSL. Hand-porting six small shaders is faster and clearer than running naga; keep the names so review is a diff:

| Program | What it draws | Uniforms to port |
|---|---|---|
| SPRITE | procedural 8x8 checkerboard + tile texture, alpha-composited | `u_resolution`, `u_scroll`, `u_zoom`, `u_tile` (sampler), `u_has_tile`, `u_tile_origin_canvas`, `u_tile_size_canvas` |
| GRID | 0.75px antialiased per-pixel grid, `fwidth`-stable | `u_resolution`, `u_scroll`, `u_zoom` |
| MAJOR_GRID | 1.0px cyan lines every `u_spacing` canvas px | + `u_spacing` |
| ONION | neighbour-frame tiles tinted red/blue, additive | + `u_tint`, `u_opacity` |
| ANTS | rect marching ants, arc-length perimeter, 8px period, 30px/s | `u_sel_min`, `u_sel_max`, `u_time` |
| MASK_ANTS | mask-outline marching ants over an R8 texture, diagonal phase | `u_mask` (R8 sampler), `u_mask_min`, `u_mask_size`, `u_time` |

Port notes:
- `COMMON_VERT` (canvas->screen transform with scroll, zoom, centering) becomes a shared WGSL vertex entry point.
- GLSL uniforms become `wgpu` bind groups + a uniform buffer. Pack the shared viewport uniforms (`resolution`, `scroll`, `zoom`, `time`) into one buffer reused across passes.
- `fwidth` exists in WGSL.
- Sampling stays NEAREST, CLAMP_TO_EDGE.
- The mask stays single-channel (`R8Unorm`); WebGL's `UNPACK_ALIGNMENT=1` dance has no `wgpu` equivalent — `write_texture` takes an explicit `bytes_per_row`.

### Tile cache (port of `tile-cache.ts`)

- `TILE_SIZE = 256` stays.
- `TileKey` becomes a struct `{ sprite_id, frame_index, tile_x, tile_y }` (was a `:`-joined string).
- Each tile is a `wgpu::Texture` (`Rgba8Unorm`, NEAREST, CLAMP_TO_EDGE) with a per-tile bind group. Start one-texture-per-tile to match the current model; a texture-array or atlas is a later optimization if bind-group churn shows up in profiling.
- The win: `upload_tile` calls `queue.write_texture` with only the dirty sub-rect, not a whole-tile re-upload. The current code re-uploads the whole 256x256 tile via `texImage2D` on every change — porting faithfully but writing only the dirty rows is the single biggest drawing speedup.
- Eviction stays manual and sprite-scoped (`evict(sprite_id)` on sprite switch). No `markAllDirty`/context-loss path is needed: `wgpu` surface loss is handled by eframe recreating the surface, and the texture cache can be rebuilt from `core` on demand.

### Viewport math (port of `viewport.ts`)

Port these pure functions to Rust unchanged; they are UI-agnostic and belong in `render/` (or a shared `geom` module):
`screen_to_canvas`, `canvas_to_screen`, `zoom_at`, `snap_zoom`, `clamp_zoom`, `fit_zoom`, `scroll_to_centre`.
Constants carry over verbatim: `PIXEL_GRID_ZOOM_THRESHOLD = 4`, `SNAP_ZOOMS = [1/16 .. 16]`, `MIN_ZOOM = 1/16`, `MAX_ZOOM = 16`.

### Embedding in egui

In the central panel, allocate the viewport rect and register a paint callback:

```rust
let (rect, response) = ui.allocate_exact_size(avail, egui::Sense::click_and_drag());
let cb = egui_wgpu::Callback::new_paint_callback(rect, ViewportPaint { /* state snapshot */ });
ui.painter().add(cb);
```

`ViewportPaint` implements `egui_wgpu::CallbackTrait`: `prepare()` uploads any dirty tiles into the renderer's textures using the `RenderState` device/queue; `paint()` issues the draw passes into egui's render pass. The renderer's GPU resources live in egui's `CallbackResources` type map (the rerun.io pattern), so they persist across frames. egui's own UI (panels, overlays) composites on top in the same frame — the brush cursor, transform handles, and shape preview (currently SVG overlays in `overlays.tsx`) become egui shapes drawn over the callback rect.

### Input (port of `input.ts`)

`response` from the viewport allocation gives pointer position, drag deltas, modifiers, and wheel. Port the routing priority exactly:
1. transform drag in progress
2. space-drag / middle-mouse pan
3. tilemap paint mode
4. selection mode
5. draw / fill tools

Each branch calls `core` directly instead of an `invoke`:

| Input | Current IPC | Native call |
|---|---|---|
| freehand stroke | `canvas_begin/extend/end_stroke` | drive a `StrokeSession` in `DocumentStore`, call `core::canvas::tools::draw_stroke`, upload dirty tiles |
| shape (line/rect/ellipse) | `canvas_draw_stroke` | same, one-shot |
| fill | `canvas_fill` | `core::canvas::tools::flood_fill` |
| tile paint/erase | `tile_place` / `tile_erase` / `tile_autotile_apply` | `core::tilemap::*` + undo |
| selection | `canvas_select_*` | `core::selection::algorithms::*` |
| pan/zoom | signals + debounced `canvas_set_viewport` | mutate viewport state directly; no sync needed |

The RAF-batched extend-stroke promise chain disappears: drawing is synchronous in the egui frame, so points accumulate and rasterize inline, bounded by the dirty region.

## Collapsing the IPC layer

The 155 `#[tauri::command]` functions in `app/src/commands/` split into two piles.

**Pure plumbing, deleted:** the `#[tauri::command]` macros, `tauri::State` unpacking, serde derives that existed only to cross IPC, the `AppCommandError` -> JSON mapping, the `Channel<InvokeResponseBody>` tile transport, and `app.emit` calls. The events themselves (tile-dirty, tilemap cell-changed, job updates) become direct calls or channel messages.

**Genuine logic, preserved:** every call these commands make into `core`/`io`/`ai`/`scripting`. In the native shell the egui code calls those directly. The command bodies that contain real orchestration (lock discipline, undo recording, blocking-pool dispatch, verb context building) move into shell-side methods on `DocumentStore` or service structs — they keep their logic, lose their IPC wrapper.

Category-by-category collapse (full per-command catalog regenerable from `app/src/commands/`):

| Category | Cmds | Collapse pattern |
|---|---|---|
| app info | 1 | static, inline |
| canvas (draw/fill/select/composite/transform) | 24 | direct `core` calls; tile emission becomes `render` texture writes; `canvas_set_tile_channel` deleted entirely |
| frames / tags / cels | 12 | direct `Sprite` mutations + undo |
| layers | 16 | direct `Sprite.layers` mutations; merge/flatten emit tiles -> texture writes |
| palette | 16 | direct `core::palette` + History commands |
| library: entity/group/tag/search | 20+ | direct `project.library` mutations |
| library: composition | 11 | direct `library.compositions` mutations + pack I/O |
| library: reference sheets | 27 | `VerbRuntime::invoke` + request managers -> shell task registry (heaviest) |
| library: lora | 5 | training job enqueue -> shell job manager |
| animation studio | 18 | verbs + `AnimationJobManager` -> shell task registry; video decode already host-side |
| ai backend settings | 10 | `ApiKeyStore` + `VerbRuntime::register_backend` |
| project / sprites | 11 | `io` decode/encode on blocking pool; swap `DocumentStore.project` |
| exports | 4 | `io::{png,animated,tiled}` on blocking pool + `rfd` save dialog |
| tiles / tilesets | 9 | `core::tilemap` + undo |
| undo / redo | 2 | two-tier `PixelHistory` then `History`, preserved exactly |
| plugins | 4 | `PluginRegistry` calls |
| verbs | 3 | `VerbRuntime` + cancellation tokens in shell registry |
| updater | 3 | `tauri-plugin-updater` -> a native updater crate or self-update; lowest priority |
| crash reporting / samples | 3 | atomics + fs read |

Two structural items to preserve carefully, called out by the inventory:
- **Two-tier undo.** Pixel ops in `PixelHistory` take precedence over structural ops in `History`. Undo/redo check pixel history first. The shell must replicate this ordering.
- **Lock discipline becomes moot for UI-thread calls** (single owner), but background tasks must still never touch `DocumentStore` across an `.await`. The message-pump model enforces this: tasks get owned copies/handles, results return by channel.

## Threading and async

The shell owns a tokio runtime. Pattern for any operation that today is an `async` command doing I/O or inference:

1. UI thread builds an owned request (no borrows into `DocumentStore`).
2. `runtime.spawn` (or `spawn_blocking` for CPU/file work) runs it.
3. Result is sent on an `mpsc`/`oneshot` back to the UI.
4. `App::update` drains the channel each frame, applies results to `DocumentStore`, calls `ctx.request_repaint()`.

This covers: project open/save, all exports, every AI verb, reference-sheet generation and chat streaming, LoRA training, i2v jobs, video decode for the frame picker, background removal. Streaming results (chat turns, first-frame candidates, job progress) push multiple messages over the channel; the UI renders whatever has arrived.

## Migration phases

Strangler at the binary level. Two shippable binaries share the logic crates. The Tauri build stays the default until native reaches parity around Phase 5, then is deleted.

### Phase 0 — Scaffold

- ADR committed (this document, referenced from `stack.md`).
- `render/` crate created, empty `ViewportRenderer` compiling against a headless `wgpu` device with a smoke test.
- `shell/` binary created: eframe + egui + egui-wgpu, empty window, owns an empty `DocumentStore`, a tokio runtime, and the results channel pump.
- CI builds both binaries on Windows/macOS/Linux.
- No behavior. This is the skeleton.

### Phase 1 — Native canvas, the go/no-go gate

Build the perf proof before sinking effort into panels.

- Port the six shaders, the tile cache (dirty-sub-rect uploads), and viewport math into `render/`.
- Wire `core` compositing to write tiles directly into `render/` textures in-process.
- Embed the viewport via the egui paint callback.
- Port input routing for pencil/eraser/fill and pan/zoom only.
- A minimal toolbar (port of `Toolbar.tsx`) and a hard-coded test sprite or `project_open` via `rfd`.
- **Gate:** load a 4096x4096 and an 8192x8192 sprite. Measure stroke latency (pointer-to-paint) and playback FPS against the current Tauri build on the same hardware. If native is not decisively faster, stop here — weeks spent, not months. Record numbers in the PR.

Overlays (brush cursor, shape preview) as egui shapes over the callback rect.

### Phase 2 — Core editing panels

- Layers (`LayerPanel`, `LayerRow`, context menu): tree with collapse/expand, drag-to-reorder, inline rename, per-row blend/opacity on the active row. egui `ScrollArea` with manual virtual scrolling (port the binary-searched row-offset scheme for 500+ layers).
- Palette (`PalettePanel`, `ColorPicker`, `PaletteGrid`, `FgBgSwatches`): HSL+value picker, swatch grid, pages. Port `color-utils.ts` math to Rust. Defer Lospec browser and harmony/ramp to a later pass.
- Tool options (`BrushSection`, `FillSection`, `DitheringSection`).
- Wire selection and transform UX end to end (from Phase 1 input).

### Phase 3 — Timeline and playback

- `TimelinePanel`: 2D virtual scroll (frame headers, layer column, cel grid) via nested egui `ScrollArea` with synced offsets, or a manual grid with culling. Frame width 32, row height 24 carry over.
- Playback timer driven by `ctx.request_repaint_after` instead of `setInterval`; loop modes (forward, ping-pong).
- Onion skin wired to the renderer's ONION pass.
- Frame ops + tags context menu.

### Phase 4 — Tilemap and selection finishing

- `TilemapPanel` three tabs: tileset grid browser, autotile rule editor (3x3 neighbor grid), tileset CRUD.
- Autotile resolve already in `core::tilemap::resolve_autotile`.
- Polish marquee/lasso/wand/color-range UX and transform handles.

### Phase 5 — AI surfaces, the long pole

- Animation studio (`animation/`, ~2,100 lines, four-stage pipeline). Video playback becomes decode-to-frames-in-Rust (`io::animated::decode_video`, already used by `animation_pick_frames`) swapped onto a texture on a timer — no HTML `<video>`. First-frame candidate streaming, i2v job polling, background removal, frame picker, normalization review, mask-inpaint canvas all route through the shell task registry + channel pump. Per-sprite studio state persistence stays (serialize into the project on close).
- Reference sheet (`sheet/`, ~3,200 lines): sheet canvas editing (reuse the Phase 1 drawing path), panel-region editor, variant history strip, prompt strip, LoRA training status, asset metadata CRUD.
- Composition library (`composition-library/`): structures/styles/prompts editors, built-in vs project records, `.pixstyle` import/export.

### Phase 6 — Chrome, parity, deletion

- Shell chrome: right-rail accordion (port `rail-state.ts` auto-open rules), status bar, welcome screen, canvas-size dialog.
- Command palette (`command-palette/`): fuzzy match, scoring, keyboard nav. Port `command-registry.ts`.
- Preferences (five tabs: general, keybinds, AI backends, plugins, privacy). Keybind rebinding UI.
- Menus: native menu bar via egui or platform menus; map the existing Tauri menu-event dispatch to the command registry.
- Updater: native self-update path or drop in favor of package managers.
- Theming: re-create the CSS-variable look in egui's `Style`/`Visuals`. Accept that pixel-perfect parity is real work.
- **Cut over:** make the native binary the default build. Delete `ui/` and the IPC layer in `app/`. Keep whatever genuine setup logic from `app/` the shell still needs (it should be little).

Each phase is its own branch and PR, referencing this document.

## Per-area port difficulty and gotchas

Easiest to hardest, from the UI inventory:

1. Shell layout, command palette, preferences — standard egui widgets and layouts.
2. Palette math, tool options — library-agnostic; the math ports cleanly.
3. Canvas — needs the custom `wgpu` integration (Phase 1); the hard part is done first on purpose.
4. Layers, timeline — egui `ScrollArea` exists but virtual scroll (especially the timeline's 2D case) is hand-rolled. Drag-to-reorder is manual hit-testing in egui, not the HTML5 drag API.
5. Tilemap — medium UI, no special rendering.
6. Animation studio — highest complexity: multi-stage state machine, streaming, video. De-risked by host-side frame decode and the channel pump, but still the long pole.
7. Reference sheet — canvas editing (reuses Phase 1), panel editor, training status.

Specific web-isms that need a native answer:
- **Video playback** -> decode to frames in Rust, animate a texture. Already have the decoder.
- **Lospec browser** -> `reqwest` fetch on a background task; render thumbnails into textures. (Low priority; can ship without it initially.)
- **Blob URLs / base64 image streams** -> raw bytes into textures; no browser indirection.
- **HTML5 drag-and-drop** -> egui pointer hit-testing and drop zones.
- **`localStorage` preferences** -> a config file (the backend was already authoritative for crash-reporting; extend that).
- **File dialogs** -> `rfd`.
- **SVG overlays** -> egui `Painter` shapes.

## Risks

- **Phase 1 fails the gate.** Mitigation: the gate exists precisely to cap the loss at weeks. Measure before building panels.
- **egui immediate-mode ergonomics** for dialog-heavy, text-entry surfaces (preferences, editors). Mitigation: these are late phases; by then the team is fluent, and egui's text/IME is adequate for a tool.
- **Accessibility regression.** AccessKit (egui's a11y layer) is good but not at DOM parity. Wire it from Phase 0; flag explicitly if a11y is a hard requirement.
- **Theming parity** is manual work; budget for it in Phase 6.
- **Virtual scroll** for layers/timeline is hand-rolled; the existing TS algorithms (binary-searched offsets) port directly, so this is effort, not unknown risk.
- **Animation studio scope.** Mitigation: it is last, fully specified by the inventory, and its hardest dependency (video) is already solved host-side.
- **Two long-lived builds** during migration. Mitigation: shared logic crates mean features land once; only the thin shell differs. Keep both in CI.

## Verification

- Phase 0: both binaries launch on all three OSes in CI; `render/` smoke test passes on a headless `wgpu` device.
- Phase 1: the documented 4K/8K benchmark, native vs Tauri, numbers in the PR. This is the project's justification — without a decisive win, do not proceed.
- Each later phase: feature parity checked against the Tauri build for that area, plus the existing `core`/`io`/`ai` test suites (unchanged) staying green via `cargo nextest run --workspace`.
- Visual regression on the canvas via the existing image-compare harness, pointed at the native renderer's output.
- Final: the Tauri build and native build produce byte-identical `.pixhaus` files for the same edit sequence (the file format is owned by `io`, untouched).

## What this does not change

`core`, `io`, `ai`, `scripting`, the `.pixhaus` format, the verb protocol, the plugin protocol, the AI backend adapters, and every test in those crates. The migration is confined to how pixels reach the screen and how the UI calls into logic that already exists.
