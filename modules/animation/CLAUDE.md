# pixhaus-mod-animation

The animation module — the Animate workspace and time (architecture bible
sections 7.3, 6.4, 15).

- **Registers:** animation clips, the timeline and onion-skin panels, playback
  controls, the Animate workspace, and the animation commands and export hooks.
- **Status:** stub.

## Boundaries

- Sibling to `mod-sprite-edit` over the same editing core — reuse its tools and
  canvas, don't fork them. Animate should feel like Draw plus time. Animating is
  editing the shared editing context over time (bible section 5.9) — the same
  context Draw uses.
- Animation belongs to sprites; it is not a separate document type. Frames,
  layers, cels, clips, and timing live on the sprite model in `core`.
- Onion skin renders through the shared canvas renderer, not a private one.
- Instrument the playback and onion-skin jobs and the module registration; keep the
  spans coarse (the job, not each frame composited inside it). See the
  `pixhaus-tracing` skill.
- Register the timeline/clip panels and the Animate workspace with keys
  (`workspace.animate.*`, `panel.timeline.title`); ship the values in
  `animation.yaml`. Reuse sprite-edit's shared keys — don't re-key the shared canvas.
  See the `pixhaus-i18n` skill.

- Record the why: when a choice here is made for a non-obvious reason — a
  trade-off, a rejected alternative, a constraint, or a workaround — state that
  reason in a `//` comment at each spot it shaped, not just in the commit. See the
  root `CLAUDE.md` "Recording decisions" rule.

Shared module rules: `modules/CLAUDE.md`. Global rules: root `CLAUDE.md`.
