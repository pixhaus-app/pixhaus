---
name: pixhaus-egui-wgpu
description: >
  Use when working with the egui-wgpu crate directly in the Pixhaus native shell — the
  integration layer between egui and raw wgpu. Covers the Renderer lifecycle
  (update_texture / update_buffers / render and the panic if you skip update_buffers),
  the custom-rendering callback system (CallbackTrait's prepare / finish_prepare / paint
  phases, Callback::new_paint_callback, the CallbackResources type-map), grabbing and
  using RenderState (device, queue, target_format, the Arc<RwLock<Renderer>>),
  ScreenDescriptor, registering a wgpu texture as an egui image
  (register_native_texture), PaintCallbackInfo / ViewportInPixels for viewport+scissor
  math, WgpuConfiguration and the WgpuSetup family, present mode, surface-error handling,
  the winit Painter (what eframe drives under the hood), screenshot capture, and the
  WgpuError type. Trigger this for ANY question about "the egui-wgpu API", "the paint
  callback", "CallbackTrait", "how do I get the wgpu device/queue/format inside the
  callback", "render the canvas in a wgpu pass", "register my texture with egui",
  "viewport in pixels", "set up wgpu for egui", "present mode / vsync", or "screenshot
  the egui frame", even when the user doesn't name egui-wgpu. egui-wgpu 0.34.2 has exact
  signatures and a strict call-order contract that older examples and memory get wrong —
  reach for this rather than guessing. For the applied Pixhaus canvas pattern and overlay
  shapes, pair with pixhaus-egui; for window/app boot, pair with pixhaus-eframe.
---

# egui-wgpu for Pixhaus

egui-wgpu is the integration crate that lets egui paint with wgpu, and — the part that
matters for Pixhaus — lets you splice your own wgpu render pass into the egui frame. It is
the seam the entire native rewrite turns on: the composited pixel canvas stays on the GPU
and never crosses a CPU copy per painted pixel. This skill is the authoritative API
reference for the crate at 0.34.2 — every type, the exact signatures, and the call-order
contracts that older examples get wrong.

The line with the sibling skills, so you reach for the right one:

- **`pixhaus-eframe`** owns the window and the loop, and hands you a `RenderState`. Start
  there for "how does the app boot" and "where do I get the wgpu device."
- **`pixhaus-egui`** is the immediate-mode UI you draw inside the frame. Its
  `references/custom-wgpu-canvas.md` is the *applied* Pixhaus canvas pattern — how the
  viewport, input routing, dirty-tile upload, and overlay shapes fit together.
- **This skill** is the egui-wgpu *crate* itself: the `Renderer` lifecycle, the full
  `CallbackTrait` surface, `RenderState`, setup/config, the winit `Painter`, capture, and
  errors. When you need an exact signature or the contract behind a call, it lives here.

By design they overlap on the callback — `pixhaus-egui` shows the Pixhaus-shaped example,
this skill is the complete API behind it. Don't guess signatures from memory; egui-wgpu's
API moves between releases and 0.34 was a large redesign. The references are derived from
docs.rs 0.34.2.

## Versions — pin in lockstep with the egui family

A mismatched `wgpu` is the most common build break: two `wgpu` versions in the tree are
different types and won't interoperate. This is the same pin as `pixhaus-egui` and
`pixhaus-eframe`; keep all three identical.

| Crate | Version |
|---|---|
| `egui-wgpu` | 0.34.2 |
| `egui` / `eframe` / `egui-winit` / `epaint` | 0.34.2 |
| `wgpu` | `=29.0.1` (pin exactly, not `"29"`) |
| `winit` | 0.30.x |

```toml
# In the render crate (UI-agnostic): you need egui-wgpu for CallbackTrait + the Renderer
# types, but NOT the winit feature — eframe owns winit.
egui-wgpu = "0.34"
wgpu      = "=29.0.1"
```

eframe pulls egui-wgpu in transitively via its `wgpu` feature, so the binary doesn't
declare it twice. When you bump any one crate, bump the whole family and re-verify against
docs.rs — see [[feedback-dep-upgrades]].

Feature flags on egui-wgpu: `winit` (the `Painter` integration — eframe enables it, you
don't), `wayland` / `x11` (Linux display backends, gated behind `winit`), `capture` (the
screenshot module), and two on-by-default fixes (`fragile-send-sync-non-atomic-wasm`,
`macos-window-resize-jitter-fix`). In the `render` crate keep it featureless.

## Three ways the crate gets used — pick the right altitude

1. **You're inside eframe (the Pixhaus default).** eframe builds the `RenderState`,
   drives the `Renderer` every frame, and owns the winit `Painter`. You touch egui-wgpu
   only to (a) read `RenderState` for the `device`/`queue`/`target_format`, (b) implement
   `CallbackTrait` for the canvas, and (c) maybe `register_native_texture`. This is 95% of
   Pixhaus work. See `references/renderer.md` and `references/callbacks.md`.

2. **You drive the winit `Painter` yourself.** Only if Pixhaus ever leaves eframe. The
   `Painter` is the full surface/render-state manager eframe wraps. Documented in
   `references/winit-painter.md` so you understand what eframe does under the hood — not
   because you should reimplement it.

3. **You build a `RenderState` / `Renderer` by hand** for an offscreen pass, a test, or a
   tool with no window. `RenderState::create` and `Renderer::new` plus the setup/config
   types. See `references/setup-and-config.md`.

## The Renderer lifecycle — a strict call order, or it panics

The `Renderer` (held inside `RenderState` as `Arc<RwLock<Renderer>>`) draws egui's own
shapes. eframe calls these for you; you only call them directly on the hand-rolled path.
The order is a contract:

```
update_texture(...)   // for every texture delta egui produced this frame
update_buffers(...)   // uploads vertex/index/uniform data; returns Vec<CommandBuffer>
render(...)           // issues the draw calls into your RenderPass<'static>
```

`render` **panics if `update_buffers` was not called** for this frame's paint jobs. And
`render` takes `&mut wgpu::RenderPass<'static>` — the `'static` bound is real and shapes
how you build the pass. `update_buffers` returns user command buffers (from any callback
`prepare` phase) that you must submit. Full signatures in `references/renderer.md`.

The lock is `parking_lot::RwLock`, so `state.renderer.write()` returns the guard directly
— no `Result`, no `.unwrap()`. Don't hold the guard across an `.await`.

## The custom rendering callback — the make-or-break feature

To draw the Pixhaus canvas in raw wgpu inside the egui frame, implement `CallbackTrait`,
wrap it with `Callback::new_paint_callback(rect, cb)`, and add the returned
`PaintCallback` to a painter. Three phases, called in this order across all registered
callbacks:

```rust
trait CallbackTrait: Send + Sync {
    // 1. before the egui pass. &mut resources, has the ScreenDescriptor. Upload buffers
    //    and dirty texture sub-rects here. Returned buffers submit before the egui pass.
    fn prepare(&self, device, queue, screen_descriptor, egui_encoder,
               callback_resources: &mut CallbackResources) -> Vec<CommandBuffer> { vec![] }

    // 2. after every prepare(). &mut resources, no ScreenDescriptor. Buffers submit after
    //    all prepare() buffers. Rarely needed.
    fn finish_prepare(&self, device, queue, egui_encoder,
                      callback_resources: &mut CallbackResources) -> Vec<CommandBuffer> { vec![] }

    // 3. REQUIRED. inside the egui pass. resources is shared (&), the pass is 'static and
    //    owned by egui-wgpu — issue draws only; never end it or begin sub-passes.
    fn paint(&self, info: PaintCallbackInfo, render_pass: &mut RenderPass<'static>,
             callback_resources: &CallbackResources);
}
```

The load-bearing rules, each of which prevents a real bug:

- **GPU resources (pipeline, buffers, bind groups, the canvas texture) live in
  `CallbackResources`, built once.** It's a concurrent `type-map` keyed by type:
  `insert::<T>` (needs `T: Send + Sync + 'static`), `get`/`get_mut`/`contains`/`remove`.
  Building a pipeline per frame is a severe stall.
- **`prepare` mutates resources (`&mut`); `paint` only reads them (`&`).** Upload to GPU in
  `prepare`; in `paint` you can only `get`. This split is why the type-map exists.
- **Upload dirty sub-rects in `prepare`, never the whole canvas**, via
  `queue.write_texture` on a sub-region. This is the entire point versus a per-frame
  re-upload — see [[8k-perf-constraint]].
- **The `Callback` struct is per-frame and cheap** — hold only what changes (camera
  transform, visible tile range), not GPU handles.
- **For viewport and scissor math in `paint`, use `info.viewport_in_pixels()` and
  `info.clip_rect_in_pixels()`**, which return `ViewportInPixels { left_px, top_px,
  from_bottom_px, width_px, height_px }` (physical pixels, `from_bottom_px` is the GL-style
  y origin). egui-wgpu already sets the pass viewport/scissor to your rect, so you usually
  don't call `set_viewport` yourself.

Exact signatures, the `CallbackResources`/`TypeMap` API, `PaintCallbackInfo`'s fields, and
the wire-up are in `references/callbacks.md`. The Pixhaus-shaped worked example (input
routing, overlays) is in `pixhaus-egui`'s `references/custom-wgpu-canvas.md`.

## Displaying a wgpu texture as an egui image (the simpler path)

If you'd rather render the canvas to your own `wgpu::Texture` and let egui draw it as an
image, register the view with the renderer and you get a `TextureId` to use in
`painter.image(...)`:

```rust
let id = state.renderer.write().register_native_texture(
    &state.device, &texture_view, wgpu::FilterMode::Nearest); // NEAREST for pixel art
// later, to re-point that id at a new view without reallocating the id:
state.renderer.write().update_egui_texture_from_wgpu_texture(
    &state.device, &new_view, wgpu::FilterMode::Nearest, id);
```

Note the in-place updater is `update_egui_texture_from_wgpu_texture` (plus a
`_with_sampler_options` variant) — there is no `update_native_texture`. Use the callback
path for the live drawing canvas; this path is fine for offscreen previews or a prototype.
See `references/renderer.md`.

## References

Open the file for the area you're in; each is a dense, verbatim 0.34.2 API reference.

| File | Covers |
|---|---|
| `references/renderer.md` | `Renderer` (full lifecycle, native-texture registration, panic contract), `RendererOptions`, `RenderState`, `ScreenDescriptor`, `Texture` |
| `references/callbacks.md` | `CallbackTrait` phases + exact signatures, `Callback::new_paint_callback`, `CallbackResources` / `TypeMap` methods, `PaintCallbackInfo` + `ViewportInPixels`, wire-up |
| `references/setup-and-config.md` | `WgpuConfiguration`, `WgpuSetup` / `WgpuSetupCreateNew` / `WgpuSetupExisting`, `RenderState::create`, `SurfaceErrorAction`, `WgpuError`, `EguiDisplayHandle`, `NativeAdapterSelectorMethod`, the free functions, feature flags |
| `references/winit-painter.md` | `winit::Painter` full lifecycle (what eframe drives) — surface/window setup, resize, `paint_and_update_textures`, screenshots, teardown |
| `references/capture.md` | The `capture` module — `CaptureState`, `capture_channel`, reading the rendered frame back to the CPU |

A standing caution: the references record the 0.34.2 API faithfully, and the high-stakes
signatures (`Renderer::new`, `CallbackTrait`, `RenderState`, `register_native_texture`,
`PaintCallbackInfo`) were confirmed against docs.rs. A few deep ones are marked "verify"
inline where the rendered docs were ambiguous (some `WgpuConfiguration` default values, the
exact `capture_channel` types). When one is load-bearing for what you're building, confirm
against https://docs.rs/egui-wgpu/0.34.2/ or the source before depending on it.
