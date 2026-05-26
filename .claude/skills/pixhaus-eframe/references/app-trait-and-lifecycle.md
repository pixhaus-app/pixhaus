# The App trait, the frame loop, and lifecycle

eframe 0.34.2. The `eframe::App` trait is the contract between the winit event loop and
your code: eframe calls into it; you mutate your state and draw. This file documents every
method, the order they run in, how to handle exit, and the runtime `Frame` handle.

## Contents
- The frame loop: what runs when
- `App` methods (every one, verified signatures)
- Repaint control
- Exit and graceful close
- The runtime `Frame` handle
- `IntegrationInfo`

## The frame loop: what runs when

eframe drives a winit event loop on the main thread. When the window needs painting (input
arrived, an animation is running, or `request_repaint` was called), eframe runs, per frame:

1. `App::logic(ctx, frame)` — once, before drawing. Non-UI per-frame work. Also runs when
   the window is hidden but a repaint was requested, so don't assume a paint follows.
2. `App::ui(ui, frame)` — the draw. Build the whole UI from scratch (immediate mode).

On an interval (`auto_save_interval`) and on shutdown, eframe also calls `App::save`. On the
way out it calls `App::on_exit`. Nothing is on a fixed clock — when idle, the loop sleeps.

## App methods

### Required

```rust
fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame)
```
Called each time the UI needs repainting, possibly many times per second. The `ui` is the
root viewport's `Ui` with **no margin and no background** — wrap your content in a
`CentralPanel` (or other panels) to get egui's framing. This is the only required method.
For additional OS windows, spawn them with `egui::Context::show_viewport_deferred` /
`show_viewport_immediate` rather than running a second `App` (see
`native-options-and-window.md`).

### Provided (override as needed)

```rust
fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) { }
```
Runs once before each `ui`, and also when hidden-but-repaint-requested. **You may not show
UI or paint here.** Use it to drain channels from worker threads, advance animation clocks,
or react to viewport state. This is the 0.34 home for per-frame non-drawing logic.

```rust
#[deprecated] fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) { }
```
The old per-frame entry point. Deprecated in favor of `ui` + `logic`. Old examples and
tutorials still show `update` taking `&Context` — that signature is the tell that the code
predates 0.34. Don't port it forward; split into `ui` (draw) and `logic` (the rest).

```rust
fn save(&mut self, storage: &mut dyn eframe::Storage) { }
```
Called on shutdown and at `auto_save_interval`. Persist your custom state here via
`eframe::set_value`. Only invoked with the `persistence` feature enabled. Keep it cheap —
it runs while the app is live. Details in `storage-and-persistence.md`.

```rust
fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) { }
```
Called once on shutdown, after `save`. For teardown that must happen no matter what (flush
a log, release an external resource). The `gl` argument is `Some` only on the glow backend;
it's `None` for Pixhaus (wgpu). Note: the actual close decision is already made by here —
to *prompt* before closing, intercept the close request during `ui` (see below).

```rust
fn auto_save_interval(&self) -> std::time::Duration { }
```
Time between automatic `save` calls. Default is 30 seconds. Override to change cadence;
return a very long duration to effectively save only on exit.

```rust
fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] { }
```
The background color the renderer clears to each frame, as **sRGB gamma-space** values in
`0.0..=1.0` (RGBA). Default derives from the visuals' window fill. Override if the editor
needs a specific backdrop behind everything (e.g. a neutral gray behind the canvas
viewport). Returning the wrong color space here is a common cause of "my background looks
slightly off."

```rust
fn persist_egui_memory(&self) -> bool { }
```
Whether egui's own `Memory` (window positions, collapsed sections, scroll offsets) is
persisted. Default `true` when the `persistence` feature is on. Turn off if you don't want
egui restoring widget state between runs.

```rust
fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) { }
```
Runs before egui processes input each frame. Mutate `raw_input` to filter or inject events
— drop events while a modal is up, remap keys, or feed synthetic input in tests. Reach for
this only when normal `Response`/`InputState` handling (in `pixhaus-egui`) can't express it.

## Repaint control

eframe is reactive. After interaction it paints, then sleeps. To keep painting:

```rust
ctx.request_repaint();                                  // paint again ASAP
ctx.request_repaint_after(std::time::Duration::from_millis(16));  // schedule one later
```

The pattern that matters for Pixhaus: a worker thread (AI request, PNG encode) holds a
cheap `Context` clone and calls `ctx.request_repaint()` when its result lands, waking the
loop so `logic` can drain the channel. Without that call, a result sitting in a channel
won't surface until the next unrelated repaint.

## Exit and graceful close

The window-close button sends a close *request* you can veto, routed through egui viewport
state — there is no boolean you return from `ui`:

```rust
fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
    let ctx = ui.ctx();
    if ctx.input(|i| i.viewport().close_requested()) {
        if self.doc.is_dirty() && !self.close_confirmed {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose); // veto
            self.show_unsaved_dialog = true;
        }
        // otherwise let the close proceed
    }
    // when the user picks "Discard" in your dialog:
    //   self.close_confirmed = true;
    //   ctx.send_viewport_cmd(egui::ViewportCommand::Close);
}
```

`ViewportCommand::CancelClose` aborts this frame's close; `ViewportCommand::Close` requests
one. Drive your confirm dialog over subsequent frames, then issue `Close` once resolved.
`on_exit` is too late for this — by then eframe is tearing down.

## The runtime Frame handle

`eframe::Frame` (the second arg to `ui`/`logic`) is "the surroundings of your app" — the
live handle to the backend and storage. Distinct from `CreationContext` (startup-only) and
from any notion of an egui frame counter.

```rust
fn is_web(&self) -> bool                              // always false for Pixhaus (desktop)
fn info(&self) -> &eframe::IntegrationInfo            // see below
fn storage(&self) -> Option<&dyn eframe::Storage>
fn storage_mut(&mut self) -> Option<&mut (dyn eframe::Storage + 'static)>
fn wgpu_render_state(&self) -> Option<&egui_wgpu::RenderState>   // wgpu feature only
fn gl(&self) -> Option<&Arc<glow::Context>>          // glow feature only — None for us
fn register_native_glow_texture(&mut self, native: glow::Texture) -> egui::TextureId  // glow only
```

`wgpu_render_state()` gives the same `RenderState` (device, queue, target format) you grabbed
from `CreationContext` at startup — use the startup one for setup, this one if you need the
device mid-run. On non-wasm, `Frame` also implements `raw_window_handle`'s
`HasWindowHandle` / `HasDisplayHandle`, so you can hand the native window to another library
if ever needed.

## IntegrationInfo

`frame.info()` returns `&IntegrationInfo`, frame-by-frame integration data:

- `cpu_usage: Option<f32>` — seconds the previous frame's CPU work took (paint + your code).
  Useful for a debug HUD; `None` on the first frame.
- web info fields exist but are irrelevant to the desktop build.

## Flagged / verify

- `on_exit`'s argument is the glow context option; exact type path
  (`eframe::glow::Context`) wasn't fully rendered in the scraped docs — confirm if you
  actually use the glow backend (Pixhaus doesn't).
- `raw_input_hook` exists as a provided method; confirm the exact `RawInput` field you
  intend to mutate against docs.rs, since `RawInput` is an egui type that evolves.
