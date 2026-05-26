# Painting, textures, and fonts

egui 0.34.2 / epaint. Raw drawing, color/alpha, displaying pixel data as textures, text.
For the GPU canvas render pass see `custom-wgpu-canvas.md`.

## Contents
- `Painter`
- Shapes, `Color32` (premultiplied alpha), `Stroke`, `CornerRadius`
- Geometry (`Pos2`, `Vec2`, `Rect`, `Align2`)
- Textures: `ColorImage`, `TextureOptions`, `load_texture`, `set_partial`
- Displaying a texture
- Fonts and text

## `Painter`

Deferred drawing onto a layer; every call queues a `Shape`.

```rust
let p = ui.painter();                       // shares the Ui's layer + clip rect
let p = ui.painter_at(rect);                // clipped to rect
let (response, p) = ui.allocate_painter(size, egui::Sense::click_and_drag()); // alloc + paint
let p2 = p.with_clip_rect(sub_rect);        // cheap narrowed clone
```

Draw calls (most return a `ShapeIdx`):

```rust
p.rect_filled(rect, corner_radius, fill);                 // corner_radius: impl Into<CornerRadius>
p.rect_stroke(rect, corner_radius, stroke, egui::StrokeKind::Inside);  // Inside|Middle|Outside
p.rect(rect, corner_radius, fill, stroke, egui::StrokeKind::Middle);
p.circle_filled(center, radius, fill);
p.circle_stroke(center, radius, stroke);
p.line_segment([a, b], stroke);
p.image(texture_id, rect, uv_rect, egui::Color32::WHITE); // uv = (0,0)..(1,1) for whole image
p.text(pos, egui::Align2::LEFT_TOP, "Layer 1", font_id, color);  // -> Rect bounds
p.add(shape);  p.extend(shapes);
```

`rect_stroke` requires a `StrokeKind` (new in recent egui). For 1px pixel-grid lines use
`StrokeKind::Middle`. For tile/canvas rects use `CornerRadius::ZERO` to avoid sub-pixel
blur at boundaries.

## Shapes, color, stroke

`Shape` variants: `Rect`, `Circle`, `Ellipse`, `LineSegment`, `Path` (polyline/polygon),
`Text`, `Mesh(Arc<Mesh>)`, beziers, `Callback` (the wgpu/glow hook), `Vec`, `Noop`.

`Color32` stores **premultiplied** sRGBA. Construct from user-facing values with
`from_rgba_unmultiplied` — using `from_rgba_premultiplied` on straight-alpha values blends
wrong for semi-transparent colors.

```rust
egui::Color32::from_rgba_unmultiplied(r, g, b, a);   // user-facing colors (color picker)
egui::Color32::from_rgb(r, g, b);                     // opaque
egui::Color32::from_gray(l);
// accessors r()/g()/b() return PREMULTIPLIED channels; to_srgba_unmultiplied() -> [u8;4]
// constants: TRANSPARENT, BLACK, WHITE, RED, …; PLACEHOLDER = "use the fallback color"

egui::Stroke::new(width, color);   egui::Stroke::NONE;

egui::CornerRadius::ZERO;          egui::CornerRadius::same(4);
// From<u8>/From<f32>: you can pass 0u8 / 4.0 where Into<CornerRadius> is expected.
// (CornerRadius replaced Rounding; per-corner fields nw/ne/sw/se: u8.)
```

`Mesh` is a textured triangle list (`vertices: Vec<Vertex { pos, uv, color }>`,
`indices: Vec<u32>`, `texture_id`). For a textured quad:

```rust
let mut mesh = egui::Mesh { texture_id: handle.id(), ..Default::default() };
mesh.add_rect_with_uv(rect, egui::Rect::from_min_max(egui::pos2(0.,0.), egui::pos2(1.,1.)),
                      egui::Color32::WHITE);
p.add(egui::Shape::Mesh(std::sync::Arc::new(mesh)));
```

## Geometry

```rust
egui::pos2(x, y);  egui::vec2(x, y);
egui::Rect::from_min_size(min, size);  ::from_center_size(c, size);  ::from_min_max(min, max);
rect.width(); rect.height(); rect.center(); rect.contains(p);
rect.shrink(1.0); rect.expand(2.0); rect.translate(v); rect.intersect(other);
// Align2::{LEFT_TOP, CENTER_CENTER, RIGHT_BOTTOM, …} for text anchor / rect anchoring.
```

## Textures (displaying pixel data)

The path: CPU bytes → `ColorImage` → `ctx.load_texture(name, image, options)` →
`TextureHandle` (RAII; freed when dropped). For pixel art, filtering is **always NEAREST**.

```rust
let image = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba_bytes); // straight alpha in
let handle = ctx.load_texture("canvas", image, egui::TextureOptions::NEAREST);
```

`ColorImage` constructors: `from_rgba_unmultiplied([w,h], &[u8])`, `from_rgb`,
`new([w,h], Vec<Color32>)`, `filled([w,h], color)`.

`TextureOptions` presets: `NEAREST`, `LINEAR`, `NEAREST_REPEAT`, `LINEAR_REPEAT`,
`NEAREST_MIRRORED_REPEAT`, `LINEAR_MIRRORED_REPEAT`. Fields: `magnification`,
`minification` (`TextureFilter::{Nearest, Linear}`), `wrap_mode`, `mipmap_mode`. Default
wrap is clamp-to-edge — keep it to avoid tile-edge bleeding.

**`load_texture` allocates GPU memory — call it once, cache the handle.** Never per frame.

**Dirty-region updates** are the key to the 8K perf target: re-upload only the changed
sub-rect with `set_partial`, never the whole texture each frame.

```rust
impl TextureHandle {
    fn set(&mut self, image: impl Into<ImageData>, options: TextureOptions);            // whole
    fn set_partial(&mut self, pos: [usize; 2], image: impl Into<ImageData>, options: TextureOptions);
    fn id(&self) -> TextureId;
    fn size(&self) -> [usize; 2];
    fn size_vec2(&self) -> Vec2;
}

// on a dirty stroke region [x, y, w, h]:
let sub = egui::ColorImage::from_rgba_unmultiplied([w, h], &subregion_bytes);
handle.set_partial([x, y], sub, egui::TextureOptions::NEAREST);
```

Note: this texture path is the simpler way to get pixels on screen, but it still copies CPU
bytes to the GPU. The fully native canvas keeps the buffer on the GPU via a `wgpu` callback
— see `custom-wgpu-canvas.md`. The texture path is fine for thumbnails, palette previews,
onion-skin layers, and a first cut of the canvas.

## Displaying a texture

```rust
ui.image((handle.id(), handle.size_vec2()));                      // convenience, auto-sized
ui.add(egui::Image::new(egui::load::SizedTexture::from_handle(&handle)));

// painter, full control of rect + UV (e.g. zoom/pan crop):
p.image(handle.id(), canvas_rect,
        egui::Rect::from_min_max(egui::pos2(0.,0.), egui::pos2(1.,1.)), egui::Color32::WHITE);
```

## Fonts and text

```rust
egui::FontId::proportional(14.0);  egui::FontId::monospace(12.0);
egui::FontId::new(13.0, egui::FontFamily::Proportional);
// TextStyle::{Small, Body, Monospace, Button, Heading} resolve to a FontId via Style.

// Lay out once, draw (reuse the galley for measuring/hit-testing):
let galley = p.layout_no_wrap("Layer 1".into(), egui::FontId::proportional(13.0),
                              egui::Color32::from_gray(220));
p.galley(pos, galley, egui::Color32::WHITE);   // fallback color used for PLACEHOLDER spans
// p.layout(text, font, color, wrap_width) wraps. ctx.fonts(|f| f.layout_no_wrap(...))
//   gives direct font access but only AFTER the first frame (pixels_per_point must be known).
```

Custom font:

```rust
let mut fonts = egui::FontDefinitions::default();
fonts.font_data.insert("pixel".into(),
    std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/Pixel.ttf"))));
fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, "pixel".into());
ctx.set_fonts(fonts);   // do this once, in App::new
```

## Pixel-art rules

| Concern | Rule |
|---|---|
| Filtering | `TextureOptions::NEAREST` always; never `LINEAR` on the canvas |
| Wrap | Keep clamp-to-edge (default) to stop tile-edge bleed |
| Upload cost | `set_partial` dirty rects; never full re-upload per frame |
| `load_texture` | Once, cache the handle |
| Alpha | `from_rgba_unmultiplied` for user colors |
| Corner radius | `CornerRadius::ZERO` on canvas/tile rects |

## Flagged / verify

- `TextureWrapMode` variant names (clamp/repeat/mirror) weren't confirmable from the
  rendered docs — the preset `TextureOptions` constants above are correct; check the exact
  enum names if you set `wrap_mode` by hand.
- `register_native_texture` (display a `wgpu::TextureView` as an egui image) exists on
  `egui_wgpu::Renderer`; `update_native_texture` for per-frame updates — verify signatures
  against egui-wgpu docs before relying on them.
