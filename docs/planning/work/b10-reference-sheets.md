# B10 — Reference sheets and the anchor mechanic

The bedrock spec for AI-generated character / item / tileset reference sheets and the anchor mechanic that uses them to keep every subsequent generation consistent.

This is the central feature that distinguishes Pixhaus from "another AI sprite generator." Every studio's art pipeline starts with model sheets — turnaround, palette, expressions, callouts — because consistency without them is impossible. AI generation amplifies the consistency problem (every fresh prompt drifts), and AI generation also solves it (one anchor sheet, fed as reference to every subsequent call). B10 is the system that makes that loop work.

Depends on B9 landing first. Built on top of the verb runtime (S21), the verb plugin protocol (B5), and the existing image-gen backends (S22).

## Why now

After B9 ships, the data model knows about Reference entities and the `anchor_reference_id` pointer on every Custom-kind entity. What it doesn't have: any way to *generate* a reference sheet, *iterate* on it, *approve* it, or *use* it. B10 ships those four behaviours.

Without B10, a Pixhaus user has fourteen AI verbs that produce sprites individually, with no shared anchor. The Hero in idle looks subtly different from the Hero in walk because the model drifts between calls. With B10, every verb invocation for a Custom entity passes its anchor sheet to the backend as a reference image (and, for backends that support it, runs the per-entity LoRA the sheet trained). Consistency is mechanical, not hopeful.

## The mental model

A reference sheet is an authoritative document about an asset. For a Character, it shows: turnaround views (front, side, three-quarter, back), 2-3 facial expressions, a labelled palette swatch row, detail callouts (scars, accessories, runes), optional outfit variants, and a side panel with structured info (name, age, species, personality notes). Studios call this a model sheet or character bible. AI image generation tools call it a reference image, but the studio version is far more structured.

For an Item or Prop, the sheet shows: multi-angle views, detail callouts, palette. For a Tileset, the sheet shows: tile primitives, autotile preview, palette. The composition differs by entity kind; the underlying data structure is the same.

The user produces a sheet through an AI workflow:
1. Open the entity, click "Generate reference sheet"
2. Pick a composition template (Character / Item / Tileset / Custom)
3. Give a prompt ("32x32 fantasy hero with a sword and blue cloak")
4. Pick a backend (Anthropic / Stability / Replicate / ComfyUI / local SDXL)
5. AI produces 1-4 candidate sheets
6. User picks one or none — if none, refine the prompt and regenerate
7. Once happy, user can refine in place ("make the hair longer", "add a scar over the left eye") — the verb runs an inpainting pass on the canonical sheet
8. User clicks "Approve" — the sheet becomes the canonical anchor for this entity

Every other AI verb run on this entity from that point forward includes the canonical sheet's image as an IP-Adapter reference, applies the sheet's extracted palette as a generation constraint, and (where backends support it) loads the per-entity LoRA. The result is consistency by mechanism rather than prompt-engineering hope.

## What B10 delivers

### B10.1 — Sheet generator verb

A new AI verb `generate-reference-sheet`. Takes a target Reference entity, a composition template, a prompt, and backend selection. Produces 1-4 `SheetVariant` candidates and stores them in the Reference's `history` (none are canonical yet — the user has to approve).

Composition templates (defined in `ai/src/verbs/reference_sheet/templates.rs`):
- **Character** — turnaround (front, side-left, side-right, three-quarter, back) + 3 expressions + palette swatch + 2 detail-callout slots + 1 outfit-variant slot. Layout: vertical strips, ~1024x1536 sheet image.
- **Item** — 4-angle turnaround + 2 detail callouts + palette swatch. Layout: 2x2 grid of views with callouts inset, ~1024x1024.
- **Tileset** — tile primitives row + transition variants row + autotile preview block + palette swatch. ~1024x1024.
- **Custom** — single full-body image + palette swatch. The simplest template, falls back when no other template fits.

Each template has a backend prompt-engineering layer that takes the user's prompt and produces a backend-appropriate generation request. For Stability / ComfyUI, this is a long structured prompt with negative prompts, layout instructions, and (where supported) ControlNet pose conditioning.

For backends without composition support, the verb either:
- Calls the backend N times with cropped sub-prompts and composites the results into one sheet (the layered approach), or
- Returns one composite generation and lets the user re-crop manually if the layout is wrong

The first approach is preferred for sheet quality; the second is a fallback for low-budget runs.

### B10.2 — Sheet iteration verb

A second verb `iterate-reference-sheet`. Takes an existing `SheetVariant` and a refinement prompt, produces a new `SheetVariant` derived via inpainting. Examples: "make the hair longer", "add a scar over the left eye", "change the cloak from blue to red". The new variant lands in the Reference's `history`.

The verb is panel-aware: if the user clicks a specific panel (e.g., the "happy" expression) before running iteration, the inpainting is scoped to that panel's `Rect`. The rest of the sheet stays pixel-stable. This matters — without panel scoping, every iteration regenerates the whole sheet and accumulates drift.

### B10.3 — Approval flow + anchor wiring

Approval is a UX flow plus a small data change. The user clicks a `SheetVariant` in the history → "Approve as canonical." The variant moves to `canonical`; the previous canonical demotes to `history[0]`. Then:

- The Reference entity's `extracted_palette` runs (eyedropper extraction across the sheet image, deduplicated, ordered by frequency)
- The Reference entity's `composition` panel rectangles get computed (the generator wrote them when it produced the sheet; approval just locks them as canonical)
- For Custom entities pointing at this Reference via `anchor_reference_id`, a project-level cache invalidates so the next AI verb invocation rebuilds the anchor payload

The anchor payload (computed lazily on verb invocation, cached until sheet changes):
- `image_base64`: the canonical sheet image, encoded for backends that accept reference images
- `palette`: the extracted palette as a constraint
- `lora_path`: optional per-entity LoRA path if the project trains one (S30 — Project Style Learning extends to per-entity LoRAs in B10)
- `composition_hints`: for backends that can use them, the panel labels and rectangles

The 14 existing AI verbs (Inbetween, Continue, Extend, Variant, Cleanup, Tile, Critique, Project Style Learning, Conversational, Motion-from-video, Auto-mesh-deformation, Audio-driven, Tileset-from-description, Sketch-finishing) gain an optional `anchor: Option<AnchorPayload>` parameter. The verb runtime resolves the anchor from `Entity.anchor_reference_id` automatically when the verb is invoked against an entity that has one. Verbs that produce sprites for the same entity inherit consistency for free.

### B10.4 — Sheet UI

A new panel in the editor: the **Sheet view**. Opens when a Reference entity is the active target, or when the user clicks "View anchor sheet" from a Custom entity. Shows:

- The canonical sheet image at fit-to-window
- Panel overlay (toggleable) showing the labelled rectangles
- Asset info side panel: editable name/age/species fields, personality notes
- History thumbnails strip across the bottom — click to preview, drag to compare side-by-side, right-click to "Approve as canonical" or "Delete"
- Prompt history strip — chronological list of prompts with re-run buttons
- "Generate variant" button — kicks off B10.1 with the current prompt as default
- "Refine selection" button — only enabled when a panel is selected via click; kicks off B10.2 scoped to the panel

### B10.5 — Per-entity LoRA training (optional, defer-able)

For backends that support LoRA training (Replicate, local Diffusers), B10 extends Project Style Learning (S30) so the LoRA can train per-entity from the canonical sheet. A small button on the Sheet view: "Train consistency LoRA from this sheet." Training takes 15-30 minutes on a consumer GPU. Once trained, the LoRA path lands in the Reference's metadata and ships in every anchor payload for that entity.

This is the mechanic that takes consistency from "good" to "indistinguishable across hundreds of generations." Worth shipping but not blocking — sheets without per-entity LoRAs already do most of the work via IP-Adapter.

## UX flows

### Generating a Hero from scratch

1. User clicks "+" in library → kind: Custom → category: "Character" → name: "Hero" → submits.
2. Library lands the Hero entity with no anchor and no states.
3. The new-entity flow asks "Generate a reference sheet now?" with a "Skip" option.
4. User clicks "Generate" → opens the sheet generator dialog → composition: Character → prompt: "32x32 fantasy hero, blue cloak, sword, brown hair" → backend: Stability → variants: 4.
5. Generator runs. ~30-60 seconds depending on backend. 4 candidate `SheetVariant`s land in a new Reference entity named "Hero Reference."
6. Hero entity's `anchor_reference_id` is set to the new Reference's id automatically.
7. User reviews the four candidates in the Sheet view. Picks the one they like → "Approve as canonical."
8. Done. The Hero is now anchored. The user can run any AI verb on the Hero (Continue, Extend, Variant) and it'll use the sheet for consistency.

### Iterating on the sheet

1. User opens the Hero's Reference in the Sheet view.
2. Selects the "happy" expression panel by click.
3. Clicks "Refine selection" → prompt: "wider smile, blushing cheeks."
4. Iteration verb runs an inpainting pass scoped to that panel. ~10-20 seconds.
5. New variant lands in `history`. User compares side-by-side and approves the new one.

### Generating states using the anchor

1. User has an approved Hero sheet.
2. Clicks Hero in the library → kind shows "Custom (Character)" with anchor sheet active.
3. Clicks "Add state" → name: "idle." A new Sprite opens.
4. Clicks "Generate" verb → backend uses the anchor's image + palette + (optional) per-entity LoRA → produces an 8-frame idle. The Hero in idle looks like the Hero in the sheet because the sheet drove the generation.
5. Repeat for walk, run, attack, hurt, death. Each state inherits the same look.

### Cross-entity reuse

1. User wants a Goblin "like the Hero but green and shorter."
2. Creates a new Custom("Enemy") entity "Goblin" with no anchor yet.
3. Runs the Variant verb (S26) with source = Hero's anchor sheet, prompt = "green skin, shorter, hunched." The Variant verb produces a new Reference entity (the Goblin sheet) anchored on the Hero's sheet for consistency.
4. User reviews and approves the Goblin sheet.
5. Goblin now has its own anchor; subsequent state generation uses the Goblin sheet.

## Implementation outline

Estimated: ~12-15 days of agent work split across five sub-tasks.

- **B10.1 — Sheet generator verb** — the `generate-reference-sheet` verb plus four composition templates (Character / Item / Tileset / Custom) plus the backend prompt-engineering for Stability + Replicate + ComfyUI. ~3-4 days.
- **B10.2 — Sheet iteration verb** — the `iterate-reference-sheet` verb with panel-scoped inpainting. ~2-3 days.
- **B10.3 — Approval flow + anchor wiring** — the UX flow, the palette / composition extraction at approval time, the cache invalidation, and the `anchor: Option<AnchorPayload>` parameter on the 14 existing verbs. The 14-verb update is mechanical but touches every verb file. ~3 days.
- **B10.4 — Sheet UI** — the Sheet view panel in Solid: image display, panel overlay, asset info side panel, history strip, prompt history strip, generate / refine buttons. ~3 days.
- **B10.5 — Per-entity LoRA training (optional)** — extend S30 to train per-entity from the canonical sheet, ship a training button on the Sheet view. ~2-3 days. Defer if the rest of B10 is producing acceptable consistency without it.

Total without B10.5: ~11-13 days. With B10.5: ~13-16 days. Critical path is B10.1 + B10.3 — approximately 6-7 days to a usable sheet system; B10.2 and B10.4 enrich the experience and B10.5 is the optional consistency-amplifier.

## Open questions

These need decisions before B10.1 dispatches. Each shapes the verb interface and the UX.

### 1. Sheet image storage: inline or external?

A 1024x1536 PNG sheet is roughly 500KB-2MB. A project with twenty Custom entities each holding a Reference with five `SheetVariant`s in history is 50-200MB of images. Storing inline in the `.pixhaus` (MessagePack + zstd) file works but bloats the project file.

Options:
- **Inline (current data-model assumption)** — simplest, project file is portable, cost is size.
- **External in `<project>/_images/`** — sheet images stored next to the `.pixhaus` file in a sibling directory. The data model stores paths instead of bytes. Project file stays small, but moving a project means moving the directory.
- **Hybrid** — inline up to a size cap, external above it. More complexity, edge cases.

Recommendation: external in `<project>/_images/` with a deterministic file name scheme (`sheet-<sheet_variant_id>.png`). Pixhaus already uses a project directory pattern (the existing format extends naturally). Cleaner for version control; users can git-lfs the images directory. The data model needs `ReferenceImage` to support both inline and path-pointer variants — a small extension.

### 2. Composition template extensibility

Should composition templates be plugin-extensible? Right now I have four built-in: Character / Item / Tileset / Custom. A studio might want a sixth: "Vehicle profile sheet" with side / 3/4 / overhead views and wheel close-ups, no expressions.

Options:
- **Built-in only for B10** — ship the four, plugin-extensible in a future spec.
- **Plugin-extensible from B10** — the verb plugin protocol (B5) extends to register composition templates.

Recommendation: built-in only for B10. The composition layout is tightly coupled to backend prompt engineering, and getting four templates right is meaningful work. Plugin extensibility is a clear follow-up that doesn't compromise the data model.

### 3. Backend selection per entity vs. project-wide

When a user clicks "Generate sheet," should backend selection be per-invocation, per-entity, or project-wide?

- **Per-invocation** — the most flexible, the most decisions for the user.
- **Per-entity** — each entity remembers its preferred backend.
- **Project-wide default with override** — project sets a default, generate dialog lets the user override. Recommended.

### 4. History cap

`ReferenceSheet.history` can grow indefinitely. A user iterating thirty times on a sheet has 29 rejected variants in history. Cap?

Recommendation: configurable cap in `ProjectAi`, default 20. Older variants get evicted oldest-first, and an "archive" option preserves a variant indefinitely (sets a flag preventing eviction). UX surfaces an "Archive" button in the history strip.

### 5. Inpainting for non-rectangular regions

Sheet panels are stored as `Rect`. Real artist refinements sometimes target non-rectangular regions ("just the eyes", "the hand holding the sword"). Should the iteration verb support arbitrary masks?

Recommendation: rectangles only for B10. Mask support is a follow-up, and most useful refinements are panel-scoped (the panels themselves are rectangles).

### 6. Anchor strength — backend-specific or uniform?

IP-Adapter has a strength parameter (0.0-1.0). Different backends have different consistency knobs. Should the anchor expose a "consistency strength" the user can tune per-verb invocation?

Recommendation: yes, per-invocation, with a project-level default of 0.7 (strong but not rigid). Surface it in the AI verb dialog as a slider labeled "Consistency vs. variation."

## Acceptance criteria

- The two verbs (`generate-reference-sheet` and `iterate-reference-sheet`) ship and pass round-trip tests against mock backends
- The four composition templates produce valid sheets for at least Stability and ComfyUI backends
- The 14 existing AI verbs accept and consume the optional anchor payload; verbs run unchanged for entities without an anchor
- The Sheet UI panel renders, supports approve / iterate / generate-new flows
- Sheet images stored externally per the decision in Open Question 1, with the `.pixhaus` project file staying under 5 MB for a typical 20-entity project
- Documentation in `docs/reference-sheets.md` covering the sheet generation workflow, the anchor mechanic, and per-entity LoRA training
- Sample project in `examples/` demonstrating: a Hero with anchor sheet, three states (idle, walk, attack), a Goblin Variant'd from the Hero, all visually consistent

## What this enables next

B10 is the substrate every AI-native workflow downstream wants:

- **Generate a full RPG cast** — a verb chain that takes a project, generates ten character sheets in a unified style, anchors each, generates standard states for each. One prompt per character, hours of art work eliminated.
- **Reference-driven Tilemap generation** — a Tileset entity with an anchor sheet (palette + tile primitives) drives a Tilemap-from-description verb that produces a full level using only colors and motifs from the sheet.
- **Cross-project asset borrow** — a user imports a sheet from a previous project; subsequent generation in the new project uses it as the anchor. Style continuity between games.
- **The "fix this character across every state" command** — when the Hero's anchor sheet changes (user iterated and re-approved), a verb sweeps every state Sprite under the Hero entity and re-generates with the new anchor. Reskinning a character is one verb away.

Without B10, every one of those is a per-prompt fight. With B10, the data model and the anchor mechanic does the work.
