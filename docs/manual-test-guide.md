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

Base form: `T-<area>-<NNN>`. The areas are: `launch`, `project`, `export`, `canvas`, `tools`, `select`, `transform`, `layers`, `palette`, `timeline`, `tilemap`, `library`, `refsheet`, `cmd` (command palette), `window`, `help`, `keys`. Numbers are stable — never renumber, only append.

Two extensions are explicitly allowed for compactness:

- **Range notation** (`T-window-001..004`) when a small set of tests differ only in which target the same scenario applies to (e.g. one test per panel). The range is a docs-only shorthand — the e2e harness does not expand ranges programmatically. The author of each spec hand-writes one `it('T-...')` per ID (see `tests/e2e/specs/window.e2e.ts` for the existing four).
- **Letter suffix** (`T-cmd-003a`, `T-cmd-003b`, …) when several near-identical commands share a scenario template and a comparison table. The base number names the scenario; the suffix names the variant. Each ID is still unique.

Both shorthands map 1:1 to e2e test names; the range `T-window-001..004` covers four hand-written tests, not one.

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

### T-layers-010: Multi-select via Ctrl/Shift-click

Pre: three or more layers.
Steps:
  1. Click layer A. Ctrl/Meta-click layer C → both highlight.
  2. Shift-click layer E → A, C, and every row between are highlighted.
Expect:
  - [DOM] every selected row carries the multi-selected state class.
  - [STATE] `selectedLayerIds` contains the expected ids.
  - Right-click any selected row → "Merge Selected" / "Flatten Visible" act on the whole set (T-layers-006/007).

### T-layers-011: Locked layer rejects strokes

Pre: pencil tool active; one layer is selected and locked (click the lock toggle on its row).
Steps:
  1. Try to drag-paint on the canvas.
Expect:
  - [VISUAL] no pixels change on the locked layer.
  - [STATE] no entry pushed to `pixel_history` for this attempt.
  - [IPC] either no `canvas_begin_stroke` fires, or the backend rejects with a "layer locked" validation error (per PR #120). Either path is acceptable; the user-observable invariant is "nothing paints".

---

## 8. Palette panel

### T-palette-001: New palette

Pre: project open, palette panel visible.
Steps:
  1. Click `+` in the palette header. A `ModalInput` (NOT a native `window.prompt`) appears titled "New palette". Type a name → click Create or press Enter.
Expect:
  - [DOM] dropdown switches to the new palette; grid is empty.
  - [DOM] the modal has explicit "Cancel" / "Create" buttons; Escape closes without creating.
  - [IPC] `palette_add`.

> **Regression guard:** PR #132 replaced `window.prompt` calls across palette flows with the in-app `ModalInput` component. If a native browser prompt appears, the regression is live.

> **Submit-label note:** `ModalInput` accepts a `submitLabel` prop and the call sites vary — "New palette" uses "Create", swatch rename uses the default "OK", and timeline tag rename uses "Rename". Test scenarios quote the literal label per call site.

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

### T-palette-013: Append-mode picker keeps empty palettes usable

Pre: a freshly-created palette with zero swatches (T-palette-001 just done; no T-palette-002 yet).
Steps:
  1. Observe the picker area below the grid.
Expect:
  - [DOM] the picker is in append mode: header label "New color", primary button labelled "Add to palette".
  - [STATE] `pickerMode() === "append"`.
  - Pick a colour → click "Add to palette" → new swatch lands in the grid (same outcome as T-palette-002).

> **Regression guard:** before PR #118 an empty palette was a dead end — the picker showed no append affordance. If the "Add to palette" button is missing on a zero-swatch palette, the regression is live.

### T-palette-014: Selecting a swatch updates the brush foreground

Pre: a palette with at least two swatches; pencil tool active.
Steps:
  1. Click swatch A. Drag a stroke on the canvas.
  2. Click swatch B. Drag another stroke.
Expect:
  - [VISUAL] first stroke uses swatch A's colour; second stroke uses swatch B's colour.
  - [STATE] `foregroundIndex` signal exported from `palette-panel-state.ts` updates on each click.

> **Regression guard:** PR #119 bridged the palette index into the brush colour. If both strokes paint the same colour regardless of swatch click, the bridge is broken.

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

### T-timeline-006: Rename / delete a tag

Pre: at least one frame tag exists (T-timeline-005).
Steps:
  1. Right-click the tag → context menu appears with "Rename tag" (testid `tl-ctx-rename`) and "Delete tag".
  2. Click "Rename tag" → a `ModalInput` titled "Rename tag" opens. Type a new name → press Enter.
Expect:
  - [DOM] tag label updates in the tag bar.
  - [IPC] `frame_tag_rename`.
  - For delete: same right-click flow → "Delete tag" → [IPC] `frame_tag_delete`.

> **Note:** rename is via the right-click context menu only — there is no double-click-to-rename. PR #121 introduced the focus-safe shortcut behavior covered in T-timeline-011.

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

### T-timeline-011: Focus-safe shortcuts during inline rename

Pre: T-timeline-006 — a "Rename tag" `ModalInput` is open with the text field focused.
Steps:
  1. With the rename field focused, press `Ctrl+G` (toggle tile grid), then `Z`, then `Ctrl+Z`.
Expect:
  - [STATE] `showTileGrid` does NOT flip.
  - [DOM] the rename input receives the keystrokes normally (no tool switching, no undo dispatched).
  - [STATE] keybind manager's `isEditableTarget()` returns early for `<input>`, `<textarea>`, `<select>`, or `contenteditable` targets.
  - Press Escape → modal closes; shortcuts work again.

> **Regression guard:** PR #121 added focus-safe routing. If shortcuts fire while typing into a text input — toggling grid, dispatching undo, switching tools — the regression is live.

---

## 10. Tilemap panel

> **Active context follows the active layer.** PR #122 wired `activeTilemapCtx` to the active layer signal: clicking a different tilemap layer in the layer panel re-points the tilemap panel (selected tileset, brush mode, autotile state) at that layer's stored context. If switching layers leaves stale tilemap-tool state from the previous layer, the regression is live.

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

## 11. Project library panel

Introduced in bedrock arc B9 (PRs #135, #159, #166, #169, #161, #176). The library panel lists the project's reusable entities — characters, props, tilesets, tilemaps, reference images, and user-defined custom kinds — and is the surface for AI library hooks (auto-tag, anchor wiring, cross-entity transfer, per-entity LoRA training).

**Locations & selectors:** panel root `data-testid="library-panel"`; header buttons `library-add-entity` (`+`, title "New entity") and `library-add-group` (folder icon, title "New group"); search input `library-search`; tree container `library-tree`. Rows expose `entity-row-{entityId}`, `group-row-{groupId}`, and `state-row-{stateId}`.

**Visibility signal:** `isLibraryPanelVisible` (default `true`). Note: there is **no command-palette toggle** for this panel — see T-library-001 below and section 17 stubs. The IPC surface is `library_*` (32+ commands in `app/src/lib.rs`).

### T-library-001: Library panel is visible by default

Pre: project open.
Steps:
  1. Observe the editor shell.
Expect:
  - [DOM] an element with `data-testid="library-panel"` is mounted; header reads "Library".
  - [STATE] `isLibraryPanelVisible() === true`.
  - [DOM] command palette → search `toggle library` returns nothing (the toggle command does not exist yet; tracked as a follow-up in section 17).

### T-library-002: Create a Custom entity

Pre: library panel visible.
Steps:
  1. Click `library-add-entity` (the `+` button).
  2. In the "New entity" modal, the Custom tab is selected by default. Type Category "Character", Name "knight", Canvas 32×32, Initial states "idle,run". Click "Create".
Expect:
  - [DOM] modal closes; a new row appears in `library-tree` with name "knight".
  - [DOM] expanding the row shows two state rows: `state-row-{id}` for "idle" and "run".
  - [IPC] `library_create_entity` then one `library_add_state` per initial state.
  - [STATE] the new entity is the active target (`library_get_active_target` returns it).

Variants: click the Tileset / Tilemap / Reference tabs and submit the corresponding fields. Each kind triggers `library_create_entity` with the matching `EntityKind` (Tileset, Tilemap, Reference). Reference kind requires picking a source image via "Choose file…".

### T-library-003: Entity context menu actions

Pre: at least one entity exists (T-library-002).
Steps:
  1. Right-click the entity row → context menu appears with: Rename, Add state (Custom only), Move to group…, Delete.
  2. Click each action in turn on separate entities.
Expect:
  - Rename → an `InlineRenameInput` replaces the row label in place (the same flow fires on double-click of the row). Type, press Enter to commit → [IPC] `library_rename_entity`. Escape cancels. NOT a `ModalInput`.
  - Add state → opens a modal for the new state name; submitting fires `library_add_state`.
  - Move to group → opens a **modal dialog** with a `<select>` dropdown listing existing groups (not a context-menu submenu). Pick a group → click confirm → [IPC] `library_move_entity_to_group`.
  - Delete → confirmation dialog; confirming fires `library_delete_entity`.

### T-library-004: Groups — create, rename, populate, expand

Pre: at least one entity exists.
Steps:
  1. Click `library-add-group`. A new group row appears **immediately** with the default name `Group {n+1}` (no naming modal). [IPC] `library_create_group { name: "Group N" }`.
  2. Rename: double-click the group's label → `InlineRenameInput` opens in place. Type "characters" → Enter. [IPC] `library_rename_group`.
  3. Drag an entity row onto the group row.
Expect:
  - [DOM] a `group-row-{id}` with chevron toggle appears at step 1.
  - [DOM] inline rename commits the new label at step 2.
  - [DOM] dragging the entity onto the group nests it (indent visible when expanded).
  - [IPC] **entity onto group** → `library_move_entity_to_group { entity_id, group_id }`. **Group onto group** (nesting groups) → `library_set_group_parent { child_group_id, parent_group_id }`. The two IPCs are distinct; the drop target's kind determines which fires.

### T-library-005: Search filters the tree

Pre: at least two entities with different names / categories / tags.
Steps:
  1. Type a partial name into `library-search`.
Expect:
  - [DOM] tree filters to matching rows; non-matches hide.
  - [IPC] `library_search` fires (debounced; latency unspecified — do not assert a timeout).
  - Clearing the search via the `×` button (visible only when the input has text) restores the full tree.

### T-library-006: AI auto-tag suggestions

Pre: a `Custom` or `Reference` entity exists (T-library-002). A backend that supports `pixhaus.builtin.critique` is configured (Preferences → AI). Without a configured backend the verb invocation aborts with a toast and no chips appear.
Steps:
  1. Right-click a `Custom` or `Reference` entity row → context menu shows a "Suggest tags" item between "Add state" and "Move to group…". The item is hidden for `Tileset` and `Tilemap` entities (the verb has no useful grounding for them today).
  2. Click "Suggest tags". An "Auto-tagging…" info toast surfaces while the verb runs.
  3. When the verb resolves, a success toast reports the suggestion count (or "No new tag suggestions." when the VLM returns nothing) and pending chips appear inline on the entity row, styled with a dashed accent border and ✓ / ✗ buttons.
  4. Click ✓ on one pending chip → the chip migrates from the pending strip into the confirmed strip (plain background, no buttons).
  5. Click ✗ on another pending chip → the chip vanishes.
  6. Add a state to the same entity (T-library-003 → "Add state") OR approve a sheet variant on a `Reference` entity (T-refsheet-004) to verify the corpus refresh path.
Expect:
  - [DOM] context menu carries `data-testid="ctx-menu-suggest-tags"` only when the right-clicked entity is `Custom` or `Reference`.
  - [DOM] pending chips render with `library-row__tag-chip--pending`; confirmed chips render with `library-row__tag-chip` only.
  - [IPC] step 2 fires `library_auto_tag_entity { entity_id }`. The returned tag IDs land in the panel's pending-suggestions cache; the call is followed by `library_list_entities` + `library_list_tags` + `library_list_groups` so the chip strip can resolve names for any newly-created tags.
  - [IPC] step 4 fires `library_accept_suggested_tag { entity_id, tag_id }` followed by a `library_list_entities` / `library_list_tags` refresh.
  - [IPC] step 5 fires `library_reject_suggested_tag { entity_id, tag_id }` followed by the same refresh.
  - [IPC] step 6 fires `library_add_state` (or `library_approve_sheet_variant`) and then a fire-and-forget `library_update_corpus { entity_ids: [<entity>] }`. The corpus refresh failing surfaces a toast but does not abort the originating mutation.

> **Out of scope for this scenario:** manual tag CRUD (add tag, delete tag, untag entity from the row) is filed against a future Category B PR. T-library-006 covers only the auto-tag accept/reject and corpus-refresh surface.

### T-library-007: Anchor wiring — DEFERRED

`library_set_entity_anchor` and `library_get_anchor_payload` exist as
IPCs but no UI affordance sets or surfaces an anchor today. The AI verb
runtime resolves anchors server-side via stored entity metadata, not via
a user-driven UI flow. Tracked as a stub in section 17. ID reserved — do
not reassign; rewrite this entry when a "Set anchor reference" control
ships.

### T-library-008: Aseprite round-trip preserves library metadata (B9.5)

Pre: a project with at least one Custom entity (`knight`) and one Tileset entity. Save the project first.
Steps:
  1. File → Export → Aseprite (or the workflow `pnpm dev` exposes — `project_export_aseprite`). Pick a path.
  2. File → Close.
  3. File → Open → re-import the just-exported `.aseprite`.
Expect:
  - [DOM] library panel rebuilds with `knight` and the Tileset entity intact.
  - [STATE] entity kind, name, states, tags, and (PR #176 follow-up) any tilemap cels survive the round-trip.
  - [IPC] `project_export_aseprite` then `project_import_aseprite`.

> **Regression guard:** before PR #161 the Aseprite import dropped library entities silently. Before PR #176 tilemap cels were lost on export. If a re-import shows an empty library or missing tilemap data, one of these regressed.

---

## 12. Reference sheets

Introduced in bedrock arc B10 (PRs #160, #165, #167, #168, #179). The reference sheet view panel displays a canonical sheet image for an entity, lets the user generate new variants via composition templates, refine specific panels via panel-scoped inpainting, approve a variant as canonical, and train a per-entity LoRA from the approved sheets.

**Locations & selectors:** panel component `ui/src/sheet/SheetView.tsx`. Visibility signal `isSheetPanelVisible` (default `false`). Toggle command id `window:toggle-sheet`, palette label "Toggle Reference Sheet Panel", palette keywords: `sheet`, `reference`, `anchor`, `character`. Verb input modal is `ModalForm` hosted by `VerbInvokeHost` (`ui/src/lib/ai/VerbInvokeHost.tsx`).

### T-refsheet-001: Open the reference sheet panel

Pre: at least one entity has an approved reference sheet variant, OR right-click an entity in the library to open the panel on a fresh entity.
Steps:
  1. Command palette → "Toggle Reference Sheet Panel", OR right-click an entity in the library → open sheet panel.
Expect:
  - [STATE] `isSheetPanelVisible() === true`.
  - [DOM] the panel mounts to the right of the canvas. Title shows the entity name, or "Reference sheet" when no entity is active.
  - [DOM] for an entity with an approved variant: the canonical sheet image renders fit-to-window with an SVG panel overlay.
  - [DOM] history strip and prompt history strip are visible below.

### T-refsheet-002: Generate a reference sheet variant

Pre: T-refsheet-001 done; a backend that supports `pixhaus.builtin.generate_reference_sheet` is configured (Preferences → AI). Without a configured backend the verb invocation aborts with a toast — the modal flow (open, fill, submit, cancel) is still exercisable, only the network call fails. No env-driven mock toggle exists today (tracked in section 17).
Steps:
  1. Click "Generate variant".
  2. The verb modal opens. Pick a composition template: Character / Item / Tileset / Custom. Type a prompt. Optional: negative prompt, num_variants (1–4), seed. Click Submit.
Expect:
  - [DOM] modal closes; a progress indicator surfaces until the verb resolves.
  - [DOM] new variant thumbnail lands in the history strip.
  - [IPC] one verb invocation of `pixhaus.builtin.generate_reference_sheet`; the request carries `entity_id`, `template`, `prompt`, and optional fields.
  - [STATE] entity's variant list grows by one.

### T-refsheet-003: Refine selection via panel-scoped inpainting

Pre: an entity with at least one variant rendered in the sheet panel (T-refsheet-002).
Steps:
  1. Click on a labelled panel region in the SVG overlay (e.g. "front", "side", "back"). The panel highlights and the "Refine selection" button becomes enabled.
  2. Click "Refine selection" → the verb modal for `pixhaus.builtin.iterate_reference_sheet` opens with `panel_label` pre-filled.
  3. Type a refinement prompt. Submit.
Expect:
  - [STATE] `selectedPanelRegion` carries the clicked panel's rect.
  - [IPC] the iterate verb runs with `source_variant_id`, `sheet_image_b64`, `panel_label`, and the prompt.
  - [DOM] when the verb resolves, the new variant lands in the history strip; the scoped region is the only area that changed visually.

### T-refsheet-004: Approve a variant as canonical

Pre: ≥2 variants in the history strip (one canonical, one non-canonical).
Steps:
  1. Right-click a non-canonical variant thumbnail.
  2. Click "Approve as canonical" in the context menu.
Expect:
  - [DOM] the clicked variant gains the "approved" badge; the previously canonical variant loses it.
  - [DOM] the canonical sheet image in the main panel switches to the newly-approved variant.
  - [IPC] `library_approve_sheet_variant` with the entity id and variant id.
  - Hover: tooltip "Canonical — currently approved" on the new canonical thumbnail.

### T-refsheet-005: Train per-entity LoRA from approved sheets

Pre: entity has at least one approved variant; an AI backend that supports LoRA training is configured (e.g. Replicate). Without a configured backend, this test verifies only the button-state transitions and the outbound IPC — the actual training never completes.
Steps:
  1. Click "Train LoRA".
Expect:
  - [DOM] button label transitions: "Train LoRA" → "Training…" (disabled while in flight).
  - [IPC] `library_train_entity_lora { entity_id }` fires.
  - On completion against a real backend (Replicate: ~15–30 minutes): a toast surfaces "Trained consistency LoRA…"; button label becomes "Retrain LoRA"; a "LoRA trained" pill appears below the button.
  - [STATE] `Entity.ai.lora_path` is now non-empty; future verb calls on this entity inherit the LoRA via `library_get_anchor_payload`.

> The 15–30 minute round-trip makes this test impractical for routine manual sweeps. Tracked in section 17 alongside the missing env-driven mock toggle.

### T-refsheet-006: Cancel an in-flight verb invocation

Pre: T-refsheet-002 or T-refsheet-003 — the verb modal is open and either has just been submitted (running) or has not yet been submitted (idle).
Steps:
  1. Idle: click "Cancel" → modal closes, no IPC fires.
  2. Running: click "Cancel running invocation" → modal stays mounted while cancellation propagates; on settle, the modal returns to idle / closes.
Expect:
  - [IPC] running case: `verb_cancel { invocation_id }` (PR #133); the runtime cancels the task.
  - [DOM] no partial variant is appended to the history strip on a cancelled run.
  - Escape on the idle modal closes it (same as Cancel).

> **Regression guard:** before PR #133 there was no in-app way to abort a long-running verb. If "Cancel running invocation" is missing or doesn't actually terminate the in-flight task, the regression is live.

---

## 13. Command palette

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
| `layer:flatten` | `layer_flatten_visible` | T-cmd-003f |
| `transform:flip-x` | `canvas_transform` (FlipHorizontal) | T-cmd-003g |
| `view:zoom-fit` | (no IPC; mutates `zoom` signal) | T-cmd-003h |
| `window:toggle-layers` | (no IPC; mutates `isLayerPanelVisible`) | T-cmd-003i |
| `window:toggle-sheet` | (no IPC; mutates `isSheetPanelVisible`) | T-cmd-003j |
| `ai:cleanup` | verb invocation `pixhaus.builtin.cleanup` (opens schema-driven input modal) | T-cmd-003k |
| `help:about` | `app_about` | T-cmd-003l |

For each: open palette, type a partial query, press Enter on the match, observe the listed IPC fires.

### T-cmd-004: Cut/Copy/Paste are NOT in the palette

Pre: palette open.
Steps:
  1. Type `cut`. Then `copy`. Then `paste`.
Expect:
  - [DOM] no `edit:cut` / `edit:copy` / `edit:paste` entries appear.

> **Regression guard:** PR #100 dropped these from the registry rather than ship broken stubs. If they reappear, the regression is "stub silently swallows the click". The original "no AI commands either" claim is now obsolete: AI verb commands ARE wired into the palette as of PR #129, and the verb input modal landed in PR #133 — see T-cmd-005.

### T-cmd-005: AI verb commands open the input modal

Pre: command palette open; at least one AI verb is registered (all built-ins are by default, per PR #126).
Steps:
  1. Type `cleanup` → first match is the `ai:cleanup` command. Press Enter.
Expect:
  - [DOM] palette closes; the verb input modal (`ModalForm` from `VerbInvokeHost`) opens.
  - [DOM] the modal renders schema-driven fields (per PR #133) — for cleanup: palette-snap toggle, AA-removal toggle, pivot-drift threshold, etc., per the verb's input schema.
  - [DOM] explicit "Cancel" and Submit buttons at the bottom.
  - [STATE] `activeVerb` signal is set.

> **Registered AI palette commands (verify against `ui/src/command-palette/command-registry.ts`):** `ai:inbetween`, `ai:continue`, `ai:variant`, `ai:cleanup`, `ai:critique`, `ai:settings`. The generate-reference-sheet verb has NO palette command today — it is reached via the "Generate variant" button in the reference sheet panel (T-refsheet-002). Tracked as a stub in section 17.

### T-cmd-006: Verb cancellation closes the modal cleanly

Pre: T-cmd-005 — the verb input modal is open.
Steps:
  1. Idle path: click "Cancel" without submitting.
  2. Running path: submit, then while in flight click "Cancel running invocation".
Expect:
  - Idle: modal closes; no IPC fires; `activeVerb` is `null`.
  - Running: [IPC] `verb_cancel { invocation_id }`; modal returns to idle once cancellation propagates; no partial output appears.
  - Escape on the idle modal closes it (equivalent to Cancel).

---

## 14. Window / panels

### T-window-001..004: Toggle each of the four originally-toggleable panels

For each of layers (`window:toggle-layers`), timeline (`window:toggle-timeline`), palette (`window:toggle-palette`), tilemap (`window:toggle-tilemap`):
Steps:
  1. Command palette → "Toggle <Panel> Panel", OR keybind where mapped (e.g. `Ctrl+Shift+L` for layers).
Expect:
  - [DOM] panel disappears / reappears.
  - [STATE] the matching `is*PanelVisible` signal flips (`isLayerPanelVisible`, `isTimelinePanelVisible`, `isPalettePanelVisible`, `isTilemapPanelVisible`).

The live e2e harness binds these IDs in `tests/e2e/specs/window.e2e.ts:99-115`. Per the doc's never-renumber rule, IDs `001..005` are stable. New panel toggles append starting at `006`.

### T-window-005: Preferences modal

Steps:
  1. Command palette → "Preferences" or `Ctrl+,`.
Expect:
  - [DOM] preferences modal opens. Tabs: General, Keybinds, etc.
  - Closing via Escape or close button restores the editor focus.

(Matches `tests/e2e/specs/window.e2e.ts:115`.)

### T-window-006: Toggle the reference sheet panel

Pre: project open.
Steps:
  1. Command palette → "Toggle Reference Sheet Panel" (id `window:toggle-sheet`).
Expect:
  - [DOM] sheet panel mounts / unmounts.
  - [STATE] `isSheetPanelVisible` flips.

### T-window-007: Library panel has no palette toggle (tracked gap)

Pre: project open.
Steps:
  1. Open the command palette. Type `toggle library`.
  2. Open the native Window menu.
  3. From the library panel's header, click the close button.
Expect:
  - [DOM] step 1: no palette entry matches. Confirmed by `ui/src/command-palette/command-registry.ts` — no `window:toggle-library` id is registered.
  - [DOM] step 2: the Window menu (`app/src/menu.rs:293-303`) lists toggles for layers, timeline, and palette only. No library entry.
  - [STATE] step 3: `setLibraryPanelVisible(false)` runs (`LibraryPanel.tsx:173-189`). The panel disappears.
  - **Once hidden, there is no in-app way to re-show the library panel.** Reopening requires either a code change or `setLibraryPanelVisible(true)` from devtools. Tracked as a stub in section 17.

---

## 15. Help

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

## 16. Keyboard shortcut sweep

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
| Toggle reference sheet panel | (none by default) | (none by default) | `window:toggle-sheet`; verify via palette |
| Tools | B/P, E, G, L, U, O | B/P, E, G, L, U, O | tool selector switches |

Tool key mapping (both presets share these as of `ui/src/keybinds/defaults.ts:41-42, 79-80`):

| Tool | Key(s) |
|---|---|
| Pencil | B, P |
| Eraser | E |
| Fill (bucket) | G |
| Line | L |
| Rectangle | U |
| Ellipse | O |

> `B` is the Photoshop "brush" muscle-memory alias for pencil; `P` is the Aseprite default. Both bind to `tool:pencil` in both presets — no separate brush tool exists yet. The Aseprite and Photoshop presets currently agree on every tool keybind. The doc previously claimed `F` for fill (Aseprite) and `U`/`U` for rect/ellipse (Photoshop) — those mappings were never correct and have been removed. If the presets diverge in the future, split this row back out.

---

## 17. Known stubs & out-of-scope

These are deliberate gaps. Do not file bugs against them — file follow-ups instead.

- **Edit > Cut / Copy / Paste**: not in the palette; menu items exist but are dropped from the palette per PR #100 (no clipboard pipeline yet).
- **AI backend configuration**: the verb runtime, the input modal, and verb cancellation all work. What's still gated is per-backend setup — API keys are entered via Preferences → AI (Anthropic, OpenAI, Replicate, Ollama, ComfyUI, Stability). Verbs targeting an unconfigured backend surface a toast and abort.
- **Env-driven verb mock toggle**: there is NO `PIXHAUS_AI_MOCK` or equivalent environment variable wired into `ai/src/runtime/` today. The only mock infrastructure is `window.__PIXHAUS_MOCK__` in `tests/visual/helpers/tauri-mock.ts`, which is scoped to the visual-test harness — not usable for manual `pnpm dev` sessions. Follow-up: wire an env-driven short-circuit that returns deterministic mock output for every built-in verb so manual sweeps of T-refsheet-* and T-cmd-005 don't require a real backend.
- **`window:toggle-library` palette command**: every other panel (layers, timeline, palette, tilemap, sheet) registers a `window:toggle-*` id in `ui/src/command-palette/command-registry.ts`. The library does not, and the native Window menu (`app/src/menu.rs:293-303`) only toggles layers/timeline/palette. Once the panel's close button fires `setLibraryPanelVisible(false)` there is no in-app way to re-show it. Follow-up: register `window:toggle-library` AND add a Window-menu entry.
- **Manual tag CRUD UI**: the IPCs `library_add_tag` and `library_delete_tag` (and `library_untag_entity` from the row chip) are registered but unused — there is no "Add tag" input or chip-untag affordance on the library row. The auto-tag accept/reject surface is wired (T-library-006); manual tag management is filed for a follow-up Category B PR.
- **Library anchor wiring UI**: `library_set_entity_anchor` and `library_get_anchor_payload` are registered but unused in `ui/src/`. There is no "Set anchor reference" context-menu item; the AI verb runtime resolves anchors server-side via stored entity metadata. T-library-007 is reserved.
- **`ai:generate-reference-sheet` palette command**: the verb itself works, but it has no command-palette entry. It is reachable only via the "Generate variant" button in the reference sheet panel (T-refsheet-002). Follow-up: register the palette command so verb sweeps can use the same `T-cmd-005`-style flow as `ai:cleanup`.
- **Per-entity LoRA training latency**: a real training run (Replicate) takes 15–30 minutes per entity. T-refsheet-005 is impractical for routine manual sweeps without the env-driven mock toggle above.
- **Line tool real-time preview**: the line currently only paints on release. Real-time preview needs a separate "anchor + cursor" pipeline — out of scope for PR #104.
- **Rect / ellipse drag-time preview**: same as line — paints on release only.
- **Layer-drop undo**: pixel undo works (one entry per stroke / per merge). Resurrecting a dropped-by-merge layer via Ctrl+Z does NOT yet work — requires project-level history support that's not landed.
- **Sample thumbnails**: the welcome screen shows sample names only, no thumbnails. Out of scope for v1.
- **Multi-frame TMX export**: TMX export writes a single frame. Multi-frame is a follow-up.
- **Tablet pressure**: pressure is hard-coded to 1.0 per point.
- **Onion skin on freshly-loaded sprites**: the renderer's tile cache only populates after a frame is drawn or scrubbed onto. A sample opened cold may show no onion overlay until you tab through frames.
- **No e2e coverage yet for library / reference sheet flows**: `tests/e2e/specs/` has no `library.e2e.ts` or `refsheet.e2e.ts` files at time of writing. The new `T-library-*` and `T-refsheet-*` IDs in sections 11–12 are documented but not yet automated — see Appendix A.

---

## Appendix A: Notes for whoever writes the e2e suite

The e2e harness landed in PR #123 and lives at `tests/e2e/`. It uses **WebdriverIO + tauri-driver** against the real Rust backend — `[IPC]` assertions are real round-trips, not mocks. A separate, smaller pixel-diff harness still exists at `tests/visual/` (Playwright + image-compare) for visual baselines; treat them as complementary.

**Layout** (verified against the current tree):

```
tests/e2e/
  wdio.conf.ts            # WebdriverIO config; spawns / kills tauri-driver on :4444
  specs/                  # one file per area
    canvas.e2e.ts
    cmd.e2e.ts
    export.e2e.ts
    help.e2e.ts
    keys.e2e.ts
    launch.e2e.ts
    layers.e2e.ts
    palette.e2e.ts
    project.e2e.ts
    select.e2e.ts
    smoke.e2e.ts
    tilemap.e2e.ts
    timeline.e2e.ts
    tools.e2e.ts
    transform.e2e.ts
    window.e2e.ts
  helpers/
    app.ts                # session lifecycle, project bootstrap
    canvas.ts             # canvas-pixel coordinate utilities
    dialog.ts             # native dialog interception
    ipc.ts                # capture and assert against `[IPC]` round-trips
    selectors.ts          # central testid registry
    state.ts              # read Solid signals through the tauri-driver bridge
```

**Conventions for new specs:**

- One test per ID: `test('T-tools-001: pencil drag paints in real time', async () => { ... })`. The framework is Mocha (`wdio.conf.ts:84`), not Jest.
- Add new testids to `tests/e2e/helpers/selectors.ts` — don't sprinkle bare strings across specs.
- For the new B9 / B10 areas, add `tests/e2e/specs/library.e2e.ts` and `tests/e2e/specs/refsheet.e2e.ts`. The testids these scenarios reference (`library-panel`, `library-add-entity`, `library-add-group`, `library-search`, `library-tree`, `entity-row-{id}`, `group-row-{id}`, `state-row-{id}`) already exist in the UI; register them in `selectors.ts` first.
- The IDs in this guide are stable — never renumber, only append. Commit history references the original numbers; renumbering breaks every back-reference at once.

**Visual diffs** live in the separate Playwright harness at `tests/visual/`. Baselines in `tests/visual/baselines/` are generated on Linux/Chromium to match CI's anti-aliasing. Add new baselines from the same target; macOS / Windows captures drift just enough to flake.

**Local run:**

```bash
pnpm e2e                  # full sweep
pnpm e2e -- --spec tests/e2e/specs/tools.e2e.ts   # one file
```

**Platform support** (per `tests/e2e/wdio.conf.ts:12` and `tests/e2e/README.md:22`):

- **Linux**: install `webkit2gtk-driver` from your package manager (`apt install webkit2gtk-driver` on Debian/Ubuntu), plus `tauri-driver` via `cargo install tauri-driver`.
- **Windows**: install `msedgedriver` matching your Edge version, plus `tauri-driver`.
- **macOS**: **not supported** by tauri-driver. Tauri's docs are explicit that macOS lacks a WebKit WebDriver tool, so the e2e suite can only run on Linux or Windows. The `scripts/setup-e2e.{sh,ps1}` helpers reflect this.

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
