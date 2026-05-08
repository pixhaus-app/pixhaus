# Pixhaus manual test guide

A structured walkthrough of every UI-testable surface in the Pixhaus desktop app.

This doc has two readers:

1. **A human running `pnpm dev`** who needs a checklist to drive the app and confirm each surface still works.
2. **The future e2e author.** Each scenario below maps cleanly to one Playwright test block.

The format is therefore terser than prose. Treat each test ID as a stable identifier; the e2e suite will reference them by name (`test('T-tools-001 ...', ...)`).

## How to use

1. Pull `main`, run `pnpm install`, run `pnpm dev`. A Tauri window opens.
2. On first run, dismiss the crash-reporting consent dialog (T-launch-001 covers it).
3. Walk the sections top to bottom. Sections are ordered the way a tester would naturally exercise the app: launch → project → canvas → tools → panels → menu/palette.
4. For every test, check the **Expect** lines. Anything that fails: open an issue and reference the test ID.

## Test ID convention

Base form: `T-<area>-<NNN>`. The areas are: `launch`, `project`, `export`, `canvas`, `tools`, `select`, `transform`, `layers`, `palette`, `timeline`, `tilemap`, `cmd` (command palette), `window`, `help`, `keys`. Numbers are stable — never renumber, only append.

Two extensions are explicitly allowed for compactness:

- **Range notation** (`T-window-001..004`) when a small set of tests differ only in which target the same scenario applies to (e.g. one test per panel). Each number in the range is its own test ID; the e2e suite expands them into individual `test('T-window-001: ...')` blocks.
- **Letter suffix** (`T-cmd-003a`, `T-cmd-003b`, …) when several near-identical commands share a scenario template and a comparison table. The base number names the scenario; the suffix names the variant. Each ID is still unique.

Both shorthands map 1:1 to e2e test names; the range `T-window-001..004` becomes four tests, not one.

## Per-test format

The full form, used for any scenario with non-trivial steps or assertions:

```
### T-area-NNN: <one-line scenario>
Pre: <state to be in before>
Steps:
  1. <action>
  2. <action>
Expect:
  - <[VISUAL] | [DOM] | [IPC] | [STATE]> <observable assertion>
```

The bracket prefix on each Expect tells the future automator which kind of check to wire:

- **[DOM]** — a Testing-Library / Playwright DOM query (text, attribute, aria-state).
- **[VISUAL]** — pixel-diff via the existing `tests/visual/` harness.
- **[IPC]** — a Tauri command spy in `tests/visual/helpers/tauri-mock.ts`.
- **[STATE]** — a devtools-readable signal value (Solid signals are inspectable via the project-state module).

Two compact shorthands are allowed when the full form would be repetitive:

- **Inline shorthand**: `### T-area-NNN: <scenario> — <one-sentence steps + expected>.` Used when the scenario is a trivial variant of the previous test (same shape, different target). The automator should expand it back to the full form.
- **Comparison table**: when several near-identical scenarios fit in a row each, a markdown table headed by an Expected column carries the per-row assertion. Every row's first column is its own test ID. See `T-cmd-003` and the keyboard shortcut sweep (`T-keys-NN`) for examples.

In both shorthands, an automator should still produce one `test('T-...', async () => { ... })` per ID, with the assertion derived from the row / sentence.

---

## 1. Setup & cold start

### T-launch-001: First-launch crash-reporting dialog appears

Pre: fresh user profile (`~/.pixhaus` empty or non-existent).
Steps:
  1. `pnpm dev`.
Expect:
  - [DOM] a modal dialog with title text "Help improve Pixhaus?".
  - [DOM] two buttons: "No thanks" and "Yes, send crash reports".
  - [DOM] the rest of the shell is rendered behind the modal but un-clickable (backdrop intercepts).

### T-launch-002: Decline crash reporting

Pre: T-launch-001 dialog is showing.
Steps:
  1. Click "No thanks".
Expect:
  - [DOM] dialog closes.
  - [STATE] `crashReportingEnabled` signal is `false`.
  - [STATE] `crashReportingDialogShown` is `true` (dialog won't reappear next launch).

### T-launch-003: Welcome screen renders with all sections

Pre: project is closed (no active project).
Steps:
  1. Observe the welcome screen.
Expect:
  - [DOM] header "Pixhaus" + subtitle.
  - [DOM] two primary buttons: "New Project" and "Open Project...".
  - [DOM] a "Samples" section listing the 5 bundled samples (see section 2).
  - [DOM] a "Recent" section appears only if recent projects exist (empty on fresh profile).

---

## 2. Project lifecycle

### T-project-001: New Project creates a 32×32 sprite with one layer and one frame

Pre: welcome screen visible.
Steps:
  1. Click "New Project".
Expect:
  - [DOM] welcome screen unmounts; editor shell mounts (canvas + layer panel + timeline + palette).
  - [VISUAL] canvas shows a 32×32 transparent checkerboard.
  - [DOM] layer panel contains exactly one row labelled "Layer".
  - [DOM] timeline panel contains exactly one frame.
  - [DOM] "Add Layer" (`+`) button is enabled.
  - [DOM] "New Palette" (`+`) button is enabled.
  - [IPC] one `project_new` followed by one `sprite_add` then one `project_get` (via the `createNewProject` helper).

> **Critical:** this is the regression we fixed via `createNewProject`. If the canvas is empty / layer panel says "Open a project to see layers", the seed sprite step regressed.

### T-project-002: Open a sample project from the welcome screen

Pre: welcome screen visible.
Steps:
  1. Click the `character-knight` entry under Samples.
Expect:
  - [DOM] editor shell mounts.
  - [DOM] timeline panel shows multiple frames (the knight has 167 in the canonical fixture).
  - [DOM] palette panel populated with the knight's indexed palette.
  - [IPC] one `list_samples` (on welcome mount), one `project_open` against the resolved sample path.
  - [STATE] `recentProjects` now contains an entry for the sample.

Other sample fixtures are testable the same way: `enemy-slime`, `level-forest`, `tileset-forest`, `ui-sprites` (all under `examples/samples/`).

### T-project-003: Open an `.aseprite` file via File > Open

Pre: welcome screen.
Steps:
  1. Click "Open Project...".
  2. Navigate to `examples/aseprite-roundtrip/single-frame-rgba.aseprite`. Confirm.
Expect:
  - [DOM] editor opens with the imported sprite.
  - [DOM] title bar / status bar shows the project is dirty (no `.pixhaus` path yet).
  - [IPC] `project_import_aseprite` (NOT `project_open` — the extension router routes it).

Repeat with `multi-frame-tags.aseprite`, `indexed-with-palette.aseprite`, `tilemap-with-tileset.aseprite`, `group-multiply.aseprite` to cover frame tags, indexed palettes, tilemaps, and group/blend semantics.

### T-project-004: Save with no path falls through to Save As dialog

Pre: a dirty project from T-project-003 (no on-disk path yet).
Steps:
  1. Press `Ctrl+S`.
Expect:
  - [DOM] a native save dialog appears, filtered to `.pixhaus`.
  - [IPC] one `project_save` returning `Validation { kind: "validation", message: "save requires a path..." }`, then one `dialogSave`, then a second `project_save` with the chosen path.
  - On user cancel: no toast (cancel is silent), project stays dirty.

### T-project-005: Save updates dirty flag

Pre: project saved at least once via T-project-004.
Steps:
  1. Make a small edit (one pencil click).
  2. `Ctrl+S`.
Expect:
  - [STATE] before save: `dirty === true`.
  - [STATE] after save: `dirty === false`.
  - [IPC] one `project_save` with the existing path (no dialog).

### T-project-006: Close Project returns to welcome screen

Pre: project open.
Steps:
  1. File > Close, OR `Ctrl+W`, OR command palette → "Close Project".
Expect:
  - [DOM] editor shell unmounts; welcome screen mounts.
  - [STATE] `activeProject` is `null`, `activeSpriteId` is `null`.
  - [IPC] `project_close`.

### T-project-007: Recent Projects list updates after open

Pre: at least one save or open has happened.
Steps:
  1. Close the project.
Expect:
  - [DOM] welcome screen "Recent" section lists the just-closed project, name + path.
  - Click the recent entry → reopens via the same extension router as T-project-003.

---

## 3. Export

### T-export-001: PNG sprite sheet export

Pre: a project with at least one sprite is open.
Steps:
  1. File > Export > "PNG Sprite Sheet...".
  2. Pick an output path. Confirm.
Expect:
  - [IPC] `export_png_sprite_sheet` with the active sprite id and chosen path.
  - The output file exists on disk and decodes as a valid PNG.
  - [VISUAL] visual: open the PNG in an image viewer; frames are laid out in a strip.

### T-export-002: Animated GIF export

Pre: a multi-frame project (T-project-002 with `character-knight` qualifies).
Steps:
  1. File > Export > "Animated GIF...".
  2. Pick a path. Confirm.
Expect:
  - [IPC] `export_animated_gif`.
  - Output `.gif` plays the animation when opened.

### T-export-003: Animated WebP export

Same as T-export-002 with WebP target. [IPC] `export_animated_webp`.

### T-export-004: Tilemap TMX export

Pre: a project with a tilemap layer (T-project-002 with `level-forest`).
Steps:
  1. File > Export > "Tilemap (Tiled .tmx)...".
  2. Pick a path. Confirm.
Expect:
  - [IPC] `export_tmx`.
  - Output `.tmx` opens cleanly in Tiled (1.10+).

---

## 4. Canvas viewport

### T-canvas-001: Spacebar + drag pans the canvas

Pre: project open.
Steps:
  1. Hold `Space`. Cursor turns to grab.
  2. Left-mouse drag.
Expect:
  - [VISUAL] canvas content moves with the cursor.
  - [STATE] `scrollX` / `scrollY` signals change.
  - [IPC] no IPC during the drag; one `canvas_set_viewport` fires after release once the debounce settles. Don't pin a target latency — the debounce window varies and brittle timeouts make the e2e suite flaky.

### T-canvas-002: Middle-mouse drag pans

Pre: project open.
Steps:
  1. Middle-mouse drag (no spacebar).
Expect: same as T-canvas-001.

### T-canvas-003: Wheel scroll zooms (smooth)

Pre: project open, cursor over canvas.
Steps:
  1. Scroll wheel up.
Expect:
  - [STATE] `zoom` signal increases continuously (factor 1.1 per tick).
  - [VISUAL] zoom is anchored at the cursor (point under cursor stays under cursor).

### T-canvas-004: Ctrl+wheel snaps zoom to power-of-2 levels

Pre: zoom at 1.0.
Steps:
  1. Ctrl+wheel up.
Expect:
  - [STATE] `zoom` becomes 2.
  - Repeat → 4 → 8 → ...

### T-canvas-005: Keyboard zoom shortcuts

Pre: zoom at 1.0.
Steps:
  1. `Ctrl+=` zooms in (snaps).
  2. `Ctrl+-` zooms out.
  3. `Ctrl+0` fits sprite to viewport.
  4. `Ctrl+1` resets zoom to 1.0.
Expect: [STATE] `zoom` reflects each step.

### T-canvas-006: Shift+wheel pans horizontally

Pre: project open.
Steps:
  1. Shift+wheel up.
Expect:
  - [STATE] `scrollX` changes; `scrollY` unchanged.

### T-canvas-007: Toggle pixel grid

Pre: zoom is high enough for the grid (>= ~4×).
Steps:
  1. Command palette → "Toggle Pixel Grid", OR menu View > "Toggle Pixel Grid".
Expect:
  - [STATE] `showPixelGrid` flips.
  - [VISUAL] thin lines appear/disappear between every canvas pixel.

Note: at default 1× zoom the grid never renders even if `showPixelGrid === true`. Threshold is intentional. If a tester reports "grid toggle does nothing", check zoom first.

### T-canvas-008: Toggle tile grid

Pre: project open.
Steps:
  1. `Ctrl+G`, OR command palette → "Toggle Grid".
Expect:
  - [STATE] `showTileGrid` flips.
  - [VISUAL] major-grid lines appear at the configured spacing (default 8).

### T-canvas-009: Onion skin toggle

Pre: a multi-frame project. Active frame is not the first.
Steps:
  1. Click the onion skin toggle in the timeline header.
Expect:
  - [STATE] `onionSkin` flips to `true`.
  - [VISUAL] adjacent frames (default 1 prev, 1 next) overlay at reduced opacity.

Note: onion skin currently relies on the tile cache populated by drawing. On a freshly-opened sample with no edits, the overlay may be empty until you scrub to a different frame and back. Document as a known limitation; it'll improve as the renderer's pixel-fetch path matures.

---

## 5. Drawing tools (the freshly merged real-time path)

> **Critical regression coverage:** these tests assert the begin/extend/end work from PR #104.

### T-tools-001: Pencil drag paints in real time

Pre: project open, pencil tool active (press `P` or click pencil in toolbar), foreground colour is something visible (red), brush size 1, brush shape pixel.
Steps:
  1. Press and hold left mouse button on the canvas at canvas-pixel `(2,2)`.
  2. WHILE still holding, move the mouse to `(10,10)` slowly.
  3. Release.
Expect:
  - [VISUAL] **pixels appear under the cursor as it moves**, NOT only on release. This is the real-time stroke regression-guard.
  - [IPC] sequence: one `canvas_begin_stroke`, ≥1 `canvas_extend_stroke` (one per RAF tick during the drag), one `canvas_end_stroke`.
  - [IPC] every `canvas_extend_stroke` and the `canvas_end_stroke` have the same `session_id` from the begin response.
  - [VISUAL] after release, a continuous line of red pixels from `(2,2)` to `(10,10)`.

### T-tools-002: Eraser drag

Pre: a layer with painted pixels; eraser tool active (press `E`).
Steps:
  1. Drag across painted pixels.
Expect:
  - [VISUAL] painted pixels disappear under cursor in real time.
  - [IPC] same sequence as T-tools-001 but the `canvas_begin_stroke` arg has `erase: true`.

### T-tools-003: One Ctrl+Z reverts an entire drag

Pre: T-tools-001 just completed.
Steps:
  1. Press `Ctrl+Z` exactly once.
Expect:
  - [VISUAL] every red pixel from the drag disappears in one undo step.
  - [STATE] one entry was popped off `pixel_history`.

> **Critical:** if Ctrl+Z only reverts a fragment of the drag, the per-extend undo bug has regressed.

### T-tools-004: Brush size

Pre: pencil active.
Steps:
  1. In the Tool Options panel, drag size slider to 4. Brush shape "circle".
  2. Click the canvas at `(8,8)`.
Expect:
  - [VISUAL] a 4-pixel-diameter filled circle painted around `(8,8)`.

### T-tools-005: Brush shape pixel/circle/square

Pre: pencil active, brush size 5.
Steps:
  1. Toggle each shape; click once.
Expect:
  - [VISUAL] pixel: single pixel only (size doesn't apply).
  - [VISUAL] circle: filled disk with ~5px diameter, anti-aliased to pixel grid.
  - [VISUAL] square: filled square, 5×5.

### T-tools-006: Pixel-perfect toggle

Pre: pencil active, brush size 1.
Steps:
  1. Pixel-perfect ON. Drag a diagonal line.
Expect:
  - [VISUAL] no L-corner artifacts (no two adjacent pixels in the same row + column).
  2. Pixel-perfect OFF. Same drag.
Expect:
  - [VISUAL] L-corners visible at every diagonal step.

### T-tools-007: Fill tool

Pre: a layer with a closed shape (e.g. a rectangle outline).
Steps:
  1. Press `F` (fill tool). Set tolerance 0.
  2. Click inside the shape.
Expect:
  - [VISUAL] all contiguous transparent pixels inside the shape fill with foreground.
  - [IPC] one `canvas_fill` with the click coords + tolerance.

### T-tools-008: Line tool

Pre: pencil active path open. Press `L`.
Steps:
  1. Click anchor at `(5,5)`. Drag to `(20,5)`. Release.
Expect:
  - [VISUAL] a horizontal line of pixels from `(5,5)` to `(20,5)`.
  - [IPC] one `canvas_draw_stroke` (NOT begin/extend/end — line is the one-shot path).

Note: line tool currently has no drag-time preview. The line only appears on release. This is documented as out-of-scope in the PR #104 description.

### T-tools-009: Rect tool

Pre: rect tool active (press `R`).
Steps:
  1. Click anchor at `(4,4)`. Drag to `(12,9)`. Release.
Expect:
  - [VISUAL] outline rectangle from `(4,4)` to `(12,9)`.
  - [IPC] one `canvas_draw_stroke` with the perimeter point list.

### T-tools-010: Ellipse tool

Pre: ellipse tool active (press `O`).
Steps:
  1. Click anchor at `(4,4)`. Drag to `(12,9)`. Release.
Expect:
  - [VISUAL] elliptical outline within the bounding box.
  - [IPC] one `canvas_draw_stroke` with midpoint-circle perimeter points.

---

## 6. Selection & transform

### T-select-001: Select All

Pre: project open.
Steps:
  1. `Ctrl+A`.
Expect:
  - [VISUAL] marching-ants marquee around the entire sprite canvas.
  - [STATE] `selectionRect` covers the full sprite size.
  - [IPC] `canvas_select_all`.

### T-select-002: Deselect

Pre: a selection exists.
Steps:
  1. `Ctrl+D`.
Expect:
  - [VISUAL] marquee disappears.
  - [STATE] `selectionRect` is `null`.
  - [IPC] `canvas_set_selection` with `region: null`.

### T-select-003: Drag selection body translates pixels

Pre: a non-empty rect selection over painted pixels.
Steps:
  1. Mouse-down inside the marquee. Drag by `(dx, dy)`.
  2. Release.
Expect:
  - [VISUAL] both the marquee AND the pixels under it shift by `(dx, dy)`.
  - [IPC] one `canvas_transform` with a single `Translate { dx, dy }` op, then one `canvas_set_selection` with the moved region.

> **Regression guard:** before PR #99 this only moved the marquee. If pixels stay put, the body-drag-translate regressed.

### T-transform-001: Flip horizontal

Pre: an asymmetric painted region.
Steps:
  1. Command palette → "Flip Horizontal".
Expect:
  - [VISUAL] painted region mirrored across the vertical axis (within the active selection if one is set, full layer otherwise).
  - [IPC] one `canvas_transform` with a `FlipHorizontal` op.

### T-transform-002: Flip vertical — same shape, `FlipVertical` op.

### T-transform-003: Rotate 90 CW

Pre: an asymmetric painted region.
Steps:
  1. Command palette → "Rotate 90° CW".
Expect:
  - [VISUAL] region rotated quarter-turn clockwise.
  - [IPC] `canvas_transform` with `Rotate90Cw`.

### T-transform-004: Rotate 90 CCW — same with `Rotate90Ccw`.

---

## 7. Layer panel

### T-layers-001: Add layer

Pre: project open with at least one sprite.
Steps:
  1. Click the `+` button in the layer panel header.
Expect:
  - [DOM] a new row "Layer 2" (or auto-incremented) appears at the top.
  - [STATE] `activeLayerId` updates to the new layer.
  - [IPC] one `layer_add`.

### T-layers-002: Rename via context menu

Pre: at least one layer.
Steps:
  1. Right-click a layer row → "Rename".
  2. Type "Background" → press Enter.
Expect:
  - [DOM] row label is "Background".
  - [IPC] one `layer_rename`.

### T-layers-003: Delete with confirmation

Pre: at least two layers.
Steps:
  1. Right-click a layer → "Delete".
  2. Confirm in the dialog.
Expect:
  - [DOM] the row disappears.
  - [IPC] `layer_delete`.

Note: Delete is disabled when the panel is down to one layer. Verify the disabled state by trying to delete the last remaining layer.

### T-layers-004: Drag to reorder

Pre: three layers A (top), B, C (bottom).
Steps:
  1. Drag B above A.
Expect:
  - [DOM] order is now B, A, C.
  - [IPC] `layer_reorder`.

### T-layers-005: Merge Down composites pixels and drops the active layer

Pre: two raster layers, top has a red square at `(5,5)`-`(10,10)`, bottom is fully transparent. Top is active.
Steps:
  1. Right-click top → "Merge Down", OR command palette → "Merge Down".
Expect:
  - [DOM] top row disappears; bottom row remains.
  - [VISUAL] bottom row's cel now contains the red square at `(5,5)`-`(10,10)`.
  - [IPC] `layer_merge_down`.

> **Regression guard:** before PR #103 this returned `Unimplemented` and silently failed. If the click does nothing, the backend regressed.

### T-layers-006: Merge Selected

Pre: three raster layers, all selected (Ctrl+click). Each has a different colour painted at a different position.
Steps:
  1. Right-click → "Merge Selected".
Expect:
  - [DOM] only one row remains (top of selection).
  - [VISUAL] the top layer now shows all three colours composited (per blend mode + opacity).
  - [IPC] `layer_merge_selected`.

### T-layers-007: Flatten Visible

Pre: 3 visible raster layers + 1 hidden raster layer.
Steps:
  1. Right-click → "Flatten Visible".
Expect:
  - [DOM] one composited row + one untouched hidden row remain.
  - [VISUAL] the composited row contains the visible layers' merged content.
  - [IPC] `layer_flatten_visible`.

### T-layers-008: Convert to Group

Pre: a regular raster layer.
Steps:
  1. Right-click → "Convert to Group".
Expect:
  - [DOM] the row becomes a group/folder; child indent visible.
  - [IPC] `layer_wrap_in_group` (the menu label still says "Convert to Group" but the IPC dispatched is `layer_wrap_in_group`).

### T-layers-009: Convert to Tilemap Layer

Pre: a regular raster layer; the sprite has at least one tileset.
Steps:
  1. Right-click → "Convert to Tilemap Layer".
Expect:
  - [DOM] row icon changes to tilemap; canvas tools switch context.
  - [IPC] `layer_convert_to_tilemap`.

Note: Without an existing tileset the conversion fails silently in the current build (logged to console). T-tilemap-001 covers creating the tileset first.

---

## 8. Palette panel

### T-palette-001: New palette

Pre: project open, palette panel visible.
Steps:
  1. Click `+` in the palette header. Enter a name. Confirm.
Expect:
  - [DOM] dropdown switches to the new palette; grid is empty.
  - [IPC] `palette_add`.

### T-palette-002: Add a colour

Pre: a palette is active.
Steps:
  1. Pick a colour in the colour picker. Click "Add to palette".
Expect:
  - [DOM] new swatch appears in the grid.
  - [IPC] `palette_add_color`.

### T-palette-003: Edit a swatch — click swatch → picker → drag → release. [IPC] `palette_set_color`.

### T-palette-004: Delete a swatch — hover swatch → click delete. [IPC] `palette_remove_color`.

### T-palette-005: Reorder via drag — drag swatch to new position. [IPC] `palette_reorder_colors`.

### T-palette-006: Harmony generator

Pre: foreground colour set.
Steps:
  1. Open Harmony sub-panel.
  2. Click "Triadic" (or any harmony).
Expect:
  - [DOM] 3 new swatches appended (or 4 for tetradic, 2 for complementary).

### T-palette-007: Ramp generator — open Ramp sub-panel, set steps, click generate. New swatches appended.

### T-palette-008: Import .gpl (GIMP)

Pre: a `.gpl` file (e.g. one downloaded from Lospec).
Steps:
  1. Palette I/O Menu → Import → pick `.gpl`.
Expect:
  - [DOM] swatches appended to active palette.

### T-palette-009..011: Import .hex / .pal / .aco — same shape with the matching format. (No `.aco` fixture in repo — bring your own.)

### T-palette-012: Export

Pre: a populated palette.
Steps:
  1. Palette I/O Menu → Export → pick format (gpl/hex/pal) → save dialog.
Expect:
  - File written with correct format. Re-import round-trips colour values.

---

## 9. Timeline panel

### T-timeline-001: Add frame

Pre: project with at least one frame. Active frame index = 0.
Steps:
  1. Click the add-frame button in the timeline header, OR command palette "New Frame".
Expect:
  - [DOM] new frame appears after the active one. Active frame index advances.
  - [IPC] `frame_add { sprite_id, after: 0 }`.

### T-timeline-002: Delete frame

Pre: at least 2 frames.
Steps:
  1. Click delete-frame. Confirm.
Expect:
  - [DOM] active frame removed; active index steps back if it was last.
  - [IPC] `frame_delete`.

### T-timeline-003: Duplicate frame

Pre: at least 1 frame with content.
Steps:
  1. Click duplicate.
Expect:
  - [DOM] new frame inserted with same cels as the source.
  - [IPC] `frame_duplicate`.

### T-timeline-004: Set frame duration

Pre: a frame is active.
Steps:
  1. Edit the duration field for that frame, type `200`. Press Enter.
Expect:
  - [STATE] frame's duration is 200ms.
  - [IPC] `frame_set_duration`.

### T-timeline-005: Create a frame tag

Pre: ≥3 frames.
Steps:
  1. Open the tag bar; create a tag covering frames 1..2 named "walk".
Expect:
  - [DOM] coloured bar above frame columns 1..2 labelled "walk".
  - [IPC] `frame_tag_create`.

### T-timeline-006: Rename / delete a tag — context-menu actions on the tag bar. Same shape as layer rename/delete.

### T-timeline-007: Play

Pre: ≥2 frames.
Steps:
  1. Click play.
Expect:
  - [VISUAL] active frame cycles forward at the configured frame rate.
  - [DOM] play button toggles to "pause".

### T-timeline-008: Pause — clicking pause stops playback at the current frame.

### T-timeline-009: Loop — checkbox toggles whether playback wraps around or stops at the last frame.

### T-timeline-010: Onion skin neighbours and opacity

Pre: ≥3 frames; onion skin enabled (T-canvas-009).
Steps:
  1. Set `onionSkinPrev = 2`, `onionSkinNext = 1`, `onionSkinOpacity = 0.5`.
Expect:
  - [VISUAL] previous 2 frames + next 1 frame ghost over the canvas at half opacity.

---

## 10. Tilemap panel

### T-tilemap-001: Add a tileset

Pre: a sprite is active.
Steps:
  1. Tilemap panel → "Tilesets" tab → "New Tileset" form. Name: "main". Tile size: 16×16. Confirm.
Expect:
  - [DOM] tileset list grows by one. Tab auto-switches to "Tileset" with the new tileset selected.
  - [IPC] `tileset_add`.

### T-tilemap-002: Place a tile

Pre: T-tilemap-001 done; layer is a tilemap layer (T-layers-009 or natively created); tilemap pencil active; a tile is selected from the tileset grid.
Steps:
  1. Click on a tilemap layer cell.
Expect:
  - [VISUAL] the selected tile appears at that cell.
  - [IPC] one `tile_place`.

> **Regression guard:** before PR #102 this returned `Unimplemented`. If clicks do nothing, the backend regressed.

### T-tilemap-003: Erase a tile

Pre: at least one placed tile; tilemap erase tool active.
Steps:
  1. Click on the placed cell.
Expect:
  - [VISUAL] cell goes empty.
  - [IPC] `tile_erase`.

### T-tilemap-004: Autotile mode

Pre: a tileset with autotile rules; a source tile selected.
Steps:
  1. Toggle "Autotile" mode.
  2. Drag-paint a region of cells.
Expect:
  - [VISUAL] cells fill with the rule-resolved tile based on neighbour pattern (corners, edges, fills).
  - [IPC] one `tile_autotile_apply` per click (or batched, depending on input rate).

### T-tilemap-005: Tile property persistence (collision)

Pre: a tileset with at least one tile.
Steps:
  1. Right-click a tile in the tileset grid → toggle "Collision".
  2. Re-render the panel (switch tabs and back, or close + reopen the project).
Expect:
  - [STATE] the tile's `collision` metadata is `true` after re-render.
  - [IPC] one `tileset_set_tile_metadata` on the toggle.

> **Regression guard:** before PR #102 the toggle was UI-only. If the value resets on re-render, persistence regressed.

---

## 11. Command palette

### T-cmd-001: Ctrl+K toggles the palette

Steps:
  1. `Ctrl+K`.
Expect:
  - [DOM] command palette overlay appears with input focused.
  2. `Ctrl+K` again.
Expect:
  - [DOM] palette closes.

### T-cmd-002: Fuzzy match

Pre: palette open.
Steps:
  1. Type `flip h`.
Expect:
  - [DOM] "Flip Horizontal" appears at the top of the result list.

### T-cmd-003: Each non-stub command dispatches its IPC

Spot-check (full sweep is the e2e suite's job). Pick one from each category:

| Command | Expected IPC | Test |
|---|---|---|
| `file:new` | `project_new` + `sprite_add` | T-cmd-003a |
| `edit:undo` | `undo` | T-cmd-003b |
| `sprite:new` | `sprite_add` | T-cmd-003c |
| `frame:new` | `frame_add` | T-cmd-003d |
| `layer:new` | `layer_add` | T-cmd-003e |
| `transform:flip-x` | `canvas_transform` (FlipHorizontal) | T-cmd-003f |
| `view:zoom-fit` | (no IPC; mutates `zoom` signal) | T-cmd-003g |
| `window:toggle-layers` | (no IPC; mutates `isLayerPanelVisible`) | T-cmd-003h |
| `help:about` | `app_about` | T-cmd-003i |

For each: open palette, type a partial query, press Enter on the match, observe the listed IPC fires.

### T-cmd-004: Stubs are NOT in the palette

Pre: palette open.
Steps:
  1. Type `cut`. Then `copy`. Then `paste`.
Expect:
  - [DOM] no `edit:cut` / `edit:copy` / `edit:paste` entries appear.

> **Regression guard:** PR #100 dropped these from the registry rather than ship broken stubs. If they reappear, the regression is "stub silently swallows the click".

---

## 12. Window / panels

### T-window-001..004: Toggle each panel

For each of layers, timeline, palette, tilemap:
Steps:
  1. Command palette → "Toggle <Panel> Panel", OR keybind (e.g. `Ctrl+Shift+L` for layers).
Expect:
  - [DOM] panel disappears / reappears.
  - [STATE] the matching `is*PanelVisible` signal flips.

### T-window-005: Preferences modal

Steps:
  1. Command palette → "Preferences" or `Ctrl+,`.
Expect:
  - [DOM] preferences modal opens. Tabs: General, Keybinds, etc.
  - Closing via Escape or close button restores the editor focus.

---

## 13. Help

### T-help-001: About modal shows version

Steps:
  1. Command palette → "About Pixhaus".
Expect:
  - [DOM] message box with title "Pixhaus" and a body containing the app version (matches `Cargo.toml` `version`).
  - [IPC] one `app_about`.

### T-help-002: Docs link opens browser

Steps:
  1. Command palette → "Documentation".
Expect:
  - A browser tab opens at `https://pixhaus.app/docs`.

Note: in dev builds without the `tauri-plugin-shell`, this falls back to `window.open`. Either path is acceptable for the test.

---

## 14. Keyboard shortcut sweep

Verify each shortcut dispatches the expected action. Switch presets in Preferences > Keybinds and re-run for each preset.

| Action | Aseprite preset | Photoshop preset | Expected |
|---|---|---|---|
| New project | Ctrl+N | Ctrl+N | T-project-001 dispatches |
| Open | Ctrl+O | Ctrl+O | dialog appears |
| Save | Ctrl+S | Ctrl+S | T-project-005 dispatches |
| Save As | Ctrl+Shift+S | Ctrl+Shift+S | dialog appears |
| Close project | Ctrl+W | Ctrl+W | T-project-006 dispatches |
| Undo | Ctrl+Z | Ctrl+Z | T-tools-003 dispatches |
| Redo | Ctrl+Shift+Z | Ctrl+Shift+Z | redo IPC dispatches |
| Select all | Ctrl+A | Ctrl+A | T-select-001 dispatches |
| Deselect | Ctrl+D | Ctrl+D | T-select-002 dispatches |
| Zoom in | Ctrl+= | Ctrl+= | T-canvas-005 |
| Zoom out | Ctrl+- | Ctrl+- | T-canvas-005 |
| Fit | Ctrl+0 | Ctrl+0 | T-canvas-005 |
| 100% | Ctrl+1 | Ctrl+1 | T-canvas-005 |
| Toggle grid | Ctrl+G | Ctrl+' | T-canvas-008 |
| Command palette | Ctrl+K | Ctrl+K | T-cmd-001 |
| Preferences | Ctrl+, | Ctrl+, | T-window-005 |
| Tools | P/E/F/L/R/O | P/E/G/L/U/U | tool selector switches |

For tool keys, Aseprite and Photoshop diverge — verify the active preset's mapping before reporting "wrong tool selected".

---

## 15. Known stubs & out-of-scope

These are deliberate gaps. Do not file bugs against them — file follow-ups instead.

- **Edit > Cut / Copy / Paste**: not in the palette; menu items exist but are dropped from the palette per PR #100 (no clipboard pipeline yet).
- **AI menu**: every entry is a stub. The verb runtime + backend adapters exist but no UI configures the API keys or routes results back to the canvas yet.
- **Line tool real-time preview**: the line currently only paints on release. Real-time preview needs a separate "anchor + cursor" pipeline — out of scope for PR #104.
- **Rect / ellipse drag-time preview**: same as line — paints on release only.
- **Layer-drop undo**: pixel undo works (one entry per stroke / per merge). Resurrecting a dropped-by-merge layer via Ctrl+Z does NOT yet work — requires project-level history support that's not landed.
- **Sample thumbnails**: the welcome screen shows sample names only, no thumbnails. Out of scope for v1.
- **Multi-frame TMX export**: TMX export writes a single frame. Multi-frame is a follow-up.
- **Tablet pressure**: pressure is hard-coded to 1.0 per point.
- **Onion skin on freshly-loaded sprites**: the renderer's tile cache only populates after a frame is drawn or scrubbed onto. A sample opened cold may show no onion overlay until you tab through frames.

---

## Appendix A: Notes for whoever writes the e2e suite

- The existing Playwright harness lives at `tests/visual/`. Use the same setup; add new specs under `tests/visual/specs/`.
- The Tauri command mock layer is at `tests/visual/helpers/tauri-mock.ts` — extend it with response stubs per IPC. Each `[IPC]` assertion in this guide maps to an entry there.
- One Playwright test per test ID: `test('T-tools-001: pencil drag paints in real time', async ({ page }) => { ... })`.
- For visual diffs, the existing baseline directory is `tests/visual/baselines/`. New baselines should be generated on Linux/Chromium to match CI's anti-aliasing.
- Tauri 2 supports `tauri-driver` for in-process WebDriver; if you wire it instead of the mock, the `[IPC]` assertions become real round-trips through the Rust backend, which is more confidence at higher cost.
- Do not consolidate test IDs across reorganisations — keep the IDs stable so commit history references stay valid. Append new IDs; don't renumber.

## Appendix B: Cross-references

Sample fixtures (verified to exist in repo):

- `examples/samples/character-knight.pixhaus`
- `examples/samples/enemy-slime.pixhaus`
- `examples/samples/level-forest.pixhaus`
- `examples/samples/tileset-forest.pixhaus`
- `examples/samples/ui-sprites.pixhaus`

Aseprite fixtures:

- `examples/aseprite-roundtrip/single-frame-rgba.aseprite`
- `examples/aseprite-roundtrip/multi-frame-tags.aseprite`
- `examples/aseprite-roundtrip/indexed-with-palette.aseprite`
- `examples/aseprite-roundtrip/tilemap-with-tileset.aseprite`
- `examples/aseprite-roundtrip/group-multiply.aseprite`

Tutorial starters:

- `examples/tutorials/walk-cycle-start.pixhaus`
- `examples/tutorials/walk-cycle-finished.pixhaus`
- `examples/tutorials/export-unity-start.pixhaus`
- `examples/tutorials/lua-palette-start.pixhaus`
- `examples/tutorials/lua-palette-finished.pixhaus`

No `.psd` or palette (`.aco`/`.gpl`/`.pal`) fixtures are committed. Tests requiring those formats will note "bring your own file".
