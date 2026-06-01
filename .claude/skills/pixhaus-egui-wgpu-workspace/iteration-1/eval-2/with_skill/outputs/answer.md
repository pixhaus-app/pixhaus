# Tuning the egui-wgpu setup for Pixhaus

All three knobs you want — discrete-GPU preference, low-latency vsync, and raised
texture-dimension limits — live in **one place**: `egui_wgpu::WgpuConfiguration`. On
the Pixhaus-on-eframe path you don't build a `RenderState` yourself; you hand the config
to eframe through `NativeOptions.wgpu_options` and eframe creates the instance, adapter,
device, and surface from it.

The important trap first: on the **wgpu** backend the glow-only top-level `NativeOptions`
fields (`vsync`, `multisampling`, `hardware_acceleration`) do nothing. The wgpu
equivalents — present mode, power preference, device limits/features — live inside
`wgpu_options`. Set them there or they're silently ignored.

## Where each setting maps

| Goal | Field | Value |
|---|---|---|
| Prefer discrete / high-perf GPU | `wgpu_setup` → `WgpuSetupCreateNew::power_preference` (and optionally `native_adapter_selector` to pin it hard) | `wgpu::PowerPreference::HighPerformance` |
| Vsync on, no tearing | `WgpuConfiguration::present_mode` | `wgpu::PresentMode::AutoVsync` |
| Minimize brush-stroke input latency | `WgpuConfiguration::desired_maximum_frame_latency` | `Some(1)` |
| 8K texture support | `WgpuSetupCreateNew::device_descriptor` closure → `DeviceDescriptor::required_limits` | raise `max_texture_dimension_2d` to ≥ 8192 |

## The code

This goes in `main.rs` where you build `NativeOptions`. eframe 0.34.2 / egui-wgpu 0.34.2 /
wgpu `=29.0.1`.

```rust
use std::sync::Arc;

use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};

/// Minimum 2D texture dimension Pixhaus needs for an 8K canvas. wgpu's
/// downlevel default is 8192, so most desktop GPUs already clear this — but we
/// request it explicitly so a device that can't meet it fails loudly at startup
/// instead of mysteriously refusing an 8192-wide texture later.
const MIN_TEXTURE_DIM: u32 = 8192;

fn wgpu_config() -> WgpuConfiguration {
    // CreateNew lets egui-wgpu build the instance/adapter/device. We tweak the
    // power preference and the device descriptor closure; the display handle is
    // injected by eframe at instance-creation time, so leave it default here.
    let mut setup = WgpuSetupCreateNew::default();

    // 1. Prefer the discrete / high-performance adapter. On a laptop with an
    //    iGPU + dGPU this steers wgpu to the dGPU. This is the soft preference;
    //    see native_adapter_selector below for a hard pin.
    setup.power_preference = wgpu::PowerPreference::HighPerformance;

    // 2. Request higher limits. The closure receives the chosen adapter, so we
    //    start from the adapter's own limits (never request more than it offers
    //    across the board) and only raise the one dimension we care about. This
    //    is the documented place to ask for an 8K-capable device.
    setup.device_descriptor = Arc::new(|adapter: &wgpu::Adapter| {
        let adapter_limits = adapter.limits();

        // Start from a sane baseline, then lift the texture dimension. Using the
        // adapter's max as the ceiling avoids over-requesting on weaker GPUs.
        let mut limits = wgpu::Limits::default();
        limits.max_texture_dimension_2d =
            adapter_limits.max_texture_dimension_2d.max(MIN_TEXTURE_DIM);

        wgpu::DeviceDescriptor {
            label: Some("pixhaus-device"),
            required_features: wgpu::Features::empty(),
            required_limits: limits,
            // Hint the allocator we'll keep large textures around.
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }
    });

    WgpuConfiguration {
        // 3. Vsync on, no tearing, idle-friendly. egui repaints on demand so
        //    vsync rarely adds latency that matters; combined with frame
        //    latency = 1 below, this is the low-latency-and-no-tearing combo.
        present_mode: wgpu::PresentMode::AutoVsync,

        // 4. The brush-latency knob. One frame of queued work instead of the
        //    driver default of 2 — the painted pixel reaches the screen a frame
        //    sooner. This is the single biggest win for stroke feel.
        desired_maximum_frame_latency: Some(1),

        wgpu_setup: WgpuSetup::CreateNew(setup),

        // Recreate the surface on a non-Success acquire (resize/minimize/
        // device-lost races) — the right desktop default.
        ..WgpuConfiguration::default()
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: wgpu_config(),
        viewport: egui::ViewportBuilder::default().with_title("Pixhaus"),
        ..Default::default()
    };

    eframe::run_native(
        "pixhaus",
        options,
        Box::new(|cc| Ok(Box::new(PixhausApp::new(cc)))),
    )
}
```

## Hard-pinning the discrete GPU (optional, recommended for a pro tool)

`power_preference = HighPerformance` is a *hint* — a driver can still hand back the iGPU.
If you want certainty, override selection entirely with `native_adapter_selector`. It
receives every adapter plus the optional surface and returns the one you choose (or an
error string that surfaces as `WgpuError::CustomNativeAdapterSelectionError`):

```rust
use eframe::egui_wgpu::NativeAdapterSelectorMethod;

setup.native_adapter_selector = Some(Arc::new(
    |adapters: &[wgpu::Adapter], surface: Option<&wgpu::Surface<'_>>|
        -> Result<wgpu::Adapter, String> {
        // Prefer a DiscreteGpu that can present to our surface.
        let pick = adapters
            .iter()
            .filter(|a| surface.is_none_or(|s| a.is_surface_supported(s)))
            .find(|a| a.get_info().device_type == wgpu::DeviceType::DiscreteGpu)
            // Fall back to whatever HighPerformance would have chosen.
            .or_else(|| adapters.iter().next())
            .ok_or_else(|| "no compatible GPU adapter found".to_owned())?;

        Ok(pick.clone())
    },
) as NativeAdapterSelectorMethod);
```

When `native_adapter_selector` is `Some`, it wins over `power_preference` on native
(the preference only matters when the selector is `None`). Keep both set: the selector
for native certainty, the preference as the fallback path.

## Confirm what actually got picked

Adapter selection is the first thing to check when a user reports a render bug. Log the
chosen adapter once at boot with the crate's helper — grab the adapter from `RenderState`
in your `App::new`:

```rust
// in PixhausApp::new(cc), with the wgpu render state available:
if let Some(rs) = cc.wgpu_render_state.as_ref() {
    let info = rs.adapter.get_info();
    log::info!("gpu: {}", eframe::egui_wgpu::adapter_info_summary(&info));
}
```

## Notes and caveats

- **8K is usually already available.** wgpu's downlevel-default `max_texture_dimension_2d`
  is 8192, so on most desktop GPUs you don't strictly need to raise the limit for a single
  8192×8192 texture. Requesting it explicitly turns a silent "texture too large" failure
  later into a loud `WgpuError::RequestDeviceError` at startup, which is the behavior you
  want. If you ever need a canvas larger than 8192 on a side you'll have to tile it —
  `max_texture_dimension_2d` tops out at 16384 on most hardware and isn't unbounded.
- **`AutoVsync` vs `AutoNoVsync`.** Stick with `AutoVsync` for the editor. `AutoNoVsync`
  only makes sense when you're measuring raw throughput, and it costs you tearing and power.
  The latency you care about for brush feel is governed by `desired_maximum_frame_latency`,
  not by turning vsync off.
- **The two latency knobs work together.** `present_mode` controls tearing/queueing policy;
  `desired_maximum_frame_latency = Some(1)` shortens the queue depth. Both at once gives
  no-tearing plus minimal lag.
- **`device_descriptor` is a closure, called with the chosen adapter.** Branch on
  `adapter.limits()` / `adapter.get_info()` inside it — don't hard-code limits you haven't
  checked the adapter can meet, or device creation fails with `RequestDeviceError`.
- **`required_features`** stays `empty()` here — Pixhaus's compositing doesn't need an
  optional feature yet. Add to it from inside the same closure if a future verb needs one
  (e.g. a compute-only feature), gating on `adapter.features().contains(...)`.
- **Env-var escape hatch.** `WgpuSetup::CreateNew` honors `WGPU_POWER_PREF`, `WGPU_BACKEND`,
  `WGPU_VALIDATION`, etc., so you can override adapter/backend selection at runtime for
  debugging without recompiling.
- **Errors.** In the `render` crate wrap `WgpuError` in your `thiserror` enum with
  `#[from]`; in the binary `anyhow` swallows it. The two variants you'll actually hit are
  `RequestAdapterError` (no compatible GPU) and `RequestDeviceError` (limits the adapter
  can't meet — e.g. you asked for more than `max_texture_dimension_2d` supports).
- **Field-value caveat from the reference.** docs.rs didn't render the exact `Default`
  values for every `WgpuConfiguration` field; the eframe defaults (auto-vsync,
  recreate-surface-on-error) are sensible, but if a specific default is load-bearing for
  you, confirm against the egui-wgpu 0.34.2 source.
```
