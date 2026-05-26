# Capturing the rendered frame (screenshots)

egui-wgpu 0.34.2, `capture` feature. Helpers for reading the rendered frame back from the
GPU to the CPU — for "export current view as PNG", a thumbnail, or a visual-regression
snapshot. The surface texture often can't be copied directly (`COPY_SRC` isn't allowed on
all platforms), so the frame is first rendered to an intermediate texture, then copied to
both the surface and a readback buffer.

In Pixhaus-on-eframe the easy path is eframe's screenshot mechanism (request a screenshot
via the viewport command; eframe routes the bytes back as an event). Drop to this module
only when you drive the `winit::Painter` yourself or need the raw readback inside the
`render` crate.

## Module surface

```rust
// egui_wgpu::capture
pub struct CaptureState { /* texture + readback buffer */ }
pub fn capture_channel() -> (CaptureSender, CaptureReceiver);
pub type CaptureSender = /* sender half */;
pub type CaptureReceiver = /* receiver half */;
```

verify: the concrete target types of `CaptureSender` / `CaptureReceiver` and the exact
return of `capture_channel` didn't render fully on docs.rs — confirm against source if you
need to name them. They're an async channel of finished screenshots.

## `CaptureState`

```rust
pub struct CaptureState {
    pub texture: Texture,   // egui_wgpu::Texture: the intermediate render target
    /* private fields */
}

impl CaptureState {
    pub fn new(device: &Device, surface_texture: &wgpu::Texture) -> Self;

    // Reallocate if the surface size changed. Call each frame before capturing.
    pub fn update(&mut self, device: &Device, texture: &wgpu::Texture);

    // Copy the capture texture to the surface (render pass) AND to a readback buffer
    // (texture-to-buffer copy). Returns the buffer to hand to read_screen_rgba.
    pub fn copy_textures(
        &mut self,
        device: &Device,
        output_frame: &wgpu::SurfaceTexture,
        encoder: &mut CommandEncoder,
    ) -> wgpu::Buffer;

    // Non-blocking: maps the buffer and sends the RGBA result to `tx` when ready. Call
    // AFTER the encoder has been submitted. Pass the buffer returned by copy_textures.
    pub fn read_screen_rgba(
        &self,
        ctx: egui::Context,
        buffer: wgpu::Buffer,
        data: Vec<egui::UserData>,
        tx: CaptureSender,
        viewport_id: egui::ViewportId,
    );
}
```

The sequence:

1. `update` the capture state to match the current surface size.
2. Render your frame into `capture_state.texture` instead of straight to the surface.
3. `copy_textures` to fan the result out to the surface (so the user still sees it) and to
   a readback buffer.
4. Submit the encoder.
5. `read_screen_rgba` to map the buffer asynchronously; the RGBA bytes arrive on the
   receiver half of `capture_channel`.

`read_screen_rgba` is non-blocking and must come after `queue.submit` — calling it before
submission maps a buffer whose copy hasn't run. Drain the receiver on a later frame (it
won't be ready the same frame). `egui::UserData` is the tag you attach so you can match a
finished screenshot to the request that asked for it.

## Pixhaus guidance

- For "export the canvas to PNG," prefer rendering the document straight to your own
  offscreen texture in the `render` crate and reading that back — you don't need the egui
  chrome in the export, and you avoid coupling export to the surface size. This module is
  for capturing what's *on screen*, chrome included.
- For visual-regression tests, `RendererOptions::PREDICTABLE` (see `renderer.md`) plus a
  fixed-size offscreen render gives deterministic bytes to compare with `image-compare`
  (see [[pixhaus-testing-conventions]]).
