---
name: pixhaus-tauri-patterns
description: Use when writing Tauri 2 IPC commands, state, events, windows, or platform-specific code in Pixhaus — covers commands, state, events, tauri-specta, menus, and per-OS quirks
---

# Pixhaus Tauri 2 patterns

The patterns every UI stream and the AI runtime use. Tauri 2.x has a
different API surface from 1.x — verify against the v2 docs at
[v2.tauri.app](https://v2.tauri.app/) when in doubt, not against
training-data memory of v1.

## Architecture, briefly

- **Rust core process** owns heavy work: pixel buffers, file I/O, AI
  orchestration, Lua scripts. Lives in the workspace crates (`core`, `io`,
  `ai`, `scripting`).
- **WebView UI** owns the visual layer: command palette, panels, canvas
  overlay, theming. Lives in `ui/` (Solid + Vite + TypeScript).
- **Tauri shell** (`app/`) wires them together via IPC commands and
  events.

Heavy data (pixel buffers) does not cross the IPC bridge as JSON. The Rust
side keeps it in memory; the UI receives opaque handles plus rendered
textures for display.

## IPC commands

### Signature shape

```rust
// app/src/commands/canvas.rs

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct DrawStrokeArgs {
    pub layer_id: String,
    pub points: Vec<(f32, f32)>,
    pub brush: BrushConfig,
}

#[derive(Debug, Serialize)]
pub struct DrawStrokeResult {
    pub stroke_id: String,
    pub bbox: (f32, f32, f32, f32),
}

#[tauri::command(async)]
pub async fn draw_stroke(
    args: DrawStrokeArgs,
    state: State<'_, AppState>,
) -> Result<DrawStrokeResult, String> {
    state.canvas
        .draw_stroke(args.layer_id, args.points, args.brush)
        .await
        .map_err(|e| e.to_string())
}
```

Rules:

- Inputs and outputs are typed structs, not loose `serde_json::Value`.
- Errors stringify at the IPC boundary. Library errors stay rich internally;
  the boundary serializes them.
- `#[tauri::command(async)]` runs the command on Tauri's thread pool.
  Drop the `(async)` only if the command is genuinely synchronous and fast
  (`<1 ms`); most editor commands are async.
- The state argument uses lifetime `'_`: `State<'_, AppState>`. Tauri 2
  enforces this.

### Registering commands

```rust
// app/src/lib.rs

pub fn run() -> Result<(), AppError> {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::project::open,
            commands::project::save,
            commands::canvas::draw_stroke,
            commands::layers::add,
            commands::verbs::invoke,
        ])
        .run(tauri::generate_context!())?;

    Ok(())
}
```

Group commands by category in `app/src/commands/<category>.rs`. The
`generate_handler!` list is the canonical IPC catalog — keep it sorted
to make conflicts in PRs trivial to resolve.

### From the UI side

```ts
// ui/src/lib/commands/canvas.ts

import { invoke } from "@tauri-apps/api/core";

export type DrawStrokeArgs = {
  layerId: string;
  points: Array<[number, number]>;
  brush: BrushConfig;
};

export type DrawStrokeResult = {
  strokeId: string;
  bbox: [number, number, number, number];
};

export async function drawStroke(args: DrawStrokeArgs): Promise<DrawStrokeResult> {
  return invoke<DrawStrokeResult>("draw_stroke", { args });
}
```

The TypeScript wrapper is the only place the UI mentions string command
names. Components import the typed function.

## State management — `tauri::State<T>`

### One state struct, registered with `.manage()`

```rust
// app/src/state.rs

pub struct AppState {
    pub canvas: Arc<CanvasService>,
    pub project: Arc<tokio::sync::Mutex<Option<Document>>>,
    pub verbs: Arc<VerbRuntime>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            canvas: Arc::new(CanvasService::new()),
            project: Arc::new(tokio::sync::Mutex::new(None)),
            verbs: Arc::new(VerbRuntime::new()),
        }
    }
}
```

Commands receive `State<'_, AppState>` and dereference: `state.canvas.foo()`.
The state struct is `Send + Sync`; everything inside it must be too.

### Don't put `Mutex<T>` directly in state if you don't need exclusive access

Wrong reflex:

```rust
// BAD — every reader serializes on the same mutex
pub struct AppState {
    pub config: Mutex<Config>,
}
```

Better:

```rust
// GOOD — read-mostly state behind ArcSwap or RwLock
pub struct AppState {
    pub config: arc_swap::ArcSwap<Config>,
}
```

Use `Mutex` only when the state mutates frequently and reads are rare. The
active document is the canonical case for `Mutex`; runtime config and
loaded plugins are not.

## Events

### Emitting from Rust

```rust
use tauri::{AppHandle, Emitter};

pub async fn notify_project_changed(app: &AppHandle, payload: ProjectChange) {
    if let Err(err) = app.emit("project:changed", payload) {
        tracing::warn!("failed to emit project:changed: {err}");
    }
}
```

Event names use `domain:verb` form: `project:changed`, `verb:progress`,
`document:dirty`. Keep the namespace consistent so the UI can subscribe by
prefix.

### Subscribing from the UI

```ts
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { onCleanup } from "solid-js";

export function onProjectChanged(handler: (change: ProjectChange) => void) {
  let unlisten: UnlistenFn | undefined;

  listen<ProjectChange>("project:changed", (event) => {
    handler(event.payload);
  })
    .then((fn) => {
      unlisten = fn;
    })
    .catch((err) => console.error("listen project:changed failed", err));

  onCleanup(() => unlisten?.());
}
```

Always pair `listen()` with cleanup. Solid components call `onCleanup`;
imperative call sites store the returned `UnlistenFn` and call it on
shutdown. Leaking listeners adds up fast across hot reloads.

### Window-scoped vs. global events

`AppHandle::emit` fires globally. To target a specific window, use
`Emitter::emit_to`:

```rust
app.emit_to("inspector", "selection:changed", payload)?;
```

This matters once Pixhaus has multiple windows (inspector, splash, prefs).
For now there's one window; default to `emit`.

## Windows

### The main window is declared in `tauri.conf.json`

```json
{
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "Pixhaus",
        "width": 1280,
        "height": 800,
        "minWidth": 800,
        "minHeight": 600
      }
    ]
  }
}
```

### Spawning at runtime

```rust
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};

pub fn open_inspector(app: &AppHandle) -> tauri::Result<()> {
    let _window = WebviewWindowBuilder::new(
        app,
        "inspector",
        WebviewUrl::App("/inspector".into()),
    )
    .title("Inspector")
    .inner_size(360.0, 600.0)
    .resizable(true)
    .build()?;

    Ok(())
}
```

The window label is the identity. Reuse it on subsequent `open_inspector`
calls — Tauri will focus the existing window instead of creating a duplicate
if you check first:

```rust
if let Some(existing) = app.get_webview_window("inspector") {
    existing.set_focus()?;
    return Ok(());
}
```

## Cross-thread main-thread issues

Some platform calls must run on the main thread:

- macOS NSWindow operations
- macOS dock icon changes
- Linux DBus appearance queries

Use `AppHandle::run_on_main_thread`:

```rust
app.run_on_main_thread(move || {
    // platform call here
})?;
```

CPU work goes the other way — off the main thread via `spawn_blocking`.
The main thread serves the event loop; never block it for more than a
frame.

## `tauri-specta` for typed IPC

`tauri-specta` generates TypeScript types from Rust commands so the UI
wrappers stay in sync automatically. Once it lands in the workspace:

```rust
use specta::Type;
use tauri_specta::collect_commands;

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct LayerInfo {
    pub id: String,
    pub name: String,
    pub kind: LayerKind,
    pub visible: bool,
}

#[tauri::command(async)]
#[specta::specta]
pub async fn list_layers(state: State<'_, AppState>) -> Result<Vec<LayerInfo>, String> {
    state.list_layers().await.map_err(|e| e.to_string())
}

// In app/src/lib.rs:
let (invoke_handler, register_events) =
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(collect_commands![list_layers])
        .header("// AUTO-GENERATED by tauri-specta — do not edit")
        .build()?;
```

The build emits `ui/src/lib/bindings.ts` with typed wrappers. Until S04
lands, hand-mirror types in `ui/src/lib/commands/`. After S04, never edit
`bindings.ts` by hand.

## Native menus

Tauri 2 native menus build with `tauri::menu`:

```rust
use tauri::menu::{Menu, MenuItemBuilder, SubmenuBuilder};

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let file = SubmenuBuilder::new(app, "File")
        .item(&MenuItemBuilder::new("New Project").id("file:new").build(app)?)
        .item(&MenuItemBuilder::new("Open...").id("file:open").accelerator("CmdOrCtrl+O").build(app)?)
        .separator()
        .item(&MenuItemBuilder::new("Save").id("file:save").accelerator("CmdOrCtrl+S").build(app)?)
        .build()?;

    Menu::with_items(app, &[&file])
}
```

Wire menu events in the builder:

```rust
.on_menu_event(|app, event| match event.id().as_ref() {
    "file:new" => { /* dispatch */ }
    "file:open" => { /* dispatch */ }
    _ => {}
})
```

Use `CmdOrCtrl` accelerators — Tauri maps to ⌘ on macOS and Ctrl on
Windows/Linux. Don't hard-code platform-specific shortcuts.

## Per-OS quirks

### macOS

- The traffic-light buttons live in the title bar; don't draw a custom
  close button. If you need a chromeless look, use `decorations: false`
  and rebuild the controls in the UI.
- Menus apply globally: the menu bar at the top of the screen replaces
  the per-window menu Windows/Linux users expect.
- `NSWindow` mutations need the main thread (`run_on_main_thread`).
- Sandbox vs. signed builds: the dev `cargo tauri dev` builds are not
  sandboxed; `cargo tauri build --bundles app` produces an `.app` that
  may need notarization.
- Retina: window sizes in `tauri.conf.json` are logical (CSS) pixels,
  not device pixels. Don't multiply by 2 manually.

### Windows

- The webview is **WebView2** (Chromium, Edge release channel).
- DPI scaling is per-monitor; the UI receives pixel-correct sizes via
  CSS. Don't read `window.devicePixelRatio` and scale layouts off it —
  trust the OS.
- WiX vs. NSIS bundle targets: NSIS is the default and produces a smaller
  installer; WiX adds MSI for enterprise deployment. Pick per release.
- Console window: `app/src/main.rs` has
  `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`
  to hide the console in release. Keep it in debug for stdout/stderr.
- File paths: prefer `Path::new(s)` and avoid string-joining slashes by
  hand. UNC paths (`\\?\C:\...`) appear when files are deep.

### Linux

- The webview is **WebKitGTK** — older than Edge or WebKit/macOS. CSS
  features stable in Chrome 105 / Safari 16 may not be present. Test
  there.
- Wayland vs. X11: window decorations differ. Test both if you touch
  window chrome.
- AppImage vs. deb vs. rpm bundles: AppImage is the default
  recommendation; the others are for distro repos.
- DBus availability is not guaranteed (containers, minimal images). Wrap
  DBus calls in `if let Ok(...)`, don't expect them.

## Configuration sanity

`app/tauri.conf.json` checklist before changing:

- `productName` — the user-visible app name (`Pixhaus`)
- `version` — keep in sync with `Cargo.toml` workspace version
- `identifier` — reverse-DNS app id (`app.pixhaus.desktop`)
- `build.frontendDist` — points at `../ui/dist`
- `build.devUrl` — `http://localhost:1420` (matches `vite.config.ts`)
- `build.beforeDevCommand` — `pnpm --filter pixhaus-ui dev`
- `build.beforeBuildCommand` — `pnpm --filter pixhaus-ui build`
- `app.windows[0].label` — `main` (referenced from capabilities)
- `app.security.csp` — default `null` for dev, hardened later (S37)
- `bundle.icon` — array of paths to icons; `cargo tauri icon` regenerates
  the set from a single high-res source

## Capabilities

Tauri 2 introduces capability files at `app/capabilities/*.json`. They
gate which permissions the front-end can use. Default minimal:

```json
{
  "identifier": "default",
  "description": "Default permissions for the Pixhaus main window.",
  "windows": ["main"],
  "permissions": ["core:default"]
}
```

Add permissions only when a feature needs them. Don't blanket-grant
`core:fs:default` for "in case we need file access" — narrow it to the
specific permissions (`core:fs:allow-read-text-file`, etc.).

## Quick reference

| Need | API |
|---|---|
| Define an IPC command | `#[tauri::command(async)]` |
| Access shared state in a command | `state: State<'_, AppState>` |
| Emit an event globally | `AppHandle::emit("event:name", payload)` |
| Emit to one window | `AppHandle::emit_to("label", "event", payload)` |
| Listen in JS | `listen("event:name", handler)` |
| Open a new window | `WebviewWindowBuilder::new(app, label, url)` |
| Find an existing window | `app.get_webview_window("label")` |
| Run on main thread | `app.run_on_main_thread(closure)` |
| Build a menu | `tauri::menu::SubmenuBuilder` |
| Add a permission | edit `app/capabilities/default.json` |

## When in doubt

- Verify against [v2.tauri.app](https://v2.tauri.app/) before assuming a
  v1 API still exists. The migration changed many surfaces.
- For platform-specific behavior, test on the target OS rather than
  trusting cross-platform abstractions.
- Don't reach into `tauri::api::*` modules — they're internal and unstable.
- Surface API surface changes to a maintainer; the planning docs pin a
  version, and silent upgrades break stream assumptions.
