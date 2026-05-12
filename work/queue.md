# Pixhaus task queue

Authoritative list of work the ralph loop and human contributors can claim.
Edited atomically by `scripts/claim-next-task.sh` (`mkdir`-based lock at
`.queue.lock`).

## Status markers

- `- [ ] UNCLAIMED: <ID> ...` — available
- `- [~] CLAIMED:<worktree>:<ID> ...` — in progress in named worktree
- `- [x] DONE: <ID> ...` — merged
- Trailing `[FAIL: <reason>]` — bounced back from finalize, needs attention

Pick the first `UNCLAIMED` line top-to-bottom. Bedrock B2 must land before
the rest of bedrock fans out (every B3–B7 brief depends on the data model).
B5 is flagged as Opus-required — it is the highest-leverage spec in the
project and warrants the higher-cost model.

## Bedrock — open

- [x] DONE: B2 — Core data model (Rust + TS). Brief: docs/planning/work/bedrock.md#b2-core-data-model. Priority: highest, single agent. Blocks: B3 B4 B5 B6.
- [x] DONE: B3 — `.pixhaus` project file format. Brief: docs/planning/work/bedrock.md#b3-project-file-format-pixhaus. Depends on: B2.
- [x] DONE: B4 — IPC command catalog. Brief: docs/planning/work/bedrock.md#b4-ipc-command-catalog. Depends on: B2.
- [x] DONE: B5 — AI verb plugin protocol [OPUS-REQUIRED]. Brief: docs/planning/work/bedrock.md#b5-ai-verb-plugin-protocol. Depends on: B2. Highest leverage in project.
- [x] DONE: B6 — Unity handoff format. Brief: docs/planning/work/bedrock.md#b6-unity-handoff-format. Depends on: B2.
- [x] DONE: B7 — Aseprite format compatibility spec. Brief: docs/planning/work/bedrock.md#b7-aseprite-format-compatibility-spec. Parallel with B2 (no blocker).

## Bedrock — done (this scaffold)

- [x] DONE: B1 — Repo scaffold. Cargo workspace, pnpm workspace, Tauri 2 shell, Unity package skeleton, lints, hooks, CI.
- [x] DONE: B8 — Agent handbook (this scaffold). CONTRIBUTING.md, four pre-build skills, hook configuration, ralph loop infrastructure.

## Streams — queued, blocked on bedrock

The full list is in `docs/planning/work/streams.md`. Critical-path streams
listed here for visibility — the ralph loop should not claim them until
their bedrock dependencies are merged.

- [x] DONE: S01 — Pixel buffer and blend modes (★ critical path). Brief: docs/planning/work/streams.md#s01. Blocked by: B2.
- [x] DONE: S02 — Color and palette ops (★ critical path). Brief: docs/planning/work/streams.md#s02. Blocked by: B2.
- [x] DONE: S05 — Undo/redo command pattern (★ critical path). Brief: docs/planning/work/streams.md#s05. Blocked by: B2.
- [x] DONE: S06 — Tilemap data structures and autotile rules (★ critical path). Brief: docs/planning/work/streams.md#s06. Blocked by: B2.
- [x] DONE: S07 — `.pixhaus` native format (★ critical path). Brief: docs/planning/work/streams.md#s07. Blocked by: B3.
- [x] DONE: S08 — `.aseprite` read/write (★ critical path). Brief: docs/planning/work/streams.md#s08. Blocked by: B7.
- [x] DONE: S10 — PNG sprite sheet + JSON export (★ critical path). Brief: docs/planning/work/streams.md#s10. Blocked by: B2 B6.
- [x] DONE: S13 — Application shell and command palette (★ critical path). Brief: docs/planning/work/streams.md#s13. Blocked by: B4.
- [x] DONE: S14 — Canvas viewport (WebGL2) (★ critical path). Brief: docs/planning/work/streams.md#s14. Blocked by: B4 S01.
- [x] DONE: S21 — Verb runtime (★ critical path). Brief: docs/planning/work/streams.md#s21. Blocked by: B5.
- [x] DONE: S22 — Backend adapters (★ critical path). Brief: docs/planning/work/streams.md#s22. Blocked by: B5.
- [x] DONE: S39 — Unity importer package (★ critical path). Brief: docs/planning/work/streams.md#s39. Blocked by: B6.
- [x] DONE: S49 — CI/CD pipelines (★ critical path). Brief: docs/planning/work/streams.md#s49. Independent of bedrock; can start anytime.

## Streams — second wave (post-critical-path, parallel-safe)

- [x] DONE: S03 — Selection algorithms. Brief: docs/planning/work/streams.md#s03. Blocked by: B2 S01.
- [x] DONE: S09 — `.psd` import. Brief: docs/planning/work/streams.md#s09. Blocked by: B2 S01.
- [x] DONE: S11 — Animated GIF + WebP export. Brief: docs/planning/work/streams.md#s11. Blocked by: S01 S02.
- [x] DONE: S12 — TMX tilemap export. Brief: docs/planning/work/streams.md#s12. Blocked by: B6 S06 S10.
- [x] DONE: S17 — Layer panel. Brief: docs/planning/work/streams.md#s17. Blocked by: S05 S13.
- [x] DONE: S18 — Color and palette panel. Brief: docs/planning/work/streams.md#s18. Blocked by: S02.
- [x] DONE: S20 — Tilemap UI. Brief: docs/planning/work/streams.md#s20. Blocked by: S06 S13 S14.
- [x] DONE: S41 — User documentation site. Brief: docs/planning/work/streams.md#s41. Stub now, fills as features land.
- [x] DONE: S45 — Sample projects and fixtures. Brief: docs/planning/work/streams.md#s45.
- [x] DONE: S46 — Logo, visual identity, design tokens. Brief: docs/planning/work/streams.md#s46.
- [x] DONE: S52 — Visual regression test harness. Brief: docs/planning/work/streams.md#s52. Blocked by: B1 S14.

## Streams — third wave (review follow-ups)

Each `*-followup` task carries the gaps the review pass deliberately
deferred from its parent stream's PR. Bodies are short — the parent PR
description and the linked review comments are the source of truth.

- [x] DONE: S09-followup — apply raster layer masks during PSD import; build a real-world fixture corpus (Photoshop CC, Affinity Photo); wire S13 file-open dialog to accept `.psd`. Parent: PR #43. Shipped: PR #50.
- [x] DONE: S11-followup — round-trip decode tests for GIF, WebP, MP4 using `image-rs` plus an external decoder gate. Parent: PR #40. Shipped: PR #51.
- [x] DONE: S12-followup — accept multiple tilesets in `export_tilemap()` (TMX `firstgid` math + `TiledLayerInput` carries a tileset reference); add an XSD schema validation pass against Tiled's spec; add an automated round-trip test against the S39 importer. Parent: PR #38. Shipped: PR #53.
- [x] DONE: S17-followup — variable-height virtualization so the active row's 56px height stops conflicting with the 32px assumption; implement merge-down / merge-selected / flatten-visible / convert-to-group / convert-to-tilemap-layer context-menu items; drag-into-group reparenting; commit-rename-on-unmount; fix the test-count-off-by-one. Parent: PR #35. Shipped: PR #60.
- [x] DONE: S18-followup — wire palette reorder through a real IPC command (currently visual-only with a console.warn); add `.aco` (Photoshop) format support to PaletteIOMenu. Parent: PR #41. Shipped: PR #59.
- [x] DONE: S20-followup — unit tests for AutotileRuleEditor; allow TileIndex(0) in the custom-rule editor + default-tile input + selectedTileIndex defaults; full tilemap CRUD UI in TilemapPanel; expose `tilesets` from `lib/commands/index.ts`. Parent: PR #37. Shipped: PR #52.
- [x] DONE: S41-followup — write the Aseprite-compat preset README the keybinds doc references. Parent: PR #39. Shipped: PR #57.
- [x] DONE: S45-followup — preserve per-tile metadata (animation, collision shapes) when inlining the forest tileset into the level sample. Parent: PR #42. Shipped: PR #56.
- [x] DONE: S52-followup — replace the `null`-default Tauri mock with realistic per-command responses so canvas tests actually render content; commit the seed baseline PNGs (must run on Linux to match CI's Chromium AA). Parent: PR #36. Shipped: PR #61.

## Streams — fourth wave (cross-cutting prep + UX gaps)

Items the third-wave review surfaced that did not fit any single stream's scope.

- [x] DONE: S15-prep — add a pixel-buffer cache to `app::state::DocumentStore` so loaded sprites retain their pixels across the `decode_from_file` round-trip. Surfaced by Copilot review of PR #49. Touches `app/src/state.rs` and the `project_open`/`project_save`/`project_import_psd` commands. Shipped: PR #63.
- [x] DONE: ui-toast — replaced `window.alert()` in `reportCommandFailure` with a non-blocking toast host (`ui/src/lib/toast/`). ToastHost is mounted once in Shell; any catch block calls `pushToast()`. Auto-dismiss after 6s; manual close button.

## Streams — fifth wave (real editor surface)

Brush, selection/transform, timeline, raster ops. Critical-path
infra (S01/S05/S13/S14) is done; these turn the project from
"viewer" into "editor". Fanned out together — they touch different
panels and rarely overlap.

- [x] DONE: S04 — Transform operations (flip, rotate, scale, free-transform commit). Brief: docs/planning/work/streams.md#s04. Blocked by: S03 S05. Shipped: PR #90.
- [x] DONE: S15 — Brush engine UI (pencil, eraser, fill, line, rectangle, ellipse + per-tool settings). Brief: docs/planning/work/streams.md#s15. Blocked by: S01 S14. Shipped: PR #89.
- [x] DONE: S16 — Selection and transform UI (marquee/lasso/wand handlers, transform handles). Brief: docs/planning/work/streams.md#s16. Blocked by: S03 S14. Shipped: PR #95.
- [x] DONE: S19 — Timeline panel (frame strip, play/pause, onion skin toggle, frame tags). Brief: docs/planning/work/streams.md#s19. Blocked by: S05 S13. Shipped: PR #79.

## Streams — AI verbs (B5/S21/S22 unblocked)

Each verb is its own stream under `ai/src/verbs/<name>/`. Briefs in
docs/planning/work/streams.md#s23-s36. Independent of each other
except for the shared `ai/src/verbs/mod.rs` registration — expect
small merge conflicts there as PRs land.

- [x] DONE: S23 — Verb: Inbetween (interpolated key-frame fills). Brief: docs/planning/work/streams.md#s23. Blocked by: B5 S21 S22. Shipped: PR #74.
- [x] DONE: S24 — Verb: Continue (predict next frames). Brief: docs/planning/work/streams.md#s24. Blocked by: B5 S21 S22. Shipped: PR #87.
- [x] DONE: S25 — Verb: Extend (multi-direction views). Brief: docs/planning/work/streams.md#s25. Blocked by: B5 S21 S22. Shipped: PR #85.
- [x] DONE: S26 — Verb: Variant (palette swaps, equipment, expressions). Brief: docs/planning/work/streams.md#s26. Blocked by: B5 S21 S22. Shipped: PR #70.
- [x] DONE: S27 — Verb: Cleanup (palette snap, AA removal, pivot fix). Brief: docs/planning/work/streams.md#s27. Blocked by: B5 S21 S22. Shipped: PR #78.
- [x] DONE: S28 — Verb: Tile (autotile generation). Brief: docs/planning/work/streams.md#s28. Blocked by: B5 S21 S22 S06. Shipped: PR #77.
- [x] DONE: S29 — Verb: Critique (VLM visual/quality analysis). Brief: docs/planning/work/streams.md#s29. Blocked by: B5 S21 S22. Shipped: PR #88.
- [x] DONE: S30 — Verb: Repaint (style transfer at fixed geometry). Brief: docs/planning/work/streams.md#s30. Blocked by: B5 S21 S22. Shipped: PR #83.
- [x] DONE: S31 — Verb: Reference (image-to-sprite from photo). Brief: docs/planning/work/streams.md#s31. Blocked by: B5 S21 S22. Shipped: PR #69.
- [x] DONE: S32 — Verb: Style (style match across sprites). Brief: docs/planning/work/streams.md#s32. Blocked by: B5 S21 S22. Shipped: PR #76.
- [x] DONE: S33 — Verb: Backgrounds (procedural backgrounds). Brief: docs/planning/work/streams.md#s33. Blocked by: B5 S21 S22. Shipped: PR #73.
- [x] DONE: S34 — Verb: Audio-driven timing (beat detection + lip sync). Brief: docs/planning/work/streams.md#s34. Blocked by: B5 S21 S22. Shipped: PR #75.
- [x] DONE: S35 — Verb: Mesh-deform (auto-rig + deformation). Brief: docs/planning/work/streams.md#s35. Blocked by: B5 S21 S22. Shipped: PR #80.
- [x] DONE: S36 — Verb: Promote-to-3D (sprite to volumetric). Brief: docs/planning/work/streams.md#s36. Blocked by: B5 S21 S22. Shipped: PR #84.

## Streams — extension surfaces, packaging, content

- [x] DONE: S37 — Plugin loader and public API surface (extism + Lua entry points). Brief: docs/planning/work/streams.md#s37. Blocked by: B5. Shipped: PR #96.
- [x] DONE: S38 — Lua scripting bindings (mlua project/sprite/layer/cel APIs). Brief: docs/planning/work/streams.md#s38. Blocked by: B5 S37. Shipped: PR #81.
- [x] DONE: S40 — Unity sample project demonstrating importer round-trip. Brief: docs/planning/work/streams.md#s40. Blocked by: S39. Shipped: PR #86.
- [x] DONE: S42 — Migration guide from Aseprite (docs page + workflow translations). Brief: docs/planning/work/streams.md#s42. Shipped: PR #66.
- [x] DONE: S43 — Plugin developer guide (SDK quickstart + verb authoring). Brief: docs/planning/work/streams.md#s43. Shipped: PR #67.
- [x] DONE: S44 — Tutorial content (5-10 walkthrough docs). Brief: docs/planning/work/streams.md#s44. Shipped: PR #68.
- [x] DONE: S47 — Website (pixhaus.app landing). Brief: docs/planning/work/streams.md#s47. Shipped: PR #82.
- [ ] UNCLAIMED: S48 — Discord and community setup. Brief: docs/planning/work/streams.md#s48. Deferred: not needed pre-launch.
- [x] DONE: S50 — Release packaging (installer/dmg/AppImage + auto-update). Brief: docs/planning/work/streams.md#s50. Shipped: PR #72.
- [x] DONE: S51 — Crash reporting (opt-in Sentry). Brief: docs/planning/work/streams.md#s51. Shipped: PR #71.

## Library and anchor sheets (B9 + B10)

The next major bedrock wave. B9 turns the flat `Vec<Sprite>` into a typed
library of named entities (Custom / Tileset / Tilemap / Reference) with
groups, tags, and AI metadata. B10 builds the AI-generated reference-sheet
system on top — the anchor mechanic that makes every subsequent generation
visually consistent for an entity. Pre-launch breaking change; no migration
path. Design: docs/planning/work/b9-project-library.md and
docs/planning/work/b10-reference-sheets.md.

- [x] DONE: B9.1 — Library data model + fixture rebuild. Brief: docs/planning/work/b9.1-dispatch-brief.md. Blocks: B9.2 B9.3 B9.4 B9.5 B10.*. [OPUS-RECOMMENDED] Shipped: PR #135.
- [x] DONE: B9.2 — IPC commands for library operations. Brief: docs/planning/work/b9-project-library.md#implementation-outline (B9.2). Blocked by: B9.1. Shipped: PR #159.
- [x] DONE: B9.3 — Library panel UI in Solid. Brief: docs/planning/work/b9-project-library.md#implementation-outline (B9.3). Blocked by: B9.1 B9.2. Shipped: PR #166.
- [x] DONE: B9.4 — AI library hooks (auto-tag, cross-entity transfer, project-LoRA wiring). Brief: docs/planning/work/b9-project-library.md#implementation-outline (B9.4). Blocked by: B9.1 B9.2. Shipped: PR #169.
- [x] DONE: B9.5 — Aseprite round-trip for stated entities. Brief: docs/planning/work/b9-project-library.md#implementation-outline (B9.5). Blocked by: B9.1. Shipped: PR #161.
- [x] DONE: B10.1 — `generate-reference-sheet` verb + four composition templates. Brief: docs/planning/work/b10-reference-sheets.md#b101. Blocked by: B9.1. Shipped: PR #160.
- [x] DONE: B10.2 — `iterate-reference-sheet` verb (panel-scoped inpainting). Brief: docs/planning/work/b10-reference-sheets.md#b102. Blocked by: B10.1. Shipped: PR #165.
- [x] DONE: B10.3 — Approval flow + anchor wiring across the 14 existing AI verbs. Brief: docs/planning/work/b10-reference-sheets.md#b103. Blocked by: B10.1. [OPUS-RECOMMENDED] Shipped: PR #168.
- [x] DONE: B10.4 — Sheet UI panel. Brief: docs/planning/work/b10-reference-sheets.md#b104. Blocked by: B10.1. Shipped: PR #167.
- [x] DONE: B10.5 — Per-entity LoRA training (optional; defer if anchor-without-LoRA quality is acceptable). Brief: docs/planning/work/b10-reference-sheets.md#b105. Blocked by: B10.3. Shipped: PR #179.

## Operating notes

- One claim per worktree; the loop stops if the lock dir cannot be acquired
  in 30 seconds.
- Mark a task DONE only after the PR is merged and CI is green on `main`.
  `finalize-task.sh ok` does this; do not edit DONE markers by hand.
- A FAIL marker means the task came back from finalize. Check
  `logs/ralph/*.json` for the agent transcript, decide whether to fix the
  brief or re-claim, then move it back to UNCLAIMED.
- For tasks not in this file (an ad-hoc bug fix, a docs update), open a PR
  directly without queue routing. The queue is for parallelizable streams.
