# AI Studio redesign — design and implementation plan

Living plan for reworking the v2 shell's top-level workflow around a unified AI
Studio. Update the checkboxes and the "Current state" note as work lands. This file
is the durable record across sessions — read it first when resuming.

## Current state

All four workstreams plus the cockpit/library integration are implemented on
branch `feat/ai-studio-redesign` (2026-05-28). Gate suite green: `cargo fmt
--check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest
run --workspace` (646 passed), `cargo test --doc`, and `cargo doc` all pass.
`cargo deny` still reports the pre-existing license failure (`Ubuntu-font-1.0`
from the egui fonts and the null-license `cfg_block` pulled via keyring 4.x →
turso) — unchanged by this work, which added no dependencies. Manual, in-app
verification still needs a human at the running binary.

### Follow-up phase — cockpit + library folded in (done)

The cockpit and composition library are no longer separate Create surfaces.
Create is always the full-screen AI studio. The studio's **Anchor stage is the
cockpit**: its inspector hosts the composition controls (context,
structure/template/dials, references, composed prompt, generate, history); its
center is the variant gallery (re-roll / more-like-this / refine / branch / card
/ provenance / approve). The composition **library opens as an overlay** from a
header button (browser + record editor), and "Save as template" routes there.
`CreateView`, `create_dock`, and the dock/central cockpit-or-library routing are
gone; the anchor's `GenThread` and the `GenTarget` generalization are removed
(only the first-frame stage uses the chat-style generator, which keeps
mask-inpaint, the box gizmo, and hand-edit). Saved **project records now feed
generation** — the pickers, the composed-prompt preview, and the verb merge
project structures/styles/prompts over built-ins (shadow by id).

### Follow-up phase — shared pan/zoom center viewport (done)

Both generation stages share one center viewport (`RefineView` + the free
`refine_surface` / `refine_canvas` in `studio.rs`): the selected result is shown
large with wheel-zoom about the cursor, drag-to-pan, and a Fit reset, plus the
inpaint mask (brush or box gizmo) and a Regenerate-masked-region action in a
center toolbar. The **Anchor stage's results gallery moves into the right
inspector** (cockpit cards); clicking a card selects it for the center
(`anchor_selected`). The **First-frame stage gains pan/zoom** via the same
viewport. Inpaint-refining a selected anchor result lands a **new linked
`CockpitCandidate`** (inheriting the parent's provenance) via
`ai::spawn_anchor_refine` -> `ShellMsg::AnchorRefineDone` -> `land_anchor_refine`.
The unused sprite-canvas preview (`show_sheet_preview`) is dropped.

## Verbatim intent

Pixhaus is an AI-native tool for creating game sprites. Today it opens on an
unnamed 64x64 sprite and lets you draw and use layers, but the workflow is flawed:

- Color and Layers live in separate tabs, which makes using both at once hard.
- AI work is split: "Create" mode jumps straight to the anchor, or you move to the
  animation Studio mode.
- Everything is tied to a sprite — without a sprite, nothing makes sense. A sprite
  is the entry point for everything (e.g. before the Studio you must generate an
  anchor).

The intent: replace AI create mode with a unified AI Studio that takes over the
whole screen like the current animation Studio, but with a way to navigate, manage,
and select between all sprites. Guide the user — if they go to first frame, guide
them to anchor creation first. The anchor must be fully refinable, which calls for
an agentic, chat-style loop: generate the first image, keep iterating, with an
inpaint mask tool with gizmos and a full editing mode to refine anything in the
anchor. Apply the same generation-and-refine loop to the first frame. Treat this as
a chance to fully refine the UI — a massive change is acceptable.

## Locked decisions

- **Studio scope:** merge the Cockpit and the animation Studio into one AI Studio.
  Keep Draw and Animate untouched as the manual raster editors you drop into for
  hand work. The AI Studio is not a replacement for manual pixel editing; it
  orchestrates generation and hands off to Draw/Animate.
- **Color + Layers:** stack them as collapsible sections in one right dock with a
  draggable divider, both visible at once. No more mutually-exclusive tabs.
- **Chat agency:** conversational generation. Each turn issues one concrete request
  (text-to-image, inpaint, or variation); the thread carries lineage; the user
  drives every step. This is conversational image generation, not an autonomous
  LLM agent loop.
- **Anchor refinement depth:** inpaint mask with transform gizmos (move, scale,
  rotate the masked region), plus a hand-edit button that opens the image in the
  existing Draw pixel editor and returns it to the studio on save.

## Architecture overview

```
ShellApp
  Workspace::Draw / Animate  -> manual raster editors (mostly unchanged)
    right dock: PALETTE + LAYERS stacked (collapsible), Sprites tab removed
  Workspace::Create -> AI Studio (full-screen takeover; replaces Cockpit/Studio split)
    +-- left:   SPRITE GALLERY (all sprites, thumbnails, new / select / manage)
    +-- center: active STAGE surface
    +-- right:  stage inspector + GENERATION CHAT thread
    stages: Anchor -> First frame -> Motion -> Clip -> Pick -> Land
            ^ guided gating: stage N+1 stays reachable but shows a guiding
              call-to-action until stage N's artifact is approved
```

The studio already lays out a header, a left stages rail, a center surface, and a
right inspector (`shell/src/studio.rs:403`), already free-navigates its stages, and
already drives the clip pipeline unchanged. This redesign extends that shell rather
than rebuilding it: it folds the Cockpit's anchor generation in as the first two
stages, adds the sprite gallery, and upgrades anchor and first-frame generation to a
conversational thread with gizmo inpaint.

## Ground truth about today's code

- `ShellApp` (`shell/src/app.rs`) holds `workspace: Workspace` (`Draw | Animate |
  Create`, enum near `app.rs:244`), `right_tab: RightTab` (`Color | Layers |
  Sprites`, `app.rs:319`), `create_view: CreateView` (`cockpit.rs:26`), and
  `studio: StudioState` (`app.rs:443`).
- The right dock renders the `RightTab` strip near `app.rs:2043` with the dock host
  near `app.rs:2357`. Panel bodies are `palette_panel.rs:18` and
  `layers_panel.rs:46`.
- The Cockpit (`shell/src/cockpit.rs`, ~1170 lines) is the AI front door:
  structure/style/dials prompt composition, a reference-sheet generation gallery,
  and approval of a variant as the anchor via `doc.set_active_anchor(png)`
  (`document.rs:104`).
- The anchor is `AnchorPayload` (`ai/src/plugin/anchor.rs`), stored per-sprite as
  `anchors: HashMap<SpriteId, Vec<u8>>` (`document.rs:74`). Read with
  `active_anchor()` (`document.rs:112`). Almost all downstream generation gates on
  `active_anchor().is_some()`.
- The animation Studio (`shell/src/studio.rs`) is a full-screen `CreateView::Studio`
  takeover with six stages (`StudioStage`, `studio.rs:40`): Anchor, FirstFrame,
  Motion, Clip, Pick, Land. It already has first-frame candidates
  (`FirstFrameCandidate`, `studio.rs:170`), an inpaint `MaskOverlay`
  (`studio.rs:191`, brush `stamp` at `studio.rs:229`, canvas at `studio.rs:619`),
  and an `approved_first_frame`.
- The data model already supports many sprites: Project -> Entities -> Sprite states
  (`core/src/project/`). `shell/src/library.rs:60` enumerates them, but selection is
  a single switcher — there is no gallery.
- Generation is async: tokio plus a `ShellMsg` mpsc channel (`shell/src/ai.rs`,
  `ShellMsg` near `ai.rs:62`). First-frame path: `FirstFrameJob` (`ai.rs:556`),
  `run_first_frame` (`ai.rs:588`), `spawn_first_frame` (`ai.rs:854`); results post
  back over the channel and call `request_repaint`. Backends are FAL and OpenAI
  (`ai/src/backends/`).

Line numbers are a 2026-05-28 snapshot; re-read before editing.

## Workstream 1 — Unified right dock (Color + Layers)

Goal: both panels visible in Draw and Animate, no tab switching.

- In `shell/src/app.rs`, replace the `RightTab` tab strip with one vertical layout:
  a collapsing section for Palette on top and one for Layers below, separated by a
  draggable splitter (egui `Resize`, or a manual drag handle storing a fraction on
  `ShellApp`).
- Reuse the panel bodies unchanged: `palette_panel.rs:18` and `layers_panel.rs:46`.
  Only their host container changes.
- The Sprites tab leaves the dock. Rich sprite navigation moves to the AI Studio
  gallery (W2). For Draw/Animate, keep the existing lightweight top-bar sprite
  switcher; the `library.rs` browser stays reachable.
- Remove `RightTab` (or reduce it to a persisted split fraction) and delete the dead
  tab-selection state.

Files: `shell/src/app.rs` (dock host, `RightTab` removal); reuse `palette_panel.rs`,
`layers_panel.rs`.

## Workstream 2 — AI Studio shell + sprite gallery + guided flow

Goal: one full-screen AI workspace; the sprite is the entry point; the flow guides
you anchor-first.

- Entry: `Workspace::Create` routes straight to the studio takeover (`studio_view`,
  `studio.rs:403`). Drop the `CreateView::Cockpit` surface and fold what it did into
  the Anchor and First-frame stages. Resolve `CreateView::Library` (fold into the
  gallery, or keep as a secondary view) during the build.
- Sprite gallery (left region): a new panel listing every sprite across the
  project's entities with thumbnails, a "New sprite" action, and select / rename /
  delete. Extract a reusable `sprite_gallery` widget from the enumeration logic in
  `shell/src/library.rs:60`; render thumbnails from each sprite's pixel buffer via
  the texture-upload path the canvas already uses. Selecting a sprite sets
  `doc.select(...)` so every stage targets it.
- Guided gating: the stage rail already free-navigates (`StudioStage`,
  `studio.rs:40`). Add per-stage prerequisite state — First frame, Motion, and later
  stages stay clickable but render a guiding call-to-action ("Create an anchor
  first") with a button that jumps to the prerequisite stage, instead of an empty
  surface. Anchor existence already gates via `doc.active_anchor()`; extend the same
  pattern to first-frame -> motion.
- Per-sprite session state: the studio's in-flight state (`StudioState`, candidates,
  masks, approved frames — `studio.rs:170`–`282`) is currently a single session on
  `ShellApp`. Key it by `SpriteId`, or reset on switch, so switching sprites in the
  gallery is predictable. Default to reset with a warning if a job is running;
  revisit persist-vs-reset during the build.

Files: `shell/src/studio.rs` (gallery region, gating CTAs, session keying),
`shell/src/app.rs` (Create routing, drop the Cockpit surface), `shell/src/cockpit.rs`
(salvage anchor-generation pieces into the stages, then shrink or retire); reuse
`shell/src/library.rs`, `shell/src/document.rs`.

## Workstream 3 — Conversational generation (anchor + first frame)

Goal: generate the first image, then iterate by replying; the thread shows lineage;
the same loop serves the Anchor stage and the First-frame stage.

- Model: a `GenThread` of `GenTurn`s. Each turn records the request kind
  (`TextToImage { prompt }`, `Inpaint { mask, base }`, `Variation { base }`), its
  inputs, and the resulting candidate image(s); a selected candidate carries forward
  as the base for the next turn. This generalizes the existing `FirstFrameCandidate
  { png, parent, origin }` (`studio.rs:170`) and the Cockpit's variant-lineage
  tracking into one structure both stages share.
- UI: the right inspector becomes a chat-style transcript — past turns (prompt plus
  thumbnails, click to select or branch) above a composer (prompt box plus action
  buttons: Generate, Make variations, Inpaint selection). The center surface shows
  the selected candidate large.
- Async: reuse the existing tokio plus `ShellMsg` mpsc flow unchanged
  (`shell/src/ai.rs`: `spawn_first_frame:854`, `run_first_frame:588`,
  `FirstFrameJob:556`, `ShellMsg:62`, `request_repaint`). Add or repurpose job
  variants so anchor generation runs the same way first-frame does. Approving the
  selected candidate calls `doc.set_active_anchor(png)` for the anchor stage
  (`document.rs:104`) or sets `approved_first_frame` for the first-frame stage.
- Backend: generation uses the FAL and OpenAI backends already wired
  (`ai/src/backends/`). This is conversational image generation, not an LLM agent —
  no new chat-planning backend is required.

Files: `shell/src/studio.rs` (thread UI and state), `shell/src/ai.rs` (anchor job
parity); reuse `shell/src/document.rs`, `ai/src/backends/`.

## Workstream 4 — Inpaint gizmos + hand-edit hop

Goal: refine any region of the anchor or first frame precisely.

- Gizmo inpaint: extend the existing `MaskOverlay` (`studio.rs:191`, canvas at
  `studio.rs:619`, brush `stamp` at `studio.rs:229`) with a transform gizmo on the
  masked region — move, scale, and rotate handles drawn with egui shapes, so you can
  place and reshape the repaint area, not just brush it. Wire the result into the
  existing `FirstFrameJob::Inpaint` path (`studio.rs` start_inpaint -> `ai.rs`),
  reused for both the anchor and first-frame threads.
- Hand-edit hop: a Hand-edit button drops the current image into the Draw editor as
  a sprite/layer, switches to `Workspace::Draw`, and on save returns to the studio
  with the edited pixels as a new thread turn. Reuse the existing editor
  (`editor.rs`), the undo history, and the Reference/Raster layer plumbing in
  `core/src/project/layer.rs`. Track a "return to studio" breadcrumb on `ShellApp`.

Files: `shell/src/studio.rs` (gizmo and hop trigger), `shell/src/app.rs`
(Draw <-> Studio round-trip); reuse `shell/src/editor.rs`, `shell/src/canvas.rs`,
`shell/src/ai.rs`.

## Build order

Each workstream ships on its own `feat/<slug>` branch and PR per the repo workflow.

- [x] W1 — unified dock. Palette + Layers stacked as collapsing sections.
- [x] W2 — studio shell + gallery + gating. Create lands in the full-screen
  studio; the left panel hosts the sprite browser; stages guide anchor-first.
- [x] W3 — conversational generation. A shared `GenThread` drives the Anchor and
  First-frame stages: prompt, candidate thread, inpaint, approve.
- [x] W4 — gizmo inpaint + hand-edit hop. A move/scale/rotate box mask and a
  round trip into the drawing editor that lands the edit as a new candidate.

## Risks and open questions

- Cockpit retirement: the Cockpit holds structure/style/dials prompt composition.
  Decide what migrates into the Anchor-stage chat versus what is dropped. Do not lose
  the prompt-template machinery if it is load-bearing.
- Session per sprite: persist in-flight studio state per sprite, or reset on switch.
  Pick during W2; reset-with-warning is the safe default.
- Thumbnails at scale: gallery thumbnails for many large sprites must stay cheap
  (cache textures, downscale once) given the 8K performance constraint.
- `CreateView::Library`: fold into the gallery, or keep as a secondary view. Resolve
  in W2.

## Verification

- Gates (the Stop hook): `cargo fmt --all`, `cargo clippy --workspace --all-targets
  -- -D warnings`, `cargo nextest run --workspace`, `cargo test --doc --workspace`.
  New state and logic get unit tests; any visual panel gets an `insta` or
  `image-compare` check per the testing conventions.
- Manual, per workstream, by running the app (`cargo run -p shell`):
  - W1: open Draw, confirm Palette and Layers are both visible, the divider drags,
    and painting plus layer ops work without tab switching.
  - W2: enter the AI Studio, see all sprites in the gallery, create / select / rename
    one, and confirm the stages guide you to the anchor first.
  - W3: generate an anchor from a prompt, reply to iterate, branch from a past turn,
    approve a candidate; repeat on the first-frame stage; confirm the animation
    pipeline (Motion -> Land) still runs on the approved frame.
  - W4: mask a region, transform the gizmo, regenerate; press Hand-edit, paint in
    Draw, return, and confirm the edit lands as a new turn.
- Capture before/after clips for the PRs (UI change).
