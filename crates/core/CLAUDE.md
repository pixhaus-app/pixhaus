# pixhaus-core

The creative core — the authoritative domain model and pure operations
(architecture bible sections 4.3, 9, 12). The deepest layer; everything depends
on it, it depends on nothing in the workspace.

- **Owns:** projects, documents, sprites, layers, frames, cels, palettes,
  selections, art-mode metadata, the typed ids that key them, the `Command` trait,
  and pure pixel ops.
- **Depends on:** no workspace crate. External: `serde`, `thiserror`, `glam`,
  `bytemuck` as the model needs them.
- **Used by:** every other crate.
- **Status:** stub.

## Boundaries

- MUST NOT depend on `egui`, `wgpu`, or any UI/GPU crate.
- MUST NOT do I/O — no file or network access. That's `io`/`services`.
- MUST NOT depend on another workspace crate. If you reach for one, the
  abstraction belongs here and the concrete adapter belongs in the consumer.
- All project-state mutation is expressed as a `Command`; pixel buffers are
  `Vec<u8>` with explicit stride, never `Vec<Vec<u8>>`.

Global rules: root `CLAUDE.md`. Architecture: `docs/pixhaus_architecture_bible.md`.
