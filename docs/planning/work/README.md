# Work organization

The build is structured for maximum parallel agent dispatch. Read in this order:

1. **[../architecture/rust-vs-electron.md](../architecture/rust-vs-electron.md)** — the Rust question, answered. Tauri + Rust is the call.
2. **[../architecture/stack.md](../architecture/stack.md)** — the locked tech stack, repo layout, and IPC architecture.
3. **[bedrock.md](bedrock.md)** — the small set of contracts that must exist before parallel work can fan out (8 specs, ~1 week).
4. **[streams.md](streams.md)** — the 52 parallel work streams, each with an agent brief ready to dispatch.
5. **[dev-workflow.md](dev-workflow.md)** — Claude Code + ralph loop + worktrees + hooks + model strategy (Opus plan, Sonnet execute).
6. **[skills.md](skills.md)** — the pre-build and stream-triggered skills to author so agents reach for the right patterns by default.

## Dispatch model

Bedrock is mostly sequential. Streams are mostly parallel. The bottleneck is review, not execution — once you accept the bedrock contracts, the streams don't need to coordinate beyond rare cross-references.

Critical-path streams (marked ★ in `streams.md`) unblock other streams or block the first usable build. Staff those first.

## What's deliberately not phased

There's no v1 / v1.5 / v2 framing. The project ships everything, in parallel, as the streams complete. Some streams are quick (1-2 weeks); some are longer (3-4 weeks). The first usable internal build comes when the critical-path streams complete (roughly week 6-8). The first public release follows when documentation, brand, and packaging streams (S41-S50) catch up.

## Engine target

Unity only. No Godot importer, no Unreal importer, no GameMaker integration in this scope. The `.tmx` export (S12) exists because Unity users go through Tiled importers; not because we're targeting Tiled the engine.

## Updates to this plan

When a new feature, capability, or dependency emerges, add a new stream entry to `streams.md` with the same brief structure. Don't bury new work as "v2" — just add it to the parallel list with appropriate dependencies.
