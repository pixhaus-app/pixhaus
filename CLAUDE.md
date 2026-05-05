# Pixhaus

Open-source AI-native pixel art editor for sprites, animations, and tilemaps. Tauri 2 + Rust workspace + TypeScript/Solid UI + WebGL2. Unity-only engine target. MIT license. Status: active development.

Complete planning context lives in `docs/planning/`. Read `docs/planning/product/scope.md` for what's being built, `docs/planning/architecture/stack.md` for the locked tech, and `docs/planning/work/dev-workflow.md` for how to work in this repo.

## Stack — locked, do not relitigate

- Rust core, Tauri 2.x app shell, TypeScript + Solid.js UI, WebGL2 viewport
- Project file format: MessagePack (`rmp-serde`) + zstd; schema spec at `docs/planning/work/bedrock.md` (B3)
- AI: multi-backend runtime via adapter pattern — Anthropic, OpenAI, Replicate, Ollama, ComfyUI, Stability
- Scripting: Lua via `mlua`. Plugins: `extism` for cross-language WASM
- Engine target: Unity 2022.3 LTS+ only. No Godot, Unreal, GameMaker
- License: MIT. Do not add GPL/LGPL/AGPL dependencies without explicit approval

## Skills — load when relevant

Four skills in `.claude/skills/` define how to write code in this repo:

- `pixhaus-rust-conventions` — Rust patterns, error handling, async, the no-unwrap rule
- `pixhaus-tauri-patterns` — IPC commands, state, events, tauri-specta
- `pixhaus-testing-conventions` — rstest, proptest, insta, image-compare, mockall
- `pixhaus-claude-code-workflow` — branches, commits, PRs, ralph loop, hooks

Load `pixhaus-rust-conventions` for any Rust work. Load `pixhaus-claude-code-workflow` whenever you commit. Stream-triggered skills (image-processing, aseprite-format, verb-protocol, solid-ui, ai-backend-adapter) come online as their streams land.

## Repo layout

```
core/        Rust crate — pixel manipulation, blend modes, undo, project model
io/          Rust crate — .aseprite, .psd, .pixhaus, PNG sprite sheets, TMX
ai/          Rust crate — verb runtime, backend adapters, built-in verbs
scripting/   Rust crate — Lua bindings (mlua)
app/         Tauri 2 shell, IPC commands, binary entry point
ui/          TypeScript + Solid UI (pnpm workspace)
unity/       Unity UPM package (importer + runtime helpers)
docs/        User documentation site; docs/planning/ snapshots all planning material
plugins/     Sample plugins (Lua + WASM)
examples/    Sample projects, fixtures, Unity demo
scripts/     Build, dev, hook, ralph-loop scripts
work/        Live task queue and stream tracking
```

## Commands

```bash
# Dev
pnpm dev                              # opens Pixhaus window with HMR
bacon                                 # background Rust check + clippy

# Build
cargo build --workspace
pnpm build                            # UI production bundle
pnpm tauri build                      # full release artifact

# Test
cargo nextest run --workspace
cargo nextest run -p core             # one crate
pnpm test

# Lint and format
cargo fmt --all
cargo clippy --workspace -- -D warnings
pnpm prettier --write ui/ && pnpm eslint ui/

# Pre-PR gate
./scripts/pre-pr.sh

# Setup and launch
pnpm bootstrap                        # first-time setup, idempotent
pnpm run doctor                       # check the dev environment (use `run`: pnpm has its own `doctor`)
pnpm dispatch B2 --model claude-opus-4-7
pnpm fan-out                          # print parallel ralph commands for unclaimed bedrock
pnpm finalize <worktree> <task> ok    # after the PR merges; flips queue to DONE
```

## Conventions

**Branches:** `feat/sNN-<slug>` for streams, `fix/<issue>-<slug>`, `chore/<slug>`, `docs/<slug>`. Every change ships on its own branch — never commit directly to `main`, even for one-line fixes. If you find yourself on `main` with edits, branch first and re-apply.

**Commits:** Conventional Commits — `feat:`, `fix:`, `chore:`, `docs:`, `refactor:`, `test:`.

**PRs:** every push opens a PR. Run `gh pr create --base main` immediately after `git push -u origin <branch>` — don't stop at the push and don't wait to be asked. Never merge directly. Reference the stream from `docs/planning/work/streams.md`. Use the PR template — what changed, why, test plan, screenshots if UI.

**Errors:** `thiserror` in library crates (`core`, `io`, `ai`, `scripting`). `anyhow` only in `app/`. Never `Box<dyn Error>` in public APIs. No `unwrap()` or `panic!()` outside tests.

**Async:** `tokio` runtime. Never hold a lock across `.await`. Use `spawn_blocking` for CPU-bound work. Native `async fn` in traits for static dispatch; `async-trait` only for `dyn Trait`.

**Memory:** every project has a single owner. Avoid `Arc<Mutex<>>` except at the app boundary. Pixel buffers are `Vec<u8>` with explicit stride, not `Vec<Vec<u8>>`.

**Tests:** every public function has at least one test. Property tests via `proptest` for image ops. Snapshot tests via `insta` for text, `image-compare` for visual regression. Mocks via `mockall` (trait-then-mock pattern).

**Rust style:** rustfmt defaults, clippy with `-D warnings`. Prefer iterators over indexing. Avoid premature `Box<dyn Trait>` when generics fit. Newtype wrappers for type safety. Sealed traits where extension is internal-only.

**TypeScript:** Prettier + ESLint, strict mode, no `any`, no unchecked nulls. Solid signals over stores unless state is broadly shared.

## Voice

Pragmatic Leader: direct, declarative, opinion-backed, contrarian-with-cause when the evidence supports it. State the rule, then the why, then the how-to-apply. Avoid LLM tells — no "moreover", "furthermore", "comprehensive", "robust", "powerful", "intuitive", "watershed", "stands as a testament". Sentence-case headings. Straight quotes. No emojis in code, comments, commit messages, or PR descriptions.

This applies to every artifact you produce: code comments, doc comments, commit messages, PR bodies, error messages, log messages, README updates.

## Working with the task queue

`work/queue.md` is the live task list. Each task has a state: UNCLAIMED, CLAIMED:&lt;worktree&gt;, DONE. Ralph loop usage and the worktree pattern are detailed in `docs/planning/work/dev-workflow.md`.

To claim a task manually: edit `work/queue.md` to mark it `CLAIMED:<your-worktree>`, work in a `git worktree`, open a PR when CI passes, mark DONE after merge.

Bedrock specs (B2-B7) block feature streams. The data model (B2) blocks everything else. The verb plugin protocol (B5) blocks all 14 AI verb streams (S23-S36). Don't dispatch streams that depend on unfinished bedrock.

## What Pixhaus is not

- Not a vector editor. Raster-only.
- Not a skeletal animation tool. No bones; mesh deformation arrives via the auto-mesh-deformation verb.
- Not a multi-engine tool. Unity only.
- Not a multiplayer editor. Single-user, file-based.
- Not subscription-funded. No "Pro" tier, no license server, no telemetry by default.
- Not mobile or web-targeted. Desktop only — Windows, macOS, Linux.

## When in doubt

1. Read the relevant skill in `.claude/skills/`.
2. Read the stream brief in `docs/planning/work/streams.md`.
3. Read the bedrock spec in `docs/planning/work/bedrock.md` if the question is contractual (data model, file format, IPC, verb protocol).
4. If the planning docs are silent, ask via PR comment or queue annotation. Don't guess on architectural decisions — the stack is locked, and if you think it shouldn't be, that's a planning-doc revision, not a code change.

The codebase is the source of truth for the present. The planning docs are the source of truth for the design. When they conflict, the planning docs win unless there's a recorded ADR overriding them.