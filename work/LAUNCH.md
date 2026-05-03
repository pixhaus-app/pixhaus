# Launch checklist

What to confirm before starting the ralph loop on B2. Verified state of the repo as of 2026-05-03.

## Verified ✓

- Cargo workspace with five crates (`core`, `io`, `ai`, `scripting`, `app`), Rust 2024 edition, MSRV 1.85, toolchain pinned to 1.95
- Tauri 2.11 shell with `app/src/main.rs` + `lib.rs::run()` + tracing setup + `tauri::generate_context!()` (with the documented Tauri-2.11 macro lint allowance)
- Workspace lints set at the workspace level: `unwrap_used = deny`, `expect_used = deny`, `panic = deny`, `pedantic = warn`
- pnpm 10 workspace with Vite + Solid 1.9 + TypeScript 6 + ESLint 10 + Prettier 3 + Vitest 4
- Cross-platform scripts: every script exists in both `.sh` and `.ps1`, dispatched through `scripts/run.mjs` (picks the right OS variant)
- `post-edit.sh` reads JSON from stdin (the actual PostToolUse hook payload), runs `cargo fmt` + `cargo check --tests -p <crate>` for Rust and `prettier --write` + `tsc --noEmit` for TS, exits 0 on errors so they surface to Claude's next turn rather than blocking the hook
- `claim-next-task.sh` uses mkdir-based atomic locking (works on Windows Git Bash)
- `ralph.sh` env-configurable (model, sleep, max iter), logs to `logs/ralph/<timestamp>.json`
- `pre-commit` blocks bad fmt / clippy / typos / prettier / eslint with helpful fix commands
- Four pre-build skills, all substantive: `pixhaus-rust-conventions` (594 lines), `pixhaus-tauri-patterns` (470), `pixhaus-testing-conventions` (487), `pixhaus-claude-code-workflow` (252) — frontmatter present, descriptions follow the "Use when..." trigger pattern
- CI: Rust job (fmt + clippy + test + doc with `-D warnings` everywhere), UI job (typecheck + lint + format:check + test + build), Tauri Linux deps installed, smart `ui/dist` stub for `generate_context!`, modern action versions
- `work/queue.md` populated: B2-B7 unclaimed, B5 flagged `[OPUS-REQUIRED]`, B1+B8 marked DONE, critical-path streams listed but properly blocked on bedrock
- `docs/planning/` snapshot of the full planning corpus
- Git remote correctly pointed at `git@github.com:pixhaus-app/pixhaus.git`
- 8 dependabot PRs already merged, CI green on `main`

## Fixed during verification

- `scripts/pre-pr.sh` and `pre-pr.ps1` created (referenced in `CLAUDE.md` but missing from initial scaffold). Runs the full local gate before opening a PR — clippy + nextest + doc + UI typecheck/lint/test/build.
- `rustfmt.toml` updated to `edition = "2024"` (was on 2021, mismatched the workspace).
- Launch automation added: `pnpm bootstrap` (idempotent setup), `pnpm run doctor` (env check), `pnpm dispatch <task>` (one-shot Claude run), `pnpm fan-out` (parallel ralph commands for B3–B7).
- `finalize-task` regex now tolerates the space after the colon that `claim-next-task` writes (`CLAIMED:<wt>: <id>`); the no-whitespace pattern never matched.

## Manual prerequisites before launching

These need your hands or your auth — not Claude Code's.

```bash
pnpm bootstrap     # idempotent: installs cargo + pnpm tools, wires hooks,
                   # fetches deps, builds the ui/dist stub, runs cargo check
                   # + pnpm typecheck + tests, ends with doctor.
pnpm run doctor    # PASS/WARN/FAIL report on toolchain, env vars, gh auth,
                   # ANTHROPIC_API_KEY, claude CLI, queue state, disk space.
                   # Use `run`: pnpm reserves `pnpm doctor` for itself.
```

`bootstrap` is safe to re-run. `doctor` is read-only. Both work on Windows
(PowerShell) and *nix.

The doctor report lists what to fix if anything's red. Pixhaus uses
Claude Code via subscription (Pro/Max), not direct API — the `claude`
CLI handles auth via its own OAuth login, so `ANTHROPIC_API_KEY` is
not required. Doctor reports it informationally either way.

Two things doctor can't fix automatically:

- `claude` not logged in — run `claude` once interactively to complete
  the subscription login flow.
- `gh auth status` — `gh auth login` (interactive); needed for the
  ralph loop to open PRs.

## First dispatch

Bedrock B2 (the core data model). Single agent, Opus, careful review. Everything else depends on this.

```bash
pnpm dispatch B2 --model claude-opus-4-7
```

`dispatch` claims B2, creates the worktree at `../pixhaus-worktrees/stream-b2`,
runs Claude once, finalizes ok/fail, and tees the transcript to
`logs/dispatch/`. One-shot — exits when Claude does.

Review the resulting PR carefully. B2 is the data model that every other
stream consumes; mistakes propagate fast.

## After B2 merges

The bedrock fans out. B3, B4, B6, B7 in parallel on Sonnet; B5 stays on
Opus (highest-leverage spec).

```bash
pnpm fan-out
```

Default mode prints one command block per terminal — copy/paste each into
its own shell. With `--background`, fan-out runs the ralph loops as
backgrounded jobs in the current shell instead (bash uses `nohup ... &`,
PowerShell uses `Start-Job`); print PIDs / job IDs and use those to stop.

`tmuxinator` or `zellij` makes managing the printed multi-terminal flow
less painful.

## Things to watch

- Review queue length. If you're more than 24 hours behind on PR review, throttle the ralph loops with `Ctrl-C`. Don't let the queue overflow.
- `logs/ralph/*.json` for what each agent actually did. Worth a daily skim.
- The first PR. Review it like a senior reviewer would — the patterns set in B2's PR define how the rest of the project looks. If the agent picked an unusual abstraction or naming, push back.
- The cost dashboard. Anthropic Console shows daily spend. If Opus runs cost more than expected, default-down to Sonnet for B3, B4, B6, B7 and only keep Opus on B5.
- CI durations. If Rust tests creep past 8 minutes, we have a problem (likely a slow proptest case). Profile early.

## When to stop and intervene

- A task bounces back to the queue 3 times → the brief is wrong, not the agent. Edit it.
- CI infra breaks → fix CI before more agents pile on top.
- A bedrock spec turns out to be wrong → every dependent stream is now suspect; pause dispatch until the spec is corrected.
- Your review backlog crosses 24 hours → slow down dispatch, not review.

## After the bedrock lands

Streams S01, S02, S05, S06, S07, S08, S10, S13, S14, S21, S22, S39, S49 unlock in parallel. Repeat the worktree pattern. Plan to run 4-6 in parallel; expect 2-4 PRs per day to review.

The path to internal v1 is roughly six to eight weeks of agent wall-clock from here, dominated by your review capacity rather than execution time.
