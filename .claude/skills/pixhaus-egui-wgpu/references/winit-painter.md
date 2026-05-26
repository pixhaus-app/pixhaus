# The winit Painter — what eframe drives under the hood

egui-wgpu 0.34.2, `winit` feature. `egui_wgpu::winit::Painter` is the full surface and
render-state manager: it owns the `wgpu::Instance`, creates a `Surface` per window, holds
the `Renderer`, and runs the per-frame `update_textures → render → present` sequence.

**You almost never touch this in Pixhaus.** eframe wraps `Painter` and drives it for you;
the binary uses `eframe::run_native` (see [[pixhaus-eframe]]). This file exists so that when
you read an eframe stack trace, debug a surface or resize bug, or evaluate ever leaving
eframe, you know what the layer below is doing. Reach for it only on a deliberate
no-eframe path.

## Lifecycle

```rust
// 1. Create the painter. Builds only the wgpu Instance; device/surface are deferred until
//    a window exists. async — block with pollster off the event loop, see [[pixhaus-pollster]].
pub async fn new(
    context: egui::Context,
    configuration: WgpuConfiguration,
    support_transparent_backbuffer: bool,
    options: RendererOptions,
) -> Self

// 2. Attach a window: creates the Surface and initializes render state. Must be called
//    before any rendering. On Android, call with Some(window) on Resumed, None on Paused.
pub async fn set_window(
    &mut self,
    viewport_id: egui::ViewportId,
    window: Option<Arc<winit::window::Window>>,
) -> Result<(), WgpuError>

// 2b. Borrow-only variant; caller must keep the window alive for the painter's lifetime.
pub async unsafe fn set_window_unsafe(
    &mut self,
    viewport_id: egui::ViewportId,
    window: Option<&winit::window::Window>,
) -> Result<(), WgpuError>

// 3. Read the render state once a window is attached (None before set_window).
pub fn render_state(&self) -> Option<RenderState>

// Max texture dimension the device supports (None before set_window).
pub fn max_texture_side(&self) -> Option<usize>
```

`set_window_unsafe` is the one `unsafe` surface in this crate — and Pixhaus forbids
`unsafe` workspace-wide, so use `set_window` with an `Arc<Window>`, never the unsafe
variant.

## Per-frame

```rust
// On a resize event. width/height are NonZeroU32 physical pixels.
pub fn on_window_resized(
    &mut self,
    viewport_id: egui::ViewportId,
    width_in_pixels: NonZeroU32,
    height_in_pixels: NonZeroU32,
)

// Platform resize-state hook (e.g. the macOS Metal jitter fix); call on resize start/end.
pub fn on_window_resize_state_change(&mut self, viewport_id: egui::ViewportId, resizing: bool)

// The frame: applies texture deltas, renders the paint jobs, presents. Returns approximate
// vsync wait in seconds. Pass a non-empty capture_data to grab a screenshot this frame.
pub fn paint_and_update_textures(
    &mut self,
    viewport_id: egui::ViewportId,
    pixels_per_point: f32,
    clear_color: [f32; 4],
    clipped_primitives: &[egui::ClippedPrimitive],
    textures_delta: &egui::TexturesDelta,
    capture_data: Vec<egui::UserData>,
) -> f32

// Call at frame start to receive any screenshots requested via capture_data.
pub fn handle_screenshots(&self, events: &mut Vec<egui::Event>)
```

`paint_and_update_textures` is the method that does the `update_texture` /
`update_buffers` / `render` sequence from `renderer.md` internally — which is why you don't
call those directly on this path. The returned vsync estimate is what eframe uses to pace
the loop.

## Teardown

```rust
// Drop dead viewports. Pass the set of still-active viewport ids.
pub fn gc_viewports(&mut self, active_viewports: &ViewportIdSet)

// Tear down all surfaces/render state.
pub fn destroy(&mut self)
```

verify: the exact arg type of `gc_viewports` (`&ViewportIdSet` vs `&[ViewportIdSet]`) was
ambiguous in the rendered docs — confirm against source if you call it. All egui viewports
share one `Painter`, so viewport-scoped methods take a `ViewportId`.

## Why this is reference-only for Pixhaus

The whole point of [[pixhaus-eframe]] is that it owns this object plus the winit event
loop, the persistence directory, and the renderer choice. Reimplementing the `Painter`
drive loop means reimplementing eframe — out of scope for the editor. Read this to
understand, not to rebuild.
