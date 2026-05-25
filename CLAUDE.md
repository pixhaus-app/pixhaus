# Pixhaus

Open-source AI-native pixel art editor for sprites, animations, and tilemaps.
Native Rust: an `eframe` + `egui` shell with a `wgpu` canvas renderer. Unity-only
engine target. MIT license.

## Status: clean-slate restart

This is the `v2` branch — a deliberate rebuild. The previous version ran on
Tauri 2 + a TypeScript/Solid UI + a WebGL2 viewport, and proved out the domain.
Drawing and playback hit a wall at 4K/8K: pixel data lives in Rust, the GPU
surface lived in the webview, and every painted pixel had to cross the IPC
boundary. No encoding trick removes that copy. The fix is to put the GPU surface
in Rust.

So `v2` starts empty and goes native. There is no carried-over code yet — crates
land as the work defines them. Don't assume a `core`/`io`/`shell` exists until
you see it.

The workspace has no members yet (`Cargo.toml`'s `members = []`). Until the first
crate lands, `cargo` workspace commands and the `conclaude` Stop gate will error
with "the manifest is virtual, and the workspace has no members" — that is
expected, not a misconfiguration. The gate goes green the moment you add a crate
to `members`. Don't weaken the gate to silence it; add the first crate instead.

## Stack — locked, do not relitigate

- Rust everywhere. UI shell on `eframe` + `egui` + `egui-wgpu`; canvas renderer
  on `wgpu`, kept UI-agnostic so it survives a UI-toolkit change.
- Project file format: MessagePack (`rmp-serde`) + zstd.
- AI: multi-backend runtime via an adapter pattern — Anthropic, OpenAI,
  Replicate, Ollama, ComfyUI, Stability.
- Scripting: Lua via `mlua`. Plugins: `extism` for cross-language WASM.
- Engine target: Unity 2022.3 LTS+ only. No Godot, Unreal, GameMaker.
- License: MIT. Do not add GPL/LGPL/AGPL dependencies. `cargo deny` enforces this.

Why `egui`: the MIT lock rules out Slint (GPL-or-commercial). Among MIT/Apache
options, `egui` wins on the make-or-break requirement — embedding a custom `wgpu`
render pass inside a UI region is first-class (`egui_wgpu::Callback`), proven at
pro-tool scale, and has the largest contributor pool. The cost is immediate-mode
ergonomics for heavy dialog UIs, which is a learning curve, not a blocker.

## Skills — load when relevant

Three skills in `.claude/skills/` define how to write code in this repo:

- `pixhaus-rust-conventions` — Rust patterns, error handling, async, the no-unwrap rule
- `pixhaus-testing-conventions` — rstest, proptest, insta, image-compare, mockall
- `pixhaus-claude-code-workflow` — branches, commits, PRs, hook output handling

Load `pixhaus-rust-conventions` for any Rust work. Load
`pixhaus-claude-code-workflow` whenever you commit.

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

A clean slate. The root holds the workspace `Cargo.toml`, the toolchain and lint
config (`rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`), `.cargo/deny.toml`,
and the hook config (`.conclaude.yaml`, `.claude/`).

Crates land as the work defines them. The expected shape:

```
render/   UI-agnostic wgpu viewport renderer. Depends on core + wgpu.
          Knows nothing about egui. The perf-critical code.
shell/    eframe + egui + egui-wgpu binary. Owns app state, hosts panels,
          embeds render/ via an egui paint callback. Depends on everything.
core/     pixel ops, blend, undo, project model, selection, transforms (as it lands)
io/       the .pixhaus format, PNG, sprite sheets, etc. (as it lands)
```

Don't add a new top-level directory without updating this section first.
`conclaude`'s `preventRootAdditions` blocks it otherwise.

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

1. Read the relevant skill in `.claude/skills/`.
2. The stack is locked. If you think it shouldn't be, that's a discussion to
   raise, not a code change to make unilaterally.
3. Don't guess on architectural decisions. Surface the question.
