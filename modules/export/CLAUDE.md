# pixhaus-mod-export

The export module — production output (architecture bible sections 7.3, 6.7, 19).

- **Registers:** the Export workspace, export validators and presets, and the
  spritesheet, PNG, GIF/video, and engine-metadata exporters.
- **Status:** stub.

## Boundaries

- Export is production discipline: validate before writing (bible section 19.4) —
  frame sizes, empty frames, stray transparency, color counts, missing animation
  coverage, naming conflicts.
- The engine target is Unity only. No Godot, Unreal, or GameMaker metadata.
- Codecs live in `io`; this module wires them to the workspace, presets, and
  validators. It doesn't reimplement encoding.

Shared module rules: `modules/CLAUDE.md`. Global rules: root `CLAUDE.md`.
