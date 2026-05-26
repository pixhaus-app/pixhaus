# NativeOptions, the window, and the renderer

eframe 0.34.2. Everything you set before the window exists: `NativeOptions`, the
`ViewportBuilder` that configures the OS window, the renderer choice, the window icon, and
the cargo features that gate it all.

## Contents
- `NativeOptions` — every field
- `ViewportBuilder` — the window
- `Renderer` and `HardwareAcceleration`
- The window icon (`icon_data`)
- Additional windows (viewports)
- Feature flags

## NativeOptions — every field

Built once in `main`, passed to `run_native`. `NativeOptions::default()` is the base;
override fields with struct-update syntax. Fields (type, default, what it does):

| Field | Type | Default | Notes |
|---|---|---|---|
| `viewport` | `egui::ViewportBuilder` | default | The root OS window: title, size, icon, decorations, fullscreen. See below. This is where most window config lives. |
| `renderer` | `Renderer` | backend-dependent | `Glow` or `Wgpu`. Pixhaus sets `Wgpu` explicitly. |
| `vsync` | `bool` | `true` | Cap FPS to the display refresh. **Only affects glow** — wgpu vsync is set via `wgpu_options`'s present mode. |
| `multisampling` | `u16` | `0` | MSAA level; power of two; `0` = off. egui's own AA is usually enough; a custom wgpu pass manages its own MSAA. |
| `depth_buffer` | `u8` | `0` | Depth bits. egui needs none; set if your wgpu canvas pass wants a depth attachment. |
| `stencil_buffer` | `u8` | `0` | Stencil bits. egui needs none. |
| `hardware_acceleration` | `HardwareAcceleration` | `Preferred` | Prefer/require/forbid a HW GPU. **glow only.** |
| `run_and_return` | `bool` | `true` | If `true`, `run_native` returns after the window closes (control flow continues in `main`); if `false` it may exit the process. Keep `true` to run cleanup after the loop. |
| `event_loop_builder` | `Option<EventLoopBuilderHook>` | `None` | Boxed hook to tweak the winit `EventLoop` before it runs (platform-specific flags). |
| `window_builder` | `Option<WindowBuilderHook>` | `None` | Boxed hook to tweak the window attributes the platform way. Prefer `viewport` for ordinary settings. |
| `shader_version` | `Option<ShaderVersion>` | `None` | glow GLSL version override (e.g. VirtualBox VMSVGA / GLES). Irrelevant to wgpu. |
| `centered` | `bool` | `false` | Center the window on launch. Not supported on Wayland. |
| `wgpu_options` | `egui_wgpu::WgpuConfiguration` | default | Instance/adapter/device/surface and present-mode config for the wgpu backend. This is where wgpu present mode (vsync), power preference, and required limits/features go. |
| `persist_window` | `bool` | `true`* | Persist window position and size between runs. *Effective only with the `persistence` feature. |
| `persistence_path` | `Option<PathBuf>` | `None` | Override the folder eframe stores state in. `None` = OS default (see `storage-and-persistence.md`). |
| `dithering` | `bool` | `true` | Dither the sRGB output to kill gradient banding. Leave on. |

Note the split: **glow-only knobs** (`vsync`, `multisampling` for the main surface,
`hardware_acceleration`, `shader_version`) do nothing on the wgpu backend Pixhaus uses.
For wgpu, the equivalents live in `wgpu_options` (`WgpuConfiguration`) — present mode,
power preference, device limits. Set those there, not on the top-level fields.

```rust
let options = eframe::NativeOptions {
    renderer: eframe::Renderer::Wgpu,
    viewport: egui::ViewportBuilder::default()
        .with_title("Pixhaus")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([640.0, 480.0])
        .with_icon(load_icon()),
    persist_window: true,
    ..Default::default()
};
```

## ViewportBuilder — the window

`viewport` is an `egui::ViewportBuilder` (an egui type, not eframe). It is the real window
config surface. The builders you'll reach for:

- `.with_title(&str)` — title-bar text (distinct from the app id passed to `run_native`).
- `.with_inner_size([w, h])` / `.with_min_inner_size(..)` / `.with_max_inner_size(..)`
- `.with_position([x, y])`
- `.with_resizable(bool)`
- `.with_decorations(bool)` — native title bar and border on/off (off for custom chrome).
- `.with_fullscreen(bool)` / `.with_maximized(bool)`
- `.with_transparent(bool)`
- `.with_icon(Arc<egui::IconData>)` — taskbar/title icon (see below).
- `.with_app_id(&str)` — Wayland/X11 app id for grouping.
- `.with_window_level(..)`, `.with_taskbar(bool)`, `.with_drag_and_drop(bool)`

At runtime, change these by sending `egui::ViewportCommand`s
(`ctx.send_viewport_cmd(ViewportCommand::Fullscreen(true))`,
`::InnerSize`, `::Title`, `::Close`) — the builder is startup-only.

## Renderer and HardwareAcceleration

```rust
pub enum Renderer { Glow, Wgpu }
```

- `Wgpu` — the `egui-wgpu` backend. Required for Pixhaus's custom canvas paint callback.
- `Glow` — the `egui_glow` (OpenGL) backend. Smaller binary, but no path to the wgpu canvas.
- `Default` for `Renderer` is gated on features: it picks `Glow` if the `glow` feature is
  on, else `Wgpu`. Because Pixhaus disables default features and enables only `wgpu`, the
  default is already `Wgpu` — but set it explicitly so a future feature edit can't silently
  flip it. `Display`/`FromStr` exist (also feature-gated) for config files.

```rust
pub enum HardwareAcceleration { Preferred, Required, Off }
```
glow-only. `Preferred` falls back to software if no GPU; `Required` errors without one;
`Off` forces software. For wgpu, express this through `wgpu_options.power_preference`.

## The window icon (icon_data)

The `eframe::icon_data` module helps build the `egui::IconData` that
`ViewportBuilder::with_icon` wants:

```rust
fn load_icon() -> std::sync::Arc<egui::IconData> {
    let bytes = include_bytes!("../assets/icon.png");
    std::sync::Arc::new(
        eframe::icon_data::from_png_bytes(bytes).expect("valid PNG icon"),
    )
}
```

`from_png_bytes(&[u8]) -> Result<egui::IconData, _>` decodes a PNG into raw RGBA + size.
The module also exposes an `IconDataExt` trait with conversion helpers. Embed the PNG with
`include_bytes!` so the icon ships in the binary.

## Additional windows (viewports)

eframe runs one `App`. To open more native windows, don't start a second eframe — use egui
*viewports* from inside `ui`:

- `ctx.show_viewport_deferred(id, builder, |ctx, class| { … })` — a persistent second window
  whose contents you redraw each frame (a detached tool window, a preview).
- `ctx.show_viewport_immediate(id, builder, |ctx, class| { … })` — synchronous, for modal-
  style secondary windows.

Each takes its own `ViewportBuilder`. This is the supported multi-window path; the `App::ui`
doc explicitly points here for "additional OS windows."

## Feature flags

eframe's defaults include `glow`, `wgpu`, `accesskit`, `default_fonts`, `wayland`, `x11`,
`web_screen_reader`. Pixhaus wants a lean, wgpu-only desktop build:

```toml
eframe = { version = "0.34", default-features = false, features = ["wgpu", "persistence"] }
```

| Feature | Default | Keep for Pixhaus? | Purpose |
|---|---|---|---|
| `wgpu` | ✓ | yes | wgpu backend via `egui-wgpu` — the canvas depends on it. |
| `persistence` | | yes | Saving app state, window geometry, egui memory to disk. Off by default — opt in. |
| `glow` | | no | OpenGL backend. Not used; dropping it shrinks the binary. |
| `default_fonts` | ✓ | optional | Bundles default fonts. Drop only if you ship your own. |
| `accesskit` | ✓ | optional | Platform accessibility. Keep unless size-critical. |
| `wayland` / `x11` | ✓ | yes on Linux | Linux display server support; keep both for portability. |
| `web_screen_reader` | ✓ | no | Web-only; irrelevant to desktop. |
| `android-*` | | no | Android backends. |

Selecting `default-features = false` then re-adding `wgpu` + `persistence` is the
deliberate choice — see the SKILL.md version table. glow and wgpu *can* coexist if you ever
want a runtime renderer switch, but Pixhaus has locked wgpu.

## Flagged / verify

- `from_png_bytes` return type is `Result<egui::IconData, _>`; the exact error type wasn't
  rendered in the scraped docs — `.expect()`/`?` against it and confirm if you need the
  concrete error.
- `persist_window`'s documented default wasn't explicit in the scrape; treat it as effective
  only with the `persistence` feature and set it explicitly to be safe.
