# Wiring the Pixhaus canvas viewport (egui-wgpu 0.34)

You want the composited RGBA texture drawn in a raw wgpu pass spliced into the egui
frame, and you want a brush stroke to re-upload only its dirty rectangle — not the whole
8192x8192 canvas. That is exactly what the `CallbackTrait` path is for, and the dirty-rect
upload is the entire reason to use it over `register_native_texture`.

The split that makes it work:

- The heavy GPU objects (pipeline, bind group, sampler, uniform buffer, and the canvas
  `wgpu::Texture` itself) live in `egui_wgpu::CallbackResources` — built once at startup,
  not per frame. Rebuilding a pipeline per frame is a severe stall.
- The per-frame `Callback` struct holds only what changes — the camera transform and an
  optional dirty rect to flush. It carries no GPU handles.
- Uploads (uniform write, dirty-rect `write_texture`) happen in `prepare`, which gets
  `&mut CallbackResources`. `paint` only reads (`&CallbackResources`) and issues draws.

## 1. Where the GPU resources live — `CanvasResources`

This struct is inserted once into the renderer's `callback_resources` type-map and persists
for the life of the app. Build the pipeline against `RenderState::target_format` or blending
and format will mismatch.

```rust
use wgpu::util::DeviceExt as _;

/// All persistent GPU state for the canvas. Lives in `CallbackResources`, built once.
pub struct CanvasResources {
    pipeline: wgpu::RenderPipeline,
    uniform_buf: wgpu::Buffer,   // the MVP / camera transform
    bind_group: wgpu::BindGroup, // uniform + canvas texture view + sampler
    canvas_texture: wgpu::Texture, // the composited 8192x8192 RGBA8 pixels, kept on the GPU
    canvas_size: (u32, u32),
}

impl CanvasResources {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let canvas_size = (8192, 8192);

        // The composited canvas. Rgba8UnormSrgb so the shader samples linear and the
        // surface presents gamma-correct. COPY_DST lets queue.write_texture target it.
        let canvas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("pixhaus.canvas"),
            size: wgpu::Extent3d {
                width: canvas_size.0,
                height: canvas_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let canvas_view = canvas_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // NEAREST in both directions — pixel art must not blur when zoomed.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("pixhaus.canvas.sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // 4x4 MVP matrix: 64 bytes.
        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pixhaus.canvas.uniform"),
            size: std::mem::size_of::<[[f32; 4]; 4]>() as u64,
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
                    resource: uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&canvas_view),
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

        // A full-screen-quad shader generates 6 vertices from vertex_index, so there is no
        // vertex buffer to bind. The fragment samples canvas_texture at the quad's UVs.
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pixhaus.canvas.pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    // BUILD AGAINST THE SURFACE FORMAT egui hands you, not a guessed one.
                    format: target_format,
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

        Self { pipeline, uniform_buf, bind_group, canvas_texture, canvas_size }
    }
}
```

Insert it once at startup, in `eframe`'s `CreationContext`, where you have the
`RenderState` and thus `target_format`:

```rust
impl Pixhaus {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let wgpu = cc
            .wgpu_render_state
            .as_ref()
            .expect("run_native must select Renderer::Wgpu");

        let res = CanvasResources::new(&wgpu.device, wgpu.target_format);
        // callback_resources is the one public field on Renderer; renderer is a
        // parking_lot RwLock, so write() returns the guard directly — no Result, no unwrap.
        wgpu.renderer.write().callback_resources.insert(res);

        Self { /* app state, camera, document, dirty tracking … */ }
    }
}
```

`RenderState` for reference (from `wgpu_render_state`):

```rust
pub struct RenderState {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub target_format: wgpu::TextureFormat, // build the pipeline against THIS
    pub renderer: Arc<RwLock<egui_wgpu::Renderer>>, // parking_lot RwLock
    // ...adapter, available_adapters
}
```

## 2. The `CallbackTrait` implementation

The per-frame struct carries the camera transform and an optional dirty rect captured from
the brush stroke this frame. `prepare` flushes both to the GPU; `paint` draws the quad.

```rust
use egui_wgpu::{CallbackResources, CallbackTrait, ScreenDescriptor};
use egui::epaint::PaintCallbackInfo;
use wgpu::{
    CommandBuffer, CommandEncoder, Device, Extent3d, Origin3d, Queue, RenderPass,
    TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect,
};

/// One dirty sub-rectangle to flush this frame: tight RGBA8, `w * h * 4` bytes,
/// rows top-to-bottom. Produced by the brush/compositor for the touched region only.
pub struct DirtyRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub pixels: Vec<u8>,
}

/// Per-frame, cheap. No GPU handles — only what changes.
pub struct CanvasCallback {
    /// Camera: canvas pixels -> clip space (scroll + zoom). Built each frame from the rect.
    pub mvp: [[f32; 4]; 4],
    /// `Some` only on frames where the user painted; `None` is the common case.
    pub dirty: Option<DirtyRect>,
}

impl CallbackTrait for CanvasCallback {
    fn prepare(
        &self,
        _device: &Device,
        queue: &Queue,
        _screen: &ScreenDescriptor,
        _egui_encoder: &mut CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<CommandBuffer> {
        let res = resources
            .get_mut::<CanvasResources>()
            .expect("CanvasResources inserted at startup");

        // 1. Camera transform — tiny, write every frame.
        queue.write_buffer(&res.uniform_buf, 0, bytemuck::cast_slice(&[self.mvp]));

        // 2. THE 8K PERF PATH: upload only the dirty sub-rectangle, never the whole canvas.
        //    Work is bounded by the brush footprint, not by 8192x8192.
        if let Some(d) = &self.dirty {
            debug_assert_eq!(d.pixels.len(), (d.w * d.h * 4) as usize);
            queue.write_texture(
                TexelCopyTextureInfo {
                    texture: &res.canvas_texture,
                    mip_level: 0,
                    origin: Origin3d { x: d.x, y: d.y, z: 0 },
                    aspect: TextureAspect::All,
                },
                &d.pixels, // &[u8] — write_texture stages it, so the 256-byte row rule does NOT apply
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(d.w * 4), // tight stride; any width works here
                    rows_per_image: Some(d.h),    // required when height > 1
                },
                Extent3d { width: d.w, height: d.h, depth_or_array_layers: 1 },
            );
        }

        // We submit nothing ourselves — queue.write_* are queued on the shared queue and
        // run before the egui pass. Return an empty Vec.
        Vec::new()
    }

    fn paint(
        &self,
        _info: PaintCallbackInfo,
        render_pass: &mut RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        let res = resources
            .get::<CanvasResources>()
            .expect("CanvasResources inserted at startup");

        // egui-wgpu already set the viewport and scissor to the callback rect, so we don't
        // touch set_viewport / set_scissor_rect. Just draw the full-screen quad.
        render_pass.set_pipeline(&res.pipeline);
        render_pass.set_bind_group(0, &res.bind_group, &[]);
        render_pass.draw(0..6, 0..1); // 2 triangles, vertices generated in the shader
    }
}
```

Notes that prevent real bugs here:

- **Dirty upload goes in `prepare`, not `paint`.** `paint` gets `&CallbackResources`
  (shared) and a `RenderPass<'static>` it must not mutate, end, or sub-pass. Only `prepare`
  has `&mut` and can touch the queue.
- **`write_texture` takes a plain `&[u8]`**, so wgpu stages it and the 256-byte
  `bytes_per_row` alignment rule does **not** bind you — a 4097-px-wide dirty region uploads
  with `bytes_per_row = Some(4097 * 4)` directly, no padding math. (That rule only applies
  to buffer-to-texture copies.)
- **`rows_per_image` is required when `h > 1`.** Leaving it `None` is the classic "only the
  first row uploaded" bug.
- **Returning `Vec::new()` is correct here.** You only return command buffers from `prepare`
  if you record into your *own* encoder. `queue.write_buffer` / `queue.write_texture` queue
  work directly on the shared queue, which runs before the egui pass — no encoder needed.

The WGSL is a standard full-screen quad that maps to canvas UVs (sketch for completeness):

```wgsl
@group(0) @binding(0) var<uniform> mvp: mat4x4<f32>;
@group(0) @binding(1) var canvas_tex: texture_2d<f32>;
@group(0) @binding(2) var canvas_smp: sampler;

struct VsOut { @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    // Two triangles covering the canvas quad in canvas-local [0,1] space.
    var corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
    );
    let c = corners[vi];
    var out: VsOut;
    out.clip = mvp * vec4(c, 0.0, 1.0);
    out.uv = c;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(canvas_tex, canvas_smp, in.uv);
}
```

## 3. Pushing the callback from the central panel

Allocate the viewport rect with a `Sense` so you capture brush input on the same response,
build this frame's dirty rect from the stroke, then add the callback to the painter.
Overlays (brush cursor, marching ants) are egui shapes drawn over the same rect — they
composite on top of your wgpu pass in the same frame.

```rust
impl eframe::App for Pixhaus {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.canvas(ui);
        });
    }
}

impl Pixhaus {
    fn canvas(&mut self, ui: &mut egui::Ui) {
        // Allocate the viewport rect and capture pointer input in one call.
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let rect = response.rect;

        // Route the brush. paint_at returns the touched sub-rect as fresh RGBA8 so we only
        // re-upload that region. None on frames with no stroke — the common case.
        let dirty: Option<DirtyRect> = response
            .interact_pointer_pos()
            .filter(|_| response.dragged() || response.clicked())
            .and_then(|pos| {
                let canvas_pos = self.camera.screen_to_canvas(pos, rect);
                self.paint_at(canvas_pos) // returns Option<DirtyRect> for the stamped region
            });

        // Push the GPU callback for this rect. egui-wgpu sets the viewport/scissor to `rect`.
        painter.add(egui_wgpu::Callback::new_paint_callback(
            rect,
            CanvasCallback { mvp: self.camera.mvp(rect), dirty },
        ));

        // Overlays composite on top in the same frame.
        if let Some(cursor) = response.hover_pos() {
            painter.circle_stroke(
                cursor,
                self.brush_radius,
                egui::Stroke::new(1.0, egui::Color32::WHITE),
            );
        }
    }
}
```

`new_paint_callback(rect, cb)` returns an `epaint::PaintCallback`; `painter.add(...)`
queues it as a shape. The rect you pass becomes `PaintCallbackInfo::viewport` downstream and
is what egui-wgpu uses to set the pass viewport/scissor before calling `paint`.

## Why the callback path and not `register_native_texture`

`register_native_texture` + `painter.image(...)` would also display the texture, and it's
fine for an offscreen preview. But it re-samples through egui's own shader and, more to the
point, gives you no clean place to do a bounded dirty upload tied to the frame's draw — you'd
be uploading and registering outside the render flow. The callback path is the one that
satisfies your hard requirement: in `prepare` you `write_texture` exactly the dirty rect, so
per-stroke GPU work scales with the brush footprint, not the 8192x8192 canvas. That bounded
upload is the entire reason this path exists (see the 8K perf constraint).

## Call-order and lifetime contract (so it doesn't panic)

You don't call the `Renderer` lifecycle yourself — eframe drives
`update_texture` → `update_buffers` → `render` for you, and `render` panics if
`update_buffers` was skipped. Your only obligations are the three phases of `CallbackTrait`,
which egui-wgpu calls in this order across all registered callbacks:

```
prepare (all)  ->  finish_prepare (all)  ->  egui render pass begins  ->  paint (all)
```

`finish_prepare` you can ignore (the default no-op). The `RenderPass<'static>` in `paint` is
real — the pass is owned by egui-wgpu and must outlive your draws, so never store frame-local
borrows in it, never call `end`, and never begin a sub-pass.
