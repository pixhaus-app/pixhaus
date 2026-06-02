---
name: pixhaus-i18n
description: >-
  Use when writing, reviewing, or debugging anything that puts text in front of a
  user in Pixhaus - adding a panel title, tool label, menu item, command, workspace
  name, status string, dialog text, or error message; choosing or naming an i18n
  key; calling the localization service (`tr` / `tr_args` / `tr_plural` /
  `set_language`); the `rust-i18n` `t!` macro, the `i18n!` init, the
  `crates/services/locales/*.yaml` bundles, interpolation (`%{name}`) or
  pluralization; the fallback language; the dev key-display toggle; or a "missing
  key" / "why is my label the raw key" / "the menu reordered" problem. Trigger
  whenever the question is "where does this string come from", "is this hardcoded",
  "what key do I use", "how do I switch language", "add a translation", or "register
  a panel/tool/menu" - even when the user does not say "i18n" or "localization". The
  ONE service lives in `crates/services` (`src/i18n.rs`); `app` sets the language at
  boot; `core` and `render` never localize; the shell resolves keys to text at
  render time. For where the locale dir lives on disk pair with `pixhaus-directories`;
  for the missing-key warning landing in the log, `pixhaus-tracing`; for the
  design-system rule that labels go through tokens-and-keys, `pixhaus-ui-conventions`.
---

# Pixhaus localization (i18n)

Strings are addressed by stable key and resolved to display text at render time, in
the active language, behind one Pixhaus-owned service. This is the string-side
parallel to "the binary owns the one tracing subscriber": there is exactly one place
the backing crate is wired up, and everyone else emits keys.

The backing crate is `rust-i18n` (4.x, MIT). It is the deliberate substitute for the
architecture bible's original `egui-i18n` candidate, which is abandoned. The service
boundary stays even if the backing crate changes (bible section 32.1), so never call
`rust_i18n::*` outside `crates/services/src/i18n.rs`.

## 1. The one architectural fact

One localization service, `crates/services/src/i18n.rs`, configured by `app` at boot.
Libraries and modules emit KEYS; the shell resolves them. A crate that hardcodes a
user-facing display string, or that calls `rust_i18n::set_locale` directly, is a bug.

- `crates/services/src/lib.rs` holds the single `rust_i18n::i18n!("locales", fallback = "en")`.
  The `t!` macro only works in the crate that called `i18n!`, which is why the service
  - and the locale files - live in `services`.
- `core` and `render` never localize (they store keys and ids, not display text).
- `ui` and `modules/*` reach the service through the free functions in
  `pixhaus_services::i18n`, or through `MsgKey::tr` (see rule 4).

## 2. The key scheme

Keys are dotted, in a module-owned namespace (bible section 32.2). Derive the key
from the existing stable id so it is predictable:

| Thing | Key | Owner |
|---|---|---|
| Workspace name / purpose | `workspace.<wsid>.title` / `.purpose` | the module |
| Workspace status item | `workspace.<wsid>.status.<slug>` | the module |
| Panel title | `panel.<panelid>.title` | the module |
| Tool label / tooltip | `tool.<toolid>.label` / `.tooltip` | the module |
| Action label | `command.<actionid>` (for `ActionId("layer.new")` -> `command.layer.new`) | the module |
| Menu group label | `app.menu.<group>` (also the group's stable identity) | shell or module |
| Shell chrome | `app.ui.<area>.<slug>` | the shell (`crates/ui`) |
| Error | `error.<domain>.<case>` | the surfacing module |

The shell owns `app.*`; each module owns its `workspace.*` / `panel.*` / `tool.*` /
`command.*` namespace. Keys are `&'static str` wrapped in `MsgKey`, written `const`
at registration time - no allocation, no locale touched.

## 3. No hardcoded user-facing strings

This is the string analogue of the design system's "tokens, not literals". Every
label, title, menu item, status phrase, and dialog string a user reads is a key
resolved through the service. Developer-facing text is NOT keyed: `tracing` messages,
`Debug` output, test assertions, and panics stay English.

Where the line sits in practice: a real phrase ("Layers", "Make seamless", "AI Ready")
is keyed. A numeric or mock placeholder shown only until `core` provides real data
(the mock `64 x 64` canvas size, the zoom `%`, the transitional canvas HUD, sample
layer names like `Layer 3`) is left as a literal - keying placeholder data is
premature. When in doubt, flip the dev toggle (rule 7): anything that does NOT turn
into a key was never routed through the service.

## 4. Registries carry keys; the shell resolves at render time

The contribution traits carry `MsgKey`, not display text:
`PanelMeta.title`, `ToolMeta.label`/`.tooltip`, `WorkspaceMeta.name`/`.purpose`,
`ActionDesc.label`, `MenuGroup.label`, `MenuItem.label` are all `MsgKey`.

```rust
// registration (in a module) - const, no allocation, no locale
fn meta(&self) -> PanelMeta {
    PanelMeta { title: MsgKey("panel.layers.title"), icon: icons::LAYERS, .. }
}

// render (in the shell) - resolve to the active language
ui.label(meta.title.tr());            // MsgKey::tr() -> String
ui.menu_button(group.label.tr(), ..);
```

`MsgKey` lives in `pixhaus_ui::contrib_api`; `MsgKey::tr(self) -> String` calls the
service. The newtype is the enforcement: a raw `MsgKey` cannot be handed to
`ui.label(..)`, only the resolved `String` from `.tr()` can, so "display a key
without resolving it" is a compile error. `StatusItem.text` is the one exception - it
is computed display `String`, set with `MsgKey("...").tr()` at `layout()` time.

## 5. The service API

Free functions in `pixhaus_services::i18n` (the active locale is process-global, so
there is no handle to thread):

```rust
i18n::tr("panel.layers.title")                         // -> "Layers"
i18n::tr_args("app.ui.palette.switch_to",
              &[("name", "Draw")])                     // "Switch to %{name}" -> "Switch to Draw"
i18n::tr_plural("panel.layers.count", 3)               // selects .one/.other, fills %{count}
i18n::set_language("es");                               // runtime switch
i18n::current_language()                                // -> "en"
i18n::available_languages()                             // -> Vec<String>
i18n::set_show_keys(true);                              // dev toggle (rule 7)
```

`tr` returns owned `String` (egui-free, no widget types). `tr_args` takes a runtime
`&[(&str, &str)]` slice because the `t!` macro needs literal arg names; the service
resolves the template then substitutes `%{name}` itself.

## 6. set_language and boot

`rust-i18n` keeps one process-global active locale; the service wraps it. `app`
resolves the language at boot - detect the OS language via `sys-locale`, fall back to
the saved `Prefs.language`, then to `en` - and calls `i18n::set_language` BEFORE the
egui loop, the string-side parallel to building the tracing subscriber before the
runtime. Runtime switching re-renders every label the next frame; no restart.

## 7. The dev key-display toggle (bible 32.3)

`i18n::set_show_keys(true)` makes `tr` / `tr_args` / `tr_plural` return the raw key
instead of its translation. Surfaced as `View > Show i18n Keys`. Use it as a built-in
lint: any visible string that does NOT become a key when the toggle is on is
hardcoded and bypassing the service.

## 8. Plurals and interpolation

Interpolation slots are `%{name}` (NOT `{name}`). Author them in the bundle and fill
them with `tr_args`:

```yaml
app.ui.palette.switch_to:
  en: "Switch to %{name}"
```

`rust-i18n` has NO CLDR plural categories, so plural form selection lives in the
service: `tr_plural(base, count)` picks `<base>.one` when `count == 1`, else
`<base>.other`, and fills `%{count}`. Author both forms:

```yaml
panel.layers.count.one:
  en: "%{count} layer"
panel.layers.count.other:
  en: "%{count} layers"
```

A backing crate with real plural categories would change only `tr_plural`, not
callers.

## 9. Missing-key diagnostics

`rust-i18n` returns the key string itself on a miss (no `Result`), so the service
detects `resolved == key`, walks the `en` fallback, and emits a
`tracing::warn!(target: "pixhaus::i18n", ...)` - then returns the key text. This
fires only on a genuine miss, never on the hit path: resolving `meta.title` runs every
frame, and the `crates/ui` rule bans per-frame tracing, so a successful `tr` is
silent.

## 10. The locale bundles

`crates/services/locales/*.yaml`, rust-i18n `_version: 2` (one file, each key mapping
locale -> value). One file per owning area: `app.yaml` (shell), `sprite_edit.yaml`,
`animation.yaml`, `tiles.yaml`, `generation.yaml`, `export.yaml`. `i18n!` embeds and
merges every file in the dir at compile time, so the "module owns its strings" rule
holds by file-name + key-namespace convention; duplicate keys with the same value
across files are harmless. (The bible names per-module locale dirs; with rust-i18n's
single-`i18n!` model the files are centralized in `services`. Moving them physically
into each module crate via a custom backend is a possible later refinement, like
runtime-loaded community translations - both deferred.)

## 11. Testing

- An rstest that walks `build_host`'s menus/panels/tools/workspaces and asserts every
  registered `MsgKey` resolves to non-key text in `en` (catches a key that exists as a
  constant but was never added to a bundle - the common drift).
- `set_language("es")` changes resolved output (ship a small `es` value for the key).
- The dev toggle returns the raw key.
- `tr_args` / `tr_plural` interpolate and select forms.
- insta `assert_yaml_snapshot!` on the resolved menu bar / palette list per language -
  a deterministic text snapshot, not a PNG diff.

THE FOOTGUN: `rust-i18n`'s active locale is process-global, so any test that calls
`set_language` must be `#[serial]` (the `serial_test` crate) and set its own starting
language - otherwise nextest's parallelism lets one test's locale bleed into another
that assumes `en`.

## 12. Never localize core; never put data or secrets in a key

`core` and `render` store keys and ids only - a renamed label must never invalidate a
saved project (bible 18.5 / 32.2). User content (file names, prompts, provider model
names) and secrets are interpolation ARGUMENTS or data, never key segments. A key is a
stable address, not a value: `tr_args("job.generate.running", &[("name", user_prompt)])`,
never `tr(&format!("job.generate.{user_prompt}"))`.

## Related

- `pixhaus-tracing` - the missing-key warning rides the one subscriber; the no-secrets rule.
- `pixhaus-directories` - where the locale/config dirs resolve on disk.
- `pixhaus-ui-conventions` - the design-system rule that labels are tokens-and-keys, not literals.
- `pixhaus-testing-conventions` - rstest / insta / serial_test.
- `crates/services/src/i18n.rs` (the code) and bible section 32 (the model).
