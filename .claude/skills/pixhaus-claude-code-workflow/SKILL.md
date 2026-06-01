---
name: pixhaus-claude-code-workflow
description: Use when contributing to Pixhaus via Claude Code — covers branch naming, commit format, PR conventions, hook output handling, and when to escalate
---

# Pixhaus Claude Code workflow

How to work in the Pixhaus repo with Claude Code. Every agent loads this;
it shapes branch, commit, PR, and review behavior.

## The shape of a contribution

One logical change → one branch → one PR → review → merge. Keep branches
scoped. Never commit to `main`.

Steps for a typical task:

1. Branch from `main`: `git switch -c feat/<slug>`.
2. Implement. The post-edit hook formats and lints each crate you touch.
3. Run the local checks: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`.
4. Commit in Conventional Commit format.
5. Push and open a PR.
6. Wait for review. Address feedback in follow-up commits.

## Branch naming

| Prefix | Use for | Example |
|---|---|---|
| `feat/<slug>` | features | `feat/wgpu-viewport` |
| `fix/<issue>-<slug>` | bug fixes | `fix/142-tile-flicker` |
| `chore/<slug>` | infra, deps, build | `chore/bump-egui` |
| `docs/<slug>` | docs-only | `docs/render-crate-api` |
| `refactor/<slug>` | behavior-preserving restructure | `refactor/split-shell-panels` |
| `perf/<slug>` | performance work | `perf/dirty-rect-uploads` |

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

Scope is the crate or area: `core`, `io`, `render`, `shell`, `ci`, `deps`,
`skills`. Multi-scope commits are rare; if you have one, drop the scope rather
than listing many.

Subject:

- under 72 characters
- imperative mood ("add", "remove", "rename")
- no trailing period
- lower-case start (Conventional Commits convention)

Body:

- "why", not "what" — the diff shows the what
- wrap at ~72 columns
- reference the issue if there is one: `Closes #142`

Examples:

```
feat(io): add zstd compression for pixel buffer payloads

Pixel buffers are large; zstd at level 3 cuts file size ~60% with
sub-millisecond decode for typical sprite sheets. Header records the
compression level so future readers can vary it.
```

```
fix(core): clamp brush coordinates before lookup

Out-of-bounds brush ticks reached the pixel-at function and returned
None, which then triggered an unwrap two frames up the stack. Clamping
at the input layer keeps the bounds check local.

Closes #142
```

## Pull request description

Cover, concisely:

- What changed (one or two sentences)
- Why (link the issue if there is one)
- Test plan (what ran, what passed, what's not yet covered)
- Screenshots / clips for UI changes

A good test plan is concrete:

> - `cargo test --workspace --all-targets` — green
> - New tests: `core/tests/blend_modes.rs` covers the four new modes
> - Not covered: visual regression for the new transform pipeline

A bad test plan is vague:

> - Tests pass locally

Tick the basics yourself before requesting review: format, clippy, test, doc,
`cargo deny`.

## Reading hook output

The post-edit hook (`scripts/post-edit.ps1` on Windows, `scripts/post-edit.sh`
on *nix) runs after every Edit/Write. Its output appears in your tool result —
read it. Both versions emit the same `post-edit: ...` prefixes; PowerShell
wording for the underlying `cargo` output differs slightly from bash, but the
signals are the same.

Common signals:

- `post-edit: cargo clippy --manifest-path .../Cargo.toml --tests -- -D warnings`
  — the hook found the owning crate and ran clippy with warnings denied. If
  you don't see this for a `.rs` file you edited, no owning crate was found
  (the file isn't under a crate with a `[package]` yet); the hook ran
  `cargo fmt --all` instead.
- `error[E0XXX]: ...` followed by code — type error from rustc (clippy
  surfaces these the same as `cargo check`). Fix before moving on. Don't
  accumulate errors across multiple edits; the next compile will be
  noisier and the fix harder.
- `warning: ...` followed by `error: ... could not compile ... due to ...
  previous error; N warnings emitted` — a clippy lint hit `-D warnings` and
  was promoted to an error. Read the warning text; either fix it or, if the
  lint is genuinely wrong here, annotate with a scoped
  `#[allow(clippy::lint_name)]` and a one-line justification.

A separate Stop hook (via conclaude) runs `cargo fmt --check`, clippy, the
workspace tests, `cargo doc`, and `cargo deny` when the session ends. If those
fail, the session won't complete cleanly — fix the failures before declaring done.

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

- A test fails in a way you don't understand after one fix attempt
- You're tempted to add a load-bearing dependency, or one whose license
  `cargo deny` would reject
- A review comment asks you to do something that would break an established
  contract (the file format, a public crate API)
- The same task fails on the third attempt (ambiguity, not a Claude problem)

Escalation is cheap. Wrong code is expensive. The cost asymmetry favors asking.

## Don't

- Don't merge your own PRs.
- Don't push to `main` directly.
- Don't `git rebase -i` or `git reset --hard` on a branch others might
  have based work on. If you need to clean history, do it on the branch
  before opening the PR.
- Don't `--no-verify` past a hook. If the hook is wrong, fix the hook
  in a separate PR.
- Don't add `// TODO` comments without a name and a reference.
  `// TODO(luis): handle EOF` is fine; bare `// TODO` rots.
- Don't bump the workspace **MSRV** (`Cargo.toml`'s `rust-version`) without
  a maintainer's approval — it's a public contract. The **toolchain pin**
  (`rust-toolchain.toml`'s `channel`) is internal and moves freely with each
  Rust release; bumping it to pick up a new compiler feature or to satisfy a
  transitive dep is fine, just keep MSRV ≤ pin.
- Don't introduce a new top-level directory without coordination
  (conclaude's `preventRootAdditions` enforces this). The layout in CLAUDE.md
  is the source of truth.

## Do

- Do run `cargo test -p <crate>` for the crate you touched, in addition
  to the workspace test.
- Do leave the workspace in a state where `cargo build --workspace` succeeds
  before pushing.
- Do mention non-obvious decisions in the PR description ("I chose
  `parking_lot::Mutex` over `tokio::sync::Mutex` because the critical
  section is sub-microsecond and never crosses an await").
- Do open a docs change alongside a code change that alters public surface.
