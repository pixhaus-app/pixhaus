# The custom rendering callback system

egui-wgpu 0.34.2. How to splice a raw wgpu render pass into the egui frame — the
make-or-break feature for the Pixhaus canvas. This file is the complete API; the
Pixhaus-shaped worked example (input routing, camera, overlays) is in `pixhaus-egui`'s
`references/custom-wgpu-canvas.md`.

## `CallbackTrait` — verbatim signatures

```rust
use egui_wgpu::{CallbackTrait, CallbackResources, ScreenDescriptor};
use egui::epaint::PaintCallbackInfo;
use wgpu::{Device, Queue, CommandEncoder, CommandBuffer, RenderPass};

pub trait CallbackTrait: Send + Sync {
    // REQUIRED. Inside the egui render pass. resources is SHARED (&), and the pass is
    // 'static and owned by egui-wgpu: issue draws only, never end it or begin sub-passes.
    fn paint(
        &self,
        info: PaintCallbackInfo,                 // by value
        render_pass: &mut RenderPass<'static>,
        callback_resources: &CallbackResources,  // shared
    );

    // PROVIDED (default returns empty Vec). Before the egui pass. &mut resources, has the
    // ScreenDescriptor. Upload buffers and dirty texture sub-rects here. Returned command
    // buffers are submitted BEFORE the egui pass.
    fn prepare(
        &self,
        device: &Device,
        queue: &Queue,
        screen_descriptor: &ScreenDescriptor,
        egui_encoder: &mut CommandEncoder,
        callback_resources: &mut CallbackResources,
    ) -> Vec<CommandBuffer> { Vec::new() }

    // PROVIDED (default returns empty Vec). After ALL prepare() calls. &mut resources, NO
    // ScreenDescriptor. Its buffers submit after all prepare() buffers. Rarely needed.
    fn finish_prepare(
        &self,
        device: &Device,
        queue: &Queue,
        egui_encoder: &mut CommandEncoder,
        callback_resources: &mut CallbackResources,
    ) -> Vec<CommandBuffer> { Vec::new() }
}
```

Call order across every registered callback in the frame:
`prepare` (all) → `finish_prepare` (all) → main egui render pass begins → `paint` (all).
Command buffers from `prepare` submit before those from `finish_prepare`, both before the
egui pass.

Why the `&mut` vs `&` split matters: GPU uploads (writing buffers/textures, swapping
resources) happen in the prepare phases, which get `&mut CallbackResources`. `paint` gets
`&CallbackResources` because the render pass is already recording — you can only read your
pipeline/buffers and issue draws. Trying to mutate resources in `paint` won't compile, and
that's the API steering you correctly.

The supertrait is `Send + Sync`, so everything you store in `CallbackResources` must be
`Send + Sync` too (wgpu handles are).

## `Callback` — building the paint command

```rust
pub struct Callback(/* private */);

impl Callback {
    pub fn new_paint_callback(
        rect: egui::Rect,
        callback: impl CallbackTrait + 'static,
    ) -> egui::epaint::PaintCallback
}
```

`rect` is where on screen to paint; it becomes `PaintCallbackInfo::viewport` downstream.
The result is an `epaint::PaintCallback` — add it to a painter as a shape:

```rust
let (response, painter) =
    ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
painter.add(egui_wgpu::Callback::new_paint_callback(
    response.rect,
    CanvasCallback { mvp: camera.mvp(response.rect), tiles: camera.visible_tiles() },
));
```

egui-wgpu sets the wgpu viewport and scissor to `rect` before calling `paint`, so you
normally don't call `set_viewport`/`set_scissor_rect` yourself.

## `CallbackResources` — the typed GPU store

```rust
pub type CallbackResources = type_map::concurrent::TypeMap;
```

A type-map: one stored value per type. It's the public `Renderer::callback_resources`
field. Methods you use:

```rust
fn insert<T: Send + Sync + 'static>(&mut self, val: T) -> Option<T>  // returns prior value
fn get<T: 'static>(&self) -> Option<&T>
fn get_mut<T: 'static>(&mut self) -> Option<&mut T>
fn contains<T: 'static>(&self) -> bool
fn remove<T: 'static>(&mut self) -> Option<T>
fn entry<T: Send + Sync + 'static>(&mut self) -> Entry<'_, T>
fn clear(&mut self)
```

In `paint` (shared `&`) only `get`/`contains` are available; in the prepare phases
(`&mut`) use `insert`/`get_mut`/`entry`. Insert your resources struct once at startup (see
`renderer.md`), or lazily in `prepare`:

```rust
fn prepare(&self, device: &Device, queue: &Queue, _s: &ScreenDescriptor,
           _enc: &mut CommandEncoder, res: &mut CallbackResources) -> Vec<CommandBuffer> {
    if !res.contains::<CanvasResources>() {
        res.insert(CanvasResources::new(device /*, format */));
    }
    let canvas = res.get_mut::<CanvasResources>().expect("just inserted");
    queue.write_buffer(&canvas.uniform_buf, 0, bytemuck::cast_slice(&[self.mvp]));
    // upload ONLY dirty tiles: queue.write_texture(sub-rect) — never the whole canvas.
    Vec::new()
}

fn paint(&self, _info: PaintCallbackInfo, rpass: &mut RenderPass<'static>,
         res: &CallbackResources) {
    let canvas = res.get::<CanvasResources>().expect("canvas resources");
    rpass.set_pipeline(&canvas.pipeline);
    rpass.set_bind_group(0, &canvas.bind_group, &[]);
    rpass.set_vertex_buffer(0, canvas.vertex_buf.slice(..));
    rpass.draw(0..6, 0..1);
}
```

Lazy init can't see `target_format` cleanly, so prefer startup init where you have it.

## `PaintCallbackInfo` — viewport and scissor math in `paint`

Re-exported as `egui::epaint::PaintCallbackInfo` (canonical docs live on the `epaint`
crate, not `egui`).

```rust
pub struct PaintCallbackInfo {
    pub viewport: egui::Rect,    // in POINTS; this is the rect you passed to new_paint_callback
    pub clip_rect: egui::Rect,   // in POINTS
    pub pixels_per_point: f32,
    pub screen_size_px: [u32; 2],// full screen size in physical pixels
}

impl PaintCallbackInfo {
    pub fn viewport_in_pixels(&self) -> ViewportInPixels;   // for glViewport-style use
    pub fn clip_rect_in_pixels(&self) -> ViewportInPixels;  // for glScissor-style use
}

pub struct ViewportInPixels {
    pub left_px: i32,
    pub top_px: i32,
    pub from_bottom_px: i32,  // GL-style y origin (bottom); what glViewport/glScissor want
    pub width_px: i32,
    pub height_px: i32,
}
```

When you do need explicit viewport/scissor (a sub-pass, or restoring after a nested draw),
use `viewport_in_pixels()` / `clip_rect_in_pixels()` — physical pixels, already DPI-scaled.
This resolves the old "is there a physical-pixel accessor?" question the `pixhaus-egui`
canvas reference flagged: yes, these two methods, returning `ViewportInPixels`.

## Rules

- **Build GPU resources once, store in `CallbackResources`.** A pipeline rebuild per frame
  is a severe stall.
- **The `Callback` struct is per-frame and cheap** — only the changing transform / tile
  range, never GPU handles.
- **Upload dirty sub-rects in `prepare`, not the whole canvas** — `queue.write_texture` on
  a sub-region. This is the entire reason for the callback path; see [[8k-perf-constraint]].
- **`paint` must not end the pass or begin a new one.** It borrows egui's `'static` pass.
- **Build the pipeline against `RenderState::target_format`**, or blending/format
  mismatches bite.
- **Overlays (brush cursor, marching ants, transform handles) are egui shapes** over the
  same rect — they composite on top of your pass in the same frame. Draw them with the
  `painter` after adding the callback.
