# Renderer, RenderState, and the lifecycle

egui-wgpu 0.34.2. The types that draw egui's own shapes and that hand you the wgpu device.
In Pixhaus-on-eframe you read `RenderState` and rarely call `Renderer` directly — eframe
drives the lifecycle. You call it directly only on the hand-rolled / offscreen path.

## `RenderState` — your handle on the GPU

```rust
pub struct RenderState {
    pub adapter: wgpu::Adapter,
    pub available_adapters: Vec<wgpu::Adapter>,  // empty on web
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub target_format: wgpu::TextureFormat,      // build your pipeline against THIS
    pub renderer: Arc<RwLock<Renderer>>,         // parking_lot::RwLock
}
```

This is what `eframe::CreationContext::wgpu_render_state` gives you (`Option<RenderState>`)
and what `egui_wgpu::winit::Painter::render_state()` returns. `Clone`, so you can hand a
clone to a background thread.

- **Build every pipeline against `target_format`.** It's the surface's presentation format
  (gamma-space `Bgra8Unorm` or `Rgba8Unorm`). A pipeline built against a guessed format
  fails to draw or mismatches blending.
- **`renderer` is `Arc<RwLock<Renderer>>` from `parking_lot`** — `.read()` / `.write()`
  return the guard directly, no `Result`, no `.unwrap()`. Never hold the guard across
  `.await` (see [[pixhaus-parking-lot]] and [[pixhaus-rust-conventions]] on locks).
- `device` and `queue` are the cheap-to-clone wgpu handles you use everywhere.

Constructing one by hand (offscreen/test path):

```rust
pub async fn create(
    config: &WgpuConfiguration,
    instance: &wgpu::Instance,
    compatible_surface: Option<&wgpu::Surface<'static>>,
    options: RendererOptions,
) -> Result<RenderState, WgpuError>
```

This is `async` — to drive it from a sync test or the `render` crate's harness, block on it
with pollster, not tokio. See [[pixhaus-pollster]]. Config and `WgpuConfiguration` live in
`setup-and-config.md`.

## `Renderer` — draws egui's shapes; you splice in via callbacks

```rust
pub struct Renderer {
    pub callback_resources: CallbackResources,  // shared store for CallbackTrait impls
    /* private fields */
}
```

`callback_resources` is the one public field: the type-map where your canvas pipeline and
buffers live (see `callbacks.md`). Insert into it once at startup:

```rust
let res = CanvasResources::new(&state.device, state.target_format);
state.renderer.write().callback_resources.insert(res);
```

### Construction

```rust
pub fn new(
    device: &wgpu::Device,
    output_color_format: wgpu::TextureFormat,   // prefer gamma-space Rgba8Unorm/Bgra8Unorm
    options: RendererOptions,
) -> Self
```

eframe calls this for you. You only call it on the hand-rolled path. (Note: pre-`RendererOptions`
0.34.x point releases used a different `new` arg list with explicit msaa/depth args — the
0.34.2 form takes a single `RendererOptions`.)

### The lifecycle contract — order is mandatory

```rust
// 1. apply each texture delta egui produced this frame (font atlas, images, etc.)
pub fn update_texture(&mut self, device: &Device, queue: &Queue,
                      id: TextureId, image_delta: &ImageDelta)

// 2. upload vertex/index/uniform data. Returns user command buffers (from callback
//    prepare phases) that YOU must submit. Takes a &mut CommandEncoder.
pub fn update_buffers(&mut self, device: &Device, queue: &Queue,
                      encoder: &mut CommandEncoder,
                      paint_jobs: &[ClippedPrimitive],
                      screen_descriptor: &ScreenDescriptor) -> Vec<CommandBuffer>

// 3. issue the draws into an existing pass. PANICS if update_buffers was not called first.
pub fn render(&self, render_pass: &mut RenderPass<'static>,
              paint_jobs: &[ClippedPrimitive],
              screen_descriptor: &ScreenDescriptor)
```

Three things to get right:

- **`render` panics without a preceding `update_buffers`** for this frame's `paint_jobs`.
  The docs say so explicitly. If you hand-roll the loop, never skip step 2.
- **`render` takes `&mut RenderPass<'static>`.** The `'static` lifetime is real: the pass
  must not borrow frame-local data. This is the same constraint your `CallbackTrait::paint`
  sees.
- **Submit the `Vec<CommandBuffer>` from `update_buffers`** alongside your own encoder's
  buffer — those are the command buffers your callbacks' `prepare` phases returned.

### Registering a wgpu texture as an egui image

For the "render to my own texture, let egui display it" path (simpler than a callback;
good for offscreen previews):

```rust
pub fn register_native_texture(&mut self, device: &Device,
    texture: &TextureView, texture_filter: FilterMode) -> TextureId

pub fn register_native_texture_with_sampler_options(&mut self, device: &Device,
    texture: &TextureView, sampler_descriptor: SamplerDescriptor<'_>) -> TextureId

// re-point an existing TextureId at a new view WITHOUT allocating a new id:
pub fn update_egui_texture_from_wgpu_texture(&mut self, device: &Device,
    texture: &TextureView, texture_filter: FilterMode, id: TextureId)

pub fn update_egui_texture_from_wgpu_texture_with_sampler_options(&mut self,
    device: &Device, texture: &TextureView,
    sampler_descriptor: SamplerDescriptor<'_>, id: TextureId)

pub fn free_texture(&mut self, id: &TextureId)
pub fn texture(&self, id: &TextureId) -> Option<&Texture>
```

- **There is no `update_native_texture`.** The in-place updater is
  `update_egui_texture_from_wgpu_texture`. `register_*` is for first registration only —
  calling it per frame leaks `TextureId`s.
- **For pixel art always pass `wgpu::FilterMode::Nearest`** (or a `SamplerDescriptor` with
  nearest min/mag), so the canvas isn't blurred when zoomed. This is the same NEAREST rule
  as the texture path in `pixhaus-egui`.
- Use the returned `TextureId` in `painter.image(id, rect, uv, tint)`.

## `RendererOptions`

```rust
pub struct RendererOptions {
    pub msaa_samples: u32,                          // power of two; 0 or 1 = off (default)
    pub depth_stencil_format: Option<TextureFormat>,// egui needs none; default None
    pub dithering: bool,                            // default true; assumes sRGB output
    pub predictable_texture_filtering: bool,        // default false; software filtering
}

impl RendererOptions {
    pub const PREDICTABLE: Self;  // preset for image-snapshot tests (deterministic output)
}
// derives: Clone, Copy, Debug, Default
```

Use `RendererOptions::default()` for the app. Use `RendererOptions::PREDICTABLE` in
visual-regression tests so software texture filtering makes snapshots hardware-independent
(pairs with image-compare in [[pixhaus-testing-conventions]]).

## `ScreenDescriptor`

```rust
pub struct ScreenDescriptor {
    pub size_in_pixels: [u32; 2],   // window size in physical pixels
    pub pixels_per_point: f32,      // hi-DPI scale factor
}
```

A plain data struct passed into `update_buffers` and `render`, and handed to your
callback's `prepare`. No public methods in 0.34.2.

## `Texture`

The renderer's internal record for a registered texture, returned by `Renderer::texture`:

```rust
pub struct Texture {
    pub texture: Option<wgpu::Texture>,   // None if the id is a user-provided bind group
    pub bind_group: wgpu::BindGroup,      // texture + sampler bind group
    pub options: Option<TextureOptions>,  // sampler options, if egui owns the sampler
}
```

You rarely touch this; it's exposed for inspecting or reusing the bind group of a texture
egui allocated.
