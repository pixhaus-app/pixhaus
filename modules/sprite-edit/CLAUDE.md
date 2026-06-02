# pixhaus-mod-sprite-edit

The sprite-editing module — the Draw workspace and the shared editing surface
(architecture bible sections 7.3, 6.3).

- **Registers:** the sprite document type, the canvas panel, the tool shelf and
  brush tools, the layer/palette/sprite panels, the Draw workspace, and the core
  sprite-editing commands.
- **Status:** stub.

## Boundaries

- This module owns the shared sprite-editing surface that the animation module
  also builds on. Keep frame-over-time concerns out — those belong to
  `mod-animation`. Draw is editing in space; Animate is editing in space over time.
- Tools interpret input and create commands; they never mutate the model directly.
- Don't fork the canvas or tools per workspace — they are shared capabilities.
- Instrument the editing commands (`#[instrument]` on apply) and the module's
  registration (`info!`). No per-stroke spans — a stroke is a hot input loop. See
  the `pixhaus-tracing` skill.

Shared module rules: `modules/CLAUDE.md`. Global rules: root `CLAUDE.md`.
