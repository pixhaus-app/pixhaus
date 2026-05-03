# Pixhaus

Pixhaus is the open-source AI-native pixel art editor for sprites, animations, and tilemaps.

> Status: in active development. The repo is a working scaffold; the editor itself ships
> as the work streams in `docs/planning/work/streams.md` land. The first usable internal
> build comes when the critical-path streams complete.

## What Pixhaus is

A unified pixel art tool that closes three gaps:

- **Aseprite + Tiled in one app.** Sprites, animations, and tilemaps live in the same
  project file with one selection model and one undo stack.
- **AI verbs in the canvas.** Inbetween, Continue, Extend, Variant, Cleanup, Tile,
  Critique, and more — each runs against project context (palette, layers, references)
  and produces a non-destructive layer the artist can accept or reject.
- **Open from day one.** MIT license, open file format (`.pixhaus`), Lua scripting,
  and a plugin protocol that can add custom AI verbs without forking the editor.

The full scope, positioning, and what Pixhaus deliberately is *not* live in
[`docs/planning/product/scope.md`](docs/planning/product/scope.md).

## Tech stack

- Tauri 2.x application shell (Rust core + native webview)
- Rust workspace: `core`, `io`, `ai`, `scripting`, `app`
- TypeScript + Solid.js UI built with Vite
- WebGL2 viewport with hot pixel paths in Rust
- Lua scripting via `mlua`
- MessagePack + zstd project file format
- Unity 2022.3 LTS minimum for the importer

Lock-in rationale: [`docs/planning/architecture/stack.md`](docs/planning/architecture/stack.md)
and [`docs/planning/architecture/rust-vs-electron.md`](docs/planning/architecture/rust-vs-electron.md).

## Local setup

Prerequisites:

- Rust 1.95+ stable (the toolchain pin in `rust-toolchain.toml` handles this)
- Node 22 LTS or newer
- pnpm 10+ (run `corepack enable pnpm` if you don't have it)
- Tauri 2 system dependencies — see [tauri.app/start/prerequisites](https://v2.tauri.app/start/prerequisites/)
- Tauri CLI: `cargo install tauri-cli --version "^2.0.0" --locked`

Bring it up:

```bash
pnpm install
pnpm dev          # opens an empty Pixhaus window
pnpm build        # builds the production bundle
```

Run the Rust workspace independently:

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

Optional dev tooling: `bash scripts/install-tools.sh` installs the agent-side tools
(`cargo-nextest`, `cargo-deny`, `cargo-audit`, `cargo-machete`, `cargo-watch`,
`typos-cli`, `bacon`).

## How the work happens

The build is structured for parallel agent execution behind a small set of
locked contracts. Read these in order:

1. [`docs/planning/work/bedrock.md`](docs/planning/work/bedrock.md) — the eight specs
   that lock cross-stream contracts.
2. [`docs/planning/work/streams.md`](docs/planning/work/streams.md) — the 52 parallel
   work streams, each with an agent brief.
3. [`docs/planning/work/dev-workflow.md`](docs/planning/work/dev-workflow.md) — the
   Claude Code + ralph loop + worktree pattern that runs the build.

The active task queue is at [`work/queue.md`](work/queue.md).

## Contributing

Read [`CONTRIBUTING.md`](CONTRIBUTING.md) for the contributor flow, branch
conventions, and review expectations. Code of conduct: [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).
Security reports: [`SECURITY.md`](SECURITY.md).

## License

[MIT](LICENSE).
