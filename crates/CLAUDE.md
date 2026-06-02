# crates/ — the shared spine

The layer crates every workspace and module sits on (architecture bible section
4). One concern per crate; pure data and ops live deepest, I/O and integration at
the edges. If code doesn't obviously belong to one crate here, it's probably in
the wrong one — surface the question rather than smearing it across two.

## Rules for every crate in this tree

- These are library crates: errors are `thiserror`, never `anyhow` (that's the
  binary in `app/`). No `main`, no runtime ownership.
- Dependency direction is strict and acyclic: `core` is the leaf;
  `render`/`io`/`services`/`platform` depend only on `core`; `ui` depends on those.
  Never introduce a cycle, never depend on `app/`, never depend on a `modules/`
  crate.
- `core` and `render` stay egui-free, permanently — that is what lets the renderer
  survive a UI-toolkit change. `ui` is the only spine crate that may touch egui.
- Mutation of project state goes through the `Command` trait in `core`; nothing
  here mutates the model behind its back. State separates into five buckets and
  concurrency is organized as execution lanes (bible sections 22 and 31), but
  mutation stays on the `Command`/intent path regardless of which bucket or lane a
  crate touches.
- Libraries instrument with `tracing` — events (`info!`/`warn!`/`error!`/`debug!`)
  and `#[instrument]` on fallible/expensive functions — and never install a
  subscriber or `println!`/`eprintln!`. The binary in `app/` owns the one
  subscriber. See the `pixhaus-tracing` skill.

Global conventions (error policy, async, style, commits, the no-unwrap rule) live
in the root `CLAUDE.md`; the architecture is in
`docs/pixhaus_architecture_bible.md`. The per-crate files below only add that
crate's own boundaries.
