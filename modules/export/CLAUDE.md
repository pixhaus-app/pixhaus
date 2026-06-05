# pixhaus-mod-export

The export module — production output (architecture bible sections 7.3, 6.7, 19).

- **Registers:** the Export workspace, export validators and presets, and the
  spritesheet, PNG, GIF/video, and engine-metadata exporters.
- **Status:** workspace registered. The Export workspace and its panels are wired
  into the shell; the panel bodies fill out as the roadmap (bible section 26)
  reaches them.

## Boundaries

- Export is production discipline: validate before writing (bible section 19.4) —
  frame sizes, empty frames, stray transparency, color counts, missing animation
  coverage, naming conflicts.
- The engine target is Unity only. No Godot, Unreal, or GameMaker metadata.
- Codecs live in `io`; this module wires them to the workspace, presets, and
  validators. It doesn't reimplement encoding.
- `#[instrument]` the validators and the encode jobs — the encode span is the perf
  signal here. `warn!` on a failed validation (the actionable findings above). See
  the `pixhaus-tracing` skill.
- Register the Export workspace, the presets, and the validators with keys
  (`workspace.export.*`, `export.<fmt>.label`, `command.export.*`); ship the values
  in `export.yaml`. Validation findings surfaced to the user are keyed strings, never
  English literals in the validator. See the `pixhaus-i18n` skill.
- Encode and validation jobs follow the background-worker contract (bible section
  13.6), and a failed export feeds the diagnostic bundle (bible section 24.5).

- Record the why: when a choice here is made for a non-obvious reason — a
  trade-off, a rejected alternative, a constraint, or a workaround — state that
  reason in a `//` comment at each spot it shaped, not just in the commit. See the
  root `CLAUDE.md` "Recording decisions" rule.

Shared module rules: `modules/CLAUDE.md`. Global rules: root `CLAUDE.md`.
