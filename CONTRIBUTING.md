# Contributing to Pixhaus

This document covers how contributions land — branches, commits, PRs, reviews —
and the two-tier philosophy that governs both human and agent contributions.

Code of conduct: see [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).

## The two-tier contribution model

Pixhaus accepts contributions from humans and from AI agents. Both follow the
same code review bar; the difference is where the work originates and where
the human accountability sits.

- **Tier 1 — humans.** Direct contributors open PRs, request review, and merge
  on approval.
- **Tier 2 — agents.** AI agents (Claude Code, Codex, others) write code under
  the supervision of a human maintainer who is responsible for the resulting
  PR. Agents follow the same conventions plus the additional discipline laid
  out in [`docs/planning/work/dev-workflow.md`](docs/planning/work/dev-workflow.md)
  and the four pre-build skills under [`.claude/skills/`](.claude/skills/).

Both tiers ship through the same PR pipeline, the same CI, and the same
review gate. The human who opens a PR — whether they wrote the code or an
agent did — owns it.

## What to work on

The active task queue lives at [`work/queue.md`](work/queue.md). It indexes
the bedrock specs (B1–B8) and the work streams (S01–S52) defined in
[`docs/planning/work/`](docs/planning/work/). The streams are designed to fan
out in parallel once the bedrock contracts are locked.

Before claiming a task:

- Read the brief in [`docs/planning/work/bedrock.md`](docs/planning/work/bedrock.md)
  or [`docs/planning/work/streams.md`](docs/planning/work/streams.md).
- Confirm its dependencies are satisfied (the queue lists blockers).
- Mark it claimed in `work/queue.md` so two contributors don't double-up.

For unscoped work (a bug, a small enhancement, a refactor) — open an issue
first and link it from the PR.

## Branch naming

One branch per stream or task:

- `feat/sNN-<slug>` — new feature streams (e.g., `feat/s07-pixhaus-format`)
- `feat/bNN-<slug>` — bedrock specs (e.g., `feat/b3-project-file-format`)
- `fix/<issue-number>-<slug>` — bug fixes
- `chore/<slug>` — housekeeping (deps, build, infra)
- `docs/<slug>` — docs-only changes

Keep branches focused. One stream per branch, one logical change per PR.

## Commit format

[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/):

```
<type>(<scope>): <subject>

[optional body explaining why, not what]

[optional footer with refs]
```

Types: `feat`, `fix`, `chore`, `docs`, `refactor`, `perf`, `test`, `build`, `ci`.

Examples:

```
feat(io): add zstd compression for pixel buffer payloads in .pixhaus
fix(core): clamp brush coordinates before lookup
docs(skills): clarify the lock-across-await footgun
chore(deps): bump tauri to 2.5.1
```

Subject line under 72 characters. Imperative mood. No trailing period.

## Pull requests

PRs target `main`. Each PR includes:

- A reference to the stream or bedrock spec it implements
- What changed and why (the body of `.github/PULL_REQUEST_TEMPLATE.md`)
- A test plan — what runs, what passes, what's known not to be covered
- Screenshots or short clips for any UI change
- Open questions you want the reviewer to weigh in on

Don't stack PRs by default. The bedrock-first architecture is designed to
make stacking rare. If two streams genuinely depend on each other, use a
feature branch with stacked PRs and call it out in the description.

## Code review

Every PR gets reviewed before merge. The review bar:

- **Correctness first.** Does the code do what the spec says? Are the edge
  cases covered? Are the failure modes explicit?
- **Conventions next.** Does it follow the patterns in the four pre-build
  skills under `.claude/skills/`? Does it pass clippy with `-D warnings`?
  Does it pass `cargo fmt --check`?
- **Tests then.** Every public function has at least one test. Integration
  tests live in `tests/`. Visual regression baselines live alongside the
  feature they exercise.
- **Performance last** — only when the PR description says so. Optimize with
  measurements, not vibes.

A review resolves to one of: approve, request changes, comment-only. If a
reviewer requests changes and the change is contested, the contributor and
reviewer hash it out in the PR thread; if they can't agree, escalate to a
maintainer for a tie-breaker.

## CI

Every PR runs:

- `cargo fmt --check`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`
- `pnpm typecheck`
- `pnpm lint`
- `pnpm test`
- `pnpm build`

CI must be green before merge. If CI is failing on `main` due to infra,
coordinate with a maintainer before opening more PRs against the broken
state.

## Coding conventions

### Rust

- Style follows `rustfmt` defaults plus the project `rustfmt.toml`.
- Lints: `clippy::pedantic` on warn, with `unwrap_used`, `expect_used`,
  `panic`, `print_stdout`, `print_stderr` denied in non-test code.
- Errors: `thiserror` in library crates (`core`, `io`, `ai`, `scripting`),
  `anyhow` in the application crate (`app`).
- No `unwrap()` or `expect()` outside tests. Use `?`, `ok_or`, `context()`.
- Async: `tokio` runtime; never hold a `std::sync::Mutex` across `.await`
  (use `tokio::sync::Mutex` or release the lock before awaiting).
- One owner per piece of mutable state. `Arc<Mutex<T>>` is a serialization
  point, not a concurrency primitive.

Full set: [`.claude/skills/pixhaus-rust-conventions/SKILL.md`](.claude/skills/pixhaus-rust-conventions/SKILL.md).

### TypeScript

- Style follows Prettier with the config in `ui/.prettierrc.json`.
- Lints: ESLint with `@typescript-eslint` strict + `eslint-plugin-solid`.
  No `any`. No unused locals. Strict null checks on.
- Solid.js idioms: signals over component state, stores for reactive
  graphs, no React-style hooks.

### Tests

Conventions: [`.claude/skills/pixhaus-testing-conventions/SKILL.md`](.claude/skills/pixhaus-testing-conventions/SKILL.md).

- Unit tests: inline `#[cfg(test)] mod tests` for Rust, colocated `*.test.ts`
  for TypeScript.
- Integration tests: `<crate>/tests/` for Rust, `ui/tests/` for TypeScript.
- Snapshots: `insta` for Rust, Vitest snapshots for TypeScript. Update
  intentionally with `cargo insta review`.
- Visual regression: `image-compare` for Rust-rendered output. Baselines
  live next to the tests under `tests/snapshots/`.

## Local hooks

The repo ships pre-commit and post-edit hooks. Install once:

```bash
bash scripts/setup-git-hooks.sh
```

The post-edit hook runs `cargo fmt` and `cargo check` after each agent
edit so type errors surface immediately. Pre-commit runs `cargo fmt --check`,
`cargo clippy -- -D warnings`, `typos`, `prettier --check`, and `eslint`.

If a hook is blocking a legitimate commit, fix the underlying issue rather
than skipping with `--no-verify`. If the hook itself is wrong, open a PR
to fix the hook.

## Security

Vulnerabilities go to luis@agsense.es, not the public issue tracker.
Details: [`SECURITY.md`](SECURITY.md).

## Questions

Open a GitHub Discussion for design or architecture questions. Use issues
for bugs and concrete feature requests.
