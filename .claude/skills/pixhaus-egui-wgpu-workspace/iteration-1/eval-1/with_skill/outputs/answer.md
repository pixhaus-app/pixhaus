# Displaying your render crate's `wgpu::Texture` as an egui image

You're on the right path. Since `render` already composites into its own `wgpu::Texture`, you don't need the `CallbackTrait` route at all — that's for splicing a live wgpu pass *inside* the egui frame. You just want egui to sample your finished texture as an image. That's the `register_native_texture` path, and it's three moving parts: register once, update each frame, draw each frame.

## First, clear up the naming — there is no `update_native_texture`

This is the trap, and the reason you keep second-guessing. In egui-wgpu 0.34:

- `register_native_texture` — call **once** per texture. It allocates a bind group and hands you a `TextureId`. Calling it every frame leaks a `TextureId` per frame.
- `update_egui_texture_from_wgpu_texture` — the in-place updater. This is the function people mean when they say "`update_native_texture`," which **does not exist** under that name. It re-points an existing `TextureId` at a (possibly new) `TextureView` without allocating a new id.

The full set on `egui_wgpu::Renderer`:

```rust
pub fn register_native_texture(
    &mut self, device: &wgpu::Device,
    texture: &wgpu::TextureView, texture_filter: wgpu::FilterMode,
) -> egui::TextureId

pub fn register_native_texture_with_sampler_options(
    &mut self, device: &wgpu::Device,
    texture: &wgpu::TextureView, sampler_descriptor: wgpu::SamplerDescriptor<'_>,
) -> egui::TextureId

pub fn update_egui_texture_from_wgpu_texture(
    &mut self, device: &wgpu::Device,
    texture: &wgpu::TextureView, texture_filter: wgpu::FilterMode, id: egui::TextureId,
)

pub fn update_egui_texture_from_wgpu_texture_with_sampler_options(
    &mut self, device: &wgpu::Device,
    texture: &wgpu::TextureView,
    sampler_descriptor: wgpu::SamplerDescriptor<'_>, id: egui::TextureId,
)

pub fn free_texture(&mut self, id: &egui::TextureId)
```

## Crisp, not blurry — pass `FilterMode::Nearest`

This is the whole answer to "no blurring when the user zooms in." Pixel art must sample with nearest-neighbor, so pass `wgpu::FilterMode::Nearest` to `register_native_texture` (and to `update_egui_texture_from_wgpu_texture` if you ever re-point). Magnifying with `Linear` is exactly what gives you the blurred-when-zoomed look you're trying to avoid.

If you need to be explicit about both min and mag filters, use the `_with_sampler_options` variants and set `mag_filter` and `min_filter` to `wgpu::FilterMode::Nearest`.

## When do you actually call `update_egui_texture_from_wgpu_texture`?

Here's the part the question is really circling. The `TextureId` is bound to a **bind group**, which references a **`TextureView`**. So:

- If your render crate **writes into the same `wgpu::Texture` every frame** (same allocation — `queue.write_texture`, or a render pass targeting the same texture), the contents change but the `TextureView` is still valid. egui samples the live GPU texture on the next frame **with no re-registration and no update call**. Register once, draw every frame, done. This is the common case and the cheapest one.

- You only need `update_egui_texture_from_wgpu_texture` when the **`TextureView` itself changes** — i.e. you reallocated the texture. That happens when the document resizes (canvas grows past the current texture, you swap to a bigger allocation). Then the old view is stale; call the updater to re-point the existing `TextureId` at the new view, so any UI holding that id keeps working.

So the rule: **register once; re-point only on reallocation.** Don't call the updater every frame "to be safe" — it rebuilds a bind group for nothing.

## The lock — `parking_lot`, no `Result`, no `.unwrap()`

`state.renderer` is `Arc<RwLock<Renderer>>` from `parking_lot`, so `.write()` returns the guard directly. No `Result`, nothing to unwrap (which keeps you clear of the no-unwrap rule). Take the guard, do the call, drop it before you do anything `.await`-y — you won't be awaiting here anyway.

## Putting it together

Grab the `RenderState` once at startup (from `eframe::CreationContext::wgpu_render_state()`), register the texture, stash the `TextureId`. Then draw it in the viewport each frame.

### Setup — register once

```rust
use std::sync::Arc;

pub struct ViewportState {
    /// The egui-side handle to the render crate's composited texture.
    canvas_id: egui::TextureId,
    /// Texture dimensions in pixels, for aspect-correct sizing.
    canvas_size: [u32; 2],
}

impl ViewportState {
    /// Call once, when you first have both the RenderState and the render
    /// crate's texture view.
    pub fn new(
        render_state: &egui_wgpu::RenderState,
        canvas_view: &wgpu::TextureView,
        canvas_size: [u32; 2],
    ) -> Self {
        // NEAREST is the no-blur rule for pixel art — without it, zooming in
        // bilinearly interpolates and the sprite turns to mush.
        let canvas_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            canvas_view,
            wgpu::FilterMode::Nearest,
        );

        Self { canvas_id, canvas_size }
    }
}
```

### Each frame — draw it in the viewport

Hold the `RenderState` clone (it's `Clone`) on your app so the update loop can reach it. The texture's *contents* updating each frame needs nothing here — egui samples the live texture. You only touch the renderer again on reallocation (next section).

```rust
impl ViewportState {
    /// Draw the canvas inside the central panel, scaled by `zoom` and offset
    /// by `pan`, with nearest-neighbor crispness preserved by the sampler.
    pub fn show(&self, ui: &mut egui::Ui, zoom: f32, pan: egui::Vec2) {
        let tex_w = self.canvas_size[0] as f32;
        let tex_h = self.canvas_size[1] as f32;

        // The on-screen rect: texel size times zoom, placed at the pan origin.
        // CornerRadius::ZERO and an integer-ish zoom keep tile edges sharp.
        let size = egui::vec2(tex_w * zoom, tex_h * zoom);
        let min = ui.max_rect().min + pan;
        let canvas_rect = egui::Rect::from_min_size(min, size);

        // Full-image UV: (0,0)..(1,1). Crop here if you implement view culling.
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));

        ui.painter().image(
            self.canvas_id,
            canvas_rect,
            uv,
            egui::Color32::WHITE, // WHITE = no tint
        );
    }
}
```

`Color32::WHITE` as the tint means "draw the texture unmodified." `painter().image` gives you full control of the destination rect and UV, which is what you want for zoom and pan — `ui.image(...)` auto-sizes and fights you on layout.

### On document resize — re-point the id

```rust
impl ViewportState {
    /// Call ONLY when the render crate reallocated its texture (e.g. the
    /// document grew). Re-points the existing TextureId at the new view so the
    /// UI keeps the same id. Do NOT call this every frame.
    pub fn on_texture_reallocated(
        &mut self,
        render_state: &egui_wgpu::RenderState,
        new_view: &wgpu::TextureView,
        new_size: [u32; 2],
    ) {
        render_state.renderer.write().update_egui_texture_from_wgpu_texture(
            &render_state.device,
            new_view,
            wgpu::FilterMode::Nearest,
            self.canvas_id,
        );
        self.canvas_size = new_size;
    }
}
```

### Wiring into the eframe app

```rust
struct PixhausApp {
    render_state: egui_wgpu::RenderState, // RenderState is Clone
    viewport: ViewportState,
    zoom: f32,
    pan: egui::Vec2,
}

impl PixhausApp {
    fn new(cc: &eframe::CreationContext<'_>, canvas_view: &wgpu::TextureView,
           canvas_size: [u32; 2]) -> Self {
        let render_state = cc
            .wgpu_render_state()
            .expect("eframe must be built with the wgpu backend")
            .clone();
        let viewport = ViewportState::new(&render_state, canvas_view, canvas_size);
        Self { render_state, viewport, zoom: 1.0, pan: egui::Vec2::ZERO }
    }
}

impl eframe::App for PixhausApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // render crate composites into its texture here (its own pass / write_texture),
        // before egui samples it this frame.

        egui::CentralPanel::default().show(ctx, |ui| {
            self.viewport.show(ui, self.zoom, self.pan);
        });

        ctx.request_repaint(); // keep animating if playback is live
    }
}
```

(eframe 0.34's `App` method is `update`; if you're on the variant exposing `ui`, see `pixhaus-eframe` — the texture wiring is identical either way.)

## Ordering: composite before egui samples

Make sure your render crate's submit (the pass or `queue.write_texture` that fills the texture) happens **before** egui's frame samples it. Inside `update`, that means run the composite at the top of the function, before `CentralPanel`. Since egui-wgpu records the egui draw on the same device/queue, a composite submitted earlier in the same frame is visible. If your render crate composites on a background thread, gate the egui draw on a "frame ready" signal so you're not sampling a half-written texture.

## Native-texture path vs. the callback path — pick deliberately

You chose right for your situation, but know the tradeoff:

- **This path (`register_native_texture`)** — render crate owns the texture, egui blits it as an image. Simple, clean separation, ideal because your `render` crate is already UI-agnostic and produces a finished texture. Cost: the composite result lives in a texture that egui samples; you can't interleave egui shapes *into* the wgpu pass.
- **The `CallbackTrait` path** — splice a raw wgpu pass directly into the egui frame, drawing into egui's target. Use it when you need per-frame GPU work fused with egui, or want overlays (selection, grid) painted in the same pass. More machinery (`prepare`/`paint` phases, the `CallbackResources` type-map, the `update_buffers`-or-panic contract).

For "render crate makes a texture, egui shows it," the native-texture path is the correct, lighter choice. Reach for the callback only if you later need to merge GPU drawing with egui's own pass.

## Checklist

1. `register_native_texture(device, view, FilterMode::Nearest)` once → keep the `TextureId`.
2. Each frame: composite into the texture (render crate), then `painter().image(id, rect, uv, Color32::WHITE)`.
3. Don't re-register or re-update if the texture allocation is unchanged — egui samples the live texture.
4. Only on reallocation: `update_egui_texture_from_wgpu_texture(device, new_view, FilterMode::Nearest, id)`.
5. `Nearest` everywhere, `CornerRadius::ZERO` on the rect — that's your no-blur guarantee.
