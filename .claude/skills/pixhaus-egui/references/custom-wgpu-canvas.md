# The custom wgpu canvas

egui 0.34.2 / egui-wgpu 0.34.2 / wgpu 29. How to render the pixel canvas in a raw `wgpu`
pass embedded inside the egui frame. This is the make-or-break feature and the reason for
the native rewrite: the composited canvas stays on the GPU and never crosses an IPC or
CPU→GPU copy per frame.

## The shape of it

```
App::ui  →  CentralPanel  →  allocate the viewport rect + Response (for input)
         →  push egui_wgpu::Callback::new_paint_callback(rect, MyCanvasCallback { … })
egui-wgpu, during rendering:
         →  calls MyCanvasCallback::prepare(...)  (before the egui render pass; write buffers)
         →  calls MyCanvasCallback::paint(...)    (inside the egui pass; issue draw calls)
GPU resources (pipeline, buffers, bind groups, the canvas texture) live in
egui_wgpu::CallbackResources, created once at startup.
```

The per-frame `Callback` is a tiny struct holding only what changes (camera transform,
visible tile range). The heavy GPU objects live in `CallbackResources` and persist.

## `CallbackTrait` (verified signatures)

```rust
use egui_wgpu::{CallbackTrait, CallbackResources, ScreenDescriptor};

pub trait CallbackTrait: Send + Sync {
    // Before the egui render pass. Upload/update buffers. Returned command buffers are
    // submitted before the egui pass. Default: no-op.
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &ScreenDescriptor,
        egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> { Vec::new() }

    // After all prepare() calls. Returned buffers submit after prepare()'s. Default: no-op.
    fn finish_prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> { Vec::new() }

    // REQUIRED. Inside the egui render pass. Note the 'static lifetime — egui-wgpu owns
    // the pass; do NOT end it or begin sub-passes. resources is shared (&), not &mut.
    fn paint(
        &self,
        info: egui_wgpu::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &CallbackResources,
    );
}
```

`ScreenDescriptor { size_in_pixels: [u32; 2], pixels_per_point: f32 }`.

## `CallbackResources`

A type-map (`TypeMap`) of GPU state, keyed by type. Store one struct of canvas resources.

```rust
struct CanvasResources {
    pipeline: wgpu::RenderPipeline,
    vertex_buf: wgpu::Buffer,
    index_buf: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    canvas_texture: wgpu::Texture,      // the composited pixels, kept on the GPU
}

// standard TypeMap methods:
resources.insert(value);                 // -> Option<T>
resources.get::<CanvasResources>();      // -> Option<&T>
resources.get_mut::<CanvasResources>();  // -> Option<&mut T>
resources.contains::<CanvasResources>();
```

## Initialize resources once, at startup (eframe)

Grab the wgpu render state from the `CreationContext` and insert your resources into the
renderer's callback resources before the first frame.

```rust
impl Pixhaus {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let wgpu = cc.wgpu_render_state.as_ref()
            .expect("run_native must set Renderer::Wgpu");
        let res = CanvasResources::new(&wgpu.device, wgpu.target_format);
        wgpu.renderer.write().callback_resources.insert(res);
        Self { /* … */ }
    }
}
```

`cc.wgpu_render_state` is `Option<egui_wgpu::RenderState>`:

```rust
pub struct RenderState {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub target_format: wgpu::TextureFormat,    // build your pipeline against this format
    pub renderer: Arc<RwLock<egui_wgpu::Renderer>>,
}
```

Alternatively, lazy-init inside `prepare`:
`if !resources.contains::<CanvasResources>() { resources.insert(CanvasResources::new(device, /*format*/)); }`
— but the pipeline needs the target format, so startup init is cleaner.

## The per-frame callback

```rust
struct CanvasCallback {
    mvp: [[f32; 4]; 4],          // camera: scroll/zoom → clip space
    visible_tiles: (u32, u32),   // index range to draw
}

impl CallbackTrait for CanvasCallback {
    fn prepare(&self, _d: &wgpu::Device, queue: &wgpu::Queue, _s: &ScreenDescriptor,
               _enc: &mut wgpu::CommandEncoder, resources: &mut CallbackResources)
        -> Vec<wgpu::CommandBuffer>
    {
        let res = resources.get_mut::<CanvasResources>().expect("canvas resources");
        queue.write_buffer(&res.uniform_buf, 0, bytemuck::cast_slice(&[self.mvp]));
        // Upload only dirty tiles here via queue.write_texture(dirty sub-rect) — never the
        // whole canvas. This is the 8K perf path.
        Vec::new()
    }

    fn paint(&self, _info: egui_wgpu::PaintCallbackInfo,
             rpass: &mut wgpu::RenderPass<'static>, resources: &CallbackResources)
    {
        let res = resources.get::<CanvasResources>().expect("canvas resources");
        rpass.set_pipeline(&res.pipeline);
        rpass.set_bind_group(0, &res.bind_group, &[]);
        rpass.set_vertex_buffer(0, res.vertex_buf.slice(..));
        rpass.set_index_buffer(res.index_buf.slice(..), wgpu::IndexFormat::Uint32);
        let (start, end) = self.visible_tiles;
        rpass.draw_indexed(start * 6..end * 6, 0, 0..1);
    }
}
```

## Wire it into the frame

```rust
fn canvas(&mut self, ui: &mut egui::Ui) {
    // Allocate the viewport and capture input in one call.
    let (response, painter) =
        ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
    let rect = response.rect;

    // Route tool input off `response` (see input-state-and-theming.md):
    if let Some(pos) = response.interact_pointer_pos() {
        if response.dragged() { self.paint_at(self.screen_to_canvas(pos, rect)); }
    }

    // Push the GPU callback for this rect.
    painter.add(egui_wgpu::Callback::new_paint_callback(
        rect,
        CanvasCallback {
            mvp: self.camera.mvp(rect),
            visible_tiles: self.camera.visible_tile_range(rect),
        },
    ));

    // egui shapes drawn over the same rect (brush cursor, selection ants, handles)
    // composite on top in the same frame:
    painter.circle_stroke(self.cursor_pos, self.brush_radius,
                          egui::Stroke::new(1.0, egui::Color32::WHITE));
}
```

`new_paint_callback(rect, cb)` returns an `epaint::PaintCallback` that converts into a
`Shape`; `painter.add(cb)` queues it. egui-wgpu sets the wgpu viewport/scissor to `rect`
before calling `paint`, so you normally do not call `rpass.set_viewport` yourself.

## Rules

- **GPU resources live in `CallbackResources`, built once.** Building a pipeline per frame
  is a severe stall.
- **The `Callback` struct is per-frame and cheap** — only the changing transform/range.
- **`paint` gets `&CallbackResources` (shared) and a `RenderPass<'static>` you must not
  end.** Mutate GPU state only in `prepare` (which gets `&mut CallbackResources`).
- **Upload dirty sub-rects, not the whole canvas**, with `queue.write_texture` in
  `prepare`. This is the entire point versus the texture path — see [[8k-perf-constraint]].
- **Build the pipeline against `RenderState::target_format`**, or blending/format mismatches
  will bite.
- **Overlays are egui shapes over the same rect** (brush cursor, marching ants, transform
  handles) — they composite on top of your pass in the same frame.

## Alternative: render-to-texture + display

If you'd rather render the canvas to your own `wgpu::Texture` and let egui display it,
register the view as an egui texture and draw it as an image — simpler, but egui samples it
with its own shader and you give up drawing directly into egui's pass:

```rust
let tex_id = renderer.register_native_texture(&device, &canvas_view, wgpu::FilterMode::Nearest);
painter.image(tex_id, rect, egui::Rect::from_min_max(egui::pos2(0.,0.), egui::pos2(1.,1.)),
              egui::Color32::WHITE);
```

Use the callback path for the live canvas; the register-native-texture path is fine for a
prototype or for offscreen previews.

## Flagged / verify

- `PaintCallbackInfo`: `info.viewport` (logical points) is reliable; a physical-pixel
  accessor exists but its exact name (`viewport_in_pixels()` / fields) wasn't confirmable
  from rendered docs — check egui-wgpu docs if you need physical pixels in `paint`.
- `CallbackResources` is a `TypeMap` alias; the `insert/get/get_mut/contains` interface is
  standard but confirm against the egui-wgpu version if a method name differs.
- `register_native_texture` / `update_native_texture` signatures — verify before relying on
  the texture path.
