# Dev workflow — Claude Code + ralph loop + worktrees

The operational manual for running the Pixhaus build with Claude Code. Covers model strategy, the ralph loop, the worktree pattern for parallel agent work, and the hook configuration that keeps the codebase clean as agents type.

## The model strategy

Two models, one default. Don't overthink which to use.

| Use Opus 4.7 for | Use Sonnet 4.6 for |
|---|---|
| Writing bedrock specs (B1-B8) | Writing stream code |
| The verb plugin protocol (B5) — highest-leverage doc | Running the ralph loop |
| Code review on PRs before merge | Tight-loop iteration with hooks |
| Hard debugging (>20 min stuck) | Test scaffolding |
| Architectural disagreements | Fixing review feedback |
| Pre-merge audits on critical-path streams | Documentation drafting |

**Default to Sonnet. Escalate to Opus.** Sonnet 4.6 writes excellent Rust — better than most humans on idiomatic patterns. Opus is ~5x the cost. At 52 streams, the cost difference compounds. The sweet spot: Opus on the things you'd consult a senior engineer about; Sonnet on the things you'd hand to a senior IC.

Practical commands:

```bash
# Default: planning sessions with Opus
claude --model claude-opus-4-7

# Execution: ralph loop with Sonnet
claude --model claude-sonnet-4-6 --print "$(cat next-task.md)"
```

You can also configure model selection via `.claude/settings.json` per-project so the default is right without flags.

## The ralph loop

Pattern:

1. A task queue lives in version control as `work/queue.md` (or per-stream files).
2. A wrapper script picks the next unclaimed task, dispatches a Claude with a self-contained prompt, captures the output, marks the task done or returns it to the queue if it failed.
3. The script loops indefinitely. New tasks added to the queue get picked up next cycle.
4. Multiple ralph loops run in parallel via `git worktree`, each in its own checkout, each on its own branch.

Skeleton script (`scripts/ralph.sh`):

```bash
#!/usr/bin/env bash
set -euo pipefail

WORKTREE_NAME="${1:-default}"
WORKTREE_PATH="../pixhaus-worktrees/$WORKTREE_NAME"

while true; do
  # Pick next task — atomic claim by editing queue.md
  TASK=$(./scripts/claim-next-task.sh "$WORKTREE_NAME")
  if [ -z "$TASK" ]; then
    echo "No tasks. Sleeping 5 minutes."
    sleep 300
    continue
  fi

  # Run Claude with the task prompt
  cd "$WORKTREE_PATH"
  claude --model claude-sonnet-4-6 \
         --print "$(cat ../$TASK)" \
         --output-format json > "../logs/$(date -Iseconds)-$TASK.json"

  # Mark task done if Claude reported success and CI passes
  ./scripts/finalize-task.sh "$WORKTREE_NAME" "$TASK"
done
```

The actual claim/finalize scripts handle: locking the task in queue.md, branch creation, opening a PR when work is complete, marking the task done on green CI, returning it to the queue on failure.

**Key discipline:** the ralph loop never merges. It opens PRs. You merge after review. Without that gate, agent mistakes pile up faster than you catch them.

## Worktrees for parallel work

`git worktree add` creates a separate filesystem checkout pointed at a different branch. Multiple Claudes can work simultaneously without stepping on each other's files.

Setup:

```bash
# From the main repo directory
git worktree add ../pixhaus-worktrees/stream-s01 -b feat/s01-pixel-buffer
git worktree add ../pixhaus-worktrees/stream-s02 -b feat/s02-color-palette
git worktree add ../pixhaus-worktrees/stream-s07 -b feat/s07-pixhaus-format
# ...one per concurrent stream
```

Each worktree gets its own ralph loop. On Windows, swap `bash scripts/ralph.sh` for `node scripts/run.mjs ralph` (or `pwsh -File scripts/ralph.ps1`) — same flag, same behavior:

```bash
# Terminal 1
node scripts/run.mjs ralph stream-s01

# Terminal 2
node scripts/run.mjs ralph stream-s02

# etc.
```

Tools like `tmuxinator` or `zellij` make managing N parallel terminals tolerable. `tmux-resurrect` saves the session if you reboot.

**Limit on concurrency:** practical cap is 4-8 parallel ralph loops. Beyond that, your review queue overflows and the bottleneck moves from execution to your eyeballs. Start with 2-3 to calibrate review pace, scale up after you've shipped a few PRs.

**Worktree hygiene:** clean up dead worktrees. `git worktree prune` removes deleted ones. Keep a tracking file (`.worktrees.md`) listing live ones.

## The hook configuration

Claude Code runs hooks on tool events (defined in `.claude/settings.json`). For Rust, the high-leverage configuration:

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "node scripts/run.mjs post-edit"
          }
        ]
      }
    ]
  }
}
```

### Cross-platform dispatch

Every hook script in `scripts/` ships in two forms: `<name>.sh` for *nix and `<name>.ps1` for Windows PowerShell. A small Node dispatcher (`scripts/run.mjs`) picks the right one at invocation time using `process.platform`. Node is already required for the UI workspace, so this adds no runtime dependency.

The same pattern wires `.githooks/pre-commit` — that file is a tiny shim (`exec node "$(dirname "$0")/../scripts/run.mjs" pre-commit "$@"`) that git on any OS can execute. Git for Windows still launches it via Git Bash, but the dispatcher then picks `pre-commit.ps1` so the actual checks run in native PowerShell.

When you add a new hook script: write both `.sh` and `.ps1`, give them the same behavior, and invoke them through `node scripts/run.mjs <name>`.

`scripts/post-edit.sh` (the `.ps1` mirror lives next to it):

```bash
#!/usr/bin/env bash
# Run after Claude edits a file — keep code formatted, surface compile errors fast

CHANGED_FILE="${1:-}"
if [[ "$CHANGED_FILE" == *.rs ]]; then
  CRATE=$(scripts/find-crate-for-file.sh "$CHANGED_FILE")

  # Format always (instant)
  cargo fmt -- "$CHANGED_FILE"

  # Type-check the affected crate (1-3 seconds with incremental cache)
  cargo check --tests -p "$CRATE" 2>&1
fi

if [[ "$CHANGED_FILE" == *.ts || "$CHANGED_FILE" == *.tsx ]]; then
  pnpm prettier --write "$CHANGED_FILE"
  pnpm tsc --noEmit -p ui/tsconfig.json 2>&1
fi
```

Output from the hook gets fed back into Claude's context for the next turn. If `cargo check` reports an error, Claude sees it and fixes before moving on.

**Why not run `cargo build`?** Build does codegen, which is what makes Rust slow. `cargo check` only type-checks. For a single crate with incremental cache, second-run check times are sub-second. Build is for CI.

**Why not run `cargo clippy` post-edit?** Clippy is 3-10 seconds per crate. Acceptable but slows the loop. Run it pre-commit instead, where the human accepts the wait.

**Why not run `cargo test` post-edit?** Tests can be 30 seconds to several minutes. Wrong granularity for an edit hook. Run pre-PR and in CI.

### Hook tiers

| Tier | When | Commands | Goal |
|---|---|---|---|
| Post-edit | Claude saves a file | `cargo fmt` on file, `cargo check --tests -p <crate>`, `prettier` for TS, `tsc --noEmit` for TS | Errors caught immediately, fed back to Claude |
| Pre-commit | git commit | `cargo fmt --check`, `cargo clippy -- -D warnings`, `typos`, `prettier --check`, `eslint` | Gate the commit |
| Pre-PR | PR open | `cargo nextest run`, `cargo doc --no-deps`, `pnpm test`, `pnpm build` | Gate the PR |
| CI | GitHub Actions | All of the above plus `cargo deny check`, `cargo audit`, `cargo machete`, visual regression suite | Gate the merge |

### Background watchers (for the human, not Claude)

Run in a tmux pane while you work:

```bash
# Terminal pane 1: bacon for continuous Rust check + clippy
bacon

# Terminal pane 2: nextest watcher
cargo watch -x 'nextest run --no-fail-fast'

# Terminal pane 3: TS type checker watcher
pnpm tsc --noEmit -p ui/tsconfig.json --watch
```

`bacon` is purpose-built for Rust dev — better than `cargo-watch` for the always-on case. Job switching shows check, clippy, test, doc views without restarting.

## Branch and PR strategy

One branch per stream. PR opened when the stream's work is complete and CI is green. PRs reviewed by you (or Opus 4.7 acting as a reviewer if you want the second opinion). Merged on approval.

Branch naming follows Conventional Commits prefixes:

- `feat/sNN-<slug>` for new feature streams (e.g., `feat/s07-pixhaus-format`)
- `fix/<issue-number>-<slug>` for bug fixes
- `chore/<slug>` for housekeeping
- `docs/<slug>` for docs-only PRs

PRs include:

- Reference to the stream from `streams.md`
- What changed
- Test plan (which streams' tests pass, what's still untested)
- Screenshots for UI changes
- Open questions for the reviewer

The PR template (`.github/pull_request_template.md`) auto-prompts these.

**Don't stack PRs by default.** Each stream's PR targets `main`. If two streams genuinely depend on each other (rare with the bedrock-first architecture), use a feature branch with stacked PRs and graphite-style tools — but the ecosystem is designed to make this rare.

## Review cadence

Review every PR before merge. The reviewer model can be Opus 4.7 if you want a first pass, but you click merge.

Practical cadence with 4 parallel ralph loops:

- Morning standup with yourself: pull queue, see what's live, set priorities
- Mid-morning: review the previous day's PRs
- Afternoon: dispatch new tasks, supervise
- End of day: merge approved PRs, clear out dead worktrees

The bottleneck is review, not execution. With 4 loops, expect to review 2-4 PRs per day. With 8 loops, expect 4-8 PRs per day. Beyond that, review quality drops and bugs slip through.

## When to stop the ralph loop

Kill the ralph loop and intervene manually when:

- A stream has bounced back to the queue 3 times — there's a spec ambiguity to resolve, not a Claude problem
- CI infrastructure breaks — fix CI before more agents pile on
- A bedrock spec turns out to be wrong — every dependent stream is now suspect
- Your review backlog crosses 24 hours — slow down dispatch

## Cost ceiling

A 52-stream build with Sonnet at execution + Opus at planning + review will cost real money. Budget:

- Sonnet at $3/M input, $15/M output, ~50K tokens per stream PR (full context + iterations + review feedback) = ~$0.50-1.00 per stream
- Opus at $15/M input, $75/M output, ~30K tokens per spec/review session = ~$1.50-3.00 per session
- Across the project: ~$50-200 in Sonnet for 52 streams, ~$100-300 in Opus for planning + reviews
- Total: $150-500 to ship a v1 internal build

That's substantially under the $5K/month / $50K/year cost of a single junior engineer doing the same work over a year. Cost is not the gating factor; review capacity is.

## What goes wrong, and what to do

| Failure mode | Symptom | Fix |
|---|---|---|
| Two ralph loops claim the same task | Both branches modify the same files | Atomic locking in `claim-next-task.sh` — use `flock` or rename pattern |
| Claude commits broken code | CI fails on PR | Don't merge; loop sends task back to queue with the failure as context |
| Worktree disk pressure | Disk full | `git worktree prune`; reuse worktrees instead of creating new ones |
| Ralph loop infinite-fails on impossible task | Same task picked, same failure 5x | Mark task as needs-human, alert |
| Claude generates `unwrap().unwrap()` cascades | Code review flags every PR for it | Add to `pixhaus-rust-conventions` skill explicitly; clippy lint set in `clippy.toml` |
| Compile times balloon | Hooks take 10+ seconds | Per-crate `cargo check`, not workspace-wide; profile with `cargo build --timings` |
| Test suite gets flaky | CI red on retries | Mark flaky tests, fix or quarantine; never accept "retry CI" as a workflow |

## Pre-flight checklist before launching the build

Before B1 runs:

- [ ] pixhaus.app domain pointed at a placeholder
- [ ] `github.com/pixhaus` org created
- [ ] `npm` package `pixhaus` reserved with stub README
- [ ] Local Claude Code installed and configured with API key
- [ ] Rust toolchain installed (1.82+, stable channel)
- [ ] pnpm installed
- [ ] `bacon` installed for background watching
- [ ] `cargo-nextest`, `cargo-deny`, `cargo-audit`, `cargo-machete`, `cargo-watch`, `typos` installed
- [ ] `.claude/settings.json` configured with hooks
- [ ] `scripts/ralph.{sh,ps1}` and helper scripts (with their `.ps1` siblings) in place; `scripts/run.mjs` dispatcher reachable via `node`
- [ ] `work/queue.md` populated with the first batch of tasks (B1, B8 to start)
- [ ] First worktree set up (or template documented)
- [ ] Personal review cadence agreed with yourself

That's the minimum viable launchpad. Once it's all in place, run `node scripts/run.mjs ralph main` and watch B1 land.
