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
- [ ] UNCLAIMED: B3 — `.pixhaus` project file format. Brief: docs/planning/work/bedrock.md#b3-project-file-format-pixhaus. Depends on: B2.
- [ ] UNCLAIMED: B4 — IPC command catalog. Brief: docs/planning/work/bedrock.md#b4-ipc-command-catalog. Depends on: B2.
- [x] DONE: B5 — AI verb plugin protocol [OPUS-REQUIRED]. Brief: docs/planning/work/bedrock.md#b5-ai-verb-plugin-protocol. Depends on: B2. Highest leverage in project.
- [ ] UNCLAIMED: B6 — Unity handoff format. Brief: docs/planning/work/bedrock.md#b6-unity-handoff-format. Depends on: B2.
- [x] DONE: B7 — Aseprite format compatibility spec. Brief: docs/planning/work/bedrock.md#b7-aseprite-format-compatibility-spec. Parallel with B2 (no blocker).

## Bedrock — done (this scaffold)

- [x] DONE: B1 — Repo scaffold. Cargo workspace, pnpm workspace, Tauri 2 shell, Unity package skeleton, lints, hooks, CI.
- [x] DONE: B8 — Agent handbook (this scaffold). CONTRIBUTING.md, four pre-build skills, hook configuration, ralph loop infrastructure.

## Streams — queued, blocked on bedrock

The full list is in `docs/planning/work/streams.md`. Critical-path streams
listed here for visibility — the ralph loop should not claim them until
their bedrock dependencies are merged.

- [ ] UNCLAIMED: S01 — Pixel buffer and blend modes (★ critical path). Brief: docs/planning/work/streams.md#s01. Blocked by: B2.
- [ ] UNCLAIMED: S02 — Color and palette ops (★ critical path). Brief: docs/planning/work/streams.md#s02. Blocked by: B2.
- [ ] UNCLAIMED: S05 — Undo/redo command pattern (★ critical path). Brief: docs/planning/work/streams.md#s05. Blocked by: B2.
- [ ] UNCLAIMED: S06 — Tilemap data structures and autotile rules (★ critical path). Brief: docs/planning/work/streams.md#s06. Blocked by: B2.
- [ ] UNCLAIMED: S07 — `.pixhaus` native format (★ critical path). Brief: docs/planning/work/streams.md#s07. Blocked by: B3.
- [ ] UNCLAIMED: S08 — `.aseprite` read/write (★ critical path). Brief: docs/planning/work/streams.md#s08. Blocked by: B7.
- [ ] UNCLAIMED: S10 — PNG sprite sheet + JSON export (★ critical path). Brief: docs/planning/work/streams.md#s10. Blocked by: B2 B6.
- [ ] UNCLAIMED: S13 — Application shell and command palette (★ critical path). Brief: docs/planning/work/streams.md#s13. Blocked by: B4.
- [ ] UNCLAIMED: S14 — Canvas viewport (WebGL2) (★ critical path). Brief: docs/planning/work/streams.md#s14. Blocked by: B4 S01.
- [ ] UNCLAIMED: S21 — Verb runtime (★ critical path). Brief: docs/planning/work/streams.md#s21. Blocked by: B5.
- [ ] UNCLAIMED: S22 — Backend adapters (★ critical path). Brief: docs/planning/work/streams.md#s22. Blocked by: B5.
- [ ] UNCLAIMED: S39 — Unity importer package (★ critical path). Brief: docs/planning/work/streams.md#s39. Blocked by: B6.
- [ ] UNCLAIMED: S49 — CI/CD pipelines (★ critical path). Brief: docs/planning/work/streams.md#s49. Independent of bedrock; can start anytime.

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
