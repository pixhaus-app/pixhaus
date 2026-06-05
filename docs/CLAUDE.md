# docs/ — design references

Prose design references, not code. Nothing here is built, imported, or executed.

- **`pixhaus_architecture_bible.md`** — the structural source of truth. Read it
  before any structural decision; the crate graph and the root `CLAUDE.md`
  Architecture section are derived from it. It also holds the runtime, state-bucket,
  concurrency/execution-lane, and localization model (bible sections 22 and 31-33);
  the localization model is bible section 32, backed by `rust-i18n` and implemented
  in `crates/services/src/i18n.rs`. See the `pixhaus-i18n` skill.
- **`pixhaus_save_file_format_architecture.md`** — the project/save format
  direction; feeds the `io` crate and bible section 18.
- **`pixhaus_visual_ux_direction.md`** — the visual and UX direction.
- **`ui_visual_example/`** — reference frames illustrating the visual direction.

## Boundaries

- Read-only reference. Don't wire code to these files; turn a decision here into
  code, a test, or a doc comment in the owning crate.
- Keep it prose and durable. Transient task notes don't belong here.
- When code and a reference disagree on architecture, the bible wins — fix the
  code or surface the conflict. For non-architecture conflicts, surface them.

- Record the why in code: when a reference here drives a non-obvious
  implementation choice, the reason travels into a `//` comment in the owning crate
  at each spot it shaped — the prose here is the source, the code comment is where
  the next reader meets it. See the root `CLAUDE.md` "Recording decisions" rule.

Global rules: root `CLAUDE.md`.
