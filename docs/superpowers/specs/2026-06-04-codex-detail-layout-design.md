# Codex detail-tab layout: responsive columns

**Status:** approved design, pre-implementation
**Scope:** the Codex workspace center entry-detail (`modules/codex/src/codex_ws.rs`) and a small reusable layout helper in `crates/ui`
**Branch:** `feat/codex-workspace`

## Problem

The Codex entry-detail tabs render their cards in a single full-width vertical
stack (`overview_tab` and the sibling tab bodies call `widgets::info_card(...)`
one after another with `ui.add_space` between them). Two effects:

1. On a wide center pane the cards waste most of the horizontal space — it reads
   as a tall single-file list.
2. Editable text cards (Summary/Lore/Notes) reserve tall text areas even when
   empty, padding the page with dead space.

The Type/Status/Created/Updated/Author/ID/Version metadata is also shown twice:
once as a strip in the persistent header (above the tab bar) and again in the
Overview "Key Info" card.

The target is the magazine/masonry card layout the user supplied (a multi-column
grid of content-sized cards), adapted to the locked Pixhaus visual direction.

## Decisions (locked with the user)

- **Responsive 1/2/3 columns.** Column count adapts to the available center-pane
  width (it varies a lot with the left/right docks open/closed). Cards keep a
  fixed home column so order stays predictable — no dynamic shortest-column
  balancing (which would need a two-pass height measure in immediate mode).
- **Key-info lives only in the Key Info card.** The header is slimmed to portrait
  + name + status badge + `@handle` chip + type chip + description + tags +
  actions. The full metadata (+ Handle/Aliases) lives only in the Overview Key
  Info card, with an optional "View all metadata" expander.
- **Scope: all detail tabs** — Overview, Visual, Anchors, Prompt, Relations,
  History (and Coverage where columns help). The helper is built reusably and
  applied to each.

## Approach

### Layout mechanism

Use egui's built-in `ui.columns(n, |cols| { … })` (no new dependency). Choose `n`
from the pane width, then place cards into `cols[i]` sequentially. Because card
bodies run sequentially inside the closure, each can take `&mut intents` /
`&mut draft` without the borrow conflict a `Vec<closure>` would create.

### Reusable helpers (in `crates/ui/src/widgets`)

- `column_count(available_width: f32) -> usize` — pure; returns 1/2/3 by
  breakpoint (≈ ≥1040 → 3, ≥640 → 2, else 1). Tunable constants. Unit-tested.
- `distribute(card_count: usize, columns: usize) -> Vec<Vec<usize>>` — pure;
  assigns card indices to columns by a fixed, order-preserving rule (curated for
  the common 3/2 cases, natural order for 1). Unit-tested for balance and order.

A tab body builds an ordered list of its card kinds (an enum or index range),
computes `n = column_count(ui.available_width())`, calls `distribute`, then
`ui.columns(n, |cols| …)` and dispatches each card kind to `render_card(kind,
&mut cols[col], theme, intents, draft, detail)`. `render_card` is a per-tab
match that runs the existing `info_card` bodies — the card *content* and the
`Intent`s they push are unchanged; only their placement changes.

### Per-tab card sets

- **Overview:** Summary, Key Info, Lore, Role, Tags, Quick Links, Notes.
  - 3 cols: [Summary, Role, Notes] · [Key Info, Tags] · [Lore, Quick Links]
  - 2 cols: [Summary, Lore, Notes] · [Key Info, Role, Tags, Quick Links]
  - 1 col: natural order.
- **Visual:** Visual description, Key Visual, Palette, Silhouette, Quick Anchors,
  Generation Readiness — distributed the same way.
- **Anchors:** the per-kind anchor cards + the add-anchor control.
- **Prompt:** positive fragments, negatives, compiled preview.
- **Relations:** relations list/graph + add control (single wide column is
  acceptable here if a graph needs full width — `column_count` may be capped per
  tab).
- **History:** the version timeline (likely stays 1–2 columns).
- **Coverage:** the slot grid already tiles; apply content-sizing, columns
  optional.

Each tab may cap its max column count where a wide element (graph, timeline)
reads better full-width.

### Card sizing

Cards size to content. Editable multi-line text (Summary/Lore/Notes/Visual
description) uses a compact `desired_rows ≈ 3` and grows modestly rather than
reserving a large block. Stat/list/chip cards shrink to their content. This is
the main fix for wasted vertical space.

### Header slim-down

Remove the key-info strip from the header in `codex_ws.rs`. Keep the inline
rename (name + handle) behavior. The full metadata + Handle/Aliases render in the
Overview Key Info card; add a "View all metadata" expander
(`codex.overview.keyinfo.view_all` key) — optional, can ship collapsed.

## Layers touched

- `crates/ui` (widgets): `column_count`, `distribute`, and any small shared
  card-grid glue. No state/intent changes.
- `crates/services` (locales/codex.yaml): only if new label keys are needed
  (e.g. "View all metadata"). No logic.
- `modules/codex` (`codex_ws.rs`): restructure each tab body to use the helper;
  remove the header key-info strip; apply content-sizing; route the same Intents.

No `core` changes. No data-model, command, or intent changes — this is
presentation only. The deferred-intent model, theme-token rule, accent restraint,
phosphor icons, and `tr()` localization all stay intact.

## Testing

- Unit tests for `column_count` (each breakpoint) and `distribute` (correct
  column counts, order preserved, balanced spread).
- The existing `codex_layout_snapshot` test stays green (panel ids / regions
  unchanged).
- Render-harness verification: `cargo run -p pixhaus-app --example
  render_workspaces` → `target/ui-snapshots/codex.png`, compared to the user's
  target mockup and `docs/pixhaus_visual_ux_direction.md`. Because the harness
  defaults to the Overview tab, optionally widen the seed or emit per-tab frames
  to verify the other tabs.

## Out of scope

- True shortest-column masonry (dynamic height balancing).
- Real asset thumbnails / sprite store (key-visual, linked assets stay MOCK).
- The cosmetic gaps tracked separately (real relations node-graph, live top-bar
  coverage status).

## Success criteria

- Each detail tab lays its cards into 1/2/3 columns by pane width, cards
  content-sized, no large empty reserves.
- No duplicated key-info; header is slim, Key Info card is canonical.
- Full Stop gate green; layout reads cleanly against the mockup and the visual
  direction; accent restraint and tokens preserved.
