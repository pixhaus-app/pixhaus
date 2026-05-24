# Animation generation UI/UX

Branch: `docs/animation-generation-pipeline` (spec only — ships with
`animation-generation-pipeline.md` in one docs PR).

This spec defines the user-facing surface for AI animation generation: everything a
user touches to go from an approved anchor reference sheet to finished, looping sprite
animations. The companion `animation-generation-pipeline.md` defines the engine
(verbs, i2v infrastructure, normalization); this defines the screens.

The surface is a dedicated full-screen **Animation studio**, modelled on the existing
`ReferenceSheetEditor` so the patterns the user already learned for reference sheets
carry over: generate, watch streaming candidates, review, accept.

Streams advanced: S19 (timeline), S25 (multi-direction), S27 (normalization), S32
(motion-from-video / frame picker). Anchor mechanic: B10.

## Why now

B10 gives the user an approved anchor (a canonical `SheetVariant`). The anchor is the
starting point: animations derive from it. The reference-sheet editor already proves
the full-screen generate/stream/review loop the user expects
(`ui/src/sheet/ReferenceSheetEditor.tsx`). The animation surface reuses that shape and
adds what animation needs that a single still does not: an animated loop preview, a
walk-cycle frame picker, a normalization check, and a landing in the timeline as a
real tagged animation.

## Design rules

- **Anchor-first.** Every flow starts from an entity that has an approved canonical
  anchor. No anchor, no studio — the UI routes the user to create one first.
- **Reuse the reference-sheet patterns.** Streaming request strip, candidate carousel,
  pan/zoom preview, accept-to-canonical — all already exist for sheets and are reused
  verbatim where they fit.
- **The result is an animation, not a pile of frames.** Accepting a candidate lands a
  layer, cels, and a `FrameTag` with the correct `LoopDirection`, then plays it.
- **Surface cost.** Image-to-video runs are slow and pricey; the studio shows the
  estimate before the run and elapsed time during it.

## Entry points and gating

| Entry point | Behaviour |
|---|---|
| Library entity row | New anchor badge shows whether a sprite has a canonical anchor. Context menu gains "Generate animations", enabled only when a canonical anchor exists. |
| No anchor yet | "Generate animations" is disabled with an inline affordance: "Create a reference sheet first" linking to `openSheetEditor(entityId)`. |
| Command palette | `ai:animations` -> "Animation studio". Opens the studio for the active sprite; if it has no anchor, opens the create-anchor affordance instead. |
| AI backend not configured | If no image / i2v backend key is set, the Generate action is disabled with a link to AI backend settings (`openPreferences()`), matching the existing AI-settings entry. |

The standalone `AnimatedSpriteSheetForm` (`ui/src/verbs/animated-sprite-sheet/`) folds
into the studio: its grid slider, action chips (`actions.length <= gridSize`), mode
buttons, and reference upload move into the studio's controls. The command-palette
`ai:animated-sprite-sheet` entry redirects to the studio.

Anchor status surfaces in `ui/src/library/LibraryPanel.tsx`; the studio open/close
mirrors `ui/src/sheet/sheet-state.ts` (`isSheetEditorOpen` / `openSheetEditor`) with an
`isAnimationStudioOpen` / `openAnimationStudio(entityId)` pair, mounted in
`ui/src/shell/Shell.tsx` in place of the canvas, exactly as the sheet editor is today.

## The Animation studio

Full-screen, replacing the canvas. Header, left controls, center animated preview,
candidate strip, streaming request strip, and a timeline ribbon showing the entity's
animation set.

```text
+--------------------------------------------------------------+
| < Back   Hero - Animations    anchor: south(approved)        |
|          est: i2v ~60s, ~12c          [ Generate ]           |
+----------------------+---------------------------------------+
| TYPE                 |                                       |
|  (o) Idle            |        [ animated loop preview ]      |
|  ( ) Walk   (i2v)    |          plays selected candidate     |
|  ( ) Attack          |          at target fps, looping       |
|  ( ) Custom          |                                       |
|                      |   foot baseline ------------------    |
| DIRECTION            |                                       |
|  [S] [W] [N] (E=flip)|   [|< ][ > play ][ loop ]  fps: 10    |
|                      +---------------------------------------+
| MODE                 | candidates                            |
|  (o) Grid image-gen  |  [#1*][#2 ][#3 ]    [accept][reject]  |
|  ( ) Walk via i2v    |  drift: ok   scale: ok   seam: ok     |
| frames: [ 8 ]        +---------------------------------------+
| > Advanced           | requests                              |
|  [ Generate ]        |  walk/S  running 14s  [====    ][x]   |
+----------------------+---------------------------------------+
| timeline:  idle[==]  walk[======]  attack[====]   > play     |
+--------------------------------------------------------------+
```

- **Header.** Back to canvas, entity name, anchor chip (direction + approved/draft),
  pre-run cost/latency estimate from the verb `CostEstimate`, and the Generate button.
- **Center.** An animated loop preview that plays the selected candidate at the target
  fps with a foot-baseline overlay and its own transport (play/pause/loop, fps). Pan and
  zoom reuse `ui/src/sheet/preview-zoom.ts` (`zoomToCursor`, space/middle-drag pan,
  double-click fit).
- **Candidate strip.** Reuses the `HistoryStrip` pattern
  (`ui/src/sheet/HistoryStrip.tsx`) but each thumbnail is an animated loop. Per-candidate
  quality flags (drift / scale / seam). Accept / reject / re-roll.
- **Request strip.** Reuses the sheet request-strip pattern from
  `ReferenceSheetEditor` and `sheet-editor-state.ts` (`activeRequests`,
  `SheetRequestProgress`): one row per in-flight direction/candidate, partial-image
  preview, elapsed time, cancel.
- **Timeline ribbon.** A compact strip of the entity's animations; clicking one selects
  it in the full timeline below.

## Generation controls

| Control | Values / behaviour |
|---|---|
| Type | Idle, Walk, Attack, Custom. Sets the prompt scaffold and the landing `LoopDirection` (idle -> PingPong, walk -> Forward, attack -> Forward once). |
| Direction | South, West, North as buttons. East is shown as a derived horizontal flip of West, not a generate target (directional economy). |
| Mode | Grid image-gen or Walk via i2v. Walk forces i2v and disables grid-only controls; the doc's pipeline marks i2v as the only reliable walk path. |
| Frames / grid size | Grid mode: g (2-6), reusing the existing slider and the `actions.length <= gridSize` chip constraint. i2v mode: target picked-frame count (8-12). |
| Choreography / prompt | Free text; prefilled from the anchor's stored prompt. Idle/attack expose the CHARACTER x CHOREOGRAPHY scaffold; attack exposes per-beat effect scripting. |
| Advanced | Seed, cell size, frame durations, layout-guide toggle, negative prompts (direction-locked, no pivots, no background, no particles), backend and quality. |

Defaults derive from the anchor: direction defaults to the anchor's direction, palette
and style come from the canonical variant, cell size from the sprite canvas. Controls
that do not apply to the current mode are hidden, not disabled, to keep the panel short.

## Streaming and progress

Animation generation reuses the reference-sheet streaming contract. The verb emits
progress events the studio renders as request rows (mirroring `SheetRequestProgress` /
`SheetRequestComplete` / `SheetRequestCancelled`):

- One row per direction and candidate, with a partial-image thumbnail as it streams,
  elapsed seconds, and a cancel button (existing `verbCancel` / request-cancel path).
- i2v runs surface cost and latency prominently because they are slower and pricier
  than image generation; the row shows a longer expected duration and a spend estimate.
- Failures mark the row as errored and raise a toast; other rows continue.

## Candidate review

```text
+--------------------------------------------------+
|                 [ candidate #2 ]                 |
|        > playing  loop  fps 10   [ compare ]     |
|                                                  |
|   frame 8 of 8     seam: frame8 vs frame1  ok    |
|   baseline drift: 0px      scale match: 100%     |
+--------------------------------------------------+
| [#1 ] [#2*] [#3 ]                                |
|        [ accept ]   [ re-roll ]   [ reject ]     |
+--------------------------------------------------+
```

Each candidate plays as a loop. A compare toggle shows two candidates side by side.
Quality indicators (baseline drift, scale match, loop-seam) read from the normalization
measurements; accept is allowed even with warnings, but warnings are explicit. Accept
lands the animation (below); reject deletes the candidate; re-roll regenerates with the
same inputs and a new seed.

## Walk-cycle frame picker (i2v)

When a walk is generated via i2v, the backend returns a clip; the studio opens a frame
picker (advances S32, reusing `motion_from_video` keyframe detection) to extract a clean
loop before review.

```text
+--------------------------------------------------------------+
| Pick walk loop - West                                        |
|  [ clip scrubber #################################### ]      |
|        ^marker A (neutral)            ^marker B (neutral)    |
|  picked: 10 frames    [ auto-detect ] [ +/- frames ]         |
+--------------------------------------------------------------+
|  loop seam:   [ frame A ] | [ frame B ]   match: close       |
|  [ o o o o o o o o o o ]  picked frames preview              |
|                              [ Cancel ]   [ Use these ]      |
+--------------------------------------------------------------+
```

- Auto-detect places markers A and B at recurring neutral-stance poses (both feet
  together); the user can drag them.
- The picker selects evenly spaced frames between the markers (8-12) and previews the
  loop seam (last picked vs first picked).
- "Use these" feeds the picked frames into candidate review and normalization.

## Normalization review

Before an animation integrates, the studio shows the normalization result so the user
sees baseline lock and scale correction (pipeline doc, seven-step pass).

```text
+--------------------------------------------------+
| Normalize - Walk / South                         |
|  contact sheet:  [1][2][3][4][5][6][7][8]        |
|  gif preview at 1x:  [ > looping ]               |
|                                                  |
|  baseline:  locked          scale: matched       |
|  warnings:  none                                  |
|             [ re-normalize ]   [ integrate ]      |
+--------------------------------------------------+
```

Contact sheet plus a runtime-scale GIF preview (reusing `io/src/animated`), with
drift / scale-jump / halo flags. Integrate commits the normalized frames to the timeline.

## Landing in the timeline

Accepting (and integrating) an animation creates, in one undoable step: a layer named
for the animation, one cel per frame at the right indices, and a `FrameTag` (plus the
engine-side `Animation`) over the range with the right `LoopDirection`. The timeline
selects the new tag and starts playback.

```text
| tags:   [ idle (pingpong) ] [ walk-s (forward) ] [ attack ]  |
| frames:  1  2  3  4  5  6  7  8  9 10 11 12 13 14 ...         |
| Hero    [#][#][#][#][#][#][#][#][#][#][#][#][#][#]            |
|  > play   loop   onion 2/2     dir: (pingpong v)  repeat: 0   |
```

This requires one new timeline control: `ui/src/timeline/FrameTagBar.tsx` does not
surface loop direction or repeat today. Add a loop-direction selector (forward /
reverse / pingpong / pingpong-reverse) and a repeat field (0 = loop forever) on the tag,
since generated animations set these and the user must see and edit them.

## Animation set and directional cascade

A per-entity view of the whole set, so the user sees coverage and staleness. Re-rolling
the anchor marks dependents stale (pipeline doc cascade).

```text
+--------------------------------------------------+
| Hero - animation set            anchor: approved |
|            South    West    North   East(flip)   |
|  Idle      [ ok ]   [draft] [ -- ]  [ ok ]        |
|  Walk      [ ok ]   [stale] [ -- ]  [ ok ]        |
|  Attack    [draft]  [ -- ]  [ -- ]  [draft]       |
|                                                  |
|  stale: anchor changed - [ re-roll dependents ]  |
+--------------------------------------------------+
```

Cells show status (missing / draft / approved / stale). Clicking a cell opens the studio
prefilled for that direction and type. A stale banner appears when the anchor changed
after a derived animation was made, with a re-roll-dependents action.

## Cross-cutting UX

- **Undo/redo.** Integrate is a single undo entry; the studio relies on the existing
  project undo stack the way generated layers do today.
- **Toasts.** Success, completion, and error toasts via the existing `pushToast`.
- **Keyboard.** Space toggles preview playback; arrow keys step picked frames in the
  frame picker; Escape backs out of the studio.
- **States.** Explicit empty (no animations yet), loading (verb list, anchor fetch),
  and error states; never a blank panel.
- **Accessibility.** Toggle controls use `aria-pressed` as `ui/src/layers/LayerRow.tsx`
  does; the preview transport is keyboard reachable.
- **Theming.** BEM classes in `ui/src/index.css` under an `animation-studio__` block;
  dark-theme CSS vars; lucide-solid icons.

## Component and file map

New (net-new):

- `ui/src/animation/AnimationStudio.tsx` — full-screen editor shell.
- `ui/src/animation/AnimationControls.tsx` — type / direction / mode / frames / advanced.
- `ui/src/animation/CandidateReview.tsx` — animated candidate playback + accept/reject.
- `ui/src/animation/FramePicker.tsx` — i2v loop frame picker.
- `ui/src/animation/NormalizationReview.tsx` — contact sheet + GIF check.
- `ui/src/animation/AnimationSet.tsx` — directional cascade grid.
- `ui/src/animation/animation-studio-state.ts` — signals + streaming event types.
- `ui/src/lib/commands/animation.ts` — IPC wrappers (below).

Reused (exists):

- `ui/src/sheet/{HistoryStrip.tsx,preview-zoom.ts}`, the request-strip pattern in
  `ui/src/sheet/{ReferenceSheetEditor.tsx,sheet-editor-state.ts}`.
- `ui/src/timeline/{TimelinePanel.tsx,FrameTagBar.tsx,timeline-state.ts}` (extend the
  tag bar with loop-direction / repeat controls).
- `ui/src/library/{LibraryPanel.tsx,library-state.ts}` (anchor badge + context item).
- `ui/src/lib/commands/verbs.ts`, `ui/src/lib/ai/verb-invoke-state.ts`,
  `ui/src/components/ModalForm.tsx`, `ui/src/command-palette/command-registry.ts`,
  `ui/src/shell/Shell.tsx`, `ui/src/lib/ipc.ts`.

IPC the UI needs (maps to the pipeline doc; some new):

- Animation verb invoke with streaming progress events that mirror the
  `SheetRequestProgress` / `Complete` / `Cancelled` shape, so the studio reuses the
  request-strip rendering.
- Frame-pick command (clip + markers + count -> picked frames), backing the i2v picker.
- Normalize command (frames -> normalized frames + measurements), backing the review.
- Animation-set / cascade query (entity -> per-direction, per-type status), backing the set view.

These are hand-rolled wrappers in `ui/src/lib/commands/animation.ts` until tauri-specta
generates bindings, matching the existing TODO in `ui/src/lib/commands/verbs.ts`.

## Open questions

- **Fold-in vs keep.** Retire the standalone `AnimatedSpriteSheetForm`, or keep it as a
  no-anchor quick path? This spec assumes fold-in.
- **Studio scope.** Studio only for entities with an anchor, or allow an anchorless
  quick-grid mode inside it? This spec assumes anchor-required, with the quick path as a
  possible relaxation.
- **Animation-set data home → Resolved: `CharacterAnchor`.** The set view reads
  per-direction, per-type status and staleness from the `CharacterAnchor` embedded on
  `ReferenceSheet` (see the pipeline doc's resolved decisions). The `animation_set`
  command computes each cell's status from `CharacterAnchor.derived_sheets` and the
  structural `derived_from` edges; `animation_reroll_dependents` lists the stale cells.

## Cross-references

- Engine: `docs/planning/work/animation-generation-pipeline.md`.
- Methodology and prior art: `docs/planning/research/{sprite-pipeline-methodology.md,falsprite-prior-art.md}`.
- Anchor mechanic: `docs/planning/work/bedrock.md` (B10), `docs/planning/work/b10-reference-sheets.md`.
- Streams: `docs/planning/work/streams.md` (S19, S25, S27, S32).
- Reused UI: `ui/src/sheet/`, `ui/src/timeline/`, `ui/src/library/`, `ui/src/shell/Shell.tsx`.
