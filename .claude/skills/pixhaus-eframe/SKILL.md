---
name: pixhaus-eframe
description: >
  Use when working on the eframe application shell of the Pixhaus native app — booting
  the window, the App trait and its frame loop, NativeOptions / ViewportBuilder window
  config, choosing the wgpu vs glow renderer, grabbing the wgpu render state at startup,
  app lifecycle (logic vs ui, save, on_exit, auto-save), graceful-close handling, and
  on-disk persistence (Storage, get_value/set_value, persist_window). Trigger this for
  ANY work on "how the app starts", "the window options", "main.rs", "the eframe loop",
  "saving preferences", "the title bar / window icon / fullscreen", or "where do I get
  the wgpu device", even when the user doesn't say "eframe". eframe is the native shell
  around egui; its 0.34 App API (the method is `ui`, not `update`) differs sharply from
  older examples. For the UI *inside* the window — panels, widgets, the canvas paint
  callback, input — use pixhaus-egui instead.
---

# eframe for Pixhaus

eframe is the native shell: it owns the OS window, the winit event loop, the render
backend, and the persistence directory, then hands you an `egui::Context` each frame to
draw into. It is the thin layer between `fn main()` and your UI. This skill is the floor
for eframe work in Pixhaus — bootstrapping the app, the lifecycle contract, window and
renderer configuration, acquiring the wgpu render state, and saving state to disk.

The line with `pixhaus-egui`: **eframe is the window and the loop; egui is what you draw
inside it.** Anything about panels, widgets, the wgpu canvas paint callback, input
routing, or theming lives in `pixhaus-egui`. Anything about starting the app, the `App`
trait methods, `NativeOptions`, the renderer choice, or persistence lives here. They share
the same 0.34.2 version pin and the same `App::ui` skeleton — by design, not duplication.

Don't guess signatures from memory. eframe's API moved in 0.34 (the `App` method is now
`ui`, `run_simple_native` is deprecated). The references are derived from docs.rs 0.34.2.

## Versions — pin in lockstep with egui

eframe moves in lockstep with the rest of the egui family. A mismatched `wgpu` is the most
common build break, because two `wgpu` versions in the tree are different types that won't
interoperate. This is the same table as `pixhaus-egui`; keep them identical.

| Crate | Version |
|---|---|
| `eframe` | 0.34.2 |
| `egui` / `egui-wgpu` / `egui-winit` / `epaint` | 0.34.2 |
| `wgpu` | `=29.0.1` (pin exactly, not `"29"`) |
| `winit` | 0.30.x |

```toml
eframe = { version = "0.34", default-features = false, features = ["wgpu", "persistence"] }
egui   = "0.34"
wgpu   = "=29.0.1"
```

Why `default-features = false`: eframe's defaults pull in `glow` and the web screen-reader
stack. Pixhaus is wgpu-only and desktop-only, so opt in to exactly `wgpu` + `persistence`
and skip the rest. See the feature table in `references/native-options-and-window.md`.
When you bump eframe, bump the whole family and re-verify against docs.rs — see
[[feedback-dep-upgrades]].

## The mental model: eframe owns the loop, you own the state

eframe runs the winit event loop on the main thread. Each time the window needs painting
it calls your `App` — first `logic` (non-UI per-frame work), then `ui` (draw). Your `App`
struct is the single owner of application state across frames (the document, tool
selection, undo stack); eframe just keeps calling into it. This matches the egui immediate-
mode model: state lives in your struct, not in the framework.

Three things drive most correct/incorrect decisions at this layer:

1. **The `App` method is `ui(&mut self, ui, frame)`, not `update`.** egui 0.34 renamed it.
   `update(ctx, frame)` still exists but is deprecated. `ui` receives a `&mut egui::Ui` for
   the whole root viewport with no margin or background. Put per-frame non-drawing work
   (draining channels, stepping animations) in `logic(&mut self, ctx, frame)`, which runs
   once before each `ui` and also runs when the window is hidden but a repaint was
   requested. You may not paint during `logic`. Full contract in
   `references/app-trait-and-lifecycle.md`.

2. **eframe repaints on demand, not on a clock.** Idle means asleep, not a spinning core.
   To animate or to surface a background result (an AI/IO task finishing on another
   thread), call `ctx.request_repaint()` — a cheap `Context` clone held by the worker
   thread can wake the loop when a result lands. This is how Pixhaus drives async results
   into the frame.

3. **Persistence is opt-in and goes through `Storage`.** Nothing is saved unless the
   `persistence` feature is on. Your data is serialized to RON via `eframe::set_value` in
   `App::save`, restored via `eframe::get_value` from `cc.storage` in your constructor.
   Window geometry and egui memory persist separately. See
   `references/storage-and-persistence.md`.

## The app skeleton (verified 0.34 API)

```rust
use eframe::egui;

struct Pixhaus {
    doc: Document,        // owned application state — the single owner
    tool: Tool,
    ui_prefs: UiPrefs,
}

impl Pixhaus {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // 1. grab the wgpu render state for the canvas paint callback (see pixhaus-egui)
        let render_state = cc.wgpu_render_state.as_ref().expect("wgpu backend");
        // 2. install fonts/theme once
        install_theme(&cc.egui_ctx);
        // 3. restore persisted prefs (needs the "persistence" feature)
        let ui_prefs = cc.storage
            .and_then(|s| eframe::get_value::<UiPrefs>(s, "ui_prefs"))
            .unwrap_or_default();
        Self { doc: Document::new(), tool: Tool::Brush, ui_prefs }
    }
}

impl eframe::App for Pixhaus {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.drain_async_results() {   // results that arrived from worker threads
            ctx.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // panels added to the root Ui — see pixhaus-egui for the panel/canvas layer
        egui::CentralPanel::default().show_inside(ui, |ui| self.canvas(ui));
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "ui_prefs", &self.ui_prefs);
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,    // required for the wgpu canvas
        viewport: egui::ViewportBuilder::default()
            .with_title("Pixhaus")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native("Pixhaus", options,
        Box::new(|cc| Ok(Box::new(Pixhaus::new(cc)))))
}
```

Notes that are easy to get wrong:

- **`run_native` takes an `AppCreator` closure that returns `Result<Box<dyn App>, _>`.**
  The `Ok(Box::new(...))` wrapper is mandatory — the creator can fail. `run_native`
  returns `eframe::Result`, so `fn main() -> eframe::Result` propagates startup errors.
- **The first `run_native` argument is the app id**, not just a title. It's the
  persistence save-location key and the Wayland application id. The window title comes from
  `ViewportBuilder::with_title`. Don't conflate them.
- **Renderer is `Renderer::Wgpu`.** With `default-features = false` and only the `wgpu`
  feature, that's also the default, but set it explicitly so the intent survives a feature
  edit. Mixing in `glow` is off the table for Pixhaus.
- **Grab `cc.wgpu_render_state` in the constructor**, not later — it's the handle the
  canvas paint callback needs (`pixhaus-egui` → `references/custom-wgpu-canvas.md`).
- `run_ui_native(app_name, opts, |ui, frame| { … })` is the closure-only shortcut for
  throwaway tools — no `App` struct, no custom-data persistence. Not for Pixhaus, but it's
  what you reach for in a quick repro. `run_simple_native` is its deprecated predecessor.

## Graceful close — confirm before discarding work

A pixel editor must intercept window-close to prompt for unsaved changes. eframe routes
this through egui viewport commands, not a callback return value:

```rust
fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
    let ctx = ui.ctx();
    if ctx.input(|i| i.viewport().close_requested()) && self.doc.is_dirty() {
        // veto the close, then drive your own confirm dialog over the next frames
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        self.show_unsaved_dialog = true;
    }
    // ... when the user confirms discard:
    // ctx.send_viewport_cmd(egui::ViewportCommand::Close);
}
```

`App::on_exit(&mut self, gl)` runs once after `save` on the way out — use it for cleanup
that must happen regardless, not for the confirm prompt (by then the decision is made).

## Rules that prevent the recurring bugs

- **Persistence does nothing without the `persistence` feature.** If `save` never seems to
  run or `cc.storage` is always `None`, the feature is off. It also gates `persist_window`
  and `persist_egui_memory`.
- **`save` is called on shutdown and on an interval** (`auto_save_interval`, default 30s) —
  so keep it cheap and allocation-light. Serialize prefs, not the whole document, unless
  you mean to.
- **Don't block the event-loop thread.** Long work (encoding a PNG, running an AI request)
  goes on a worker thread or `spawn_blocking`; signal completion with
  `ctx.request_repaint()`. Blocking in `ui`/`logic` freezes the window.
- **`Frame` here is `eframe::Frame` (the app's surroundings), not `egui`'s frame count.**
  It's how you reach `wgpu_render_state()`, `storage()`, `info()`, and screenshots at
  runtime — distinct from the `CreationContext` you got at startup.
- **`NativeOptions` is not `Clone` in a meaningful way for hooks** — `event_loop_builder`
  and `window_builder` hold boxed closures. Build it once in `main`.

## References

Open the file for the area you're working in; each is a dense eframe 0.34.2 reference.

| File | Covers |
|---|---|
| `references/app-trait-and-lifecycle.md` | `App` trait (`ui`, `logic`, `save`, `on_exit`, `auto_save_interval`, `clear_color`, `persist_egui_memory`, `raw_input_hook`, deprecated `update`), frame loop, close/exit, repaint, `Frame` methods, `IntegrationInfo` |
| `references/native-options-and-window.md` | `NativeOptions` (every field), `ViewportBuilder` window setup, `Renderer`, `HardwareAcceleration`, the window icon (`icon_data`), multisampling/depth, `wgpu_options`, multiple viewports, the feature flags |
| `references/startup-and-render-state.md` | `run_native` / `run_ui_native` / `run_simple_native` (deprecated) / `create_native`, `AppCreator`, `CreationContext` fields, acquiring `wgpu_render_state` / `gl`, setting fonts and visuals at boot, `Error` and `eframe::Result` |
| `references/storage-and-persistence.md` | `Storage` trait, `get_value` / `set_value` / `storage_dir`, RON format, `persist_window` / `persistence_path` / `persist_egui_memory`, what persists where, the `persistence` feature |

A standing caution: the references record the 0.34.2 API faithfully, but a few deep
signatures (notably the `get_value`/`set_value` generic bounds and `icon_data` return
types) were not fully rendered in the scraped docs and are marked "verify" inline. When one
is load-bearing, confirm it against https://docs.rs/eframe/0.34.2/ before depending on it.
