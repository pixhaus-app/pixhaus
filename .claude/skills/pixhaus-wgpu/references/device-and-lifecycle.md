# wgpu 29.0.1 — instance / adapter / device / queue, capabilities, errors

Source: https://docs.rs/wgpu/29.0.1/wgpu/. Signatures copied from the v29.0.1 rendered
docs. wgpu re-exports most descriptor/limit/enum types from `wgpu-types`, so several names
below are type aliases (their `struct.*` pages 404 — use `type.*`).

In production the `render` crate does NOT create any of this: eframe/egui-wgpu hands you a
`wgpu::Device`, `wgpu::Queue`, and target `TextureFormat` at startup (see the
pixhaus-eframe skill for where they come from). The code here is for the headless device
the `render` crate builds in its own tests, benches, and examples. There is no tokio there,
so drive these futures with `pollster::block_on` — see the pixhaus-pollster skill.

## v29 deltas vs older wgpu (read first — your memory is probably stale)

- `request_adapter` and `request_device` are **fallible**. `request_adapter` returns
  `Result<Adapter, RequestAdapterError>` (older wgpu returned `Option<Adapter>`); don't
  `.expect()` on an `Option`. `request_device` returns `Result<(Device, Queue), RequestDeviceError>`.
- Both are `impl Future` (not `async fn`), carrying `+ WasmNotSend`. `.await` them.
- **`Maintain` is gone.** `Device::poll` takes `PollType` and returns
  `Result<PollStatus, PollError>`. Old `Maintain::Wait`/`Maintain::Poll` map to
  `PollType::Wait { .. }` / `PollType::Poll`.
- Error scopes return an `ErrorScopeGuard`; call `.pop()` (a future) on the guard. There is
  no free `Device::pop_error_scope` in v29.
- `on_uncaptured_error` takes `Arc<dyn UncapturedErrorHandler>` (not `Box`).
- `DeviceDescriptor` gained `experimental_features: ExperimentalFeatures` and uses
  `trace: Trace` (an enum) instead of `Option<&Path>`.

## Instance

```rust
pub fn new(desc: InstanceDescriptor) -> Self
fn default() -> Self   // Backends::all()

pub fn request_adapter(
    &self,
    options: &RequestAdapterOptions<'_, '_>,
) -> impl Future<Output = Result<Adapter, RequestAdapterError>> + WasmNotSend

pub fn enumerate_adapters(&self, backends: Backends) -> impl Future<Output = Vec<Adapter>>
pub fn create_surface<'window>(
    &self, target: impl Into<SurfaceTarget<'window>>,
) -> Result<Surface<'window>, CreateSurfaceError>
```

`RequestAdapterOptions<'a,'b>` = `RequestAdapterOptionsBase<&'a Surface<'b>>`:
`power_preference: PowerPreference`, `force_fallback_adapter: bool`,
`compatible_surface: Option<&Surface>` (pass `None` for headless).

## Adapter

```rust
pub fn request_device(
    &self, desc: &DeviceDescriptor<'_>,
) -> impl Future<Output = Result<(Device, Queue), RequestDeviceError>> + WasmNotSend

pub fn features(&self) -> Features
pub fn limits(&self) -> Limits
pub fn get_info(&self) -> AdapterInfo
pub fn get_downlevel_capabilities(&self) -> DownlevelCapabilities
pub fn get_texture_format_features(&self, format: TextureFormat) -> TextureFormatFeatures
```

`DeviceDescriptor<'a>` (`label: Label<'a>` = `Option<&str>`):
`required_features: Features`, `required_limits: Limits`,
`experimental_features: ExperimentalFeatures`, `memory_hints: MemoryHints`, `trace: Trace`.

## Capabilities — Limits / Features

`Limits::default()` sets `max_texture_dimension_2d = 8192`. That is exactly the Pixhaus 8K
ceiling, with zero headroom — a single 8192×8192 texture fits the default, and anything
larger on a side needs either a higher `required_limits` (which the adapter must support)
or tiling. Don't assume defaults when egui-wgpu hands you the device: read
`adapter.limits().max_texture_dimension_2d` and clamp canvas allocation to it, returning a
`thiserror` variant if a requested canvas exceeds it.

```rust
Limits::default()                    // modern backends; max_texture_dimension_2d = 8192
Limits::downlevel_defaults()         // GLES-3.1 / D3D11; max_texture_dimension_2d = 2048
Limits::downlevel_webgl2_defaults()  // WebGL2
```

`MemoryHints` (default `Performance`): `Performance`, `MemoryUsage`,
`Manual { suballocated_device_memory_block_size: Range<u64> }`. `Trace::Off` is the
no-trace default.

## Device

```rust
pub fn features(&self) -> Features
pub fn limits(&self) -> Limits
pub fn create_texture(&self, desc: &TextureDescriptor<'_>) -> Texture
pub fn create_buffer(&self, desc: &BufferDescriptor<'_>) -> Buffer
pub fn poll(&self, poll_type: PollType) -> Result<PollStatus, PollError>
pub fn push_error_scope(&self, filter: ErrorFilter) -> ErrorScopeGuard
pub fn on_uncaptured_error(&self, handler: Arc<dyn UncapturedErrorHandler>)
pub fn set_device_lost_callback(&self, callback: impl Fn(DeviceLostReason, String) + Send + 'static)
```

`Device`, `Queue`, `Buffer`, `Texture`, etc. are cheap cloneable handles (internally
ref-counted), not large owned blobs. Cloning a `Queue` to hand to another thread is fine;
it does not duplicate GPU memory. Still keep a single logical owner per CLAUDE.md — clone at
a thread boundary, not to dodge the borrow checker.

### Poll API (replaces Maintain)

`PollType` variants: `Wait { submission_index: Option<SubmissionIndex>, timeout: Option<Duration> }`
(block until that submission completes — most recent if `None` — which on native drives
buffer-map and `on_submitted_work_done` callbacks) and `Poll` (check once, non-blocking).
Returns `PollStatus` on success, `PollError` on failure. Headless readback: submit, map a
staging buffer, then `device.poll(PollType::Wait { submission_index: None, timeout: None })`
to block until the map callback fires. (`PollType::wait()` convenience ctor — VERIFY.)

### Error scopes + uncaptured handler

`ErrorFilter`: `Validation`, `OutOfMemory`, `Internal`. By default, uncaptured errors
**panic** — in a no-unwrap codebase, set a handler or use scopes around fallible recording.
Scopes nest and must be popped in reverse order; `.pop()` takes effect immediately (you
need not await before doing work outside the scope).

```rust
let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
// ... record commands / create resources ...
if let Some(err) = scope.pop().await {
    return Err(RenderError::GpuValidation(err.to_string())); // thiserror, no unwrap
}

device.on_uncaptured_error(std::sync::Arc::new(|err: wgpu::Error| {
    tracing::error!(%err, "uncaptured wgpu error"); // log + flag dead, never panic
}));
```

`Error` is an enum (`OutOfMemory`, `Validation { description, source }`, `Internal { .. }`)
implementing `std::error::Error` + `Display`. `DeviceLostReason`: `Unknown` (driver),
`Destroyed` (`Device::destroy` called).

## Queue

```rust
pub fn submit<I: IntoIterator<Item = CommandBuffer>>(&self, command_buffers: I) -> SubmissionIndex
pub fn on_submitted_work_done(&self, callback: impl FnOnce() + Send + 'static)
pub fn write_buffer(&self, buffer: &Buffer, offset: BufferAddress, data: &[u8])
pub fn write_texture(
    &self,
    texture: TexelCopyTextureInfo<'_>,   // v29 name (was ImageCopyTexture)
    data: &[u8],
    data_layout: TexelCopyBufferLayout,  // v29 name (was ImageDataLayout)
    size: Extent3d,
)
```

`submit` returns a `SubmissionIndex` — feed it to `PollType::Wait { submission_index: Some(idx), .. }`
or pair with `on_submitted_work_done`. `write_buffer`/`write_texture` are queued, flushed at
the next `submit`; there is nothing to await for an upload.

## Worked example — headless adapter+device+queue (tests/benches only)

```rust
use std::sync::Arc;

// Production: egui-wgpu already gave you device/queue/format — skip all of this.
// Headless: no tokio in the render crate, so block with pollster (pixhaus-pollster skill).
async fn init_headless() -> Result<(wgpu::Device, wgpu::Queue), InitError> {
    let instance = wgpu::Instance::default();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None, // headless
        })
        .await?; // RequestAdapterError -> InitError via #[from]

    let needed = wgpu::Limits { max_texture_dimension_2d: 8192, ..wgpu::Limits::default() };

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("pixhaus-headless"),
            required_features: wgpu::Features::empty(),
            required_limits: needed,
            experimental_features: wgpu::ExperimentalFeatures::disabled(), // VERIFY ctor
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        })
        .await?; // RequestDeviceError -> InitError

    device.on_uncaptured_error(Arc::new(|e: wgpu::Error| tracing::error!(%e, "wgpu")));
    Ok((device, queue))
}

#[derive(thiserror::Error, Debug)]
enum InitError {
    #[error("no compatible GPU adapter")]
    NoAdapter(#[from] wgpu::RequestAdapterError),
    #[error("device request failed")]
    NoDevice(#[from] wgpu::RequestDeviceError),
}
```

## Surface (headless windowed examples only)

In production egui-wgpu owns the surface, config, and target format; the `render` crate
must not create a surface. For a standalone example window:

```rust
pub fn configure(&self, device: &Device, config: &SurfaceConfiguration)
pub fn get_current_texture(&self) -> Result<SurfaceTexture, SurfaceError>
pub fn get_default_config(&self, adapter: &Adapter, width: u32, height: u32) -> Option<SurfaceConfiguration>
```

`SurfaceTexture { texture: Texture }` with `present(self)` — call after `submit`.
`SurfaceConfiguration`: `usage`, `format`, `width`, `height`, `present_mode`,
`desired_maximum_frame_latency`, `alpha_mode`, `view_formats`.

## VERIFY (confirm before relying on these)

- `ExperimentalFeatures` ctor name (`::disabled()` / `Default`); `PollType::wait()` ctor and
  exact `Wait` field names; `PollStatus`/`PollError`/`SurfaceError` variant lists;
  `SurfaceTexture.suboptimal` field; `DownlevelCapabilities`/`Trace` full variant lists;
  `UncapturedErrorHandler` blanket-impl bounds. None of these were on the rendered pages.
