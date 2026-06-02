# pixhaus-ui

The egui contribution surface — workspace runtime, registries, and the Module
trait (architecture bible sections 4.2, 7, 8). The only crate that knows both
egui and `render`.

- **Owns:** the Panel/Tool/Workspace/Provider/Importer/Exporter/Validator traits,
  the registries, the `Module` trait, theme tokens, and the egui-to-`render`
  canvas paint callback.
- **Depends on:** `core`, `services`, `render`, `io`. External: `egui`,
  `egui-wgpu`, `wgpu`.
- **Used by:** the modules and `app`.
- **Status:** runnable spine — `CanvasCallback` and `install_canvas_renderer`.

## Boundaries

- This is the ONLY crate that may know both egui and `render`. Don't push egui
  types down into `core`/`render`/`io`/`services`.
- MUST NOT own durable project data or long-running jobs — those are
  `core`/`services`.
- Panels capture intent and display state; they request mutations through commands
  and never mutate the model directly.
- egui is the presentation layer, not the architecture — keep workspace business
  logic out of widget code.
- Keep tracing to the shell's coarse `debug!` / `warn!` (the existing intent and
  layout-resolve events). No per-frame tracing — the loop runs at 60fps and would
  flood the log. See the `pixhaus-tracing` skill.

## Design system

This crate owns the design system; every pixel of UI flows through it. When you
write or review UI code — here or in `modules/*` — these are rules, not suggestions.
Load the `pixhaus-ui-conventions` skill for the concrete API and do/don't examples.

- **Tokens, not literals.** Colors, spacing, radii, type, and elevation come from the
  theme tokens (`pixhaus_ui::theme`, or `scope.ctx.theme` / `cx.theme` in a panel or
  tool): `theme.surfaces.*`, `theme.roles.*`, `theme.accent.*`, `theme.spacing.*`,
  `theme.type_scale.*`, `theme.radius.*`, `theme.elevation.*`, or
  `theme.surface(SurfaceTier::…)`. Never a hex / `Color32::from_rgb` literal in
  panel/region/widget code. The violet accent is reserved for the active
  tab/tool/selection, primary buttons, and AI affordances — not decoration.
- **Shared widgets, not bespoke chrome.** Build panel chrome from `pixhaus_ui::widgets`
  (`card`, `section_header`, `tool_button`, `workspace_tab`, `tray_tab`,
  `mock_row`/`mock_thumbnail_grid`/`mock_log`). `card` already draws the panel title —
  don't re-draw it. Don't hand-roll frames that duplicate these.
- **Phosphor icons, never emoji.** Symbols come from `crate::icons::*`; emoji render as
  tofu in egui's fonts. `icons::SPARKLE` with `accent.ai` marks AI actions.
- **Brand via `crate::brand`.** `ICON`/`WORDMARK`/`LOGO` render with
  `egui::TextureOptions::NEAREST` (crisp pixel art); `install_image_loaders` runs once
  at boot.
- **Deferred-intent model.** Panels are `&self`: read through the read-only
  `ContribCtx`, push an `Intent` for all mutation (applied post-frame by
  `apply_intent`); the only mutable carve-out is `PanelScope.scratch` for a `TextEdit`.
  No panel-to-panel coupling. Concrete Panel/Tool/Workspace impls live in `modules/*`,
  not here — this crate owns the traits, registries, theme, widgets, and shell runtime.
- **egui 0.34 specifics.** `ctx.global_style_mut`/`global_style` (not
  `style_mut`/`style`); `egui::Panel::left/right/top/bottom` + `default_size`/`exact_size`;
  `ctx.text_edit_focused()` for shortcut focus-gating; no `.unwrap()`/`.expect()` even in
  tests (clippy `disallowed_methods`); `///` on every public item (`missing_docs = warn`).
- **Direction + verification.** The visual target is
  `docs/pixhaus_visual_ux_direction.md` (dark, dense production cockpit; manual-first,
  AI-assisted; the region elevation tiers; accent restraint). Before committing a UI
  change, render and compare: `cargo run -p pixhaus-app --example render_workspaces`
  writes `target/ui-snapshots/*.png`; check them against `docs/ui_visual_example/`.

Reach for `pixhaus-ui-conventions` (the design system), `pixhaus-egui`, and
`pixhaus-egui-wgpu` skills here. Global rules: root `CLAUDE.md`. Architecture:
`docs/pixhaus_architecture_bible.md`.
