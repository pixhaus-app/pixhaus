# Skills to author

Claude Code skills are markdown files (`SKILL.md` per directory) that encode patterns and conventions agents use repeatedly. The ecosystem docs are reference material; skills are the actionable distillation.

Two tiers:

- **Pre-build skills** — author before B1 runs. Force multipliers; every stream uses them.
- **Stream-triggered skills** — author when the relevant stream comes online. They distill patterns that solidify only after the actual code exists.

## Pre-build skills (author before B1)

Four skills should exist before any code is written. Together they're roughly 1500-2500 lines of agent-actionable patterns.

### `pixhaus-rust-conventions`

**Purpose:** the floor every Rust-writing agent reaches for. Distillation of `ecosystem/06-rust-best-practices-2026.md` into agent-actionable patterns.

**What it covers:**
- The "no `unwrap()` in production paths" rule with the explicit alternatives
- The `thiserror` (libraries) / `anyhow` (apps) split
- The async patterns to use and the lock-across-await footguns to avoid
- The "single owner of mutable state" principle
- Common patterns Claude reaches for incorrectly in Rust, with corrections
- The newtype, sealed trait, type-state idioms with examples
- The code review checklist for Rust PRs

**Length target:** 400-600 lines. Bias toward code examples — agents learn idioms from examples, not from prose.

**Source:** `ecosystem/06-rust-best-practices-2026.md` is the input. The skill is the actionable subset.

### `pixhaus-claude-code-workflow`

**Purpose:** how to work in this repo with Claude Code. Every agent loads this; it shapes the commit/PR/branch behavior.

**What it covers:**
- Branch naming (`feat/sNN-<slug>`)
- Commit message format (Conventional Commits)
- PR template and what to include
- The ralph loop and what it expects from the agent (mark task done only after CI passes)
- Worktree pattern — why the agent shouldn't `cd` to other worktrees
- Hook configuration overview — what `cargo check` errors mean and how to act on them
- The "don't merge, open a PR" discipline
- When to escalate to a human reviewer

**Length target:** 300-400 lines.

**Source:** `work/dev-workflow.md` is the input.

### `pixhaus-tauri-patterns`

**Purpose:** Tauri-specific patterns. Every UI stream and the AI runtime use these.

**What it covers:**
- IPC command function signatures and return types
- State management with `tauri::State<T>`
- Event emission and subscription patterns
- Window management (creating, focusing, closing windows)
- Avoiding cross-thread issues with the main thread
- The `tauri-specta` typed-IPC pattern with examples
- Native menu integration
- Per-OS quirks (macOS title bar, Windows DPI scaling, Linux WebKitGTK gotchas)

**Length target:** 300-500 lines.

**Source:** `ecosystem/01-foundations.md` (Tauri section) is the primary input.

### `pixhaus-testing-conventions`

**Purpose:** how to write tests in this codebase. Every stream produces tests; consistent conventions make them composable.

**What it covers:**
- Inline `#[cfg(test)] mod tests` for unit tests; `tests/` directory for integration
- The `rstest` fixture pattern
- The `proptest` property-based test pattern with examples
- The `insta` snapshot test pattern; how to update snapshots intentionally
- Visual regression with `image-compare` — how to author tests, where baselines live
- Mocking with `mockall`; the trait-then-mock pattern
- HTTP mocking with `wiremock-rs` for AI backend tests
- The "every public function has at least one test" rule
- When to use `cargo nextest`; how to interpret its output
- How to run tests locally fast vs comprehensively

**Length target:** 400-600 lines.

**Source:** `ecosystem/04-scripting-and-testing.md` is the input.

## Stream-triggered skills (author as the relevant streams come online)

These four skills make sense to author when the underlying stream stabilizes — too early and you're encoding guesses; too late and the stream's outputs diverge.

### `pixhaus-image-processing`

**Triggered by:** S01-S02 reaching first PR.

**Purpose:** the day-to-day primitives for pixel manipulation. Every editor stream consumes these.

**Covers:** the `image` crate idioms, `imageproc` extensions, blend mode implementations, palette ops, indexed-mode discipline, `fast_image_resize` for SIMD-accelerated resizing, sprite-sheet packing patterns.

**Length:** 400-500 lines.

### `pixhaus-aseprite-format`

**Triggered by:** S08 starting, with a draft of `docs/aseprite-compat.md` from B7.

**Purpose:** the byte-level details of the `.aseprite` binary format with our compatibility level called out. Used by S08, format migration code, and any future format work.

**Covers:** chunk types, support level per chunk, the round-trip discipline, the LibreSprite reader as a reference, the test fixtures we maintain.

**Length:** 500-700 lines (the spec is dense).

### `pixhaus-verb-protocol`

**Triggered by:** B5 (verb protocol spec) and S21 (runtime) landing. Highest-leverage skill — used by all 14 verb streams.

**Purpose:** how to author an AI verb. The skill that makes verbs ship as plugins instead of editor changes.

**Covers:** the `Verb` trait, registration pattern, context injection, the preview-then-commit lifecycle, streaming output, cancellation, cost/latency declarations, backend selection. A worked example end-to-end (probably the `echo` verb from B5).

**Length:** 600-800 lines.

### `pixhaus-solid-ui`

**Triggered by:** S13 (application shell) reaching first PR.

**Purpose:** Solid.js conventions for our UI. State management, command palette, panel patterns.

**Covers:** signal patterns, when to use stores, the command palette plugin shape, panel registration API, theming via CSS custom properties, how to make a new panel that integrates with the shell.

**Length:** 400-500 lines.

### `pixhaus-ai-backend-adapter`

**Triggered by:** S22 landing.

**Purpose:** how to add a new AI inference backend. Used when extending `ai/src/backends/` with a new provider.

**Covers:** the `InferenceBackend` trait, the capability declaration, API key handling via `keyring`, cost estimation patterns, streaming response handling, error type mapping. A worked example (probably the Ollama adapter as the simplest).

**Length:** 400-500 lines.

## Authoring approach

Each skill is its own dispatch. Brief structure:

> Read `ecosystem/<source>.md` (the relevant ecosystem doc). Distill the agent-actionable patterns into a `SKILL.md` at `.claude/skills/<skill-name>/SKILL.md`. Length target: <N> lines. Bias toward code examples over prose. Bias toward "do this" patterns over "don't do that" — both useful but agents reach for affirmative patterns first. Reference: the existing skills in `~/.claude/skills/` for the format.

Author the four pre-build skills before B1 runs. Author the stream-triggered ones in the same PR that ships the corresponding stream's first non-trivial code — that way the skill encodes patterns the actual code uses, not patterns we hoped it would use.

## Where they live

Skills go in `.claude/skills/` at the repo root, one folder per skill, with `SKILL.md` inside:

```
.claude/
└── skills/
    ├── pixhaus-rust-conventions/
    │   └── SKILL.md
    ├── pixhaus-claude-code-workflow/
    │   └── SKILL.md
    ├── pixhaus-tauri-patterns/
    │   └── SKILL.md
    └── pixhaus-testing-conventions/
        └── SKILL.md
```

Claude Code auto-loads them when present in the project. Check the Claude Code skill docs for the exact frontmatter format expected.

## What skills are not

Skills are not documentation. Documentation lives in `docs/` and tells humans how to use the tool. Skills tell agents how to write code in the tool's codebase. Different audience, different format, different content.

Skills are not the ecosystem docs. The ecosystem docs are reference material. Skills are the patterns extracted from them. Don't duplicate; cross-reference.

Skills are not specs. The bedrock specs (`work/bedrock.md`) define contracts. Skills define how to write code that respects those contracts.

## Maintenance

When a pattern changes (a new convention emerges, a library deprecates), update the skill. Skills are version-controlled with the codebase, so they evolve with it. Treat the skills like any other production code: PRs, review, CI checks for broken cross-references.
