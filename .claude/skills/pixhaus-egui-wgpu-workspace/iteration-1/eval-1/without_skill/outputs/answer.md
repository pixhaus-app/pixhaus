# Handing a wgpu texture to egui and drawing it crisply

You have a `wgpu::Texture` that your render crate fills each frame. To show it in
the egui viewport you do three things:

1. Register the texture's view **once** with the egui-wgpu renderer to get a
   `TextureId`.
2. Draw it every frame as an `egui::Image` (or via `Painter::image`).
3. When the texture's *contents* change in place, do nothing — you already painted
   the latest contents. When the texture *object* is replaced (resize → new
   `wgpu::Texture`), re-point the same `TextureId` at the new view.

The two functions you keep seeing answer two different questions:

- `register_native_texture` — "give me a fresh `TextureId` for this view." Call
  it **once per texture object**.
- `update_native_texture` — "keep this *existing* `TextureId`, but it now points
  at a *different* view." Call it only when you swapped the underlying texture.

Crucial point that trips people up: **you do not call either of these every frame
just because the pixels changed.** If your render crate writes into the same
`wgpu::Texture` each frame (same texture, new contents), the `TextureId` already
refers to that texture. egui samples it live at paint time, so the new contents
show up with zero extra calls. You only touch `update_native_texture` when the
texture *handle itself* is a different object than last time.

## Why a new texture only on resize

The cheap way to think about it: a `TextureId` is a stable handle into a
`HashMap` inside egui-wgpu that maps to a `wgpu::TextureView` plus a bind group.
`register_native_texture` inserts an entry. `update_native_texture` overwrites the
view behind an existing entry (and rebuilds its bind group). Same-contents,
same-object: the entry is already correct.

So the only time you replace the texture object — and therefore the only time you
call `update_native_texture` — is when the document/viewport size changes and you
allocate a new `wgpu::Texture` with new dimensions.

## Crispness: it is all in the sampler filter

A `TextureId` you register comes with a sampler. Default filtering is linear,
which is exactly the blur you want to avoid when a user zooms a 32×32 sprite to
fill the screen. For pixel art you want **nearest** filtering. egui-wgpu's
`register_native_texture_with_sampler_descriptor` lets you pick. Use
`FilterMode::Nearest` for both `mag_filter` and `min_filter`.

(There is a second, independent place egui can blur: if `Image::texture_options`
or the `TextureOptions` you pass at paint time say `Linear`, egui's own shader
path can still soften it. With a native texture the sampler descriptor is what
matters, but set both to nearest to be safe.)

## Grabbing the renderer

The egui-wgpu `Renderer` lives behind a lock inside the `RenderState` that eframe
hands you. In an eframe app you get it from `frame.wgpu_render_state()` (or stash
the `RenderState` clone at startup). The renderer must be locked mutably to
register/update, so do it in your `ui`/`update` method on the main thread.

```rust
use eframe::egui;
use eframe::egui_wgpu;
use std::num::NonZeroU64; // not needed here; left as a reminder you may import wgpu types

struct Viewport {
    /// The texture your render crate composites into. Same object across frames;
    /// replaced only on resize.
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// Stable handle egui draws. None until first registration.
    texture_id: Option<egui::TextureId>,
    /// Track the size we registered at, so we know when a resize happened.
    registered_size: [u32; 2],
}

impl Viewport {
    fn nearest_sampler() -> wgpu::SamplerDescriptor<'static> {
        wgpu::SamplerDescriptor {
            label: Some("pixhaus-viewport-nearest"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        }
    }

    /// Call once per frame, early, before you paint.
    fn sync_registration(&mut self, render_state: &egui_wgpu::RenderState) {
        let size = [self.texture.width(), self.texture.height()];
        let mut renderer = render_state.renderer.write();

        match self.texture_id {
            // First time: register and remember the id.
            None => {
                let id = renderer.register_native_texture_with_sampler_descriptor(
                    &render_state.device,
                    &self.view,
                    Self::nearest_sampler(),
                );
                self.texture_id = Some(id);
                self.registered_size = size;
            }
            // Already registered. Only re-point the id if the texture object
            // actually changed (we detect that via a size change here; if you
            // ever swap the texture without changing size, track a generation
            // counter instead and compare that).
            Some(id) if size != self.registered_size => {
                renderer.update_egui_texture_from_wgpu_texture(
                    &render_state.device,
                    &self.view,
                    wgpu::FilterMode::Nearest,
                    id,
                );
                self.registered_size = size;
            }
            // Same object, same size: nothing to do. New contents are sampled
            // live at paint time.
            Some(_) => {}
        }
    }
}
```

Two API names to be precise about in 0.34, because the older `update_native_texture`
spelling shows up in stale examples:

- Registration: `Renderer::register_native_texture` (default sampler) or
  `register_native_texture_with_sampler_descriptor` (pick your filter). Returns
  `egui::TextureId`.
- Re-pointing an existing id: `Renderer::update_egui_texture_from_wgpu_texture`,
  which takes the id, a `&TextureView`, and a `FilterMode`. This is the method
  the "`update_native_texture`" references are gesturing at; use this exact name.

Both live on `egui_wgpu::Renderer`, reached through
`render_state.renderer` (an `Arc<RwLock<Renderer>>` — `.read()` / `.write()`).

## Drawing it each frame

Once you have the `TextureId`, painting is plain egui. The texture's current
contents are whatever your render crate last wrote; egui samples them at draw
time.

```rust
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // 1. Run your wgpu render pass that writes into self.viewport.texture.
        //    (Either directly with a queue/encoder you own, or via an
        //    egui_wgpu::Callback if you render inside egui's pass — see note below.)

        // 2. Make sure egui has a TextureId pointing at the current texture.
        let render_state = frame
            .wgpu_render_state()
            .expect("eframe must be running on the wgpu backend");
        self.viewport.sync_registration(render_state);

        // 3. Draw it.
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(id) = self.viewport.texture_id {
                let tex_size = egui::vec2(
                    self.viewport.texture.width() as f32,
                    self.viewport.texture.height() as f32,
                );

                // Scale by your zoom factor; this is where "zoom in" happens.
                let draw_size = tex_size * self.zoom;

                let image = egui::Image::new(egui::load::SizedTexture::new(id, draw_size))
                    // Nearest at egui's level too, belt-and-suspenders with the
                    // sampler. Keeps it crisp if the sampler ever defaults.
                    .texture_options(egui::TextureOptions::NEAREST)
                    .fit_to_exact_size(draw_size);

                ui.add(image);
            }
        });
    }
}
```

If you'd rather paint at an explicit rect (common for a pannable/zoomable canvas
where you compute the rect from scroll + zoom yourself):

```rust
let painter = ui.painter();
painter.image(
    id,
    target_rect,                                   // where on screen, already zoomed
    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), // full UV
    egui::Color32::WHITE,                          // tint = no tint
);
```

`Painter::image` uses the sampler you registered with, so with the nearest
descriptor above this stays crisp at any zoom.

## Updating when contents change each frame

To answer the literal question: **when only the contents change, you call
nothing.** The flow is:

1. Your render crate writes new pixels into `self.viewport.texture` (via its own
   `queue.write_texture` or a render pass).
2. egui paints `texture_id`, which still points at that same texture.
3. egui-wgpu's draw samples the texture as it stands during egui's render pass —
   it sees the new contents.

The only synchronization concern is ordering: your writes into the texture must
be submitted to the queue **before** egui's render pass runs, so egui sees the
fresh data and not last frame's. In an eframe app, anything you submit during
`update` is already queued before eframe submits the egui pass, so a direct
`queue.write_texture` or your own encoder submission in `update` orders correctly.

If your render crate instead renders *inside* egui's pass (the
`egui_wgpu::Callback` path, where you implement `CallbackTrait::paint` and draw
directly into egui's render pass), then you are not using a `TextureId` for the
viewport at all — you draw your geometry straight into the surface and skip the
register/update dance. That's the other valid architecture. Pick one:

- **Composite to your own texture, hand egui a `TextureId`** (what you asked
  about): simplest, your renderer stays fully UI-agnostic, one extra full-screen
  blit's worth of cost. Use the code above.
- **`egui_wgpu::Callback`**: zero blit, but your render code runs inside egui's
  pass and has to cope with egui's encoder/viewport. More coupling.

For a pixel editor the texture-handle path is the clean default, and it's what
keeps your render crate knowing nothing about egui.

## Cleanup

When you drop the viewport (or replace the texture and want to release the old
binding), free the entry so the bind group isn't leaked:

```rust
let mut renderer = render_state.renderer.write();
if let Some(id) = self.texture_id.take() {
    renderer.free_texture(&id);
}
```

`update_egui_texture_from_wgpu_texture` reuses the id and rebuilds its bind group,
so you don't need to free on resize — only when you're truly done with the id.

## Summary

- Register **once** with `register_native_texture_with_sampler_descriptor` using
  `FilterMode::Nearest` → get a `TextureId`.
- Draw **every frame** with `egui::Image` / `Painter::image` using that id; set
  `TextureOptions::NEAREST` too.
- Contents changed in place: **call nothing**, just make sure your texture writes
  are queued before egui's pass.
- Texture object replaced (resize): call
  `update_egui_texture_from_wgpu_texture` to re-point the same id at the new view.
- `free_texture` only when you're done with the id.
