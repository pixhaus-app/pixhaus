# pixhaus-mod-codex

The Codex module — the project-level creative bible as a workspace (Codex bible
sections 8, 9.2, 14, 21).

- **Registers:** the Codex workspace (`workspace.codex.title`, Cmd+6, a non-canvas
  layout — a left Navigator, a full-center Entry Editor / Visual Board / Graph
  surface, a right Inspector, and a Coverage / Test Generation / History bottom
  strip), its six panels (`panel.codex-navigator.title`, `panel.codex-editor.title`,
  `panel.codex-inspector.title`, `panel.codex-coverage.title`,
  `panel.codex-test-generation.title`, `panel.codex-history.title`), and the
  `codex.*` actions (`codex.new-entry` → `command.codex.add_entry`,
  `codex.compile-prompt` → `command.codex.compile_prompt`).
- **Status:** workspace registered. The Navigator, Entry Editor, Inspector, and the
  Coverage/Test/History tray panels are wired into the shell with real bodies; the
  Visual Board and Graph center surfaces and the live test-generation dispatch fill
  out as the roadmap (bible section 26) reaches them.

## Boundaries

- The Codex remembers, AI proposes, the artist decides. Every model change routes
  through a core command via an `Intent`; no panel mutates the document. AI output
  is a proposal, never a direct write to the bible.
- Panels are `&self` unit structs that read the read-only `CodexView` mirror the
  shell rebuilds each frame and push `Intent`s. The center editor edits a
  shell-owned `CodexEditorDraft`, not the document — the draft is committed through a
  command, never written behind the model's back.
- The Codex is a library-like, non-canvas workspace: it has no pixel tools and
  shares no canvas. It composes entries, anchors, `@`-refs, the prompt compiler, and
  coverage — it does not edit sprites. Sprite editing stays in `mod-sprite-edit`.
- The Codex owns no `core` data type of its own beyond what the bible's Codex model
  defines; entry/anchor/coverage state lives on the model in `core`, mutated only
  through registered commands.
- Instrument the module registration (`info!`) and any coverage/test-generation
  jobs (`#[instrument]` on the bodies — the job duration is the perf signal); keep
  the spans coarse. See the `pixhaus-tracing` skill.
- Register the workspace, the six panels, and the `codex.*` actions with keys in
  this module's namespace (`workspace.codex.*`, `panel.codex-*.title`,
  `command.codex.*`, `codex.status.*`, and the other `codex.*` enum-mapper keys);
  ship the values in `codex.yaml`. Entry names, notes, prompt fragments, palette
  RGBA, and seeds are DATA, never i18n keys — they are project content the artist
  authors, interpolated as args, never translated. See the `pixhaus-i18n` skill.
- Build the workspace UI from the `ui` design system — theme tokens via
  `ContribCtx.theme`, the `widgets` helpers (including the codex widgets in
  `crates/ui`), `icons` glyphs — never hex colors, emoji, or bespoke frames. Follow
  `docs/pixhaus_visual_ux_direction.md` and verify the look with the render harness.

- Record the why: when a choice here is made for a non-obvious reason — a
  trade-off, a rejected alternative, a constraint, or a workaround — state that
  reason in a `//` comment at each spot it shaped, not just in the commit. See the
  root `CLAUDE.md` "Recording decisions" rule.

Shared module rules: `modules/CLAUDE.md`. Global rules: root `CLAUDE.md`.
