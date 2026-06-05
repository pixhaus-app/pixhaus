# pixhaus-mod-sprite-edit

The sprite-editing module — the Draw workspace and the shared editing surface
(architecture bible sections 7.3, 6.3).

- **Registers:** the sprite document type, the canvas panel, the tool shelf and
  brush tools, the layer/palette/sprite panels, the Draw workspace, and the core
  sprite-editing commands.
- **Status:** workspace registered. The Draw workspace, the shared panels, and
  the tool shelf are wired into the shell; the panel and tool bodies fill out as
  the roadmap (bible section 26) reaches them.

## Boundaries

- This module owns the shared sprite-editing surface that the animation module
  also builds on. Keep frame-over-time concerns out — those belong to
  `mod-animation`. Draw is editing in space; Animate is editing in space over time.
- Tools interpret input and create commands; they never mutate the model directly.
  They act through the shared editing context, not a private one — that is where the
  active document, selection, and tool state live (bible sections 5.9 and 22.7).
- Don't fork the canvas or tools per workspace — they are shared capabilities.
- Instrument the editing commands (`#[instrument]` on apply) and the module's
  registration (`info!`). No per-stroke spans — a stroke is a hot input loop. See
  the `pixhaus-tracing` skill.
- Register the shared panels, the 15 brush tools, and the Draw workspace with keys in
  its namespace (`panel.layers.title`, `tool.pencil.label`, `workspace.draw.title`,
  ...); ship the values in `sprite_edit.yaml`. Keep the shared panel/tool keys stable
  — other workspaces reference the same ids. See the `pixhaus-i18n` skill.

- Record the why: when a choice here is made for a non-obvious reason — a
  trade-off, a rejected alternative, a constraint, or a workaround — state that
  reason in a `//` comment at each spot it shaped, not just in the commit. See the
  root `CLAUDE.md` "Recording decisions" rule.

Shared module rules: `modules/CLAUDE.md`. Global rules: root `CLAUDE.md`.
