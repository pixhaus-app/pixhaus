# CI gate hardening

Why the local gates let red CI through, and how they were aligned to mirror CI.

## The problem

Several PRs landed with failing CI. The most recent failing PR (#226,
`feat/s16-mask-outline-ellipse`) failed on exactly two checks, both formatting:

- Rust job: `cargo fmt --all -- --check` — `app/src/commands/canvas.rs` was left
  unformatted (line-wrap diffs rustfmt would have applied).
- UI job: `pnpm format:check` — a prettier violation in the UI workspace.

Neither is a logic bug. Both are gates the local hooks are supposed to keep
green but didn't, because of a structural hole.

## Root cause

1. Only the `.claude/settings.json` hooks are guaranteed to run for a Claude
   session. The git `pre-commit` hook fires only when `core.hooksPath=.githooks`
   is set, which happens during `pnpm bootstrap` (`scripts/bootstrap.sh`). A
   fresh clone or ralph worktree where bootstrap never ran commits with no
   pre-commit gate at all. `new-worktree.sh` did not wire hooks.
2. The post-edit hook formatted best-effort and swallowed errors:
   `cargo fmt --manifest-path Cargo.toml -- "$FILE_PATH" 2>/dev/null || true`,
   and the same `|| true` on the prettier write. The `-- <file>` path form is
   finicky with absolute paths and silently no-ops, so unformatted files slipped
   through. It also only handled `.ts/.tsx`, not `.css/.json`.
3. The conclaude `Stop` hook only ran `cargo nextest` + `pnpm test`. It never
   checked fmt, clippy, doc, typecheck, lint, format, ui:build, or deny. So a
   session could complete and commit with CI-breaking format/lint/doc state and
   nothing blocked it.
4. The pre-commit globs were narrower than CI: `src/**/*.{ts,tsx,css,json}` and
   `src/**/*.{ts,tsx}`, while CI runs `pnpm format:check` / `pnpm lint`, whose UI
   scripts cover `{src,tests}/**`. A formatting error in a `tests/` file passed
   pre-commit and failed CI — exactly #226's UI failure.

## What CI enforces (the contract to mirror)

From `.github/workflows/ci.yml` (per PR):

- Rust job: `cargo fmt --all -- --check`;
  `cargo clippy --workspace --all-targets -- -D warnings`;
  `cargo test --workspace --all-targets --no-fail-fast`;
  `cargo doc --workspace --no-deps --document-private-items` with
  `RUSTDOCFLAGS=-D warnings`. `RUSTFLAGS=-D warnings` is set workspace-wide.
- deny job: `cargo-deny check --config .cargo/deny.toml`.
- UI job: `pnpm typecheck`; `pnpm lint`; `pnpm format:check`; `pnpm test`;
  `pnpm ui:build`.

The `Visual Regression` workflow is gated on committed baselines and needs no
local mirror. The `Website` workflow's failure on `main` is a Cloudflare deploy
credential issue (missing/expired `CLOUDFLARE_API_TOKEN` /
`CLOUDFLARE_ACCOUNT_ID` secrets); it never gates a PR and is out of scope here.

## The fix

Principle: the gate that is guaranteed to run (conclaude `Stop`) must be a
faithful CI mirror, per-edit formatting must actually format, and the secondary
gates must match CI's scope.

1. `.conclaude.yaml` — `Stop` now runs the full CI-equivalent set, cheapest
   first so a format slip fails fast: `cargo fmt --check`, clippy `--all-targets`,
   `cargo test --all-targets`, `cargo doc`, `cargo deny`, `pnpm typecheck`,
   `pnpm lint`, `pnpm format:check`, `pnpm test`, `pnpm ui:build`. Uses
   `cargo test` (not nextest) to match CI's `--all-targets` set and doctests.
2. `scripts/post-edit.{sh,ps1}` — resolve the owning crate first, then
   `cargo fmt -p <crate>` (workspace config + edition, unlike the path form),
   surfacing failures instead of swallowing them. Prettier now also covers
   `.css/.json`, guarded to the `ui/` path so a root `package.json` is never
   reformatted with the UI config. The hook still returns 0 (never blocks edits)
   but echoes failures into the next turn.
3. `scripts/pre-commit.{sh,ps1}` — call `pnpm format:check` / `pnpm lint` (the
   exact scripts CI runs, covering `{src,tests}/**`) instead of hand-rolled
   `src/**` globs, and add a `cargo-deny` check gated on the binary.
4. `scripts/pre-pr.{sh,ps1}` — add a `cargo-deny` section so the manual pre-PR
   run is a complete CI mirror.
5. `scripts/new-worktree.{sh,ps1}` — set `core.hooksPath=.githooks` in the new
   worktree so the pre-commit gate fires even without `pnpm bootstrap`.

## Verification

- Inject a fmt violation in a `.rs` file and a prettier violation in a
  `ui/tests/*.ts` file; confirm `scripts/pre-commit.sh` now fails both (the
  tests/ one passed before).
- Confirm post-edit reports the fmt/prettier fix and prettifies `.css/.json`.
- `./scripts/pre-pr.sh` passes on a clean tree and includes the cargo-deny
  section.
- `new-worktree.sh <name>` then
  `git -C ../pixhaus-worktrees/<name> config --get core.hooksPath` prints
  `.githooks`.
- The PR carrying these changes has green CI.
