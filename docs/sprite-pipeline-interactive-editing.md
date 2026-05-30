# Sprite-pipeline interactive editing

Status: approved, not yet built. This is the design of record for the next round
of work on the AI animation pipeline in the native (`v2`) shell. It is a
companion to `native-ui-sprite-generation-upgrades.md`: that doc covers the
generation surface (cockpit, prompts, structures, styles); this one covers what
happens *after* a static grid sheet comes back — viewing it, cutting it, fixing
it — plus the one-click preset experience that gets a user to a good sheet on the
first click.

## Why this exists

Testing the static grid-sheet path surfaced three walls. Each traces to a
specific gap in the code, not a tuning problem:

1. **You can't see the generated sheet.** `run_static_sheet` (`shell/src/ai.rs`)
   generates one solid-magenta `cell_w*cols × cell_h*rows` PNG, decodes it,
   slices it with `sheet_to_frames`, and then drops the decoded sheet on the
   floor. Only `Vec<VideoFrame>` leaves the function over
   `ShellMsg::StaticSheetReady`. The Clip stage plays the sliced frames; the
   sheet that produced them is never shown. When the cut is wrong, there is
   nothing to look at to understand why.

2. **The auto-detected frames are often off.** Slicing is
   `core::transforms::sheet::slice_grid(sheet, rows, cols)` — uniform,
   floor-divided, row-major, seeded only by the rows/cols set in the Motion
   stage. It makes no aesthetic decision and has no offset, gutter, inset, or
   per-cell control. If the model paints the grid slightly off-center, with
   padding, or with any drift, the rigid cut splits subjects across cell
   boundaries. `slice_rects(sheet, &[(x,y,w,h)])` already exists in the same
   module for non-uniform cuts, but nothing in the UI drives it.

3. **The normalization step is a dead end.** `studio_normalize_inspector`
   (`shell/src/studio.rs`) renders the `NormalizeReport` read-only — baseline
   drift, scale match/parity, edge clear, components, loop seam, warnings — and
   offers a single "Next: land" button. The pass that produces it
   (`normalize_frames`, `core/src/transforms/normalize.rs`) takes a rich
   `NormalizeOptions` (chroma key + tolerance, alpha threshold, canvas, bottom
   margin, reference height, `ComponentMode`), but none of that is exposed. When
   the report flags a problem, the only recovery is to regenerate the whole
   sheet.

A fourth, softer wall: presets exist but aren't one-click. The Bit prompt pack
(eight actions in `ai/src/compose/builtins.rs`) is reachable only through a
Template dropdown in `cockpit.rs`, and the demo opens with no template selected.
A user has to type or pick, then choose a structure, then generate. The goal —
"open the demo, click once, get a great Bit sheet" — is a few clicks short.

## How the static-sheet path works today

The trace, so every change below lands in a known place:

```
Motion stage (cockpit / studio)
  → StaticSheetJob { canvas, anchor_png, action_prompt, rows, cols, fps, seed }
  → spawn_static_sheet            (shell/src/ai.rs)
  → run_static_sheet              generate magenta sheet PNG, decode
  → sheet_to_frames(sheet, rows, cols, fps)
       → slice_grid(sheet, rows, cols)   (core/src/transforms/sheet.rs)  [sheet dropped here]
  → ShellMsg::StaticSheetReady { frames, action, fps, seed }
  → StudioStage::Clip → Pick → Normalize → Land   (shell/src/studio.rs)
       → Normalize: compute_normalize → normalize_frames → NormalizeReport (read-only)
```

The frames join the same Clip→Pick→Normalize→Land tail as imported video, which
is why the static path inherited a review UI built for footage, not for a sheet
the user can re-cut.

## Feature 1 — retain and view the raw sheet

Stop discarding the sheet. Carry it, and the geometry that produced it, forward.

- In `run_static_sheet`, return the decoded sheet `PixelBuffer` (or its PNG
  bytes) and the generation geometry — `rows`, `cols`, the per-cell
  `(cell_w, cell_h)`, and `seed` — alongside the frames.
- Extend `ShellMsg::StaticSheetReady` to carry the sheet and geometry.
- Add `StudioStage::Sheet`, inserted between `Motion` and `Clip` in
  `StudioStage::ALL` and the rail. It shows the raw generated sheet at fit-to-view
  with a 1:1 toggle, before any slicing. This is the surface Feature 2 draws its
  gizmo on.

Holding the sheet in studio state (not the project) is fine for the working
session; what persists on the landed animation is the slice spec (Feature 2), not
the magenta sheet.

## Feature 2 — interactive slice gizmo (phased)

A grid overlay on the raw sheet, seeded from the generation geometry, that
re-cuts live as the user adjusts it. The gizmo reuses `BoxGizmo` / `GizmoHandle`
from `shell/src/gizmo.rs`.

### Slice spec model

```
struct SliceGrid {
    rows: u32,
    cols: u32,
    offset_x: u32,   // left margin before the first cell
    offset_y: u32,   // top margin before the first row
    gutter_x: u32,   // gap between columns
    gutter_y: u32,   // gap between rows
    inset: u32,      // shrink each cell inward (trim shared edges)
}
```

Plus a Phase B variant that carries explicit per-divider positions when the user
breaks from the uniform grid.

### Phase A — uniform grid + offset + inset/gutter

- Seed `SliceGrid` from the generation `rows`/`cols` and `(cell_w, cell_h)`.
- Draw the grid on the Sheet stage: cell rectangles, plus (borrowed from
  agent-sprite-forge) optional safe-area margins and center crosshairs to make
  drift legible.
- Dragging a cell edge moves every divider on that axis at once — it changes the
  uniform parameters, not one line. Spinners for each field mirror the drag.
- Re-slice live through a new helper, `slice_grid_spec(sheet, &SliceGrid)`, in
  `core/src/transforms/sheet.rs`. It computes the per-cell rects from the spec
  and routes through the existing `crop` (same primitive `slice_grid` and
  `slice_rects` already use — no new pixel copy). When the spec is a plain
  uniform grid with no offset/gutter/inset, it must produce the exact same cells
  `slice_grid` does today.

### Phase B — per-divider non-uniform overrides

- Let the user drag a single divider to a custom position, producing variable
  cell widths/heights and per-axis gutters.
- This resolves to an explicit `Vec<(x,y,w,h)>` and cuts through the existing
  `slice_rects`.
- Keep it a clean superset: a spec with no overrides is identical to Phase A.

### Persist the spec

Today `grid_rows`/`grid_cols` live only in session state, so re-opening a project
loses the cut intent. Persist the resolved `SliceGrid` on the landed `Animation`
(`core/src/project/animation.rs`), next to `AnimationQc` (`core/src/project/qc.rs`),
so the cut is reproducible and re-editable after save/load.

## Feature 3 — editable normalization

Turn the read-only report into a control surface. Keep the QC numbers as
guidance, beside the knobs that fix them, instead of as a verdict.

- Expose `NormalizeOptions` in `studio_normalize_inspector` as live widgets:
  chroma key color + tolerance, alpha threshold, canvas size, bottom margin,
  reference height, and `ComponentMode` (whole-alpha / largest / all-with-min-area).
- Changing any knob invalidates `normalize_cache_key` and re-runs
  `compute_normalize` / `refresh_normalize_cache`, so the normalized strip and the
  report update in place. The pass is CPU-only over a handful of small frames; if
  it ever hitches, move `normalize_frames` to `spawn_blocking` as the existing
  code comment already notes.
- Per-frame row on the normalized strip: an include/exclude toggle (drop a junk
  frame without re-picking) and an "edit pixels" button that opens that frame in
  the Draw workspace for manual surgery and returns the edited buffer into the
  pick set. This is the "full editor control" escape hatch for anything the knobs
  can't reach.
- Surface the *why* visually, borrowing from agent-sprite-forge: tint
  edge-touch frames, and overlay connected-component regions so a "3 parts"
  warning shows which speck or fragment triggered it.

## Feature 4 — one-click presets and the Bit demo

Get a user to a great sheet on the first click.

- Add a preset-card gallery to the cockpit, driven by the existing builtin prompt
  pack (`ai/src/compose/builtins.rs`). Each card (idle, walk, run, jump, fall,
  attack, hurt, turnaround) sets subject + template + structure + style and fires
  `cockpit_generate` in one click — no dropdown hunting.
- Pre-select a sensible default prompt when the Bit demo opens
  (`shell/src/demo.rs` + cockpit init), so the bare Generate button already works
  before the user touches anything.
- Optional "generate the whole Bit pack" action that iterates `BIT_ACTIONS` and
  queues a sheet per action, for a one-click demo of the full character.
- Treat presets as data (id, name, subject, template, structure, style,
  normalize/slice defaults) so the pack can grow without UI churn — the shape
  agent-sprite-forge uses for its mode/preset system.

## Ideas borrowed from agent-sprite-forge

The reference repo is agent-first with deterministic Python post-processing, no
interactive editor, but several of its ideas map cleanly:

- **Safe-area margins + center crosshairs** on the slice gizmo, so cell fit and
  drift are visible at a glance (`make_layout_guide.py`).
- **Edge-touch and connected-component overlays** in the normalize view — show,
  per frame, what crossed a cell edge or split into parts rather than just
  reporting a count (`generate2dsprite.py` QC metadata).
- **Component-mode toggle** as a first-class control (largest body vs. keep
  detached FX).
- **Presets as portable data**, with per-preset processor defaults
  (component mode, fit/align), not hardcoded UI.

## Phasing

1. **Phase 1 — see it and start it.** Sheet retention + the Sheet stage +
   uniform slice gizmo (Feature 1, Feature 2 Phase A) and the preset cards +
   default selection (Feature 4). Highest user value, lowest risk; unblocks the
   "can't see the sheet" and "first-click result" complaints immediately.
2. **Phase 2 — fix it.** Editable normalization knobs + per-frame
   include/exclude + raster handoff (Feature 3).
3. **Phase 3 — precision and durability.** Non-uniform dividers (Feature 2
   Phase B) and slice-spec persistence on the `Animation`.

## Critical files

- `shell/src/ai.rs` — `StaticSheetJob`, `run_static_sheet`, `sheet_to_frames`,
  `ShellMsg::StaticSheetReady` (sheet retention).
- `core/src/transforms/sheet.rs` — `slice_grid`, `slice_rects`, and the new
  `slice_grid_spec` (gizmo backend).
- `shell/src/studio.rs` — `StudioStage` (add `Sheet`),
  `studio_normalize_surface` / `studio_normalize_inspector`,
  `refresh_normalize_cache`, `normalize_cache_key`.
- `core/src/transforms/normalize.rs` — `NormalizeOptions`, `NormalizeReport`,
  `ComponentMode`.
- `core/src/project/animation.rs`, `core/src/project/qc.rs` — slice-spec
  persistence next to `AnimationQc`.
- `shell/src/gizmo.rs` — `BoxGizmo`, `GizmoHandle` (reuse for the slice overlay).
- `shell/src/cockpit.rs` — `cockpit_dials`, `cockpit_generate` (preset cards).
- `ai/src/compose/builtins.rs` — `BIT_ACTIONS`, the builtin prompt pack.
- `shell/src/demo.rs` — Bit demo seed (default pre-selection).

## Verification

Per feature, run the shell (`cargo run -p pixhaus-shell`) and check end to end:

- **Sheet visible:** open the Bit demo, generate a static sheet, confirm the raw
  sheet renders in the new Sheet stage at fit and 1:1.
- **Gizmo re-cuts:** drag a cell edge and adjust offset/gutter/inset; the sliced
  preview updates live and matches the gizmo. Confirm a default uniform spec
  cuts identically to today.
- **Normalize edits:** change a knob (e.g. chroma tolerance, reference height,
  component mode) and watch the normalized strip and report update; toggle a
  frame out; send a frame to Draw, edit, and confirm it returns to the pick set.
- **One click:** open the demo and click Generate without changing anything —
  good result. Click a preset card — generates that action in one click.

Automated:

- Unit tests for `slice_grid_spec` mirroring the existing `slice_grid` rstest /
  proptest / snapshot style in `sheet.rs`, including the invariant that a plain
  uniform spec equals `slice_grid`.
- A round-trip test that a `SliceGrid` persisted on an `Animation` survives
  MessagePack save/load (alongside the existing `AnimationQc` round-trip).
- Keep the Stop gate green: `cargo fmt --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test --workspace`, `cargo doc`,
  `cargo deny check`.
