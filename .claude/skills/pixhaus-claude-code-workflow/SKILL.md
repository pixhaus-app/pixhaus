---
name: pixhaus-claude-code-workflow
description: Use when contributing to Pixhaus via Claude Code — covers branch naming, commit format, ralph loop expectations, worktree discipline, hook output handling, and when to escalate
---

# Pixhaus Claude Code workflow

How to work in the Pixhaus repo with Claude Code. Every agent loads this;
it shapes branch, commit, PR, and review behavior.

## The shape of a Pixhaus contribution

One stream → one branch → one PR → review → merge. The bedrock-first
architecture is designed so streams don't need to coordinate; the contract
is locked, and your job is to implement against it.

Steps for a typical task:

1. Pull from `main`. Confirm the queue lists the task as unclaimed.
2. Claim the task by editing `work/queue.md` (or via `node scripts/run.mjs claim-next-task <worktree>`).
3. Read the brief in `docs/planning/work/bedrock.md` or `docs/planning/work/streams.md`.
4. Branch: `git switch -c feat/sNN-<slug>` or `feat/bN-<slug>`.
5. Implement. The post-edit hook formats and type-checks each file you save.
6. Run the local checks: `cargo test --workspace`, `pnpm test`, `pnpm typecheck`.
7. Commit in Conventional Commit format.
8. Push and open a PR with the template filled in.
9. Wait for CI. Address review feedback in follow-up commits.
10. Mark the task done in `work/queue.md` only after merge.

## Branch naming

| Prefix | Use for | Example |
|---|---|---|
| `feat/sNN-<slug>` | feature streams | `feat/s07-pixhaus-format` |
| `feat/bN-<slug>` | bedrock specs | `feat/b3-project-file-format` |
| `fix/<issue>-<slug>` | bug fixes | `fix/142-tile-flicker` |
| `chore/<slug>` | infra, deps, build | `chore/bump-tauri-2.5` |
| `docs/<slug>` | docs-only | `docs/aseprite-compat-spec` |

`<slug>` is kebab-case and short. Branches are scoped to one logical change.
If you're tempted to add an unrelated fix while you're in the file, restrain
yourself — it muddies the PR. Open a follow-up.

## Commit format

[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/):

```
<type>(<scope>): <subject>

<optional body explaining why, not what>

<optional footers>
```

Types: `feat`, `fix`, `chore`, `docs`, `refactor`, `perf`, `test`, `build`, `ci`.

Scope is the crate or area: `core`, `io`, `ai`, `scripting`, `app`, `ui`,
`unity`, `ci`, `deps`, `skills`. Multi-scope commits are rare; if you have
one, drop the scope rather than listing many.

Subject:

- under 72 characters
- imperative mood ("add", "remove", "rename")
- no trailing period
- lower-case start (Conventional Commits convention)

Body:

- "why", not "what" — the diff shows the what
- wrap at ~72 columns
- reference the stream or bedrock spec: `Implements: docs/planning/work/streams.md#s07`
- reference the issue if there is one: `Closes #142`

Examples:

```
feat(io): add zstd compression for pixel buffer payloads in .pixhaus

Pixel buffers are large; zstd at level 3 cuts file size ~60% with
sub-millisecond decode for typical sprite sheets. Header records the
compression level so future readers can vary it.

Implements: docs/planning/work/bedrock.md#b3
```

```
fix(core): clamp brush coordinates before lookup

Out-of-bounds brush ticks reached the pixel-at function and returned
None, which then triggered an unwrap two frames up the stack. Clamping
at the input layer keeps the bounds check local.

Closes #142
```

## Pull request template

The template (`.github/PULL_REQUEST_TEMPLATE.md`) prompts for:

- What changed (one or two sentences)
- Why (link to the stream / bedrock spec / issue)
- Test plan (what ran, what passed, what's not yet covered)
- Screenshots / clips for UI changes
- Open questions for the reviewer

A good test plan is concrete:

> - `cargo test --workspace --all-targets` — green
> - New tests: `core/tests/blend_modes.rs` covers the four new modes
> - Not covered: visual regression for the new transform pipeline (scheduled
>   to land with S04)

A bad test plan is vague:

> - Tests pass locally

The checklist at the bottom of the template enforces the basics: format,
clippy, typecheck, lint, build. Tick each box yourself before requesting
review.

## Ralph loop expectations

The ralph loop runs Claude in a worktree, claims tasks, opens PRs, and
waits for CI. As the agent inside the loop, your contract is:

- **Mark the task done only after CI passes.** Not after you push, not after
  the PR opens. After CI is green. The `finalize-task.sh` script enforces
  this; don't bypass it.
- **Open a PR — do not merge.** The ralph loop never clicks merge. A human
  reviews and merges. If you ever find yourself running `git push origin main`
  or `gh pr merge`, stop.
- **One task per loop iteration.** If you finish early, claim the next task
  on the next iteration. Don't pile two tasks into one PR.
- **Surface ambiguity instead of guessing.** If a brief is unclear or
  contradicts another doc, return the task to the queue with a `[FAIL]`
  reason and a one-line note. A human disambiguates faster than you can
  guess wrong and revert.
- **Don't `cd` to other worktrees.** Each ralph loop is locked to one
  worktree. Reaching into another worktree creates conflicts the loop
  can't resolve.

## Worktree discipline

Worktrees live at `../pixhaus-worktrees/<name>` next to the main repo.
Create one with:

```bash
node scripts/run.mjs new-worktree stream-s07
```

The dispatcher (`scripts/run.mjs`) picks `new-worktree.sh` on *nix and `new-worktree.ps1` on Windows. Every script in `scripts/` ships in both forms — when you add a new one, write both `.sh` and `.ps1` and route invocations through `node scripts/run.mjs <task>`.

Hard rules:

- Each worktree owns one branch. Don't switch branches inside a worktree.
- Don't `cd` into another worktree's directory mid-task. If you need to
  reference its state, use `git log <other-branch>` or open a separate
  shell.
- Clean up dead worktrees: `git worktree prune` removes deleted ones.
  Track live ones in `.worktrees.md`.
- Don't share build artifacts across worktrees. Cargo's `target/` is local
  to each worktree by default; don't symlink it across.

If two streams genuinely depend on each other, use a feature branch with
stacked PRs and call it out in the PR description. This is rare; the
bedrock-first architecture is designed to make it rare.

## Reading hook output

The post-edit hook (`scripts/post-edit.sh` on *nix, `scripts/post-edit.ps1` on
Windows; routed through `scripts/run.mjs`) runs after every Edit/Write.
Its output appears in your tool result — read it. Both versions emit the same
prefixes (`post-edit: ...`); PowerShell error wording for the underlying
`cargo` / `tsc` output differs slightly from bash, but the signals are the same.

Common signals:

- `post-edit: cargo clippy --tests -p pixhaus-core -- -D warnings` — the
  hook found the owning crate and ran clippy with warnings denied. If you
  don't see this for a `.rs` file you edited, the crate-finder failed;
  check the crate's `Cargo.toml` exists and has a `[package]` section.
- `error[E0XXX]: ...` followed by code — type error from rustc (clippy
  surfaces these the same as `cargo check`). Fix before moving on. Don't
  accumulate errors across multiple edits; the next compile will be
  noisier and the fix harder.
- `warning: ...` followed by `error: ... could not compile ...
  due to ... previous error; N warnings emitted` — a clippy lint hit
  `-D warnings` and was promoted to an error. Read the warning text;
  either fix it or, if the lint is genuinely wrong here, annotate with a
  scoped `#[allow(clippy::lint_name)]` and a one-line justification.
- `post-edit: tsc reported errors` — TypeScript type error. Fix immediately.
- `post-edit: cargo clippy failed in crate ...` — non-fatal at the hook
  level (the hook always returns 0 to avoid blocking edits), but the
  workspace is in a bad state. The errors are in the previous lines;
  scroll up.

A separate Stop hook (via conclaude) runs `cargo nextest run --workspace`
and `pnpm test` when the session ends. If those fail, the session won't
complete cleanly — fix the failures before declaring done.

What `cargo clippy` / rustc errors mean and how to act:

- **`cannot find type / function`** — missing import, wrong path, or the
  symbol moved. Search the workspace; don't define a duplicate.
- **`mismatched types`** — convert at the boundary, don't change unrelated
  signatures. Look up where the inferred vs. expected types diverge.
- **`borrow checker`** — re-examine ownership. Hoist the borrow, take a
  reference earlier, or restructure to avoid the conflict. Don't `clone()`
  large buffers to dodge it.
- **`Send / Sync not satisfied`** — usually means a non-Send type is held
  across `.await`. Identify the type (`Rc`, `RefCell`, `*mut`, raw pointers)
  and replace it (`Arc`, `tokio::sync::Mutex`, etc.).

## When to escalate

Stop and ask a human reviewer when:

- A bedrock spec contradicts the ecosystem docs (or itself across sections)
- A dependency in the planning docs has been yanked or no longer exists
- A Tauri 2 API the planning doc references has changed since the pinned version
- A test fails in a way you don't understand after one fix attempt
- You're tempted to add a dependency not listed in `docs/planning/ecosystem/`
- A review comment asks you to do something that would break the bedrock contract
- Two streams need the same change but only one of them is yours
- The same task fails on the third attempt (ambiguity, not a Claude problem)

Escalation is cheap. Wrong code is expensive. The cost asymmetry favors
asking.

## Don't

- Don't merge your own PRs.
- Don't push to `main` directly.
- Don't `git rebase -i` or `git reset --hard` on a branch others might
  have based work on. If you need to clean history, do it on the branch
  before opening the PR.
- Don't `--no-verify` past a hook. If the hook is wrong, fix the hook
  in a separate PR.
- Don't add `// TODO` comments without a name and a reference. `// TODO(s07): handle EOF`
  is fine; bare `// TODO` rots.
- Don't bump the workspace **MSRV** (`Cargo.toml`'s `rust-version`) without
  a maintainer's approval — it's a public contract that cascades to every
  consumer of the library crates. The **toolchain pin**
  (`rust-toolchain.toml`'s `channel`) is internal and moves freely with each
  Rust release; bumping it to pick up a new compiler feature or to satisfy
  a transitive dep is fine, just keep MSRV ≤ pin.
- Don't introduce a new top-level directory without coordination. The
  layout in `docs/planning/architecture/stack.md` is the source of truth.

## Do

- Do read the brief in full before writing code.
- Do run `cargo test -p <crate>` for the crate you touched, in addition
  to the workspace test.
- Do leave the workspace in a state where `cargo build --workspace` and
  `pnpm build` both succeed before pushing.
- Do mention non-obvious decisions in the PR description ("I chose
  `parking_lot::Mutex` over `tokio::sync::Mutex` because the critical
  section is sub-microsecond and never crosses an await").
- Do open a docs PR alongside a code change that changes public surface.
