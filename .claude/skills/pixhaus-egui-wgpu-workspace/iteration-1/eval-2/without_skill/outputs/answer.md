# Tuning the egui-wgpu setup in an eframe app (egui-wgpu 0.34)

All four knobs you want live in `eframe::NativeOptions`, but in two different
places:

- **GPU selection, latency, and adapter/device options** live in
  `NativeOptions::wgpu_options`, which is an `egui_wgpu::WgpuConfiguration`.
- **vsync** is its own field on `NativeOptions` (`vsync: bool`), separate from
  the wgpu config.

The texture-dimension limit is the subtle one: you can't just hand wgpu a
`Limits` struct directly in 0.34. `WgpuConfiguration` exposes a
`wgpu_setup` field (an `egui_wgpu::WgpuSetup` enum). The `CreateNew` variant
carries the `power_preference` and a `device_descriptor` closure where you set
`Limits`. That closure is where the 8K texture limit goes.

Here's the full setup.

```rust
use std::sync::Arc;

use eframe::NativeOptions;
use egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
use wgpu::PowerPreference;

fn native_options() -> NativeOptions {
    let wgpu_setup = WgpuSetupCreateNew {
        // 1. Prefer the discrete / high-performance GPU.
        //    On a laptop with hybrid graphics this picks the dGPU over the iGPU.
        instance_descriptor: wgpu::InstanceDescriptor {
            // Keep the default backend set (Vulkan/Metal/DX12) unless you have a
            // reason to narrow it. PRIMARY excludes GL, which is what you want
            // for a wgpu-native canvas.
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        },
        power_preference: PowerPreference::HighPerformance,

        // 2. Device descriptor: raise the texture-dimension limit for 8K.
        device_descriptor: Arc::new(|adapter| {
            // Start from the adapter's real limits so you never request more
            // than the hardware can give — that would fail device creation.
            let adapter_limits = adapter.limits();

            let mut limits = wgpu::Limits {
                // 8192 is enough for an 8K canvas as a single texture.
                // The wgpu *default* is 8192 already, but downlevel/GL defaults
                // are 2048, so set it explicitly and clamp to the adapter max.
                max_texture_dimension_2d: 8192,
                ..wgpu::Limits::default()
            };

            // Don't exceed what the adapter supports. If the adapter can't do
            // 8192 you have a different problem (tile the canvas), but at least
            // device creation won't hard-fail here.
            limits.max_texture_dimension_2d = limits
                .max_texture_dimension_2d
                .min(adapter_limits.max_texture_dimension_2d);

            // If you also store the canvas as one big buffer for readback,
            // bump max_buffer_size / max_storage_buffer_binding_size too:
            // 8192 * 8192 * 4 bytes = 256 MiB.
            limits.max_buffer_size = limits
                .max_buffer_size
                .max(8192 * 8192 * 4)
                .min(adapter_limits.max_buffer_size);

            wgpu::DeviceDescriptor {
                label: Some("pixhaus-device"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                // Favor performance over low memory on a desktop editor.
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            }
        }),
    };

    let wgpu_options = WgpuConfiguration {
        // The setup we just built. WgpuSetup is an enum:
        //   - CreateNew: egui makes the instance/adapter/device (what we want)
        //   - Existing:  you pass in an already-created instance/adapter/device/queue
        wgpu_setup: WgpuSetup::CreateNew(wgpu_setup),

        // 3. Minimize input latency for brush strokes.
        //    desired_maximum_frame_latency caps how many frames the present
        //    queue may buffer. The wgpu/egui default is effectively 2; setting
        //    it to 1 means a freshly painted frame reaches the screen one
        //    refresh sooner. This is the single biggest latency lever while
        //    keeping vsync on. The trade-off is slightly less slack against
        //    frame-time spikes, which is fine for a paint app where you control
        //    your own draw cost.
        desired_maximum_frame_latency: Some(1),

        // present_mode is left at its default (AutoVsync) because we want vsync.
        // See the vsync note below — don't set Immediate here if you want vsync.
        ..Default::default()
    };

    NativeOptions {
        // 4. vsync lives on NativeOptions, NOT on WgpuConfiguration.
        //    `true` lets eframe pick a vsync present mode (Fifo/AutoVsync).
        //    Combined with desired_maximum_frame_latency: Some(1) above you get
        //    "vsync on, but as little buffered latency as possible".
        vsync: true,

        wgpu_options,

        // Make sure eframe actually uses the wgpu backend, not glow.
        renderer: eframe::Renderer::Wgpu,

        ..Default::default()
    }
}

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "Pixhaus",
        native_options(),
        Box::new(|_cc| Ok(Box::new(PixhausApp::default()))),
    )
}
```

## Field-by-field, so you know what does what

### Discrete GPU — `power_preference`
`WgpuSetupCreateNew::power_preference = PowerPreference::HighPerformance`.
That's the hint the adapter request passes to the driver. On Windows/macOS
laptops with switchable graphics this is what gets you the dGPU. It's a hint,
not a guarantee — pair it on Windows with the usual driver-side exports
(`NvOptimusEnablement` / `AmdPowerXpressRequestHighPerformance`) if you must
force it, but `HighPerformance` covers the common case.

You can verify which adapter you actually got at startup from the
`eframe::CreationContext`: read `cc.wgpu_render_state` and log
`render_state.adapter.get_info()`. Do this once in your `App::new` so you can
confirm the dGPU was picked.

### vsync — `NativeOptions::vsync`
This is the one that trips people up: vsync is **not** in `WgpuConfiguration`.
It's a top-level `bool` on `NativeOptions`. `vsync: true` keeps tearing off.
Leave `WgpuConfiguration::present_mode` at its default; if you forced it to
`PresentMode::Immediate` you'd be turning vsync back off and contradicting the
`vsync: true` flag.

### Input latency — `desired_maximum_frame_latency`
`WgpuConfiguration::desired_maximum_frame_latency: Some(1)`. This is the
vsync-friendly latency control. With vsync on, the present queue normally
buffers up to ~2 frames; capping it at 1 shaves roughly one refresh interval
(~16 ms at 60 Hz) off the time between "brush pixel painted" and "pixel on
screen". This is the right lever for a drawing tool — you keep vsync (no
tearing) but stop the compositor from sitting on finished frames.

Two complementary things worth doing alongside it:
- Call `ctx.request_repaint()` while a stroke is in progress so egui doesn't
  idle between input events — otherwise reduced frame latency buys nothing if
  the app isn't redrawing.
- If you later find 60 Hz vsync still feels laggy for fast strokes, the next
  step is a higher-refresh present path, not turning vsync off. Keep `vsync:
  true` as the default.

### 8K textures — the `device_descriptor` closure
`required_limits.max_texture_dimension_2d` must be at least your canvas
dimension. wgpu's plain `Limits::default()` is already 8192, but
`Limits::downlevel_defaults()` (and the GL backend) cap at 2048, so set it
explicitly. Always clamp to `adapter.limits().max_texture_dimension_2d` so you
never request beyond hardware — an over-large request makes device creation
fail outright. Most desktop GPUs report 16384, so 8192 is safe.

If you keep the canvas as one large GPU buffer for compute or readback, also
raise `max_buffer_size` (and `max_storage_buffer_binding_size` if you bind it
as storage): 8192×8192×4 = 256 MiB, which exceeds some default buffer-size
limits.

## 0.34 gotchas

- `WgpuConfiguration` does **not** have a bare `power_preference` /
  `device_descriptor` field at the top level. They moved inside
  `WgpuSetup::CreateNew(WgpuSetupCreateNew { .. })`. Older examples that set
  `WgpuConfiguration { power_preference, device_descriptor, .. }` directly are
  pre-0.19-era and won't compile.
- `device_descriptor` is an `Arc<dyn Fn(&wgpu::Adapter) -> wgpu::DeviceDescriptor>`,
  so wrap your closure in `Arc::new(...)`.
- `DeviceDescriptor` in current wgpu has `memory_hints` and `trace` fields and
  uses `required_features` / `required_limits` (not the old `features` /
  `limits`). If clippy complains about missing fields, that's the version skew
  — use `..` sparingly here since the struct is not `#[non_exhaustive]` in the
  way you'd hope.
- Set `renderer: eframe::Renderer::Wgpu` explicitly, and make sure the `wgpu`
  feature is enabled on `eframe` in `Cargo.toml`. Otherwise `wgpu_options` is
  silently ignored.

## Where to confirm it worked

In `App::new`, pull the render state and log the adapter and limits:

```rust
if let Some(rs) = cc.wgpu_render_state.as_ref() {
    let info = rs.adapter.get_info();
    log::info!("gpu: {} ({:?}, {:?})", info.name, info.device_type, info.backend);
    log::info!("max_texture_2d: {}", rs.device.limits().max_texture_dimension_2d);
}
```

`device_type` should read `DiscreteGpu` and `max_texture_dimension_2d` should
be ≥ 8192. If `device_type` comes back `IntegratedGpu` on a hybrid machine,
the OS/driver overrode the preference — that's a driver-profile issue, not a
code one.
