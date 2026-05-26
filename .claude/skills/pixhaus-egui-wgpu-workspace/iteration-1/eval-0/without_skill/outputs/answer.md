# Embedding the Pixhaus canvas as a wgpu render pass in egui 0.34

The whole point of going native was to keep pixel data on the GPU and never copy it
across a boundary. So the canvas texture is owned by your renderer, lives for the life
of the app, and a brush stroke only touches the dirty rectangle via
`queue.write_texture`. egui's job is purely to schedule a paint callback into the right
screen rectangle; the actual draw is your own `wgpu::RenderPass`.

Here's how the pieces fit on egui-wgpu 0.34.

## Where the GPU resources live

There are two distinct lifetimes, and conflating them is the usual mistake:

1. **Persistent resources** — the render pipeline, the sampler, the bind group layout,
   the bind group, and above all the 8192x8192 canvas texture. These are created once
   and reused every frame. They must NOT be rebuilt per paint. egui-wgpu gives you a
   per-`Renderer` type map for exactly this: `RenderState::renderer`'s
   `callback_resources` (a `TypeMap`). You insert your resource struct once at startup
   and fetch it by type in the callback.

2. **Per-frame data** — the view transform (pan/zoom), the target rect size. This is
   small, changes every frame, and goes in the `Callback` value itself (the thing you
   hand to egui in the panel). It gets written to a uniform buffer in `prepare`.

The canvas texture belongs in category 1. You upload the composited RGBA8 buffer into
it; egui never sees the pixels, only a paint instruction.

### The resource struct

```rust
//! render/src/canvas.rs
//! UI-agnostic. Knows wgpu, knows nothing about egui.

use wgpu::util::DeviceExt;

/// Everything the canvas paint needs that outlives a single frame.
/// Stored in egui's `callback_resources` type map, created once.
pub struct CanvasResources {
    pipeline: wgpu::RenderPipeline,
    /// The composited canvas. RGBA8, up to 8192x8192. Owned here, never copied to egui.
    texture: wgpu::Texture,
    /// Cached so the bind group survives across frames; rebuilt only on resize.
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    texture_size: wgpu::Extent3d,
}

/// Per-frame uniform. Pan/zoom mapping canvas pixels into clip space.
/// `#[repr(C)]` + Pod so it casts to bytes without unsafe (use bytemuck).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CanvasUniform {
    /// Maps the unit quad to the canvas rect inside the viewport, in clip space.
    /// offset.xy + scale.xy. Kept as two vec2s padded to a vec4 pair.
    scale: [f32; 2],
    offset: [f32; 2],
}

impl CanvasResources {
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        canvas_width: u32,
        canvas_height: u32,
    ) -> Self {
        let texture_size = wgpu::Extent3d {
            width: canvas_width,
            height: canvas_height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("pixhaus.canvas.composited"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Rgba8UnormSrgb if your composite is in sRGB; pick one and be consistent.
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("pixhaus.canvas.sampler"),
            // Nearest: this is pixel art. No bilinear smear on zoom-in.
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pixhaus.canvas.uniform"),
            size: std::mem::size_of::<CanvasUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("pixhaus.canvas.bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pixhaus.canvas.bg"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pixhaus.canvas.shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("canvas.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pixhaus.canvas.pl"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pixhaus.canvas.pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[], // fullscreen-style quad generated from vertex_index
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    // egui's pass has already cleared/painted; blend over it.
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            texture,
            bind_group,
            bind_group_layout,
            uniform_buffer,
            texture_size,
        }
    }

    /// Re-upload ONLY the dirty rectangle. This is the perf-critical path:
    /// a brush move uploads a few hundred bytes, not 256 MB.
    ///
    /// `rows` must be tightly packed RGBA8 for `w` pixels per row, `h` rows.
    pub fn upload_dirty(
        &self,
        queue: &wgpu::Queue,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        rows: &[u8],
    ) {
        debug_assert_eq!(rows.len(), (w * h * 4) as usize);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            rows,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                // Per-row stride of the SOURCE slice. For a tight dirty-rect
                // sub-buffer this is the rect width, not the canvas width.
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }
}
```

Two things to note in `upload_dirty`:

- `write_texture`'s `bytes_per_row` describes the **source** slice layout, not the
  texture. If your dirty pixels are extracted into a tight `w*h*4` buffer, the stride is
  `w*4`. There is no 256-byte alignment requirement on `write_texture` (that constraint
  is `copy_buffer_to_texture` only), so you can upload any rectangle directly.
- `origin` is the destination top-left in the big texture. That's how a dirty rect at
  `(x, y)` lands in the right place without touching the rest.

If your composited buffer in `core` is one big `Vec<u8>` and the dirty pixels are NOT
contiguous (they're a sub-rect of a wider image), either copy the rect into a scratch
buffer first, or do one `write_texture` per row. For pixel-art brush sizes the scratch
copy is trivial. The rule from the 8K perf constraint holds: bound the work by the dirty
region, never by the canvas size.

### The shader (`render/src/canvas.wgsl`)

```wgsl
struct CanvasUniform {
    scale: vec2<f32>,
    offset: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: CanvasUniform;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    // Two-triangle quad as a unit square in [0,1].
    var quad = array<vec2<f32>, 4>(
        vec2(0.0, 0.0), vec2(1.0, 0.0),
        vec2(0.0, 1.0), vec2(1.0, 1.0),
    );
    var idx = array<u32, 6>(0u, 1u, 2u, 2u, 1u, 3u);
    let p = quad[idx[vi]];

    var out: VsOut;
    // Map unit quad into clip space using the viewport-fit transform.
    let pos = p * u.scale + u.offset;
    out.clip = vec4(pos, 0.0, 1.0);
    out.uv = vec2(p.x, p.y); // v already top-down to match texture rows
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, in.uv);
}
```

Draw it with `draw(0..6, 0..1)` — no vertex buffer, vertices come from `vertex_index`.

## The CallbackTrait implementation

On egui-wgpu 0.34 the trait is `egui_wgpu::CallbackTrait`. You implement `prepare`
(writes per-frame GPU state, runs before the render pass) and `paint` (records draw
commands into egui's render pass). Both fetch persistent resources out of
`callback_resources` by type.

```rust
//! shell/src/canvas_callback.rs
use egui_wgpu::CallbackTrait;
use crate::canvas::CanvasResources; // from the render crate

/// Per-frame payload. Small, recreated every frame in the panel.
/// Carries the view transform; the heavy texture lives in callback_resources.
pub struct CanvasCallback {
    pub scale: [f32; 2],
    pub offset: [f32; 2],
}

impl CallbackTrait for CanvasCallback {
    /// Runs before egui's render pass begins. Use it to upload uniforms.
    /// Return value is a list of command buffers to submit before paint; we
    /// don't need extra encoders here, so return empty.
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let res: &CanvasResources = resources
            .get()
            .expect("CanvasResources inserted at startup");

        let uniform = CanvasUniform {
            scale: self.scale,
            offset: self.offset,
        };
        queue.write_buffer(res.uniform_buffer(), 0, bytemuck::bytes_of(&uniform));

        Vec::new()
    }

    /// Runs INSIDE egui's render pass. You only record draw calls; you do not
    /// own the pass and must not begin/end one. Note the `'static` lifetime on
    /// the pass in 0.34 — bind groups/pipelines you reference must outlive it,
    /// which is exactly why they live in callback_resources, not in `self`.
    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let res: &CanvasResources = resources
            .get()
            .expect("CanvasResources inserted at startup");

        render_pass.set_pipeline(res.pipeline());
        render_pass.set_bind_group(0, res.bind_group(), &[]);
        render_pass.draw(0..6, 0..1);
    }
}
```

You'll need small accessors on `CanvasResources` (`pub fn pipeline(&self) -> &wgpu::RenderPipeline`,
etc.) since the fields are private — or make the fields `pub(crate)` if the callback
lives in the same crate. Keep the `CanvasUniform` definition shared between the two
files (move it to the render crate and re-export, or define it once and `use` it).

The `'static` on `RenderPass` in the `paint` signature is the thing that bites people
coming from older egui. It means you cannot capture a borrowed pipeline in the callback
struct and reference it inside the pass — the borrow checker won't allow a non-`'static`
reference to escape into a `'static` pass. The fix is structural: persistent GPU objects
go in `callback_resources` and are fetched by reference *inside* `paint`, where their
lifetime is tied to `resources`, not to `self`.

## Inserting the resources once at startup

Grab the `RenderState` from eframe at app construction and insert your struct into the
renderer's type map. The `CallbackResources` map is `renderer.callback_resources`.

```rust
//! shell/src/app.rs (inside your App::new / creation context)
fn install_canvas_resources(cc: &eframe::CreationContext<'_>) {
    let render_state = cc
        .wgpu_render_state
        .as_ref()
        .expect("app must run on the wgpu backend");

    let resources = CanvasResources::new(
        &render_state.device,
        render_state.target_format,
        8192,
        8192,
    );

    // One write lock at startup. Lives for the app's lifetime.
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(resources);
}
```

`render_state.renderer` is an `Arc<RwLock<Renderer>>`. You take the write lock exactly
once here. During frames, egui takes it internally; you never touch it again from the
update loop. The texture, pipeline, and bind group now persist across every frame for
free.

## Pushing the callback from the central panel

In the egui update loop, you allocate a response rect for the viewport, compute the
view transform from pan/zoom against that rect, build the per-frame `CanvasCallback`,
and hand it to egui via `egui_wgpu::Callback::new_paint_callback`.

```rust
//! shell/src/app.rs (inside the App::update / panel code)
egui::CentralPanel::default().show(ctx, |ui| {
    // Take the whole panel for the viewport.
    let (rect, response) =
        ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

    // --- compute the view transform here ---
    // Map the canvas (in its own pixel space, under pan/zoom) into clip space
    // for the egui paint rect. egui clip space is NDC over the target; the
    // viewport rect is given to the GPU via `info.viewport` automatically, so
    // you map the canvas quad into [-1,1] of *that rect*.
    //
    // For a canvas of size (cw, ch) drawn at zoom z, panned by (px, py),
    // inside a paint rect of size (rw, rh):
    let z = self.zoom;
    let (cw, ch) = (8192.0_f32, 8192.0_f32);
    let (rw, rh) = (rect.width(), rect.height());

    // size of the canvas in the rect, as a fraction of the rect, then *2 for NDC span
    let sx = (cw * z / rw) * 2.0;
    let sy = (ch * z / rh) * 2.0;
    // top-left placement -> NDC offset (y flipped because NDC y is up)
    let ox = (self.pan.x / rw) * 2.0 - 1.0;
    let oy = 1.0 - (self.pan.y / rh) * 2.0;

    let callback = CanvasCallback {
        scale: [sx, -sy], // negative y so the quad grows downward in screen space
        offset: [ox, oy],
    };

    // Queues the callback; prepare()/paint() fire during egui's render pass,
    // and egui restricts the GPU viewport to `rect` for us.
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        rect,
        callback,
    ));

    // Brush input -> dirty-rect upload. This is the hot path on every drag move.
    if response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            // 1. translate `pos` (screen) into canvas pixel coords via the inverse
            //    of the transform above (omitted: your screen->canvas helper).
            // 2. let core stamp the brush, returning the changed pixels + rect.
            let dirty = self.document.stamp_brush(/* canvas coords, tool */);

            // 3. upload ONLY that rect straight to the GPU texture.
            let render_state = frame
                .wgpu_render_state()
                .expect("wgpu backend");
            let renderer = render_state.renderer.read();
            let res: &CanvasResources = renderer
                .callback_resources
                .get()
                .expect("canvas resources");
            res.upload_dirty(
                &render_state.queue,
                dirty.x, dirty.y, dirty.w, dirty.h,
                &dirty.rgba,
            );
            // request another frame so the change shows immediately
            ctx.request_repaint();
        }
    }
});
```

A couple of clarifications on that panel code:

- `egui_wgpu::Callback::new_paint_callback(rect, callback)` returns an
  `egui::PaintCallback`-bearing `Shape`; you `add` it to the painter. egui clips the GPU
  viewport to `rect` for you (`PaintCallbackInfo::viewport` reflects it), which is why
  the shader only needs to map into the rect's NDC, not the whole window.
- The brush upload reads the same `callback_resources` map, this time under a `read()`
  lock, and calls `upload_dirty`. It does NOT go through `prepare`/`paint` — uploading is
  a plain queue operation you can do any time before the next submit. Doing it right at
  input time keeps the dirty data path short.
- `frame.wgpu_render_state()` (on `&mut eframe::Frame`) is the in-loop accessor for the
  same `RenderState` you used at startup. If you prefer, stash an
  `Arc<RwLock<Renderer>>` clone in your app struct at startup and skip the `frame`
  argument entirely.

## The shape of it

- **render crate** owns `CanvasResources` and the WGSL. It knows wgpu and nothing about
  egui — this is the UI-agnostic boundary the project wants. The only egui-shaped thing
  is that `CallbackTrait` impl, which is why it lives in **shell**, not render.
- **shell crate** owns the `CanvasCallback` (`CallbackTrait` impl), inserts the resources
  at startup, computes the transform each frame, pushes the callback, and routes brush
  input to `upload_dirty`.
- **The 8192x8192 texture is allocated once** and never reallocated for a stroke. Brush
  strokes are `queue.write_texture` of the dirty sub-rect only. That is the entire reason
  this architecture beats the old IPC-bound webview: the pixels are born on the GPU and
  the only per-stroke traffic is the rectangle that changed.

### Version notes for egui-wgpu 0.34

- The trait is `egui_wgpu::CallbackTrait` with `prepare(&self, device, queue,
  screen_descriptor, encoder, resources) -> Vec<CommandBuffer>` and `paint(&self, info,
  render_pass: &mut RenderPass<'static>, resources)`. The `'static` render-pass lifetime
  is current and is what forces the resources-in-typemap pattern.
- `RenderState` exposes `device`, `queue`, `target_format`, and
  `renderer: Arc<RwLock<Renderer>>`; `Renderer::callback_resources` is the `TypeMap`.
- wgpu's copy descriptor types here are `TexelCopyTextureInfo` / `TexelCopyBufferLayout`
  (the names that replaced the older `ImageCopyTexture` / `ImageDataLayout`). If your
  pinned wgpu still uses the old names, swap them — the fields are identical.
- Use `bytemuck` for the uniform byte cast; the workspace forbids `unsafe`, so the derive
  is the only sanctioned way to get `&[u8]` out of the uniform struct.
