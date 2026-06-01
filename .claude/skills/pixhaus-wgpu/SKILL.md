---
name: pixhaus-wgpu
description: >
  Use when writing, reviewing, or debugging raw wgpu code in the Pixhaus `render` crate —
  the UI-agnostic GPU viewport renderer. Covers the device/queue/adapter lifecycle, buffers,
  textures and samplers, dirty-region texture uploads, bind groups and layouts, WGSL shader
  modules, render and compute pipelines, command encoders, render/compute passes, draw and
  dispatch calls, blending, and GPU-side image ops. Trigger this for ANY GPU-rendering work
  even when the user says "the renderer", "the GPU code", "the canvas shader", "draw the
  pixels on the GPU", "make the brush upload faster", "write a compute pass to blend layers",
  "set up the render pipeline", or names a wgpu type (Device, Queue, Buffer, Texture,
  BindGroup, RenderPipeline, RenderPass, ComputePipeline) without saying "wgpu". wgpu is an
  explicit GPU API and its 29.x types differ sharply from older examples and your memory
  (TexelCopy* names, PollType, Result-returning request_adapter, Option entry_points), so
  reach for this skill rather than guessing signatures. For the egui-wgpu glue that embeds
  this renderer in the UI (the paint callback, RenderState, CallbackResources), use
  pixhaus-egui-wgpu; this skill is what runs *inside* that callback.
---

# wgpu for Pixhaus

wgpu is the explicit GPU API under the `render` crate — Pixhaus's UI-agnostic viewport
renderer, the perf-critical code and the whole reason for the native rewrite. Pixel data
lives on the GPU and is composited there; nothing crosses a CPU copy per painted pixel.

This skill is the floor for raw wgpu work: the mental model that prevents the recurring
stalls and validation panics, the verified wgpu 29 API for the load-bearing calls, and how
the pieces map onto a pixel-art canvas (the textured-quad render path and the compute path
for image ops). When you need the full surface for an area, open the matching file in
`references/`. Don't guess signatures from memory — wgpu's API moves hard between majors,
and 29 renamed and reshaped a lot.

This crate owns wgpu; the rest of the app talks to it through a clean boundary. Keep it
UI-agnostic: `render` knows nothing about egui. The embedding lives in `shell` via
egui-wgpu (the pixhaus-egui-wgpu skill).

## Versions — pin in lockstep

The renderer must use the **same `wgpu` the egui family pulls in**. egui-wgpu 0.34.2
re-exports `wgpu 29`; two `wgpu` versions in one tree are different, non-interoperating
types and the most common build break. Pin it, don't float it.

| Crate | Version |
|---|---|
| `wgpu` | `=29.0.1` (pin exactly, not `"29"`) |
| `egui-wgpu` | 0.34.2 (re-exports this wgpu) |
| `bytemuck` | `"1"` (POD casting for vertices/uniforms) |
| `pollster` | `"0.4"` (block on async init in tests/benches) |

```toml
wgpu     = "=29.0.1"
bytemuck = { version = "1", features = ["derive"] }
```

The docs.rs "latest" wgpu is a newer major than 29 — its signatures will not match this
crate and will break the egui-wgpu coupling. Always check 29.0.1
(https://docs.rs/wgpu/29.0.1/wgpu/). When you bump wgpu, bump egui/egui-wgpu in the same
move and re-verify against docs.rs — see [[feedback-dep-upgrades]].

## The mental model: explicit, build-once, record-per-frame

wgpu is not a draw-a-thing API. You describe every GPU object up front — pipelines, bind
group layouts, bind groups, buffers, textures — then each frame you only *record commands*
into an encoder and *upload changed data*. Three consequences drive almost every
correct/incorrect decision:

1. **Heavy objects are created once and reused; the hot loop touches neither.** Building a
   `RenderPipeline`, `BindGroupLayout`, or shader module per frame is a severe stall —
   pipeline creation compiles shaders. Create them at startup, store them, and in the frame
   loop only set them on a pass and issue draws. The per-frame cost is recording, plus
   uploading the bytes that actually changed.

2. **Work is bounded by the dirty region, not the canvas.** This is the 8K constraint
   ([[8k-perf-constraint]]) made concrete. A brush stroke uploads only its dirty
   sub-rectangle via `queue.write_texture` (a plain `&[u8]`, so no 256-byte row alignment to
   fight). A compute image op dispatches workgroups only over the affected extent. Never
   re-upload or re-process the whole 8192×8192 buffer because one pixel changed.

3. **Handles are cheap clones; submission is the sync point.** `Device`, `Queue`, `Buffer`,
   `Texture` are ref-counted handles — cloning one does not copy GPU memory. Uploads
   (`write_buffer`/`write_texture`) are queued and become visible at the next
   `queue.submit`; there is nothing to await for an upload. Mapping a buffer is the only
   async path and is used only for readback (export, tests), never in the paint loop.

## Where the device comes from — two contexts

- **Production:** the `render` crate does NOT create an instance, adapter, device, queue, or
  surface. eframe/egui-wgpu builds them and hands you `wgpu::Device`, `wgpu::Queue`, and the
  target `wgpu::TextureFormat` at startup. Build your pipeline's color target against that
  format or blending/format mismatches bite. See pixhaus-eframe (startup) and
  pixhaus-egui-wgpu (RenderState).
- **Headless (tests, benches, examples):** the crate builds its own device. `request_adapter`
  and `request_device` are `async` and return `Result` in v29; there is no tokio in the
  `render` crate, so drive them with `pollster::block_on` (the pixhaus-pollster skill), not
  tokio. Full pattern in `references/device-and-lifecycle.md`.

## The two render paths for the canvas

- **On-screen: a textured quad inside egui's render pass.** A vertex+fragment WGSL pipeline
  samples the canvas texture with a `Nearest` sampler and a camera MVP uniform. The draw
  calls run inside egui-wgpu's `CallbackTrait::paint`, which hands you a
  `&mut RenderPass<'static>` — you only `set_pipeline` / `set_bind_group` /
  `set_vertex_buffer` / `draw`. You do NOT begin or end the pass, finish an encoder, or
  submit. The glue is pixhaus-egui-wgpu; the pipeline and draw calls are
  `references/pipelines-and-shaders.md` and `references/commands-and-passes.md`.
- **Off-screen and image ops: your own encoder.** Layer blends, transforms, filters, and any
  GPU work that produces a texture run on a compute pipeline over a storage texture (or an
  offscreen render pass), recorded on your own `CommandEncoder` and submitted yourself —
  outside `paint`, or in `CallbackTrait::prepare`. `references/commands-and-passes.md`.

## Rules that prevent the recurring bugs

- **Create pipelines, layouts, bind groups, and shader modules once.** Cache them in the
  renderer struct (in production, in the egui-wgpu `CallbackResources` type-map). Per-frame
  creation is the classic wgpu stall.
- **Match the color-target format to what you're rendering into.** In production that is
  egui-wgpu's target format (often `Bgra8UnormSrgb`); offscreen it's your own texture's
  format. A mismatch fails pipeline creation or corrupts blending.
- **Nearest sampling, always, for pixel art.** `mag_filter`, `min_filter`, and
  `mipmap_filter` all `Nearest`. `Linear` blurs texels — the single most common pixel-art
  rendering bug.
- **Upload dirty sub-rects with `queue.write_texture`, not a staging buffer.** `write_texture`
  takes a plain slice and sidesteps the 256-byte `bytes_per_row` rule, so arbitrary dirty
  widths upload directly. The 256 rule only binds buffer-backed copies, i.e. readback.
- **Don't `unwrap()`/`panic!`.** `unsafe` is forbidden workspace-wide (so byte casting goes
  through `bytemuck`, the pixhaus-bytemuck skill). By default wgpu *panics* on uncaptured
  errors — set `device.on_uncaptured_error` and/or wrap fallible recording in a validation
  error scope, mapping failures to a `thiserror` variant (the pixhaus-rust-conventions skill).
- **Drop a pass before finishing its encoder.** The open `RenderPass`/`ComputePass` holds the
  encoder borrow; scope it in a block so it drops before `encoder.finish()`.
- **Keep matrix and uniform math in glam, byte layout uniform-safe.** Build the camera MVP
  and pack uniforms via the pixhaus-glam skill; pad uniform structs to 16-byte alignment.

## v29 API drift — what your memory gets wrong

These changed from older wgpu and are easy to write incorrectly from training data:

- Copy types are `TexelCopyTextureInfo` / `TexelCopyBufferLayout` / `TexelCopyBufferInfo`
  (were `ImageCopyTexture` / `ImageDataLayout` / `ImageCopyBuffer`).
- `request_adapter` / `request_device` return `Result` (were `Option` / infallible) and are
  `impl Future`.
- `Device::poll` takes `PollType` and returns `Result` — `Maintain` is gone.
- Pipeline `entry_point` is `Option<&str>`; every stage needs
  `compilation_options: PipelineCompilationOptions::default()`; descriptors end with
  `cache: None`.
- `PipelineLayoutDescriptor` has `immediate_size: u32` (no `push_constant_ranges`) and
  `bind_group_layouts: &[Option<&BindGroupLayout>]` — write `&[Some(&bgl)]`.
- `on_uncaptured_error` takes `Arc<dyn UncapturedErrorHandler>`.
- Premultiplied blending is `wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING`.

## References

Open the file for the area you're working in; each is a dense, verified wgpu 29.0.1 reference.

| File | Covers |
|---|---|
| `references/device-and-lifecycle.md` | Instance/Adapter/Device/Queue, headless init, limits/features (the 8192 ceiling), error scopes, `on_uncaptured_error`, poll, device lost, surface (tests only) |
| `references/buffers-and-textures.md` | Buffers, `create_buffer_init`, textures, views, samplers (`Nearest`), `write_buffer`/`write_texture`, dirty-rect upload, the 256-byte rule, formats, bytemuck |
| `references/pipelines-and-shaders.md` | Bind group layouts/entries, pipeline layout, WGSL shader modules, render pipeline (vertex/fragment/blend), compute pipeline, the textured-quad and storage-texture examples |
| `references/commands-and-passes.md` | `CommandEncoder`, render/compute passes, the v29 lifetime model, draw/dispatch, copy commands, submission, the offscreen and inside-egui-callback patterns |

The references record the 29.0.1 API faithfully; a few deep signatures flagged "VERIFY" were
not confirmable from the rendered docs (chiefly some `PollType`/`PollStatus`/`SurfaceError`
variants and a couple of descriptor field names). When one is load-bearing for what you're
building, confirm it against https://docs.rs/wgpu/29.0.1/ or the source before depending on
it.
