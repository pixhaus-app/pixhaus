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

## Manual prerequisites before launching

These need your hands or your auth — not Claude Code's.

```bash
# 1. Install dev tools
bash scripts/install-tools.sh
# (or PowerShell: pwsh scripts/install-tools.ps1)

# 2. Wire git hooks
bash scripts/setup-git-hooks.sh

# 3. Verify Rust toolchain matches rust-toolchain.toml
rustup show
# Should show 1.95 active. If not: rustup toolchain install 1.95 && rustup default 1.95

# 4. Sanity-check the build
cargo check --workspace --all-targets
cargo nextest run --workspace
pnpm install
pnpm typecheck
pnpm test
pnpm dev   # should open an empty Pixhaus window — Ctrl-C to close

# 5. Verify hooks fire
# Edit any .rs file, save it from inside a Claude Code session, watch
# scripts/post-edit.sh output cargo check results.
# Try a commit with bad formatting — pre-commit should reject it.

# 6. ANTHROPIC_API_KEY exported in the shell that will run ralph
echo $ANTHROPIC_API_KEY  # not empty

# 7. GitHub auth — gh CLI logged in for the ralph loop to open PRs
gh auth status
# If not: gh auth login
```

## First dispatch

Bedrock B2 (the core data model). Single agent, Opus, careful review. Everything else depends on this.

```bash
# Set up the worktree
bash scripts/new-worktree.sh stream-b2

# Open a fresh terminal in the worktree
cd ../pixhaus-worktrees/stream-b2

# Dispatch B2 with Opus directly (don't use the ralph loop for B2 — review it manually before B3-B7 fan out)
claude --model claude-opus-4-7 --print "$(bash ../../pixhaus/scripts/claim-next-task.sh stream-b2 | tail -n +2)"
```

Review the resulting PR carefully. B2 is the data model that every other stream consumes; mistakes propagate fast.

## After B2 merges

The bedrock fans out. Spin up four to six worktrees, run ralph in each, let Sonnet pick up B3, B4, B6, B7 in parallel. Keep B5 (verb plugin protocol) on Opus — it's the highest-leverage spec and warrants the careful run.

```bash
# In four separate terminals
bash scripts/new-worktree.sh stream-b3 && cd ../pixhaus-worktrees/stream-b3 && bash scripts/ralph.sh stream-b3
bash scripts/new-worktree.sh stream-b4 && cd ../pixhaus-worktrees/stream-b4 && bash scripts/ralph.sh stream-b4
bash scripts/new-worktree.sh stream-b6 && cd ../pixhaus-worktrees/stream-b6 && bash scripts/ralph.sh stream-b6
bash scripts/new-worktree.sh stream-b7 && cd ../pixhaus-worktrees/stream-b7 && bash scripts/ralph.sh stream-b7

# B5 separately with Opus
PIXHAUS_RALPH_MODEL=claude-opus-4-7 \
    bash scripts/new-worktree.sh stream-b5 && \
    cd ../pixhaus-worktrees/stream-b5 && \
    bash scripts/ralph.sh stream-b5
```

`tmuxinator` or `zellij` makes managing four-plus terminals less painful.

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
