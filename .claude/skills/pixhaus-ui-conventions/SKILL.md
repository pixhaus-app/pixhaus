---
name: pixhaus-ui-conventions
description: Use when writing or reviewing any user-interface code in Pixhaus — panels, regions, widgets, tools, workspaces, the shell, theme or styling, icons, the canvas, the menu/status bars — in `crates/ui` or `modules/*`. This is the design-system floor: it covers the theme tokens (color, spacing, type, elevation), the shared `widgets` set, phosphor `icons` (never emoji), the brand assets, the deferred-intent panel model (`&self` panels, `ContribCtx`, `Intent`), the egui 0.34 house rules (`global_style_mut`, `egui::Panel::left/right`, no `unwrap`/`expect` even in tests), and the render-harness verification loop against the visual direction. Trigger whenever you touch how the app LOOKS or how a panel/tool/workspace is built, even when the user doesn't say "design system". For the verified egui API itself pair with `pixhaus-egui`; for general Rust style, `pixhaus-rust-conventions`.
---

# Pixhaus UI conventions

The design-system floor for every pixel of Pixhaus UI. Read this before writing or
reviewing UI in `crates/ui` or `modules/*`. The app has ONE design system — owned by
`crates/ui` — and consistency comes from everyone building on it instead of styling
ad hoc. The visual target is `docs/pixhaus_visual_ux_direction.md`; the reference
frames are in `docs/ui_visual_example/`.

Each rule below has the pattern, the counter-example, and the why. These are rules,
not suggestions — the per-crate `CLAUDE.md` files point here.

## 1. Tokens, not literals

Colors, spacing, radii, type sizes, and shadows come from the theme tokens — never a
hex or `Color32` literal in panel/region/widget code. Reach the tokens from
`pixhaus_ui::theme`, or inside a panel/tool from `scope.ctx.theme` / `cx.theme`.

The token surface (every field is `Copy`):

- `theme.surfaces.{app_frame, panel, elevated, stage, inset, hover}` — per-region
  background tiers, or `theme.surface(SurfaceTier::Elevated)`.
- `theme.roles.{border, text_primary, text_secondary, text_disabled, success, warning, error}`.
- `theme.accent.{base, hover, muted, on_accent, ai, ai_glow, tool_active_bg, seed}`.
- `theme.spacing.{xs, sm, md, lg, xl}` (2/4/8/12/16), `theme.type_scale.{label, body, section_header, title, mono}`,
  `theme.radius.{sm, md}`, `theme.elevation.{raised, overlay}`.

```rust
// DO
egui::Frame::new().fill(theme.surfaces.elevated).stroke(egui::Stroke::new(1.0, theme.roles.border))
// DON'T
egui::Frame::new().fill(egui::Color32::from_rgb(0x24, 0x22, 0x2b))  // a literal duplicates a token and breaks theming
```

The violet accent is reserved: active tab/tool/selection, primary buttons, and AI
affordances. It is not decoration. Why: one source of truth makes a theme swap (dark
/ light / accent) recolor the whole app for free, and keeps the WCAG floors the theme
tests enforce intact.

## 2. Shared widgets, not bespoke chrome

Build panel chrome from `pixhaus_ui::widgets`, not hand-rolled frames:

- `card(ui, theme, &PanelMeta, collapsed, body)` — the elevated panel card; it draws
  the header (icon + title + collapse chevron) and runs `body` when open. It already
  draws the panel title — do NOT call `section_header` with the same title inside
  (that was the doubled-header bug).
- `section_header(ui, theme, icon, title)` — an in-body sub-divider.
- `tool_button`, `workspace_tab`, `tray_tab` — the rail/tab affordances with their
  active states.
- `mock_row`, `mock_thumbnail_grid(n)`, `mock_log(lines)` — placeholder content while
  a panel has no real `core` data yet.

```rust
// DO
widgets::card(ui, theme, &meta, collapsed, |ui| { /* body */ });
// DON'T
let f = egui::Frame::new().fill(theme.surfaces.elevated) /* + header + chevron by hand */; // duplicates `card`
```

Why: the widgets carry the spacing, elevation, and active-state language. Re-rolling
them drifts the look one panel at a time.

## 3. Phosphor icons, never emoji

Symbols are phosphor glyphs from `crate::icons::*` (in `crates/ui`) or
`pixhaus_ui::icons::*` (in modules) — e.g. `icons::PENCIL`, `icons::LAYERS`,
`icons::WARN`. `icons::SPARKLE` with `theme.accent.ai` marks AI affordances.

```rust
// DO
ui.label(egui::RichText::new(icons::SPARKLE).color(theme.accent.ai));
// DON'T
ui.label("✨");  // emoji render as tofu boxes in egui's fonts
```

Why: egui's bundled fonts have no emoji; phosphor private-use codepoints render blank
unless `install_fonts` ran. The icon set is the vocabulary — adding a stray glyph or
emoji breaks the visual grammar.

## 4. Brand assets via `crate::brand`

The brand images are `crate::brand::{ICON, WORDMARK, LOGO}` (`egui::ImageSource`) plus
`brand::ICON_PNG` (raw bytes for the OS window icon). Render them with
`egui::TextureOptions::NEAREST` so the pixel art stays crisp at any size.
`install_image_loaders(ctx)` (re-exported from `pixhaus_ui`) must run once at boot, or
images show blank.

```rust
ui.add(egui::Image::new(crate::brand::ICON).texture_options(egui::TextureOptions::NEAREST));
```

## 5. The deferred-intent panel model

Panels are `&self` — they hold no mutable state. A `Panel::ui` gets a `PanelScope`
wrapping a read-only `ContribCtx { session, ui_state, theme, intents }`. The rules:

- Read state through `scope.ctx` (session / ui_state / theme). Read-only.
- For EVERY mutation, push an `Intent` into `scope.ctx.intents`. The shell applies it
  after the frame via `apply_intent`. Never mutate session or project state directly,
  and never reach for `RefCell`/`Cell`/`Mutex` to smuggle a write.
- The ONE mutable carve-out is `scope.scratch` (`&mut String`), for binding a
  `TextEdit`. Routing real state changes through `scratch` is a review failure.
- No panel-to-panel coupling. Panels communicate only through intents and the session
  state the shell owns.
- A tool's `options_ui` gets a bare `ContribCtx` (no scratch); same rules.

```rust
// DO
fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
    if ui.button("New Sprite").clicked() {
        scope.ctx.intents.push(Intent::RunAction(ActionId("sprite.new")));
    }
}
// DON'T: take &mut self, or mutate scope.ctx.session, or push a write through scratch.
```

Concrete `Panel` / `Tool` / `Workspace` impls live in `modules/*` and register through
the `Module` / `HostRegistrar` traits. `crates/ui` owns only the traits, registries,
theme, widgets, and shell runtime — never a concrete panel body. Why: the read-only
view + single write channel makes "panels never mutate state directly" a compiler
guarantee, and keeps `crates/ui` from becoming a god-object.

## 6. egui 0.34 house rules

- `ctx.global_style_mut(...)` / `ctx.global_style()` — NOT `style_mut` / `style`
  (deprecated). Re-apply a theme variant with `apply_to_visuals(theme, ctx)`.
- `egui::Panel::left/right/top/bottom(id)` with `.default_size` / `.exact_size` — the
  unified 0.34 panel API (`SidePanel` / `TopBottomPanel` are deprecated aliases).
- `ctx.text_edit_focused()` to gate single-key shortcuts while a field has focus.
- No `.unwrap()` / `.expect()` anywhere — `clippy.toml` `disallowed_methods` bans them
  EVEN IN TESTS. Use `match` / `let … else` / `assert!` / `panic!` in tests; `?` /
  `ok_or` / `unwrap_or` in code.
- `///` on every public item (`missing_docs = warn` under `-D warnings`).

For anything else about the egui API at the pinned version, load `pixhaus-egui` and
`pixhaus-egui-wgpu` rather than guessing — training data is several versions behind.

## 7. Direction and visual verification

The look is dark, dense, and production-cockpit (Blender density, Aseprite
immediacy), manual-first and AI-assisted, with clear region elevation tiers and
restrained accent. Read `docs/pixhaus_visual_ux_direction.md` before a structural UI
change.

Before committing a UI change, render and compare — do not eyeball the code alone:

```
cargo run -p pixhaus-app --example render_workspaces   # writes target/ui-snapshots/*.png
```

Open the PNGs and check them against the reference frames in
`docs/ui_visual_example/`. The harness renders every workspace (and the About modal +
splash) headlessly via egui_kittest, so a styling change is verifiable without a
manual run.

## Cross-references

- `pixhaus-egui` / `pixhaus-egui-wgpu` — the verified egui 0.34 API surface and the
  canvas paint callback.
- `pixhaus-rust-conventions` — general Rust style, error policy, the no-unwrap rule.
- `crates/ui/CLAUDE.md` — the same rules as the crate's boundaries; `docs/pixhaus_architecture_bible.md` — the structural source of truth.
