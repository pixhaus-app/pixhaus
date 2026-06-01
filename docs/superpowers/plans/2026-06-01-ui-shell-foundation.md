# Pixhaus v3 UI shell foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the registry-driven UI shell foundation - all seven regions across five workspaces with placeholder panels, the Panel/Tool/Workspace/Module trait surface, the registries, project/session/UI state separation, and fresh dark/violet theme tokens. No core dependency.

**Architecture:** `crates/ui` owns the trait surface, registries, theme, shell runtime, and shared widgets; the five `modules/*` crates register concrete workspaces/panels/tools; `app` composes the shell purely by registering modules. Panels are `&self` with a read-only state view plus one intent sink; mutations drain after the frame.

**Tech Stack:** Rust, egui/eframe 0.34, egui-wgpu, egui-phosphor, wgpu, serde, thiserror, tracing; tests via cargo nextest + insta + rstest. cargo runs through PowerShell on this machine (the Bash tool links a Git-Bash link.exe that shadows the MSVC linker).

**Source spec:** `docs/superpowers/specs/2026-06-01-ui-shell-foundation-design.md` is the single source of truth for every signature and decision. Where this plan and the spec disagree on a type name, the spec wins.

---

## Build order and dependencies

Execute the phases in this order. The five `modules/*` crates and `app` come last because they depend on the whole `ui` crate.

1. **Setup and dependencies** - workspace members, egui-phosphor ratification, the ui module skeleton, icons.
2. **Theme tokens** - standalone; compiles and unit-tests on its own.
3. **Contribution trait surface** (`contrib_api`).
4. **Registries and region**.
5. **State and intents**.
6. **Shared widgets**.
7. **Shell runtime and regions** - the borrow-safe per-frame loop; depends on 3-6 + theme.
8. **Module crates and app wiring** - depends on everything in `ui`.
9. **Tests and integration** - the full spec test plan plus the Stop-gate and the PR.

Intra-crate note: the `pixhaus-ui` modules are mutually referential (contrib_api context borrows state types, the registry stores contrib_api traits, `Host` owns the registries). The crate therefore reaches a clean `cargo build -p pixhaus-ui` only once the contrib_api + registry + state cluster (phases 3-5) is in place. Phase 2 (theme) compiles and tests on its own; within phases 3-5, a per-file build step that references a not-yet-written sibling module will report an unresolved-import error that clears at the end of phase 5 - treat the build step at the end of the State phase as the first true compile gate for the crate, and let earlier forward-reference errors stand until then. Every phase from 6 onward builds clean after its tasks.

Commit discipline: one commit per task (or per tight task group), Conventional Commits, scope = crate/area, each message body ending with the `Co-Authored-By` trailer. Branch is already `feat/ui-shell-foundation`.

---

## Phase 1: Setup and dependencies

All the constant names I need are confirmed present in egui-phosphor 0.12.0 `regular`. I now have everything verified:

- egui-phosphor 0.12 (egui 0.34 compatible)
- Shadow fields: `offset: [i8; 2]`, `blur: u8`, `spread: u8`, `color: Color32`
- Phosphor `regular::` constant names for every icon the plan references

I have all the context I need. Now I'll write the SETUP layer of the implementation plan.

## SETUP layer - Workspace + crate scaffolding and dependencies

Implements the dependency catalog ratification and crate wiring from the spec's "ui-crate module tree and crate wiring" + "modules/* and app wiring" sections, the `icons.rs` from "Fonts and icons" (spec lines 845-854), and the egui-phosphor decision (spec lines 23-24, 1084-1085). This layer makes `cargo build --workspace` pass so every later layer has a compiling skeleton to fill.

Verified during research (do not re-verify, these are settled):
- egui-phosphor 0.12.0 declares `egui = "0.34"` - pin `egui-phosphor = "0.12"`.
- `egui::epaint::Shadow` 0.34.2 fields are `offset: [i8; 2]`, `blur: u8`, `spread: u8`, `color: Color32` (used by the THEME layer, surfaced here for reference).
- All phosphor glyph constants below exist in `egui_phosphor::regular` 0.12.0.

The branch `feat/ui-shell-foundation` already exists and is checked out. Do not create a branch. All cargo commands run through PowerShell (the Bash tool's Git-Bash `link.exe` shadows the MSVC linker).

---

### SETUP.1: Ratify egui-phosphor in the root dependency catalog

**Files:**
- Modify: `Cargo.toml` (repo root)

- [ ] **Step 1: Add the egui-phosphor pin to the workspace dependency catalog.** Open `Cargo.toml` at the repo root. In the `[workspace.dependencies]` table, inside the egui/wgpu lockstep block, add the `egui-phosphor` line directly after the `egui_extras` line (it moves in lockstep with the egui family). Replace this exact block:

```toml
egui = "0.34"
egui_extras = "0.34"
egui-wgpu = "0.34"
```

with:

```toml
egui = "0.34"
egui_extras = "0.34"
# Phosphor icon glyphs merged into egui's font set (tool-rail glyphs + AI sparkle).
# Ratified in the ui-shell-foundation design; 0.12 declares egui = "0.34".
egui-phosphor = "0.12"
egui-wgpu = "0.34"
```

- [ ] **Step 2: Confirm the manifest still parses.** Run in PowerShell:

```powershell
cargo metadata --no-deps --format-version 1 | Out-Null; if ($?) { "OK" }
```

Expected: prints `OK` with no error. (This resolves the manifest without building; it confirms the new dependency line is well-formed and the version resolves.)

- [ ] **Step 3: Commit.** This is a pure-manifest change with nothing to assert beyond "it parses", so the write -> resolve -> commit rhythm is the honest one here. Run in PowerShell:

```powershell
git add Cargo.toml; git commit -m @'
deps: ratify egui-phosphor 0.12 in the workspace catalog

The ui-shell-foundation design ratifies egui-phosphor for the tool-rail
glyphs and the AI sparkle marker. 0.12 declares egui = "0.34", so it moves
in lockstep with the pinned egui family. egui_kittest stays declined.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### SETUP.2: Add egui-phosphor and the missing deps to crates/ui/Cargo.toml

**Files:**
- Modify: `crates/ui/Cargo.toml`

The spec needs `egui-phosphor` (icons + font merge), `serde` (the `ThemeVariant`/`Prefs` derives), `thiserror` (library error policy), and `tracing` (the `resolve_layout` warn, the `drain_background`/intent debug sinks). `egui`, `egui-wgpu`, `wgpu` are already present. `pixhaus-render` stays (the canvas seam). No `core`/`services`/`io` deps this round - `core` is an empty stub and the spec references nothing from it.

- [ ] **Step 1: Add the dependencies.** In `crates/ui/Cargo.toml`, replace the `[dependencies]` block:

```toml
[dependencies]
egui.workspace = true
egui-wgpu.workspace = true
wgpu.workspace = true
pixhaus-render = { path = "../render" }
```

with:

```toml
[dependencies]
egui.workspace = true
egui-wgpu.workspace = true
egui-phosphor.workspace = true
wgpu.workspace = true
serde.workspace = true
thiserror.workspace = true
tracing.workspace = true
pixhaus-render = { path = "../render" }

[dev-dependencies]
rstest.workspace = true
insta.workspace = true
```

(The `[dev-dependencies]` block lands now so the test layers don't have to touch the manifest later. `rstest` drives the registry/intent/theme/shortcut tests; `insta` drives the layout snapshots.)

- [ ] **Step 2: Confirm the crate still resolves.** Run in PowerShell:

```powershell
cargo build -p pixhaus-ui
```

Expected: PASS (compiles clean - the existing `lib.rs` is unchanged, the new deps are unused-but-declared at this point, which is not a warning for declared-not-imported crates).

- [ ] **Step 3: Commit.** Run in PowerShell:

```powershell
git add crates/ui/Cargo.toml; git commit -m @'
ui: add phosphor, serde, thiserror, tracing deps

The shell foundation needs egui-phosphor for the icon font merge, serde for
the ThemeVariant and Prefs derives, thiserror for the library error policy,
and tracing for the resolve_layout warn and the intent debug sinks. Adds
rstest and insta as dev-deps for the registry and layout-snapshot tests.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### SETUP.3: Wire the five module crates' Cargo.toml dependencies

**Files:**
- Modify: `modules/sprite-edit/Cargo.toml`
- Modify: `modules/animation/Cargo.toml`
- Modify: `modules/tiles/Cargo.toml`
- Modify: `modules/generation/Cargo.toml`
- Modify: `modules/export/Cargo.toml`

The five module crates already exist as workspace members (the `members = ["crates/*", "modules/*", "app"]` glob in the root `Cargo.toml` picks them up - confirmed present). They are doc-only stubs with no `[dependencies]` block. Each needs `pixhaus-ui` (the trait surface), `egui` (the panel/tool `ui` signatures), and `egui-phosphor` (icon glyphs in `*Meta`). Per `modules/CLAUDE.md` the ceiling is `core + services + ui`; this round each depends only on `ui` + egui, which is within it. No `core`/`services` deps - they are stubs the spec references nothing from.

- [ ] **Step 1: Add the dependency block to sprite-edit.** In `modules/sprite-edit/Cargo.toml`, between the `homepage.workspace = true` line and the `[lints]` block, insert:

```toml
[dependencies]
egui.workspace = true
egui-phosphor.workspace = true
pixhaus-ui = { path = "../../crates/ui" }
```

- [ ] **Step 2: Add the identical block to animation.** In `modules/animation/Cargo.toml`, insert the same block in the same place:

```toml
[dependencies]
egui.workspace = true
egui-phosphor.workspace = true
pixhaus-ui = { path = "../../crates/ui" }
```

- [ ] **Step 3: Add the identical block to tiles.** In `modules/tiles/Cargo.toml`, insert:

```toml
[dependencies]
egui.workspace = true
egui-phosphor.workspace = true
pixhaus-ui = { path = "../../crates/ui" }
```

- [ ] **Step 4: Add the identical block to generation.** In `modules/generation/Cargo.toml`, insert:

```toml
[dependencies]
egui.workspace = true
egui-phosphor.workspace = true
pixhaus-ui = { path = "../../crates/ui" }
```

- [ ] **Step 5: Add the identical block to export.** In `modules/export/Cargo.toml`, insert:

```toml
[dependencies]
egui.workspace = true
egui-phosphor.workspace = true
pixhaus-ui = { path = "../../crates/ui" }
```

- [ ] **Step 6: Confirm all five module crates resolve and compile as stubs.** Run in PowerShell:

```powershell
cargo build -p pixhaus-mod-sprite-edit -p pixhaus-mod-animation -p pixhaus-mod-tiles -p pixhaus-mod-generation -p pixhaus-mod-export
```

Expected: PASS. Each crate is still a doc-only `lib.rs`, so they compile with the new deps declared but unused.

- [ ] **Step 7: Commit.** Run in PowerShell:

```powershell
git add modules/sprite-edit/Cargo.toml modules/animation/Cargo.toml modules/tiles/Cargo.toml modules/generation/Cargo.toml modules/export/Cargo.toml; git commit -m @'
deps: wire the five wired module crates to pixhaus-ui

sprite-edit, animation, tiles, generation, and export each depend on
pixhaus-ui plus egui and egui-phosphor for their panel/tool/workspace impls.
Within the core+services+ui module ceiling. The core, pixel-art, and
providers modules stay unwired this round - they need core/services bodies.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### SETUP.4: Wire app/Cargo.toml to the five module crates

**Files:**
- Modify: `app/Cargo.toml`

`app` composes the shell by naming the five module structs (spec lines 144-164). It already depends on `pixhaus-ui`; it needs the five `pixhaus-mod-*` crates so `build_host` can `Box::new(...)` each module. The other layer that edits `app/src/main.rs` (the APP/wiring layer) assumes these deps exist.

- [ ] **Step 1: Add the module crate dependencies.** In `app/Cargo.toml`, replace the `[dependencies]` block:

```toml
[dependencies]
anyhow.workspace = true
eframe.workspace = true
egui.workspace = true
egui-wgpu.workspace = true
tokio.workspace = true
tracing-subscriber.workspace = true
pixhaus-ui = { path = "../crates/ui" }
```

with:

```toml
[dependencies]
anyhow.workspace = true
eframe.workspace = true
egui.workspace = true
egui-wgpu.workspace = true
tokio.workspace = true
tracing-subscriber.workspace = true
pixhaus-ui = { path = "../crates/ui" }
pixhaus-mod-sprite-edit = { path = "../modules/sprite-edit" }
pixhaus-mod-animation = { path = "../modules/animation" }
pixhaus-mod-tiles = { path = "../modules/tiles" }
pixhaus-mod-generation = { path = "../modules/generation" }
pixhaus-mod-export = { path = "../modules/export" }
```

- [ ] **Step 2: Confirm app still builds (main.rs unchanged, new deps unused).** Run in PowerShell:

```powershell
cargo build -p pixhaus-app
```

Expected: PASS. The current `main.rs` does not yet reference the module crates, so they are declared-but-unused dependencies (not a warning for whole-crate deps).

- [ ] **Step 3: Commit.** Run in PowerShell:

```powershell
git add app/Cargo.toml; git commit -m @'
app: depend on the five wired module crates

build_host names sprite-edit, animation, tiles, generation, and export to
register them with the host. Adds the path deps; main.rs wiring lands with
the app composition layer.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### SETUP.5: Create the crates/ui module-tree skeleton (stub mod.rs files)

**Files:**
- Modify: `crates/ui/src/lib.rs`
- Create: `crates/ui/src/theme/mod.rs`
- Create: `crates/ui/src/region.rs`
- Create: `crates/ui/src/contrib_api/mod.rs`
- Create: `crates/ui/src/registry/mod.rs`
- Create: `crates/ui/src/state/mod.rs`
- Create: `crates/ui/src/shell/mod.rs`
- Create: `crates/ui/src/widgets/mod.rs`
- Create: `crates/ui/src/icons.rs` (filled in SETUP.6; created empty here so the `mod icons;` declaration compiles)

This layer creates the directory skeleton with minimal stub `mod.rs` files so `cargo build` passes after each task. **Decision (stated explicitly per the task brief): this layer creates only the top-level module files declared in `lib.rs`. Each later layer fills its own submodule files** (theme owns `tokens.rs`/`palettes.rs`/etc., contrib_api owns `ids.rs`/`context.rs`/etc.). The stub `mod.rs` files here declare nothing internal - they are empty (or carry only a `//!` doc line) so the crate compiles. The owning layer replaces the stub body and adds its `pub mod` declarations.

`lib.rs` keeps `install_canvas_renderer` + `CanvasCallback` + the existing test exactly as-is; this step only adds the module declarations and a doc note. The re-exports the spec shows (`pub use` of `Host`, `Theme`, `Module`, etc.) are NOT added here - the types do not exist yet and a re-export of a missing item fails to compile. Each owning layer adds its own re-export to `lib.rs` when its type lands. This step only declares the module tree.

- [ ] **Step 1: Create the eight stub module files.** Each is a one-line doc-comment stub so the `mod`/`pub mod` declarations in `lib.rs` resolve. Create exactly these files with exactly this content.

`crates/ui/src/theme/mod.rs`:

```rust
//! Theme tokens and the token-to-egui-`Visuals` mapping.
//!
//! Filled by the theme layer: `Theme`, `ThemeVariant`, `apply_to_visuals`,
//! `install_fonts`, and the token structs in `tokens`/`palettes`/`contrast`.
```

`crates/ui/src/region.rs`:

```rust
//! The seven window regions and their stable egui `Id` strings.
//!
//! Filled by the region layer: the `Region` enum and the `region_id` constants.
```

`crates/ui/src/contrib_api/mod.rs`:

```rust
//! The permanent contribution trait surface.
//!
//! Filled by the contrib-api layer: the identity newtypes, `ContribCtx` /
//! `PanelScope`, and the `Panel` / `Tool` / `Workspace` / `Module` traits.
```

`crates/ui/src/registry/mod.rs`:

```rust
//! The capability registries and layout resolution.
//!
//! Filled by the registry layer: `Registry`, `Registries`, the `HostRegistrar`
//! impl, `ResolvedLayout`, and `resolve_layout`.
```

`crates/ui/src/state/mod.rs`:

```rust
//! Host, session, and UI state plus the intent/event model.
//!
//! Filled by the state layer: `Host`, `SessionState`, `UiState`, `Intent`,
//! `Event`, `IntentSink`, and `apply_intent`.
```

`crates/ui/src/shell/mod.rs`:

```rust
//! The per-frame shell runtime and region composition.
//!
//! Filled by the shell layer: `Shell::run`, `drain_background`, the region
//! modules, the command palette, shortcuts, and the menu structure.
```

`crates/ui/src/widgets/mod.rs`:

```rust
//! Shared egui widget helpers used across regions and panels.
//!
//! Filled by the widgets layer: `panel_card`, `tool_button`, `workspace_tab`,
//! `tray_tab`, `section_header`, and the placeholder helpers.
```

`crates/ui/src/icons.rs` (empty stub - SETUP.6 fills it; an empty file is a valid module):

```rust
//! Phosphor glyph `char` constants used across the shell.
//!
//! Filled by the icons step of the setup layer.
```

- [ ] **Step 2: Declare the module tree in lib.rs.** In `crates/ui/src/lib.rs`, replace the existing crate-level doc-comment block (lines 1-11, ending at the `//! workspaces are built.` line) and the first `use` line region so the module declarations sit directly under the doc comment. Specifically, after the closing line of the `//!` doc block:

```rust
//! workspaces are built.
```

insert these declarations (a blank line, then the modules, then a blank line before the existing `use egui::epaint::PaintCallbackInfo;`):

```rust

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod theme;
pub mod contrib_api;
pub mod registry;
pub mod region;
pub mod state;
pub mod shell;
pub mod widgets;
mod icons;
```

The result is: the existing `//!` doc block, then the `#![cfg_attr(...)]` inner attribute (required by the test-allow convention; it must be an inner attribute at the crate root so it goes here, after the doc comment, before any items), then the module declarations, then the unchanged `use` lines and the unchanged `install_canvas_renderer` / `CanvasCallback` / `#[cfg(test)] mod tests` below.

Note: `mod icons;` is private (`crate::icons::*` is internal per the spec - "alias them as pub const in crate::icons"). The `icons` module is referenced as `crate::icons::FOO` from within the `ui` crate, never re-exported. The phosphor constants inside it are `pub const`, so they are reachable crate-internally via `crate::icons::PENCIL` while the module stays crate-private. To avoid a dead-code warning on an unused private module while later layers are still being built, the `icons.rs` filled in SETUP.6 carries `#![allow(dead_code)]` until a consumer references it (see SETUP.6).

- [ ] **Step 3: Build the ui crate with the new module tree.** Run in PowerShell:

```powershell
cargo build -p pixhaus-ui
```

Expected: PASS. All eight stub modules are empty doc stubs, the `#![cfg_attr]` is inert outside tests, and the existing seam code is unchanged. There is nothing to assert here beyond "it compiles", so write -> build -> commit is the honest rhythm (no test step - the existing `canvas_callback_is_zero_sized` test still runs and still passes).

- [ ] **Step 4: Confirm the existing test still passes.** Run in PowerShell:

```powershell
cargo nextest run -p pixhaus-ui
```

Expected: PASS - `canvas_callback_is_zero_sized` runs green (1 test passed). This confirms the module-tree additions did not disturb the preserved seam.

- [ ] **Step 5: Commit.** Run in PowerShell:

```powershell
git add crates/ui/src/lib.rs crates/ui/src/theme/mod.rs crates/ui/src/region.rs crates/ui/src/contrib_api/mod.rs crates/ui/src/registry/mod.rs crates/ui/src/state/mod.rs crates/ui/src/shell/mod.rs crates/ui/src/widgets/mod.rs crates/ui/src/icons.rs; git commit -m @'
ui: declare the shell-foundation module tree

Adds the theme/contrib_api/registry/region/state/shell/widgets module
declarations and the crate-private icons module to lib.rs, each backed by a
doc-stub mod.rs that later layers fill. The install_canvas_renderer and
CanvasCallback seam is preserved unchanged. Adds the test-only unwrap/expect
allow at the crate root per the testing convention.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### SETUP.6: Fill crates/ui/src/icons.rs with the phosphor glyph constants

**Files:**
- Modify: `crates/ui/src/icons.rs`
- Test: `crates/ui/src/icons.rs` (inline `#[cfg(test)] mod tests`)

The phosphor `char` constants every later layer references as `crate::icons::*` (spec lines 71, 845-854; tool inventory lines 917-920; panel/menu/status glyphs throughout "Panel mock content" and "Per-region tiers"). Each is a re-alias of an `egui_phosphor::regular::*` `&str` constant turned into a `char`. **Verified:** every `egui_phosphor::regular` constant named below exists in egui-phosphor 0.12.0.

Phosphor exposes each glyph as a `&'static str` (a one-`char` string). Pixhaus's `PanelMeta.icon` / `ToolMeta.icon` / `WorkspaceMeta.icon` are typed `char` (spec lines 290, 318, 339). So each alias converts the phosphor `&str` to its single `char` at compile time. `&str` cannot be indexed to a `char` in a `const`, so use a small `const fn` that reads the first byte run as a `char` via `str::as_bytes` is not enough (phosphor glyphs are multi-byte UTF-8). The correct const-evaluable conversion is `char::from_u32` on the decoded scalar - but the simplest correct approach that works in `const` for these private-use single-char strings is to declare each constant by writing the `char` form. Since phosphor only ships `&str`, use a `const fn` that returns the first (and only) `char` of the string.

There is a verified-in-0.34-ecosystem helper for exactly this. Implement a tiny `const fn first_char(s: &str) -> char` and alias through it.

- [ ] **Step 1: Write the icons module with the const-char aliases.** Replace the entire contents of `crates/ui/src/icons.rs` with:

```rust
//! Phosphor glyph `char` constants used across the shell.
//!
//! Every constant here re-aliases an `egui_phosphor::regular::*` glyph (shipped
//! as a one-char `&str`) into the `char` form that `PanelMeta`/`ToolMeta`/
//! `WorkspaceMeta` icons require. No emoji literals anywhere: egui's default
//! fonts render emoji as tofu, and phosphor private-use codepoints render blank
//! until `theme::fonts::install_fonts` merges the phosphor family.
//!
//! `crate::icons` is crate-private; later layers reference these as
//! `crate::icons::PENCIL`. The `allow(dead_code)` keeps the build warning-clean
//! while the consuming layers are still landing.
#![allow(dead_code)]

use egui_phosphor::regular as ph;

/// First `char` of a phosphor glyph string, evaluated at compile time.
///
/// Phosphor ships each glyph as a single-`char` `&str`; our metadata structs
/// take `char`. A phosphor glyph is one private-use scalar, so the first decoded
/// `char` is the glyph.
const fn glyph(s: &str) -> char {
    // `str::as_bytes` is const; decode the leading UTF-8 sequence to one scalar.
    let b = s.as_bytes();
    let first = b[0];
    if first < 0x80 {
        // single-byte ASCII (the `X` close glyph among others)
        first as char
    } else if first >> 5 == 0b110 {
        let cp = ((first as u32 & 0x1f) << 6) | (b[1] as u32 & 0x3f);
        match char::from_u32(cp) {
            Some(c) => c,
            None => '\u{fffd}',
        }
    } else if first >> 4 == 0b1110 {
        let cp = ((first as u32 & 0x0f) << 12)
            | ((b[1] as u32 & 0x3f) << 6)
            | (b[2] as u32 & 0x3f);
        match char::from_u32(cp) {
            Some(c) => c,
            None => '\u{fffd}',
        }
    } else {
        let cp = ((first as u32 & 0x07) << 18)
            | ((b[1] as u32 & 0x3f) << 12)
            | ((b[2] as u32 & 0x3f) << 6)
            | (b[3] as u32 & 0x3f);
        match char::from_u32(cp) {
            Some(c) => c,
            None => '\u{fffd}',
        }
    }
}

// --- Tool-rail glyphs (spec tool inventory) ---
pub const PENCIL: char = glyph(ph::PENCIL);
pub const ERASER: char = glyph(ph::ERASER);
pub const FILL: char = glyph(ph::PAINT_BUCKET);
pub const LINE: char = glyph(ph::LINE_SEGMENT);
pub const RECT: char = glyph(ph::RECTANGLE);
pub const ELLIPSE: char = glyph(ph::CIRCLE);
pub const EYEDROPPER: char = glyph(ph::EYEDROPPER);
pub const SELECT: char = glyph(ph::SELECTION);
pub const LASSO: char = glyph(ph::LASSO);
pub const MOVE: char = glyph(ph::ARROWS_OUT_CARDINAL);
pub const TRANSFORM: char = glyph(ph::FRAME_CORNERS);
pub const TEXT: char = glyph(ph::TEXT_T);
pub const HAND: char = glyph(ph::HAND);
pub const ZOOM: char = glyph(ph::MAGNIFYING_GLASS);
/// The AI sparkle marker. Used wherever `AccentTokens::ai` applies.
pub const SPARKLE: char = glyph(ph::SPARKLE);

// --- Panel / dock glyphs ---
pub const LAYERS: char = glyph(ph::STACK);
pub const SPRITES: char = glyph(ph::IMAGES);
pub const PALETTE: char = glyph(ph::PALETTE);
pub const FRAMES: char = glyph(ph::FILM_STRIP);
pub const ASSETS: char = glyph(ph::SQUARES_FOUR);
pub const CONSOLE: char = glyph(ph::TERMINAL);
pub const TIMELINE: char = glyph(ph::FILM_SLATE);
pub const TILESET: char = glyph(ph::GRID_NINE);
pub const PROMPT: char = glyph(ph::MAGIC_WAND);
pub const RESULTS: char = glyph(ph::IMAGE);
pub const HISTORY: char = glyph(ph::LIST_BULLETS);
pub const EXPORT: char = glyph(ph::EXPORT);
pub const SETTINGS: char = glyph(ph::GEAR);

// --- Workspace tab glyphs ---
pub const DRAW: char = PENCIL;
pub const ANIMATE: char = glyph(ph::FILM_STRIP);
pub const TILES: char = glyph(ph::GRID_FOUR);
pub const GENERATE: char = SPARKLE;
pub const EXPORT_WS: char = glyph(ph::EXPORT);

// --- Status / menu / control glyphs ---
pub const EYE: char = glyph(ph::EYE);
pub const EYE_OFF: char = glyph(ph::EYE_SLASH);
pub const LOCK: char = glyph(ph::LOCK);
pub const LOCK_OPEN: char = glyph(ph::LOCK_OPEN);
pub const ADD: char = glyph(ph::PLUS);
pub const CARET_DOWN: char = glyph(ph::CARET_DOWN);
pub const CARET_RIGHT: char = glyph(ph::CARET_RIGHT);
pub const STATUS_DOT: char = glyph(ph::CIRCLE);
pub const CHECK: char = glyph(ph::CHECK_CIRCLE);
pub const WARN: char = glyph(ph::WARNING);
pub const CLOSE: char = glyph(ph::X);
pub const STAR: char = glyph(ph::STAR);
pub const PLAY: char = glyph(ph::PLAY);
pub const PREV: char = glyph(ph::SKIP_BACK);
pub const NEXT: char = glyph(ph::SKIP_FORWARD);
pub const CROP: char = glyph(ph::CROP);
```

- [ ] **Step 2: Write a test asserting the aliases decode to real glyphs.** There is genuine logic here - the `glyph` const fn does UTF-8 decoding - so a real test earns its place. Add this at the bottom of `crates/ui/src/icons.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Every alias must decode to the same scalar phosphor ships in the string,
    /// i.e. `glyph` is a faithful first-char extractor, not a corruption.
    #[test]
    fn aliases_match_phosphor_strings() {
        assert_eq!(PENCIL, ph::PENCIL.chars().next().unwrap());
        assert_eq!(SPARKLE, ph::SPARKLE.chars().next().unwrap());
        assert_eq!(LAYERS, ph::STACK.chars().next().unwrap());
        assert_eq!(EXPORT, ph::EXPORT.chars().next().unwrap());
        assert_eq!(CLOSE, ph::X.chars().next().unwrap());
    }

    /// No alias decoded to the replacement char - that would mean a malformed
    /// decode path, not a real glyph.
    #[test]
    fn no_alias_is_the_replacement_char() {
        for c in [
            PENCIL, ERASER, FILL, LINE, RECT, ELLIPSE, EYEDROPPER, SELECT, LASSO,
            MOVE, TRANSFORM, TEXT, HAND, ZOOM, SPARKLE, LAYERS, SPRITES, PALETTE,
            FRAMES, ASSETS, CONSOLE, TIMELINE, TILESET, PROMPT, RESULTS, HISTORY,
            EXPORT, SETTINGS, EYE, EYE_OFF, LOCK, LOCK_OPEN, ADD, CARET_DOWN,
            CARET_RIGHT, STATUS_DOT, CHECK, WARN, CLOSE, STAR, PLAY, PREV, NEXT, CROP,
        ] {
            assert_ne!(c, '\u{fffd}', "alias decoded to the replacement char");
        }
    }

    /// The ASCII branch of `glyph` is exercised by `X` (single-byte close glyph)
    /// and the multi-byte branches by the private-use phosphor glyphs.
    #[test]
    fn glyph_decodes_ascii_and_multibyte() {
        assert_eq!(glyph("X"), 'X');
        // PENCIL is a 3-byte private-use codepoint; round-trips to one char.
        assert_eq!(PENCIL.len_utf8(), ph::PENCIL.len());
    }
}
```

- [ ] **Step 3: Run the icons tests - expect PASS.** The `glyph` const fn and the aliases are written, so this should pass on the first run (the const fn is a pure compile-time decoder with an established algorithm; there is no failing-first stage to stage because there is no prior implementation to replace). Run in PowerShell:

```powershell
cargo nextest run -p pixhaus-ui
```

Expected: PASS - `aliases_match_phosphor_strings`, `no_alias_is_the_replacement_char`, `glyph_decodes_ascii_and_multibyte`, and the preserved `canvas_callback_is_zero_sized` all green (4 tests passed). If `aliases_match_phosphor_strings` FAILS, the `glyph` decode is wrong - debug the UTF-8 branch, do not change the asserts.

- [ ] **Step 4: Run the doc build to confirm the module's rustdoc is clean.** Run in PowerShell:

```powershell
cargo test --doc -p pixhaus-ui
```

Expected: PASS (no doc tests in `icons.rs`, but this confirms the `///`/`//!` comments compile under `cargo doc` rules - `missing_docs` is `warn` workspace-wide and `pub const`s without docs would warn; the module-level `//!` plus the convention that simple glyph aliases inherit context is acceptable, but if `missing_docs` warns on the `pub const`s, add a one-line `///` to each in a follow-up edit before committing). If `missing_docs` warnings appear, add a terse `/// Pencil tool glyph.`-style doc line to each `pub const` and re-run.

- [ ] **Step 5: Run clippy explicitly on the crate.** Run in PowerShell:

```powershell
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```

Expected: PASS (no warnings). The post-edit hook already ran clippy on save, but this is the explicit verification gate the plan requires.

- [ ] **Step 6: Commit.** Run in PowerShell:

```powershell
git add crates/ui/src/icons.rs; git commit -m @'
ui: add phosphor glyph char constants

crate::icons aliases every phosphor glyph the shell uses (tool rail, panels,
workspace tabs, status, menus) into the char form the metadata structs take.
A const glyph() decodes phosphor's one-char &str strings at compile time;
tests assert the aliases round-trip and none decode to the replacement char.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### SETUP.7: Verify the whole workspace builds

**Files:**
- Test: none (verification only - no files created or modified)

The closing gate for this layer: every later layer assumes `cargo build --workspace` is green on the scaffold. This task adds no code; it proves the skeleton compiles end to end.

- [ ] **Step 1: Build the entire workspace.** Run in PowerShell:

```powershell
cargo build --workspace
```

Expected: PASS - all of `pixhaus-core`, `pixhaus-render`, `pixhaus-io`, `pixhaus-services`, `pixhaus-platform`, `pixhaus-ui`, the eight `pixhaus-mod-*` crates, and `pixhaus-app` compile clean. The five wired module crates and `app` have the new deps declared; the unwired modules (`core`, `pixel-art`, `providers`) are untouched stubs.

- [ ] **Step 2: Run the full workspace test suite to confirm nothing regressed.** Run in PowerShell:

```powershell
cargo nextest run --workspace
```

Expected: PASS - the only tests at this point are `pixhaus-ui`'s `canvas_callback_is_zero_sized` plus the three `icons` tests, and whatever the other scaffold crates already carry. All green.

- [ ] **Step 3: Confirm clippy is clean across the workspace.** Run in PowerShell:

```powershell
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS (no warnings). This is the same gate the Stop hook enforces; passing it here means the SETUP layer leaves the tree in a clean state for the next layer.

- [ ] **Step 4: No commit.** This task created no files and modified none - there is nothing to commit. If `git status` is not clean at this point, a prior task's commit was missed; resolve that before proceeding. Run in PowerShell to confirm a clean tree:

```powershell
git status --porcelain
```

Expected: no output (clean working tree). The SETUP layer is complete; the THEME, CONTRIB_API, REGISTRY, STATE, SHELL, WIDGETS, and module/app layers fill the stubs created here.

## Phase 2: Theme tokens

I now have everything I need. Let me verify the `egui_wants_keyboard_input` signature is in scope and confirm the `Color32` lightness/luminance approach. Both flagged items are verified:

- **Shadow fields**: `offset: [i8; 2]`, `blur: u8`, `spread: u8`, `color: Color32` (no defaults helper - construct literally). Note `offset` is `[i8; 2]` not `f32`.
- **Focus-query helper**: `Context::egui_wants_keyboard_input()` is the current name; `wants_keyboard_input()` is deprecated in 0.34.2. (The shortcut layer is another layer's concern, but `contrast.rs`/theme don't use it; this confirms the spec note for the SHORTCUTS layer.)

For the theme layer I only need the Shadow shape. I have what I need to write the THEME layer plan.

### THEME layer covers spec section "Theme token system" (lines 711-854) and test 4 (lines 1062-1065).

---

### THEME.1: Theme token structs and ThemeVariant/SurfaceTier enums

**Files:**
- Create: `crates/ui/src/theme/tokens.rs`
- Modify: `crates/ui/src/theme/mod.rs` (add `mod tokens;` + re-exports)
- Modify: `crates/ui/src/lib.rs` (ensure `pub mod theme;` exists)

This is pure data - structs, enums, field definitions with no logic. Write it, build it, commit it. No test step here; the assertions live in THEME.3 (palettes) and THEME.5 (Visuals mapping) where there is behavior to pin. The spec is explicit that the contrast/variant tests carry the coverage.

- [ ] **Step 1: Confirm `theme` is declared in `lib.rs`.** Read `crates/ui/src/lib.rs`. If it does not already contain `pub mod theme;`, add it next to the other module declarations (keep `install_canvas_renderer` and `CanvasCallback` untouched - the spec says they are preserved unchanged). If `crates/ui/src/theme/mod.rs` does not exist yet, create it with a single line so the crate compiles:

```rust
//! Theme token system: semantic roles, surfaces, accent, spacing, type, radii.
//! Dark-first; light and accent-high-contrast variants share the same role set.

pub mod tokens;
```

- [ ] **Step 2: Write `crates/ui/src/theme/tokens.rs`** - the full token struct set, copied from the spec (spec lines 720-776). `Shadow` field shape is verified against epaint 0.34.2 (`offset: [i8; 2]`, `blur: u8`, `spread: u8`, `color: Color32`); construct it with a struct literal in THEME.2, so `Elevation` here just holds `egui::epaint::Shadow` values.

```rust
//! Theme token structs. Pure data; no logic. A `Theme` is a runtime value held in
//! `Host` so a variant change can re-derive and re-apply it. Panels and regions ask
//! for `theme.surfaces.panel` or `theme.surface(SurfaceTier::Elevated)`, never a hex
//! literal.

use egui::Color32;

/// The full token set for one resolved theme. Built by `Theme::for_variant`.
#[derive(Copy, Clone)]
pub struct Theme {
    pub variant: ThemeVariant,
    pub surfaces: Surfaces,
    pub roles: Roles,
    /// Derived from a seed color; the separable preference axis (light/dark is one
    /// axis, accent color is another - the bible treats them as two preferences).
    pub accent: AccentTokens,
    pub elevation: Elevation,
    pub spacing: Spacing,
    pub type_scale: TypeScale,
    pub radius: Radii,
}

/// Which theme is active. `serde`-ready so it can round-trip through `Prefs`.
#[derive(Copy, Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum ThemeVariant {
    Dark,
    Light,
    AccentHighContrast,
}

/// Per-region background tiers (UX 6.2), near-black warm slate in dark.
#[derive(Copy, Clone)]
pub struct Surfaces {
    /// Darkest - the app frame.
    pub app_frame: Color32,
    /// Dark charcoal/slate - panels, left rail, tray.
    pub panel: Color32,
    /// Slightly lighter - cards, top bars.
    pub elevated: Color32,
    /// Deepest neutral - behind the artboard.
    pub stage: Color32,
    /// Text fields, wells, HUD.
    pub inset: Color32,
}

/// Semantic foreground and status roles.
#[derive(Copy, Clone)]
pub struct Roles {
    pub border: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_disabled: Color32,
    /// Muted green.
    pub success: Color32,
    /// Muted amber.
    pub warning: Color32,
    /// Muted red.
    pub error: Color32,
}

/// All derived from one seed (default ~#7c6cef violet).
#[derive(Copy, Clone)]
pub struct AccentTokens {
    pub seed: Color32,
    /// The accent.
    pub base: Color32,
    pub hover: Color32,
    /// Low-alpha fill behind an active tool/tab/row.
    pub muted: Color32,
    /// Sparkle marker tint (named for intent).
    pub ai: Color32,
    /// Softer halo behind AI affordances.
    pub ai_glow: Color32,
}

/// Shadow tiers (UX 6.2). The artboard "shadow" is painted manually in the canvas
/// stage - `Shadow` is not a paint primitive and is reserved for card `Frame`s and
/// the command-palette `Area`.
#[derive(Copy, Clone)]
pub struct Elevation {
    /// Card `Frame`s.
    pub raised: egui::epaint::Shadow,
    /// Command palette / windows.
    pub overlay: egui::epaint::Shadow,
}

/// Spacing scale: 2, 4, 8, 12, 16.
#[derive(Copy, Clone)]
pub struct Spacing {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
}

/// Type scale: 11, 13, 13, 15, 12.
#[derive(Copy, Clone)]
pub struct TypeScale {
    pub label: f32,
    pub body: f32,
    pub section_header: f32,
    pub title: f32,
    pub mono: f32,
}

/// Corner radii: 2, 3 - a production cockpit, not rounded mobile.
#[derive(Copy, Clone)]
pub struct Radii {
    pub sm: f32,
    pub md: f32,
}

/// Names a surface tier so callers can ask `theme.surface(tier)` at runtime.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SurfaceTier {
    AppFrame,
    Panel,
    Elevated,
    Stage,
    Inset,
}
```

- [ ] **Step 3: Re-export the token types from `theme/mod.rs`.** Add the re-export so `crate::theme::Theme` etc. resolve (the spec wires `pixhaus_ui::theme::Theme::dark()` from `app`, and `state/mod.rs` stores `Theme` in `Host`):

```rust
pub use tokens::{
    AccentTokens, Elevation, Radii, Roles, Spacing, Surfaces, SurfaceTier, Theme,
    ThemeVariant, TypeScale,
};
```

- [ ] **Step 4: Build the crate.** Run in PowerShell:

```
cargo build -p pixhaus-ui
```

Expected: PASS (compiles). The crate has no constructors yet, so nothing else references these types; a clean build is the only signal. If `missing_docs` warns on any public item, add the one-line doc the lint asks for - the workspace sets `missing_docs = "warn"` and the Stop gate treats clippy warnings as errors.

- [ ] **Step 5: Commit.**

```
git add crates/ui/src/theme/tokens.rs crates/ui/src/theme/mod.rs crates/ui/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(ui): add theme token structs and variant/tier enums

The runtime Theme value: semantic surfaces, roles, a seed-derived accent
axis, elevation/spacing/type/radius scales, plus ThemeVariant and
SurfaceTier. Pure data; constructors and the Visuals mapping land next.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### THEME.2: WCAG contrast helper (`contrast.rs`)

**Files:**
- Create: `crates/ui/src/theme/contrast.rs`
- Modify: `crates/ui/src/theme/mod.rs` (add `mod contrast;` + re-export `wcag_contrast`)
- Test: inline `#[cfg(test)] mod tests` in `contrast.rs`

`wcag_contrast(fg, bg) -> f32` is a pure function (spec line 70, test 4 lines 1063-1065). It is the cheapest place to enforce the accessibility ask and it has real logic, so use the TDD rhythm: write the failing test against known WCAG reference values first.

- [ ] **Step 1: Add the crate-root test allow-attribute if it is not already present.** Read the first lines of `crates/ui/src/lib.rs`. If the file does not start with the test allow-attribute, add it as the very first line (the conventions require it at the crate root of any crate whose tests use `unwrap`/`expect`):

```rust
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
```

- [ ] **Step 2: Write the failing test in `crates/ui/src/theme/contrast.rs`.** Create the file with only the test module and a stub signature so it compiles-but-fails. WCAG 2.x contrast ratio is `(L_lighter + 0.05) / (L_darker + 0.05)` where `L` is relative luminance. Reference checks: black-on-white is exactly `21.0`; white-on-white is `1.0`; a mid-gray `#777777` on white is ~ `4.48`. Use these as the anchored cases.

```rust
//! WCAG 2.x relative-luminance contrast ratio. Pure; used by the theme test to
//! enforce the accessibility floor (text on its surface >= 4.5, large/structural
//! >= 3.0). Order-independent: returns the same ratio whichever color is lighter.

use egui::Color32;

/// Stub - replaced in the next step.
pub fn wcag_contrast(_fg: Color32, _bg: Color32) -> f32 {
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    mod wcag_contrast {
        use super::*;

        #[test]
        fn black_on_white_is_max_ratio() {
            let r = wcag_contrast(Color32::BLACK, Color32::WHITE);
            assert!((r - 21.0).abs() < 0.05, "expected ~21.0, got {r}");
        }

        #[test]
        fn identical_colors_are_one() {
            let r = wcag_contrast(Color32::WHITE, Color32::WHITE);
            assert!((r - 1.0).abs() < 0.001, "expected 1.0, got {r}");
        }

        #[test]
        fn is_order_independent() {
            let a = wcag_contrast(Color32::BLACK, Color32::WHITE);
            let b = wcag_contrast(Color32::WHITE, Color32::BLACK);
            assert!((a - b).abs() < 0.001, "ratio must not depend on argument order");
        }

        #[test]
        fn mid_gray_on_white_matches_reference() {
            // #777777 on #ffffff is a well-known WCAG reference at ~4.48:1.
            let gray = Color32::from_rgb(0x77, 0x77, 0x77);
            let r = wcag_contrast(gray, Color32::WHITE);
            assert!((r - 4.48).abs() < 0.1, "expected ~4.48, got {r}");
        }
    }
}
```

- [ ] **Step 3: Run the test, expect FAIL.**

```
cargo nextest run -p pixhaus-ui wcag_contrast
```

Expected: FAIL (the stub returns `0.0`; `black_on_white_is_max_ratio` and the others fail their asserts). Confirm it is the assertion that fails, not a compile error.

- [ ] **Step 4: Implement `wcag_contrast`.** Replace the stub. `Color32` channels are premultiplied, but the theme tokens are all fully opaque, so reading `r()/g()/b()` is correct here; compute sRGB relative luminance per the WCAG formula.

```rust
/// WCAG 2.x contrast ratio between two colors, `(Llight + 0.05) / (Ldark + 0.05)`.
/// Both colors are treated as opaque (theme tokens always are). Range 1.0..=21.0.
pub fn wcag_contrast(fg: Color32, bg: Color32) -> f32 {
    let l1 = relative_luminance(fg);
    let l2 = relative_luminance(bg);
    let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

/// sRGB relative luminance per WCAG 2.x (0.0 black .. 1.0 white).
fn relative_luminance(c: Color32) -> f32 {
    let r = linearize(f32::from(c.r()) / 255.0);
    let g = linearize(f32::from(c.g()) / 255.0);
    let b = linearize(f32::from(c.b()) / 255.0);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Inverse sRGB companding for one channel.
fn linearize(channel: f32) -> f32 {
    if channel <= 0.040_45 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}
```

- [ ] **Step 5: Run the test, expect PASS.**

```
cargo nextest run -p pixhaus-ui wcag_contrast
```

Expected: PASS (all four cases green).

- [ ] **Step 6: Wire the module and re-export.** Add to `crates/ui/src/theme/mod.rs`:

```rust
mod contrast;

pub use contrast::wcag_contrast;
```

- [ ] **Step 7: Commit.**

```
git add crates/ui/src/theme/contrast.rs crates/ui/src/theme/mod.rs crates/ui/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(ui): add WCAG contrast helper for the theme test

wcag_contrast(fg, bg) returns the WCAG 2.x ratio over sRGB relative
luminance, order-independent. Anchored against black-on-white (21:1) and
the #777 reference (~4.48:1). The accessibility floor the theme test enforces.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### THEME.3: Palettes - `for_variant`, `dark`/`light`/`accent_high_contrast`, accent derivation, `surface`, `accent_seed`

**Files:**
- Create: `crates/ui/src/theme/palettes.rs`
- Modify: `crates/ui/src/theme/mod.rs` (add `mod palettes;`; export `DEFAULT_ACCENT_SEED`)
- Test: inline `#[cfg(test)] mod tests` in `palettes.rs`

This builds the constructors (spec lines 780-792) plus the `DEFAULT_ACCENT_SEED ~ #7c6cef`. Real logic with a real contract (every role populated, surface tiers ordered by lightness, contrast floors), so TDD: the tests are spec test 4's variant + tier + WCAG assertions, which belong here because they exercise the produced `Theme` values.

- [ ] **Step 1: Write the failing test module in `crates/ui/src/theme/palettes.rs`.** Create the file with the constructor signatures as stubs that return an all-black theme (so tests compile and fail on the populated-role / ordering / contrast assertions). Pull `wcag_contrast` from the sibling module and the tier helper from the spec.

```rust
//! Theme construction. `for_variant` is the single source; `dark`/`light`/
//! `accent_high_contrast` are named wrappers. Accent tokens derive from one seed so
//! a future accent preference recolors independently of light/dark. Only `dark()` is
//! visually tuned this round; the other variants are structured in.

use egui::Color32;

use super::contrast::wcag_contrast;
use super::tokens::{
    AccentTokens, Elevation, Radii, Roles, Spacing, SurfaceTier, Surfaces, Theme,
    ThemeVariant, TypeScale,
};

/// Default accent seed: a warm violet (~#7c6cef).
pub const DEFAULT_ACCENT_SEED: Color32 = Color32::from_rgb(0x7c, 0x6c, 0xef);

impl Theme {
    /// The tuned dark theme - the only finished variant this round.
    pub fn dark() -> Self {
        Self::for_variant(ThemeVariant::Dark, DEFAULT_ACCENT_SEED)
    }

    /// Build a theme for a variant with a given accent seed.
    pub fn for_variant(_v: ThemeVariant, _accent_seed: Color32) -> Self {
        todo!("implemented in the next step")
    }

    /// The light variant (structured in, not visually tuned this round).
    pub fn light() -> Self {
        Self::for_variant(ThemeVariant::Light, DEFAULT_ACCENT_SEED)
    }

    /// The accent-high-contrast variant (structured in, not tuned this round).
    pub fn accent_high_contrast() -> Self {
        Self::for_variant(ThemeVariant::AccentHighContrast, DEFAULT_ACCENT_SEED)
    }

    /// The seed the accent tokens were derived from (the separable preference axis).
    pub fn accent_seed(&self) -> Color32 {
        self.accent.seed
    }

    /// Resolve a surface tier to its color at runtime.
    pub fn surface(&self, t: SurfaceTier) -> Color32 {
        match t {
            SurfaceTier::AppFrame => self.surfaces.app_frame,
            SurfaceTier::Panel => self.surfaces.panel,
            SurfaceTier::Elevated => self.surfaces.elevated,
            SurfaceTier::Stage => self.surfaces.stage,
            SurfaceTier::Inset => self.surfaces.inset,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Perceptual lightness proxy (sRGB luma), only used to order surface tiers.
    fn luma(c: Color32) -> f32 {
        0.2126 * f32::from(c.r()) + 0.7152 * f32::from(c.g()) + 0.0722 * f32::from(c.b())
    }

    /// No role color may be left at the default all-zero black (a population leak).
    fn assert_no_black_leak(theme: &Theme) {
        for (name, c) in [
            ("border", theme.roles.border),
            ("text_primary", theme.roles.text_primary),
            ("text_secondary", theme.roles.text_secondary),
            ("text_disabled", theme.roles.text_disabled),
            ("success", theme.roles.success),
            ("warning", theme.roles.warning),
            ("error", theme.roles.error),
            ("accent.base", theme.accent.base),
            ("accent.hover", theme.accent.hover),
            ("accent.ai", theme.accent.ai),
        ] {
            assert_ne!(c, Color32::BLACK, "{name} left at default black");
        }
    }

    #[test]
    fn dark_uses_the_default_accent_seed() {
        assert_eq!(Theme::dark().accent_seed(), DEFAULT_ACCENT_SEED);
    }

    #[test]
    fn every_variant_populates_every_role() {
        assert_no_black_leak(&Theme::dark());
        assert_no_black_leak(&Theme::light());
        assert_no_black_leak(&Theme::accent_high_contrast());
    }

    #[test]
    fn dark_surface_tiers_are_ordered_by_lightness() {
        let t = Theme::dark();
        // app_frame is darkest; panel sits above it; elevated above that.
        assert!(
            luma(t.surfaces.app_frame) < luma(t.surfaces.panel),
            "app_frame must be darker than panel"
        );
        assert!(
            luma(t.surfaces.panel) < luma(t.surfaces.elevated),
            "panel must be darker than elevated"
        );
    }

    #[test]
    fn surface_helper_matches_fields() {
        let t = Theme::dark();
        assert_eq!(t.surface(SurfaceTier::AppFrame), t.surfaces.app_frame);
        assert_eq!(t.surface(SurfaceTier::Panel), t.surfaces.panel);
        assert_eq!(t.surface(SurfaceTier::Elevated), t.surfaces.elevated);
        assert_eq!(t.surface(SurfaceTier::Stage), t.surfaces.stage);
        assert_eq!(t.surface(SurfaceTier::Inset), t.surfaces.inset);
    }

    #[test]
    fn dark_text_meets_wcag_floors() {
        let t = Theme::dark();
        assert!(
            wcag_contrast(t.roles.text_primary, t.surfaces.panel) >= 4.5,
            "text_primary on panel below 4.5: {}",
            wcag_contrast(t.roles.text_primary, t.surfaces.panel)
        );
        assert!(
            wcag_contrast(t.roles.text_secondary, t.surfaces.panel) >= 4.5,
            "text_secondary on panel below 4.5: {}",
            wcag_contrast(t.roles.text_secondary, t.surfaces.panel)
        );
        assert!(
            wcag_contrast(t.roles.text_primary, t.surfaces.elevated) >= 4.5,
            "text_primary on elevated below 4.5: {}",
            wcag_contrast(t.roles.text_primary, t.surfaces.elevated)
        );
        assert!(
            wcag_contrast(t.roles.text_primary, t.accent.muted) >= 3.0,
            "text_primary on accent.muted below 3.0: {}",
            wcag_contrast(t.roles.text_primary, t.accent.muted)
        );
    }
}
```

- [ ] **Step 2: Run the test, expect FAIL.**

```
cargo nextest run -p pixhaus-ui --no-fail-fast theme::palettes
```

Expected: FAIL - `for_variant` is `todo!()`, so every test that builds a `Theme` panics. Confirm the panics come from the `todo!`, not a compile error.

- [ ] **Step 3: Implement `for_variant` and the accent derivation.** Replace the `todo!` body. Dark is the tuned variant; light and high-contrast are structured in with sensible but unfinished values that still satisfy the no-black-leak and (for dark) contrast floors. The dark surface ramp is near-black warm slate. The accent tokens derive from the seed by lightening (hover), low-alpha (muted), and reusing the seed for `ai` with a softer halo. Choose dark text/surface values that clear the WCAG floors the test asserts - `text_primary` ~ `#e6e3ef` on `panel` ~ `#1b1a20` clears 4.5 comfortably; `accent.muted` is the seed at low alpha drawn over `panel`, so the test compares `text_primary` against the muted color value directly (a conservative proxy).

Add these helpers and the body inside `palettes.rs` (above the `#[cfg(test)]` module):

```rust
/// Lighten each channel toward white by `t` (0.0 = unchanged, 1.0 = white).
fn lighten(c: Color32, t: f32) -> Color32 {
    let mix = |ch: u8| -> u8 {
        let v = f32::from(ch);
        (v + (255.0 - v) * t).round().clamp(0.0, 255.0) as u8
    };
    Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()))
}

/// Darken each channel toward black by `t` (0.0 = unchanged, 1.0 = black).
fn darken(c: Color32, t: f32) -> Color32 {
    let mix = |ch: u8| -> u8 { (f32::from(ch) * (1.0 - t)).round().clamp(0.0, 255.0) as u8 };
    Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()))
}

/// Derive the full accent token set from one seed.
fn accent_from_seed(seed: Color32) -> AccentTokens {
    AccentTokens {
        seed,
        base: seed,
        hover: lighten(seed, 0.15),
        // Low-alpha fill; the muted value the contrast test reads is the opaque
        // mix of the seed darkened toward the dark panel, a conservative proxy.
        muted: darken(seed, 0.55),
        ai: lighten(seed, 0.10),
        ai_glow: Color32::from_rgba_unmultiplied(seed.r(), seed.g(), seed.b(), 40),
    }
}
```

Then the real `for_variant`:

```rust
    pub fn for_variant(v: ThemeVariant, accent_seed: Color32) -> Self {
        let accent = accent_from_seed(accent_seed);
        let spacing = Spacing { xs: 2.0, sm: 4.0, md: 8.0, lg: 12.0, xl: 16.0 };
        let type_scale = TypeScale {
            label: 11.0,
            body: 13.0,
            section_header: 13.0,
            title: 15.0,
            mono: 12.0,
        };
        let radius = Radii { sm: 2.0, md: 3.0 };
        let elevation = Elevation {
            raised: egui::epaint::Shadow {
                offset: [0, 2],
                blur: 8,
                spread: 0,
                color: Color32::from_black_alpha(96),
            },
            overlay: egui::epaint::Shadow {
                offset: [0, 6],
                blur: 24,
                spread: 0,
                color: Color32::from_black_alpha(128),
            },
        };

        let (surfaces, roles) = match v {
            ThemeVariant::Dark => (
                Surfaces {
                    app_frame: Color32::from_rgb(0x12, 0x11, 0x16),
                    panel: Color32::from_rgb(0x1b, 0x1a, 0x20),
                    elevated: Color32::from_rgb(0x24, 0x22, 0x2b),
                    stage: Color32::from_rgb(0x0d, 0x0c, 0x10),
                    inset: Color32::from_rgb(0x15, 0x14, 0x19),
                },
                Roles {
                    border: Color32::from_rgb(0x33, 0x31, 0x3c),
                    text_primary: Color32::from_rgb(0xe6, 0xe3, 0xef),
                    text_secondary: Color32::from_rgb(0xa8, 0xa4, 0xb4),
                    text_disabled: Color32::from_rgb(0x6c, 0x69, 0x77),
                    success: Color32::from_rgb(0x6f, 0xb5, 0x84),
                    warning: Color32::from_rgb(0xd1, 0xa8, 0x5f),
                    error: Color32::from_rgb(0xd1, 0x6f, 0x6f),
                },
            ),
            ThemeVariant::Light => (
                // Structured in, not tuned this round. Values clear the no-black-leak
                // floor; visual tuning is a follow-up.
                Surfaces {
                    app_frame: Color32::from_rgb(0xd8, 0xd6, 0xde),
                    panel: Color32::from_rgb(0xec, 0xea, 0xf0),
                    elevated: Color32::from_rgb(0xf6, 0xf5, 0xf9),
                    stage: Color32::from_rgb(0xc8, 0xc6, 0xd0),
                    inset: Color32::from_rgb(0xff, 0xff, 0xff),
                },
                Roles {
                    border: Color32::from_rgb(0xc2, 0xc0, 0xcc),
                    text_primary: Color32::from_rgb(0x1b, 0x1a, 0x20),
                    text_secondary: Color32::from_rgb(0x53, 0x50, 0x5e),
                    text_disabled: Color32::from_rgb(0x9a, 0x97, 0xa6),
                    success: Color32::from_rgb(0x2f, 0x7d, 0x4c),
                    warning: Color32::from_rgb(0x8a, 0x66, 0x1f),
                    error: Color32::from_rgb(0x9a, 0x33, 0x33),
                },
            ),
            ThemeVariant::AccentHighContrast => (
                // Structured in, not tuned this round.
                Surfaces {
                    app_frame: Color32::from_rgb(0x00, 0x00, 0x00),
                    panel: Color32::from_rgb(0x0a, 0x0a, 0x0d),
                    elevated: Color32::from_rgb(0x16, 0x15, 0x1c),
                    stage: Color32::from_rgb(0x00, 0x00, 0x00),
                    inset: Color32::from_rgb(0x05, 0x05, 0x07),
                },
                Roles {
                    border: accent.base,
                    text_primary: Color32::from_rgb(0xff, 0xff, 0xff),
                    text_secondary: Color32::from_rgb(0xd6, 0xd4, 0xe4),
                    text_disabled: Color32::from_rgb(0x8a, 0x87, 0x99),
                    success: Color32::from_rgb(0x7c, 0xe0, 0x9a),
                    warning: Color32::from_rgb(0xf0, 0xc8, 0x6f),
                    error: Color32::from_rgb(0xf0, 0x8a, 0x8a),
                },
            ),
        };

        Self { variant: v, surfaces, roles, accent, elevation, spacing, type_scale, radius }
    }
```

Note on `app_frame` in AccentHighContrast: it is pure black, which would trip a naive "black leak" check - but the test only checks `roles.*` and `accent.*`, never `surfaces.*`, so a black app-frame surface is fine and intended.

- [ ] **Step 4: Run the test, expect PASS.**

```
cargo nextest run -p pixhaus-ui --no-fail-fast theme::palettes
```

Expected: PASS (all six cases green). If `dark_text_meets_wcag_floors` fails on the `accent.muted` case, nudge the `muted` derivation darker (raise the `darken` factor) until `wcag_contrast(text_primary, muted) >= 3.0`; do not lower the threshold.

- [ ] **Step 5: Wire the module and re-export.** Add to `crates/ui/src/theme/mod.rs`:

```rust
mod palettes;

pub use palettes::DEFAULT_ACCENT_SEED;
```

(`Theme::dark()` etc. are inherent methods, so they need no separate re-export - they ride on the `Theme` export from THEME.1.)

- [ ] **Step 6: Commit.**

```
git add crates/ui/src/theme/palettes.rs crates/ui/src/theme/mod.rs
git commit -m "$(cat <<'EOF'
feat(ui): build theme variants and seed-derived accent

for_variant is the single constructor; dark/light/accent_high_contrast
wrap it. Accent tokens derive from one seed (default ~#7c6cef) so the
accent axis is separable from light/dark. Dark is tuned and clears the
WCAG floors; light and high-contrast are structured in. surface(tier)
and accent_seed() resolve tokens at runtime.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### THEME.4: Token-to-`Visuals` mapping (`apply_to_visuals`)

**Files:**
- Modify: `crates/ui/src/theme/mod.rs` (add `apply_to_visuals` + its test)
- Test: inline `#[cfg(test)] mod tests` in `theme/mod.rs`

`apply_to_visuals(theme, ctx)` maps tokens onto egui `Visuals`/`Style` (spec lines 796-821), re-applied on a variant change by `apply_intent`. This is spec test 4's "theme -> Visuals" half: assert the mapped fields equal the tokens. Real behavior, so TDD. A headless `egui::Context` is constructible without a window or GPU, so the test runs under nextest.

- [ ] **Step 1: Write the failing test in `crates/ui/src/theme/mod.rs`.** Add a stub `apply_to_visuals` (empty body) and the test module. The test builds a default `Context`, applies the dark theme, and reads `ctx.style()` back.

```rust
/// Stub - replaced in the next step.
pub fn apply_to_visuals(_theme: &Theme, _ctx: &egui::Context) {}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Context;

    #[test]
    fn dark_panel_fill_maps_from_surfaces_panel() {
        let theme = Theme::dark();
        let ctx = Context::default();
        apply_to_visuals(&theme, &ctx);
        assert_eq!(ctx.style().visuals.panel_fill, theme.surfaces.panel);
    }

    #[test]
    fn dark_selection_stroke_maps_from_accent_base() {
        let theme = Theme::dark();
        let ctx = Context::default();
        apply_to_visuals(&theme, &ctx);
        assert_eq!(ctx.style().visuals.selection.stroke.color, theme.accent.base);
    }

    #[test]
    fn dark_extreme_bg_maps_from_inset() {
        let theme = Theme::dark();
        let ctx = Context::default();
        apply_to_visuals(&theme, &ctx);
        assert_eq!(ctx.style().visuals.extreme_bg_color, theme.surfaces.inset);
    }

    #[test]
    fn dark_override_text_color_maps_from_text_primary() {
        let theme = Theme::dark();
        let ctx = Context::default();
        apply_to_visuals(&theme, &ctx);
        assert_eq!(
            ctx.style().visuals.override_text_color,
            Some(theme.roles.text_primary)
        );
    }

    #[test]
    fn light_variant_sets_dark_mode_false() {
        let theme = Theme::light();
        let ctx = Context::default();
        apply_to_visuals(&theme, &ctx);
        assert!(!ctx.style().visuals.dark_mode);
    }

    #[test]
    fn dark_variant_sets_dark_mode_true() {
        let theme = Theme::dark();
        let ctx = Context::default();
        apply_to_visuals(&theme, &ctx);
        assert!(ctx.style().visuals.dark_mode);
    }
}
```

- [ ] **Step 2: Run the test, expect FAIL.**

```
cargo nextest run -p pixhaus-ui --no-fail-fast theme::tests
```

Expected: FAIL - the stub does nothing, so `panel_fill` stays at egui's default and the asserts fire. Confirm assertion failures, not compile errors.

- [ ] **Step 3: Implement `apply_to_visuals`.** Replace the stub with the spec mapping (spec lines 797-821). The `Shadow` values come straight from `theme.elevation`. Place it above the `#[cfg(test)]` module.

```rust
/// Map theme tokens onto egui's `Visuals`/`Style`. Called once at boot and re-applied
/// by `apply_intent` on a variant change so a theme switch actually repaints. Uses
/// `style_mut` to avoid cloning the whole style.
pub fn apply_to_visuals(theme: &Theme, ctx: &egui::Context) {
    ctx.style_mut(|style| {
        let v = &mut style.visuals;
        v.dark_mode = theme.variant != ThemeVariant::Light;
        v.panel_fill = theme.surfaces.panel;
        v.window_fill = theme.surfaces.elevated;
        v.extreme_bg_color = theme.surfaces.inset;
        v.faint_bg_color = theme.surfaces.elevated;
        v.override_text_color = Some(theme.roles.text_primary);
        v.hyperlink_color = theme.accent.base;
        v.selection.bg_fill = theme.accent.muted;
        v.selection.stroke = egui::Stroke::new(1.0, theme.accent.base);
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, theme.roles.border);
        v.widgets.hovered.bg_fill = theme.accent.muted;
        v.widgets.active.bg_fill = theme.accent.base;
        v.window_shadow = theme.elevation.overlay;
        v.popup_shadow = theme.elevation.overlay;
        style.spacing.item_spacing = egui::vec2(theme.spacing.sm, theme.spacing.xs);
        style.spacing.button_padding = egui::vec2(theme.spacing.sm, theme.spacing.xs);
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(theme.type_scale.body));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(theme.type_scale.title));
        style
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(theme.type_scale.label));
        style
            .text_styles
            .insert(egui::TextStyle::Monospace, egui::FontId::monospace(theme.type_scale.mono));
    });
}
```

- [ ] **Step 4: Run the test, expect PASS.**

```
cargo nextest run -p pixhaus-ui --no-fail-fast theme::tests
```

Expected: PASS (all six cases green).

- [ ] **Step 5: Run clippy on the crate to confirm no pedantic warnings on the new code.**

```
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```

Expected: PASS (no warnings). If `clippy::cast_possible_truncation` or similar fires on the `as u8` casts in `palettes.rs`, the `.round().clamp(0.0, 255.0)` already bounds them; add a scoped `#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]` on the `lighten`/`darken` helpers with a one-line comment that the clamp makes the cast safe.

- [ ] **Step 6: Commit.**

```
git add crates/ui/src/theme/mod.rs
git commit -m "$(cat <<'EOF'
feat(ui): map theme tokens onto egui Visuals

apply_to_visuals(theme, ctx) writes surfaces, roles, accent, elevation
shadows, spacing and the text-style scale into egui's Visuals/Style via
style_mut. Tested against a headless Context: panel_fill, selection
stroke, extreme_bg, override text, and dark_mode all track the tokens.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### THEME.5: Font installation (`install_fonts`) - UI sans + mono + phosphor fallback family

**Files:**
- Create: `crates/ui/src/theme/fonts.rs`
- Modify: `crates/ui/src/theme/mod.rs` (add `mod fonts;` + re-export `install_fonts`)
- Modify: `Cargo.toml` (root) - add `egui-phosphor` to the catalog (ratified, spec line 23-24)
- Modify: `crates/ui/Cargo.toml` - depend on `egui-phosphor`
- Test: inline `#[cfg(test)] mod tests` in `fonts.rs`

`install_fonts(ctx)` merges a UI sans (egui's bundled font this round), a mono, and the phosphor glyph ranges into `FontDefinitions` as a fallback family so `crate::icons::*` resolve (spec lines 845-854). egui-phosphor is the ratified icon dependency. The spec accepts shipping egui's bundled fonts for sans/mono this round and layering a chosen font later - so this task adds phosphor only, not new font assets.

- [ ] **Step 1: Add `egui-phosphor` to the workspace catalog.** In the root `Cargo.toml`, under `[workspace.dependencies]`, next to the egui stack, add the pinned line. egui-phosphor tracks egui's version; use the 0.34-compatible release. Add it directly under the `egui-wgpu = "0.34"` line:

```toml
egui-phosphor = "0.10"
```

(egui-phosphor 0.10 targets egui 0.34. If `cargo build` in Step 5 reports a version-mismatch against egui 0.34.2, run `cargo search egui-phosphor` in PowerShell to find the exact release built against egui 0.34 and pin that minor instead - the family API used here, `Variant::Regular.font_data()` and `add_to_fonts`, is stable across its recent minors.)

- [ ] **Step 2: Add the dependency to `crates/ui/Cargo.toml`.** Read `crates/ui/Cargo.toml` first to match its existing `[dependencies]` style (workspace inheritance). Add under `[dependencies]`:

```toml
egui-phosphor = { workspace = true }
```

- [ ] **Step 3: Write the failing test in `crates/ui/src/theme/fonts.rs`.** Create the file with a stub `install_fonts` (empty body) and a test that applies it to a headless `Context` and asserts the phosphor fallback landed in both font families. The observable contract: after `install_fonts`, the `Proportional` and `Monospace` families each list the phosphor font key, so icon glyphs resolve everywhere text is drawn.

```rust
//! Font installation. Registers egui's bundled sans/mono this round and merges the
//! phosphor glyph ranges as a fallback family so `crate::icons::*` resolve. A
//! higher-quality UI font is a later polish step - fonts are an asset decision, not
//! architecture (spec). No emoji literals anywhere: egui's default fonts render emoji
//! as tofu, and phosphor private-use codepoints render blank without this font.

/// Stub - replaced in the next step.
pub fn install_fonts(_ctx: &egui::Context) {}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Context, FontFamily};

    #[test]
    fn phosphor_is_a_fallback_in_proportional_and_monospace() {
        let ctx = Context::default();
        install_fonts(&ctx);
        ctx.fonts(|fonts| {
            // The phosphor family key is present in both families as a fallback, so
            // an icon glyph renders inside both proportional and monospace text.
            let families = fonts.families();
            assert!(
                families.contains(&FontFamily::Proportional),
                "proportional family missing"
            );
            assert!(
                families.contains(&FontFamily::Monospace),
                "monospace family missing"
            );
        });
    }
}
```

Note: `Context::fonts` requires `pixels_per_point` to be known, which `Context::default()` provides (it defaults to 1.0), so this runs headless without a frame. If `fonts(...)` returns before fonts are realized, fall back to asserting on the `FontDefinitions` you pass to `set_fonts` by extracting the merge into a pure helper `fn merged_fonts() -> egui::FontDefinitions` and testing that helper directly (it lists the phosphor key in both families) - see Step 4's structure, which already splits the helper out for exactly this reason.

- [ ] **Step 4: Run the test, expect FAIL.**

```
cargo nextest run -p pixhaus-ui --no-fail-fast theme::fonts
```

Expected: FAIL - the stub never calls `set_fonts`, so the assertion on the realized families... actually passes for the two default families. Before implementing, strengthen the test to the pure-helper form so it has real teeth, then implement. Replace the test body with the helper-based assertion in Step 5; run again to confirm it fails on the missing phosphor key.

- [ ] **Step 5: Implement `install_fonts` with an extracted pure `merged_fonts` helper, and retarget the test at the helper.** Replace the stub and the test. The helper builds `FontDefinitions`, inserts the phosphor font data, and prepends its key to both families' fallback lists; `install_fonts` just hands the result to `ctx.set_fonts`. egui-phosphor exposes `Variant::Regular.font_data()` returning `egui::FontData` and a family-name constant; use its `add_to_fonts` if available, else insert by hand. The hand form (robust across egui-phosphor minors):

```rust
use egui::{FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

/// The font key under which phosphor's regular glyphs are registered.
const PHOSPHOR_KEY: &str = "phosphor";

/// Build the merged `FontDefinitions`: egui's bundled sans/mono plus phosphor as a
/// fallback in both the proportional and monospace families. Pure, so it is the
/// test target.
fn merged_fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();

    // Phosphor regular variant as a static font, registered once.
    fonts.font_data.insert(
        PHOSPHOR_KEY.to_owned(),
        Arc::new(FontData::from_static(egui_phosphor::Variant::Regular.font_data())),
    );

    // Append phosphor as a fallback so icon glyphs resolve inside ordinary text in
    // both families. Push to the back: the bundled font wins for normal characters,
    // phosphor only fills its private-use icon range.
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push(PHOSPHOR_KEY.to_owned());
    }

    fonts
}

/// Install the merged fonts on the context. Call once, at boot.
pub fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(merged_fonts());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phosphor_is_registered_as_font_data() {
        let fonts = merged_fonts();
        assert!(
            fonts.font_data.contains_key(PHOSPHOR_KEY),
            "phosphor font data not registered"
        );
    }

    #[test]
    fn phosphor_is_a_fallback_in_proportional() {
        let fonts = merged_fonts();
        let fam = fonts
            .families
            .get(&FontFamily::Proportional)
            .expect("proportional family missing");
        assert!(
            fam.iter().any(|k| k == PHOSPHOR_KEY),
            "phosphor not in proportional fallback list"
        );
    }

    #[test]
    fn phosphor_is_a_fallback_in_monospace() {
        let fonts = merged_fonts();
        let fam = fonts
            .families
            .get(&FontFamily::Monospace)
            .expect("monospace family missing");
        assert!(
            fam.iter().any(|k| k == PHOSPHOR_KEY),
            "phosphor not in monospace fallback list"
        );
    }
}
```

If `egui_phosphor::Variant::Regular.font_data()` does not exist under that exact path in the pinned release, check the crate's actual API in PowerShell with `cargo doc -p egui-phosphor --no-deps --open` or read its `lib.rs` under `~/.cargo/registry/src/*/egui-phosphor-*/src/lib.rs`; the crate's documented one-call helper is `egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular)`, which inserts the font data and appends it to both families - if that helper is present, replace the manual insert + the family loop with that single call and keep `PHOSPHOR_KEY` only for the test by reading the key the helper used (the crate exposes it as a const), or drop the key-based assertions in favor of asserting `fonts.font_data.len()` grew. Prefer the documented `add_to_fonts` helper when available.

- [ ] **Step 6: Run the test, expect PASS.**

```
cargo nextest run -p pixhaus-ui --no-fail-fast theme::fonts
```

Expected: PASS (all three cases green).

- [ ] **Step 7: Wire the module and re-export.** Add to `crates/ui/src/theme/mod.rs`:

```rust
mod fonts;

pub use fonts::install_fonts;
```

- [ ] **Step 8: Build the whole workspace to confirm the new dependency resolves and the lockfile updates.**

```
cargo build -p pixhaus-ui
```

Expected: PASS. The first build fetches `egui-phosphor` and updates `Cargo.lock`. If it fails on a version mismatch against egui 0.34.2, pin the egui-phosphor minor that targets egui 0.34 per Step 1's note, then rebuild.

- [ ] **Step 9: Commit (include the lockfile and both manifests).**

```
git add Cargo.toml Cargo.lock crates/ui/Cargo.toml crates/ui/src/theme/fonts.rs crates/ui/src/theme/mod.rs
git commit -m "$(cat <<'EOF'
feat(ui): install fonts with a phosphor fallback family

install_fonts merges egui's bundled sans/mono with the phosphor glyph
range as a fallback in both the proportional and monospace families, so
crate::icons::* resolve in any text. Adds the ratified egui-phosphor
dependency to the catalog. A higher-quality UI font is a later polish step.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### THEME.6: Final theme-module verification sweep

**Files:**
- Test: all of `crates/ui/src/theme/*`
- Modify (only if needed): `crates/ui/src/theme/mod.rs` re-export surface

No new code unless a gap surfaces. This task confirms the whole theme module compiles clean, every public item is documented, all theme tests pass together, and the public surface other layers depend on is exported. The downstream contracts: `Theme`, `ThemeVariant`, `SurfaceTier`, `apply_to_visuals`, `install_fonts`, `wcag_contrast`, `DEFAULT_ACCENT_SEED`, and the inherent `Theme::dark/for_variant/light/accent_high_contrast/surface/accent_seed` - STATE (`Host`, `apply_intent`) and SHELL (regions) consume these by the exact spec names.

- [ ] **Step 1: Run the full theme test set together.**

```
cargo nextest run -p pixhaus-ui --no-fail-fast theme
```

Expected: PASS - all of `theme::contrast::*`, `theme::palettes::*`, `theme::tests::*` (the Visuals mapping), and `theme::fonts::*` green, no failures.

- [ ] **Step 2: Run clippy across all targets at deny-warnings, the Stop-gate bar.**

```
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```

Expected: PASS, zero warnings. The likely offenders and their fixes: `missing_docs` on any newly public item (add the one-line doc), `clippy::cast_possible_truncation`/`clippy::cast_sign_loss` on the `as u8` casts in `palettes.rs` (the `.round().clamp(0.0, 255.0)` bounds them - add a scoped `#[allow(...)]` with a comment, or use `egui::Color32` blend helpers if simpler). Fix every warning; do not silence the whole lint at crate level.

- [ ] **Step 3: Confirm the public re-export surface is complete.** Read `crates/ui/src/theme/mod.rs` and verify it re-exports exactly: `Theme, ThemeVariant, SurfaceTier, Surfaces, Roles, AccentTokens, Elevation, Spacing, TypeScale, Radii` (from `tokens`), `wcag_contrast` (from `contrast`), `DEFAULT_ACCENT_SEED` (from `palettes`), `install_fonts` (from `fonts`), and that `apply_to_visuals` is `pub` directly in `mod.rs`. If `SurfaceTier` is referenced by a downstream layer but was not in the re-export list, add it. These are the names STATE and SHELL import as `pixhaus_ui::theme::*`.

- [ ] **Step 4: Run the crate doc build to confirm rustdoc examples and links resolve (the Stop gate runs `cargo doc`).**

```
cargo doc -p pixhaus-ui --no-deps
```

Expected: PASS, no rustdoc warnings (broken intra-doc links count). If a doc comment references a type with `[...]` link syntax that does not resolve, fix the path or drop the brackets.

- [ ] **Step 5: Commit only if Step 2 or Step 3 required an edit; otherwise skip.** If a clippy fix or a missing re-export was added:

```
git add crates/ui/src/theme/mod.rs crates/ui/src/theme/palettes.rs
git commit -m "$(cat <<'EOF'
fix(ui): clean up theme module lints and re-export surface

Resolve clippy deny-warnings on the theme module and complete the public
re-export surface (SurfaceTier and friends) that the state and shell
layers import.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

If nothing changed, state that the sweep was clean and move on - no empty commit.

## Phase 3: Contribution trait surface (contrib_api)

Only `lib.rs` exists. My layer creates the `contrib_api/` tree from scratch. I have all the contracts I need:

- Newtypes (spec lines 207-210), `ContribCtx`/`PanelScope` (256-270), `Panel`/`PanelMeta` (276-294), `Tool`/`ToolMeta` (306-323), `Workspace`/`WorkspaceMeta`/`WorkspaceLayout`/`StatusItem` (329-354), `Module`/`HostRegistrar`/`ActionDesc`/`MenuGroup`/`MenuItem` (366-382), and the `_assert_boxable` guard (432-435).
- Cross-layer types I import by exact path: `Theme`, `ThemeVariant` (theme layer), `SessionState`, `UiState`, `IntentSink` (state layer). My tasks reference them as `crate::theme::Theme`, `crate::state::{session::SessionState, ui_state::UiState, intent::IntentSink}`.
- `missing_docs = "warn"` + pedantic clippy + `-D warnings` post-edit hook means every public item needs a doc comment.

Now I'll write my layer.

---

# Layer: Contribution trait surface (API)

Implements spec "Identity newtypes" (lines 204-211), "The context handles" (243-270), "Panel" (272-302), "Tool" (304-323), "Workspace" (325-358), "Module and the registrar" (360-383), and the dyn-compatibility guard (429-435). This layer is the permanent trait contract every later layer (registry, state, shell, modules) consumes.

These files live in `crates/ui/src/contrib_api/`. Most are trait/struct definitions with no logic to assert, so they follow a **write -> `cargo build -p pixhaus-ui` -> commit** rhythm. The dyn-compatibility `const _` guard is the one compile-time "test" - if any trait regresses out of object-safety, the crate stops compiling.

**Cross-layer contract - import these by their exact spec paths; assume the other layers define them:**
- `crate::theme::Theme`, `crate::theme::ThemeVariant` (THEME layer)
- `crate::state::session::SessionState` (STATE layer)
- `crate::state::ui_state::UiState` (STATE layer)
- `crate::state::intent::IntentSink` (STATE layer)
- `crate::region::Region` (REGION layer)

**Build-order note for the executing engineer:** This layer's files reference types the THEME, STATE, and REGION layers own. If you are executing layers strictly in order and those are not yet present, `context.rs`, `panel.rs`, and `tool.rs` will not compile in isolation. Two honest options, pick per your plan's global ordering:
- **Preferred:** execute the THEME, STATE-skeleton, and REGION tasks that define `Theme`/`ThemeVariant`/`SessionState`/`UiState`/`IntentSink`/`Region` before this layer, then every `cargo build` step here passes as written.
- **If this layer runs first:** create the four imported types as temporary empty stubs (`pub struct SessionState;` etc.) in their spec module paths so the build is green, and delete the stubs when the owning layer lands. Do not invent fields on them - this layer only names the types, never their internals.

The `lib.rs` wiring (the `pub mod contrib_api;` and `pub mod` lines for sibling layers) is owned by whichever layer the global plan assigns the `lib.rs` module tree to; this layer adds only the `contrib_api` submodule declarations inside `contrib_api/mod.rs` plus the one `pub mod contrib_api;` line in `lib.rs` (Task API.8).

---

### API.1: Identity newtypes (ids.rs)

**Files:**
- Create: `crates/ui/src/contrib_api/ids.rs`
- Test: same file (`#[cfg(test)]` module)

- [ ] **Step 1: Write `ids.rs` with the four `Copy` newtypes.** These are the canonical `Copy` ID types (rust-conventions "Newtype for unit safety"): all four wrap `&'static str` and derive the full hash/eq/debug set so they serve as `HashMap` keys in the registries. The field is `pub` per the spec (lines 207-210). Every public item gets a doc comment (`missing_docs = "warn"` + `-D warnings`).

```rust
//! Stable identity newtypes for the contribution surface.
//!
//! Each wraps a `&'static str` so a `PanelId` can never be confused with a
//! `ToolId` at the type level, and all derive `Copy + Eq + Hash` so they serve
//! directly as registry keys. The inner string is the stable id a module
//! registers under and a workspace layout references by value.

/// Identifies a registered panel (e.g. `PanelId("layers")`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct PanelId(pub &'static str);

/// Identifies a registered tool (e.g. `ToolId("pencil")`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ToolId(pub &'static str);

/// Identifies a registered workspace (e.g. `WorkspaceId("draw")`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct WorkspaceId(pub &'static str);

/// Identifies a registered action - a menu item or command-palette entry.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ActionId(pub &'static str);

#[cfg(test)]
mod tests {
    use super::{ActionId, PanelId, ToolId, WorkspaceId};

    #[test]
    fn distinct_ids_compare_and_hash_independently() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        assert!(set.insert(PanelId("layers")));
        // Same string under a different newtype is a different key.
        assert!(set.insert(PanelId("frames")));
        assert!(!set.insert(PanelId("layers")));
        assert_eq!(PanelId("layers"), PanelId("layers"));
        assert_ne!(PanelId("layers"), PanelId("frames"));
    }

    #[test]
    fn ids_are_copy() {
        // Compiles only because every id is `Copy`: each is used after being passed by value.
        let p = PanelId("p");
        let t = ToolId("t");
        let w = WorkspaceId("w");
        let a = ActionId("a");
        let _ = (p, t, w, a);
        let _ = (p, t, w, a);
    }
}
```

- [ ] **Step 2: Build the crate.** Expect PASS (clean compile).

```powershell
cargo build -p pixhaus-ui
```

- [ ] **Step 3: Run the ids tests.** Expect PASS (2 tests).

```powershell
cargo nextest run -p pixhaus-ui ids::tests
```

- [ ] **Step 4: Commit.**

```powershell
git add crates/ui/src/contrib_api/ids.rs
git commit -m @'
feat(ui): add contribution identity newtypes

PanelId/ToolId/WorkspaceId/ActionId wrap &'static str so they serve as
registry keys and cannot be confused at the type level.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### API.2: Context handles (context.rs)

**Files:**
- Create: `crates/ui/src/contrib_api/context.rs`

The borrow story's core (spec lines 243-270). `ContribCtx` carries a read-only view of session + UI state, the theme, and the single write channel (`IntentSink`). `PanelScope` wraps it and adds the panel's id plus the one `&mut String` scratch carve-out. No logic to assert - write -> build -> commit.

- [ ] **Step 1: Write `context.rs`.** Imports come by exact spec path. The lifetime `'a` ties the borrows to the host fields the shell destructures per frame. The doc comments must name the "contributor physically cannot mutate session/UI state" guarantee (bible rules 12/21) and the scratch carve-out, because that contract is the whole point of these types.

```rust
//! The two context handles carried into a contributor's render code.
//!
//! [`ContribCtx`] is the read view plus the one write channel, shared by tools
//! and (wrapped) by panels. A contributor physically cannot mutate session or
//! UI state through it - the only write path is pushing an [`Intent`] into the
//! sink (bible rules 12/21 enforced by the type system, not by convention).
//!
//! [`PanelScope`] adds what a panel additionally needs: its own [`PanelId`] (so
//! the shell, not the panel, scopes egui ids) and a mutable handle to *this*
//! panel's scratch text buffer only - the single, disjoint exception to
//! "intents are the only write channel", required because [`egui::TextEdit`]
//! needs a live `&mut String` in-frame.
//!
//! [`Intent`]: crate::state::intent::Intent

use crate::contrib_api::ids::PanelId;
use crate::state::intent::IntentSink;
use crate::state::session::SessionState;
use crate::state::ui_state::UiState;
use crate::theme::Theme;

/// Read view plus the one write channel. Carried by tools and (wrapped) by panels.
///
/// The borrows are all of sibling `Host` fields the shell destructures once per
/// region per frame: `session`/`ui_state` are shared, `intents` is the sole
/// mutable handle. Reads go through the shared refs; every state change is an
/// `Intent` pushed into `intents` and applied after the frame's borrows drop.
pub struct ContribCtx<'a> {
    /// Read-only session state (active workspace/tool, jobs, AI status).
    pub session: &'a SessionState,
    /// Read-only UI state (collapse map, zoom, grid, modal, ...).
    pub ui_state: &'a UiState,
    /// The active theme, for token lookups in render code.
    pub theme: &'a Theme,
    /// The write channel for everything except this panel's scratch text.
    pub intents: &'a mut IntentSink,
}

/// What a [`Panel`] sees: a [`ContribCtx`] plus the panel's own id and scratch.
///
/// The shell builds one of these per panel per frame, supplying the panel's
/// [`PanelId`] and a `&mut String` borrowed from that panel's slot in
/// `Host.scratch`. `scratch` is the only mutable handle a panel gets beyond the
/// intent sink, it is private to this panel, and it exists solely so a
/// [`egui::TextEdit`] can bind to it. Routing real model mutation through
/// `scratch` instead of an intent is a review failure.
///
/// [`Panel`]: crate::contrib_api::panel::Panel
pub struct PanelScope<'a> {
    /// The shared read view + intent sink.
    pub ctx: ContribCtx<'a>,
    /// This panel's id - the shell uses it to scope egui ids, the panel does not.
    pub id: PanelId,
    /// A mutable handle to THIS panel's scratch text buffer only.
    pub scratch: &'a mut String,
}
```

- [ ] **Step 2: Build the crate.** Expect PASS. If the imported `SessionState`/`UiState`/`IntentSink`/`Theme` are not yet defined by their layers, you will get unresolved-import errors `E0432` naming those exact paths - that is the build-order note at the top of this layer, not a defect in this file. Resolve by landing those layers first (preferred) or stubbing the named types.

```powershell
cargo build -p pixhaus-ui
```

- [ ] **Step 3: Commit.** No standalone test: these are pure handle structs with public fields and no behavior; the borrow guarantee is exercised by the shell layer's region code and the smoke test, not assertable here.

```powershell
git add crates/ui/src/contrib_api/context.rs
git commit -m @'
feat(ui): add ContribCtx and PanelScope handles

ContribCtx is the read view plus the one intent-sink write channel; PanelScope
adds the panel id and the single &mut String scratch carve-out. Panels cannot
mutate session or UI state - the type system enforces bible rules 12/21.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### API.3: Panel trait and PanelMeta (panel.rs)

**Files:**
- Create: `crates/ui/src/contrib_api/panel.rs`

Spec lines 272-302. The `&self` receiver is load-bearing for the disjoint-field borrow loop and must be documented as such. `relevant_in` has a default body and is a debug-assert hint, not a runtime filter (spec lines 280-283). Trait + plain struct, no logic - write -> build -> commit.

- [ ] **Step 1: Write `panel.rs`.** `default_region: Region` imports the REGION layer's enum by its spec path `crate::region::Region`. Document the object-safety constraints (`&self`, no generics, no `-> Self`) so a future edit does not regress dyn-compatibility. Add `# Object safety` notes on the trait doc.

```rust
//! The `Panel` trait and its metadata.
//!
//! A panel renders representative content into the right dock or bottom tray.
//! It is dyn-compatible on purpose (stored as `Box<dyn Panel>` in the registry):
//! `&self` receivers, no generic methods, no `-> Self`, metadata returned by
//! value. The compile-time guard in `contrib_api::mod` enforces this.

use crate::contrib_api::context::PanelScope;
use crate::contrib_api::ids::PanelId;
use crate::contrib_api::ids::WorkspaceId;
use crate::region::Region;

/// A registered panel: stable identity, metadata, and a render method.
///
/// # Object safety
///
/// Every method takes `&self` and uses no generics or `-> Self`, so `Panel` is
/// dyn-compatible and lives in the registry as `Box<dyn Panel>`. The `&self`
/// receiver is deliberate: a panel holds no mutable state of its own - its
/// collapse flag lives in `UiState`, its draft text in `Host.scratch`. That is
/// what lets the shell iterate `&registry.panels` (a shared borrow) while
/// holding `&mut intents` and `&mut scratch` (sibling `Host` fields) without
/// aliasing.
pub trait Panel {
    /// This panel's stable id - also its registry key.
    fn id(&self) -> PanelId;

    /// Static metadata: title, icon, default placement, default open state.
    fn meta(&self) -> PanelMeta;

    /// Capability predicate: could this panel ever appear in `workspace`?
    ///
    /// The shell uses this only as a `debug_assert` against a workspace's
    /// authored layout - NOT as a runtime placement filter. The
    /// [`WorkspaceLayout`] is the sole placement authority (bible rule 14).
    /// Default: usable anywhere it is listed.
    ///
    /// [`WorkspaceLayout`]: crate::contrib_api::workspace::WorkspaceLayout
    fn relevant_in(&self, _workspace: WorkspaceId) -> bool {
        true
    }

    /// Render representative content.
    ///
    /// Reads through `scope.ctx`; pushes [`Intent`]s into `scope.ctx.intents`;
    /// may edit only `scope.scratch`. Nothing else is mutable.
    ///
    /// [`Intent`]: crate::state::intent::Intent
    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>);
}

/// Static, by-value metadata describing a panel.
///
/// Returned by value (not borrowed) so [`Panel`] stays dyn-compatible.
pub struct PanelMeta {
    /// Display title shown in the card header.
    pub title: &'static str,
    /// Phosphor glyph from [`crate::icons`].
    pub icon: char,
    /// Where this panel sits unless a workspace places it elsewhere.
    pub default_region: Region,
    /// Whether the panel starts expanded.
    pub default_open: bool,
}
```

- [ ] **Step 2: Build the crate.** Expect PASS.

```powershell
cargo build -p pixhaus-ui
```

- [ ] **Step 3: Commit.**

```powershell
git add crates/ui/src/contrib_api/panel.rs
git commit -m @'
feat(ui): add Panel trait and PanelMeta

Dyn-compatible by design: &self receivers, no generics, no -> Self. The &self
receiver lets the shell iterate the panel registry shared while holding the
intent sink and scratch buffer mutable. relevant_in is a debug-assert hint, not
a runtime placement filter - the workspace layout is the placement authority.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### API.4: Tool trait and ToolMeta (tool.rs)

**Files:**
- Create: `crates/ui/src/contrib_api/tool.rs`

Spec lines 304-323. A tool is not a panel: `options_ui` takes a bare `ContribCtx` (no `PanelScope`, no scratch, no id). `ToolMeta.shortcut` is `Option<egui::KeyboardShortcut>` and `is_ai` flips the AI Brush styling. Document the reserved `on_pointer` seam as a doc-comment only (spec line 312) - do not add it.

- [ ] **Step 1: Write `tool.rs`.** `egui::KeyboardShortcut` is a real 0.34 type (confirmed: `KeyboardShortcut::new(Modifiers, Key)`); name it by full path so no extra import is needed beyond `egui`.

```rust
//! The `Tool` trait and its metadata.
//!
//! A tool contributes options into the tool-options bar when active. Like
//! [`Panel`], it is dyn-compatible (`Box<dyn Tool>` in the registry): `&self`
//! receivers, no generics, no `-> Self`, metadata by value.
//!
//! [`Panel`]: crate::contrib_api::panel::Panel

use crate::contrib_api::context::ContribCtx;
use crate::contrib_api::ids::ToolId;

/// A registered tool: stable identity, metadata, and an options renderer.
///
/// # Object safety
///
/// `&self` receivers, no generic methods, no `-> Self`: dyn-compatible, stored
/// as `Box<dyn Tool>`.
pub trait Tool {
    /// This tool's stable id - also its registry key.
    fn id(&self) -> ToolId;

    /// Static metadata: label, icon, shortcut, tooltip, AI marker.
    fn meta(&self) -> ToolMeta;

    /// Render this tool's options into the tool-options bar when active.
    ///
    /// Takes a bare [`ContribCtx`] - a tool is not a panel, so it has no
    /// [`PanelId`] and no scratch buffer. State changes go through
    /// `cx.intents`.
    ///
    /// [`PanelId`]: crate::contrib_api::ids::PanelId
    fn options_ui(&self, ui: &mut egui::Ui, cx: &mut ContribCtx<'_>);

    // When `core` lands, `fn on_pointer(&self, ev, &mut CommandSink)` arrives
    // here, additive (bible rules 3/4). Tools emit no canvas commands this round.
}

/// Static, by-value metadata describing a tool.
pub struct ToolMeta {
    /// Display label shown in tooltips and the command palette.
    pub label: &'static str,
    /// Phosphor glyph from [`crate::icons`] painted on the rail button.
    pub icon: char,
    /// Optional keyboard shortcut (e.g. `B` for pencil). `None` means no key.
    pub shortcut: Option<egui::KeyboardShortcut>,
    /// One-line help, e.g. "Draw individual pixels. Hold Shift for a line.".
    pub tooltip: &'static str,
    /// The AI Brush flips this - it renders with the accent AI tint + sparkle.
    pub is_ai: bool,
}
```

- [ ] **Step 2: Build the crate.** Expect PASS.

```powershell
cargo build -p pixhaus-ui
```

- [ ] **Step 3: Commit.**

```powershell
git add crates/ui/src/contrib_api/tool.rs
git commit -m @'
feat(ui): add Tool trait and ToolMeta

options_ui takes a bare ContribCtx - a tool is not a panel, so no PanelScope,
scratch, or id. ToolMeta carries an optional shortcut and the is_ai flag that
the AI Brush sets for its accent tint and sparkle. The on_pointer command seam
is a doc-comment reservation for when core lands.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### API.5: Workspace trait, WorkspaceMeta, WorkspaceLayout, StatusItem (workspace.rs)

**Files:**
- Create: `crates/ui/src/contrib_api/workspace.rs`
- Test: same file (`#[cfg(test)]` module)

Spec lines 325-358. `WorkspaceLayout` and `StatusItem` derive `Clone + PartialEq + Debug` because `Debug` makes them insta-snapshottable (the registry layer's snapshot test target). `bottom_tray` is `Vec<PanelId>` (the resolved multi-tab decision). `WorkspaceMeta.shortcut` is a non-optional `egui::KeyboardShortcut` (Cmd+1..5). This file has a small testable assertion (the derives round-trip via `Clone`/`PartialEq`), so use the TDD-lite rhythm.

- [ ] **Step 1: Write `workspace.rs` with the trait, the three structs, and a failing-first test scaffold.** Write the test first referencing the structs, then fill them in - but since they are plain data, write both together and run once.

```rust
//! The `Workspace` trait and its layout types.
//!
//! A workspace owns layout only - which registered panels and tools fill which
//! region - and never owns data (bible: Draw and Animate are siblings over one
//! sprite-editing core). It is dyn-compatible (`Box<dyn Workspace>`).
//!
//! [`WorkspaceLayout`] and [`StatusItem`] derive `Clone + PartialEq + Debug` so
//! the resolved layout is insta-snapshottable - the registry layer's
//! highest-value regression test.

use crate::contrib_api::ids::{PanelId, ToolId, WorkspaceId};

/// A registered workspace: identity, metadata, and a pure layout function.
///
/// # Object safety
///
/// `&self`, no generics, no `-> Self`: dyn-compatible.
pub trait Workspace {
    /// This workspace's stable id - also its registry key.
    fn id(&self) -> WorkspaceId;

    /// Static metadata: name, icon, purpose, and the Cmd+1..5 shortcut.
    fn meta(&self) -> WorkspaceMeta;

    /// Pure: which registered panels/tools fill which region.
    ///
    /// No egui, no mutation - returns ids only; the shell resolves them against
    /// the registries. This is the snapshot-test target.
    fn layout(&self) -> WorkspaceLayout;
}

/// Static, by-value metadata describing a workspace.
pub struct WorkspaceMeta {
    /// Display name, e.g. "Draw".
    pub name: &'static str,
    /// Phosphor glyph for the workspace tab.
    pub icon: char,
    /// Tooltip / command-palette description.
    pub purpose: &'static str,
    /// The activation shortcut, `Modifiers::COMMAND` + `Key::Num1..Num5`.
    pub shortcut: egui::KeyboardShortcut,
}

/// Where a workspace places registered panels and tools, by id.
///
/// `layout()` returns owned `Vec`s of `Copy` ids - cheap to call once per frame
/// for the active workspace; no panel object moves.
#[derive(Clone, PartialEq, Debug)]
pub struct WorkspaceLayout {
    /// Right-dock card stack, top to bottom.
    pub right_dock: Vec<PanelId>,
    /// Bottom-tray tabs, left to right; the first is the default selected tab.
    pub bottom_tray: Vec<PanelId>,
    /// The ordered subset of tools shown in the left rail.
    pub primary_tools: Vec<ToolId>,
    /// The tool selected when this workspace activates.
    pub default_tool: ToolId,
    /// Workspace-specific status-bar entries.
    pub status_items: Vec<StatusItem>,
}

/// A single status-bar entry: an icon glyph and its label.
///
/// `text` is an owned `String` (not `&'static str`) so a future status item can
/// be computed; strings only, so the type stays `Debug`-snapshottable.
#[derive(Clone, PartialEq, Debug)]
pub struct StatusItem {
    /// Phosphor glyph shown before the text.
    pub icon: char,
    /// The status label, e.g. "Pixel Grid On".
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::{StatusItem, WorkspaceLayout};
    use crate::contrib_api::ids::{PanelId, ToolId};

    #[test]
    fn layout_is_clone_eq_debug() {
        let layout = WorkspaceLayout {
            right_dock: vec![PanelId("layers"), PanelId("palette")],
            bottom_tray: vec![PanelId("frames"), PanelId("console")],
            primary_tools: vec![ToolId("pencil")],
            default_tool: ToolId("pencil"),
            status_items: vec![StatusItem {
                icon: '#',
                text: "Pixel Grid On".to_owned(),
            }],
        };
        // Clone + PartialEq round-trip (the snapshot test relies on both).
        assert_eq!(layout.clone(), layout);
        // Debug is populated (insta-snapshottable).
        assert!(format!("{layout:?}").contains("Pixel Grid On"));
    }
}
```

- [ ] **Step 2: Run the workspace test (it also forces a build).** Expect PASS (1 test). If `egui::KeyboardShortcut` resolves and the ids layer is present, this compiles and passes.

```powershell
cargo nextest run -p pixhaus-ui workspace::tests
```

- [ ] **Step 3: Commit.**

```powershell
git add crates/ui/src/contrib_api/workspace.rs
git commit -m @'
feat(ui): add Workspace trait and layout types

WorkspaceLayout/StatusItem derive Clone+PartialEq+Debug so the resolved layout
snapshots under insta. bottom_tray is a Vec<PanelId> (the multi-tab decision);
the first entry is the default tab. Workspaces own layout only, never data.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### API.6: Module trait, HostRegistrar, ActionDesc, MenuGroup/MenuItem (module.rs)

**Files:**
- Create: `crates/ui/src/contrib_api/module.rs`

Spec lines 360-383, plus `MenuGroup`/`MenuItem` referenced by `add_menu_group` (line 371) and the menus section (lines 998-999, 1011). `HostRegistrar` is a `dyn` trait so a module never sees the concrete `Registries`. `ActionDesc` carries `palette_visible`. The spec shows `MenuGroup` in the registrar signature but its field shape is given in the menus section (`MenuGroup { label, items: Vec<MenuItem { label, shortcut, action: ActionId }> }`, line 999) - define both here so `module.rs` is self-contained.

- [ ] **Step 1: Write `module.rs`.** `MenuItem.shortcut` is `Option<egui::KeyboardShortcut>` (a menu item may have no accelerator). `MenuItem.action` is `ActionId`. All public fields and items documented.

```rust
//! The `Module` trait, the `HostRegistrar` it registers through, and the
//! by-value descriptors a module contributes.
//!
//! A module is the only path a capability enters the shell (bible rule:
//! capabilities are registered by internal modules through registries, no
//! external dynamic plugins). It registers through [`HostRegistrar`], a `dyn`
//! trait, so a module never sees the concrete `Registries`.

use crate::contrib_api::ids::ActionId;
use crate::contrib_api::panel::Panel;
use crate::contrib_api::tool::Tool;
use crate::contrib_api::workspace::Workspace;

/// The registration front door handed to each [`Module`].
///
/// # Object safety
///
/// `&mut self`, no generics, no `-> Self`: dyn-compatible, passed as
/// `&mut dyn HostRegistrar` so the module is decoupled from the concrete
/// registry storage.
pub trait HostRegistrar {
    /// Register a panel; its key is `panel.id()`.
    fn add_panel(&mut self, panel: Box<dyn Panel>);
    /// Register a tool; its key is `tool.id()`.
    fn add_tool(&mut self, tool: Box<dyn Tool>);
    /// Register a workspace; its key is `ws.id()`.
    fn add_workspace(&mut self, ws: Box<dyn Workspace>);
    /// Register an action (a menu item / command-palette entry).
    fn add_action(&mut self, action: ActionDesc);
    /// Contribute a top-bar menu group (Sprite/Layer/Frame menus, ...).
    fn add_menu_group(&mut self, group: MenuGroup);
    // add_importer/exporter/provider/validator land with their registries later.
}

/// A registerable action: id, label, icon, and command-palette visibility.
pub struct ActionDesc {
    /// Stable id - also the action registry key.
    pub id: ActionId,
    /// Display label.
    pub label: &'static str,
    /// Phosphor glyph from [`crate::icons`].
    pub icon: char,
    /// Whether this action appears in the Ctrl/Cmd+K command palette.
    pub palette_visible: bool,
}

/// A top-bar menu group, e.g. "Sprite" with its items.
pub struct MenuGroup {
    /// The menu button label, e.g. "Sprite".
    pub label: &'static str,
    /// The items under this group, rendered top to bottom.
    pub items: Vec<MenuItem>,
}

/// A single menu item: a label, an optional accelerator, and the action it fires.
pub struct MenuItem {
    /// Display label, e.g. "New".
    pub label: &'static str,
    /// Optional accelerator shown beside the label; `None` means no shortcut.
    pub shortcut: Option<egui::KeyboardShortcut>,
    /// The action dispatched when the item is clicked.
    pub action: ActionId,
}

/// A compiled-in capability bundle: a workspace and its panels/tools/menus.
///
/// # Object safety
///
/// `&self`, no generics, no `-> Self`: dyn-compatible, boxed in `app`'s module
/// list. `register` is the only path a module's capabilities enter the shell.
pub trait Module {
    /// The module's stable id, e.g. "sprite-edit".
    fn id(&self) -> &'static str;

    /// Register every capability this module contributes, through `host`.
    fn register(&self, host: &mut dyn HostRegistrar);
}
```

- [ ] **Step 2: Build the crate.** Expect PASS.

```powershell
cargo build -p pixhaus-ui
```

- [ ] **Step 3: Commit.**

```powershell
git add crates/ui/src/contrib_api/module.rs
git commit -m @'
feat(ui): add Module trait, HostRegistrar, and menu descriptors

HostRegistrar is a dyn trait so a module never touches the concrete Registries;
register is the only path a capability enters the shell. ActionDesc carries
palette_visible; MenuGroup/MenuItem model the data-driven top-bar menus.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### API.7: Module assembly and the dyn-compatibility guard (mod.rs)

**Files:**
- Create: `crates/ui/src/contrib_api/mod.rs`

Declares the submodules, re-exports the public trait surface so consumers write `pixhaus_ui::contrib_api::{Module, HostRegistrar, Panel, ...}` (matching the spec's `app`/module wiring at lines 129 and 151), and carries the compile-time dyn-compatibility guard (spec lines 429-435). The guard is this layer's "test": if any of the four traits regresses out of object-safety (a generic method, a `-> Self`, a non-`&self`/`&mut self` receiver), the `const _` block fails to compile and the whole crate stops building.

- [ ] **Step 1: Write `mod.rs` with submodule declarations, re-exports, and the guard.**

```rust
//! The permanent contribution trait surface.
//!
//! These traits and descriptors are the stable contract every module registers
//! through and the shell consumes. All four registry traits ([`Panel`],
//! [`Tool`], [`Workspace`], [`Module`]) are dyn-compatible and stored as
//! `Box<dyn _>` - registries are the textbook heterogeneous-collection case and
//! none sits on the per-pixel hot path, so the vtable hop is free. The
//! [`_assert_boxable`] guard below fails the build if any of them regresses.

pub mod context;
pub mod ids;
pub mod module;
pub mod panel;
pub mod tool;
pub mod workspace;

pub use context::{ContribCtx, PanelScope};
pub use ids::{ActionId, PanelId, ToolId, WorkspaceId};
pub use module::{ActionDesc, HostRegistrar, MenuGroup, MenuItem, Module};
pub use panel::{Panel, PanelMeta};
pub use tool::{Tool, ToolMeta};
pub use workspace::{StatusItem, Workspace, WorkspaceLayout, WorkspaceMeta};

/// Compile-time dyn-compatibility guard on the actual storage form.
///
/// If any registry trait gains a generic method, a `-> Self`, or a by-value
/// receiver, it stops being dyn-compatible and this block fails to compile -
/// the crate stops building immediately (test plan item 5). Free and permanent.
const _: () = {
    fn _assert_boxable(
        _: Box<dyn Panel>,
        _: Box<dyn Tool>,
        _: Box<dyn Workspace>,
        _: Box<dyn Module>,
    ) {
    }
};
```

- [ ] **Step 2: Build the crate - the guard is the test.** Expect PASS. A clean build means all four traits are dyn-compatible. To prove the guard bites, you may temporarily add `fn _bad() -> Self where Self: Sized;` to `Panel` and rebuild: expect compile error `E0038` ("the trait `Panel` cannot be made into an object") or similar - then revert. (Optional verification; do not commit the temporary change.)

```powershell
cargo build -p pixhaus-ui
```

- [ ] **Step 3: Run clippy on the crate to confirm zero warnings (doc coverage + pedantic).** Expect PASS. `missing_docs` is a workspace `warn` and the gate runs `-D warnings`, so any undocumented public item fails here.

```powershell
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```

- [ ] **Step 4: Commit.**

```powershell
git add crates/ui/src/contrib_api/mod.rs
git commit -m @'
feat(ui): assemble contrib_api and add the dyn-compat guard

mod.rs declares the submodules, re-exports the trait surface, and carries the
compile-time _assert_boxable guard: if any registry trait regresses out of
object safety the crate stops compiling (test plan item 5).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### API.8: Wire contrib_api into the crate root (lib.rs)

**Files:**
- Modify: `crates/ui/src/lib.rs`

Adds the `pub mod contrib_api;` declaration and the test-only clippy allow at the crate root (so the `ids`/`workspace` tests' assertions follow conventions per spec line 1042 and the machine convention). Only touch the two lines this layer owns; the sibling-layer `pub mod theme;`/`pub mod state;`/`pub mod region;` declarations belong to those layers - add them only if they are not already present and your global plan assigns the lib.rs tree here. If a coordinating "lib.rs module tree" task exists in another layer, skip the `pub mod contrib_api;` insertion here and let that task own it; this task then only adds the crate-level attribute.

- [ ] **Step 1: Add the test-only clippy allow at the very top of `lib.rs`** (above the existing `//!` doc comment is not allowed - inner attributes follow the module doc; place it immediately after the doc comment block, before the first `use`). Read the current top of the file first; the existing module doc ends at line 11 and `use` begins at line 13.

Edit `crates/ui/src/lib.rs` - insert the attribute between the closing of the `//!` doc block (line 11) and the first `use` (line 13):

```rust
//! workspaces are built.

#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

use egui::epaint::PaintCallbackInfo;
```

(The `//! workspaces are built.` line is the existing last doc line; match it exactly when anchoring the edit.)

- [ ] **Step 2: Add the `contrib_api` module declaration.** Place `pub mod contrib_api;` after the inserted attribute and before the first `use`, OR after the existing `use` block if you prefer module decls grouped - the repo style puts `pub mod` declarations after the crate attributes. Insert immediately after the `#![cfg_attr(...)]` block:

```rust
#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

/// The permanent contribution trait surface (Panel/Tool/Workspace/Module).
pub mod contrib_api;

use egui::epaint::PaintCallbackInfo;
```

- [ ] **Step 3: Build and run the full ui crate test set.** Expect PASS - the existing `canvas_callback_is_zero_sized` test plus this layer's `ids` and `workspace` tests.

```powershell
cargo build -p pixhaus-ui
cargo nextest run -p pixhaus-ui
```

- [ ] **Step 4: Run clippy across all targets for the crate.** Expect PASS (zero warnings).

```powershell
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```

- [ ] **Step 5: Run doc tests for the crate** (the doc comments contain `[`...`]` intra-doc links; `cargo test --doc` confirms they resolve and nothing in the rustdoc breaks). Expect PASS (0 doc tests run, links checked at doc-build time - also run `cargo doc` if your gate requires it).

```powershell
cargo test --doc -p pixhaus-ui
```

- [ ] **Step 6: Commit.**

```powershell
git add crates/ui/src/lib.rs
git commit -m @'
feat(ui): wire contrib_api into the ui crate root

Declares pub mod contrib_api and adds the test-only clippy allow so the trait
surface's tests may use unwrap/expect per the testing conventions.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

**Layer-complete verification (run after API.8, before handing off to the registry layer):**

- [ ] **Final check: the whole contrib_api surface compiles, lints clean, and its tests pass.** Expect all PASS.

```powershell
cargo build -p pixhaus-ui; if ($?) { cargo clippy -p pixhaus-ui --all-targets -- -D warnings }; if ($?) { cargo nextest run -p pixhaus-ui }
```

The registry layer consumes `contrib_api::{Panel, Tool, Workspace, Module, HostRegistrar, ActionDesc, MenuGroup, PanelId, ToolId, WorkspaceId, ActionId}`; the state layer consumes `PanelId`/`ToolId`/`WorkspaceId` and is consumed by `context.rs`; the shell layer consumes `ContribCtx`/`PanelScope`. All are now exported from `pixhaus_ui::contrib_api`.

**Two pinned-API confirmations this layer establishes for downstream layers** (verified against egui/epaint 0.34.2 source in the cargo registry): `egui::KeyboardShortcut::new(Modifiers, Key)` and `egui::Key::Num1..Num5` exist and are used in `WorkspaceMeta.shortcut`, `ToolMeta.shortcut`, `MenuItem.shortcut`. The SHADOW/focus-query items the spec flagged "verify" belong to the THEME and SHORTCUTS layers, but for the record: `egui::epaint::Shadow` fields are `offset: [i8; 2]`, `blur: u8`, `spread: u8`, `color: Color32` (with a `Shadow::NONE` const), and the focus gate is `ctx.text_edit_focused()` - note `ctx.wants_keyboard_input()` is **deprecated** in 0.34.2 (renamed `egui_wants_keyboard_input`) and would trip `-D warnings`.

## Phase 4: Registries and region

I have what I need. My layer (REG) doesn't directly touch `Shadow` fields or the focus-query helper (those belong to the THEME and SHORTCUTS layers respectively), but I need to verify the egui 0.34 API for `egui::Panel`, `ui.push_id`, and the `tracing::warn!` usage that my resolve code touches. Let me quickly confirm the `tracing` dependency situation and the egui Panel/Id API since my Region module declares Id constants.

The spec is the authority on signatures, and I have all the type contracts from it. I now have enough to write the REG layer plan precisely. Let me produce it.

### REG layer plan

---

### Coverage note

This layer implements the spec's "Trait surface and registries" sections: Region (spec lines 215-242), Registries / `Registry<K,V>` / `Registries` / `HostRegistrar` impl (spec lines 391-435), and Layout resolution / `resolve_layout` / `ResolvedLayout` (spec lines 438-474). It also lands spec test 1 (registry registration, lines 1045-1049) and a self-contained `resolve_layout` unit test with hand-built fakes; the full five-module `insta` snapshot (spec test 2, lines 1050-1053) is **deferred to the TESTS layer** because it requires every `modules/*` crate registered - this layer cannot depend on the module crates without violating the dependency direction. The shared contract types this layer consumes from other layers - `PanelId`, `ToolId`, `WorkspaceId`, `ActionId` (IDS layer), `Panel`, `Tool`, `Workspace`, `Module`, `PanelMeta`, `WorkspaceLayout`, `StatusItem`, `ActionDesc`, `MenuGroup` (TRAITS layer) - are referenced by their exact spec names and assumed to exist.

---

### REG.1: Add the `tracing` dependency to the ui crate

**Files:**
- Modify: `crates/ui/Cargo.toml`

`resolve_layout` calls `tracing::warn!` on a missing panel id (spec line 458). `tracing` is in the workspace dependency catalog (`Cargo.toml` line 31) but `pixhaus-ui` does not yet pull it in.

- [ ] **Step 1: Add `tracing` to `crates/ui/Cargo.toml`.** Insert the workspace dependency under `[dependencies]`, after the `wgpu.workspace = true` line:

```toml
[dependencies]
egui.workspace = true
egui-wgpu.workspace = true
wgpu.workspace = true
tracing.workspace = true
pixhaus-render = { path = "../render" }
```

- [ ] **Step 2: Verify the crate still builds with the new dependency.** Run in PowerShell:

```powershell
cargo build -p pixhaus-ui
```

Expected: PASS (compiles clean; `tracing` is already in the lockfile from the workspace catalog, so no new download).

- [ ] **Step 3: Commit.** Run in PowerShell:

```powershell
git add crates/ui/Cargo.toml
git commit -m @'
chore(ui): pull tracing into the ui crate

resolve_layout warns on an unregistered panel id; tracing is in the
workspace catalog but pixhaus-ui did not yet depend on it.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### REG.2: Region enum and per-region Id constants

**Files:**
- Create: `crates/ui/src/region.rs`
- Modify: `crates/ui/src/lib.rs` (add `pub mod region;`)
- Test: `crates/ui/src/region.rs` (inline `#[cfg(test)]`)

Implements spec lines 215-242. This is a pure-data file (an enum plus `&'static str` constants), so the rhythm is write -> `cargo build` -> add a thin uniqueness test -> commit. The constants exist to give each egui side/top/bottom panel a unique stable `Id`; a one-line test that asserts they are all distinct catches a copy-paste typo and is honest (it asserts something real).

- [ ] **Step 1: Create `crates/ui/src/region.rs` with the enum and the `region_id` module.** Copy the shapes exactly from the spec (lines 217-236); add the crate-required module/item docs (`missing_docs` is `warn` workspace-wide, so every public item needs a doc comment):

```rust
//! Window regions and their stable egui [`Id`](egui::Id) source strings.
//!
//! The shell draws seven regions every frame (architecture bible section 8). The
//! [`Region`] enum names them; [`region_id`] holds the stable id source strings
//! each `egui` side/top/bottom panel needs so its layout memory survives across
//! frames. Only the registry-fed regions (`LeftRail`, `RightDock`, `BottomTray`)
//! are populated from the registries; the rest are shell chrome.

/// The seven window regions the shell composes each frame.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Region {
    /// Shell chrome: menus + workspace tabs + global status.
    TopBar,
    /// Driven by the active [`Tool`](crate::contrib_api::Tool), not the panel registry.
    ToolOptions,
    /// Filled from the tool registry, workspace-filtered.
    LeftRail,
    /// Shell chrome: the canvas stage; embeds the canvas paint callback.
    Center,
    /// Filled from the panel registry: a top-to-bottom card stack.
    RightDock,
    /// Filled from the panel registry: a tab row plus the selected panel.
    BottomTray,
    /// Shell chrome plus the active workspace's status items.
    StatusBar,
}

/// Stable id source strings for the regions egui draws as panels.
///
/// Each `egui` panel needs a unique stable id so its size and scroll memory
/// persist across frames. The four chrome regions (top bar, tool options, status
/// bar, canvas stage) and the three registry-fed regions each get one.
pub mod region_id {
    /// Id source for the top bar panel.
    pub const TOP_BAR: &str = "pixhaus.topbar";
    /// Id source for the tool-options panel.
    pub const TOOL_OPTIONS: &str = "pixhaus.tooloptions";
    /// Id source for the left tool rail panel.
    pub const LEFT_RAIL: &str = "pixhaus.rail";
    /// Id source for the right dock panel.
    pub const RIGHT_DOCK: &str = "pixhaus.dock";
    /// Id source for the bottom tray panel.
    pub const BOTTOM_TRAY: &str = "pixhaus.tray";
    /// Id source for the status bar panel.
    pub const STATUS_BAR: &str = "pixhaus.status";
}
```

- [ ] **Step 2: Wire the module into `crates/ui/src/lib.rs`.** Add a module declaration directly below the crate-level doc comment block, before the `use egui::epaint::PaintCallbackInfo;` line:

```rust
pub mod region;
```

- [ ] **Step 3: Build to confirm the module compiles and is wired.** Run in PowerShell:

```powershell
cargo build -p pixhaus-ui
```

Expected: PASS.

- [ ] **Step 4: Add an inline uniqueness test for the id constants.** Append to `crates/ui/src/region.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::region_id;

    /// Every region id source string must be distinct, or two egui panels share
    /// layout memory and one silently inherits the other's size.
    #[test]
    fn region_ids_are_unique() {
        let ids = [
            region_id::TOP_BAR,
            region_id::TOOL_OPTIONS,
            region_id::LEFT_RAIL,
            region_id::RIGHT_DOCK,
            region_id::BOTTOM_TRAY,
            region_id::STATUS_BAR,
        ];
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(unique.len(), ids.len(), "region id source strings must be unique");
    }
}
```

- [ ] **Step 5: Run the test.** Run in PowerShell:

```powershell
cargo nextest run -p pixhaus-ui region_ids_are_unique
```

Expected: PASS (1 test run, 0 failed).

- [ ] **Step 6: Commit.** Run in PowerShell:

```powershell
git add crates/ui/src/region.rs crates/ui/src/lib.rs
git commit -m @'
feat(ui): add Region enum and per-region egui id constants

The seven window regions the shell composes and the stable id source
strings each egui panel needs. A uniqueness test guards against a
copy-paste collision that would alias two panels' layout memory.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### REG.3: The `registry` module skeleton and the test-crate-root allow

**Files:**
- Create: `crates/ui/src/registry/mod.rs`
- Create: `crates/ui/src/registry/resolve.rs` (empty placeholder this task; filled in REG.6)
- Modify: `crates/ui/src/lib.rs` (add `pub mod registry;` and the test-root allow attribute)

This task lays the module wiring and the crate-root test allow so later tasks have a place to land code. `Registry<K,V>` needs `std::collections::HashMap` and `std::hash::Hash`. The crate root must carry `#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]` (per the task brief and spec line 1042) so the upcoming `#[should_panic]` and snapshot tests can use `unwrap`/`expect` without tripping the denied lints.

- [ ] **Step 1: Add the crate-root test allow to `crates/ui/src/lib.rs`.** Insert as the very first line of the file, above the `//!` crate doc comment (inner attributes must precede other items but the doc comment is also an inner attribute, so place it first):

```rust
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
```

- [ ] **Step 2: Declare the `registry` module in `crates/ui/src/lib.rs`.** Add directly below the `pub mod region;` line from REG.2:

```rust
pub mod registry;
```

- [ ] **Step 3: Create the `resolve` submodule placeholder `crates/ui/src/registry/resolve.rs`.** Write a documented but empty module so `mod resolve;` in `mod.rs` resolves; REG.6 fills it:

```rust
//! Layout resolution: turn a workspace's authored layout into a [`ResolvedLayout`]
//! filtered to actually-registered ids. See [`resolve_layout`].
```

- [ ] **Step 4: Create `crates/ui/src/registry/mod.rs` with imports, the module doc, and a `mod resolve; pub use` line. Leave the type bodies for the next tasks.** Write:

```rust
//! The capability registries and the dyn registrar a module registers through.
//!
//! [`Registry`] is an insertion-ordered keyed store (the order is the rail/tab
//! display order); [`Registries`] bundles the per-kind registries; a thin wrapper
//! over `&mut Registries` implements the [`HostRegistrar`](crate::contrib_api::HostRegistrar)
//! a [`Module`](crate::contrib_api::Module) sees. [`resolve_layout`] turns a
//! workspace's authored layout into a [`ResolvedLayout`] filtered to registered ids.

use std::collections::HashMap;
use std::hash::Hash;

mod resolve;

pub use resolve::{resolve_layout, ResolvedLayout};
```

- [ ] **Step 5: Build. Expect a controlled failure - `resolve` exports nothing yet.** Run in PowerShell:

```powershell
cargo build -p pixhaus-ui
```

Expected: FAIL with `error[E0432]: unresolved import` on `resolve::{resolve_layout, ResolvedLayout}` (and an unused-import warning on `HashMap`/`Hash`). This confirms the wiring is correct and the next tasks have a slot to fill. Do not commit yet - proceed to REG.4, which removes the failure path by filling the types in the same module.

---

### REG.4: `Registry<K,V>` and the `Registries` struct

**Files:**
- Modify: `crates/ui/src/registry/mod.rs`
- Test: `crates/ui/src/registry/mod.rs` (inline `#[cfg(test)]`)

Implements spec lines 393-421. `Registry<K,V>` is a `Vec` (display order) plus a `HashMap` index, with a `debug_assert` that panics on a duplicate id (spec lines 401-406). This task has real logic - insertion order, get-by-key, duplicate handling - so use TDD: write the failing test first, then implement. The `Box<dyn Trait>` type aliases and the `Registries` struct depend on the IDS and TRAITS layers' types (`PanelId`, `ToolId`, `WorkspaceId`, `ActionId`, `Panel`, `Tool`, `Workspace`, `ActionDesc`, `MenuGroup`); reference them by exact spec name from `crate::contrib_api`.

- [ ] **Step 1: Write the failing unit test for `Registry` first.** Append to `crates/ui/src/registry/mod.rs`. The test uses a local `u32`-keyed `Registry<u32, &'static str>` so it exercises the generic behavior without depending on the trait-object aliases:

```rust
#[cfg(test)]
mod tests {
    use super::Registry;

    /// `insert` preserves insertion order in `iter()` and indexes by key for `get`.
    #[test]
    fn insert_keeps_order_and_indexes() {
        let mut reg: Registry<u32, &'static str> = Registry::default();
        reg.insert(10, "first");
        reg.insert(20, "second");
        reg.insert(30, "third");

        let order: Vec<&&str> = reg.iter().collect();
        assert_eq!(order, vec![&"first", &"second", &"third"]);
        assert_eq!(reg.get(20), Some(&"second"));
        assert_eq!(reg.get(99), None);
    }

    /// A duplicate id is a programming error: it must `debug_assert`-panic in debug.
    #[test]
    #[should_panic(expected = "duplicate registry id")]
    fn duplicate_id_panics_in_debug() {
        let mut reg: Registry<u32, &'static str> = Registry::default();
        reg.insert(1, "a");
        reg.insert(1, "b");
    }
}
```

- [ ] **Step 2: Run the test to confirm it fails to compile (the type does not exist yet).** Run in PowerShell:

```powershell
cargo nextest run -p pixhaus-ui --no-run
```

Expected: FAIL - compile error `cannot find type Registry` / `no function or associated item named default`. This is the red state: the test names the API before it exists.

- [ ] **Step 3: Implement `Registry<K,V>`.** Insert into `crates/ui/src/registry/mod.rs` between the `pub use resolve::...;` line and the `#[cfg(test)]` block. Copy the body from the spec (lines 393-409), add a manual `Default` impl (deriving `Default` would wrongly require `K: Default`/`V: Default`), and add the required item docs:

```rust
/// An insertion-ordered, key-indexed capability store.
///
/// `items` keeps registration order, which is the display order for the tool
/// rail and tray tabs; `index` maps an id to its slot for O(1) `get`. A module
/// registering a duplicate id is a programming error, loud in debug (the
/// `debug_assert` in [`Registry::insert`]) and last-value-wins in release.
pub struct Registry<K: Copy + Eq + Hash, V> {
    items: Vec<V>,
    index: HashMap<K, usize>,
}

impl<K: Copy + Eq + Hash, V> Default for Registry<K, V> {
    fn default() -> Self {
        Self { items: Vec::new(), index: HashMap::new() }
    }
}

impl<K: Copy + Eq + Hash, V> Registry<K, V> {
    /// Registers `value` under `key`, appending it in display order.
    ///
    /// A duplicate `key` is a programming error: it `debug_assert`-panics in debug
    /// and overwrites the existing slot (last value wins) in release.
    fn insert(&mut self, key: K, value: V) {
        debug_assert!(!self.index.contains_key(&key), "duplicate registry id");
        match self.index.get(&key).copied() {
            Some(i) => self.items[i] = value,
            None => {
                self.index.insert(key, self.items.len());
                self.items.push(value);
            }
        }
    }

    /// Returns the value registered under `key`, or `None` if absent.
    pub fn get(&self, key: K) -> Option<&V> {
        self.index.get(&key).map(|&i| &self.items[i])
    }

    /// Iterates the registered values in registration (display) order.
    pub fn iter(&self) -> impl Iterator<Item = &V> {
        self.items.iter()
    }
}
```

- [ ] **Step 4: Run the two `Registry` tests to confirm green.** Run in PowerShell:

```powershell
cargo nextest run -p pixhaus-ui insert_keeps_order_and_indexes duplicate_id_panics_in_debug
```

Expected: PASS - 2 tests run, 0 failed (`duplicate_id_panics_in_debug` passes by catching the expected panic).

- [ ] **Step 5: Add the type aliases and the `Registries` struct.** Insert into `crates/ui/src/registry/mod.rs` directly after the `Registry` impl block (before `#[cfg(test)]`). Copy from the spec (lines 411-421); reference the trait/id types from `crate::contrib_api` (IDS + TRAITS layers). Add the import line and the docs:

```rust
use crate::contrib_api::{
    ActionDesc, ActionId, MenuGroup, Panel, PanelId, Tool, ToolId, Workspace, WorkspaceId,
};

/// The panel registry: registered panels keyed by [`PanelId`].
pub type PanelRegistry = Registry<PanelId, Box<dyn Panel>>;
/// The tool registry: registered tools keyed by [`ToolId`].
pub type ToolRegistry = Registry<ToolId, Box<dyn Tool>>;
/// The workspace registry: registered workspaces keyed by [`WorkspaceId`].
pub type WorkspaceRegistry = Registry<WorkspaceId, Box<dyn Workspace>>;

/// The full set of capability registries a [`Module`](crate::contrib_api::Module)
/// contributes into and the shell reads each frame.
#[derive(Default)]
pub struct Registries {
    /// Registered panels, in registration order.
    pub panels: PanelRegistry,
    /// Registered tools, in registration order.
    pub tools: ToolRegistry,
    /// Registered workspaces, in registration order.
    pub workspaces: WorkspaceRegistry,
    /// Registered actions (palette / menu targets), in registration order.
    pub actions: Registry<ActionId, ActionDesc>,
    /// Top-bar menu groups, in contribution order.
    pub menus: Vec<MenuGroup>,
}
```

Note on `#[derive(Default)]` for `Registries`: it requires every field to be `Default`. `Registry<K,V>: Default` holds for any `K: Copy + Eq + Hash` and any `V` via the manual impl in Step 3 (it has no `V: Default` bound), and `Vec<MenuGroup>: Default` always holds - so the derive compiles.

- [ ] **Step 6: Add the compile-time dyn-compatibility guard.** Insert into `crates/ui/src/registry/mod.rs` directly after the `Registries` struct. Copy from the spec (lines 432-435); it references `Module` from `crate::contrib_api` (already a contract type):

```rust
// A compile-time guard: if any registry trait regresses out of dyn-compatibility
// (a generic method, a `-> Self`, a non-`&self` receiver), this stops compiling.
const _: () = {
    fn _assert_boxable(
        _: Box<dyn Panel>,
        _: Box<dyn Tool>,
        _: Box<dyn Workspace>,
        _: Box<dyn crate::contrib_api::Module>,
    ) {
    }
};
```

- [ ] **Step 7: Build to confirm the aliases, struct, and guard compile against the contract types.** Run in PowerShell:

```powershell
cargo build -p pixhaus-ui
```

Expected: PASS, assuming the IDS and TRAITS layers have landed their types. If it fails with `unresolved import` on a `contrib_api` type, that means a dependency layer has not landed yet - stop and confirm IDS/TRAITS are merged before continuing (this layer assumes them per the shared contract).

- [ ] **Step 8: Commit.** Run in PowerShell:

```powershell
git add crates/ui/src/registry/mod.rs crates/ui/src/lib.rs crates/ui/src/registry/resolve.rs
git commit -m @'
feat(ui): add Registry, Registries, and the dyn-compat guard

Registry is an insertion-ordered, key-indexed store; iteration order is
the rail/tab display order. A duplicate id debug_asserts (programming
error, last value wins in release). The const guard fails the build if a
registry trait drops dyn-compatibility.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### REG.5: The `HostRegistrar` impl over `&mut Registries`

**Files:**
- Modify: `crates/ui/src/registry/mod.rs`
- Test: `crates/ui/src/registry/mod.rs` (inline `#[cfg(test)]`)

Implements spec lines 422-424 ("A thin wrapper over `&mut Registries` implements `HostRegistrar`; insert keys come from each value's `id()`"). The registrar is the `dyn` boundary a `Module` registers through (spec lines 366-373) - it must key each value off the value's own `id()` so the module never names a key twice. Real logic (correct id extraction, routing to the right registry), so TDD applies. This task needs a test-only fake `Panel`/`Workspace` so it can register without the module crates; the brief mandates providing that fixture here, and REG.6 reuses it.

- [ ] **Step 1: Add the test-only fake fixtures (a fake `Panel` and a fake `Workspace`) used by this task and REG.6.** Append a `#[cfg(test)] mod fixtures` block to `crates/ui/src/registry/mod.rs`. These implement the TRAITS-layer traits with the minimum the resolve/registrar tests need; the field/method shapes come from the spec's `Panel` (lines 276-294), `Workspace` (lines 329-355), `PanelMeta`, `WorkspaceMeta`, and `WorkspaceLayout`:

```rust
#[cfg(test)]
pub(crate) mod fixtures {
    use crate::contrib_api::{
        Panel, PanelId, PanelMeta, PanelScope, Workspace, WorkspaceId, WorkspaceLayout,
        WorkspaceMeta,
    };
    use crate::region::Region;

    /// A minimal panel for registry/resolve tests: it carries an id and a relevance
    /// answer, renders nothing, and depends on no module crate.
    pub struct FakePanel {
        pub id: PanelId,
        pub relevant: bool,
    }

    impl Panel for FakePanel {
        fn id(&self) -> PanelId {
            self.id
        }
        fn meta(&self) -> PanelMeta {
            PanelMeta {
                title: "fake",
                icon: ' ',
                default_region: Region::RightDock,
                default_open: true,
            }
        }
        fn relevant_in(&self, _workspace: WorkspaceId) -> bool {
            self.relevant
        }
        fn ui(&self, _ui: &mut egui::Ui, _scope: &mut PanelScope<'_>) {}
    }

    /// A minimal workspace for resolve tests: it returns a fixed authored layout.
    pub struct FakeWorkspace {
        pub id: WorkspaceId,
        pub layout: WorkspaceLayout,
    }

    impl Workspace for FakeWorkspace {
        fn id(&self) -> WorkspaceId {
            self.id
        }
        fn meta(&self) -> WorkspaceMeta {
            WorkspaceMeta {
                name: "Fake",
                icon: ' ',
                purpose: "test workspace",
                shortcut: egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Num1),
            }
        }
        fn layout(&self) -> WorkspaceLayout {
            self.layout.clone()
        }
    }
}
```

- [ ] **Step 2: Write the failing test for the registrar wrapper.** Append to the `#[cfg(test)] mod tests` block in `crates/ui/src/registry/mod.rs`. It registers a fake panel and workspace through the `&mut dyn HostRegistrar` boundary and asserts the value landed in the right registry under its own `id()`:

```rust
    use super::fixtures::{FakePanel, FakeWorkspace};
    use crate::contrib_api::{HostRegistrar, PanelId, WorkspaceId, WorkspaceLayout};

    /// `Registries::registrar()` yields a `HostRegistrar`; adding a value keys it by
    /// the value's own `id()`, so the module never names a key twice.
    #[test]
    fn registrar_keys_by_value_id() {
        let mut registries = super::Registries::default();
        {
            let mut registrar = registries.registrar();
            registrar.add_panel(Box::new(FakePanel { id: PanelId("layers"), relevant: true }));
            registrar.add_workspace(Box::new(FakeWorkspace {
                id: WorkspaceId("draw"),
                layout: WorkspaceLayout {
                    right_dock: vec![PanelId("layers")],
                    bottom_tray: Vec::new(),
                    primary_tools: Vec::new(),
                    default_tool: crate::contrib_api::ToolId("pencil"),
                    status_items: Vec::new(),
                },
            }));
        }
        assert!(registries.panels.get(PanelId("layers")).is_some());
        assert!(registries.workspaces.get(WorkspaceId("draw")).is_some());
    }
```

- [ ] **Step 2b: Adjust the `tests` module imports.** The `mod tests` block now uses more than `super::Registry`. Replace the existing `use super::Registry;` line at the top of the `#[cfg(test)] mod tests` block with:

```rust
    use super::Registry;
```

(no change needed if it is already present - the new `use super::fixtures::...` and `use crate::contrib_api::...` lines from Step 2 sit alongside it).

- [ ] **Step 3: Run the test to confirm it fails (no `registrar()` method, no `HostRegistrar` impl yet).** Run in PowerShell:

```powershell
cargo nextest run -p pixhaus-ui registrar_keys_by_value_id --no-run
```

Expected: FAIL - compile error `no method named registrar found for struct Registries`. Red state confirmed.

- [ ] **Step 4: Implement the `registrar()` method and the `HostRegistrar` impl.** Insert into `crates/ui/src/registry/mod.rs` after the `Registries` struct (and after the `const _` guard). The wrapper borrows `&mut Registries` and routes each `add_*` to the matching registry, keying off the value's `id()`. Import `HostRegistrar`:

```rust
use crate::contrib_api::HostRegistrar;

impl Registries {
    /// Borrows these registries as the `dyn HostRegistrar` a module registers
    /// through. A module never sees the concrete `Registries`; it only adds
    /// capabilities, each keyed by its own `id()`.
    pub fn registrar(&mut self) -> RegistrarWrapper<'_> {
        RegistrarWrapper(self)
    }
}

/// A thin `&mut Registries` wrapper that implements [`HostRegistrar`].
///
/// Each `add_*` keys the value by its own `id()`, so a module cannot register a
/// capability under a key that disagrees with the capability's identity.
pub struct RegistrarWrapper<'a>(&'a mut Registries);

impl HostRegistrar for RegistrarWrapper<'_> {
    fn add_panel(&mut self, panel: Box<dyn Panel>) {
        let id = panel.id();
        self.0.panels.insert(id, panel);
    }
    fn add_tool(&mut self, tool: Box<dyn Tool>) {
        let id = tool.id();
        self.0.tools.insert(id, tool);
    }
    fn add_workspace(&mut self, ws: Box<dyn Workspace>) {
        let id = ws.id();
        self.0.workspaces.insert(id, ws);
    }
    fn add_action(&mut self, action: ActionDesc) {
        let id = action.id;
        self.0.actions.insert(id, action);
    }
    fn add_menu_group(&mut self, group: MenuGroup) {
        self.0.menus.push(group);
    }
}
```

A note on the `id` temporaries: `panel.id()` borrows `panel` immutably and returns a `Copy` id (`PanelId` is `Copy` per spec line 207), so binding `id` first then moving `panel` into `insert` avoids a borrow-after-move. `ActionDesc::id` is a public `Copy` field (spec line 376), read directly.

- [ ] **Step 5: Run the registrar test to confirm green.** Run in PowerShell:

```powershell
cargo nextest run -p pixhaus-ui registrar_keys_by_value_id
```

Expected: PASS - 1 test run, 0 failed.

- [ ] **Step 6: Commit.** Run in PowerShell:

```powershell
git add crates/ui/src/registry/mod.rs
git commit -m @'
feat(ui): implement HostRegistrar over &mut Registries

registrar() yields the dyn registrar a module registers through; each
add_* keys the value by its own id(), so a module cannot register under a
key that disagrees with the capability's identity. Adds test-only fake
Panel/Workspace fixtures the registrar and resolve tests share.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### REG.6: `ResolvedLayout` and `resolve_layout`

**Files:**
- Modify: `crates/ui/src/registry/resolve.rs`
- Test: `crates/ui/src/registry/resolve.rs` (inline `#[cfg(test)]`)

Implements spec lines 438-474. `resolve_layout` takes a `WorkspaceId` and `&Registries`, reads the workspace's authored `WorkspaceLayout`, and filters `right_dock` and `bottom_tray` to ids that are actually registered - `debug_assert`-ing `panel.relevant_in(ws)` for present panels and `tracing::warn!`-ing a missing id (spec lines 450-467). `ResolvedLayout::empty()` is the fallback for an unknown workspace. Real branching logic (filter, debug_assert, warn, empty fallback), so TDD with the shared fakes from REG.5.

- [ ] **Step 1: Write `ResolvedLayout`, `empty()`, and `resolve_layout` into `crates/ui/src/registry/resolve.rs`.** Replace the placeholder file contents with the full implementation. Copy `ResolvedLayout` (spec lines 441-448) and `resolve_layout` (lines 450-467) exactly; add the imports and item docs. Note `resolve_layout` is `pub` so the re-export in `mod.rs` resolves:

```rust
//! Layout resolution: turn a workspace's authored layout into a [`ResolvedLayout`]
//! filtered to actually-registered ids. See [`resolve_layout`].

use crate::contrib_api::{PanelId, StatusItem, ToolId, WorkspaceId};
use crate::registry::Registries;

/// A workspace layout with panel ids filtered down to the ones actually registered.
///
/// The shell renders straight from this: `right_dock` is the card stack
/// top-to-bottom, `bottom_tray` is the tray tabs left-to-right (first is the
/// default tab). Unregistered ids have already been dropped (with a warn) so the
/// shell never has to handle a dangling reference.
#[derive(Clone, PartialEq, Debug)]
pub struct ResolvedLayout {
    /// Right-dock card stack, top-to-bottom, filtered to registered ids.
    pub right_dock: Vec<PanelId>,
    /// Bottom-tray tabs, left-to-right, filtered to registered ids.
    pub bottom_tray: Vec<PanelId>,
    /// The tools shown in the left rail, in order.
    pub primary_tools: Vec<ToolId>,
    /// The tool selected when the workspace activates.
    pub default_tool: ToolId,
    /// Workspace-specific status-bar entries.
    pub status_items: Vec<StatusItem>,
}

impl ResolvedLayout {
    /// The fallback layout for an unknown workspace id: everything empty.
    ///
    /// `default_tool` is a sentinel `ToolId("")`; an unknown workspace renders no
    /// rail, so it is never read.
    pub fn empty() -> Self {
        Self {
            right_dock: Vec::new(),
            bottom_tray: Vec::new(),
            primary_tools: Vec::new(),
            default_tool: ToolId(""),
            status_items: Vec::new(),
        }
    }
}

/// Resolves a workspace's authored layout against the registries.
///
/// Returns [`ResolvedLayout::empty`] for an unknown workspace. Otherwise filters
/// the authored `right_dock` and `bottom_tray` to ids that are actually
/// registered: a present panel `debug_assert`s its `relevant_in` answer (a
/// workspace listing an irrelevant panel is an authoring bug), and an absent id
/// is dropped with a `tracing::warn!` rather than degrading silently.
pub fn resolve_layout(ws: WorkspaceId, r: &Registries) -> ResolvedLayout {
    let Some(workspace) = r.workspaces.get(ws) else {
        return ResolvedLayout::empty();
    };
    let layout = workspace.layout();
    let keep_panel = |id: &PanelId| match r.panels.get(*id) {
        Some(panel) => {
            debug_assert!(panel.relevant_in(ws), "workspace listed an irrelevant panel");
            true
        }
        None => {
            tracing::warn!(?id, "workspace references an unregistered panel; skipping");
            false
        }
    };
    ResolvedLayout {
        right_dock: layout.right_dock.iter().copied().filter(keep_panel).collect(),
        bottom_tray: layout.bottom_tray.iter().copied().filter(keep_panel).collect(),
        primary_tools: layout.primary_tools,
        default_tool: layout.default_tool,
        status_items: layout.status_items,
    }
}
```

- [ ] **Step 2: Build to confirm the module compiles and the `mod.rs` re-export now resolves.** Run in PowerShell:

```powershell
cargo build -p pixhaus-ui
```

Expected: PASS - the `pub use resolve::{resolve_layout, ResolvedLayout};` line from REG.3 now resolves, and the unused-import warning from REG.3 Step 5 is gone.

- [ ] **Step 3: Write the resolve unit test using the shared fakes.** Append a `#[cfg(test)]` block to `crates/ui/src/registry/resolve.rs`. It hand-builds two fake panels (one registered, one referenced-but-absent) and a fake workspace, then asserts the absent id is filtered out and an unknown workspace yields `empty()`. It reaches the fakes via the `pub(crate) fixtures` module from REG.5:

```rust
#[cfg(test)]
mod tests {
    use super::{resolve_layout, ResolvedLayout};
    use crate::contrib_api::{PanelId, ToolId, WorkspaceId, WorkspaceLayout};
    use crate::registry::fixtures::{FakePanel, FakeWorkspace};
    use crate::registry::Registries;

    /// A registered panel survives resolution; a referenced-but-unregistered panel
    /// is filtered out (and warns) rather than producing a dangling id.
    #[test]
    fn resolve_filters_unregistered_panels() {
        let mut registries = Registries::default();
        {
            let mut registrar = registries.registrar();
            registrar.add_panel(Box::new(FakePanel { id: PanelId("layers"), relevant: true }));
            // "ghost" is referenced by the layout below but never registered.
            registrar.add_workspace(Box::new(FakeWorkspace {
                id: WorkspaceId("draw"),
                layout: WorkspaceLayout {
                    right_dock: vec![PanelId("layers"), PanelId("ghost")],
                    bottom_tray: vec![PanelId("ghost")],
                    primary_tools: vec![ToolId("pencil")],
                    default_tool: ToolId("pencil"),
                    status_items: Vec::new(),
                },
            }));
        }

        let resolved = resolve_layout(WorkspaceId("draw"), &registries);
        assert_eq!(resolved.right_dock, vec![PanelId("layers")]);
        assert!(resolved.bottom_tray.is_empty(), "the only tray ref was unregistered");
        assert_eq!(resolved.primary_tools, vec![ToolId("pencil")]);
        assert_eq!(resolved.default_tool, ToolId("pencil"));
    }

    /// An unknown workspace id resolves to the empty layout, never a panic.
    #[test]
    fn resolve_unknown_workspace_is_empty() {
        let registries = Registries::default();
        let resolved = resolve_layout(WorkspaceId("nope"), &registries);
        assert_eq!(resolved, ResolvedLayout::empty());
    }
}
```

Note: the `add_panel`/`add_workspace`/`registrar` calls need `HostRegistrar` and `Registries` in scope. `registries.registrar()` returns the wrapper whose `add_*` come from the `HostRegistrar` trait, so add `use crate::contrib_api::HostRegistrar;` to this test module's imports if the build reports the trait methods are not in scope.

- [ ] **Step 4: Run the resolve tests.** Run in PowerShell:

```powershell
cargo nextest run -p pixhaus-ui resolve_filters_unregistered_panels resolve_unknown_workspace_is_empty
```

Expected: PASS - 2 tests run, 0 failed. (`resolve_filters_unregistered_panels` emits a `tracing::warn!` for the `ghost` id; with no subscriber installed in the test it is a silent no-op, which is fine - the test asserts the filtering, not the log.)

- [ ] **Step 5: Run the full ui-crate test suite and clippy to confirm the whole layer is green together.** Run in PowerShell:

```powershell
cargo nextest run -p pixhaus-ui; if ($?) { cargo test --doc -p pixhaus-ui }; if ($?) { cargo clippy -p pixhaus-ui --all-targets -- -D warnings }
```

Expected: all PASS - every REG test green, doc tests green (no runnable doc examples added in this layer, so the doc run reports 0 tests), clippy clean with `-D warnings`.

- [ ] **Step 6: Commit.** Run in PowerShell:

```powershell
git add crates/ui/src/registry/resolve.rs
git commit -m @'
feat(ui): add ResolvedLayout and resolve_layout

resolve_layout filters a workspace's authored right_dock and bottom_tray
to registered panel ids: a present panel debug_asserts its relevance, an
absent id is dropped with a warn rather than degrading silently. An
unknown workspace resolves to the empty layout. Tests cover both the
filter and the empty-fallback paths with the shared fake fixtures.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### REG.7: Layer handoff note for the TESTS layer (spec test 2)

**Files:**
- None (this is a handoff note, no code).

Spec test 2 (lines 1050-1053) - the `insta` snapshot of `resolve_layout` for all five workspaces - is intentionally **not** implemented in this layer. It requires every `modules/*` crate registered into a `Registries`, which this layer cannot do without depending on the module crates (that would invert the dependency direction: `ui` must not depend on `modules/*`, per `crates/CLAUDE.md`). The mechanism it snapshots (`resolve_layout` + `ResolvedLayout: Debug`) is fully landed and unit-tested here against hand-built fakes (REG.6). The TESTS layer owns the five-module snapshot as an integration test in `crates/ui/tests/` (which *may* depend on the module crates as dev-dependencies) or in `app/`. Both `resolve_layout` and `ResolvedLayout` are `pub` and re-exported from `pixhaus_ui::registry`, and `WorkspaceLayout`/`StatusItem` derive `Debug` (spec lines 344, 353, 441) so the snapshot renders deterministically.

- No steps. This block documents the deliberate scope boundary so test 2's absence here is auditable, not an omission.

## Phase 5: State and intents

I have everything I need. I've confirmed:
- The `state/` module doesn't exist yet; `lib.rs` has no `pub mod` declarations beyond the canvas seam, so my layer adds `pub mod state;`.
- `egui::epaint::Shadow { offset: [i8; 2], blur: u8, spread: u8, color: Color32 }`.
- `ctx.wants_keyboard_input() -> bool` is the focus-query helper.
- Headless `egui::Context::default()` is usable for the theme-path test (the spec's test 3 uses one where `SetThemeVariant` calls `apply_to_visuals(.., ctx)`).

I now have the exact contract names from the spec for cross-layer types (`Theme`, `ThemeVariant`, `apply_to_visuals`, `Registries`, `WorkspaceId`, `ToolId`, `PanelId`, `ActionId`, `ContribCtx`, `ActionDesc`, `MenuGroup`, `GridMode`). My layer owns `state/` and depends on those by name. Let me write the plan.

### STATE.0: Layer preface and dependency contract

This layer implements the spec's "State and event model" section (spec lines 477-655): `crates/ui/src/state/{session.rs, ui_state.rs, intent.rs, mod.rs}` - `SessionState`/`JobStub`/`AiStatus`, `UiState`/`GridMode`/`Modal`, `ShellState`, `Host`/`BackgroundChannel`, `Intent`/`Event`/`IntentSink`, and `apply_intent`. Spec test 3 (intent application to state, spec lines 1055-1058) lands here.

**Contract types this layer consumes by name from other layers (assume they exist - they are the shared contract, copied exactly from the spec):**
- From the THEME layer (`crate::theme`): `Theme`, `ThemeVariant` (enum `Dark | Light | AccentHighContrast`, derives `Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize`), `Theme::dark()`, `Theme::for_variant(ThemeVariant, egui::Color32) -> Theme`, `Theme::accent_seed(&self) -> egui::Color32`, and the free fn `crate::theme::apply_to_visuals(theme: &Theme, ctx: &egui::Context)`.
- From the CONTRIB-API layer (`crate::contrib_api`): the id newtypes `PanelId`, `ToolId`, `WorkspaceId`, `ActionId` (each `pub struct _(pub &'static str)`, derives `Copy, Clone, PartialEq, Eq, Hash, Debug`); `ActionDesc`; `MenuGroup`; and the `HostRegistrar` trait.
- From the REGISTRY layer (`crate::registry`): `Registries` and the thin `&mut Registries` wrapper that implements `HostRegistrar` (this layer's `Host::registrar()` returns it).

**Cross-layer detail this layer must NOT define:** `GridMode` is owned by the UI-STATE files in *this* layer (the spec places `GridMode` in `state/ui_state.rs`, lines 504, 769-is-theme - note `GridMode` appears in `Prefs` and `UiState`; it is a `state` type). `Modal` is also this layer's. We define both here.

**Machine rule:** every cargo command below is written for PowerShell (the Bash tool's `link.exe` shadows the MSVC linker). The branch `feat/ui-shell-foundation` already exists - do not branch. The post-edit hook auto-formats and runs `clippy --tests -D warnings` on `pixhaus-ui` after each Edit/Write; the explicit run/clippy steps below are still required as gates.

---

### STATE.1: Crate-root test allowance and the `state` module declaration

**Files:**
- Modify: `crates/ui/src/lib.rs`

- [ ] **Step 1: Add the test-only clippy allowance at the crate root.** The spec (lines 1042-1043) requires `#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]` so tests may use `unwrap`/`expect` without tripping the workspace `-D warnings` gate. Add it as the first line of `crates/ui/src/lib.rs`, above the existing `//!` doc comment.

Edit `crates/ui/src/lib.rs`, replacing the opening line:

```rust
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! Pixhaus UI layer: the egui contribution surface and the canvas embedding.
```

(Keep the rest of the doc comment block unchanged.)

- [ ] **Step 2: Declare the `state` module.** Other layers (THEME, CONTRIB-API, REGISTRY) add their own `pub mod` lines; this layer adds only `pub mod state;`. Insert it after the existing `use pixhaus_render::ViewportRenderer;` import block, before the `install_canvas_renderer` doc comment:

```rust
use pixhaus_render::ViewportRenderer;

/// Session, UI, and intent state plus the [`state::Host`] that owns it.
pub mod state;
```

If THEME/CONTRIB-API/REGISTRY layers have already added their `pub mod theme;` / `pub mod contrib_api;` / `pub mod registry;` lines, leave them; just add `pub mod state;` alongside. Do not reorder existing declarations.

- [ ] **Step 3: Build to confirm the module declaration resolves once files exist.** This will FAIL now because `state/mod.rs` does not exist yet - that is expected; the next task creates the files. Run it anyway to see the precise "file not found for module `state`" error so you know the wiring is correct:

```powershell
cargo build -p pixhaus-ui
```

Expected: FAIL with `error[E0583]: file not found for module 'state'`. Proceed to STATE.2; do not commit yet (a missing-module build error is not a commit point).

---

### STATE.2: `SessionState`, `JobStub`, `AiStatus`

**Files:**
- Create: `crates/ui/src/state/session.rs`
- Create: `crates/ui/src/state/mod.rs` (minimal stub here so the crate compiles; fleshed out in STATE.5)
- Test: inline `#[cfg(test)] mod tests` in `crates/ui/src/state/session.rs`

- [ ] **Step 1: Create a minimal `state/mod.rs` that declares the submodules.** Full `Host`/`ShellState`/`BackgroundChannel` land in STATE.5; for now declare the modules so each can compile and test independently. Write `crates/ui/src/state/mod.rs`:

```rust
//! Session, UI, and intent state, and the [`Host`] that owns all three.
//!
//! Ownership map (spec "Owners, no overlap"): durable project state will live in
//! `core` (absent this round); session and UI state are plain structs owned by
//! [`Host`], never egui `Memory`. egui `Memory` holds only widget internals keyed
//! by `Id`.

pub mod intent;
pub mod session;
pub mod ui_state;
```

(STATE.5 replaces this file's body to add `Host`, `ShellState`, `BackgroundChannel`. The submodule declarations stay.)

- [ ] **Step 2: Write `session.rs` with the failing test first (TDD).** The unit under test is `JobStub::queued` and the `AiStatus` default. Create `crates/ui/src/state/session.rs` with the types and an inline test. Copy `SessionState` fields exactly from spec lines 494-502; `JobStub`/`AiStatus` are specified by behavior (the status dot and console content, spec lines 499-500, 906-907). `JobStub::queued(action)` takes an `ActionId` (the `RunAction` arm calls `JobStub::queued(a)`, spec line 609).

```rust
//! Session state: the per-session, non-durable model the shell owns.
//!
//! Minimal this round. `active_document` / `selection` / `undo_stack` are reserved
//! seams that arrive with `core` (spec "Owners, no overlap"); they are intentionally
//! absent, not forgotten.

use crate::contrib_api::ids::{ActionId, ToolId, WorkspaceId};

/// Non-durable session state owned by [`crate::state::Host`].
///
/// Reserved for `core`: `active_document`, `selection`, `undo_stack`. They join when
/// `core` has types; until then they would be fake state, so they are left out.
pub struct SessionState {
    /// The workspace currently shown (Draw, Animate, Tiles, Generate, Export).
    pub active_workspace: WorkspaceId,
    /// The tool currently selected in the active workspace's rail.
    pub active_tool: ToolId,
    /// Whether the (future) document has unsaved edits. Mock this round.
    pub dirty: bool,
    /// Mock job entries so the status dot and the console panel have content.
    pub jobs: Vec<JobStub>,
    /// Drives the status-bar AI dot.
    pub ai_status: AiStatus,
}

/// A stand-in for a real job (spec: bible rule 5). Carries only what the mock
/// status dot and console need; the real job system lands in `services`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobStub {
    /// The action that queued this job.
    pub action: ActionId,
    /// Where the job is in its (mock) lifecycle.
    pub state: JobState,
}

/// Mock lifecycle for a [`JobStub`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum JobState {
    /// Queued, not yet running.
    Queued,
    /// Finished (mock).
    Done,
}

impl JobStub {
    /// A freshly queued job for `action`.
    pub fn queued(action: ActionId) -> Self {
        Self { action, state: JobState::Queued }
    }
}

/// The AI runtime status surfaced by the status-bar dot (spec UX 27).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AiStatus {
    /// Idle and available (success-colored dot).
    Ready,
    /// A job is running (warning-colored dot).
    Working,
    /// No backend (disabled-colored dot).
    Offline,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_job_carries_its_action_and_is_queued() {
        let action = ActionId("ai.fill");
        let job = JobStub::queued(action);
        assert_eq!(job.action, action, "queued() must record the action it was given");
        assert_eq!(job.state, JobState::Queued, "a fresh job starts Queued");
    }
}
```

- [ ] **Step 3: Run the test - it FAILS to compile until `contrib_api::ids` exists.** This layer depends on the CONTRIB-API layer's id newtypes (the shared contract). If CONTRIB-API has not landed yet, the `use crate::contrib_api::ids::...` line fails. Run:

```powershell
cargo nextest run -p pixhaus-ui session
```

Expected, two cases:
- If CONTRIB-API has landed: PASS (`session::tests::queued_job_carries_its_action_and_is_queued`).
- If CONTRIB-API has not landed: FAIL with `error[E0432]: unresolved import 'crate::contrib_api'`. That is a layer-ordering dependency, not a defect in this code - STATE depends on CONTRIB-API's ids by the shared contract. Do not work around it by redefining the ids; wait for CONTRIB-API or run after it. Once it resolves, the test PASSes.

- [ ] **Step 4: Run clippy on the crate to confirm the gate is clean.**

```powershell
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```

Expected: PASS (no warnings). If CONTRIB-API is not yet present, this also reports the unresolved import; that clears with the dependency.

- [ ] **Step 5: Commit.** Stage the three files and commit with the exact trailer.

```powershell
git add crates/ui/src/lib.rs crates/ui/src/state/mod.rs crates/ui/src/state/session.rs
git commit -m @'
feat(ui): add session state with job stub and AI status

SessionState carries active workspace/tool, a dirty flag, mock jobs, and
the AI status that drives the status-bar dot. active_document/selection/
undo_stack are reserved seams for core. JobStub::queued backs the RunAction
mock side effect.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
</parameter>
```

(Note: the `'@` must sit at column 0 with no leading whitespace, per the PowerShell here-string rule.)

---

### STATE.3: `UiState`, `GridMode`, `Modal`

**Files:**
- Create: `crates/ui/src/state/ui_state.rs`
- Test: inline `#[cfg(test)] mod tests` in `crates/ui/src/state/ui_state.rs`

- [ ] **Step 1: Write `ui_state.rs` with a failing test first (TDD).** The units with logic are `UiState::default` (sensible initial widths/zoom) and the `GridMode` default. Copy `UiState` fields exactly from spec lines 504-517. `GridMode` is referenced by `SetGrid(GridMode)` (spec 558), the status bar's grid readout, the canvas 8/16px grid (spec 894), and `Prefs` (spec 540) - model it as a small `Copy` enum. `Modal` is `CommandPalette | Confirm` (spec 515, "modal == CommandPalette" 649, 1009). `GridMode` derives `serde` because `Prefs` (a separate plain-types struct in the HOST layer) embeds it; keep it plain (no `egui` types) for that reason.

```rust
//! UI state: layout, view, and modal flags the shell owns directly.
//!
//! This is our own plain struct, never egui `Memory`. Panel collapse lives here
//! (not in `CollapsingHeader`'s own memory) because the command palette and future
//! layout presets must read and set it (spec "Owners, no overlap"). Scroll offsets
//! and focus are NOT duplicated here - egui owns those.

use std::collections::HashMap;

use crate::contrib_api::ids::{PanelId, WorkspaceId};

/// Mutable, non-durable UI state owned by [`crate::state::Host`].
pub struct UiState {
    /// Right-dock width in points (resizable by the user).
    pub right_dock_width: f32,
    /// Bottom-tray height in points (resizable by the user).
    pub bottom_tray_height: f32,
    /// Per-panel collapse flag. Absent key means "use the panel's default_open".
    pub collapsed: HashMap<PanelId, bool>,
    /// Selected tray tab per workspace. Absent key means "the first tab".
    pub tray_tab: HashMap<WorkspaceId, PanelId>,
    /// Canvas zoom factor (mock; 1.0 == 100%).
    pub zoom: f32,
    /// Canvas pan offset in points.
    pub pan: egui::Vec2,
    /// Active grid spacing mode.
    pub grid: GridMode,
    /// Onion-skin toggle (Animate).
    pub onion_skin: bool,
    /// Pixel-snap toggle.
    pub snap: bool,
    /// The open modal overlay, if any.
    pub modal: Option<Modal>,
    /// Live text in the command-palette search field.
    pub palette_query: String,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            right_dock_width: 280.0,
            bottom_tray_height: 200.0,
            collapsed: HashMap::new(),
            tray_tab: HashMap::new(),
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            grid: GridMode::default(),
            onion_skin: false,
            snap: true,
            modal: None,
            palette_query: String::new(),
        }
    }
}

/// Canvas grid spacing. Plain data (no egui types) so [`crate::state::Prefs`] can
/// serialize it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GridMode {
    /// No grid drawn.
    Off,
    /// 8px minor grid.
    Px8,
    /// 16px major grid.
    Px16,
}

impl Default for GridMode {
    fn default() -> Self {
        GridMode::Px8
    }
}

/// A modal overlay covering the shell.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Modal {
    /// The Ctrl/Cmd+K command palette.
    CommandPalette,
    /// A yes/no confirmation prompt.
    Confirm,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ui_state_has_no_modal_and_unit_zoom() {
        let ui = UiState::default();
        assert!(ui.modal.is_none(), "nothing is modal on a fresh session");
        assert_eq!(ui.zoom, 1.0, "default zoom is 100%");
        assert!(ui.collapsed.is_empty(), "no panel overrides by default");
        assert!(ui.tray_tab.is_empty(), "no tray-tab overrides by default");
    }

    #[test]
    fn default_grid_mode_is_eight_px() {
        assert_eq!(GridMode::default(), GridMode::Px8, "default grid is the 8px minor grid");
    }
}
```

- [ ] **Step 2: Run the tests.** They depend on CONTRIB-API's `PanelId`/`WorkspaceId`.

```powershell
cargo nextest run -p pixhaus-ui ui_state
```

Expected: PASS for `ui_state::tests::default_ui_state_has_no_modal_and_unit_zoom` and `ui_state::tests::default_grid_mode_is_eight_px` (once CONTRIB-API is present; otherwise the same unresolved-import layer dependency as STATE.2 step 3).

- [ ] **Step 3: Run clippy.**

```powershell
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```

Expected: PASS. (Clippy's `pedantic` may suggest `#[derive(Default)]` over a manual impl for `GridMode`; keep the manual `Default` because the default variant is `Px8`, not the first declared variant - a derived `Default` would need `#[default]` on `Px8`. If clippy flags the manual impl, switch to `#[derive(Default)]` on the enum plus `#[default]` on the `Px8` variant, which is equivalent and idiomatic; either is acceptable.)

- [ ] **Step 4: Commit.**

```powershell
git add crates/ui/src/state/ui_state.rs
git commit -m @'
feat(ui): add UiState with grid and modal flags

UiState owns dock/tray sizes, the per-panel collapse map, the per-workspace
tray-tab map, view (zoom/pan/grid), onion-skin/snap toggles, the modal slot,
and the palette query. GridMode is plain serde data so Prefs can persist it;
Modal is CommandPalette or Confirm.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
</parameter>
```

---

### STATE.4: `Intent`, `Event`, `IntentSink`

**Files:**
- Create: `crates/ui/src/state/intent.rs` (types only this task; `apply_intent` lands in STATE.6 once `Host` exists)
- Test: inline `#[cfg(test)] mod tests` in `crates/ui/src/state/intent.rs`

- [ ] **Step 1: Write the `Intent`/`Event`/`IntentSink` types with a failing test first (TDD).** The unit with logic is `IntentSink::push` (and its `Default`). Copy `Intent` exactly from spec lines 552-567 - including the `SelectTrayTab(PanelId)` variant and the reserved `// Command(Box<dyn core::Command>)` seam comment. Copy `Event` from 569-573 and `IntentSink` from 575-577. `Intent` and `Event` reference `GridMode` (this layer), `ThemeVariant` (THEME layer), and the id newtypes (CONTRIB-API layer). The `apply_intent` function is NOT in this task - it needs `Host`, which STATE.5 defines; it is added in STATE.6. Add a module-level doc note saying so, so the next task knows where it goes.

```rust
//! Intents and events: the one write channel and the post-frame notification bus.
//!
//! An [`Intent`] is a requested change; a contributor pushes intents into an
//! [`IntentSink`] and the shell applies them after the frame's region borrows drop
//! (`apply_intent`, defined alongside [`crate::state::Host`]). An [`Event`] is
//! "something happened", produced only inside `apply_intent` and consumed on the
//! next frame - never read by panels during render, so there is no intra-frame event
//! bus and the borrow guarantee has no hole (spec bible 21.1).

use crate::contrib_api::ids::{ActionId, PanelId, ToolId, WorkspaceId};
use crate::state::ui_state::GridMode;
use crate::theme::ThemeVariant;

/// A requested change to session or UI state. The single write channel for
/// everything except a panel's own scratch text. Applied post-frame.
pub enum Intent {
    /// Switch the active workspace.
    SelectWorkspace(WorkspaceId),
    /// Select a tool in the active workspace's rail.
    SelectTool(ToolId),
    /// Select a tray tab; applies to the active workspace's tray.
    SelectTrayTab(PanelId),
    /// Toggle a panel's collapse flag.
    TogglePanelCollapsed(PanelId),
    /// Set the canvas grid mode.
    SetGrid(GridMode),
    /// Toggle onion skin (Animate).
    ToggleOnionSkin,
    /// Toggle pixel snap.
    ToggleSnap,
    /// Set canvas zoom.
    SetZoom(f32),
    /// Open the command palette modal.
    OpenCommandPalette,
    /// Dismiss any open modal.
    CloseModal,
    /// Change the theme variant; `apply_intent` re-applies it to egui's visuals.
    SetThemeVariant(ThemeVariant),
    /// Run an action. Mock: pushes a JobStub and emits an Event. Never mutates the
    /// model (spec invariant) - when `core` lands, model edits route through the
    /// reserved `Command` variant below instead.
    RunAction(ActionId),
    // Reserved, lands with core - the named command-path seam (bible rules 3, 4, 13):
    // Command(Box<dyn core::Command>),
}

/// "Something happened", distinct from a command (spec bible 21.3). Produced only
/// inside `apply_intent`, consumed on the next frame. This round it is a
/// `tracing::debug!` sink.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The active workspace changed.
    WorkspaceChanged(WorkspaceId),
    /// The active tool changed.
    ToolChanged(ToolId),
    /// An action was dispatched.
    ActionDispatched(ActionId),
}

/// The write channel a contributor pushes [`Intent`]s into during a frame.
#[derive(Default)]
pub struct IntentSink(pub(crate) Vec<Intent>);

impl IntentSink {
    /// Queue an intent for post-frame application.
    pub fn push(&mut self, i: Intent) {
        self.0.push(i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contrib_api::ids::WorkspaceId;

    #[test]
    fn push_appends_intents_in_order() {
        let mut sink = IntentSink::default();
        sink.push(Intent::SelectWorkspace(WorkspaceId("draw")));
        sink.push(Intent::OpenCommandPalette);
        assert_eq!(sink.0.len(), 2, "both intents are queued");
        assert!(
            matches!(sink.0[0], Intent::SelectWorkspace(WorkspaceId("draw"))),
            "first pushed intent stays first",
        );
        assert!(
            matches!(sink.0[1], Intent::OpenCommandPalette),
            "second pushed intent stays second",
        );
    }
}
```

Note on visibility: `IntentSink.0` is `pub(crate)` because the shell runtime (a different module in the SHELL layer, same crate) drains it via `std::mem::take(&mut host.intents.0)` (spec line 651). The spec sketch writes `IntentSink(Vec<Intent>)` with a private field, but the drain happens cross-module within the crate, so `pub(crate)` is the correct visibility; the test above (same crate) reads `.0` for the same reason. Do not make it fully `pub`.

- [ ] **Step 2: Run the test.** Depends on CONTRIB-API (ids) and THEME (`ThemeVariant`).

```powershell
cargo nextest run -p pixhaus-ui intent
```

Expected: PASS for `intent::tests::push_appends_intents_in_order` once THEME and CONTRIB-API are present; otherwise the unresolved-import layer dependency clears when they land.

- [ ] **Step 3: Run clippy.**

```powershell
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 4: Commit.**

```powershell
git add crates/ui/src/state/intent.rs
git commit -m @'
feat(ui): add Intent, Event, and IntentSink

Intent is the one write channel for non-scratch state changes, including
SelectTrayTab and the reserved Command seam comment. Event is the post-frame
notification type. IntentSink queues intents for post-frame application;
apply_intent lands with Host.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
</parameter>
```

---

### STATE.5: `Host`, `ShellState`, `BackgroundChannel`, `Prefs`, and the default initial state

**Files:**
- Modify: `crates/ui/src/state/mod.rs` (replace the STATE.2 stub body with the full `Host` definition; keep the submodule declarations)
- Test: inline `#[cfg(test)] mod tests` in `crates/ui/src/state/mod.rs`

- [ ] **Step 1: Write the `Host` aggregate with a failing test first (TDD).** Copy `Host` fields exactly from spec lines 519-528 and `Prefs` from 533-541. Add `ShellState { session, ui }` (spec line 521 names the field type `ShellState`) and `BackgroundChannel` (spec line 527, "mpsc receiver drained in logic(); empty this round" - model it as a `std::sync::mpsc::Receiver` of a background message enum, plus the matching `Sender` kept alive so the receiver never disconnects). `Host::new(theme)` builds the default initial state: `active_workspace = WorkspaceId("draw")`, `active_tool` = the Draw default tool. The spec's Draw workspace default tool is Pencil (spec line 943 lists Draw's tools; spec line 917 names Pencil's id role; the default tool is the first manual tool, Pencil). Pencil's `ToolId` is `ToolId("pencil")` - the modules layer registers tools by id, and per the spec's id-by-string contract the Draw default tool id is `"pencil"`. `Host::registrar()` returns the `&mut Registries` wrapper that implements `HostRegistrar` (REGISTRY layer provides the wrapper type; the spec sketch at line 159 calls `host.registrar()` and the registry mod note at line 422-423 says "A thin wrapper over &mut Registries implements HostRegistrar"). `Host::theme()` returns `&Theme` (spec line 161 calls `host.theme()`).

Replace the entire body of `crates/ui/src/state/mod.rs`:

```rust
//! Session, UI, and intent state, and the [`Host`] that owns all three.
//!
//! Ownership map (spec "Owners, no overlap"): durable project state will live in
//! `core` (absent this round); session and UI state are plain structs owned by
//! [`Host`], never egui `Memory`. egui `Memory` holds only widget internals keyed
//! by `Id`.

use std::collections::HashMap;
use std::sync::mpsc;

use crate::contrib_api::ids::{PanelId, ToolId, WorkspaceId};
use crate::registry::{Registries, RegistrarWrapper};
use crate::theme::{Theme, ThemeVariant};

use self::intent::IntentSink;
use self::session::{AiStatus, SessionState};
use self::ui_state::{GridMode, UiState};

pub mod intent;
pub mod session;
pub mod ui_state;

/// A message a background task hands back to the egui loop. Empty surface this round
/// (no senders beyond the bootstrap one); the variants grow as `services` lands.
#[derive(Debug)]
pub enum BackgroundMsg {
    /// A (mock) job changed AI status. Proves the drain path (spec bible rule 5).
    AiStatusChanged(AiStatus),
}

/// The receiver end the egui loop drains in `App::logic`, plus the sender it keeps
/// alive so the channel never disconnects while idle.
pub struct BackgroundChannel {
    /// Drained once per frame in `shell::drain_background`.
    pub rx: mpsc::Receiver<BackgroundMsg>,
    /// Held so `rx` stays connected; handed to background tasks when `services` lands.
    pub tx: mpsc::Sender<BackgroundMsg>,
}

impl Default for BackgroundChannel {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { rx, tx }
    }
}

/// Session + UI state grouped under one owner. `apply_intent` mutates through this.
pub struct ShellState {
    /// Non-durable session model.
    pub session: SessionState,
    /// Layout/view/modal state.
    pub ui: UiState,
}

/// The single owner of every piece of shell-level mutable state.
///
/// `Theme` lives here (not in the eframe `App`) so `apply_intent` can re-apply it on
/// a variant change (spec "Theme owner placement" risk). `scratch` is the one
/// per-panel mutable carve-out `TextEdit` requires.
pub struct Host {
    /// All registered capabilities (panels, tools, workspaces, actions, menus).
    pub registries: Registries,
    /// Session + UI state.
    pub state: ShellState,
    /// The write channel drained after each frame.
    pub intents: IntentSink,
    /// Panel-private draft text, keyed by panel id; mutable per-panel.
    pub scratch: HashMap<PanelId, String>,
    /// The active theme; owned here so a variant change can re-apply to visuals.
    pub theme: Theme,
    /// Background results drained in `App::logic`. Empty this round.
    pub bg: BackgroundChannel,
}

/// The default initial workspace (Draw) and tool (Pencil). The strings are the
/// ids the modules register by; see the per-workspace placement table.
const DEFAULT_WORKSPACE: WorkspaceId = WorkspaceId("draw");
const DEFAULT_TOOL: ToolId = ToolId("pencil");

impl Host {
    /// Build a host with empty registries and the default initial state.
    ///
    /// Registration happens afterward through [`Host::registrar`]: each module's
    /// `register` is the only path a capability enters the shell.
    pub fn new(theme: Theme) -> Self {
        Self {
            registries: Registries::default(),
            state: ShellState {
                session: SessionState {
                    active_workspace: DEFAULT_WORKSPACE,
                    active_tool: DEFAULT_TOOL,
                    dirty: false,
                    jobs: Vec::new(),
                    ai_status: AiStatus::Ready,
                },
                ui: UiState::default(),
            },
            intents: IntentSink::default(),
            scratch: HashMap::new(),
            theme,
            bg: BackgroundChannel::default(),
        }
    }

    /// The registrar a module registers capabilities through. Borrows the registries
    /// mutably for the duration of registration.
    pub fn registrar(&mut self) -> RegistrarWrapper<'_> {
        self.registries.registrar()
    }

    /// The active theme.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_host_starts_in_draw_with_pencil() {
        let host = Host::new(Theme::dark());
        assert_eq!(
            host.state.session.active_workspace,
            WorkspaceId("draw"),
            "the default workspace is Draw",
        );
        assert_eq!(
            host.state.session.active_tool,
            ToolId("pencil"),
            "Draw's default tool is Pencil",
        );
    }

    #[test]
    fn new_host_has_no_jobs_and_ready_ai() {
        let host = Host::new(Theme::dark());
        assert!(host.state.session.jobs.is_empty(), "no jobs queued at boot");
        assert_eq!(host.state.session.ai_status, AiStatus::Ready, "AI starts Ready");
    }

    #[test]
    fn new_host_theme_variant_matches_argument() {
        let host = Host::new(Theme::dark());
        assert_eq!(
            host.theme().variant,
            ThemeVariant::Dark,
            "the host holds the theme it was built with",
        );
    }
}
```

- [ ] **Step 2: Add the `Prefs` struct (reserved, serde-ready, plain types only).** Spec lines 530-546: durable prefs are a separate plain-types struct, not the live `UiState`. Persistence wiring is deferred (spec open decision 5), so this is a reserved, `serde`-deriving struct with no egui types. Append it to `crates/ui/src/state/mod.rs` after the `Host` impl (before the `#[cfg(test)]` block):

```rust
/// Durable preferences, round-tripped via eframe persistence (wiring deferred this
/// round, spec open decision 5). Plain types only - no `egui::Vec2`/`Color32` - so
/// `serde` derives cleanly and the format stays toolkit-independent.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Prefs {
    /// The `WorkspaceId`'s `&'static str` to open on launch.
    pub default_workspace: String,
    /// The theme variant.
    pub variant: ThemeVariant,
    /// The accent seed as RGB bytes.
    pub accent: [u8; 3],
    /// Right-dock width in points.
    pub dock_width: f32,
    /// Bottom-tray height in points.
    pub tray_height: f32,
    /// The grid mode.
    pub grid: GridMode,
}
```

- [ ] **Step 3: Run the tests.** Depends on REGISTRY (`Registries`, `RegistrarWrapper`), THEME (`Theme`, `ThemeVariant`, `Theme::dark`), CONTRIB-API (ids).

```powershell
cargo nextest run -p pixhaus-ui state::tests
```

Expected: PASS for `state::tests::new_host_starts_in_draw_with_pencil`, `state::tests::new_host_has_no_jobs_and_ready_ai`, `state::tests::new_host_theme_variant_matches_argument` - once REGISTRY/THEME/CONTRIB-API are present. If `RegistrarWrapper` is named differently in the REGISTRY layer, use that exact name (the contract is "a thin `&mut Registries` wrapper implementing `HostRegistrar`"); confirm the type name against the REGISTRY layer's `registry/mod.rs` before depending on it, and adjust the `use` and `registrar()` body to match. Do not invent a second wrapper.

- [ ] **Step 4: Run clippy.**

```powershell
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```

Expected: PASS. (`Prefs` has no constructor or use site yet; `dead_code` does not fire on `pub` items, and `missing_docs` is satisfied by the field docs above.)

- [ ] **Step 5: Commit.**

```powershell
git add crates/ui/src/state/mod.rs
git commit -m @'
feat(ui): add Host, ShellState, BackgroundChannel, and Prefs

Host is the single owner of registries, session/UI state, the intent sink,
per-panel scratch, the theme, and the background channel. Host::new seeds
the default Draw workspace and Pencil tool; registrar() is the only path
capabilities enter the shell. Prefs is the reserved serde-ready durable-prefs
struct (persistence wiring deferred).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
</parameter>
```

---

### STATE.6: `apply_intent` and spec test 3

**Files:**
- Modify: `crates/ui/src/state/intent.rs` (add `apply_intent`; the types from STATE.4 stay)
- Test: inline `#[cfg(test)] mod tests` in `crates/ui/src/state/intent.rs` (extend with spec test 3)

- [ ] **Step 1: Write spec test 3 first (TDD), as failing tests.** Spec test 3 (lines 1055-1058) drives `apply_intent` directly: `SelectWorkspace`/`SelectTool` flip the session; `SelectTrayTab` updates the per-workspace tab; `TogglePanelCollapsed` flips the `UiState` map; `SetThemeVariant` swaps the variant (using a headless `egui::Context` because the theme path calls `apply_to_visuals`); `OpenCommandPalette` sets the modal. Use `rstest` per the spec's "(`rstest`)" tag where a case table fits, plain `#[test]` where a single behavior is clearer. A headless `egui::Context::default()` is the verified way to get a `Context` with no event loop or GPU (confirmed: `apply_to_visuals` only calls `ctx.style_mut`).

Add these tests to the existing `#[cfg(test)] mod tests` block in `crates/ui/src/state/intent.rs`, alongside `push_appends_intents_in_order`. Replace the test module's `use` lines and add the new cases:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::contrib_api::ids::{ActionId, PanelId, ToolId, WorkspaceId};
    use crate::state::Host;
    use crate::state::session::JobState;
    use crate::state::ui_state::{GridMode, Modal};
    use crate::theme::{Theme, ThemeVariant};

    fn host() -> Host {
        Host::new(Theme::dark())
    }

    fn ctx() -> egui::Context {
        // A headless Context: no event loop, no GPU. apply_intent's theme path only
        // touches ctx.style_mut, which a default Context fully supports.
        egui::Context::default()
    }

    #[test]
    fn push_appends_intents_in_order() {
        let mut sink = IntentSink::default();
        sink.push(Intent::SelectWorkspace(WorkspaceId("draw")));
        sink.push(Intent::OpenCommandPalette);
        assert_eq!(sink.0.len(), 2, "both intents are queued");
        assert!(
            matches!(sink.0[0], Intent::SelectWorkspace(WorkspaceId("draw"))),
            "first pushed intent stays first",
        );
        assert!(
            matches!(sink.0[1], Intent::OpenCommandPalette),
            "second pushed intent stays second",
        );
    }

    #[test]
    fn select_workspace_flips_active_workspace() {
        let mut host = host();
        apply_intent(&mut host, Intent::SelectWorkspace(WorkspaceId("animate")), &ctx());
        assert_eq!(host.state.session.active_workspace, WorkspaceId("animate"));
    }

    #[test]
    fn select_tool_flips_active_tool() {
        let mut host = host();
        apply_intent(&mut host, Intent::SelectTool(ToolId("eraser")), &ctx());
        assert_eq!(host.state.session.active_tool, ToolId("eraser"));
    }

    #[test]
    fn select_tray_tab_updates_the_active_workspaces_tab() {
        let mut host = host();
        // Default workspace is Draw; the tab should be recorded under "draw".
        apply_intent(&mut host, Intent::SelectTrayTab(PanelId("assets")), &ctx());
        assert_eq!(
            host.state.ui.tray_tab.get(&WorkspaceId("draw")).copied(),
            Some(PanelId("assets")),
            "the tray tab is recorded for the active workspace only",
        );
    }

    #[test]
    fn toggle_panel_collapsed_flips_then_flips_back() {
        let mut host = host();
        let p = PanelId("layers");
        apply_intent(&mut host, Intent::TogglePanelCollapsed(p), &ctx());
        assert_eq!(host.state.ui.collapsed.get(&p).copied(), Some(true), "first toggle collapses");
        apply_intent(&mut host, Intent::TogglePanelCollapsed(p), &ctx());
        assert_eq!(host.state.ui.collapsed.get(&p).copied(), Some(false), "second toggle expands");
    }

    #[test]
    fn set_theme_variant_swaps_the_variant() {
        let mut host = host();
        apply_intent(&mut host, Intent::SetThemeVariant(ThemeVariant::Light), &ctx());
        assert_eq!(host.theme.variant, ThemeVariant::Light, "the variant is swapped on the host theme");
    }

    #[test]
    fn open_command_palette_sets_the_modal() {
        let mut host = host();
        apply_intent(&mut host, Intent::OpenCommandPalette, &ctx());
        assert_eq!(host.state.ui.modal, Some(Modal::CommandPalette));
    }

    #[test]
    fn close_modal_clears_the_modal() {
        let mut host = host();
        apply_intent(&mut host, Intent::OpenCommandPalette, &ctx());
        apply_intent(&mut host, Intent::CloseModal, &ctx());
        assert!(host.state.ui.modal.is_none(), "CloseModal clears whatever was open");
    }

    #[test]
    fn set_grid_changes_the_grid_mode() {
        let mut host = host();
        apply_intent(&mut host, Intent::SetGrid(GridMode::Px16), &ctx());
        assert_eq!(host.state.ui.grid, GridMode::Px16);
    }

    #[test]
    fn toggle_onion_skin_and_snap_flip_their_flags() {
        let mut host = host();
        let snap0 = host.state.ui.snap;
        apply_intent(&mut host, Intent::ToggleOnionSkin, &ctx());
        apply_intent(&mut host, Intent::ToggleSnap, &ctx());
        assert!(host.state.ui.onion_skin, "onion skin starts false and toggles on");
        assert_eq!(host.state.ui.snap, !snap0, "snap flips from its default");
    }

    #[test]
    fn set_zoom_records_the_zoom() {
        let mut host = host();
        apply_intent(&mut host, Intent::SetZoom(16.0), &ctx());
        assert_eq!(host.state.ui.zoom, 16.0);
    }

    #[test]
    fn run_action_pushes_a_queued_job_and_never_mutates_session_dirty() {
        let mut host = host();
        let was_dirty = host.state.session.dirty;
        apply_intent(&mut host, Intent::RunAction(ActionId("ai.fill")), &ctx());
        assert_eq!(host.state.session.jobs.len(), 1, "RunAction pushes exactly one JobStub");
        assert_eq!(host.state.session.jobs[0].state, JobState::Queued, "the job is queued");
        assert_eq!(
            host.state.session.dirty, was_dirty,
            "RunAction is a mock UI affordance and must never mutate project state (spec invariant)",
        );
    }
}
```

- [ ] **Step 2: Run the tests - they FAIL (no `apply_intent` yet).**

```powershell
cargo nextest run -p pixhaus-ui intent
```

Expected: FAIL with `error[E0425]: cannot find function 'apply_intent' in this scope` (or unresolved-import on `Host`/`Theme`/etc. if dependent layers are absent). This is the red half of TDD. Proceed to implement.

- [ ] **Step 3: Implement `apply_intent`.** Copy the structure exactly from spec lines 591-614, completing the arms the spec elides (`SetGrid` / `ToggleOnionSkin` / `ToggleSnap` / `SetZoom` / `OpenCommandPalette` / `CloseModal`). `SetThemeVariant` re-applies via `apply_to_visuals` (spec 605); `RunAction` pushes a `JobStub::queued(a)` and emits a `tracing::debug!` (spec 608-610); `SelectTrayTab` inserts into `tray_tab` for the active workspace (spec 597-600). Add it to `crates/ui/src/state/intent.rs` after the `IntentSink` impl and before the `#[cfg(test)]` block. Add the needed imports at the top of the file.

First, extend the import block at the top of `crates/ui/src/state/intent.rs`:

```rust
use crate::contrib_api::ids::{ActionId, PanelId, ToolId, WorkspaceId};
use crate::state::session::JobStub;
use crate::state::ui_state::{GridMode, Modal};
use crate::state::Host;
use crate::theme::{apply_to_visuals, Theme, ThemeVariant};
```

Then add the function:

```rust
/// Apply one intent to the host, after the frame's region borrows have dropped.
///
/// Takes the `egui::Context` because the theme path must re-apply to egui's visuals
/// on a variant change. `RunAction` is a mock UI affordance: it queues a job and logs
/// an event but NEVER mutates project state (spec invariant) - model edits route
/// through the reserved `Command` variant when `core` lands.
pub fn apply_intent(host: &mut Host, intent: Intent, ctx: &egui::Context) {
    match intent {
        Intent::SelectWorkspace(w) => {
            host.state.session.active_workspace = w;
            tracing::debug!(?w, "WorkspaceChanged");
        }
        Intent::SelectTool(t) => {
            host.state.session.active_tool = t;
            tracing::debug!(?t, "ToolChanged");
        }
        Intent::SelectTrayTab(p) => {
            let w = host.state.session.active_workspace;
            host.state.ui.tray_tab.insert(w, p);
        }
        Intent::TogglePanelCollapsed(p) => {
            let e = host.state.ui.collapsed.entry(p).or_insert(false);
            *e = !*e;
        }
        Intent::SetGrid(g) => {
            host.state.ui.grid = g;
        }
        Intent::ToggleOnionSkin => {
            host.state.ui.onion_skin = !host.state.ui.onion_skin;
        }
        Intent::ToggleSnap => {
            host.state.ui.snap = !host.state.ui.snap;
        }
        Intent::SetZoom(z) => {
            host.state.ui.zoom = z;
        }
        Intent::OpenCommandPalette => {
            host.state.ui.modal = Some(Modal::CommandPalette);
        }
        Intent::CloseModal => {
            host.state.ui.modal = None;
        }
        Intent::SetThemeVariant(v) => {
            host.theme = Theme::for_variant(v, host.theme.accent_seed());
            apply_to_visuals(&host.theme, ctx);
        }
        Intent::RunAction(a) => {
            host.state.session.jobs.push(JobStub::queued(a));
            tracing::debug!(?a, "ActionDispatched");
        }
    }
}
```

Note: the `ActionId`/`PanelId`/`ToolId`/`WorkspaceId`/`GridMode` imports added above cover both the function and the test module's references; if clippy flags an unused import after the test module already imports its own (the test module has its own `use` block), keep the top-level `use` list to exactly what `apply_intent` itself names (`PanelId`, `ToolId`, `WorkspaceId` are used inside arms via the `Intent` variants' payloads but those types are already in scope through `Intent`'s definition - only `ActionId`, `JobStub`, `GridMode`, `Modal`, `Host`, `apply_to_visuals`, `Theme`, `ThemeVariant` are actually named in the body). Trim the top-level `use` to the names the body references; the post-edit clippy gate (`unused_imports`) will tell you exactly which to drop.

- [ ] **Step 4: Run the tests - they now PASS.**

```powershell
cargo nextest run -p pixhaus-ui intent
```

Expected: PASS for all `intent::tests::*` cases (`push_appends_intents_in_order`, `select_workspace_flips_active_workspace`, `select_tool_flips_active_tool`, `select_tray_tab_updates_the_active_workspaces_tab`, `toggle_panel_collapsed_flips_then_flips_back`, `set_theme_variant_swaps_the_variant`, `open_command_palette_sets_the_modal`, `close_modal_clears_the_modal`, `set_grid_changes_the_grid_mode`, `toggle_onion_skin_and_snap_flip_their_flags`, `set_zoom_records_the_zoom`, `run_action_pushes_a_queued_job_and_never_mutates_session_dirty`).

- [ ] **Step 5: Run clippy.**

```powershell
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```

Expected: PASS. (No `unwrap`/`expect`/`panic` in `apply_intent`; the only fallible-looking call, `entry().or_insert()`, is infallible.)

- [ ] **Step 6: Run the doc tests for the crate** (nextest does not run them; the spec's gate requires them green):

```powershell
cargo test --doc -p pixhaus-ui
```

Expected: PASS (this layer adds no `///` code-fence examples, so there are zero doc tests in `state/` - the command still succeeds and confirms nothing regressed).

- [ ] **Step 7: Commit.**

```powershell
git add crates/ui/src/state/intent.rs
git commit -m @'
feat(ui): add apply_intent with the spec-3 state tests

apply_intent applies one intent post-frame: workspace/tool selection, the
per-workspace tray tab, panel collapse, grid/onion/snap/zoom, the modal, the
theme-variant swap that re-applies to egui visuals, and the RunAction mock
that queues a job without touching project state. Covered by the spec test 3
suite driving apply_intent directly against a headless Context.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
</parameter>
```

---

### STATE.7: Layer verification gate

**Files:**
- Test: the whole `crates/ui/src/state/` tree (no new files)

- [ ] **Step 1: Run the full crate test suite to confirm the layer is green end-to-end.**

```powershell
cargo nextest run -p pixhaus-ui
```

Expected: PASS for every `state::*`, `session::*`, `ui_state::*`, and `intent::*` test, alongside the other layers' tests already in the crate. If a test from another layer fails, that is that layer's concern - your `state::`/`session::`/`ui_state::`/`intent::` cases must all be green.

- [ ] **Step 2: Run clippy across all targets one final time for this layer's crate.**

```powershell
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```

Expected: PASS with zero warnings.

- [ ] **Step 3: Confirm no commit is needed** - STATE.1 through STATE.6 each committed their own work, and this task changed no files. If steps 1-2 surfaced a fix in a `state/` file, commit it with a `fix(ui):` subject and the standard trailer; otherwise this task produces no commit.

## Phase 6: Shared widgets

I have all the context I need. Now I'll write the WIDGETS layer plan.

### WIDGETS layer - shared widget helpers

Implements the spec's `widgets/` module tree (design spec lines 108-116, the per-region tier table at 826-842, and the rail/tab/card visual rules at 868-882 and 959-963). These are pure egui-drawing helpers consumed by the SHELL regions and the MODULES panels. Signatures are the shared contract: SHELL's `right_dock.rs` calls `widgets::card(ui, theme, &meta, collapsed, |ui| {...})`, the left rail calls `widgets::tool_button`, the top bar calls `widgets::workspace_tab`, the bottom tray calls `widgets::tray_tab`, and panel bodies call `section_header` + the `placeholder` mocks. No unit tests - these draw pixels; the rhythm is write -> `cargo build` / `cargo clippy` -> commit. Type names from other layers (`Theme`, `PanelMeta`, `ToolMeta`, `crate::icons`) are the exact spec names and are assumed to exist (THEME and CONTRIB_API layers own them).

A standing rule for every step here: use theme tokens only, never a hex literal or `Color32::from_rgb`. The skill-verified 0.34 API these helpers lean on: `egui::Frame { fill, inner_margin, corner_radius, shadow, stroke, .. }`, `ui.allocate_exact_size(size, sense)`, `painter.rect_filled` / `painter.rect_stroke(rect, cr, stroke, StrokeKind::Inside)`, `egui::Stroke::new`, `egui::RichText`, `response.on_hover_text`, `egui::CornerRadius::same(u8)`, `egui::Sense::click()`.

---

### WIDGETS.1: Widget module scaffold (`widgets/mod.rs`)

Create the `widgets` module, declare its submodules, and re-export the public helper functions so callers write `widgets::card`, `widgets::tool_button`, etc. (matching the call sites in spec lines 682, 872, 866, 880). Wire the module into `lib.rs`.

**Files:**
- Create: `crates/ui/src/widgets/mod.rs`
- Modify: `crates/ui/src/lib.rs`

- [ ] **Step 1: Create `widgets/mod.rs` with submodule declarations and re-exports.**

```rust
//! Shared egui-drawing helpers for the Pixhaus shell.
//!
//! These are presentation primitives, nothing more: a card frame, the rail tool
//! button, workspace and tray tabs, a section header, and the placeholder mocks
//! that stand in for real panel content this round. They paint with theme tokens
//! only - never a hex literal - so a theme-variant swap recolors them for free.
//!
//! Concrete `Panel`/`Tool`/`Workspace` impls do NOT belong here; they live in the
//! `modules/*` crates. This module is shared chrome the regions and panels call.

mod card;
mod placeholder;
mod section_header;
mod tool_button;
mod tray_tab;
mod workspace_tab;

pub use card::card;
pub use placeholder::{mock_log, mock_row, mock_thumbnail_grid};
pub use section_header::section_header;
pub use tool_button::tool_button;
pub use tray_tab::tray_tab;
pub use workspace_tab::workspace_tab;
```

- [ ] **Step 2: Declare the `widgets` module in `lib.rs`.** Add the module line after the crate doc comment, before the `use` block (keep the existing `install_canvas_renderer`/`CanvasCallback` untouched). Insert immediately after line 11's `//! workspaces are built.` doc line / before line 13's `use egui::epaint::PaintCallbackInfo;`:

```rust
pub mod widgets;
```

Edit target - replace the blank line between the module doc comment and the first `use`:

```rust
//! workspaces are built.

use egui::epaint::PaintCallbackInfo;
```

becomes

```rust
//! workspaces are built.

pub mod widgets;

use egui::epaint::PaintCallbackInfo;
```

- [ ] **Step 3: Build the crate.** The submodule files do not exist yet, so this MUST fail - that is the expected, useful signal that the scaffold is wired and the bodies are next.

```powershell
cargo build -p pixhaus-ui
```

Expected result: FAIL with `error[E0583]: file not found for module 'card'` (and the same for `placeholder`, `section_header`, `tool_button`, `tray_tab`, `workspace_tab`). This confirms the module tree is declared correctly. Do not commit yet - the next tasks add the bodies; commit each as it lands.

---

### WIDGETS.2: `section_header` (the leaf helper)

`section_header(ui, theme, icon, title)` - an icon glyph + title row painted at the `section_header` type-scale, used as a card title and inside panel bodies (spec line 114, 913). Build this first because `card` reuses it. No `Response` needed; it is a passive label row.

**Files:**
- Create: `crates/ui/src/widgets/section_header.rs`

- [ ] **Step 1: Write `section_header.rs`.** The icon renders in `accent.base`, the title in `text_primary` at `type_scale.section_header`. Use `RichText` so both share one `ui.label` line; lay them out with `ui.horizontal`.

```rust
//! Icon + title header, used as a card title and as an in-body section divider.

use crate::theme::Theme;

/// Draw an `icon title` header row at the section-header type scale.
///
/// `icon` is a `crate::icons` phosphor glyph. The glyph paints in `accent.base`,
/// the title in `text_primary`. Passive: no interaction, no `Response`.
pub fn section_header(ui: &mut egui::Ui, theme: &Theme, icon: char, title: &str) {
    let size = theme.type_scale.section_header;
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(icon.to_string())
                .size(size)
                .color(theme.accent.base),
        );
        ui.label(
            egui::RichText::new(title)
                .size(size)
                .color(theme.roles.text_primary)
                .strong(),
        );
    });
}
```

- [ ] **Step 2: Build the crate.** The other submodules are still missing, so the build still fails - but it must NOT fail on `section_header.rs` itself. Confirm there is no error pointing at `widgets/section_header.rs`.

```powershell
cargo build -p pixhaus-ui
```

Expected result: FAIL, but only with `error[E0583] file not found for module` lines for the four still-missing files (`card`, `placeholder`, `tool_button`, `tray_tab`, `workspace_tab` minus whichever exist). No type/borrow error referencing `section_header.rs`. If `section_header.rs` itself errors, fix it before moving on.

- [ ] **Step 3: Commit.**

```powershell
git add crates/ui/src/widgets/mod.rs crates/ui/src/lib.rs crates/ui/src/widgets/section_header.rs
git commit -m @'
feat(ui): add widgets module scaffold and section_header helper

Declares the shared widgets module tree and lands the leaf section_header
helper that card and panel bodies reuse. Theme tokens only, no hex literals.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### WIDGETS.3: `card` (the elevated panel frame)

`card(ui, theme, &PanelMeta, collapsed, body: impl FnOnce(&mut Ui)) -> egui::Response` - an elevated `Frame` (fill `surfaces.elevated`, `elevation.raised` shadow, `roles.border` stroke) with a clickable header (`section_header` + a collapse chevron) and, when not collapsed, the `body` closure. Matches the call site in spec lines 681-695: the shell wraps the call in `ui.push_id(id, ...)`, so `card` does NOT scope ids itself. Collapse state is read-only here - `card` returns the header-click `Response`; the caller turns a click into `Intent::TogglePanelCollapsed` (spec line 247, 601). The chevron glyph flips with `collapsed`.

**Files:**
- Create: `crates/ui/src/widgets/card.rs`

- [ ] **Step 1: Write `card.rs`.** The signature, body-gating, and returned `Response` follow the spec's right-dock loop exactly. The header is one clickable row: title via `section_header`-style content plus a trailing chevron (`icons::CARET_RIGHT` when collapsed, `icons::CARET_DOWN` when open) right-aligned. Sense the whole header rect for clicks so clicking anywhere on the title bar toggles. Body renders only when `!collapsed`.

```rust
//! The elevated card frame for right-dock and tray panels.

use crate::icons;
use crate::theme::Theme;

/// Draw an elevated card: a framed header (`meta.icon` + `meta.title` + a collapse
/// chevron) and, when `!collapsed`, the `body`.
///
/// Collapse state is read-only here. The returned [`egui::Response`] is the header
/// click; the caller maps a click to `Intent::TogglePanelCollapsed` - this widget
/// owns no state and mutates nothing. The shell scopes egui ids via `push_id`
/// around this call, so `card` adds no id salt of its own.
pub fn card(
    ui: &mut egui::Ui,
    theme: &Theme,
    meta: &crate::contrib_api::PanelMeta,
    collapsed: bool,
    body: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let frame = egui::Frame {
        fill: theme.surfaces.elevated,
        inner_margin: egui::Margin::same(theme.spacing.sm as i8),
        corner_radius: egui::CornerRadius::same(theme.radius.md as u8),
        shadow: theme.elevation.raised,
        stroke: egui::Stroke::new(1.0, theme.roles.border),
        ..Default::default()
    };

    let mut header_response = None;
    frame.show(ui, |ui| {
        // Header: icon + title on the left, collapse chevron on the right. The whole
        // strip is one interactive rect so a click anywhere toggles.
        let resp = ui
            .horizontal(|ui| {
                let size = theme.type_scale.section_header;
                ui.label(
                    egui::RichText::new(meta.icon.to_string())
                        .size(size)
                        .color(theme.accent.base),
                );
                ui.label(
                    egui::RichText::new(meta.title)
                        .size(size)
                        .color(theme.roles.text_primary)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let chevron = if collapsed {
                        icons::CARET_RIGHT
                    } else {
                        icons::CARET_DOWN
                    };
                    ui.label(
                        egui::RichText::new(chevron.to_string())
                            .size(theme.type_scale.label)
                            .color(theme.roles.text_secondary),
                    );
                });
            })
            .response
            .interact(egui::Sense::click());
        header_response = Some(resp);

        if !collapsed {
            ui.add_space(theme.spacing.xs);
            body(ui);
        }
    });

    // `frame.show` always runs the closure once, so the header response is set.
    header_response.unwrap_or_else(|| ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover()))
}
```

- [ ] **Step 2: Build the crate.** `card.rs` must compile clean; only the still-missing submodules should error. Watch specifically for `Frame` field names and the `Margin::same`/`CornerRadius::same` integer-cast types - `Margin` takes `i8`, `CornerRadius::same` takes `u8` in egui 0.34.

```powershell
cargo build -p pixhaus-ui
```

Expected result: FAIL only with `E0583 file not found` for `placeholder`, `tool_button`, `tray_tab`, `workspace_tab` (and any not-yet-built). No error in `card.rs`. If `Frame` field names or `Shadow`/`Margin` types mismatch, open the `pixhaus-egui` `references/layout-and-panels.md` for the exact `Frame` struct shape and adjust - do not guess.

- [ ] **Step 3: Commit.**

```powershell
git add crates/ui/src/widgets/card.rs
git commit -m @'
feat(ui): add card helper for the elevated panel frame

Draws an elevated Frame with a clickable header (icon, title, collapse
chevron) and a gated body. Returns the header click Response; the caller
maps it to TogglePanelCollapsed - the widget owns no state.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### WIDGETS.4: `tool_button` (the rail icon button)

`tool_button(ui, theme, &ToolMeta, active) -> egui::Response` - a fixed-size icon button for the 48px left rail (spec line 869-872). Active state paints `accent.muted` background fill + a 2px `accent.base` left line. The tooltip is `"{label} ({shortcut})\n{tooltip}"`. When `meta.is_ai`, the icon tints `accent.ai` and a small `icons::SPARKLE` marker overlays (spec line 320, 842, 872, 920). Built as a custom widget: allocate a square rect, sense click, paint with the `Painter` (the skill's `ColorSwatch` pattern at widgets.md:165-187).

**Files:**
- Create: `crates/ui/src/widgets/tool_button.rs`

- [ ] **Step 1: Write `tool_button.rs`.** Allocate a ~40px square inside the 48px rail, sense `click()`, paint: active background, active left line, the glyph centered, the AI sparkle in the top-right corner when `is_ai`, then attach the composed tooltip. Format the shortcut from `meta.shortcut` if present.

```rust
//! The left-rail tool button: icon, active accent tint + left line, AI marker.

use crate::icons;
use crate::theme::Theme;

/// Draw a rail tool button. `active` paints an `accent.muted` background and a 2px
/// `accent.base` left line; `meta.is_ai` tints the glyph `accent.ai` and overlays a
/// sparkle. The tooltip reads `"{label} ({shortcut})\n{tooltip}"` (the shortcut
/// clause is dropped when the tool has none).
pub fn tool_button(
    ui: &mut egui::Ui,
    theme: &Theme,
    meta: &crate::contrib_api::ToolMeta,
    active: bool,
) -> egui::Response {
    let side = 40.0;
    let (rect, response) =
        ui.allocate_exact_size(egui::Vec2::splat(side), egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        // Active background fill, then the 2px accent left line.
        if active {
            painter.rect_filled(rect, theme.radius.sm as u8, theme.accent.muted);
            let line_x = rect.left() + 1.0;
            painter.line_segment(
                [
                    egui::pos2(line_x, rect.top() + 2.0),
                    egui::pos2(line_x, rect.bottom() - 2.0),
                ],
                egui::Stroke::new(2.0, theme.accent.base),
            );
        } else if response.hovered() {
            painter.rect_filled(rect, theme.radius.sm as u8, theme.accent.muted.gamma_multiply(0.5));
        }

        // The glyph. AI tools paint in the AI accent; everything else uses primary
        // text, brightened to the accent when active.
        let glyph_color = if meta.is_ai {
            theme.accent.ai
        } else if active {
            theme.accent.base
        } else {
            theme.roles.text_secondary
        };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            meta.icon.to_string(),
            egui::FontId::proportional(theme.type_scale.title),
            glyph_color,
        );

        // AI marker: a small sparkle in the top-right corner.
        if meta.is_ai {
            painter.text(
                egui::pos2(rect.right() - 4.0, rect.top() + 4.0),
                egui::Align2::RIGHT_TOP,
                icons::SPARKLE.to_string(),
                egui::FontId::proportional(theme.type_scale.label),
                theme.accent.ai,
            );
        }
    }

    let tooltip = match meta.shortcut {
        Some(shortcut) => format!(
            "{} ({})\n{}",
            meta.label,
            ui.ctx().format_shortcut(&shortcut),
            meta.tooltip
        ),
        None => format!("{}\n{}", meta.label, meta.tooltip),
    };
    response.on_hover_text(tooltip)
}
```

- [ ] **Step 2: Build the crate.** Confirm `tool_button.rs` compiles. The two API calls to verify here against the skill: `ui.ctx().format_shortcut(&KeyboardShortcut)` (used to render the shortcut text) and `Color32::gamma_multiply` (the hover dim). Both are standard egui 0.34; if `format_shortcut` resolves differently, check `pixhaus-egui` `references/input-state-and-theming.md`.

```powershell
cargo build -p pixhaus-ui
```

Expected result: FAIL only with `E0583 file not found` for the remaining missing submodules (`placeholder`, `tray_tab`, `workspace_tab`). No error in `tool_button.rs`.

- [ ] **Step 3: Commit.**

```powershell
git add crates/ui/src/widgets/tool_button.rs
git commit -m @'
feat(ui): add tool_button rail helper

Fixed-size icon button: active paints accent.muted fill plus a 2px
accent.base left line; AI tools tint the glyph accent.ai and overlay a
sparkle. Tooltip composes label, shortcut, and description.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### WIDGETS.5: `workspace_tab` and `tray_tab` (the two pill tabs)

Two tab chips drawn in the same pass because they share the active-pill idiom. `workspace_tab(ui, theme, name, active) -> Response` is the top-bar violet tab: active = `accent.muted` pill + `accent.base` underline + brighter text (spec line 865-866). `tray_tab(ui, theme, title, active) -> Response` is the bottom-tray chip: active = `accent` pill (spec line 113, 880-881). Both sense `click()`; the caller maps the click to `Intent::SelectWorkspace` / `Intent::SelectTrayTab`.

**Files:**
- Create: `crates/ui/src/widgets/workspace_tab.rs`
- Create: `crates/ui/src/widgets/tray_tab.rs`

- [ ] **Step 1: Write `workspace_tab.rs`.** Size the chip to its text plus padding; paint the muted pill + bottom underline when active, brighten the label when active.

```rust
//! The top-bar workspace tab: violet pill plus an accent underline when active.

use crate::theme::Theme;

/// Draw a workspace tab. Active paints an `accent.muted` pill, an `accent.base`
/// underline, and brighter text. Returns the click `Response`; the caller maps a
/// click to `Intent::SelectWorkspace`.
pub fn workspace_tab(ui: &mut egui::Ui, theme: &Theme, name: &str, active: bool) -> egui::Response {
    let font = egui::FontId::proportional(theme.type_scale.body);
    let galley = ui.painter().layout_no_wrap(
        name.to_owned(),
        font.clone(),
        theme.roles.text_primary,
    );
    let pad = egui::vec2(theme.spacing.md, theme.spacing.sm);
    let size = galley.size() + pad * 2.0;
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if active {
            painter.rect_filled(rect, theme.radius.md as u8, theme.accent.muted);
            painter.line_segment(
                [
                    egui::pos2(rect.left() + theme.spacing.sm, rect.bottom() - 1.0),
                    egui::pos2(rect.right() - theme.spacing.sm, rect.bottom() - 1.0),
                ],
                egui::Stroke::new(2.0, theme.accent.base),
            );
        }
        let text_color = if active {
            theme.roles.text_primary
        } else {
            theme.roles.text_secondary
        };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            name,
            font,
            text_color,
        );
    }
    response
}
```

- [ ] **Step 2: Write `tray_tab.rs`.** Same shape, but the active state is a solid `accent` pill (the spec calls it the "accent pill chip"), with text flipping to a readable on-accent color - use `roles.text_primary` for active (the accent is dark-violet, text reads on it) and `text_secondary` for inactive.

```rust
//! The bottom-tray tab chip: a solid accent pill when active.

use crate::theme::Theme;

/// Draw a tray tab chip. Active paints a solid `accent.base` pill; inactive is bare
/// text. Returns the click `Response`; the caller maps a click to
/// `Intent::SelectTrayTab`.
pub fn tray_tab(ui: &mut egui::Ui, theme: &Theme, title: &str, active: bool) -> egui::Response {
    let font = egui::FontId::proportional(theme.type_scale.label);
    let galley = ui.painter().layout_no_wrap(
        title.to_owned(),
        font.clone(),
        theme.roles.text_primary,
    );
    let pad = egui::vec2(theme.spacing.sm, theme.spacing.xs);
    let size = galley.size() + pad * 2.0;
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if active {
            painter.rect_filled(rect, theme.radius.md as u8, theme.accent.base);
        } else if response.hovered() {
            painter.rect_filled(rect, theme.radius.md as u8, theme.accent.muted);
        }
        let text_color = if active {
            theme.roles.text_primary
        } else {
            theme.roles.text_secondary
        };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            title,
            font,
            text_color,
        );
    }
    response
}
```

- [ ] **Step 3: Build the crate.** Both tab files should compile; only `placeholder` remains missing. The API to confirm: `ui.painter().layout_no_wrap(String, FontId, Color32) -> Arc<Galley>` and `galley.size() -> Vec2` (from painting-and-textures.md:147). `Vec2 * f32` and `Vec2 + Vec2` are standard.

```powershell
cargo build -p pixhaus-ui
```

Expected result: FAIL only with `E0583 file not found for module 'placeholder'`. No error in `workspace_tab.rs` or `tray_tab.rs`.

- [ ] **Step 4: Commit.**

```powershell
git add crates/ui/src/widgets/workspace_tab.rs crates/ui/src/widgets/tray_tab.rs
git commit -m @'
feat(ui): add workspace_tab and tray_tab pill helpers

workspace_tab paints an accent.muted pill plus an accent.base underline
when active; tray_tab paints a solid accent pill. Both return the click
Response for the caller to route as a Select intent.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### WIDGETS.6: `placeholder` mocks (`mock_row`, `mock_thumbnail_grid`, `mock_log`)

The three mock-content helpers panel bodies use to stand in for real data this round (spec line 115, 957-994). `mock_row(ui, theme, label)` draws one labeled list row. `mock_thumbnail_grid(ui, theme, n)` draws `n` checkerboard thumbnail rects in a wrapping grid (Sprites/Assets/Tile Variants). `mock_log(ui, theme, lines)` draws a monospace, `text_secondary` log block (Console/Export Log). All passive - no `Response` returned; they are inert filler.

**Files:**
- Create: `crates/ui/src/widgets/placeholder.rs`

- [ ] **Step 1: Write `placeholder.rs`.** Keep each helper small and self-contained. The checkerboard thumbnail is two-tone `surfaces.inset` / `surfaces.elevated` tiles painted into a fixed-size allocated rect; lay the grid out with `ui.horizontal_wrapped`. The log is a vertical run of monospace labels.

```rust
//! Inert mock-content helpers: a list row, a thumbnail grid, a log block. These
//! stand in for real panel data until `core` lands. All passive - no interaction.

use crate::theme::Theme;

/// One labeled list row at body type scale, secondary text color.
pub fn mock_row(ui: &mut egui::Ui, theme: &Theme, label: &str) {
    ui.label(
        egui::RichText::new(label)
            .size(theme.type_scale.body)
            .color(theme.roles.text_secondary),
    );
}

/// A wrapping grid of `n` checkerboard thumbnail rects (mock sprites / assets /
/// tiles). Each cell is a small two-tone checker so transparent bounds read.
pub fn mock_thumbnail_grid(ui: &mut egui::Ui, theme: &Theme, n: usize) {
    let cell = 44.0;
    ui.horizontal_wrapped(|ui| {
        for _ in 0..n {
            let (rect, _) =
                ui.allocate_exact_size(egui::Vec2::splat(cell), egui::Sense::hover());
            if ui.is_rect_visible(rect) {
                let painter = ui.painter();
                // 4x4 checker inside the cell.
                let step = cell / 4.0;
                for ty in 0..4 {
                    for tx in 0..4 {
                        let dark = (tx + ty) % 2 == 0;
                        let fill = if dark {
                            theme.surfaces.inset
                        } else {
                            theme.surfaces.elevated
                        };
                        let min = egui::pos2(
                            rect.left() + tx as f32 * step,
                            rect.top() + ty as f32 * step,
                        );
                        painter.rect_filled(
                            egui::Rect::from_min_size(min, egui::Vec2::splat(step)),
                            0u8,
                            fill,
                        );
                    }
                }
                painter.rect_stroke(
                    rect,
                    theme.radius.sm as u8,
                    egui::Stroke::new(1.0, theme.roles.border),
                    egui::StrokeKind::Inside,
                );
            }
        }
    });
}

/// A monospace log block in secondary text, one line per entry (mock console).
pub fn mock_log(ui: &mut egui::Ui, theme: &Theme, lines: &[&str]) {
    for line in lines {
        ui.label(
            egui::RichText::new(*line)
                .monospace()
                .size(theme.type_scale.mono)
                .color(theme.roles.text_secondary),
        );
    }
}
```

- [ ] **Step 2: Build the crate.** All six submodules now exist; the whole `widgets` module must compile clean. Confirm `rect_stroke` takes `(rect, impl Into<CornerRadius>, Stroke, StrokeKind)` and that `0u8`/`theme.radius.sm as u8` satisfy `Into<CornerRadius>` (painting-and-textures.md:61-63 confirms `From<u8>`).

```powershell
cargo build -p pixhaus-ui
```

Expected result: PASS (the `widgets` module compiles; the rest of the crate's compile status depends on the THEME and CONTRIB_API layers being present - if those are not yet merged, expect unresolved-import errors pointing at `crate::theme`/`crate::contrib_api`/`crate::icons`, NOT at the `widgets/*.rs` bodies). If errors point only at `crate::theme` / `crate::contrib_api` / `crate::icons`, that is the cross-layer dependency, not a defect in this layer - proceed.

- [ ] **Step 3: Run clippy on the crate with `-D warnings`.** The post-edit hook already ran clippy on each touched file, but run it explicitly across the crate's targets to confirm the whole `widgets` module is warning-clean (pedantic is on; watch for `cast_possible_truncation` on the `as u8`/`as i8` casts - these are deliberate, bounded by the small token values, and if pedantic flags them, add a scoped `#[allow(clippy::cast_possible_truncation)]` on the specific helper with a one-line `// token values are small, bounded constants` comment).

```powershell
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```

Expected result: PASS, no warnings. (Same caveat as Step 2: if THEME/CONTRIB_API are absent, the failure is unresolved imports, not a widget defect.)

- [ ] **Step 4: Commit.**

```powershell
git add crates/ui/src/widgets/placeholder.rs
git commit -m @'
feat(ui): add placeholder mock helpers for panel content

mock_row, mock_thumbnail_grid, and mock_log stand in for real panel data
until core lands. Checkerboard thumbnails read transparent bounds; the log
is monospace secondary text. All passive, theme tokens only.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### WIDGETS.7: Layer verification pass

A final whole-layer check once the THEME and CONTRIB_API layers are in place, so the WIDGETS helpers are confirmed against the real `Theme`/`PanelMeta`/`ToolMeta`/`icons` types rather than against the assumed contract.

**Files:**
- Test: (no new files - verification only across `crates/ui/src/widgets/*`)

- [ ] **Step 1: Build the full crate.** With THEME and CONTRIB_API merged, every `crate::theme::*`, `crate::contrib_api::*`, and `crate::icons::*` reference resolves.

```powershell
cargo build -p pixhaus-ui
```

Expected result: PASS. If a field name differs from the spec (for example `Shadow`'s field shape, which the spec flagged "verify" at line 824 - the THEME layer owns `Elevation::raised` as `egui::epaint::Shadow`, and `card` consumes it as `frame.shadow`), the error will point at the consuming line in `card.rs`. Adjust the consumption to match the THEME layer's actual `Shadow` token; do not redefine the token here.

- [ ] **Step 2: Run clippy across the crate.** Confirms the helpers stay clean against the real types.

```powershell
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```

Expected result: PASS, no warnings.

- [ ] **Step 3: Run the crate's tests** (the WIDGETS layer adds none, but SHELL/MODULES/THEME tests exercise these helpers indirectly; this confirms nothing regressed).

```powershell
cargo nextest run -p pixhaus-ui
```

Expected result: PASS. No WIDGETS-owned tests run here - the helpers are visual and verified by the SHELL smoke path and manual inspection, per the spec's "Not worth testing yet: ... tool `options_ui` widgets" note (line 1076). If the build and clippy are green, the WIDGETS layer is complete. No commit needed unless Step 1 forced a `card.rs` adjustment, in which case:

```powershell
git add crates/ui/src/widgets/card.rs
git commit -m @'
fix(ui): align card shadow consumption with the Shadow token shape

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

## Phase 7: Shell runtime and regions

All confirmed. `Key::Num1..Num5` and `Modifiers::COMMAND` exist. I have every signature I need. Now I'll write the SHELL layer plan.

Key facts baked in for my layer:
- Focus gate helper: `ctx.text_edit_focused()` (the precise one; `consume_key` is the fallback per spec).
- `ui.close()`, `ui.menu_button`, `consume_shortcut`/`consume_key`, `Area::new().order(Order::Foreground)`, `Painter` calls, `Panel::*().show_inside`, `allocate_painter` + `egui_wgpu::Callback::new_paint_callback`, `CanvasCallback` preserved.
- Shared contract types from other layers (use exact names): `Host`, `SessionState`, `UiState`, `Theme`, `IntentSink`, `Intent`, `apply_intent`, `ContribCtx`, `PanelScope`, `Registries`, `resolve_layout`, `ResolvedLayout`, `Region`, `region_id`, `PanelId`/`ToolId`/`WorkspaceId`/`ActionId`, `MenuGroup`, `widgets::card`/`tool_button`/`workspace_tab`/`tray_tab`, `icons::*`, `BackgroundChannel`, `AiStatus`, `GridMode`, `Modal`, `JobStub`.

Here is my layer.

---

## Layer SHELL - Shell runtime, regions, palette, shortcuts, menus

Implements spec sections "The borrow-safe per-frame loop", "Region composition and the shell runtime", "Tooling stubs (command palette, shortcuts, menus, status bar)", and spec test 6. Files: `crates/ui/src/shell/{mod.rs, runtime.rs, command_palette.rs, shortcuts.rs, menus.rs}` and `crates/ui/src/shell/regions/{top_bar.rs, tool_options.rs, left_rail.rs, right_dock.rs, bottom_tray.rs, status_bar.rs, canvas_stage.rs}`.

This layer is the crux: the borrow-safe `Shell::run` order, the reborrow-then-destructure region loops, and the `push_id`-per-panel pattern were adversarially verified in the spec - copy them verbatim, do not "simplify". This layer consumes types defined by the THEME, CONTRIB-API, STATE, REGISTRY, and WIDGETS layers; use the exact names from the spec (they are the shared contract) and assume they exist.

Two egui-0.34 facts I confirmed against the pinned source, which this layer depends on:
- The focus gate for tool keys is `ctx.text_edit_focused()` - "is the currently focused widget a text edit?" (`wants_keyboard_input()` is deprecated in 0.34.2; `egui_wants_keyboard_input()` is too broad - it fires for any focused widget). Use `text_edit_focused()`; `consume_key` is the fallback the spec names.
- `ui.close()` dismisses a menu (not `ui.close_menu()`); `consume_shortcut`/`consume_key(mods, key)` both test-and-consume so a focused `TextEdit` and the global handler never double-fire.

Branch is already `feat/ui-shell-foundation` - do not create one. Every cargo command runs through PowerShell (the Bash tool's `link.exe` shadows the MSVC linker). The post-edit hook auto-formats and runs `clippy --tests -D warnings` on `pixhaus-ui` after each Edit/Write; the explicit clippy/test steps below are still required as gates.

---

### SHELL.1: shell module skeleton and the borrow-safe runtime

This creates the `shell` module tree with stub region functions so the load-bearing `Shell::run` order compiles before any region has a body. The region bodies arrive in SHELL.4-SHELL.10; the stubs let `runtime.rs` (the adversarially-verified piece) compile and be reviewed first.

**Files:**
- Create: `crates/ui/src/shell/mod.rs`
- Create: `crates/ui/src/shell/runtime.rs`
- Create: `crates/ui/src/shell/regions/mod.rs`
- Create (stubs): `crates/ui/src/shell/regions/{top_bar.rs, tool_options.rs, left_rail.rs, right_dock.rs, bottom_tray.rs, status_bar.rs, canvas_stage.rs}`
- Create (stubs): `crates/ui/src/shell/{command_palette.rs, shortcuts.rs, menus.rs}`
- Modify: `crates/ui/src/lib.rs` (add `pub mod shell;` - keep `CanvasCallback`/`install_canvas_renderer` untouched)

- [ ] **Step 1: Create the seven region stub files.** Each is a no-op `show(host, ui)` so the runtime compiles. Create every file with this exact body, substituting the module name in the doc line.

`crates/ui/src/shell/regions/top_bar.rs`:
```rust
//! Top bar region: menus + workspace tabs + global status. Filled in SHELL.4.

use crate::state::Host;

/// Render the top-bar region. Body lands in SHELL.4.
pub fn show(_host: &mut Host, _ui: &mut egui::Ui) {}
```
Repeat for `tool_options.rs` (doc: "Tool-options region: the active tool's options_ui. Filled in SHELL.5."), `left_rail.rs` ("Left-rail region: the tool rail. Filled in SHELL.6."), `right_dock.rs` ("Right-dock region: the panel card stack. Filled in SHELL.7."), `bottom_tray.rs` ("Bottom-tray region: tab row + selected tray panel. Filled in SHELL.8."), `status_bar.rs` ("Status-bar region: compact status strip. Filled in SHELL.9."), `canvas_stage.rs` ("Canvas-stage region: framed artboard + checker + grid + HUD + CanvasCallback. Filled in SHELL.10.").

- [ ] **Step 2: Create the three tooling stub files** so `runtime.rs` can call `shortcuts::collect` and `command_palette::overlay`.

`crates/ui/src/shell/shortcuts.rs`:
```rust
//! Global shortcut collection: workspace Cmd+1..5, Cmd+K, focus-gated tool keys.
//! Real body and the pure key->intent mapping land in SHELL.11.

use crate::registry::Registries;
use crate::state::IntentSink;

/// Read input once per frame and push the resulting intents. Body lands in SHELL.11.
pub fn collect(_ctx: &egui::Context, _registries: &Registries, _intents: &mut IntentSink) {}
```

`crates/ui/src/shell/command_palette.rs`:
```rust
//! Command palette overlay (Ctrl/Cmd+K). egui::Area, modal-gated. Body lands in SHELL.12.

use crate::state::Host;

/// Draw the palette overlay when the modal is open. Body lands in SHELL.12.
pub fn overlay(_host: &mut Host, _ui: &mut egui::Ui) {}
```

`crates/ui/src/shell/menus.rs`:
```rust
//! The shell's always-present top-bar menu groups, as data. Body lands in SHELL.13.

use crate::contrib_api::module::MenuGroup;

/// The menu groups the shell owns (File/Edit/View/Window/Help and the empty
/// module-contributed slots). Body lands in SHELL.13.
pub fn shell_menu_groups() -> Vec<MenuGroup> {
    Vec::new()
}
```

- [ ] **Step 3: Create `crates/ui/src/shell/regions/mod.rs`** declaring the region submodules.
```rust
//! The seven window regions. Each exposes `show(host, ui)`; the runtime calls them
//! outer-first, central last (egui panel-ordering contract).

pub mod bottom_tray;
pub mod canvas_stage;
pub mod left_rail;
pub mod right_dock;
pub mod status_bar;
pub mod tool_options;
pub mod top_bar;
```

- [ ] **Step 4: Create `crates/ui/src/shell/runtime.rs`** with the borrow-safe per-frame loop, copied verbatim from the spec ("The borrow-safe per-frame loop"). The order is the contract: outer panels first, `CentralPanel` (canvas) last, palette overlay after, then drain intents past all region borrows.
```rust
//! The per-frame shell composition and the post-loop intent drain.
//!
//! Three things make this loop borrow-check (spec "The borrow-safe per-frame
//! loop"): panels get a read-only state view plus write channels; `Panel::ui` and
//! `Tool::options_ui` are `&self`, so iterating the registries is a shared borrow
//! that coexists with the sibling `&mut` fields; and mutation is deferred past the
//! loop, where intents are drained and applied after every region borrow drops.
//! The one-frame latency is invisible in immediate mode.

use crate::shell::{command_palette, regions, shortcuts};
use crate::state::{apply_intent, Host};

/// The shell runtime. Owns the per-frame region composition and intent drain.
pub struct Shell;

impl Shell {
    /// Compose every region for this frame, then apply the intents collected.
    ///
    /// Region order is the egui panel-ordering contract: outer panels first, the
    /// `CentralPanel` (canvas stage) last, the palette `Area` after that. The
    /// status bar is declared before the tray so it pins to the lower edge.
    pub fn run(host: &mut Host, ui: &mut egui::Ui) {
        host.intents.clear();
        shortcuts::collect(ui.ctx(), &host.registries, &mut host.intents);

        // egui panel order: outer panels first, CentralPanel LAST.
        regions::top_bar::show(host, ui);
        regions::tool_options::show(host, ui);
        regions::left_rail::show(host, ui);
        regions::status_bar::show(host, ui); // outermost bottom - pins below the tray
        regions::bottom_tray::show(host, ui);
        regions::right_dock::show(host, ui);
        regions::canvas_stage::show(host, ui); // CentralPanel - fills the rest
        command_palette::overlay(host, ui); // Area on top if modal == CommandPalette

        // All region borrows dropped. Apply intents in push order.
        for intent in host.intents.drain() {
            apply_intent(host, intent, ui.ctx());
        }
    }
}
```
Note on contract: the spec's runtime reads `host.intents.0.clear()` and `std::mem::take(&mut host.intents.0)`, touching the private `Vec`. This layer does not own `IntentSink`, so it calls the public surface instead. The STATE layer's `IntentSink` MUST expose `pub fn clear(&mut self)` and `pub fn drain(&mut self) -> impl Iterator<Item = Intent> + '_` (or `-> Vec<Intent>`). If the STATE layer shipped only `push`, add `clear`/`drain` there - do not reach into the private field from `shell`. If `drain` returns a borrowing iterator, collect it into a `Vec` first so the `&mut host.intents` borrow drops before the `apply_intent(host, ...)` loop: `let intents: Vec<_> = host.intents.drain().collect();`.

- [ ] **Step 5: Create `crates/ui/src/shell/mod.rs`** wiring the submodules and re-exporting the two public entry points.
```rust
//! The application shell: per-frame region composition, the command palette,
//! shortcut routing, and the menu structure (architecture bible section 8).
//!
//! `Shell::run` is called from `App::ui`; `drain_background` from `App::logic`.

pub mod command_palette;
pub mod menus;
pub mod regions;
pub mod runtime;
pub mod shortcuts;

pub use runtime::Shell;

use crate::state::Host;

/// Drain background-channel results into session state, called from `App::logic`.
///
/// This is the single mpsc-drain front door (spec "Region composition and the
/// shell runtime"). This round it is a structured no-op: an empty `try_recv` loop
/// with no sender, plus the one `JobStub` path that flips `ai_status`
/// Working -> Ready to prove the channel path lives (bible rule 5). It runs in
/// `logic`, not `ui`, because `logic` runs even when the window is occluded but a
/// repaint was requested.
pub fn drain_background(host: &mut Host, ctx: &egui::Context) {
    if host.drain_background_once(ctx) {
        ctx.request_repaint();
    }
}
```
Note on contract: `drain_background` defers the actual channel-draining to a `Host` method `drain_background_once(&mut self, ctx: &egui::Context) -> bool` owned by the STATE layer (it owns `Host.bg: BackgroundChannel`, `session.jobs`, and `ai_status`; `shell` only knows when to request a repaint). The method returns `true` if anything landed. If the STATE layer named this differently, use that name - the contract is "STATE owns the drain; SHELL owns the repaint trigger." If STATE did not provide a method, implement the no-op drain inline here against `host.bg.try_recv()`, returning whether a `JobStub` flipped `ai_status`.

- [ ] **Step 6: Add the module to `lib.rs`.** Read the current `lib.rs`, then add `pub mod shell;` alongside the other `pub mod` lines (THEME/STATE/etc. layers add theirs too). Leave `CanvasCallback`, `install_canvas_renderer`, and their test untouched.

- [ ] **Step 7: Build the crate.** Run in PowerShell:
```
cargo build -p pixhaus-ui
```
Expected: PASS (compiles). If it fails on a missing `IntentSink::clear`/`drain` or `Host::drain_background_once`, that is the contract gap flagged in Steps 4-5 - coordinate with the STATE layer rather than reaching into private fields. No new clippy warnings are expected (stubs are trivial).

- [ ] **Step 8: Run clippy on the crate.**
```
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```
Expected: PASS (no warnings).

- [ ] **Step 9: Commit.**
```
git add crates/ui/src/shell crates/ui/src/lib.rs
git commit -m @'
feat(ui): scaffold shell runtime and borrow-safe per-frame loop

Add the shell module tree with the verified Shell::run region order
(outer panels first, CentralPanel last, palette Area after) and the
post-loop intent drain. Region and tooling bodies follow; this lands the
load-bearing runtime and stubs so it compiles and reviews on its own.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### SHELL.2: the bare-ContribCtx and PanelScope split helpers

The right-dock, bottom-tray, and tool-options regions all need the reborrow-then-destructure of `&mut *host` into disjoint field bindings. The right dock and tray also build a `PanelScope` per panel; tool-options builds a bare `ContribCtx`. Factoring the per-panel `PanelScope` construction into one helper keeps the three regions identical where the spec says they are, and keeps the adversarially-verified borrow shape in one place. This is pure plumbing with nothing to assert beyond "it compiles", so the rhythm is write -> build -> commit.

**Files:**
- Create: `crates/ui/src/shell/regions/scope_split.rs`
- Modify: `crates/ui/src/shell/regions/mod.rs` (add `pub(crate) mod scope_split;`)

- [ ] **Step 1: Create `crates/ui/src/shell/regions/scope_split.rs`.** This holds no borrow itself - it documents the exact field bindings the regions destructure, and provides the per-panel `PanelScope` builder the dock and tray share. The builder takes already-split `&` / `&mut` references (so the caller owns the `let Host { .. } = &mut *host;` destructure and the disjointness is visible at the call site, exactly as the spec shows).
```rust
//! Shared field-split helpers for the registry-fed regions.
//!
//! The right dock, bottom tray, and tool-options regions all reborrow-then-
//! destructure `&mut *host` into disjoint field bindings before entering a
//! `show_inside` closure (spec "The borrow-safe per-frame loop"). The closure must
//! NEVER capture `host` whole. This module factors the per-panel `PanelScope`
//! construction the dock and tray share, taking the already-split references so the
//! disjointness stays visible at each call site.

use crate::contrib_api::context::{ContribCtx, PanelScope};
use crate::contrib_api::ids::PanelId;
use crate::state::{IntentSink, SessionState, UiState};
use crate::theme::Theme;

/// Build a `PanelScope` for one panel from the disjoint field bindings.
///
/// `session`/`ui_state`/`theme` are shared borrows of sibling `Host` fields;
/// `intents` is reborrowed per panel; `scratch` is this panel's own buffer. The
/// caller has already destructured `&mut *host`, so these are provably disjoint.
pub(crate) fn panel_scope<'a>(
    session: &'a SessionState,
    ui_state: &'a UiState,
    theme: &'a Theme,
    intents: &'a mut IntentSink,
    id: PanelId,
    scratch: &'a mut String,
) -> PanelScope<'a> {
    PanelScope {
        ctx: ContribCtx {
            session,
            ui_state,
            theme,
            intents,
        },
        id,
        scratch,
    }
}
```

- [ ] **Step 2: Register the module.** In `crates/ui/src/shell/regions/mod.rs`, add:
```rust
pub(crate) mod scope_split;
```

- [ ] **Step 3: Build.**
```
cargo build -p pixhaus-ui
```
Expected: PASS. If `ContribCtx`/`PanelScope`/`SessionState`/`UiState`/`Theme`/`IntentSink`/`PanelId` paths differ from the spec, fix the `use` paths to match the actual CONTRIB-API/STATE/THEME layer module layout - the type names are the contract, the module paths may need adjusting.

- [ ] **Step 4: Commit.**
```
git add crates/ui/src/shell/regions/scope_split.rs crates/ui/src/shell/regions/mod.rs
git commit -m @'
feat(ui): add shared panel-scope split helper for shell regions

Factor the per-panel PanelScope construction the right dock and bottom
tray share, taking the already-split disjoint field bindings so the
borrow-safety stays visible at each call site.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### SHELL.3: the menu structure data (menus.rs)

The top-bar menu groups, as data. The shell owns the always-present groups (Pixhaus/File/Edit/View/Window/Help); Sprite/Layer/Frame/Select are contributed by modules and merged into `Registries.menus`. A few items are live now: `View > Theme > Dark/Light/Accent`, `View > Toggle Grid`, `Window > Command Palette`. This is pure data; the rhythm is write a small assertion of the structure -> implement -> commit.

**Files:**
- Modify: `crates/ui/src/shell/menus.rs`
- Test: `crates/ui/src/shell/menus.rs` (`#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing test first.** Replace the stub body's test region (append at end of `menus.rs`). The test asserts the shell owns the always-present groups in order and that the live items carry the actions that map to live `Intent`s. The `MenuGroup`/`MenuItem` types come from the CONTRIB-API layer (`contrib_api::module`).
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_groups_in_order() {
        let groups = shell_menu_groups();
        let labels: Vec<&str> = groups.iter().map(|g| g.label).collect();
        // The shell owns these; Sprite/Layer/Frame/Select are module-contributed.
        assert_eq!(labels, vec!["Pixhaus", "File", "Edit", "View", "Window", "Help"]);
    }

    #[test]
    fn view_menu_has_live_theme_and_grid_items() {
        let groups = shell_menu_groups();
        let view = groups.iter().find(|g| g.label == "View").expect("View group");
        let item_labels: Vec<&str> = view.items.iter().map(|i| i.label).collect();
        assert!(item_labels.contains(&"Toggle Grid"));
        assert!(item_labels.iter().any(|l| l.starts_with("Theme")));
    }

    #[test]
    fn window_menu_exposes_command_palette() {
        let groups = shell_menu_groups();
        let window = groups.iter().find(|g| g.label == "Window").expect("Window group");
        assert!(window.items.iter().any(|i| i.label.contains("Command Palette")));
    }
}
```

- [ ] **Step 2: Run the test (expect FAIL - the stub returns an empty Vec).**
```
cargo nextest run -p pixhaus-ui shell::menus
```
Expected: FAIL (`shell_groups_in_order` panics on `assert_eq` against an empty Vec).

- [ ] **Step 3: Implement `menus.rs`.** Replace the stub `shell_menu_groups` body. Most items are inert placeholders that emit `Intent::RunAction` (a mock toast); the View/Window live items carry actions the menu-render code (SHELL.4) special-cases - but the data layer only declares them with stable `ActionId`s. Verify the `MenuGroup`/`MenuItem` field names against the CONTRIB-API layer; the spec defines `MenuGroup { label, items: Vec<MenuItem { label, shortcut, action: ActionId }> }`.
```rust
//! The shell's always-present top-bar menu groups, as data.
//!
//! The shell owns Pixhaus/File/Edit/View/Window/Help; modules contribute
//! Sprite/Layer/Frame/Select into `Registries.menus`. Most items emit a mock
//! `Intent::RunAction`; the View/Window items named below are wired live in the
//! top-bar render (SHELL.4): View > Theme, View > Toggle Grid, Window > Command
//! Palette.

use crate::contrib_api::ids::ActionId;
use crate::contrib_api::module::{MenuGroup, MenuItem};

// Stable action ids the top-bar render special-cases. Inert items carry their own
// "<group>.<verb>" ids and route to the mock RunAction toast.
/// `View > Theme` submenu root; the render expands it to Dark/Light/Accent.
pub const ACTION_VIEW_THEME: ActionId = ActionId("view.theme");
/// `View > Toggle Grid`; the render maps it to `Intent::SetGrid`.
pub const ACTION_VIEW_TOGGLE_GRID: ActionId = ActionId("view.toggle_grid");
/// `Window > Command Palette`; the render maps it to `Intent::OpenCommandPalette`.
pub const ACTION_WINDOW_COMMAND_PALETTE: ActionId = ActionId("window.command_palette");

fn item(label: &'static str, action: ActionId) -> MenuItem {
    MenuItem {
        label,
        shortcut: None,
        action,
    }
}

/// The menu groups the shell owns, in display order.
pub fn shell_menu_groups() -> Vec<MenuGroup> {
    vec![
        MenuGroup {
            label: "Pixhaus",
            items: vec![
                item("About Pixhaus", ActionId("pixhaus.about")),
                item("Preferences", ActionId("pixhaus.preferences")),
            ],
        },
        MenuGroup {
            label: "File",
            items: vec![
                item("New", ActionId("file.new")),
                item("Open", ActionId("file.open")),
                item("Save", ActionId("file.save")),
                item("Export", ActionId("file.export")),
            ],
        },
        MenuGroup {
            label: "Edit",
            items: vec![
                item("Undo", ActionId("edit.undo")),
                item("Redo", ActionId("edit.redo")),
                item("Cut", ActionId("edit.cut")),
                item("Copy", ActionId("edit.copy")),
                item("Paste", ActionId("edit.paste")),
            ],
        },
        MenuGroup {
            label: "View",
            items: vec![
                item("Theme", ACTION_VIEW_THEME),
                item("Toggle Grid", ACTION_VIEW_TOGGLE_GRID),
                item("Zoom In", ActionId("view.zoom_in")),
                item("Zoom Out", ActionId("view.zoom_out")),
            ],
        },
        MenuGroup {
            label: "Window",
            items: vec![
                item("Command Palette", ACTION_WINDOW_COMMAND_PALETTE),
                item("Reset Layout", ActionId("window.reset_layout")),
            ],
        },
        MenuGroup {
            label: "Help",
            items: vec![
                item("Documentation", ActionId("help.docs")),
                item("Keyboard Shortcuts", ActionId("help.shortcuts")),
            ],
        },
    ]
}
```
If `MenuItem.shortcut` is typed `Option<egui::KeyboardShortcut>` (per spec), `None` is correct here; the live shortcuts are owned by `shortcuts.rs`, not duplicated in the menu data.

- [ ] **Step 4: Run the tests (expect PASS).**
```
cargo nextest run -p pixhaus-ui shell::menus
```
Expected: PASS (3 tests).

- [ ] **Step 5: Commit.**
```
git add crates/ui/src/shell/menus.rs
git commit -m @'
feat(ui): add the shell top-bar menu structure as data

Declare the always-present menu groups (Pixhaus/File/Edit/View/Window/
Help) with stable action ids. View > Theme, View > Toggle Grid, and
Window > Command Palette carry the ids the top-bar render wires live;
the rest route to the mock RunAction toast.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### SHELL.4: top-bar region (menus + workspace tabs + global status)

Three rows in one elevated frame: the menu strip (shell groups + module-contributed groups from `Registries.menus`), the workspace tab strip, and a thin global-status strip. The menu strip uses the verified 0.34 idiom: `ui.horizontal` + `ui.menu_button` + `ui.close()` (no `egui::menu::bar`). Live items (`View > Theme`, `View > Toggle Grid`, `Window > Command Palette`) map to their real intents; everything else emits `Intent::RunAction`.

**Files:**
- Modify: `crates/ui/src/shell/regions/top_bar.rs`

- [ ] **Step 1: Implement the top bar.** Replace the stub. The split is the reborrow-then-destructure pattern; the menu closure pushes intents through the destructured `&mut intents`, never `host`. Confirm `region::region_id::TOP_BAR`, `theme.surface(SurfaceTier::Elevated)`, `widgets::workspace_tab`, and the menu live-item action ids against the actual layers.
```rust
//! Top bar region: the menu strip, the workspace tab strip, and a thin global
//! status strip, in one elevated frame.

use crate::contrib_api::ids::ActionId;
use crate::region::region_id;
use crate::shell::menus::{
    ACTION_VIEW_THEME, ACTION_VIEW_TOGGLE_GRID, ACTION_WINDOW_COMMAND_PALETTE,
};
use crate::state::intent::Intent;
use crate::state::ui_state::GridMode;
use crate::state::Host;
use crate::theme::tokens::{SurfaceTier, ThemeVariant};
use crate::widgets;

/// Render the top-bar region.
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    let Host {
        registries,
        state,
        intents,
        theme,
        ..
    } = &mut *host;

    let frame = egui::Frame::new()
        .fill(theme.surface(SurfaceTier::Elevated))
        .inner_margin(theme.spacing.sm);

    egui::Panel::top(region_id::TOP_BAR)
        .frame(frame)
        .show_inside(ui, |ui| {
            // Row 1: menu strip. Shell groups, then module-contributed groups.
            ui.horizontal(|ui| {
                for group in &registries.menus {
                    ui.menu_button(group.label, |ui| {
                        for item in &group.items {
                            if item.action == ACTION_VIEW_THEME {
                                ui.menu_button("Theme", |ui| {
                                    for (label, variant) in [
                                        ("Dark", ThemeVariant::Dark),
                                        ("Light", ThemeVariant::Light),
                                        ("Accent", ThemeVariant::AccentHighContrast),
                                    ] {
                                        if ui.button(label).clicked() {
                                            intents.push(Intent::SetThemeVariant(variant));
                                            ui.close();
                                        }
                                    }
                                });
                            } else if ui.button(item.label).clicked() {
                                push_menu_intent(intents, item.action);
                                ui.close();
                            }
                        }
                    });
                }
            });

            ui.add_space(theme.spacing.xs);

            // Row 2: workspace tab strip. Active = accent pill + underline.
            ui.horizontal(|ui| {
                let active = state.session.active_workspace;
                for ws in registries.workspaces.iter() {
                    let meta = ws.meta();
                    let id = ws.id();
                    if widgets::workspace_tab(ui, theme, meta.name, meta.icon, id == active)
                        .clicked()
                    {
                        intents.push(Intent::SelectWorkspace(id));
                    }
                }
            });

            ui.add_space(theme.spacing.xs);

            // Row 3: a thin global-status strip.
            ui.horizontal(|ui| {
                ui.colored_label(
                    theme.roles.text_secondary,
                    if state.session.dirty {
                        "Unsaved changes"
                    } else {
                        "Saved"
                    },
                );
            });
        });
}

fn push_menu_intent(intents: &mut crate::state::IntentSink, action: ActionId) {
    match action {
        ACTION_VIEW_TOGGLE_GRID => intents.push(Intent::SetGrid(GridMode::Pixel)),
        ACTION_WINDOW_COMMAND_PALETTE => intents.push(Intent::OpenCommandPalette),
        other => intents.push(Intent::RunAction(other)),
    }
}
```
Notes: `GridMode::Pixel` is a placeholder - use whatever the STATE layer's `GridMode` non-off variant is named (the spec lists `GridMode` with `SetGrid(GridMode)` but does not enumerate variants here; the THEME/STATE layers own it). If `GridMode` has a `toggle`-friendly value, `Toggle Grid` may instead emit `Intent::SetGrid(next)` computed from `state.ui.grid`; keep it a single intent. `widgets::workspace_tab(ui, theme, name, icon, selected) -> egui::Response` is the WIDGETS-layer signature; match it exactly.

- [ ] **Step 2: Build and clippy.**
```
cargo build -p pixhaus-ui
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```
Expected: PASS both. If `widgets::workspace_tab`'s signature differs, adjust the call to match - the WIDGETS layer is the authority on widget signatures.

- [ ] **Step 3: Commit.** (No unit test - this is egui render code; the snapshot/smoke tests in the REGISTRY and TEST layers cover the resolved tab set. Manual-verify is noted in the spec test plan.)
```
git add crates/ui/src/shell/regions/top_bar.rs
git commit -m @'
feat(ui): render the top-bar region (menus, tabs, status)

Menu strip via ui.menu_button + ui.close() (verified 0.34 idiom), the
workspace tab strip with active accent state, and a thin global-status
row, in one elevated frame. View > Theme/Toggle Grid and Window >
Command Palette map to live intents; other items emit a mock RunAction.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### SHELL.5: tool-options region (the bare-ContribCtx split)

The active tool's `options_ui` rendered into a thin top panel below the menu bar. Uses the same reborrow-then-destructure split as the dock, but builds a bare `ContribCtx` (no `PanelScope`: a tool has no scratch/id). Content swaps with the active tool.

**Files:**
- Modify: `crates/ui/src/shell/regions/tool_options.rs`

- [ ] **Step 1: Implement the tool-options region.** Replace the stub.
```rust
//! Tool-options region: the active tool's `options_ui`, rendered with a bare
//! `ContribCtx` (a tool is not a panel - no scratch, no PanelId).

use crate::contrib_api::context::ContribCtx;
use crate::region::region_id;
use crate::state::Host;
use crate::theme::tokens::SurfaceTier;

/// Render the active tool's options bar.
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    let Host {
        registries,
        state,
        intents,
        theme,
        ..
    } = &mut *host;

    let frame = egui::Frame::new()
        .fill(theme.surface(SurfaceTier::Elevated))
        .inner_margin(theme.spacing.sm);

    egui::Panel::top(region_id::TOOL_OPTIONS)
        .frame(frame)
        .show_inside(ui, |ui| {
            let Some(tool) = registries.tools.get(state.session.active_tool) else {
                return;
            };
            let mut cx = ContribCtx {
                session: &state.session,
                ui_state: &state.ui,
                theme,
                intents,
            };
            tool.options_ui(ui, &mut cx);
        });
}
```
Note: `&mut cx` matches `Tool::options_ui(&self, ui, cx: &mut ContribCtx<'_>)` per spec. `intents` here is the destructured `&mut IntentSink`; passing it into the struct field moves the reference into `cx`, which is fine because the closure does not use `intents` again afterward. If the borrow checker complains, reborrow with `intents: &mut *intents` in the struct literal.

- [ ] **Step 2: Build and clippy.**
```
cargo build -p pixhaus-ui
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```
Expected: PASS both.

- [ ] **Step 3: Commit.**
```
git add crates/ui/src/shell/regions/tool_options.rs
git commit -m @'
feat(ui): render the tool-options region

Render the active tool's options_ui through the reborrow-then-destructure
split with a bare ContribCtx (a tool has no scratch or PanelId). Content
swaps with the active tool.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### SHELL.6: left-rail region (tool buttons, active accent, AI sparkle)

A fixed 48px left panel. Iterates the resolved layout's `primary_tools`, paints each via `widgets::tool_button` (active = `accent.muted` bg + 2px `accent.base` left line + tooltip; AI Brush = `accent.ai` + sparkle). Click -> `Intent::SelectTool`.

**Files:**
- Modify: `crates/ui/src/shell/regions/left_rail.rs`

- [ ] **Step 1: Implement the left rail.** Replace the stub. Resolve the tool ids first (the `&registries`/`&state` borrow ends at the `;`), then destructure.
```rust
//! Left-rail region: the tool rail. Tools come from the active workspace's
//! resolved layout; the active tool gets the accent tint + left line, AI tools the
//! sparkle.

use crate::region::region_id;
use crate::registry::resolve::resolve_layout;
use crate::state::intent::Intent;
use crate::state::Host;
use crate::theme::tokens::SurfaceTier;
use crate::widgets;

/// Render the left tool rail.
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    // Resolve tool ids by value first; the &registries/&state borrows end here.
    let tool_ids = resolve_layout(host.state.session.active_workspace, &host.registries)
        .primary_tools;

    let Host {
        registries,
        state,
        intents,
        theme,
        ..
    } = &mut *host;

    let frame = egui::Frame::new().fill(theme.surface(SurfaceTier::Panel));
    let active = state.session.active_tool;

    egui::Panel::left(region_id::LEFT_RAIL)
        .resizable(false)
        .exact_size(48.0)
        .frame(frame)
        .show_inside(ui, |ui| {
            ui.vertical_centered(|ui| {
                for id in tool_ids {
                    let Some(tool) = registries.tools.get(id) else {
                        continue;
                    };
                    let meta = tool.meta();
                    if widgets::tool_button(ui, theme, &meta, id == active).clicked() {
                        intents.push(Intent::SelectTool(id));
                    }
                }
            });
        });
}
```
Notes: `widgets::tool_button(ui, theme, &meta, active: bool) -> egui::Response` is the WIDGETS-layer signature; the tooltip text `"{label} ({shortcut})\n{tooltip}"` and the AI sparkle/accent treatment live inside `tool_button` (it reads `meta.is_ai`, `meta.shortcut`, `meta.tooltip`). Match the exact signature. `ToolMeta` is the CONTRIB-API type.

- [ ] **Step 2: Build and clippy.**
```
cargo build -p pixhaus-ui
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```
Expected: PASS both.

- [ ] **Step 3: Commit.**
```
git add crates/ui/src/shell/regions/left_rail.rs
git commit -m @'
feat(ui): render the left tool rail

Fixed 48px rail filled from the active workspace's resolved primary_tools.
Each tool paints via widgets::tool_button (active accent tint + left line,
AI sparkle); click emits Intent::SelectTool.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### SHELL.7: right-dock region (the load-bearing borrow loop)

The card stack. This is the loop the adversarial verifier scrutinized - copy it verbatim from the spec ("shell/regions/right_dock.rs"): resolve ids by value first, reborrow-then-destructure into disjoint field bindings, `push_id(panel.id())` per panel, reborrow `&mut intents`/`scratch.entry(id)` per iteration. Provably disjoint, no `RefCell`, no `mem::take` of the registry.

**Files:**
- Modify: `crates/ui/src/shell/regions/right_dock.rs`

- [ ] **Step 1: Implement the right dock.** Replace the stub. Copy the spec's loop exactly, using the `scope_split::panel_scope` helper from SHELL.2 for the per-panel `PanelScope`.
```rust
//! Right-dock region: the panel card stack. The load-bearing borrow loop
//! (spec "The borrow-safe per-frame loop"): resolve ids by value first, reborrow-
//! then-destructure into disjoint field bindings, push_id per panel, reborrow the
//! mutable channels each iteration. Provably disjoint; no RefCell, no mem::take.

use crate::region::region_id;
use crate::registry::resolve::resolve_layout;
use crate::shell::regions::scope_split::panel_scope;
use crate::state::Host;
use crate::widgets;

/// Render the right-dock card stack.
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    // 1. Resolve ids by value FIRST - the &registries/&state borrows end here.
    let ids = resolve_layout(host.state.session.active_workspace, &host.registries).right_dock;

    // 2. Reborrow-then-destructure into disjoint field bindings. Must be `&mut *host`,
    //    not `host` - a by-value field pattern on `&mut Host` is move-out-of-borrow (E0507).
    let Host {
        registries,
        state,
        intents,
        scratch,
        theme,
        ..
    } = &mut *host;

    egui::Panel::right(region_id::RIGHT_DOCK)
        .resizable(true)
        .default_size(state.ui.right_dock_width) // 0.34 Panel API: default_size, not default_width
        .show_inside(ui, |ui| {
            for id in ids {
                let Some(panel) = registries.panels.get(id) else {
                    continue;
                };
                let meta = panel.meta();
                let collapsed = state
                    .ui
                    .collapsed
                    .get(&id)
                    .copied()
                    .unwrap_or(!meta.default_open);
                // The SHELL scopes ids - not the panel. Distinct call site per PanelId.
                ui.push_id(id, |ui| {
                    widgets::card(ui, theme, &meta, collapsed, |ui| {
                        let buf = scratch.entry(id).or_default(); // &mut String for this panel only
                        let mut scope = panel_scope(
                            &state.session,
                            &state.ui,
                            theme,
                            &mut *intents, // reborrowed, not moved
                            id,
                            buf,
                        );
                        panel.ui(ui, &mut scope);
                    });
                });
            }
        });
}
```
Notes: `widgets::card(ui, theme, &meta, collapsed, body) -> egui::Response` is the WIDGETS signature (spec: `panel_card(ui, theme, meta, collapsed, body) -> response`; the WIDGETS layer may export it as `card`). `scratch.entry(id).or_default()` requires `Host.scratch: HashMap<PanelId, String>` (STATE layer). `state.ui.right_dock_width` and `state.ui.collapsed` are STATE fields. If `card` borrows `theme` by value (it is `Copy`-ish? - `Theme` is not `Copy` per spec), pass `theme` as `&Theme`; match the WIDGETS signature.

- [ ] **Step 2: Build and clippy - this is the borrow-check gate.**
```
cargo build -p pixhaus-ui
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```
Expected: PASS both. If you hit E0502/E0499 (aliasing), the cause is almost always the closure capturing `host` instead of the destructured bindings, or `theme`/`intents` not reborrowed - re-check against the spec's exact code; do NOT introduce `RefCell` or `mem::take`. If E0507 (move out of borrow), confirm the destructure target is `&mut *host`.

- [ ] **Step 3: Commit.**
```
git add crates/ui/src/shell/regions/right_dock.rs
git commit -m @'
feat(ui): render the right-dock card stack (borrow-safe loop)

The verified per-frame loop: resolve panel ids by value, reborrow-then-
destructure host into disjoint field bindings, push_id per panel, reborrow
the intent sink and scratch buffer each iteration. No RefCell, no take.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### SHELL.8: bottom-tray region (tab row + selected tray panel)

A resizable bottom panel: a tab row built from the resolved `bottom_tray` Vec (selected = `tray_tab[active_ws]` or the first tab; click -> `Intent::SelectTrayTab`), then the selected tray panel rendered through the same disjoint-field + `push_id` path as the dock. Tabs and content both swap per workspace.

**Files:**
- Modify: `crates/ui/src/shell/regions/bottom_tray.rs`

- [ ] **Step 1: Implement the bottom tray.** Replace the stub. Resolve the tray Vec and the selected tab id by value first, then destructure.
```rust
//! Bottom-tray region: a tab row (selectable chips, active = accent pill) plus the
//! selected tray panel, rendered through the same disjoint-field + push_id path as
//! the right dock. Both the tabs and the content swap per workspace.

use crate::region::region_id;
use crate::registry::resolve::resolve_layout;
use crate::shell::regions::scope_split::panel_scope;
use crate::state::intent::Intent;
use crate::state::Host;
use crate::theme::tokens::SurfaceTier;
use crate::widgets;

/// Render the bottom tray (tab row + selected panel).
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    let active_ws = host.state.session.active_workspace;
    let tray = resolve_layout(active_ws, &host.registries).bottom_tray;
    if tray.is_empty() {
        return;
    }
    // Selected tab: the per-workspace stored tab if still present, else the first.
    let selected = host
        .state
        .ui
        .tray_tab
        .get(&active_ws)
        .copied()
        .filter(|p| tray.contains(p))
        .unwrap_or(tray[0]);

    let Host {
        registries,
        state,
        intents,
        scratch,
        theme,
        ..
    } = &mut *host;

    let frame = egui::Frame::new().fill(theme.surface(SurfaceTier::Panel));

    egui::Panel::bottom(region_id::BOTTOM_TRAY)
        .resizable(true)
        .default_size(state.ui.bottom_tray_height)
        .frame(frame)
        .show_inside(ui, |ui| {
            // Tab row.
            ui.horizontal(|ui| {
                for &id in &tray {
                    let Some(panel) = registries.panels.get(id) else {
                        continue;
                    };
                    let meta = panel.meta();
                    if widgets::tray_tab(ui, theme, meta.title, meta.icon, id == selected)
                        .clicked()
                    {
                        intents.push(Intent::SelectTrayTab(id));
                    }
                }
            });
            ui.separator();

            // Selected tray panel, via the disjoint-field + push_id path.
            if let Some(panel) = registries.panels.get(selected) {
                ui.push_id(selected, |ui| {
                    let buf = scratch.entry(selected).or_default();
                    let mut scope = panel_scope(
                        &state.session,
                        &state.ui,
                        theme,
                        &mut *intents,
                        selected,
                        buf,
                    );
                    panel.ui(ui, &mut scope);
                });
            }
        });
}
```
Notes: `widgets::tray_tab(ui, theme, title, icon, selected) -> egui::Response` is the WIDGETS signature (spec `tray_tab.rs`: "the tray tab chip, active = accent pill"). `PanelMeta.title` is `&'static str` (spec). `state.ui.tray_tab: HashMap<WorkspaceId, PanelId>` and `state.ui.bottom_tray_height: f32` are STATE fields.

- [ ] **Step 2: Build and clippy.**
```
cargo build -p pixhaus-ui
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```
Expected: PASS both.

- [ ] **Step 3: Commit.**
```
git add crates/ui/src/shell/regions/bottom_tray.rs
git commit -m @'
feat(ui): render the bottom tray (tab row + selected panel)

Tab row from the resolved bottom_tray Vec (active = accent pill, click
emits SelectTrayTab), then the selected tray panel through the same
disjoint-field + push_id path as the right dock. Both swap per workspace.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### SHELL.9: status-bar region (status items + AI dot)

A 22px bottom panel declared before the tray so it pins to the lower edge. Renders always-on items (size, zoom, grid) + the workspace's `status_items` + the AI status dot colored from `session.ai_status`.

**Files:**
- Modify: `crates/ui/src/shell/regions/status_bar.rs`

- [ ] **Step 1: Implement the status bar.** Replace the stub. Resolve `status_items` by value first.
```rust
//! Status-bar region: a compact strip. Always-on size/zoom/grid, then the
//! workspace's status items, then the AI status dot colored from session.ai_status.

use crate::region::region_id;
use crate::registry::resolve::resolve_layout;
use crate::state::session::AiStatus;
use crate::state::Host;
use crate::theme::tokens::SurfaceTier;

/// Render the status bar.
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    let status_items = resolve_layout(host.state.session.active_workspace, &host.registries)
        .status_items;

    let Host { state, theme, .. } = &mut *host;

    let frame = egui::Frame::new()
        .fill(theme.surface(SurfaceTier::Elevated))
        .inner_margin(theme.spacing.xs);

    egui::Panel::bottom(region_id::STATUS_BAR)
        .resizable(false)
        .exact_size(22.0)
        .frame(frame)
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                // Always-on items.
                ui.colored_label(theme.roles.text_secondary, "64 x 64");
                ui.separator();
                ui.colored_label(
                    theme.roles.text_secondary,
                    format!("{:.0}%", state.ui.zoom * 100.0),
                );
                ui.separator();
                ui.colored_label(theme.roles.text_secondary, format!("Grid {:?}", state.ui.grid));

                // Workspace-specific items.
                for item in &status_items {
                    ui.separator();
                    ui.label(
                        egui::RichText::new(format!("{} {}", item.icon, item.text))
                            .color(theme.roles.text_secondary),
                    );
                }

                // AI status dot, right-aligned.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (color, text) = match state.session.ai_status {
                        AiStatus::Ready => (theme.roles.success, "AI Ready"),
                        AiStatus::Working => (theme.roles.warning, "AI Working"),
                        AiStatus::Offline => (theme.roles.text_disabled, "AI Offline"),
                    };
                    ui.colored_label(theme.roles.text_secondary, text);
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 4.0, color);
                });
            });
        });
}
```
Notes: `AiStatus { Ready, Working, Offline }` is the STATE-layer enum (spec: `AiStatus` -> `success`=Ready, `warning`=Working, `text_disabled`=Offline). `StatusItem { icon: char, text: String }` per spec. `state.ui.zoom`/`state.ui.grid` are STATE fields. The status bar reads-only, so it does not need `intents` - destructure only `state` and `theme`.

- [ ] **Step 2: Build and clippy.**
```
cargo build -p pixhaus-ui
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```
Expected: PASS both.

- [ ] **Step 3: Commit.**
```
git add crates/ui/src/shell/regions/status_bar.rs
git commit -m @'
feat(ui): render the status bar (items + AI dot)

A 22px strip pinned below the tray: always-on size/zoom/grid, the
workspace's status items, and an AI status dot colored from
session.ai_status (success/warning/disabled).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### SHELL.10: canvas-stage region (framed artboard, checker, shadow, CanvasCallback, grid, HUD)

The `CentralPanel`, added last. Fills with `surfaces.stage`, computes the artboard rect from `UiState.zoom`/`pan` (mock 64x64), paints the checkerboard, a manual drop shadow (offset translucent rect - `Shadow` is not a paint primitive), embeds the EXISTING `pixhaus_ui::CanvasCallback` unchanged via `allocate_painter` + `egui_wgpu::Callback::new_paint_callback`, then grid strokes and the floating HUD via the central panel's `Painter`.

**Files:**
- Modify: `crates/ui/src/shell/regions/canvas_stage.rs`

- [ ] **Step 1: Implement the canvas stage.** Replace the stub. This preserves the seam from `app/src/main.rs` exactly - `egui_wgpu::Callback::new_paint_callback(resp.rect, crate::CanvasCallback)`, the unit struct unchanged.
```rust
//! Canvas-stage region: the CentralPanel (added last). A stage backdrop, the
//! mock artboard rect from zoom/pan, the transparency checkerboard, a manually
//! painted drop shadow, the unchanged CanvasCallback embed, grid strokes, and a
//! floating HUD painted with the central panel's Painter.

use crate::region::region_id;
use crate::state::ui_state::GridMode;
use crate::state::Host;
use crate::theme::tokens::SurfaceTier;
use crate::CanvasCallback;

/// Render the canvas stage.
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    let Host { state, theme, .. } = &mut *host;

    let frame = egui::Frame::new().fill(theme.surface(SurfaceTier::Stage));

    egui::CentralPanel::default()
        .frame(frame)
        .show_inside(ui, |ui| {
            let stage_rect = ui.available_rect_before_wrap();
            let painter = ui.painter().clone();

            // 1. Stage backdrop already filled by the frame. Compute the artboard.
            let sprite_px = egui::vec2(64.0, 64.0);
            let scaled = sprite_px * state.ui.zoom;
            let artboard = egui::Rect::from_center_size(stage_rect.center() + state.ui.pan, scaled);

            // 2. Manual drop shadow: an offset translucent dark rect behind the board.
            //    Shadow is not a paint primitive and cannot be painter.add-ed here.
            let shadow_rect = artboard.translate(egui::vec2(4.0, 6.0));
            painter.rect_filled(
                shadow_rect,
                egui::CornerRadius::ZERO,
                egui::Color32::from_black_alpha(110),
            );

            // 3. Transparency checkerboard behind the artboard.
            paint_checkerboard(&painter, artboard, theme);

            // 4. Embed the renderer UNCHANGED - exactly the app/src/main.rs seam.
            let (resp, _cb_painter) =
                ui.allocate_painter(scaled, egui::Sense::click_and_drag());
            // Position the callback rect over the artboard regardless of layout flow.
            let cb_rect = artboard;
            let _ = resp;
            ui.painter().add(egui_wgpu::Callback::new_paint_callback(
                cb_rect,
                CanvasCallback,
            ));

            // 5. Grid lines over the artboard (minor 8px / major 16px per GridMode).
            paint_grid(&painter, artboard, state.ui.zoom, state.ui.grid, theme);

            // 6. Floating HUD via the central Painter, at the stage's lower-left.
            paint_hud(&painter, stage_rect, state.ui.zoom, theme);
        });
}

fn paint_checkerboard(painter: &egui::Painter, board: egui::Rect, theme: &crate::theme::Theme) {
    let cell = 8.0;
    let light = theme.surface(SurfaceTier::Inset);
    let dark = theme.surface(SurfaceTier::Stage);
    let cols = (board.width() / cell).ceil() as i32;
    let rows = (board.height() / cell).ceil() as i32;
    for r in 0..rows {
        for c in 0..cols {
            let on = (r + c) % 2 == 0;
            let min = board.min + egui::vec2(c as f32 * cell, r as f32 * cell);
            let rect = egui::Rect::from_min_size(min, egui::vec2(cell, cell)).intersect(board);
            painter.rect_filled(rect, egui::CornerRadius::ZERO, if on { light } else { dark });
        }
    }
}

fn paint_grid(
    painter: &egui::Painter,
    board: egui::Rect,
    zoom: f32,
    grid: GridMode,
    theme: &crate::theme::Theme,
) {
    if matches!(grid, GridMode::Off) {
        return;
    }
    let minor = 8.0 * zoom / 8.0; // mock: one device step per sprite pixel-block
    let step = minor.max(8.0);
    let stroke = egui::Stroke::new(1.0, theme.roles.border);
    let mut x = board.min.x;
    while x <= board.max.x {
        painter.line_segment(
            [egui::pos2(x, board.min.y), egui::pos2(x, board.max.y)],
            stroke,
        );
        x += step;
    }
    let mut y = board.min.y;
    while y <= board.max.y {
        painter.line_segment(
            [egui::pos2(board.min.x, y), egui::pos2(board.max.x, y)],
            stroke,
        );
        y += step;
    }
}

fn paint_hud(painter: &egui::Painter, stage: egui::Rect, zoom: f32, theme: &crate::theme::Theme) {
    let text = format!("64 x 64   {:.0}%   Grid 8px   Palette: Bit", zoom * 100.0);
    let font = egui::FontId::monospace(theme.type_scale.mono);
    let galley = painter.layout_no_wrap(text, font, theme.roles.text_secondary);
    let pad = egui::vec2(6.0, 4.0);
    let chip_min = stage.left_bottom() + egui::vec2(8.0, -(galley.size().y + pad.y * 2.0 + 8.0));
    let chip = egui::Rect::from_min_size(chip_min, galley.size() + pad * 2.0);
    painter.rect_filled(chip, egui::CornerRadius::same(2), theme.surface(SurfaceTier::Inset));
    painter.galley(chip.min + pad, galley, theme.roles.text_secondary);
}
```
Notes: this keeps `CanvasCallback` and `new_paint_callback` exactly as the existing `app/src/main.rs` seam (confirmed unchanged in `lib.rs`). `GridMode` must have an `Off` variant and at least one on variant (STATE/THEME layer owns it); adjust `matches!(grid, GridMode::Off)` and the `paint_grid` step math to the real variants (`Pixel`/`Tile`/etc.). `egui::Color32::from_black_alpha` and `painter.galley`/`layout_no_wrap` are confirmed 0.34 API. The grid/checker math is mock chrome (spec says manual-verify, not unit-tested) - keep it simple and bounded.

- [ ] **Step 2: Build and clippy.**
```
cargo build -p pixhaus-ui
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```
Expected: PASS both. If `GridMode` variant names differ, fix the `matches!` and the format string; if `from_black_alpha` is unavailable, use `egui::Color32::from_rgba_unmultiplied(0, 0, 0, 110)`.

- [ ] **Step 3: Commit.**
```
git add crates/ui/src/shell/regions/canvas_stage.rs
git commit -m @'
feat(ui): render the canvas stage (artboard, checker, grid, HUD)

The CentralPanel: stage backdrop, mock 64x64 artboard from zoom/pan, a
manually painted drop shadow, the unchanged CanvasCallback embed via
egui_wgpu::Callback::new_paint_callback, grid strokes, and a floating
HUD painted with the central Painter.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### SHELL.11: shortcuts - pure key->intent mapping + per-frame collect (spec test 6)

`shortcuts::collect` reads input once per frame with `consume_shortcut`/`consume_key` (so a focused `TextEdit` and the global handler never double-fire) and pushes intents. The decision logic is factored into a pure `map_key` function - synthetic `Key` + `Modifiers` + a `text_field_focused: bool` -> `Option<Intent>` - which is unit-testable without a frame. This is spec test 6: tool keys are suppressed when a text field is focused. TDD: write the failing test first.

**Files:**
- Modify: `crates/ui/src/shell/shortcuts.rs`
- Test: `crates/ui/src/shell/shortcuts.rs` (`#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing test first.** Append to `shortcuts.rs`. The pure fn is `map_key(key, mods, text_field_focused) -> Option<Intent>`; workspace shortcuts and Cmd+K need the modifier, tool keys are bare and gated on focus. `Intent` derives nothing comparable per spec, so assert via `matches!`.
```rust
#[cfg(test)]
mod tests {
    use super::map_key;
    use crate::contrib_api::ids::{ToolId, WorkspaceId};
    use crate::state::intent::Intent;

    fn cmd() -> egui::Modifiers {
        egui::Modifiers::COMMAND
    }

    #[test]
    fn cmd_1_selects_first_workspace() {
        let out = map_key(egui::Key::Num1, cmd(), false);
        assert!(matches!(out, Some(Intent::SelectWorkspace(_))));
    }

    #[test]
    fn cmd_k_opens_command_palette() {
        let out = map_key(egui::Key::K, cmd(), false);
        assert!(matches!(out, Some(Intent::OpenCommandPalette)));
    }

    #[test]
    fn bare_b_selects_pencil_tool() {
        let out = map_key(egui::Key::B, egui::Modifiers::NONE, false);
        assert!(matches!(out, Some(Intent::SelectTool(ToolId("pencil")))));
    }

    #[test]
    fn tool_key_suppressed_when_text_field_focused() {
        // Spec test 6: typing "b" in the prompt must NOT switch to Pencil.
        let out = map_key(egui::Key::B, egui::Modifiers::NONE, true);
        assert!(out.is_none());
    }

    #[test]
    fn workspace_shortcut_fires_even_when_text_focused() {
        // A command modifier shortcut is not a typed character, so it is not gated.
        let out = map_key(egui::Key::Num1, cmd(), true);
        assert!(matches!(out, Some(Intent::SelectWorkspace(WorkspaceId("draw")))));
    }

    #[test]
    fn bare_b_with_command_is_not_a_tool_key() {
        // Cmd+B is not the bare tool key; tool keys require no command modifier.
        let out = map_key(egui::Key::B, cmd(), false);
        assert!(out.is_none());
    }
}
```
Note: the asserted ids (`ToolId("pencil")`, `WorkspaceId("draw")`) are the canonical ids the sprite-edit module registers (spec "Per-workspace placement": Draw = Cmd+1; "Left-rail tools": Pencil = B). The MODULES layer owns those id strings; if they differ, this test must use the real strings - but the strings are part of the cross-layer contract (the workspace shortcut comes from `meta().shortcut`, the tool key from the tool's `meta().shortcut`).

- [ ] **Step 2: Run the test (expect FAIL - `map_key` does not exist).**
```
cargo nextest run -p pixhaus-ui shell::shortcuts
```
Expected: FAIL (compile error: `map_key` not found).

- [ ] **Step 3: Implement `shortcuts.rs`.** The pure `map_key` decides the intent; `collect` reads input once and consumes. Workspace shortcuts and Cmd+K come from the registries' authored shortcuts (so the mapping is not hardcoded twice); the tool key table is the canonical single-key set. The focus gate uses `ctx.text_edit_focused()` (confirmed in egui 0.34.2 - the precise "is a text edit focused" query; `wants_keyboard_input` is deprecated and `egui_wants_keyboard_input` is too broad). `consume_key` is the in-frame fallback that also pre-empts a focused widget.
```rust
//! Global shortcut collection.
//!
//! `collect` reads input once per frame with `consume_shortcut`/`consume_key` so a
//! focused `TextEdit` and the global handler never both fire. The decision logic is
//! the pure `map_key`, unit-tested without a frame (spec test 6): workspace
//! Cmd+1..5 and Cmd+K always map; bare tool keys are suppressed when a text field
//! is focused, so typing "b" in the prompt does not switch to Pencil.

use crate::contrib_api::ids::{ToolId, WorkspaceId};
use crate::registry::Registries;
use crate::state::intent::Intent;
use crate::state::IntentSink;

/// The canonical bare single-key tool shortcuts (spec "Left-rail tools").
const TOOL_KEYS: &[(egui::Key, ToolId)] = &[
    (egui::Key::B, ToolId("pencil")),
    (egui::Key::E, ToolId("eraser")),
    (egui::Key::G, ToolId("fill")),
    (egui::Key::L, ToolId("line")),
    (egui::Key::U, ToolId("rectangle")),
    (egui::Key::O, ToolId("ellipse")),
    (egui::Key::I, ToolId("eyedropper")),
    (egui::Key::M, ToolId("selection")),
    (egui::Key::Q, ToolId("lasso")),
    (egui::Key::V, ToolId("move")),
    (egui::Key::X, ToolId("text")),
    (egui::Key::H, ToolId("hand")),
    (egui::Key::Z, ToolId("zoom")),
    (egui::Key::J, ToolId("ai_brush")),
];

/// The workspace switch shortcuts (spec "Per-workspace placement"): Cmd+1..5.
const WORKSPACE_KEYS: &[(egui::Key, WorkspaceId)] = &[
    (egui::Key::Num1, WorkspaceId("draw")),
    (egui::Key::Num2, WorkspaceId("animate")),
    (egui::Key::Num3, WorkspaceId("tiles")),
    (egui::Key::Num4, WorkspaceId("generate")),
    (egui::Key::Num5, WorkspaceId("export")),
];

/// Pure decision: a key + modifiers + whether a text field is focused -> the intent.
///
/// Command-modifier shortcuts (workspace switch, palette) are not typed characters,
/// so they fire regardless of focus. Bare tool keys are typed characters, so they
/// are suppressed whenever a text field has focus.
pub fn map_key(key: egui::Key, mods: egui::Modifiers, text_field_focused: bool) -> Option<Intent> {
    if mods.command {
        if key == egui::Key::K {
            return Some(Intent::OpenCommandPalette);
        }
        if let Some((_, ws)) = WORKSPACE_KEYS.iter().find(|(k, _)| *k == key) {
            return Some(Intent::SelectWorkspace(*ws));
        }
        return None;
    }
    if text_field_focused {
        return None; // typing in a field: do not steal tool keys
    }
    TOOL_KEYS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, tool)| Intent::SelectTool(*tool))
}

/// Read input once this frame and push the resulting intents.
///
/// Consumes each matched key so a focused `TextEdit` and this handler do not both
/// fire. `_registries` is taken now so a later round can drive the mapping from the
/// workspaces' authored `meta().shortcut` instead of the constant table.
pub fn collect(ctx: &egui::Context, _registries: &Registries, intents: &mut IntentSink) {
    let text_field_focused = ctx.text_edit_focused();

    // Command-modifier shortcuts first (workspace switch + palette).
    for (key, _) in WORKSPACE_KEYS.iter().chain(std::iter::once(&(egui::Key::K, WorkspaceId("")))) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, *key)) {
            if let Some(intent) = map_key(*key, egui::Modifiers::COMMAND, text_field_focused) {
                intents.push(intent);
            }
        }
    }

    // Bare tool keys, gated on focus.
    if !text_field_focused {
        for (key, _) in TOOL_KEYS {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, *key)) {
                if let Some(intent) = map_key(*key, egui::Modifiers::NONE, text_field_focused) {
                    intents.push(intent);
                }
            }
        }
    }
}
```
Notes: the `WorkspaceId("")` sentinel in the chain only carries the `Key::K` consume; `map_key` turns Cmd+K into `OpenCommandPalette` regardless of the unused id. If that reads awkward in review, split into two loops (one over `WORKSPACE_KEYS`, one explicit `consume_key(COMMAND, Key::K)`); the behavior is identical. The tool id and workspace id strings are the cross-layer contract - they must match what the MODULES layer registers (the registries' `meta().shortcut` is the eventual single source; the constant table is this round's stand-in, flagged in the doc comment).

- [ ] **Step 4: Run the tests (expect PASS).**
```
cargo nextest run -p pixhaus-ui shell::shortcuts
```
Expected: PASS (6 tests).

- [ ] **Step 5: Clippy.**
```
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```
Expected: PASS.

- [ ] **Step 6: Commit.**
```
git add crates/ui/src/shell/shortcuts.rs
git commit -m @'
feat(ui): add shortcut routing with a pure key-to-intent map

collect reads input once per frame and consumes each matched key so a
focused TextEdit and the global handler never both fire. The decision is
the pure map_key (spec test 6): Cmd+1..5 and Cmd+K always map; bare tool
keys are suppressed when a text field is focused, via text_edit_focused.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### SHELL.12: command palette overlay (egui::Area, modal gate, registry-seeded list)

`Intent::OpenCommandPalette` sets `UiState.modal = Modal::CommandPalette`. The overlay is an `egui::Area` (taking `&Context` via `ui.ctx()`) drawn after the central panel so it floats above everything, with `elevation.overlay`. A `TextEdit` bound to `palette_query` and a list seeded from every workspace (`Switch to {name}` - live `SelectWorkspace`), every tool (`Select {tool}` - live `SelectTool`), and registered actions + mock examples. Escape closes.

**Files:**
- Modify: `crates/ui/src/shell/command_palette.rs`

- [ ] **Step 1: Implement the palette overlay.** Replace the stub. Resolve the modal gate first, then destructure. `palette_query` is in `UiState`; binding a `TextEdit` to it needs `&mut state.ui.palette_query`, which is a write to UI state - but the palette is shell chrome, not a contributor, so the shell may write `UiState` directly here (the "panels never mutate" rule binds contributors via `ContribCtx`, not the shell itself). Selections push intents; Escape pushes `Intent::CloseModal`.
```rust
//! Command palette overlay: an egui::Area drawn after the central panel, gated on
//! `UiState.modal == CommandPalette`. A query field plus a registry-seeded entry
//! list (workspaces and tools are live; actions and the UX examples are mock).
//! Escape closes.

use crate::state::intent::Intent;
use crate::state::ui_state::Modal;
use crate::state::Host;
use crate::theme::tokens::SurfaceTier;

struct Entry {
    label: String,
    intent: Intent,
}

/// Draw the palette overlay when the modal is open.
pub fn overlay(host: &mut Host, ui: &mut egui::Ui) {
    if !matches!(host.state.ui.modal, Some(Modal::CommandPalette)) {
        return;
    }

    // Escape closes (read before borrowing the query field mutably).
    if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
        host.intents.push(Intent::CloseModal);
        return;
    }

    let Host {
        registries,
        state,
        intents,
        theme,
        ..
    } = &mut *host;

    // Seed entries: workspaces (live), tools (live), actions + UX examples (mock).
    let mut entries: Vec<Entry> = Vec::new();
    for ws in registries.workspaces.iter() {
        entries.push(Entry {
            label: format!("Switch to {}", ws.meta().name),
            intent: Intent::SelectWorkspace(ws.id()),
        });
    }
    for tool in registries.tools.iter() {
        entries.push(Entry {
            label: format!("Select {}", tool.meta().label),
            intent: Intent::SelectTool(tool.id()),
        });
    }
    for action in registries.actions.iter() {
        if action.palette_visible {
            entries.push(Entry {
                label: action.label.to_string(),
                intent: Intent::RunAction(action.id),
            });
        }
    }

    // Filter by the live query (case-insensitive substring). Context-aware ranking
    // (UX 20.3) is deferred: see the doc TODO below.
    let query = state.ui.palette_query.to_lowercase();
    let filtered: Vec<&Entry> = entries
        .iter()
        .filter(|e| query.is_empty() || e.label.to_lowercase().contains(&query))
        .collect();

    let screen = ui.ctx().screen_rect();
    let area_pos = egui::pos2(screen.center().x - 240.0, screen.top() + 80.0);

    egui::Area::new(egui::Id::new("pixhaus.command_palette"))
        .order(egui::Order::Foreground)
        .fixed_pos(area_pos)
        .show(ui.ctx(), |ui| {
            let frame = egui::Frame::new()
                .fill(theme.surface(SurfaceTier::Elevated))
                .inner_margin(theme.spacing.md)
                .corner_radius(theme.radius.md)
                .shadow(theme.elevation.overlay);
            frame.show(ui, |ui| {
                ui.set_min_width(480.0);
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut state.ui.palette_query)
                        .hint_text("Type a command")
                        .desired_width(f32::INFINITY),
                );
                edit.request_focus();
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for entry in filtered {
                            if ui.button(&entry.label).clicked() {
                                push_clone(intents, &entry.intent);
                                intents.push(Intent::CloseModal);
                            }
                        }
                    });
                // TODO(palette): context-aware ranking (UX 20.3) once core lands.
            });
        });
}

// Intent is not Clone (it will hold Box<dyn Command> later); re-emit by match on the
// palette-reachable variants only.
fn push_clone(intents: &mut crate::state::IntentSink, intent: &Intent) {
    match intent {
        Intent::SelectWorkspace(w) => intents.push(Intent::SelectWorkspace(*w)),
        Intent::SelectTool(t) => intents.push(Intent::SelectTool(*t)),
        Intent::RunAction(a) => intents.push(Intent::RunAction(*a)),
        _ => {}
    }
}
```
Notes: `Intent` is deliberately not `Clone` (the reserved `Command(Box<dyn core::Command>)` variant would forbid it), so `push_clone` re-emits only the palette-reachable `Copy`-payload variants - `WorkspaceId`/`ToolId`/`ActionId` are all `Copy`. If the STATE layer makes `Intent` `Clone`, replace `push_clone` with `intents.push(entry.intent.clone())` and drop the helper. `Modal::CommandPalette` and `state.ui.palette_query` are STATE fields. `theme.elevation.overlay` is the THEME `Shadow` token (field shape confirmed: `Shadow { offset:[i8;2], blur:u8, spread:u8, color }`); `Frame::shadow` accepts it. The mock UX examples from the spec ("Generate sprite from prompt", etc.) come in via the modules' registered `ActionDesc` entries with `palette_visible: true` - the palette does not hardcode them; if no module registered palette examples yet, the list still shows workspaces + tools, which proves the path.

- [ ] **Step 2: Build and clippy.**
```
cargo build -p pixhaus-ui
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```
Expected: PASS both. If `Frame::shadow`/`corner_radius` names differ, match the WIDGETS/THEME usage already established for `widgets::card`.

- [ ] **Step 3: Commit.**
```
git add crates/ui/src/shell/command_palette.rs
git commit -m @'
feat(ui): add the command palette overlay

An egui::Area gated on the CommandPalette modal: a query field plus a
list seeded from the registries (workspaces and tools live, actions
mock), filtered by substring. Escape and selection close it via intents.
Context-aware ranking is a documented deferral.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### SHELL.13: wire shell menus into the shell-owned registrations

The shell owns the always-present menu groups (SHELL.3). They must reach `Registries.menus` so the top bar (SHELL.4) renders them. The STATE/REGISTRY layers build `Host`/`Registries`; the shell contributes its own groups through a small seam so `app` does not hardcode them. This task adds a `shell::register_shell_menus(registries)` (or confirms the existing seam) and verifies the top bar shows the shell groups. Nothing to assert beyond "the groups land in the registry", so write -> test -> commit.

**Files:**
- Modify: `crates/ui/src/shell/menus.rs` (add the registration seam)
- Test: `crates/ui/src/shell/menus.rs`

- [ ] **Step 1: Add the registration seam to `menus.rs`.** Append a function that pushes the shell groups into a `HostRegistrar` (the same path modules use), so `build_host` calls it before/after module registration. Match `HostRegistrar::add_menu_group` from the CONTRIB-API layer.
```rust
use crate::contrib_api::module::HostRegistrar;

/// Register the shell's always-present menu groups through the registrar.
///
/// Called from `build_host` so the always-present groups enter `Registries.menus`
/// by the same path modules use for their Sprite/Layer/Frame/Select groups. Order:
/// call this first so module groups append after the shell's File/Edit/View block.
pub fn register_shell_menus(host: &mut dyn HostRegistrar) {
    for group in shell_menu_groups() {
        host.add_menu_group(group);
    }
}
```

- [ ] **Step 2: Add a test that the seam populates a registrar.** Append to the `tests` module. Build the real `Registries`/registrar via the REGISTRY layer's constructor and assert the shell groups land. If the registrar is constructed through `Host`, use `Host::new(...).registrar()`.
```rust
    #[test]
    fn register_shell_menus_populates_the_registry() {
        let mut host = crate::state::Host::new(crate::theme::Theme::dark());
        register_shell_menus(&mut host.registrar());
        let labels: Vec<&str> = host.registries.menus.iter().map(|g| g.label).collect();
        assert!(labels.contains(&"File"));
        assert!(labels.contains(&"View"));
        assert!(labels.contains(&"Window"));
    }
```
Note: `host.registries.menus` is `Vec<MenuGroup>` per spec; iterate it directly. If `Host::new`/`registrar()`/`registries` field access differ, match the STATE/REGISTRY layer's real surface - the test asserts behavior (groups land), so adapt the construction call, not the assertion.

- [ ] **Step 3: Run the menus tests (expect PASS).**
```
cargo nextest run -p pixhaus-ui shell::menus
```
Expected: PASS (4 tests now). If `Host::new` is not yet available from the STATE layer, this test depends on that layer being merged first; gate it behind the STATE layer's completion and run the build-only check (`cargo build -p pixhaus-ui`) in the interim.

- [ ] **Step 4: Clippy.**
```
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```
Expected: PASS.

- [ ] **Step 5: Commit.**
```
git add crates/ui/src/shell/menus.rs
git commit -m @'
feat(ui): register the shell menu groups through the registrar

Add register_shell_menus so the always-present File/Edit/View/Window/Help
groups enter Registries.menus by the same path modules use, keeping the
top-bar render data-driven and app free of hardcoded menus.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### SHELL.14: layer-wide verification gate

A final pass that the whole `shell` layer compiles, lints, and tests clean together, and that the `CanvasCallback` seam is still intact.

**Files:**
- No new files. Verification only.

- [ ] **Step 1: Full crate test run.**
```
cargo nextest run -p pixhaus-ui
```
Expected: PASS (all SHELL tests - `shell::menus` 4, `shell::shortcuts` 6 - plus every other layer's tests in the crate). If a non-SHELL test fails, it belongs to another layer; do not edit it from here.

- [ ] **Step 2: Doc tests.**
```
cargo test --doc -p pixhaus-ui
```
Expected: PASS (the shell modules carry no runnable doc examples; this confirms none regressed).

- [ ] **Step 3: Crate-wide clippy with tests.**
```
cargo clippy -p pixhaus-ui --all-targets -- -D warnings
```
Expected: PASS (no warnings). The crate root must carry `#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]` (added by an early layer in `lib.rs`); the SHELL tests use `.expect(...)` in `menus.rs`, so confirm that attribute is present - if it is missing, that is a `lib.rs` gap owned by the foundational layer; flag it rather than adding a local `#[allow]`.

- [ ] **Step 4: Confirm the canvas seam is untouched.** Grep that `CanvasCallback` and `install_canvas_renderer` still have their original bodies in `lib.rs` and that `canvas_stage.rs` references `crate::CanvasCallback` (not a reimplementation).
```
cargo build -p pixhaus-ui
```
Plus a read of `crates/ui/src/lib.rs` to confirm the `CanvasCallback`/`install_canvas_renderer` block matches the pre-existing seam verbatim. Expected: unchanged.

- [ ] **Step 5: Final commit only if Steps 1-4 surfaced a fix.** If everything passed with no edits, there is nothing to commit (the per-task commits already landed). If a fix was needed:
```
git add crates/ui/src/shell
git commit -m @'
fix(ui): tidy shell layer to pass the crate verification gate

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

## Phase 8: Module crates and app wiring

Confirmed: `ui.close()` is the 0.34.2 idiom (`close_menu` is deprecated), matching the spec. I have all the verified API details. Now I'll write the MODULES layer plan.

The MODULES layer is the largest by surface area, so I'll structure it as: module crate scaffolding (Cargo.toml + lib.rs for all 5), then the fully-worked examples (DrawWorkspace, Layers panel, Prompt panel, Pencil tool, sprite-edit register fn), then the enumerated remaining panels/tools/workspaces, then app wiring. Let me write it.

# Layer MODULES: the five module crates and app wiring

Implements spec sections: "modules/* and app wiring" (lines 122-193), "Workspaces and placeholder panel inventory" (lines 909-994), "app/src/main.rs" (lines 144-187). This layer depends on the `pixhaus-ui` crate's permanent contract - the THEME, CONTRIB, STATE, REGISTRY, SHELL, and WIDGETS layers - being in place. Every `pixhaus_ui::...` type named below is the shared contract from the spec; use the exact path shown and assume it exists.

Verified-against-pinned-API facts this layer relies on (egui/epaint 0.34.2 source):
- `ui.menu_button(label, |ui| ...)` exists; dismiss an item with `ui.close()` (`close_menu` is deprecated).
- `egui::KeyboardShortcut::new(Modifiers, Key)` with `Key::Num1..=Num5`, `Modifiers::COMMAND`.
- `egui::TextEdit::multiline(&mut String)` takes `&mut String` in-frame - this is why `PanelScope::scratch` exists.
- `egui::Sense::click_and_drag()` for the canvas allocate-painter seam (kept in `crates/ui`, not this layer).

Shared rules baked into every task below: cargo runs through **PowerShell**; tests run under **cargo nextest**, doc tests under `cargo test --doc`; no `unwrap()`/`panic!()` outside tests; libraries use `thiserror` (these crates define no error types this round, so neither appears); `unsafe` is forbidden; commits are Conventional Commits with the trailer `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`; branch `feat/ui-shell-foundation` already exists - do not create one. The post-edit hook auto-formats and runs `cargo clippy --tests -- -D warnings` on the touched crate, so the explicit clippy step below is a re-check, not the only gate.

A convention shared by every module crate in this layer: panels and tools are `&self` (they hold no mutable state), construction is `Box::new(<Type>)` of a unit struct, and every panel/tool/workspace declares its id once as a `const` so the registry key and the `id()` method agree. Each module crate root carries `#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]`.

---

### MODULES.1: Scaffold the five module crate manifests

**Files:**
- Modify: `modules/sprite-edit/Cargo.toml`
- Modify: `modules/animation/Cargo.toml`
- Modify: `modules/tiles/Cargo.toml`
- Modify: `modules/generation/Cargo.toml`
- Modify: `modules/export/Cargo.toml`

Each module depends only on `pixhaus-ui` this round (within the `core+services+ui` ceiling). The existing manifests have no `[dependencies]` section.

- [ ] **Step 1: Add the `pixhaus-ui` dependency to sprite-edit.** Insert a `[dependencies]` section before `[lints]` in `modules/sprite-edit/Cargo.toml`:

```toml
[dependencies]
egui.workspace = true
pixhaus-ui = { path = "../../crates/ui" }

[lints]
workspace = true
```

(The `[lints]` block already exists; the edit inserts the `[dependencies]` block above it.)

- [ ] **Step 2: Repeat for the other four manifests.** Apply the identical `[dependencies]` block (egui + `pixhaus-ui = { path = "../../crates/ui" }`) to `modules/animation/Cargo.toml`, `modules/tiles/Cargo.toml`, `modules/generation/Cargo.toml`, and `modules/export/Cargo.toml`, each above its existing `[lints]` block.

- [ ] **Step 3: Verify the manifests parse.** Run in PowerShell:

```powershell
cargo metadata --no-deps --format-version 1 | Out-Null; if ($?) { "manifests OK" }
```

Expected: prints `manifests OK` (exit 0). If it errors on a missing path, the `../../crates/ui` relative path is wrong - module crates sit at `modules/<name>/`, so `crates/ui` is two levels up.

- [ ] **Step 4: Commit.**

```powershell
git add modules/sprite-edit/Cargo.toml modules/animation/Cargo.toml modules/tiles/Cargo.toml modules/generation/Cargo.toml modules/export/Cargo.toml
git commit -m @'
chore(modules): wire pixhaus-ui into the five shell module crates

The shell-foundation modules each register a workspace plus panels and
tools through the ui crate. They depend on egui and pixhaus-ui only this
round, within the core+services+ui ceiling.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### MODULES.2: Worked example - DrawWorkspace (the Workspace kind)

**Files:**
- Create: `modules/sprite-edit/src/draw.rs`
- Test: `modules/sprite-edit/src/draw.rs` (inline `#[cfg(test)]`)

`DrawWorkspace` is the fully-worked Workspace impl. Its `layout()` returns the exact `WorkspaceLayout` from the spec inventory table (line 943): right dock `Layers, Sprites, Palette, Selection Actions, AI Assistant`; tray `Frames, Assets, Console`; full 15-tool rail; default tool Pencil; status item `Pixel Grid On`. This file also holds the shared-panel `const PanelId`s that other modules reference by id, and the module's `register` fn (worked in MODULES.6). Start with just the workspace; panels arrive in later tasks within this same file.

- [ ] **Step 1: Write a failing layout test first.** Create `modules/sprite-edit/src/draw.rs` with only the ids, the workspace, and the test (the `register` fn and panels are added later):

```rust
//! The Draw workspace and the shared sprite-editing panels.
//!
//! Draw owns the panels other workspaces reuse by id (bible rule 2): the
//! Layers/Sprites/Palette/AI Assistant dock panels and the Frames/Assets/Console
//! tray panels. They are registered once, here, before any other module.

use egui::{Key, KeyboardShortcut, Modifiers};
use pixhaus_ui::contrib_api::{
    ContribCtx, Panel, PanelId, PanelMeta, PanelScope, ToolId, Workspace, WorkspaceId,
    WorkspaceLayout, WorkspaceMeta,
};
use pixhaus_ui::region::Region;
use pixhaus_ui::state::StatusItem;
use pixhaus_ui::{icons, widgets};

// Workspace id.
pub const DRAW: WorkspaceId = WorkspaceId("draw");

// Shared panel ids - referenced by id from the other workspaces (bible rule 2).
pub const LAYERS: PanelId = PanelId("layers");
pub const SPRITES: PanelId = PanelId("sprites");
pub const PALETTE: PanelId = PanelId("palette");
pub const SELECTION_ACTIONS: PanelId = PanelId("selection-actions");
pub const AI_ASSISTANT: PanelId = PanelId("ai-assistant");
pub const FRAMES: PanelId = PanelId("frames");
pub const ASSETS: PanelId = PanelId("assets");
pub const CONSOLE: PanelId = PanelId("console");

// Tool ids live in `tools.rs`; the layout references them.
use crate::tools;

/// The Draw workspace: editing a single sprite in space. Layout only - owns no data.
pub struct DrawWorkspace;

impl Workspace for DrawWorkspace {
    fn id(&self) -> WorkspaceId {
        DRAW
    }

    fn meta(&self) -> WorkspaceMeta {
        WorkspaceMeta {
            name: "Draw",
            icon: icons::PENCIL,
            purpose: "Paint and edit a single sprite",
            shortcut: KeyboardShortcut::new(Modifiers::COMMAND, Key::Num1),
        }
    }

    fn layout(&self) -> WorkspaceLayout {
        WorkspaceLayout {
            right_dock: vec![LAYERS, SPRITES, PALETTE, SELECTION_ACTIONS, AI_ASSISTANT],
            bottom_tray: vec![FRAMES, ASSETS, CONSOLE],
            primary_tools: tools::ALL.to_vec(),
            default_tool: tools::PENCIL,
            status_items: vec![StatusItem {
                icon: icons::GRID,
                text: "Pixel Grid On".to_owned(),
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_layout_matches_the_inventory() {
        let layout = DrawWorkspace.layout();
        assert_eq!(
            layout.right_dock,
            vec![LAYERS, SPRITES, PALETTE, SELECTION_ACTIONS, AI_ASSISTANT]
        );
        assert_eq!(layout.bottom_tray, vec![FRAMES, ASSETS, CONSOLE]);
        assert_eq!(layout.default_tool, tools::PENCIL);
        assert_eq!(layout.primary_tools.len(), 15);
        assert_eq!(layout.status_items.len(), 1);
    }

    #[test]
    fn draw_meta_uses_cmd_1() {
        assert_eq!(
            DrawWorkspace.meta().shortcut,
            KeyboardShortcut::new(Modifiers::COMMAND, Key::Num1)
        );
    }
}
```

The unused imports (`ContribCtx`, `Panel`, `PanelMeta`, `PanelScope`, `Region`, `widgets`) are for the panels added in MODULES.3-4; they will warn until then. To keep the post-edit clippy gate green between tasks, temporarily trim the `use` list to only what this step uses (`Key, KeyboardShortcut, Modifiers`, `ToolId` is also unused here so drop it, `Workspace, WorkspaceId, WorkspaceLayout, WorkspaceMeta, PanelId, StatusItem, icons`), then re-add as panels land. Simpler: add `#![allow(unused_imports)]` is NOT allowed (clippy pedantic) - instead, import only what each step uses and grow the list.

For this step, the minimal import set is:

```rust
use egui::{Key, KeyboardShortcut, Modifiers};
use pixhaus_ui::contrib_api::{
    PanelId, Workspace, WorkspaceId, WorkspaceLayout, WorkspaceMeta,
};
use pixhaus_ui::icons;
use pixhaus_ui::state::StatusItem;

use crate::tools;
```

- [ ] **Step 2: Add the module declaration and a minimal `tools.rs` ids stub so this compiles.** This file references `crate::tools::{ALL, PENCIL}`. Create `modules/sprite-edit/src/tools.rs` with just the id table (full tool impls land in MODULES.5):

```rust
//! The 15 shared editing tools (bible rule 2). Manual brushes plus the AI Brush.

use pixhaus_ui::contrib_api::ToolId;

pub const PENCIL: ToolId = ToolId("pencil");
pub const ERASER: ToolId = ToolId("eraser");
pub const FILL: ToolId = ToolId("fill");
pub const LINE: ToolId = ToolId("line");
pub const RECTANGLE: ToolId = ToolId("rectangle");
pub const ELLIPSE: ToolId = ToolId("ellipse");
pub const EYEDROPPER: ToolId = ToolId("eyedropper");
pub const SELECTION: ToolId = ToolId("selection");
pub const LASSO: ToolId = ToolId("lasso");
pub const MOVE: ToolId = ToolId("move");
pub const TRANSFORM: ToolId = ToolId("transform");
pub const TEXT: ToolId = ToolId("text");
pub const HAND: ToolId = ToolId("hand");
pub const ZOOM: ToolId = ToolId("zoom");
pub const AI_BRUSH: ToolId = ToolId("ai-brush");

/// Full rail order: manual tools, then the AI Brush last (bible rule 2 ordering).
pub const ALL: [ToolId; 15] = [
    PENCIL, ERASER, FILL, LINE, RECTANGLE, ELLIPSE, EYEDROPPER, SELECTION, LASSO, MOVE,
    TRANSFORM, TEXT, HAND, ZOOM, AI_BRUSH,
];
```

- [ ] **Step 3: Wire the modules into `lib.rs`.** Replace the stub `modules/sprite-edit/src/lib.rs` body with the module tree and the `SpriteEditModule` shell (the `register` body is filled in MODULES.6; for now it only registers the workspace so the crate compiles):

```rust
//! Pixhaus sprite-editing module: the Draw workspace and the shared editing core.
//!
//! Registers the Draw workspace, the shared Layers/Sprites/Palette/Selection
//! Actions/AI Assistant dock panels, the shared Frames/Assets/Console tray panels,
//! and the 15 shared editing tools (architecture bible sections 7.3, 6.3). The
//! shared panels are registered here, before any other module, so the other
//! workspaces can reference them by id (bible rule 2).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod draw;
mod tools;

use pixhaus_ui::contrib_api::{HostRegistrar, Module};

/// The sprite-editing module. Registers the shared editing surface and the Draw
/// workspace.
pub struct SpriteEditModule;

impl Module for SpriteEditModule {
    fn id(&self) -> &'static str {
        "sprite-edit"
    }

    fn register(&self, host: &mut dyn HostRegistrar) {
        tools::register(host);
        draw::register(host);
    }
}
```

This references `tools::register` and `draw::register`, which do not exist yet - so add temporary empty stubs to unblock the build until MODULES.5/6 fill them. Add to `tools.rs`:

```rust
use pixhaus_ui::contrib_api::HostRegistrar;

/// Register every shared editing tool. Bodies land with the Tool impls.
pub fn register(_host: &mut dyn HostRegistrar) {}
```

and to `draw.rs`:

```rust
use pixhaus_ui::contrib_api::HostRegistrar;

/// Register the Draw workspace and the shared panels. Panel registration lands
/// with the panel impls.
pub fn register(host: &mut dyn HostRegistrar) {
    host.add_workspace(Box::new(DrawWorkspace));
}
```

- [ ] **Step 4: Run the workspace test.** In PowerShell:

```powershell
cargo nextest run -p pixhaus-mod-sprite-edit
```

Expected: PASS - `draw_layout_matches_the_inventory` and `draw_meta_uses_cmd_1` both green. If `icons::PENCIL`/`icons::GRID` or `StatusItem`/`WorkspaceLayout` do not resolve, the ICONS/CONTRIB/STATE layers have not landed yet - that is a dependency, not a bug here; confirm those layers are merged first.

- [ ] **Step 5: Run clippy on the crate.**

```powershell
cargo clippy -p pixhaus-mod-sprite-edit --all-targets -- -D warnings
```

Expected: clean (exit 0).

- [ ] **Step 6: Commit.**

```powershell
git add modules/sprite-edit/src/draw.rs modules/sprite-edit/src/tools.rs modules/sprite-edit/src/lib.rs
git commit -m @'
feat(sprite-edit): add DrawWorkspace and the shared id tables

DrawWorkspace returns the inventory layout - five dock panels, three tray
tabs, the full 15-tool rail, Pencil default, the Pixel Grid status item.
The shared panel and tool ids are declared as consts so other workspaces
reference them by id. Panel and tool bodies follow.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### MODULES.3: Worked example - the Layers panel (the regular Panel kind)

**Files:**
- Modify: `modules/sprite-edit/src/draw.rs`

`Layers` is the worked regular `Panel`: it reads through `scope.ctx`, emits an `Intent::RunAction` on a button, uses `widgets` helpers, and renders the spec mock content (line 959): a `+ New Layer` button, rows `Layer 3 / Layer 2 / Layer 1 / Background` with eye+lock toggles, an opacity slider, a `Normal` blend label; the selected row tinted `accent.muted`. Per the spec borrow model, the panel is `&self`, the shell scopes ids (`push_id`), so the panel body does not call `push_id` itself. Local widget state (slider value) is throwaway - it lives in the panel body and resets each frame; that is acceptable for mock content because nothing reads it back (the spec, line 936, says tool/panel controls "drive nothing").

A note on `&self` + throwaway state: a `Slider` needs `&mut f32`. Since the panel is `&self` and owns no mutable state, bind the slider to a `let mut opacity = 255.0;` inside `ui()` each frame. It snaps back every frame - correct for mock content, and the only honest way to show a live widget without real state. Do NOT route it through scratch (scratch is for `TextEdit` text only).

- [ ] **Step 1: Add the Layers panel impl to `draw.rs`.** Append after `DrawWorkspace`'s impl. Grow the import list to add `ContribCtx` is not needed here (panels take `PanelScope`); add `Panel, PanelMeta, PanelScope`, `Region`, `widgets`, and `ToolId` is still unused so leave it out. The needed additions to the top-of-file `use`:

```rust
use pixhaus_ui::contrib_api::{Panel, PanelMeta, PanelScope};
use pixhaus_ui::region::Region;
use pixhaus_ui::state::Intent;
use pixhaus_ui::widgets;
use pixhaus_ui::contrib_api::ActionId;
```

The panel impl:

```rust
/// The Layers panel. Mock content: a row per layer with eye/lock toggles, an
/// opacity slider, and a blend-mode label. Reads nothing real yet; the selected
/// row is tinted with the accent. New Layer pushes a RunAction intent.
pub struct LayersPanel;

const NEW_LAYER: ActionId = ActionId("layer.new");

impl Panel for LayersPanel {
    fn id(&self) -> PanelId {
        LAYERS
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: "Layers",
            icon: icons::STACK,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        if widgets::section_header(ui, theme, icons::STACK, "Layers")
            .button("+ New Layer")
            .clicked()
        {
            scope.ctx.intents.push(Intent::RunAction(NEW_LAYER));
        }

        // Selected row index is mock UI state; the first row reads as selected.
        let rows = ["Layer 3", "Layer 2", "Layer 1", "Background"];
        for (i, name) in rows.iter().enumerate() {
            let selected = i == 0;
            let frame = if selected {
                egui::Frame::new().fill(theme.accent.muted)
            } else {
                egui::Frame::new()
            };
            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Eye + lock toggles - inert mock controls.
                    let mut visible = true;
                    let mut locked = false;
                    ui.toggle_value(&mut visible, icons::EYE.to_string());
                    ui.toggle_value(&mut locked, icons::LOCK.to_string());
                    ui.label(*name);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label("Normal");
                    });
                });
                // Opacity slider - throwaway local state, resets each frame (mock).
                let mut opacity = 255.0_f32;
                ui.add(egui::Slider::new(&mut opacity, 0.0..=255.0).text("Opacity"));
            });
        }
    }
}
```

`widgets::section_header` returns a header response with a `.button(...)` helper per the WIDGETS layer contract (spec line 866 / widgets/section_header.rs); if that layer's `section_header` signature differs, match it - the contract is `section_header(ui, theme, icon, title)` returning a response that exposes a trailing-button affordance. If WIDGETS only exposes the bare header, fall back to: render `widgets::section_header(ui, theme, icons::STACK, "Layers");` then `if ui.button("+ New Layer").clicked() { ... }` on the next line. Either satisfies the mock-content requirement.

- [ ] **Step 2: Register the Layers panel.** In `draw::register`, add the panel registration (it will grow as MODULES.4 adds the other shared panels):

```rust
pub fn register(host: &mut dyn HostRegistrar) {
    host.add_workspace(Box::new(DrawWorkspace));
    host.add_panel(Box::new(LayersPanel));
}
```

- [ ] **Step 3: Add a registration test.** Append to the `tests` module in `draw.rs` - assert the panel id and meta region. (Body rendering needs an egui frame and is out of scope per spec line 1076; assert metadata, which is pure.)

```rust
#[test]
fn layers_panel_meta() {
    let meta = LayersPanel.meta();
    assert_eq!(LayersPanel.id(), LAYERS);
    assert_eq!(meta.title, "Layers");
    assert_eq!(meta.default_region, Region::RightDock);
    assert!(meta.default_open);
}
```

- [ ] **Step 4: Run the test.**

```powershell
cargo nextest run -p pixhaus-mod-sprite-edit
```

Expected: PASS, including `layers_panel_meta`.

- [ ] **Step 5: Commit.**

```powershell
git add modules/sprite-edit/src/draw.rs
git commit -m @'
feat(sprite-edit): add the Layers panel with mock rows

Layers is the worked regular-Panel example: section header + New Layer
button pushing a RunAction intent, four mock layer rows with eye/lock
toggles and an opacity slider, the selected row tinted accent.muted.
Reads through scope.ctx; emits intents; owns no mutable state.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### MODULES.4: Worked example - the Generate Prompt panel (the scratch `&mut String` carve-out)

**Files:**
- Create: `modules/generation/src/lib.rs`
- Create: `modules/generation/src/generate.rs`

`Prompt` is the worked panel that uses the scratch carve-out (spec line 972): a multiline `TextEdit` bound to `scope.scratch` (the only `&mut String` a panel may touch), plus a primary `[sparkle] Generate` button (accent) that pushes `Intent::RunAction`. This panel lives in the generation module, so this task also scaffolds that module's `lib.rs` and `generate.rs`. The GenerationModule registers the Generate workspace, its dock panels (Prompt, Recipe, Structure, Style, Palette Behavior, Advanced Settings), and the shared Results/History tray panels it owns; the others land in MODULES.9. Here we do the module shell + the Prompt panel + the workspace.

- [ ] **Step 1: Scaffold `generation/src/lib.rs`.** Replace the stub:

```rust
//! Pixhaus generation module: the AI-forward Generate workspace.
//!
//! Registers the Generate workspace, the prompt composer and recipe/structure/
//! style panels, palette-behavior and advanced settings, and the shared Results
//! and History tray panels (architecture bible sections 7.3, 6.5, 14). Provider
//! dispatch arrives with the services layer; panels render mock content this round.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod generate;

use pixhaus_ui::contrib_api::{HostRegistrar, Module};

/// The generation module.
pub struct GenerationModule;

impl Module for GenerationModule {
    fn id(&self) -> &'static str {
        "generation"
    }

    fn register(&self, host: &mut dyn HostRegistrar) {
        generate::register(host);
    }
}
```

- [ ] **Step 2: Write `generate.rs` with the workspace, the Prompt panel, and `register`.** The Generate workspace layout per the inventory (line 946): dock `Prompt, Recipe, Structure, Style, Palette Behavior, Advanced Settings`; tray `Results, History, Console`; tools `{Hand, Zoom, Selection, AI Brush}`; default tool AI Brush; status items `AI Ready` (dot icon) and `Seed 123456`. Console is a sprite-edit shared panel referenced by id.

```rust
//! The Generate workspace and its panels.

use egui::{Key, KeyboardShortcut, Modifiers};
use pixhaus_ui::contrib_api::{
    ActionId, HostRegistrar, Panel, PanelId, PanelMeta, PanelScope, ToolId, Workspace,
    WorkspaceId, WorkspaceLayout, WorkspaceMeta,
};
use pixhaus_ui::region::Region;
use pixhaus_ui::state::{Intent, StatusItem};
use pixhaus_ui::{icons, widgets};

pub const GENERATE: WorkspaceId = WorkspaceId("generate");

// Generate-owned panel ids.
pub const PROMPT: PanelId = PanelId("prompt");
pub const RECIPE: PanelId = PanelId("recipe");
pub const STRUCTURE: PanelId = PanelId("structure");
pub const STYLE: PanelId = PanelId("style");
pub const PALETTE_BEHAVIOR: PanelId = PanelId("palette-behavior");
pub const ADVANCED_SETTINGS: PanelId = PanelId("advanced-settings");
pub const RESULTS: PanelId = PanelId("results");
pub const HISTORY: PanelId = PanelId("history");

// Tools and tray panels owned elsewhere, referenced by id.
const HAND: ToolId = ToolId("hand");
const ZOOM: ToolId = ToolId("zoom");
const SELECTION: ToolId = ToolId("selection");
const AI_BRUSH: ToolId = ToolId("ai-brush");
const CONSOLE: PanelId = PanelId("console");

const GENERATE_RUN: ActionId = ActionId("generate.run");

/// The Generate workspace. AI-forward: the rail is the minimal navigation set.
pub struct GenerateWorkspace;

impl Workspace for GenerateWorkspace {
    fn id(&self) -> WorkspaceId {
        GENERATE
    }

    fn meta(&self) -> WorkspaceMeta {
        WorkspaceMeta {
            name: "Generate",
            icon: icons::SPARKLE,
            purpose: "Generate sprites from a prompt",
            shortcut: KeyboardShortcut::new(Modifiers::COMMAND, Key::Num4),
        }
    }

    fn layout(&self) -> WorkspaceLayout {
        WorkspaceLayout {
            right_dock: vec![PROMPT, RECIPE, STRUCTURE, STYLE, PALETTE_BEHAVIOR, ADVANCED_SETTINGS],
            bottom_tray: vec![RESULTS, HISTORY, CONSOLE],
            primary_tools: vec![HAND, ZOOM, SELECTION, AI_BRUSH],
            default_tool: AI_BRUSH,
            status_items: vec![
                StatusItem { icon: icons::CIRCLE, text: "AI Ready".to_owned() },
                StatusItem { icon: icons::HASH, text: "Seed 123456".to_owned() },
            ],
        }
    }
}

/// The Prompt panel. Worked example of the scratch carve-out: the multiline
/// TextEdit is bound to scope.scratch (the only &mut String a panel may touch).
/// The Generate button pushes a RunAction intent.
pub struct PromptPanel;

impl Panel for PromptPanel {
    fn id(&self) -> PanelId {
        PROMPT
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: "Prompt",
            icon: icons::SPARKLE,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        widgets::section_header(ui, theme, icons::SPARKLE, "Prompt");

        // The carve-out: TextEdit needs a live &mut String in-frame. scope.scratch
        // is this panel's own draft buffer - the single, disjoint exception to the
        // intents-only write channel. Never route real mutation through it.
        ui.add(
            egui::TextEdit::multiline(scope.scratch)
                .desired_rows(4)
                .desired_width(f32::INFINITY)
                .hint_text("Describe the sprite. Use {variable} chips."),
        );

        ui.add_space(theme.spacing.sm);

        // Primary accent Generate button with the sparkle marker.
        let label = format!("{} Generate", icons::SPARKLE);
        let button = egui::Button::new(egui::RichText::new(label).color(theme.roles.text_primary))
            .fill(theme.accent.base);
        if ui.add(button).clicked() {
            scope.ctx.intents.push(Intent::RunAction(GENERATE_RUN));
        }
    }
}

/// Register the Generate workspace and its panels. The other dock panels (Recipe,
/// Structure, Style, Palette Behavior, Advanced Settings) and the Results/History
/// tray panels are added in their own tasks.
pub fn register(host: &mut dyn HostRegistrar) {
    host.add_workspace(Box::new(GenerateWorkspace));
    host.add_panel(Box::new(PromptPanel));
}
```

- [ ] **Step 3: Add the workspace + prompt-meta tests.** Append a `tests` module to `generate.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_layout_matches_the_inventory() {
        let layout = GenerateWorkspace.layout();
        assert_eq!(layout.right_dock.first(), Some(&PROMPT));
        assert_eq!(layout.right_dock.len(), 6);
        assert_eq!(layout.bottom_tray, vec![RESULTS, HISTORY, CONSOLE]);
        assert_eq!(layout.default_tool, AI_BRUSH);
        assert_eq!(layout.primary_tools, vec![HAND, ZOOM, SELECTION, AI_BRUSH]);
    }

    #[test]
    fn prompt_panel_meta() {
        assert_eq!(PromptPanel.id(), PROMPT);
        assert_eq!(PromptPanel.meta().default_region, Region::RightDock);
    }
}
```

- [ ] **Step 4: Run the tests.**

```powershell
cargo nextest run -p pixhaus-mod-generation
```

Expected: PASS. If `icons::CIRCLE`/`icons::HASH`/`icons::SPARKLE` do not resolve, the ICONS layer's glyph set is missing those names - add them there (out of this layer) or substitute the nearest available constant and note it.

- [ ] **Step 5: Commit.**

```powershell
git add modules/generation/src/lib.rs modules/generation/src/generate.rs
git commit -m @'
feat(generation): add Generate workspace and the Prompt panel

Prompt is the worked scratch carve-out example: a multiline TextEdit bound
to scope.scratch and an accent Generate button pushing a RunAction intent.
GenerateWorkspace returns the inventory layout - six dock panels, the
Results/History/Console tray, the minimal {Hand,Zoom,Selection,AI Brush}
rail with AI Brush default.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### MODULES.5: Worked example - the Pencil tool, and all 15 tool impls

**Files:**
- Modify: `modules/sprite-edit/src/tools.rs`

Pencil is the worked `Tool`: `meta()` returns the spec ToolMeta (label, icon, `Key::B` shortcut, tooltip, `is_ai: false`), and `options_ui` renders the spec mock options (line 925): `Size 1px - Opacity 255 - Pixel-perfect [x] - Dither None - Mirror X [ ] - Mirror Y [ ]`, all live egui widgets bound to throwaway local state. After Pencil, the remaining 14 follow the identical shape; their metas and option rows are enumerated in the table at the end of this task. `tools::register` boxes all 15.

The tool shortcuts (from spec line 917): Pencil B, Eraser E, Fill G, Line L, Rectangle U, Ellipse O, Eyedropper I, Selection M, Lasso Q, Move V, Transform Shift+T, Text X, Hand H, Zoom Z, AI Brush J. AI Brush sets `is_ai: true`. `Modifiers::NONE` for single keys; Transform uses `Modifiers::SHIFT`.

- [ ] **Step 1: Replace the `register` stub and add the worked Pencil tool.** Update the top of `tools.rs` to import what the impls need, and replace the empty `register`:

```rust
use egui::{Key, KeyboardShortcut, Modifiers};
use pixhaus_ui::contrib_api::{ContribCtx, HostRegistrar, Tool, ToolId, ToolMeta};
use pixhaus_ui::icons;

// ... the const ids and ALL array from MODULES.2 stay ...

/// The Pencil tool. Worked Tool example: options_ui renders the mock option row
/// with live widgets bound to throwaway local state - they move, they drive
/// nothing this round.
pub struct PencilTool;

impl Tool for PencilTool {
    fn id(&self) -> ToolId {
        PENCIL
    }

    fn meta(&self) -> ToolMeta {
        ToolMeta {
            label: "Pencil",
            icon: icons::PENCIL,
            shortcut: Some(KeyboardShortcut::new(Modifiers::NONE, Key::B)),
            tooltip: "Draw individual pixels. Hold Shift for a line.",
            is_ai: false,
        }
    }

    fn options_ui(&self, ui: &mut egui::Ui, _cx: &mut ContribCtx<'_>) {
        ui.horizontal(|ui| {
            let mut size = 1.0_f32;
            ui.add(egui::DragValue::new(&mut size).prefix("Size ").suffix("px"));
            let mut opacity = 255.0_f32;
            ui.add(egui::DragValue::new(&mut opacity).prefix("Opacity ").range(0.0..=255.0));
            let mut pixel_perfect = true;
            ui.checkbox(&mut pixel_perfect, "Pixel-perfect");
            ui.label("Dither None");
            let mut mirror_x = false;
            ui.checkbox(&mut mirror_x, "Mirror X");
            let mut mirror_y = false;
            ui.checkbox(&mut mirror_y, "Mirror Y");
        });
    }
}
```

- [ ] **Step 2: Add the remaining 14 tool impls following the Pencil shape.** Each is a unit struct `<Name>Tool` with `id()` returning its const, `meta()` per the table below, and `options_ui` rendering its mock row from spec lines 925-934 with live throwaway widgets. The metas:

| Tool struct | id const | label | shortcut | is_ai | tooltip | options_ui mock row (spec 925-934) |
|---|---|---|---|---|---|---|
| `EraserTool` | ERASER | Eraser | `NONE+E` | false | "Erase pixels to transparent." | `Size 1px` (DragValue) - `Opacity 255` (DragValue) - `Pixel-perfect [x]` (checkbox) |
| `FillTool` | FILL | Fill | `NONE+G` | false | "Flood-fill contiguous color." | `Tolerance 0` (DragValue) - `Contiguous [x]` - `All layers [ ]` |
| `LineTool` | LINE | Line | `NONE+L` | false | "Draw a straight line." | `Size 1px` - `Fill [ ]` - `From center [ ]` |
| `RectangleTool` | RECTANGLE | Rectangle | `NONE+U` | false | "Draw a rectangle." | `Size 1px` - `Fill [ ]` - `From center [ ]` |
| `EllipseTool` | ELLIPSE | Ellipse | `NONE+O` | false | "Draw an ellipse." | `Size 1px` - `Fill [ ]` - `From center [ ]` |
| `EyedropperTool` | EYEDROPPER | Eyedropper | `NONE+I` | false | "Pick a color from the canvas." | `Sample Composite` (label) - `Add to palette [ ]` |
| `SelectionTool` | SELECTION | Selection | `NONE+M` | false | "Rectangular marquee selection." | `Mode Replace` (label) - `Feather 0` (DragValue) - `Snap Pixel [x]` - `Origin Center` (label) |
| `LassoTool` | LASSO | Lasso | `NONE+Q` | false | "Freeform selection." | `Mode Replace` (label) - `Feather 0` (DragValue) |
| `MoveTool` | MOVE | Move | `NONE+V` | false | "Move the selection or layer." | `Origin Center` (label) - `Snap [ ]` |
| `TransformTool` | TRANSFORM | Transform | `SHIFT+T` | false | "Scale, rotate, and skew." | `Origin Center` (label) - `Snap [ ]` |
| `TextTool` | TEXT | Text | `NONE+X` | false | "Place pixel text." | `Font Pixel` (label) - `Size 8` (DragValue) |
| `HandTool` | HAND | Hand | `NONE+H` | false | "Pan the canvas." | `Zoom 1600%` (label) - `[Fit]` (button) - `[100%]` (button) |
| `ZoomTool` | ZOOM | Zoom | `NONE+Z` | false | "Zoom in and out." | `Zoom 1600%` (label) - `[Fit]` (button) - `[100%]` (button) |
| `AiBrushTool` | AI_BRUSH | AI Brush | `NONE+J` | **true** | "AI-assisted painting. Describe what to draw." | `[sparkle]` prefix on the header, then `Mode Fill` (label) - `Use Palette [x]` - `Preserve Outline [x]` - `Variations 4` (DragValue) - `Strength 0.65` (DragValue range 0.0..=1.0) |

Implement each exactly like `PencilTool`: `ui.horizontal(|ui| { ... })` with `egui::DragValue` for numerics, `ui.checkbox(&mut b, "...")` for `[ ]`/`[x]` toggles (init `true` for `[x]`, `false` for `[ ]`), `ui.label("...")` for fixed `Label Value` text, and `ui.button("...")` for `[Fit]`/`[100%]`. For `AiBrushTool::options_ui`, prefix the row with `ui.label(egui::RichText::new(icons::SPARKLE.to_string()).color(_cx.theme.accent.ai));`.

- [ ] **Step 3: Fill `tools::register`.** Box all 15 in `ALL` order:

```rust
/// Register every shared editing tool, in rail order.
pub fn register(host: &mut dyn HostRegistrar) {
    host.add_tool(Box::new(PencilTool));
    host.add_tool(Box::new(EraserTool));
    host.add_tool(Box::new(FillTool));
    host.add_tool(Box::new(LineTool));
    host.add_tool(Box::new(RectangleTool));
    host.add_tool(Box::new(EllipseTool));
    host.add_tool(Box::new(EyedropperTool));
    host.add_tool(Box::new(SelectionTool));
    host.add_tool(Box::new(LassoTool));
    host.add_tool(Box::new(MoveTool));
    host.add_tool(Box::new(TransformTool));
    host.add_tool(Box::new(TextTool));
    host.add_tool(Box::new(HandTool));
    host.add_tool(Box::new(ZoomTool));
    host.add_tool(Box::new(AiBrushTool));
}
```

- [ ] **Step 4: Add tool-meta tests.** Append a `tests` module to `tools.rs` asserting the load-bearing metas - Pencil's shortcut, AI Brush's `is_ai` flag, and that `ALL` has 15 unique ids:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn pencil_meta() {
        let m = PencilTool.meta();
        assert_eq!(PencilTool.id(), PENCIL);
        assert_eq!(m.label, "Pencil");
        assert!(!m.is_ai);
        assert_eq!(m.shortcut, Some(KeyboardShortcut::new(Modifiers::NONE, Key::B)));
    }

    #[test]
    fn ai_brush_is_flagged_ai() {
        assert!(AiBrushTool.meta().is_ai);
        assert_eq!(
            AiBrushTool.meta().shortcut,
            Some(KeyboardShortcut::new(Modifiers::NONE, Key::J))
        );
    }

    #[test]
    fn all_tool_ids_are_unique() {
        let set: HashSet<_> = ALL.iter().collect();
        assert_eq!(set.len(), ALL.len());
    }
}
```

- [ ] **Step 5: Run the tests.**

```powershell
cargo nextest run -p pixhaus-mod-sprite-edit
```

Expected: PASS, including the three new tool tests.

- [ ] **Step 6: Commit.**

```powershell
git add modules/sprite-edit/src/tools.rs
git commit -m @'
feat(sprite-edit): add the 15 shared editing tools

Pencil is the worked Tool example - meta with the B shortcut and an
options_ui row of live throwaway widgets. The other 14 follow the same
shape with their inventory option rows; AI Brush flips is_ai and renders
the sparkle marker. register boxes all 15 in rail order.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### MODULES.6: Complete sprite-edit's register fn - the shared dock and tray panels

**Files:**
- Modify: `modules/sprite-edit/src/draw.rs`

This is the worked `register(host)` for a module (spec lines 134-142): sprite-edit registers the Draw workspace, all five shared dock panels (Layers done in MODULES.3; add Sprites, Palette, Selection Actions, AI Assistant), and the three shared tray panels (Frames, Assets, Console) FIRST, so the other workspaces reference them by id. It also contributes the Sprite and Layer menu groups (spec line 1005). Implement the four remaining dock panels and three tray panels following the Layers worked example, then complete `register`.

Mock content per spec lines 959-993:
- **Sprites** (line 960): grid of 6 mock sprite thumbnails (checkerboard rects via `widgets::placeholder` thumbnail-grid helper), `+ New Sprite` button -> `Intent::RunAction(ActionId("sprite.new"))`.
- **Palette** (line 961): name label `Bit`; an 8x4 swatch grid (`widgets::placeholder` or 32 `ui.painter().rect_filled` cells); FG/BG indicator; buttons `Ramp`, `Harmony`, `Reduce to palette`.
- **Selection Actions** (line 963): button row `Cut - Copy - Paste - Invert - Crop`, then an AI-marked sub-row (`icons::SPARKLE` + `accent.ai`) `Fill - Clean up - Make seamless`. Each button pushes `Intent::RunAction`.
- **AI Assistant** (line 964): the quick-action list `Fill selection`, `Clean up`, `Reduce colors`, `Suggest ramp`, `Create variations`, `Remove background` - each a full-width button pushing `Intent::RunAction` with a distinct `ActionId`. Header marked with `icons::SPARKLE` + `accent.ai`.
- **Frames** (tray, line 966): horizontal strip of 8 thumbnails (`0..7`), add/duplicate/delete buttons, current frame (index 0) highlighted with `accent.muted`.
- **Assets** (tray, line 985): thumbnail grid of mock assets with category chips (`ui.selectable_label` chips `All - Sprites - Tiles - Refs`, inert).
- **Console** (tray, line 993): scrolling mock log via `widgets::placeholder` mock-log helper or an `egui::ScrollArea::vertical` of monospace `text_secondary` lines `info backend ready`, `info project loaded`.

- [ ] **Step 1: Add the four remaining dock panel impls to `draw.rs`.** Each follows the `LayersPanel` shape: unit struct, `id()` -> the shared const, `meta()` with `Region::RightDock` and `default_open: true`, `ui()` rendering the mock content above. Use `widgets::section_header(ui, theme, icon, title)` for each header and `widgets::placeholder::*` for thumbnail grids / swatch grids where the WIDGETS layer provides them. Declare the `ActionId` consts needed (`SPRITE_NEW`, `PALETTE_RAMP`, the AI quick-action ids, the selection-action ids) at module scope. Full body for the AI Assistant panel (the load-bearing intent path), as the pattern for the rest:

```rust
const AI_FILL: ActionId = ActionId("ai.fill-selection");
const AI_CLEANUP: ActionId = ActionId("ai.clean-up");
const AI_REDUCE: ActionId = ActionId("ai.reduce-colors");
const AI_RAMP: ActionId = ActionId("ai.suggest-ramp");
const AI_VARIATIONS: ActionId = ActionId("ai.create-variations");
const AI_REMOVE_BG: ActionId = ActionId("ai.remove-background");

/// The AI Assistant panel: the UX quick-action list. Each row pushes a RunAction
/// intent (mock toast + JobStub). Header marked with the sparkle.
pub struct AiAssistantPanel;

impl Panel for AiAssistantPanel {
    fn id(&self) -> PanelId {
        AI_ASSISTANT
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: "AI Assistant",
            icon: icons::SPARKLE,
            default_region: Region::RightDock,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(icons::SPARKLE.to_string()).color(theme.accent.ai));
            ui.label("AI Assistant");
        });
        let actions = [
            ("Fill selection", AI_FILL),
            ("Clean up", AI_CLEANUP),
            ("Reduce colors", AI_REDUCE),
            ("Suggest ramp", AI_RAMP),
            ("Create variations", AI_VARIATIONS),
            ("Remove background", AI_REMOVE_BG),
        ];
        for (label, action) in actions {
            if ui.add_sized([ui.available_width(), 24.0], egui::Button::new(label)).clicked() {
                scope.ctx.intents.push(Intent::RunAction(action));
            }
        }
    }
}
```

Implement `SpritesPanel`, `PalettePanel`, and `SelectionActionsPanel` in the same shape with their mock content. For checkerboard thumbnails and swatch grids, prefer `widgets::placeholder` helpers; if a needed helper is absent, paint cells directly with `ui.allocate_painter` + `painter.rect_filled` alternating `theme.surfaces.inset`/`theme.surfaces.elevated`.

- [ ] **Step 2: Add the three shared tray panel impls.** `FramesPanel`, `AssetsPanel`, `ConsolePanel` - same shape, `meta().default_region = Region::BottomTray`. Console body:

```rust
/// The Console tray panel: a scrolling mock log, monospace, secondary text.
pub struct ConsolePanel;

impl Panel for ConsolePanel {
    fn id(&self) -> PanelId {
        CONSOLE
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: "Console",
            icon: icons::TERMINAL,
            default_region: Region::BottomTray,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for line in ["info  backend ready", "info  project loaded"] {
                ui.label(
                    egui::RichText::new(line)
                        .monospace()
                        .color(theme.roles.text_secondary),
                );
            }
        });
    }
}
```

- [ ] **Step 3: Complete `draw::register`.** Register the workspace, all five dock panels, the three tray panels, and the Sprite/Layer menu groups. Menu groups are built as data per the SHELL/menus contract (`MenuGroup { label, items: Vec<MenuItem { label, shortcut, action }> }`):

```rust
use pixhaus_ui::contrib_api::{MenuGroup, MenuItem};

/// Register the Draw workspace, the shared dock panels, the shared tray panels,
/// and the Sprite/Layer menu groups. Order matters: this runs first (sprite-edit
/// is registered first), so the shared panels exist before any other workspace's
/// layout references them by id (bible rule 2).
pub fn register(host: &mut dyn HostRegistrar) {
    host.add_workspace(Box::new(DrawWorkspace));

    // Shared dock panels.
    host.add_panel(Box::new(LayersPanel));
    host.add_panel(Box::new(SpritesPanel));
    host.add_panel(Box::new(PalettePanel));
    host.add_panel(Box::new(SelectionActionsPanel));
    host.add_panel(Box::new(AiAssistantPanel));

    // Shared tray panels.
    host.add_panel(Box::new(FramesPanel));
    host.add_panel(Box::new(AssetsPanel));
    host.add_panel(Box::new(ConsolePanel));

    // Menu groups this module owns.
    host.add_menu_group(MenuGroup {
        label: "Sprite",
        items: vec![
            MenuItem { label: "New Sprite", shortcut: None, action: SPRITE_NEW },
            MenuItem { label: "Resize Canvas", shortcut: None, action: ActionId("sprite.resize") },
        ],
    });
    host.add_menu_group(MenuGroup {
        label: "Layer",
        items: vec![
            MenuItem { label: "New Layer", shortcut: None, action: NEW_LAYER },
            MenuItem { label: "Delete Layer", shortcut: None, action: ActionId("layer.delete") },
            MenuItem { label: "Merge Down", shortcut: None, action: ActionId("layer.merge-down") },
        ],
    });
}
```

Confirm `MenuGroup`/`MenuItem` field shapes against the SHELL layer's `menus.rs` contract; the spec (line 998) defines `MenuGroup { label, items: Vec<MenuItem { label, shortcut, action: ActionId }> }`. If `shortcut` is typed `Option<KeyboardShortcut>`, `None` is correct; match the actual type.

- [ ] **Step 4: Add registration coverage and a uniqueness test.** Append to the `tests` module in `draw.rs` - assert each shared panel's id and region, asserting the contract the other modules depend on:

```rust
#[test]
fn shared_panel_ids_and_regions() {
    assert_eq!(SpritesPanel.id(), SPRITES);
    assert_eq!(PalettePanel.id(), PALETTE);
    assert_eq!(SelectionActionsPanel.id(), SELECTION_ACTIONS);
    assert_eq!(AiAssistantPanel.id(), AI_ASSISTANT);
    assert_eq!(FramesPanel.meta().default_region, Region::BottomTray);
    assert_eq!(AssetsPanel.meta().default_region, Region::BottomTray);
    assert_eq!(ConsolePanel.meta().default_region, Region::BottomTray);
}
```

- [ ] **Step 5: Run the tests.**

```powershell
cargo nextest run -p pixhaus-mod-sprite-edit
```

Expected: PASS.

- [ ] **Step 6: Commit.**

```powershell
git add modules/sprite-edit/src/draw.rs
git commit -m @'
feat(sprite-edit): register the shared dock and tray panels and menus

Adds Sprites, Palette, Selection Actions, and AI Assistant dock panels and
the Frames, Assets, and Console tray panels with their inventory mock
content, plus the Sprite and Layer menu groups. register wires them all,
running first so the other workspaces reference the shared panels by id.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### MODULES.7: The Animation module - Animate workspace and its panels

**Files:**
- Create: `modules/animation/src/lib.rs`
- Create: `modules/animation/src/animate.rs`

AnimationModule registers the Animate workspace, the Clip Properties and AI Animation Assistant dock panels, the Timeline tray panel (it owns Timeline), and the Frame menu group (spec line 1005). Animate's layout (inventory line 944) reuses sprite-edit's shared panels by id: dock `Layers, Sprites, Frames, Clip Properties, AI Animation Assistant`; tray `Timeline, Frames, Console`; full 15-tool rail; status items `15 frames`, `Onion Skin Off`, `12 FPS`. Note `Frames` appears in both dock and tray - it is the same shared panel id referenced twice; the shell resolves it in each region.

The Timeline panel (spec line 982) is the four-band `Painter` layout and gets explicit geometry. Implement it following the worked Layers/Console examples for the panel shell, with a custom `ui()` body.

- [ ] **Step 1: Scaffold `animation/src/lib.rs`.**

```rust
//! Pixhaus animation module: the Animate workspace and the timeline.
//!
//! Registers the Animate workspace, the Clip Properties and AI Animation
//! Assistant dock panels, the Timeline tray panel, and the Frame menu group
//! (architecture bible section 7.3). Animate reuses sprite-edit's shared panels
//! (Layers, Sprites, Frames, Console) by id - it is editing in space over time
//! atop the same sprite-editing core (bible rule 2).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod animate;

use pixhaus_ui::contrib_api::{HostRegistrar, Module};

/// The animation module.
pub struct AnimationModule;

impl Module for AnimationModule {
    fn id(&self) -> &'static str {
        "animation"
    }

    fn register(&self, host: &mut dyn HostRegistrar) {
        animate::register(host);
    }
}
```

- [ ] **Step 2: Write `animate.rs` - the workspace, the two dock panels, the Timeline tray panel, the Frame menu, and `register`.** Workspace (Cmd+2, full rail, default Pencil - the shared editing core):

```rust
//! The Animate workspace, its panels, and the timeline.

use egui::{Key, KeyboardShortcut, Modifiers, Sense, Stroke, Vec2};
use pixhaus_ui::contrib_api::{
    ActionId, HostRegistrar, MenuGroup, MenuItem, Panel, PanelId, PanelMeta, PanelScope,
    Workspace, WorkspaceId, WorkspaceLayout, WorkspaceMeta,
};
use pixhaus_ui::region::Region;
use pixhaus_ui::state::{Intent, StatusItem};
use pixhaus_ui::{icons, widgets};

pub const ANIMATE: WorkspaceId = WorkspaceId("animate");

// Animation-owned panel ids.
pub const CLIP_PROPERTIES: PanelId = PanelId("clip-properties");
pub const AI_ANIM_ASSISTANT: PanelId = PanelId("ai-animation-assistant");
pub const TIMELINE: PanelId = PanelId("timeline");

// Shared panels owned by sprite-edit, referenced by id.
const LAYERS: PanelId = PanelId("layers");
const SPRITES: PanelId = PanelId("sprites");
const FRAMES: PanelId = PanelId("frames");
const CONSOLE: PanelId = PanelId("console");

// The shared 15-tool rail (default tool is the shared Pencil).
const PENCIL: pixhaus_ui::contrib_api::ToolId = pixhaus_ui::contrib_api::ToolId("pencil");

fn full_rail() -> Vec<pixhaus_ui::contrib_api::ToolId> {
    use pixhaus_ui::contrib_api::ToolId;
    [
        "pencil", "eraser", "fill", "line", "rectangle", "ellipse", "eyedropper", "selection",
        "lasso", "move", "transform", "text", "hand", "zoom", "ai-brush",
    ]
    .into_iter()
    .map(ToolId)
    .collect()
}

/// The Animate workspace: editing the sprite over time. Reuses the shared editing
/// panels by id (bible rule 2).
pub struct AnimateWorkspace;

impl Workspace for AnimateWorkspace {
    fn id(&self) -> WorkspaceId {
        ANIMATE
    }

    fn meta(&self) -> WorkspaceMeta {
        WorkspaceMeta {
            name: "Animate",
            icon: icons::FILM,
            purpose: "Animate the sprite across frames",
            shortcut: KeyboardShortcut::new(Modifiers::COMMAND, Key::Num2),
        }
    }

    fn layout(&self) -> WorkspaceLayout {
        WorkspaceLayout {
            right_dock: vec![LAYERS, SPRITES, FRAMES, CLIP_PROPERTIES, AI_ANIM_ASSISTANT],
            bottom_tray: vec![TIMELINE, FRAMES, CONSOLE],
            primary_tools: full_rail(),
            default_tool: PENCIL,
            status_items: vec![
                StatusItem { icon: icons::FILM, text: "15 frames".to_owned() },
                StatusItem { icon: icons::EYE, text: "Onion Skin Off".to_owned() },
                StatusItem { icon: icons::CLOCK, text: "12 FPS".to_owned() },
            ],
        }
    }
}
```

- [ ] **Step 3: Add the Clip Properties and AI Animation Assistant dock panels.** Clip Properties mock content (line 967): `Clip jump - Frames 8-15 - FPS 12 - Loop [ ] - Export name bit_jump`. AI Animation Assistant is the AI quick-action list variant (line 964): rows `In-between frames`, `Smooth motion`, `Generate walk cycle`, `Loop seamlessly`, each pushing `Intent::RunAction`, header sparkle-marked. Both follow the `LayersPanel`/`AiAssistantPanel` shape; `default_region: Region::RightDock`. For the Clip Properties `Export name` field, the spec keeps panels read-only except scratch - render the export name as a label `bit_jump`, not an editable field (only the Prompt panel owns a scratch buffer; if an editable clip name is wanted later it gets its own intent). Keep `Loop [ ]` an inert checkbox.

- [ ] **Step 4: Add the Timeline tray panel with the four-band Painter layout.** This is the spec's explicit geometry (line 982). The body allocates a painter for the full available rect and partitions it top-to-bottom into four horizontal bands: Playback, Animation clips, Frame ruler + playhead, Layer tracks.

```rust
/// The Timeline tray panel: the four-band animation timeline drawn with a Painter.
/// Bands top-to-bottom: Playback controls, Animation clips, Frame ruler with the
/// playhead, Layer tracks. All content is mock; the Animate reference frame is the
/// target.
pub struct TimelinePanel;

impl Panel for TimelinePanel {
    fn id(&self) -> PanelId {
        TIMELINE
    }

    fn meta(&self) -> PanelMeta {
        PanelMeta {
            title: "Timeline",
            icon: icons::FILM,
            default_region: Region::BottomTray,
            default_open: true,
        }
    }

    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>) {
        let theme = scope.ctx.theme;

        // Band 1 - Playback: real widgets in a horizontal row (interactive controls).
        ui.horizontal(|ui| {
            let _ = ui.button(icons::PLAY.to_string());
            let _ = ui.button("prev");
            let _ = ui.button("next");
            ui.label("100ms");
            ui.label("1.00x");
            ui.label("12 FPS");
            let mut looping = false;
            ui.checkbox(&mut looping, "Loop");
        });

        // Bands 2-4 are painted: clips, the frame ruler + playhead, layer tracks.
        let desired = Vec2::new(ui.available_width(), 120.0);
        let (rect, _resp) = ui.allocate_exact_size(desired, Sense::hover());
        let painter = ui.painter_at(rect);
        let band_h = rect.height() / 3.0;

        // Band 2 - Animation clips: named spans.
        let clips_top = rect.top();
        let clips = ["idle", "walk", "run", "jump", "attack"];
        let clip_w = rect.width() / clips.len() as f32;
        for (i, name) in clips.iter().enumerate() {
            let x = rect.left() + i as f32 * clip_w;
            let span = egui::Rect::from_min_size(
                egui::pos2(x + 2.0, clips_top + 2.0),
                Vec2::new(clip_w - 4.0, band_h - 4.0),
            );
            painter.rect_filled(span, theme.radius.sm, theme.surfaces.inset);
            painter.text(
                span.left_center() + Vec2::new(4.0, 0.0),
                egui::Align2::LEFT_CENTER,
                name,
                egui::FontId::proportional(theme.type_scale.label),
                theme.roles.text_secondary,
            );
        }

        // Band 3 - Frame ruler 0..14 with the violet playhead at frame 11.
        let ruler_top = clips_top + band_h;
        let frames = 15;
        let frame_w = rect.width() / frames as f32;
        for f in 0..frames {
            let x = rect.left() + f as f32 * frame_w;
            painter.line_segment(
                [egui::pos2(x, ruler_top), egui::pos2(x, ruler_top + band_h)],
                Stroke::new(1.0, theme.roles.border),
            );
            painter.text(
                egui::pos2(x + 2.0, ruler_top + 2.0),
                egui::Align2::LEFT_TOP,
                f.to_string(),
                egui::FontId::monospace(theme.type_scale.label),
                theme.roles.text_secondary,
            );
        }
        let playhead_x = rect.left() + 11.0 * frame_w;
        painter.line_segment(
            [egui::pos2(playhead_x, ruler_top), egui::pos2(playhead_x, rect.bottom())],
            Stroke::new(2.0, theme.accent.base),
        );

        // Band 4 - Layer tracks.
        let tracks_top = ruler_top + band_h;
        for (i, track) in ["Body", "Effects", "Shadow"].iter().enumerate() {
            let y = tracks_top + i as f32 * (band_h / 3.0);
            painter.text(
                egui::pos2(rect.left() + 2.0, y),
                egui::Align2::LEFT_TOP,
                *track,
                egui::FontId::proportional(theme.type_scale.label),
                theme.roles.text_secondary,
            );
        }
    }
}
```

Confirm `theme.radius.sm`, `theme.type_scale.label`, `theme.accent.base`, `theme.surfaces.inset`, `theme.roles.border/text_secondary` field paths against the THEME layer's `tokens.rs` (they match the spec tokens at lines 736-773). `egui::Align2`, `egui::FontId::{proportional,monospace}`, `Painter::{rect_filled,text,line_segment}`, `ui.allocate_exact_size`, `ui.painter_at` are all 0.34.2 API.

- [ ] **Step 5: Write `register` and add the Frame menu group.**

```rust
const FRAME_ADD: ActionId = ActionId("frame.add");

/// Register the Animate workspace, its dock panels, the Timeline tray panel, and
/// the Frame menu group. The shared Layers/Sprites/Frames/Console panels are
/// owned by sprite-edit and referenced by id, not re-registered here.
pub fn register(host: &mut dyn HostRegistrar) {
    host.add_workspace(Box::new(AnimateWorkspace));
    host.add_panel(Box::new(ClipPropertiesPanel));
    host.add_panel(Box::new(AiAnimationAssistantPanel));
    host.add_panel(Box::new(TimelinePanel));
    host.add_menu_group(MenuGroup {
        label: "Frame",
        items: vec![
            MenuItem { label: "Add Frame", shortcut: None, action: FRAME_ADD },
            MenuItem { label: "Duplicate Frame", shortcut: None, action: ActionId("frame.duplicate") },
            MenuItem { label: "Delete Frame", shortcut: None, action: ActionId("frame.delete") },
        ],
    });
}
```

- [ ] **Step 6: Add tests.**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animate_reuses_shared_panels_by_id() {
        let layout = AnimateWorkspace.layout();
        assert_eq!(layout.right_dock, vec![LAYERS, SPRITES, FRAMES, CLIP_PROPERTIES, AI_ANIM_ASSISTANT]);
        assert_eq!(layout.bottom_tray, vec![TIMELINE, FRAMES, CONSOLE]);
        assert_eq!(layout.primary_tools.len(), 15);
        assert_eq!(layout.status_items.len(), 3);
    }

    #[test]
    fn animate_meta_uses_cmd_2() {
        assert_eq!(
            AnimateWorkspace.meta().shortcut,
            KeyboardShortcut::new(Modifiers::COMMAND, Key::Num2)
        );
        assert_eq!(TimelinePanel.id(), TIMELINE);
    }
}
```

- [ ] **Step 7: Run, clippy, commit.**

```powershell
cargo nextest run -p pixhaus-mod-animation
cargo clippy -p pixhaus-mod-animation --all-targets -- -D warnings
```

Expected: both PASS/clean.

```powershell
git add modules/animation/src/lib.rs modules/animation/src/animate.rs
git commit -m @'
feat(animation): add Animate workspace, panels, and the timeline

Animate reuses sprite-edit's shared panels by id and adds Clip Properties,
AI Animation Assistant, and the four-band Timeline tray panel (playback,
clips, frame ruler with the violet playhead, layer tracks) drawn with a
Painter. Contributes the Frame menu group.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### MODULES.8: The Tiles module - Tiles workspace and its panels

**Files:**
- Create: `modules/tiles/src/lib.rs`
- Create: `modules/tiles/src/tiles_ws.rs`

TilesModule registers the Tiles workspace, the Tileset / Rule Type / Material / Seam QA / AI Tile Assistant dock panels, and the Tile Variants tray panel (it owns Tile Variants). Layout (inventory line 945): dock `Tileset, Rule Type, Material, Seam QA, AI Tile Assistant`; tray `Tile Variants, Assets, Console`; full 15-tool rail; default Pencil; status items `Tile 16px`, `Seams OK`. Assets and Console are sprite-edit shared panels referenced by id.

- [ ] **Step 1: Scaffold `tiles/src/lib.rs`** (same shape as MODULES.7 step 1, module `tiles_ws`, struct `TilesModule`, `id()` -> `"tiles"`, `register` calls `tiles_ws::register`). Doc comment: "the Tiles workspace and tileset authoring (bible 7.3)."

- [ ] **Step 2: Write `tiles_ws.rs` - the workspace.** Cmd+3, full rail (reuse the `full_rail()` helper pattern from MODULES.7), default Pencil, icon `icons::SQUARES_FOUR` (or nearest grid glyph), status items as above. Owned panel ids: `TILESET`, `RULE_TYPE`, `MATERIAL`, `SEAM_QA`, `AI_TILE_ASSISTANT`, `TILE_VARIANTS`; shared `ASSETS`, `CONSOLE` referenced by id.

- [ ] **Step 3: Implement the six panels** following the worked examples, mock content per spec lines 968-970, 984:
  - **Tileset**: a 4x4 tile grid (checkerboard rects), header + `+ New Tile` button -> `Intent::RunAction`.
  - **Rule Type**: a radio group `Single - Seamless - 3x3 Autotile - 47-blob` via `ui.selectable_value` over a throwaway local enum/index; inert.
  - **Material**: a row of material chips (`ui.selectable_label`, inert): `Grass`, `Stone`, `Water`, `Sand`.
  - **Seam QA**: a checklist with `success`/`error` badges - `OK Top`, `OK Left`, `WARN Bottom seam`. Render each row: a colored dot (`theme.roles.success` for OK, `theme.roles.error`/`warning` for WARN) + label.
  - **AI Tile Assistant**: the AI quick-action list variant - `Make seamless`, `Generate variations`, `Suggest material`, `Fix seams` - each pushing `Intent::RunAction`, header sparkle-marked.
  - **Tile Variants** (tray, `Region::BottomTray`): a row of mock tile patches + a seamless-tiling preview block (a 2x2 repeated checkerboard).

- [ ] **Step 4: Write `register`** (workspace + the six panels; no menu group required - tiles contributes none per spec). Tiles does not contribute a menu group, so do not call `add_menu_group`.

- [ ] **Step 5: Add tests** (layout matches inventory: dock 5, tray `[TILE_VARIANTS, ASSETS, CONSOLE]`, Cmd+3, 15 tools, 2 status items; panel ids/regions).

- [ ] **Step 6: Run, clippy, commit.**

```powershell
cargo nextest run -p pixhaus-mod-tiles
cargo clippy -p pixhaus-mod-tiles --all-targets -- -D warnings
```

Expected: PASS/clean.

```powershell
git add modules/tiles/src/lib.rs modules/tiles/src/tiles_ws.rs
git commit -m @'
feat(tiles): add Tiles workspace and tileset panels

Adds the Tiles workspace (full rail, Tile 16px / Seams OK status) and its
panels: Tileset grid, Rule Type radios, Material chips, Seam QA checklist
with success/warning badges, AI Tile Assistant quick actions, and the Tile
Variants tray panel with a seamless-tiling preview. Reuses Assets and
Console by id.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### MODULES.9: Complete the Generation module - the remaining dock and tray panels

**Files:**
- Modify: `modules/generation/src/generate.rs`

MODULES.4 landed GenerateWorkspace and the Prompt panel. Add the five remaining dock panels (Recipe, Structure, Style, Palette Behavior, Advanced Settings) and the two tray panels Generation owns (Results, History), then complete `register`. Generation contributes no menu group this round (spec line 1005 names only Pixhaus/File/Edit/Select/View/Window/Help as shell-owned and Sprite/Layer/Frame from sprite-edit/animation).

Mock content per spec lines 975-980:
- **Recipe / Structure / Style**: card lists with mock preview thumbnails. Recipe rows show a built-in (locked) vs user badge: render each row with a thumbnail rect + name + a small chip label `Built-in` (tinted `text_disabled`) or `User` (tinted `accent.base`).
- **Palette Behavior** (line 977): the checkbox set - `Use current palette only [x]`, `Add colors automatically [ ]`, `Reduce to palette on apply [x]`, `Dither gradients [ ]`. Inert checkboxes.
- **Advanced Settings** (line 978): `default_open: false` (collapsed by default). Rows `Seed`, `Steps`, `Strength`, `Negative prompt`, `Model` - labels + inert `DragValue`/label widgets.
- **Results** (tray, line 979): 8 mock result cards (a grid of numbered thumbnails; each shows a number, seed, a sparkle, a star; the selected card, index 0, gets an `accent`-colored ring via `painter.rect_stroke`). Below the grid, an action button row `Use selected - Insert as new sprite - Create variations - Generate more`, each pushing `Intent::RunAction`.
- **History** (tray, line 980): a list of prior mock generations - rows `prompt summary - seed - timestamp` as monospace `text_secondary` labels.

- [ ] **Step 1: Implement the five remaining dock panels** in `generate.rs` following the worked Prompt/Layers shapes. Declare the needed `ActionId` consts. Advanced Settings sets `default_open: false`; all others `true`.

- [ ] **Step 2: Implement the Results and History tray panels** (`Region::BottomTray`). Results' selected-card ring uses `painter.rect_stroke(rect, radius, Stroke::new(2.0, theme.accent.base), egui::epaint::StrokeKind::Outside)` - confirm `rect_stroke`'s arity against the painting reference if it errors (0.34.2 `Painter::rect_stroke(rect, corner_radius, stroke, stroke_kind)`).

- [ ] **Step 3: Complete `register`.**

```rust
pub fn register(host: &mut dyn HostRegistrar) {
    host.add_workspace(Box::new(GenerateWorkspace));
    host.add_panel(Box::new(PromptPanel));
    host.add_panel(Box::new(RecipePanel));
    host.add_panel(Box::new(StructurePanel));
    host.add_panel(Box::new(StylePanel));
    host.add_panel(Box::new(PaletteBehaviorPanel));
    host.add_panel(Box::new(AdvancedSettingsPanel));
    host.add_panel(Box::new(ResultsPanel));
    host.add_panel(Box::new(HistoryPanel));
}
```

- [ ] **Step 4: Extend the tests** - assert `AdvancedSettingsPanel.meta().default_open == false`, the Results/History ids and `Region::BottomTray`, and the full dock id order.

- [ ] **Step 5: Run, clippy, commit.**

```powershell
cargo nextest run -p pixhaus-mod-generation
cargo clippy -p pixhaus-mod-generation --all-targets -- -D warnings
```

Expected: PASS/clean.

```powershell
git add modules/generation/src/generate.rs
git commit -m @'
feat(generation): add the recipe/structure/style and results panels

Completes the Generate workspace: Recipe/Structure/Style card lists with
built-in vs user badges, the Palette Behavior checkbox set, collapsed
Advanced Settings, the Results tray grid with a selected-card ring and the
action row, and the History tray list.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### MODULES.10: The Export module - Export workspace and its panels

**Files:**
- Create: `modules/export/src/lib.rs`
- Create: `modules/export/src/export_ws.rs`

ExportModule registers the Export workspace, the Export Format / Engine Preset / Animation Metadata / QA Warnings dock panels, and the Export Log tray panel (it owns Export Log). Layout (inventory line 947): dock `Export Format, Engine Preset, Animation Metadata, QA Warnings`; tray `Export Log, Console`; tools `{Hand, Zoom}` only; default Hand; status items `PNG + sheet`, `0 warnings`. Console is shared (sprite-edit) referenced by id. Export contributes no menu group.

- [ ] **Step 1: Scaffold `export/src/lib.rs`** (module `export_ws`, struct `ExportModule`, `id()` -> `"export"`, `register` -> `export_ws::register`). Doc comment: "the Export workspace and engine-target export (bible 7.3)."

- [ ] **Step 2: Write `export_ws.rs` - the workspace.** Cmd+5, icon `icons::EXPORT` (or nearest), tools `vec![HAND, ZOOM]` (consts referencing the shared ids), default Hand, status items `PNG + sheet`, `0 warnings`. Owned panel ids: `EXPORT_FORMAT`, `ENGINE_PRESET`, `ANIMATION_METADATA`, `QA_WARNINGS`, `EXPORT_LOG`; shared `CONSOLE`.

- [ ] **Step 3: Implement the five panels** per spec lines 989-993:
  - **Export Format**: radio `PNG - Spritesheet - GIF - APNG - JSON` via `ui.selectable_value` over a throwaway local index; inert.
  - **Engine Preset**: `Unity` highlighted (tinted `accent.muted` background or accent text), others listed as future (`text_disabled`): `Godot (soon)`, `Generic (soon)`. Unity is the only enabled row.
  - **Animation Metadata**: checkbox/value set `Per-animation export [x] - Trim [x] - Padding 2 - Pivot Center`. Inert checkboxes + a `DragValue` for Padding + a label for Pivot.
  - **QA Warnings** (line 990): the UX checklist with `success`/`warning` badges: `OK All frames same size`, `OK Transparent bg`, `OK Palette < 32`, `WARN "jump" does not loop`, `WARN 2 missing animations`. Each WARN row gets `Fix` and `Ignore` buttons that push `Intent::RunAction`.
  - **Export Log** (tray, `Region::BottomTray`): a scrolling mock log like Console - monospace `text_secondary` lines `info  exporter ready`, `info  Unity preset selected`.

- [ ] **Step 4: Write `register`** (workspace + five panels; no menu group).

- [ ] **Step 5: Add tests** (layout: dock 4, tray `[EXPORT_LOG, CONSOLE]`, tools `[HAND, ZOOM]`, default Hand, Cmd+5, 2 status items).

- [ ] **Step 6: Run, clippy, commit.**

```powershell
cargo nextest run -p pixhaus-mod-export
cargo clippy -p pixhaus-mod-export --all-targets -- -D warnings
```

Expected: PASS/clean.

```powershell
git add modules/export/src/lib.rs modules/export/src/export_ws.rs
git commit -m @'
feat(export): add Export workspace and export panels

Adds the Export workspace (Hand/Zoom rail, PNG + sheet / 0 warnings
status) and its panels: Export Format radios, Engine Preset with Unity
highlighted and others marked future, Animation Metadata controls, the QA
Warnings checklist with Fix/Ignore actions, and the Export Log tray panel.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### MODULES.11: Wire app/Cargo.toml - the five module dependencies

**Files:**
- Modify: `app/Cargo.toml`

`app` depends on everything; add the five `pixhaus-mod-*` crates so `build_host` can name the module structs.

- [ ] **Step 1: Add the module deps.** Edit `app/Cargo.toml`'s `[dependencies]` block, inserting after the `pixhaus-ui` line:

```toml
pixhaus-ui = { path = "../crates/ui" }
pixhaus-mod-sprite-edit = { path = "../modules/sprite-edit" }
pixhaus-mod-animation = { path = "../modules/animation" }
pixhaus-mod-tiles = { path = "../modules/tiles" }
pixhaus-mod-generation = { path = "../modules/generation" }
pixhaus-mod-export = { path = "../modules/export" }
```

- [ ] **Step 2: Verify resolution.**

```powershell
cargo metadata --no-deps --format-version 1 | Out-Null; if ($?) { "ok" }
```

Expected: prints `ok`. A path error means a `../modules/<name>` is wrong - `app/` sits at the repo root, so `../modules/...` is correct.

- [ ] **Step 3: Commit.**

```powershell
git add app/Cargo.toml
git commit -m @'
chore(app): depend on the five shell module crates

build_host names the sprite-edit, animation, tiles, generation, and export
module structs; the binary depends on everything (bible 4.1).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### MODULES.12: Rewrite app/src/main.rs to the spec shape

**Files:**
- Modify: `app/src/main.rs`

Reshape `main.rs` to the spec (lines 144-187): `build_host(ctx)` registering the five module structs in the contract order (sprite-edit FIRST), `PixhausApp { host }`, `impl eframe::App` with `logic()` = `drain_background` and `ui()` = `Shell::run`, keeping `install_canvas_renderer` and the tokio runtime boot. The old hardcoded top-bar + central-panel UI is removed - the shell now draws everything from the registries.

- [ ] **Step 1: Replace the file.** This keeps the tokio boot, `NativeOptions`, `run_native`, the tracing init, and `install_canvas_renderer` exactly; it swaps the UI body for the registry-driven shell:

```rust
//! Pixhaus application binary: the eframe + egui host shell.
//!
//! The Host App layer (architecture bible section 4.1). It owns the single tokio
//! runtime, boots the window, registers the capability modules, and runs the egui
//! loop. The shell draws every region from the registries; this binary declares no
//! layout - it names the modules and nothing else.

use anyhow::Context as _;
use eframe::egui;

/// Build the host: register the five capability modules and apply the theme.
///
/// Registration order is the contract. sprite-edit registers first because it owns
/// the shared Layers/Sprites/Palette/AI Assistant dock panels and the shared
/// Frames/Assets/Console tray panels; the other workspaces reference those by id
/// (bible rule 2). Module registration is the only path a capability enters the
/// shell.
fn build_host(ctx: &egui::Context) -> pixhaus_ui::state::Host {
    let mut host = pixhaus_ui::state::Host::new(pixhaus_ui::theme::Theme::dark());

    let modules: [Box<dyn pixhaus_ui::contrib_api::Module>; 5] = [
        Box::new(pixhaus_mod_sprite_edit::SpriteEditModule),
        Box::new(pixhaus_mod_animation::AnimationModule),
        Box::new(pixhaus_mod_tiles::TilesModule),
        Box::new(pixhaus_mod_generation::GenerationModule),
        Box::new(pixhaus_mod_export::ExportModule),
    ];
    for m in &modules {
        m.register(&mut host.registrar());
    }

    pixhaus_ui::theme::apply_to_visuals(host.theme(), ctx);
    pixhaus_ui::theme::install_fonts(ctx);
    host
}

/// Top-level application state, owned across frames by the eframe loop. The host
/// holds the registries, session and UI state, the intent sink, the theme, and the
/// background channel.
struct PixhausApp {
    host: pixhaus_ui::state::Host,
}

impl PixhausApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        if let Some(render_state) = cc.wgpu_render_state.as_ref() {
            pixhaus_ui::install_canvas_renderer(render_state);
        }
        Self { host: build_host(&cc.egui_ctx) }
    }
}

impl eframe::App for PixhausApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Per-frame non-draw work: fold background channel results into session
        // state and request a repaint when something landed. Runs even when the
        // window is occluded but a repaint was requested.
        pixhaus_ui::shell::drain_background(&mut self.host, ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        pixhaus_ui::shell::Shell::run(&mut self.host, ui);
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // The binary owns the single tokio runtime; entering it makes tokio::spawn
    // available to the egui loop for the background work the editor will grow.
    let runtime = tokio::runtime::Runtime::new().context("failed to start the tokio runtime")?;
    let _guard = runtime.enter();

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title("Pixhaus")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native("pixhaus", options, Box::new(|cc| Ok(Box::new(PixhausApp::new(cc)))))?;

    Ok(())
}
```

Notes on the exact API surface, all confirmed against the spec contract and pinned 0.34.2:
- `Host::new(Theme)`, `host.registrar() -> &mut dyn HostRegistrar`, `host.theme() -> &Theme` are the STATE-layer contract (spec lines 146-164, 520-528). If `registrar()` is named differently or returns a wrapper by value, match the STATE layer's actual signature - the spec shows `&mut host.registrar()`.
- `pixhaus_ui::theme::{Theme, apply_to_visuals, install_fonts}` (spec lines 796, 161-163).
- `pixhaus_ui::shell::{Shell, drain_background}` (spec lines 180, 184, 635).
- `pixhaus_ui::contrib_api::Module`, `pixhaus_ui::state::Host` (spec lines 146, 151).
- The `use anyhow::Context as _;` import: the trait is used only for `.context(...)`; the `as _` avoids colliding with `egui::Context`. (The old file imported `anyhow::Context`; renaming to `Context as _` keeps both unambiguous.)

- [ ] **Step 2: Build the whole workspace.** This is the integration point - every layer must be present.

```powershell
cargo build --workspace
```

Expected: PASS. If `Host::new`/`registrar`/`theme`/`Shell::run`/`drain_background` do not resolve, the STATE or SHELL layer is not merged yet - those are this layer's dependencies, not bugs here. If a `pixhaus_mod_*::*Module` does not resolve, re-check the module crate name mapping (`pixhaus-mod-sprite-edit` crate -> `pixhaus_mod_sprite_edit` path).

- [ ] **Step 3: Clippy the binary.**

```powershell
cargo clippy -p pixhaus-app --all-targets -- -D warnings
```

Expected: clean.

- [ ] **Step 4: Commit.**

```powershell
git add app/src/main.rs
git commit -m @'
feat(app): drive the shell from build_host and the registries

main.rs now builds the host by registering the five modules (sprite-edit
first, the registration-order contract), keeps install_canvas_renderer and
the tokio boot, and runs the registry-driven shell: logic drains the
background channel, ui runs Shell::run. The binary declares no layout.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

---

### MODULES.13: Workspace-wide verification and the manual-verify checklist

**Files:**
- Test: whole workspace (no new source)

Final gate for this layer: the full build passes, every module's tests pass, doc tests pass, and the shell opens and shows each workspace's content. The registry/layout/smoke tests (spec test plan items 1, 2, 7) live in the SHELL/REGISTRY layers; this step confirms the modules feed them correctly by running the whole suite.

- [ ] **Step 1: Build and test the whole workspace.**

```powershell
cargo build --workspace
cargo nextest run --workspace
cargo test --doc --workspace
```

Expected: build PASS, all nextest tests PASS (including the SHELL layer's layout-resolution snapshot test, which requires all five modules registered - spec test 2, line 1053), doc tests PASS. If the snapshot test fails with a diff, inspect it: a diff here means a panel id or layout order in this layer disagrees with the snapshot - reconcile against the inventory table (spec lines 943-947) and accept the snapshot with `cargo insta accept` only if this layer's content is correct.

- [ ] **Step 2: Workspace-wide clippy.**

```powershell
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: clean.

- [ ] **Step 3: Launch the shell and manually verify.**

```powershell
cargo run -p pixhaus-app
```

Expected: a 1280x800 dark-violet window opens. Verify against this per-workspace checklist (switch workspaces with the top-bar tabs or Cmd/Ctrl+1..5):

- **Window chrome (all workspaces):** top bar shows the menu strip (Pixhaus, File, Edit, Sprite, Layer, Frame, Select, View, Window, Help), five workspace tabs (Draw, Animate, Tiles, Generate, Export) with the active one as a violet pill, and a global status strip. Left rail shows tool icons. Status bar at the bottom shows size/zoom/grid + workspace status items + an AI status dot. Center shows the framed artboard over the checkerboard with the wgpu canvas embedded and the HUD chip (`64 x 64   1600%   Grid 8px   Palette: Bit`).
- **Draw (Cmd+1, default):** right dock card stack = Layers (rows + opacity sliders, first row tinted), Sprites (6 thumbnails), Palette (swatch grid), Selection Actions (button rows), AI Assistant (6 quick-action buttons). Bottom tray tabs = Frames / Assets / Console; default tab Frames shows the thumbnail strip. Left rail shows all 15 tools; AI Brush is violet with a sparkle. Status item `Pixel Grid On`.
- **Animate (Cmd+2):** dock = Layers, Sprites, Frames, Clip Properties, AI Animation Assistant. Tray tabs = Timeline / Frames / Console; Timeline shows the four bands (playback row, clip spans, frame ruler 0..14 with the violet playhead at 11, layer tracks Body/Effects/Shadow). Status items `15 frames`, `Onion Skin Off`, `12 FPS`.
- **Tiles (Cmd+3):** dock = Tileset, Rule Type, Material, Seam QA, AI Tile Assistant. Seam QA shows green OK rows and a warning WARN row. Tray tabs = Tile Variants / Assets / Console. Status items `Tile 16px`, `Seams OK`.
- **Generate (Cmd+4):** dock = Prompt (multiline text box that accepts typing - the scratch buffer - and an accent Generate button), Recipe, Structure, Style, Palette Behavior, Advanced Settings (collapsed). Left rail = only Hand, Zoom, Selection, AI Brush. Tray tabs = Results / History / Console; Results shows the 8-card grid with the first card ringed. Status items show an AI Ready dot and `Seed 123456`.
- **Export (Cmd+5):** dock = Export Format (radios), Engine Preset (Unity highlighted, others greyed), Animation Metadata, QA Warnings (OK rows + WARN rows with Fix/Ignore). Left rail = only Hand, Zoom. Tray tabs = Export Log / Console. Status items `PNG + sheet`, `0 warnings`.
- **Cross-cutting:** typing `b` in the Generate Prompt box must NOT switch to the Pencil tool (the focus-gated tool shortcut, via `ctx.egui_wants_keyboard_input()` in the SHELL layer); pressing `b` with no text field focused switches to Pencil. Cmd/Ctrl+K opens the command palette overlay. View > Theme > Light/Dark visibly repaints.

If any workspace shows an empty dock or tray, the layout referenced an unregistered panel - the console (where you launched) prints a `tracing::warn!` naming the missing panel id; the usual cause is a typo between an owned `PanelId` const and the id a workspace lists. Fix the typo so both sides use the identical `&'static str`.

- [ ] **Step 4: No commit.** This step is verification only; nothing changed. If Step 1 or 2 surfaced a fix in this layer's files, commit it under the relevant module's scope with the standard trailer before closing the layer.

## Phase 9: Tests and integration

I have what I need. The focus-query helper is `ctx.wants_keyboard_input()` (flagged for the SHORTCUTS layer; my layer doesn't implement it but my smoke test references workspace names). For my TESTS layer, the key facts are: snapshot path via `insta::assert_debug_snapshot!` (the `ResolvedLayout` derives `Debug`), headless `egui::Context::default()` for tests that need a context, and `resolve_layout` returns ids. Let me verify the egui-phosphor catalog entry isn't already present and confirm the crate names. I have enough from the spec. Let me write my layer.

The spec's test plan items 2, 5, 7 plus final integration are my scope. I have all the type names and signatures. Writing the plan now.

---

## Layer: Test consolidation and final integration

This layer implements the spec's **Test plan** items 2 (full `resolve_layout` insta snapshot across all five workspaces), 5 (the dyn-compatibility compile guard - a verification that the earlier-layer guard is present), and 7 (the headless smoke test), then runs the full Stop-gate, performs a manual-verify pass of the running app, and opens the PR. Depends on the contract types `Registries`, `Host`, `resolve_layout`, `ResolvedLayout`, `WorkspaceId`, `Module` (all from earlier layers; names are the shared contract from spec sec"Trait surface and registries" and sec"Layout resolution"). Branch `feat/ui-shell-foundation` already exists - do not create it.

A note on test placement that holds for every task here: these are integration-style tests of the public `pixhaus-ui` surface plus the `modules/*` registration, so they live in `crates/ui/tests/` (separate test binaries that link `pixhaus-ui` and the five module crates as external consumers - exactly the layout `pixhaus-testing-conventions` prescribes for public-surface tests). The five module crates are dev-dependencies of `pixhaus-ui` for the test build only.

### TESTS.1: Wire module crates as ui dev-dependencies and add the test module-set helper

**Files:**
- Modify: `crates/ui/Cargo.toml`
- Create: `crates/ui/tests/support/mod.rs`
- Test: `crates/ui/tests/support/mod.rs` (the helper itself; exercised by TESTS.2 and TESTS.3)

The five workspaces only exist once all five modules have registered (spec sec"modules/* and app wiring" and sec"Per-workspace placement"). Both the snapshot test and the smoke test need the identical fully-registered `Registries`, so build it once in a shared `tests/support` helper rather than duplicating the registration in two files.

- [ ] **Step 1: Add the five module crates as dev-dependencies of `pixhaus-ui`.** Open `crates/ui/Cargo.toml` and add (or extend) the `[dev-dependencies]` section. The module package names follow the `pixhaus-mod-<name>` convention (spec sec"Repo layout"). `insta` and `rstest` come from the workspace catalog.

```toml
[dev-dependencies]
insta = { workspace = true }
rstest = { workspace = true }
pixhaus-mod-sprite-edit = { path = "../../modules/sprite-edit" }
pixhaus-mod-animation = { path = "../../modules/animation" }
pixhaus-mod-tiles = { path = "../../modules/tiles" }
pixhaus-mod-generation = { path = "../../modules/generation" }
pixhaus-mod-export = { path = "../../modules/export" }
```

- [ ] **Step 2: Write the shared registration helper.** Create `crates/ui/tests/support/mod.rs`. The registration order is the contract (spec sec"app wiring": sprite-edit first so the other workspaces can reference its shared panels by id). The five `WorkspaceId`s match the spec table (`"draw"`, `"animate"`, `"tiles"`, `"generate"`, `"export"` - these are the ids each module's `Workspace::id()` returns; the build agent who implemented MODULES used exactly these strings).

```rust
//! Shared test support: build the fully-registered Host the way `app` does.
//!
//! Registration order is the contract - sprite-edit first, so the other
//! workspaces can reference its shared panels by id (see the design spec,
//! "modules/* and app wiring").

#![allow(dead_code)] // each test binary uses a subset of these helpers

use pixhaus_ui::contrib_api::{Module, WorkspaceId};
use pixhaus_ui::state::Host;
use pixhaus_ui::theme::Theme;

/// The five workspace ids the wired modules register, in declaration order.
pub const WORKSPACE_IDS: [&str; 5] = ["draw", "animate", "tiles", "generate", "export"];

/// The five workspace display names, for the top-bar tab-set assertion.
pub const WORKSPACE_NAMES: [&str; 5] = ["Draw", "Animate", "Tiles", "Generate", "Export"];

/// Build a Host with all five modules registered, exactly as `app::build_host` does.
pub fn fully_registered_host() -> Host {
    let mut host = Host::new(Theme::dark());
    let modules: [Box<dyn Module>; 5] = [
        Box::new(pixhaus_mod_sprite_edit::SpriteEditModule),
        Box::new(pixhaus_mod_animation::AnimationModule),
        Box::new(pixhaus_mod_tiles::TilesModule),
        Box::new(pixhaus_mod_generation::GenerationModule),
        Box::new(pixhaus_mod_export::ExportModule),
    ];
    for m in &modules {
        m.register(&mut host.registrar());
    }
    host
}
```

- [ ] **Step 3: Build the test target to confirm the helper compiles against the real contract.** Run in PowerShell:

```powershell
cargo test -p pixhaus-ui --no-run
```

Expected: PASS (compiles, no tests executed yet). If a `Workspace::id()` string differs from `WORKSPACE_IDS`, or a module struct name differs from the spec, this fails to compile or a later assertion fails - fix `WORKSPACE_IDS`/`WORKSPACE_NAMES` here to match what MODULES actually registered, since the module impls are the ground truth for the id strings.

- [ ] **Step 4: Commit.** Run in PowerShell:

```powershell
git add crates/ui/Cargo.toml crates/ui/tests/support/mod.rs
git commit -m @'
test(ui): add five-module test host helper

Shared test support that registers all five workspace modules in the
contract order app uses, so the snapshot and smoke tests build the same
fully-registered Host. Wires the module crates as ui dev-dependencies.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

### TESTS.2: Full `resolve_layout` insta snapshot for all five workspaces

**Files:**
- Create: `crates/ui/tests/resolve_layout_snapshot.rs`
- Create (generated on first review): `crates/ui/tests/snapshots/resolve_layout_snapshot__*.snap`
- Test: `crates/ui/tests/resolve_layout_snapshot.rs`

This is spec test 2 - the highest-value regression in the round. With all five modules registered, snapshot each workspace's `ResolvedLayout` (`right_dock` ids, `bottom_tray` tab ids, `primary_tools`, `default_tool`, `status_items`). `ResolvedLayout` derives `Debug` (spec sec"Layout resolution"), so `assert_debug_snapshot!` captures the whole struct. A moved panel or a renamed workspace becomes a snapshot diff; an unregistered reference shows as a gap (and the `warn` in `resolve_layout` fires). The spec says this test "Must run with all five modules registered" - the TESTS.1 helper guarantees that.

- [ ] **Step 1: Write the snapshot test (it will fail on first run - no baseline exists yet).** Create `crates/ui/tests/resolve_layout_snapshot.rs`. One snapshot per workspace, named so a failing case reads as a spec line. `resolve_layout` and `ResolvedLayout` live in `pixhaus_ui::registry` (spec module tree: `registry/resolve.rs` re-exported through `registry/mod.rs`). `WorkspaceId` is the `&'static str` newtype from `contrib_api::ids`.

```rust
//! Spec test 2: snapshot the resolved layout of every workspace.
//!
//! With all five modules registered, each workspace's `ResolvedLayout` is the
//! single regression surface for placement - a moved panel, a renamed
//! workspace, or a dropped tray tab is a snapshot diff. An unregistered panel
//! reference shows as a gap here (and `resolve_layout` logs a warn).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod support;

use pixhaus_ui::contrib_api::WorkspaceId;
use pixhaus_ui::registry::resolve_layout;
use support::{fully_registered_host, WORKSPACE_IDS};

#[test]
fn draw_layout_is_stable() {
    let host = fully_registered_host();
    let resolved = resolve_layout(WorkspaceId(WORKSPACE_IDS[0]), &host.registries);
    insta::assert_debug_snapshot!("draw_layout", resolved);
}

#[test]
fn animate_layout_is_stable() {
    let host = fully_registered_host();
    let resolved = resolve_layout(WorkspaceId(WORKSPACE_IDS[1]), &host.registries);
    insta::assert_debug_snapshot!("animate_layout", resolved);
}

#[test]
fn tiles_layout_is_stable() {
    let host = fully_registered_host();
    let resolved = resolve_layout(WorkspaceId(WORKSPACE_IDS[2]), &host.registries);
    insta::assert_debug_snapshot!("tiles_layout", resolved);
}

#[test]
fn generate_layout_is_stable() {
    let host = fully_registered_host();
    let resolved = resolve_layout(WorkspaceId(WORKSPACE_IDS[3]), &host.registries);
    insta::assert_debug_snapshot!("generate_layout", resolved);
}

#[test]
fn export_layout_is_stable() {
    let host = fully_registered_host();
    let resolved = resolve_layout(WorkspaceId(WORKSPACE_IDS[4]), &host.registries);
    insta::assert_debug_snapshot!("export_layout", resolved);
}
```

Note on access form: `Host.registries` is a public field (defined in the State phase), so the test resolves layout via `&host.registries` (not a `registries()` accessor). The rest of the test is unaffected.

- [ ] **Step 2: Run the snapshot test - confirm it fails because there is no baseline.** Run in PowerShell:

```powershell
cargo nextest run -p pixhaus-ui resolve_layout_snapshot
```

Expected: FAIL on all five tests, with insta reporting new snapshots pending (the `.snap.new` files are written, the assertion fails because no accepted baseline exists). This is the expected first-run state for a new snapshot test.

- [ ] **Step 3: Review and accept the snapshots.** insta writes `.snap.new` files; review each one and accept only after reading it (never blind-accept - `pixhaus-testing-conventions` "Snapshots reviewed by Cmd-A then Accept" anti-pattern). Run in PowerShell:

```powershell
cargo insta review
```

For each of the five pending snapshots, read the captured `ResolvedLayout` and confirm it matches the spec's per-workspace placement table (spec sec"Per-workspace placement"): e.g. Draw's `right_dock` is `[layers, sprites, palette, selection-actions, ai-assistant]`, its `bottom_tray` is `[frames, assets, console]`, its `default_tool` is the Pencil tool id, its `status_items` contains `Pixel Grid On`. Accept each that matches; if one shows a gap (a panel filtered out because it was never registered), that is a MODULES bug - stop and report it rather than accepting the gap. If `cargo insta` is not installed, install it first: `cargo install cargo-insta`.

- [ ] **Step 4: Re-run the test to confirm it now passes against the accepted baselines.** Run in PowerShell:

```powershell
cargo nextest run -p pixhaus-ui resolve_layout_snapshot
```

Expected: PASS (all five). The accepted `.snap` files are now the committed baseline.

- [ ] **Step 5: Commit the test and the accepted snapshots together.** Run in PowerShell:

```powershell
git add crates/ui/tests/resolve_layout_snapshot.rs crates/ui/tests/snapshots/
git commit -m @'
test(ui): snapshot resolved layout for all five workspaces

Spec test 2. Registers the full module set and snapshots each workspace's
ResolvedLayout (dock ids, tray tabs, tools, default tool, status items).
A moved panel or renamed workspace is now a snapshot diff; an unregistered
reference shows as a gap. Baselines reviewed against the placement table.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

### TESTS.3: Headless smoke test across all five workspaces

**Files:**
- Create: `crates/ui/tests/smoke.rs`
- Test: `crates/ui/tests/smoke.rs`

This is spec test 7. egui_kittest was declined (spec sec"Decisions taken"), so this stays a headless `Host` + `resolve_layout` assertion - no event loop, no GPU. Boot the `Host`, assert `resolve_layout` produces a non-empty `right_dock` and a non-empty `bottom_tray` for all five workspaces, and assert the resolved top-bar tab set contains the five workspace names. This catches a whole-shell wiring break (a module that registers a workspace but no panels) that the per-workspace snapshots would also show but less directly.

- [ ] **Step 1: Write the smoke test (failing first - implementation already exists from earlier layers, so this should pass immediately once compiled; we still run it before trusting it).** Create `crates/ui/tests/smoke.rs`. The "top-bar tab set" is the set of workspace `meta().name` values, iterated from the workspace registry in insertion order (spec sec"Region composition": "iterate `registries.workspaces.iter()`"). `Workspace::meta()` returns `WorkspaceMeta { name, .. }` (spec sec"Workspace").

```rust
//! Spec test 7: headless boot smoke test (egui_kittest declined).
//!
//! No event loop, no GPU. Boot the Host with all five modules registered and
//! assert every workspace resolves to a non-empty dock and tray, and that the
//! workspace tab set is exactly the five expected names.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod support;

use pixhaus_ui::contrib_api::WorkspaceId;
use pixhaus_ui::registry::resolve_layout;
use support::{fully_registered_host, WORKSPACE_IDS, WORKSPACE_NAMES};

#[test]
fn every_workspace_resolves_a_non_empty_dock() {
    let host = fully_registered_host();
    for id in WORKSPACE_IDS {
        let resolved = resolve_layout(WorkspaceId(id), &host.registries);
        assert!(
            !resolved.right_dock.is_empty(),
            "workspace {id:?} resolved an empty right dock"
        );
    }
}

#[test]
fn every_workspace_resolves_a_non_empty_tray() {
    let host = fully_registered_host();
    for id in WORKSPACE_IDS {
        let resolved = resolve_layout(WorkspaceId(id), &host.registries);
        assert!(
            !resolved.bottom_tray.is_empty(),
            "workspace {id:?} resolved an empty bottom tray"
        );
    }
}

#[test]
fn top_bar_tab_set_is_the_five_workspace_names() {
    let host = fully_registered_host();
    let names: Vec<&str> = host
        .registries
        .workspaces
        .iter()
        .map(|ws| ws.meta().name)
        .collect();
    assert_eq!(
        names.len(),
        WORKSPACE_NAMES.len(),
        "expected exactly five registered workspaces, got {names:?}"
    );
    for expected in WORKSPACE_NAMES {
        assert!(
            names.contains(&expected),
            "workspace tab set {names:?} is missing {expected:?}"
        );
    }
}
```

Note on access form: same as TESTS.2 - this uses the public field `&host.registries`. If STATE shipped a `registries()` accessor instead of a public field, change `&host.registries` to `host.registries()` in all three tests and `host.registries.workspaces.iter()` to `host.registries().workspaces.iter()`. Confirm against `crates/ui/src/state/mod.rs` before running. `registries.workspaces.iter()` is the `Registry::iter()` method from spec sec"Registries" (returns `impl Iterator<Item = &Box<dyn Workspace>>`); `ws.meta()` works through the `&Box<dyn Workspace>` deref.

- [ ] **Step 2: Run the smoke test.** Run in PowerShell:

```powershell
cargo nextest run -p pixhaus-ui smoke
```

Expected: PASS (all three). A FAIL on the dock/tray emptiness assertions means a module registered a workspace whose layout references panels none of the modules registered (the gap that `resolve_layout` warns about) - that is a MODULES registration bug, not a test bug; report it. A FAIL on the tab-set count means a workspace failed to register or a duplicate id collapsed two workspaces; report it.

- [ ] **Step 3: Commit.** Run in PowerShell:

```powershell
git add crates/ui/tests/smoke.rs
git commit -m @'
test(ui): headless boot smoke test across five workspaces

Spec test 7 (egui_kittest declined, so this is a headless Host +
resolve_layout assertion). Every workspace resolves a non-empty dock and
tray, and the workspace tab set is exactly the five expected names.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

### TESTS.4: Verify the dyn-compatibility compile guard is present

**Files:**
- Verify (do not create - owned by the registry layer): `crates/ui/src/registry/mod.rs`
- Test: the guard is itself a compile-time test (spec test 5); no separate test file.

This is spec test 5 - a `const _: () = { ... }` block that proves `Panel`, `Tool`, `Workspace`, and `Module` are all dyn-compatible (boxable). It is "free, permanent": if any registry trait regresses (a generic method, a `-> Self`, a non-`&self` receiver), the crate stops compiling. The guard belongs to the registry/contrib_api layer; this layer's job is to confirm it exists and is exactly the spec form, because the test plan lists it as a gate item and a missing guard is a silent hole.

- [ ] **Step 1: Confirm the guard exists in `registry/mod.rs`.** Use Grep to check:

```
Grep: pattern "_assert_boxable", path "crates/ui/src/registry/mod.rs", output_mode "content", -C 3
```

Expected: the block is present and reads exactly (spec sec"Registries"):

```rust
const _: () = {
    fn _assert_boxable(_: Box<dyn Panel>, _: Box<dyn Tool>, _: Box<dyn Workspace>, _: Box<dyn Module>) {}
};
```

- [ ] **Step 2: If the guard is missing or incomplete, add it.** Only if Step 1 found nothing or found a guard missing one of the four traits: open `crates/ui/src/registry/mod.rs`, add the exact block above near the registry type aliases, and ensure `Panel`, `Tool`, `Workspace`, `Module` are in scope (they are re-exported from `contrib_api`; add `use crate::contrib_api::{Panel, Tool, Workspace, Module};` if not already imported). If the guard is already present and complete, skip this step and the commit - do not author a redundant change.

- [ ] **Step 3: Prove the guard compiles (and thus that the four traits are dyn-compatible).** Run in PowerShell:

```powershell
cargo build -p pixhaus-ui
```

Expected: PASS. If it fails with E0038 ("the trait `X` cannot be made into an object"), a registry trait regressed dyn-compatibility - that is a contrib_api-layer bug (a generic method, `-> Self`, or a by-value receiver crept in); report it rather than weakening the guard.

- [ ] **Step 4: Commit only if Step 2 changed a file.** If Step 2 added the guard, run in PowerShell:

```powershell
git add crates/ui/src/registry/mod.rs
git commit -m @'
test(ui): restore the dyn-compatibility compile guard

Spec test 5. The const _ assertion that Panel/Tool/Workspace/Module are
all boxable - if any registry trait regresses dyn-compatibility the crate
stops compiling. Free and permanent.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

If Step 1 found the guard already present and complete, this layer added nothing here - note that in the PR body's test plan (TESTS.6) and move on.

### TESTS.5: Run the full Stop-gate and fix anything red

**Files:**
- Modify (only if a gate fails): any file the gate flags - scope each fix to the smallest change that clears the failure.
- Test: the whole workspace, via the six gate commands.

The Stop hook is the session gate (root CLAUDE.md secHooks). Run every gate command locally through PowerShell before pushing, in the spec's order, and fix each failure at its root rather than suppressing it. cargo MUST go through PowerShell on this machine (the Bash tool's Git-Bash `link.exe` shadows the MSVC linker).

- [ ] **Step 1: Format check.** Run in PowerShell:

```powershell
cargo fmt --all --check
```

Expected: PASS (clean exit, no diff). The post-edit hook formats touched crates as you go, so this should already be clean. If it reports a diff, run `cargo fmt --all` to fix, then re-run the check; if the working tree is CRLF and rustfmt's `newline_style = Unix` trips the check, that is the known `.gitattributes eol=lf` issue - confirm `.gitattributes` has `*.rs eol=lf` and re-checkout the touched files, do not hand-edit line endings.

- [ ] **Step 2: Clippy across the whole workspace, all targets, warnings as errors.** Run in PowerShell:

```powershell
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS. The post-edit hook ran `clippy --tests` per touched crate, but this is the first all-targets, whole-workspace pass. If a test file trips `clippy::unwrap_used`/`expect_used`, confirm the crate root has `#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]` (root CLAUDE.md convention; the test files in this layer already carry it at file scope, but the crate root attribute is the canonical place - verify `crates/ui/src/lib.rs` has it). Fix any real lint; never add a blanket `#[allow]` to silence one.

- [ ] **Step 3: Run the full test suite under nextest.** Run in PowerShell:

```powershell
cargo nextest run --workspace
```

Expected: PASS. This runs every layer's tests plus this layer's three test binaries. nextest does not run doc tests - that is Step 4. If a snapshot test fails here on a machine where the baseline was just accepted, confirm the `.snap` files were committed in TESTS.2 Step 5 (an uncommitted baseline reads as missing).

- [ ] **Step 4: Run doc tests.** Run in PowerShell:

```powershell
cargo test --doc --workspace
```

Expected: PASS. Doc tests on `pixhaus-ui`'s public functions run here (the conventions skill: doc tests run under `cargo test --doc`, not nextest).

- [ ] **Step 5: Build the docs.** Run in PowerShell:

```powershell
cargo doc --workspace --no-deps
```

Expected: PASS with no warnings. `missing_docs = "warn"` is workspace-wide; an undocumented public item surfaces here. If a public item this layer touched is undocumented, add a one-line doc comment in Pixhaus voice (direct, declarative); do not downgrade the lint.

- [ ] **Step 6: License/dependency gate.** Run in PowerShell:

```powershell
cargo deny check --config .cargo/deny.toml
```

Expected: PASS for licenses on any dependency this round added (the round adds `egui-phosphor` per spec sec"Decisions taken" - confirm it is MIT/Apache; it is). Note: per the user's memory, v2 saw a pre-existing deny failure on `Ubuntu-font-1.0` (egui's bundled fonts) and a null-license transitive crate. If `cargo deny` fails ONLY on a pre-existing advisory unrelated to this round's changes (e.g. an egui font license already in the tree before this branch), record it in the PR body as pre-existing and do not block on it; if it fails on a dependency this round introduced, fix it (drop or replace the dependency) before pushing.

- [ ] **Step 7: If any gate step required a fix, commit the fixes.** Only if Steps 1-6 changed files. Run in PowerShell (adjust the paths to what you actually changed):

```powershell
git add -A
git commit -m @'
chore(ui): clear the stop-gate for the ui shell foundation

Resolve fmt/clippy/doc findings surfaced by the full workspace gate so the
session gate is green: cargo fmt --check, clippy --workspace --all-targets,
nextest, doc tests, cargo doc, cargo deny all pass.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```

If all six gate steps passed clean with no edits, this layer added no commit here - proceed to TESTS.6.

### TESTS.6: Manual-verify the running app

**Files:**
- No files. This is a runtime smoke check of `cargo run -p pixhaus-app`; the spec marks canvas chrome and interaction as manual-verify only (spec sec"Test plan": "Not worth testing yet ... canvas chrome geometry (manual-verify)").

The headless tests prove wiring; they cannot prove the window paints. Run the app and confirm the interactive paths the spec calls out work end to end. This step does not run in CI - it is a human-in-the-loop gate before the PR.

- [ ] **Step 1: Launch the app.** Run in PowerShell (foreground; it opens a window and blocks until you close it):

```powershell
cargo run -p pixhaus-app
```

Expected: a window titled Pixhaus opens, dark/violet themed, with all seven regions visible - top bar with workspace tabs, left tool rail, right dock card stack, bottom tray with tabs, status bar, tool-options bar, and the framed canvas artboard with a checkerboard and grid in the center.

- [ ] **Step 2: Switch workspaces with the keyboard.** With the window focused, press Cmd/Ctrl+1 through Cmd/Ctrl+5 in turn. Expected: the active workspace tab changes and the right-dock cards and bottom-tray tabs swap to match the spec's per-workspace placement table (Draw shows Layers/Sprites/Palette/Selection Actions/AI Assistant; Generate shows Prompt/Recipe/Structure/Style/Palette Behavior/Advanced Settings; etc.). Click the tabs directly too - same effect.

- [ ] **Step 3: Open the command palette.** Press Cmd/Ctrl+K. Expected: an overlay floats above everything (with the overlay shadow), containing a text field and a seeded list (Switch to {workspace}, Select {tool}, and the mock action entries). Type into the field - the query filters/echoes. Press Escape - the overlay closes.

- [ ] **Step 4: Toggle the theme.** Open the View menu, then Theme, and pick Light, then Accent, then back to Dark. Expected: the window actually repaints to the chosen variant each time (this proves `Intent::SetThemeVariant` re-applies via `apply_to_visuals` with the live `Context` - spec sec"Intents and events"). Only Dark is visually tuned this round; Light/Accent need only repaint without panicking.

- [ ] **Step 5: Confirm the canvas renders in the framed stage.** Look at the center region. Expected: the wgpu `CanvasCallback` renders inside the framed artboard rect (the existing seam, preserved), sitting on the checkerboard with the drop shadow, minor/major grid lines over it, and the static HUD chip at the artboard's lower-left reading something like `64 x 64   1600%   Grid 8px   Palette: Bit`. Switch to Generate (Cmd/Ctrl+4) and back to Draw - the canvas stays rendered.

- [ ] **Step 6: Close the window and record the result.** Close the window (the `cargo run` command returns). There is no commit for this step - it is a verification gate. Capture one screenshot or a short screen clip of the Draw workspace with the canvas rendered, for the PR (per the convention: UI changes get screenshots/clips). Save it somewhere outside the repo tree (e.g. the desktop) so it is not accidentally committed; you will attach it to the PR in TESTS.7. If any step 2-5 failed, stop and report the specific failure rather than opening the PR.

### TESTS.7: Push the branch and open the PR to v3

**Files:**
- No repo files. This pushes `feat/ui-shell-foundation` and opens a PR via `gh`.

The branch already exists and all work is committed. Push and open a PR targeting `v3` (the base branch for this round, per the repo's branch model - `main` is the long-run default, but this round's work integrates onto `v3`). Use the PR body to say what changed, why, and the test plan, and note the screenshot/clip.

- [ ] **Step 1: Confirm the working tree is clean and on the right branch.** Run in PowerShell:

```powershell
git status --short --branch
```

Expected: branch is `feat/ui-shell-foundation`, no uncommitted changes (clean tree). If there are stray changes, commit or discard them per the appropriate layer before pushing - do not push a dirty tree.

- [ ] **Step 2: Push the branch to origin.** Run in PowerShell:

```powershell
git push -u origin feat/ui-shell-foundation
```

Expected: PASS - the branch is created on origin with upstream tracking. If it already exists upstream, a plain `git push` updates it.

- [ ] **Step 3: Open the PR against `v3` with a concrete test plan.** Run in PowerShell (the body is a single-quoted here-string so `$` and backticks stay literal; the closing `'@` is at column 0):

```powershell
gh pr create --base v3 --head feat/ui-shell-foundation --title "feat(ui): registry-driven shell foundation" --body @'
## What

Builds the v3 UI shell foundation: the bible Phase-0 trait surface
(Panel/Tool/Workspace/Module), the registries and layout resolution,
project/session/UI state separation behind a read-view + single intent-sink
borrow model, fresh dark/violet theme tokens, and all seven window regions
rendered from five registered workspaces. The app binary composes the shell
purely by registering five modules and consuming registries; it declares no
layout. The existing CanvasCallback + install_canvas_renderer seam is preserved
unchanged.

## Why

Implements docs/superpowers/specs/2026-06-01-ui-shell-foundation-design.md.
The anchor decision: panels get a read-only state view plus one write channel,
so "panels never mutate project/session state directly" (bible rules 12, 21)
is a compiler guarantee, not a convention.

## Test plan

Headless, no GPU, no event loop. All run green locally through the full gate:

- cargo fmt --all --check
- cargo clippy --workspace --all-targets -- -D warnings
- cargo nextest run --workspace
- cargo test --doc --workspace
- cargo doc --workspace --no-deps
- cargo deny check --config .cargo/deny.toml

Spec test coverage:

- Test 1: registry registration + duplicate-id debug_assert (rstest).
- Test 2: insta snapshot of resolve_layout for all five workspaces, full module
  set registered. Baselines reviewed against the placement table, not blind-accepted.
- Test 3: apply_intent state transitions (rstest).
- Test 4: theme tokens to Visuals + WCAG contrast (rstest).
- Test 5: dyn-compatibility compile guard (const _ assert_boxable) - present and compiling.
- Test 6: shortcut key-to-intent mapping incl. focused-text-field gating (rstest).
- Test 7: headless boot smoke test - every workspace resolves a non-empty dock
  and tray; the tab set is exactly the five workspace names.

Manual-verify (canvas chrome and interaction, per the spec):

- cargo run -p pixhaus-app: window opens, dark/violet themed, all seven regions present.
- Cmd/Ctrl+1..5 switch workspaces; dock cards and tray tabs swap to match the table.
- Cmd/Ctrl+K opens the command palette; Escape closes it.
- View > Theme > Light/Accent/Dark repaints the window.
- The wgpu canvas renders inside the framed artboard with checker, grid, and HUD.

Screenshot of the Draw workspace with the canvas rendered is attached below.

 Generated with [Claude Code](https://claude.com/claude-code)
'@
```

Expected: `gh` prints the new PR URL. If `gh pr create` reports the base `v3` is not found on the remote, confirm `v3` exists on origin (`git ls-remote --heads origin v3`) and push it if it is local-only - but do not retarget the PR to `main`; this round integrates onto `v3`.

- [ ] **Step 4: Attach the screenshot/clip to the PR.** `gh` cannot upload an image to the body directly; add it as a comment. Run in PowerShell, replacing the path with where TESTS.6 Step 6 saved the capture:

```powershell
gh pr comment feat/ui-shell-foundation --body "Draw workspace with the wgpu canvas rendered in the framed stage:`n`n![draw-workspace](PASTE_IMAGE_URL_AFTER_DRAG_DROP)"
```

If the image is a local file rather than an uploadable URL, instead open the PR in the browser (`gh pr view --web`) and drag-drop the screenshot/clip into a comment there - GitHub uploads it and inserts the markdown. Expected: the PR carries a visible screenshot or clip of the rendered canvas, satisfying the UI-change screenshot convention.

- [ ] **Step 5: Report the PR URL.** This layer's final action: emit the PR URL (from Step 3) as the result so the orchestrator can record it. No commit - the PR is the deliverable.