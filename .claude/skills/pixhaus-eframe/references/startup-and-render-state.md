# Startup: entry points, CreationContext, and the render state

eframe 0.34.2. How the app actually boots: the `run_*` entry points, the `AppCreator`
closure, the `CreationContext` you get inside it, and how to grab the wgpu render state and
install fonts/visuals before the first frame.

## Contents
- Entry points (`run_native`, `run_ui_native`, deprecated `run_simple_native`, `create_native`)
- `AppCreator`
- `CreationContext` — fields
- Grabbing the wgpu render state
- One-time setup: fonts and visuals
- `Error` and `eframe::Result`

## Entry points

```rust
pub fn run_native(
    app_name: &str,
    native_options: NativeOptions,
    app_creator: AppCreator<'_>,
) -> eframe::Result
```
The standard desktop entry point and the one Pixhaus uses. `app_name` is the **app id**: the
persistence save-location key and the Wayland application id — not the window title (that's
`ViewportBuilder::with_title`). Can fail if the graphics context can't be set up, so return
its `Result` from `main`. Blocks running the event loop until the window closes (with
`run_and_return = true`, control returns here afterward).

```rust
pub fn run_ui_native(
    app_name: &str,
    native_options: NativeOptions,
    ui_fun: impl FnMut(&mut egui::Ui, &mut eframe::Frame) + 'static,
) -> eframe::Result
```
The simplest start: a bare closure, no `App` struct. Persists egui data (window positions
etc.) but **not custom user data** — there's no `save` hook. Good for a quick repro or a
one-off tool; not for Pixhaus, which needs custom persistence and lifecycle hooks.

```rust
#[deprecated] pub fn run_simple_native(app_name, native_options, update_fun: impl FnMut(&Context, &mut Frame)) -> Result
```
The predecessor to `run_ui_native`, taking a `&Context` closure. Deprecated — its presence
in example code is another tell that the code predates 0.34. Use `run_ui_native`.

```rust
pub fn create_native(...) -> EframeWinitApplication<...>
```
Builds the eframe winit application on a **custom event loop** you own, instead of letting
`run_native` create and run one. Advanced — reach for it only if you must drive the winit
event loop yourself (e.g. integrating another event source). Pairs with `EframePumpStatus`
for manual pumping. Pixhaus does not need this.

## AppCreator

```rust
pub type AppCreator<'app> =
    Box<dyn FnOnce(&CreationContext<'_>) -> Result<Box<dyn App + 'app>, Box<dyn Error + Send + Sync>> + 'app>;
```
The third argument to `run_native`. A boxed once-closure: given the `CreationContext`,
return your boxed `App` or an error. The `Ok(Box::new(...))` wrapper is required, and you
can fail startup by returning `Err` (e.g. a required asset failed to load):

```rust
eframe::run_native("Pixhaus", options, Box::new(|cc| {
    let app = Pixhaus::new(cc)?;     // propagate a startup error if you have a fallible new
    Ok(Box::new(app))
}))
```

## CreationContext — fields

Passed to the `AppCreator`. The one chance to do setup with the egui context and backend
handles before the first frame. Public fields:

| Field | Type | Use |
|---|---|---|
| `egui_ctx` | `egui::Context` | Customize egui at boot: `set_fonts`, `set_visuals`, `set_style`, `set_zoom_factor`. Clone it to hand to worker threads for `request_repaint`. |
| `integration_info` | `IntegrationInfo` | Info about the surrounding environment. |
| `storage` | `Option<&dyn Storage>` | Restore persisted state (`eframe::get_value`). `Some` only with the `persistence` feature. |
| `wgpu_render_state` | `Option<egui_wgpu::RenderState>` | The wgpu device/queue/target format. `Some` with the `wgpu` feature + `Renderer::Wgpu`. The handle the canvas needs. |
| `gl` | `Option<Arc<glow::Context>>` | glow context. `None` for Pixhaus (wgpu). |
| `get_proc_address` | `Option<Arc<dyn Fn(&CStr) -> *const c_void + Send + Sync>>` | GL proc loader; glow only. |

## Grabbing the wgpu render state

This is the load-bearing reason `new` takes `cc`. `RenderState` carries the `wgpu::Device`,
`wgpu::Queue`, and the target texture format the canvas paint callback must match.

```rust
fn new(cc: &eframe::CreationContext<'_>) -> Self {
    let render_state = cc
        .wgpu_render_state
        .as_ref()
        .expect("eframe was started with Renderer::Wgpu");

    let device: &wgpu::Device = &render_state.device;
    let queue:  &wgpu::Queue  = &render_state.queue;
    let target_format = render_state.target_format;

    // Build the canvas's pipeline/bind groups now and stash them in the
    // egui_wgpu callback resources — see pixhaus-egui → custom-wgpu-canvas.md.
    // ...
}
```

If `wgpu_render_state` is `None`, you started with the wrong renderer or without the `wgpu`
feature — that's the bug, not a reason to branch. The same `RenderState` is reachable
mid-run via `frame.wgpu_render_state()`; prefer the startup one for one-time setup.

## One-time setup: fonts and visuals

Do it in `new`, against `cc.egui_ctx`, so it's set before the first paint:

```rust
fn install_theme(ctx: &egui::Context) {
    // fonts
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("ui".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/Inter.ttf"))));
    fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "ui".into());
    ctx.set_fonts(fonts);

    // visuals (full palette in pixhaus-egui → input-state-and-theming.md)
    ctx.set_visuals(egui::Visuals::dark());
}
```

Setting fonts is the expensive part — do it once here, never per frame.

## Error and eframe::Result

```rust
pub type Result<T = ()> = std::result::Result<T, eframe::Error>;
```
`eframe::Result` defaults its `Ok` type to `()`, so `fn main() -> eframe::Result` is the
idiomatic signature — propagate startup failures straight out of `main`. `eframe::Error`
enumerates the things that can go wrong starting the app (graphics/winit setup, backend
init). You rarely match on it; the value is letting `?` bubble a startup failure into a
clean process exit with a printed error rather than a panic.

In the Pixhaus binary, the `anyhow`/`thiserror` split from `pixhaus-rust-conventions` still
holds: `main` can return `eframe::Result` directly, or you can wrap startup in `anyhow` and
convert. Don't expose `eframe::Error` from library crates.

## Flagged / verify

- The exact `AppCreator` type alias (the `'app` lifetime and the boxed-error type) was not
  fully rendered in the scrape; the shape above matches eframe's published signature, but
  confirm the lifetime parameter against docs.rs if you write the alias by hand rather than
  passing a `Box::new(|cc| …)` literal (which infers it).
