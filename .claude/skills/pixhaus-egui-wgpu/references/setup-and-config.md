# Setup, configuration, and errors

egui-wgpu 0.34.2. How wgpu gets configured for egui — the instance/adapter/device choices,
present mode, surface-error policy — plus the error type and the helper functions. In
Pixhaus-on-eframe you set most of this through eframe's `WgpuConfiguration` field; you build
it by hand only on the offscreen/test path.

## `WgpuConfiguration` — the top-level config

```rust
pub struct WgpuConfiguration {
    pub present_mode: wgpu::PresentMode,
    pub desired_maximum_frame_latency: Option<u32>,
    pub wgpu_setup: WgpuSetup,
    pub on_surface_status:
        Arc<dyn Fn(&wgpu::CurrentSurfaceTexture) -> SurfaceErrorAction + Send + Sync>,
}
// implements Default
```

- `present_mode` — vsync policy for the primary surface. `PresentMode::AutoVsync` for the
  editor (no tearing, low power when idle); `AutoNoVsync` only if you're measuring raw
  throughput. egui repaints on demand, so vsync rarely costs you latency that matters.
- `desired_maximum_frame_latency` — `Some(1)` for low latency (responsive brush), `Some(2)`
  for throughput, `None` for the wgpu default. A pixel editor wants low latency.
- `wgpu_setup` — how the adapter/device get created (next section).
- `on_surface_status` — called whenever acquiring a frame doesn't return `Success`; return
  a `SurfaceErrorAction`. The default recreates the surface.

verify: docs.rs did not render the exact `Default` field values; the eframe defaults are
sensible (auto-vsync, recreate-on-error). Confirm against source if a specific default is
load-bearing.

## `WgpuSetup` — create new vs reuse existing

```rust
pub enum WgpuSetup {
    CreateNew(WgpuSetupCreateNew),   // let egui-wgpu build instance/adapter/device (default)
    Existing(WgpuSetupExisting),     // hand it an already-built wgpu setup
}

impl WgpuSetup {
    pub fn from_display_handle(display_handle: impl EguiDisplayHandle) -> Self; // -> CreateNew
    pub fn without_display_handle() -> Self;                                    // -> CreateNew
    pub async fn new_instance(&self) -> wgpu::Instance; // builds or clones the instance
}
// From<WgpuSetupCreateNew> and From<WgpuSetupExisting> for WgpuSetup
```

`CreateNew` honors the standard wgpu env vars (`WGPU_BACKEND`, `WGPU_POWER_PREF`,
`WGPU_VALIDATION`, `WGPU_TRACE`, etc.), which is handy for debugging adapter selection
without recompiling.

### `WgpuSetupCreateNew`

```rust
pub struct WgpuSetupCreateNew {
    pub instance_descriptor: wgpu::InstanceDescriptor,
    pub display_handle: Option<Box<dyn EguiDisplayHandle>>,
    pub power_preference: wgpu::PowerPreference,
    pub native_adapter_selector: Option<NativeAdapterSelectorMethod>,
    pub device_descriptor:
        Arc<dyn Fn(&wgpu::Adapter) -> wgpu::DeviceDescriptor<'static> + Send + Sync>,
}

impl WgpuSetupCreateNew {
    pub fn from_display_handle(display_handle: impl EguiDisplayHandle) -> Self; // recommended
    pub fn without_display_handle() -> Self;  // headless / most platforms
}
// implements Default; From<WgpuSetupCreateNew> for WgpuSetup
```

- `device_descriptor` is a closure: given the chosen adapter, return the
  `DeviceDescriptor` (limits, required features). This is where you request higher texture
  limits if the 8K canvas needs them — branch on `adapter.limits()`.
- `power_preference` picks the adapter when `native_adapter_selector` is `None`. Use
  `HighPerformance` for a GPU-bound editor.
- `native_adapter_selector` overrides selection entirely (see the type below).
- Leave the display field in `instance_descriptor` as `None`; the `display_handle` field is
  injected at instance creation.

### `WgpuSetupExisting`

```rust
pub struct WgpuSetupExisting {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}
// From<WgpuSetupExisting> for WgpuSetup
```

Fill all four from a setup you already built, then `WgpuSetup::Existing(setup)` (or
`.into()`). Use this when the `render` crate already owns a wgpu device — e.g. an offscreen
compositor created in a test — and you want egui to share it rather than make a second one.
Two devices means two of every GPU resource and no sharing.

## `RenderState::create` — building it all by hand

```rust
pub async fn create(
    config: &WgpuConfiguration,
    instance: &wgpu::Instance,
    compatible_surface: Option<&wgpu::Surface<'static>>,
    options: RendererOptions,
) -> Result<RenderState, WgpuError>
```

`async` — block on it with pollster on the sync/test path, not tokio (see
[[pixhaus-pollster]]). `compatible_surface` lets adapter selection prefer an adapter that
can present to your window; pass `None` for offscreen.

## `NativeAdapterSelectorMethod` — custom adapter choice

```rust
pub type NativeAdapterSelectorMethod = Arc<
    dyn Fn(&[wgpu::Adapter], Option<&wgpu::Surface<'_>>) -> Result<wgpu::Adapter, String>
        + Send + Sync,
>;
```

Given all available adapters and an optional surface for compatibility checks, return the
chosen adapter or an error message. No effect on web (there `power_preference` wins). Use
this to pin a discrete GPU or skip a known-bad driver.

## `EguiDisplayHandle` — cloneable display handle

```rust
pub trait EguiDisplayHandle:
    raw_window_handle::HasDisplayHandle + Debug + Send + Sync + 'static
{
    fn clone_for_wgpu(&self) -> Box<dyn WgpuHasDisplayHandle>;
    fn clone_display_handle(&self) -> Box<dyn EguiDisplayHandle>;
}
// blanket impl for any T: HasDisplayHandle + Clone + Debug + Send + Sync + 'static
```

wgpu's display handle isn't cloneable; this trait wraps one so it can live alongside the
egui config and be cloned. There's a blanket impl, so you rarely implement it yourself —
`winit::event_loop::OwnedDisplayHandle` already qualifies. Relevant only when you build the
instance manually (eframe handles it).

## `SurfaceErrorAction`

```rust
pub enum SurfaceErrorAction {
    SkipFrame,         // do nothing, skip this frame
    RecreateSurface,   // recreate the surface, then skip this frame
}
// Copy, Clone
```

Returned from `WgpuConfiguration::on_surface_status`. `RecreateSurface` is the right
default for a desktop window (handles resize/minimize/device-lost races); `SkipFrame` if
you want to handle recreation yourself.

## `WgpuError`

```rust
pub enum WgpuError {
    RequestAdapterError(wgpu::RequestAdapterError),
    CustomNativeAdapterSelectionError(String),   // from your NativeAdapterSelectorMethod
    NoSurfaceFormatsAvailable,
    RequestDeviceError(wgpu::RequestDeviceError),
    CreateSurfaceError(wgpu::CreateSurfaceError),
    HandleError(raw_window_handle::HandleError),
}
// Debug, Display, std::error::Error (with source()); From for each wrapped wgpu error
```

It implements `std::error::Error`, so in the `render` crate wrap it in your `thiserror`
enum with `#[from]`; in the binary `anyhow` swallows it directly (see
[[pixhaus-rust-conventions]] on errors). The most common variant in practice is
`RequestAdapterError` (no compatible GPU) and `RequestDeviceError` (requested limits the
adapter can't meet).

## Free functions

```rust
// Pick the framebuffer format egui prefers from a candidate list (errs if list is empty).
pub fn preferred_framebuffer_format(formats: &[TextureFormat]) -> Result<TextureFormat, WgpuError>

// Map epi/eframe depth+stencil bit counts to a wgpu depth format.
pub fn depth_format_from_bits(depth_buffer: u8, stencil_buffer: u8) -> Option<TextureFormat>

// Human-readable one-line adapter summary (for logging at startup).
pub fn adapter_info_summary(info: &wgpu::AdapterInfo) -> String

// Human-readable GPU vendor from the numeric vendor id.
pub fn parse_vendor_id(vendor_id: u32) -> &'static str
```

`adapter_info_summary` is worth logging once at boot — it tells you which GPU and backend
the editor actually picked, which is the first thing you want when a user reports a render
bug.

## Feature flags

| Feature | Effect |
|---|---|
| `winit` | The `egui_wgpu::winit::Painter` integration. eframe enables it; you don't in the `render` crate. |
| `wayland` / `x11` | Linux display backends, gated behind `winit`. |
| `capture` | The `capture` module for reading frames back to the CPU. |
| `fragile-send-sync-non-atomic-wasm` (default) | Makes the renderer `Sync` on wasm. Irrelevant to desktop Pixhaus but on by default. |
| `macos-window-resize-jitter-fix` (default) | Reduces resize jitter on macOS Metal. Leave on. |

In the `render` crate, depend on egui-wgpu with no extra features. The binary gets the
`winit` path transitively through eframe.
