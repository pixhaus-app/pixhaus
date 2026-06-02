# Pixhaus

Open-source, AI-native, native-Rust tool for creating and animating sprites
across many art styles — pixel art is a first-class mode, not the whole product.
An `eframe` + `egui` shell with a `wgpu` canvas renderer. Unity-only engine
target. MIT license.

## Status: clean-slate restart (v3)

This is the `v3` branch — a second deliberate rebuild. `v2` proved out the native
Rust direction (an `eframe` + `egui` shell embedding a `wgpu` canvas, pixel data
and the GPU surface both in Rust so painting never crosses an IPC boundary — the
wall the original Tauri 2 + TypeScript/WebGL2 build hit at 4K/8K) but accumulated
breadth — AI backends, local FLUX inference, sprite pipelines — before the core
editor was solid. `v3` keeps the proven direction and the discipline.

What's landed: the architectural scaffold from
`docs/pixhaus_architecture_bible.md`. The workspace is now the full layered crate
graph (see Repo layout) with a runnable egui/wgpu spine — `cargo run -p
pixhaus-app` opens a window with a wgpu-drawn canvas. The layer and module crates
beyond that spine are compiling stubs; they gain bodies as the roadmap (bible §26)
reaches them. Don't mistake a stub for a missing decision — the boundary exists on
purpose; fill it, don't reshape it without cause.

The stack stays narrow: Rust + `egui`/`wgpu` and the async/serde backbone, and
nothing else until a crate earns the next dependency.

## Stack — locked, do not relitigate

- Rust everywhere. UI shell on `eframe` + `egui` + `egui-wgpu`; canvas renderer
  on `wgpu`, kept UI-agnostic so it survives a UI-toolkit change.
- Async: `tokio`, owned by the binary. Errors: `thiserror` in libs, `anyhow` in
  the binary. Serialization backbone: `serde` (add concrete formats when a crate
  needs them).
- Engine target: Unity 2022.3 LTS+ only. No Godot, Unreal, GameMaker.
- License: MIT. Do not add GPL/LGPL/AGPL dependencies. `cargo deny` enforces this.

The dependency catalog in `Cargo.toml` is intentionally minimal — the UI/render
stack, the async/error/serde backbone, and the test stack. Adding anything else
is a decision, not a default: justify the dependency, check its license, and load
the matching `pixhaus-<dep>` skill before using its API. The per-dependency skills
for crates not yet in the catalog (image, serde formats, http, etc.) stay
available for when a crate first reaches for them.

Why `egui`: the MIT lock rules out Slint (GPL-or-commercial). Among MIT/Apache
options, `egui` wins on the make-or-break requirement — embedding a custom `wgpu`
render pass inside a UI region is first-class (`egui_wgpu::Callback`), proven at
pro-tool scale, and has the largest contributor pool. The cost is immediate-mode
ergonomics for heavy dialog UIs, which is a learning curve, not a blocker.

## Skills — load when relevant

`.claude/skills/` holds two kinds of skill. They're auto-discovered: each one
triggers off its own `description`, so this file states the policy, not the
inventory — don't maintain a list here that would drift as skills come and go.

- **Conventions** — how to write code in this repo, regardless of crate:
  - `pixhaus-rust-conventions` — Rust patterns, error handling, async, the no-unwrap rule
  - `pixhaus-testing-conventions` — rstest, proptest, insta, image-compare, mockall
  - `pixhaus-claude-code-workflow` — branches, commits, PRs, hook output handling
  - `pixhaus-ui-conventions` — the design system: theme tokens, shared widgets, phosphor icons, the deferred-intent UI model
  - `pixhaus-tracing` — logging/tracing/diagnostics: levels, `#[instrument]`, spans, the one subscriber, the secrets rule
- **Per-dependency** — one skill per locked dependency (`pixhaus-egui`,
  `pixhaus-wgpu`, `pixhaus-tokio`, `pixhaus-image`, …), each the verified API and
  idioms for that crate at its pinned version. They fire when you work with that
  crate; load the matching one before reaching for a dependency's API from memory,
  since pinned versions drift from training data.

Always load `pixhaus-rust-conventions` for Rust work and `pixhaus-claude-code-workflow`
whenever you commit.

## Hooks

`.claude/settings.json` invokes `conclaude` (a Rust hook handler — install once
per machine from https://github.com/connerohnesorge/conclaude) for PreToolUse
guards and Stop validation. The full rule set lives in `.conclaude.yaml`; the
`pixhaus-claude-code-workflow` skill explains the agent-facing behavior.
PostToolUse on Edit/Write runs `scripts/post-edit.{ps1,sh}`, which formats and
runs `cargo clippy --tests -- -D warnings` on the touched crate.

The Stop hook is the session gate: `cargo fmt --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo test --workspace`, `cargo doc`, and
`cargo deny check`. A clean session means these all pass — once at least one crate
exists. On the empty workspace they error on the virtual manifest (see the status
note above); the gate becomes meaningful with the first crate.

## Repo layout

The root holds the workspace `Cargo.toml`, the toolchain and lint config
(`rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`), `.cargo/deny.toml`, the
hook config (`.conclaude.yaml`, `.claude/`), and `docs/` (the architecture bible
lives there). Code lives in three trees:

```
crates/    the shared spine - layer crates every workspace and module sits on
  core/      domain model: project, document, sprite, layer, frame, cel, palette,
             selection, art mode, ids, the Command trait. No egui, no wgpu.
  render/    UI-agnostic wgpu viewport renderer. Depends on core. Knows nothing
             about egui. The perf-critical code.
  io/        project format, PNG, sprite sheets, importer/exporter traits.
  services/  command execution, undo/transactions, the job system, provider dispatch.
  platform/  native dialogs, recent files, OS paths, GPU capability detection.
  ui/        the egui contribution surface - Panel/Tool/Workspace/Provider/Importer/
             Exporter/Validator traits, the registries, the Module trait, theme
             tokens, and the egui<->render canvas paint callback. The only crate
             that knows both egui and render.

modules/   internal capability modules (bible section 7). Each registers
           capabilities with the host; none owns core data. Compiled in, not
           dynamically loaded.
  core/ sprite-edit/ animation/ generation/ pixel-art/ tiles/ export/ providers/

app/       the eframe binary (Host App layer). Owns the tokio runtime, boots the
           window, registers modules, runs the egui loop. Depends on everything;
           nothing depends on it.
```

Dependency direction is strict and acyclic: `core` is deepest and egui-free;
`render`, `io`, `services`, and `platform` depend only on `core`; `ui` depends on
those; `modules/*` depend on `core` + `services` + `ui`; `app` depends on all.
`core` and `render` never see egui — that keeps the renderer alive across a
UI-toolkit change. Crate packages are `pixhaus-<dir>` (`pixhaus-core`,
`pixhaus-render`, …) and `pixhaus-mod-<name>` for modules.

Don't add a new top-level directory without updating this section first.
`conclaude`'s `preventRootAdditions` blocks it otherwise.

## Architecture

`docs/pixhaus_architecture_bible.md` is the source of truth — read it before
making a structural decision. The load-bearing rules it sets, which the crate
graph above encodes:

- Workspaces are task-focused layouts over shared capabilities; they don't own
  data models. Draw and Animate are siblings over one sprite-editing core.
- Tools create commands; commands own all mutation of project state and are
  undoable. Tools and AI results never mutate the model directly.
- Long, expensive, or external work is a job. Jobs produce results; applying a
  result is a command. AI generation never touches the canvas directly.
- Capabilities are registered by internal modules through registries — no
  external dynamic plugins.
- State separates into five buckets — durable project, session, UI,
  tool-interaction, and derived/cache — and concurrency is organized as execution
  lanes through jobs and services, not scattered in UI code; the runtime, state,
  and concurrency model lives in the bible (sections 22, 31-33).
- GPU textures are caches and views; the project model is the source of truth.
- Pixel art is a deep, dedicated mode, not the whole product.

When a change spans a boundary the bible draws, follow the bible, not
convenience.

## Design system

The app has one design system, owned by `crates/ui`: theme tokens (color, spacing,
type, elevation), the shared `widgets`, and phosphor `icons`. All UI is built from
them — no hex colors, no emoji, no bespoke chrome — so the look stays consistent and
survives a theme change. The visual target is `docs/pixhaus_visual_ux_direction.md`;
verify a UI change by rendering it (`cargo run -p pixhaus-app --example
render_workspaces` writes `target/ui-snapshots/`) and comparing to
`docs/ui_visual_example/`. The full rules live in `crates/ui/CLAUDE.md` and the
`pixhaus-ui-conventions` skill.

## Logging

Structured `tracing` from day one. The binary (`app/`) owns the ONE subscriber
(`app/src/diagnostics.rs`): a console sink on stderr plus a rolling daily
`pixhaus.log` under the OS log dir (`pixhaus_platform::log_dir()`), gated by a single
`EnvFilter`. Libraries emit (`tracing::info!`/`warn!`/`error!`/`debug!`,
`#[instrument]`) and never install a subscriber or `println!`. `RUST_LOG` overrides
the default filter. Profiling for now means reading `#[instrument]` span durations —
puffin/tracy/criterion are a later, deliberate step. Never log API keys or secrets.
The full rules are in the `pixhaus-tracing` skill.

## Commands

```bash
# Build
cargo build --workspace

# Test
cargo nextest run --workspace
cargo nextest run -p <crate>
cargo test --doc --workspace

# Lint and format
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check --config .cargo/deny.toml

# Background watcher
bacon
```

## Conventions

**Branches:** `feat/<slug>`, `fix/<issue>-<slug>`, `chore/<slug>`, `docs/<slug>`,
`refactor/<slug>`, `perf/<slug>`. Every change ships on its own branch — never
commit directly to `main`, even for one-line fixes. If you find yourself on
`main` with edits, branch first and re-apply.

**Commits:** Conventional Commits — `feat:`, `fix:`, `chore:`, `docs:`,
`refactor:`, `perf:`, `test:`.

**PRs:** open a PR from the feature branch; never merge directly. Use the
PR body to say what changed, why, and the test plan; add screenshots/clips for
UI changes.

**Errors:** `thiserror` in library crates. `anyhow` only in the binary. Never
`Box<dyn Error>` in public APIs. No `unwrap()` or `panic!()` outside tests
(clippy-enforced).

**Async:** `tokio` runtime, owned by the binary — one owner, no scattered
`#[tokio::main]`. Never hold a lock across `.await`. Use `spawn_blocking` for
CPU-bound work. The egui update loop runs on one thread and owns the document
directly; background tasks return results over channels the loop drains each frame.

**Memory:** every piece of mutable state has a single owner. Avoid `Arc<Mutex<>>`
except where state is genuinely shared across threads. Pixel buffers are
`Vec<u8>` with explicit stride, not `Vec<Vec<u8>>`.

**Tests:** every public function has at least one test. Property tests via
`proptest` for image ops. Snapshot tests via `insta` for text, `image-compare`
for visual regression. Mocks via `mockall` (trait-then-mock pattern).

**Rust style:** rustfmt defaults, clippy with `-D warnings`. Prefer iterators
over indexing. Avoid premature `Box<dyn Trait>` when generics fit. Newtype
wrappers for type safety. Sealed traits where extension is internal-only.
`unsafe` is forbidden workspace-wide.

## Voice

Pragmatic Leader: direct, declarative, opinion-backed, contrarian-with-cause when
the evidence supports it. State the rule, then the why, then the how-to-apply.
Avoid LLM tells — no "moreover", "furthermore", "comprehensive", "robust",
"powerful", "intuitive", "watershed", "stands as a testament". Sentence-case
headings. Straight quotes. No emojis in code, comments, commit messages, or PR
descriptions.

This applies to every artifact you produce: code comments, doc comments, commit
messages, PR bodies, error messages, log messages, README updates.

## What Pixhaus is not

- Not a vector editor. Raster-only.
- Not a skeletal animation tool. No bones; mesh deformation arrives via the
  auto-mesh-deformation verb.
- Not a multi-engine tool. Unity only.
- Not a multiplayer editor. Single-user, file-based.
- Not subscription-funded. No "Pro" tier, no license server, no telemetry by default.
- Not mobile or web-targeted. Desktop only — Windows, macOS, Linux.

## When in doubt

1. Read `docs/pixhaus_architecture_bible.md` — it settles most structural questions.
2. Read the relevant skill in `.claude/skills/`.
3. The stack is locked. If you think it shouldn't be, that's a discussion to
   raise, not a code change to make unilaterally.
4. Don't guess on architectural decisions. Surface the question.
