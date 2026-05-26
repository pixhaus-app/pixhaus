# Painting, textures, and custom GPU rendering in egui

**Verified versions:** egui 0.34.2, egui-wgpu 0.34.2 (wgpu 29.0.1 transitive dep)

---

## 1. Painter

`Painter` is egui's drawing primitive. It wraps a `LayerId` and a clip rect; all draw calls are deferred as `Shape` entries into that layer's paint list.

### Obtaining a painter

```rust
// From within a Ui closure — shares the Ui's layer and clip rect
let painter: &Painter = ui.painter();

// Clip to a sub-rect of the Ui (still same layer)
let painter: Painter = ui.painter_at(rect);

// Allocate layout space AND get a painter — the idiomatic custom-widget entry point
let (response, painter): (Response, Painter) =
    ui.allocate_painter(desired_size, Sense::click_and_drag());

// Full-screen painter for a named layer (use for background canvases)
let painter: Painter = ctx.layer_painter(LayerId::new(Order::Middle, Id::new("canvas")));
```

`painter.with_clip_rect(rect) -> Painter` creates a new `Painter` value (cheap, no allocation) whose clip is narrowed to the intersection of `rect` and the current clip.

```rust
let clipped = painter.with_clip_rect(tile_rect);
clipped.rect_filled(tile_rect, CornerRadius::ZERO, Color32::RED);
```

### Painter methods — full signatures

All methods return `ShapeIdx` (an opaque index into the layer's shape list) unless noted.

```rust
// Filled rect
pub fn rect_filled(
    &self,
    rect: Rect,
    corner_radius: impl Into<CornerRadius>,
    fill_color: impl Into<Color32>,
) -> ShapeIdx

// Stroked rect — note the new StrokeKind parameter (added ~0.28)
pub fn rect_stroke(
    &self,
    rect: Rect,
    corner_radius: impl Into<CornerRadius>,
    stroke: impl Into<Stroke>,
    stroke_kind: StrokeKind,   // Inside | Middle | Outside
) -> ShapeIdx

// Fill + stroke in one call
pub fn rect(
    &self,
    rect: Rect,
    corner_radius: impl Into<CornerRadius>,
    fill_color: impl Into<Color32>,
    stroke: impl Into<Stroke>,
    stroke_kind: StrokeKind,
) -> ShapeIdx

// Circles
pub fn circle_filled(&self, center: Pos2, radius: f32, fill_color: impl Into<Color32>) -> ShapeIdx
pub fn circle_stroke(&self, center: Pos2, radius: f32, stroke: impl Into<Stroke>) -> ShapeIdx

// Line segment
pub fn line_segment(&self, points: [Pos2; 2], stroke: impl Into<Stroke>) -> ShapeIdx

// Arbitrary shape — anything that converts Into<Shape>
pub fn add(&self, shape: impl Into<Shape>) -> ShapeIdx

// Add many shapes; no return value (avoids per-shape overhead)
pub fn extend<I: IntoIterator<Item = Shape>>(&self, shapes: I)

// Draw a texture/image
// uv: Rect::from_min_max(pos2(0.0,0.0), pos2(1.0,1.0)) for the whole image
// tint: Color32::WHITE for no tinting
pub fn image(
    &self,
    texture_id: TextureId,
    rect: Rect,
    uv: Rect,
    tint: Color32,
) -> ShapeIdx

// Draw text at a point with anchor
// Returns the bounding rect of the rendered text
pub fn text(
    &self,
    pos: Pos2,
    anchor: Align2,       // e.g. Align2::LEFT_TOP, Align2::CENTER_CENTER
    text: impl ToString,
    font_id: FontId,
    text_color: Color32,
) -> Rect

// Draw a pre-laid-out galley; PLACEHOLDER colors use fallback_color
pub fn galley(&self, pos: Pos2, galley: Arc<Galley>, fallback_color: Color32)

// Same but overrides ALL colors in the galley
pub fn galley_with_override_text_color(&self, pos: Pos2, galley: Arc<Galley>, text_color: Color32)

// Layout text (produces a galley) — wraps at wrap_width
pub fn layout(
    &self,
    text: String,
    font_id: FontId,
    color: Color32,
    wrap_width: f32,
) -> Arc<Galley>

// Layout text, no wrapping (single line)
pub fn layout_no_wrap(&self, text: String, font_id: FontId, color: Color32) -> Arc<Galley>

// Clip rect access
pub fn clip_rect(&self) -> Rect
pub fn set_clip_rect(&mut self, clip_rect: Rect)
pub fn with_clip_rect(&self, rect: Rect) -> Self   // returns new Painter
```

### StrokeKind (used by rect_stroke / rect)

```rust
pub enum StrokeKind {
    Inside,   // stroke painted entirely inside the shape boundary
    Middle,   // stroke centered on the edge, half inside half outside
    Outside,  // stroke painted entirely outside the shape boundary
}
```

For pixel-art grid lines use `StrokeKind::Middle` at width 1.0.

### Layer ordering (Order enum)

```rust
pub enum Order {
    Background,         // behind all floating windows
    Middle,             // normal moveable windows
    Foreground,         // popups and menus
    Tooltip,            // tooltips — no interaction
    Debug,              // always on top (= Order::TOP)
}
```

For the canvas: `Order::Background` so panels render above it, or `Order::Middle` to share z with tool windows. Use `LayerId::new(order, Id::new("pixhaus_canvas"))`.

---

## 2. Shape variants and epaint types

### Shape enum

```rust
pub enum Shape {
    Noop,
    Vec(Vec<Shape>),        // nested; avoid in hot paths
    Circle(CircleShape),
    Ellipse(EllipseShape),
    LineSegment { points: [Pos2; 2], stroke: Stroke },
    Path(PathShape),        // polyline or filled polygon
    Rect(RectShape),
    Text(TextShape),        // pre-laid-out text; recreate if pixels_per_point changes
    Mesh(Arc<Mesh>),        // arbitrary triangle mesh; the pixel art canvas primitive
    QuadraticBezier(QuadraticBezierShape),
    CubicBezier(CubicBezierShape),
    Callback(PaintCallback), // backend-specific (wgpu, glow)
}
```

Convenience constructors live on `Shape::`:

```rust
Shape::rect_filled(rect, corner_radius, fill_color) -> Shape
Shape::rect_stroke(rect, corner_radius, stroke, stroke_kind) -> Shape
Shape::line_segment([p0, p1], stroke) -> Shape
Shape::circle_filled(center, radius, color) -> Shape
```

### Color32

Stored internally as **premultiplied sRGBA** u8×4.

```rust
// Standard "paint-bucket" colors — use these in most cases
Color32::from_rgba_unmultiplied(r, g, b, a) -> Color32
// egui converts internally: stored_r = (r as u16 * a as u16 / 255) as u8
Color32::from_rgb(r, g, b) -> Color32          // a = 255
Color32::from_gray(l) -> Color32               // a = 255
Color32::from_black_alpha(a) -> Color32        // black with given opacity
Color32::from_white_alpha(a) -> Color32        // white with given opacity
Color32::from_rgba_premultiplied(r, g, b, a) -> Color32  // avoid; only if you pre-multiply yourself

// Accessors — r/g/b return PREMULTIPLIED values (< a unless opaque)
pub fn r(&self) -> u8
pub fn g(&self) -> u8
pub fn b(&self) -> u8
pub fn a(&self) -> u8

// Convert back to unpremultiplied (may have rounding errors for semi-transparent)
pub fn to_srgba_unmultiplied(&self) -> [u8; 4]
pub fn to_array(&self) -> [u8; 4]  // premultiplied

// Common constants
Color32::TRANSPARENT   // (0,0,0,0)
Color32::BLACK         // (0,0,0,255)
Color32::WHITE         // (255,255,255,255)
Color32::RED, GREEN, BLUE, YELLOW, etc.
Color32::PLACEHOLDER   // sentinel used in galleys for "use fallback color"
```

**Alpha pitfall:** When constructing `Color32` from user-facing RGBA (e.g., a color picker), always use `from_rgba_unmultiplied`. Using `from_rgba_premultiplied` with unmodified values produces incorrect blending for semi-transparent colors. Premultiplied alpha enables additive blending (set a=0 with r/g/b>0 for additive glow effects) and cleaner texture filtering.

### Stroke

```rust
pub struct Stroke {
    pub width: f32,
    pub color: Color32,
}
impl Stroke {
    pub const NONE: Stroke;
    pub fn new(width: impl Into<f32>, color: impl Into<Color32>) -> Self
    pub fn is_empty(&self) -> bool  // true if width==0 or color is transparent
}
```

### CornerRadius (replaces Rounding)

**FLAG — VERIFIED RENAME:** `Rounding` was renamed to `CornerRadius` in egui 0.29/0.30. In 0.34.2 `Rounding` is either removed or a deprecated alias. Always use `CornerRadius`.

```rust
pub struct CornerRadius {
    pub nw: u8,  // top-left, pixels
    pub ne: u8,  // top-right
    pub sw: u8,  // bottom-left
    pub se: u8,  // bottom-right
}
impl CornerRadius {
    pub const ZERO: CornerRadius;
    pub fn same(radius: u8) -> Self  // uniform rounding
}
// From<u8> and From<f32> implemented (f32 truncated to u8)
// So you can pass: CornerRadius::same(4), 0u8, 4.0f32
```

For pixel art: always use `CornerRadius::ZERO` on tile/canvas rects to avoid sub-pixel blurring.

### Mesh and Vertex

The primary way to draw the pixel canvas in software mode (before switching to wgpu callback):

```rust
pub struct Mesh {
    pub indices: Vec<u32>,    // triangle list (every 3 = one triangle)
    pub vertices: Vec<Vertex>,
    pub texture_id: TextureId,
}

pub struct Vertex {
    pub pos: Pos2,
    pub uv: Pos2,       // texture coordinates [0..1]
    pub color: Color32, // tint; use WHITE for unmodified texture
}
```

Build a textured quad (two triangles covering a rect):

```rust
fn textured_rect(rect: Rect, tex_id: TextureId) -> Mesh {
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    let mut mesh = Mesh { texture_id: tex_id, ..Default::default() };
    mesh.add_rect_with_uv(rect, uv, Color32::WHITE);
    mesh
}
painter.add(Shape::Mesh(Arc::new(textured_rect(canvas_rect, tex_id))));
```

`Mesh::add_rect_with_uv(rect, uv, color)` is a convenience method that appends 4 vertices and 6 indices.

### TextureId

```rust
pub enum TextureId {
    Managed(u64),  // allocated by ctx.load_texture() — do not construct directly
    User(u64),     // registered via Renderer::register_native_texture()
}
```

### Geometry types (emath)

```rust
Pos2 { x: f32, y: f32 }     pos2(x, y) — absolute screen position
Vec2 { x: f32, y: f32 }     vec2(x, y) — relative offset / size
Rect { min: Pos2, max: Pos2 }
    Rect::from_min_size(min: Pos2, size: Vec2)
    Rect::from_center_size(center: Pos2, size: Vec2)
    Rect::from_min_max(min: Pos2, max: Pos2)
    rect.width() / .height() / .size() / .center() / .contains(Pos2)
    rect.expand(f32) / .shrink(f32) / .translate(Vec2) / .intersect(Rect)
Align2 — used for text anchor:
    Align2::LEFT_TOP, LEFT_CENTER, LEFT_BOTTOM
    Align2::CENTER_TOP, CENTER_CENTER, CENTER_BOTTOM
    Align2::RIGHT_TOP, RIGHT_CENTER, RIGHT_BOTTOM
```

---

## 3. Textures (critical for pixel art)

### Lifecycle overview

```
CPU pixels -> ColorImage -> ctx.load_texture() -> TextureHandle (RAII)
                                                         |
                                              handle.id() -> TextureId
                                              (used in Mesh / painter.image)
```

### ColorImage

```rust
pub struct ColorImage {
    pub size: [usize; 2],     // [width, height]
    pub pixels: Vec<Color32>, // row-major, premultiplied
}

// Constructors
pub fn from_rgba_unmultiplied(size: [usize; 2], rgba: &[u8]) -> ColorImage
// rgba.len() must be size[0] * size[1] * 4; input bytes are straight alpha

pub fn from_rgb(size: [usize; 2], rgb: &[u8]) -> ColorImage
// rgb.len() must be size[0] * size[1] * 3; produces opaque pixels

pub fn new(size: [usize; 2], pixels: Vec<Color32>) -> ColorImage
// pixels.len() must be size[0] * size[1]; pre-built Color32 slice

pub fn filled(size: [usize; 2], color: Color32) -> ColorImage
// all pixels set to color

pub fn example() -> ColorImage  // small test image
```

**Pixel art note:** input to `from_rgba_unmultiplied` should be standard painter-style RGBA; egui premultiplies internally. Do not premultiply before passing to this function.

### TextureOptions

```rust
pub struct TextureOptions {
    pub magnification: TextureFilter,
    pub minification:  TextureFilter,
    pub wrap_mode:     TextureWrapMode,
    pub mipmap_mode:   Option<TextureFilter>,
}

pub enum TextureFilter {
    Nearest,  // no interpolation — correct for pixel art
    Linear,   // bilinear interpolation — blurs pixels
}

// FLAG: TextureWrapMode variants not confirmed from docs.rs; likely ClampToEdge | Repeat | MirroredRepeat
// The constants below are verified:
TextureOptions::NEAREST          // Nearest mag + min, default wrap
TextureOptions::LINEAR           // Linear mag + min (default)
TextureOptions::NEAREST_REPEAT
TextureOptions::NEAREST_MIRRORED_REPEAT
TextureOptions::LINEAR_REPEAT
TextureOptions::LINEAR_MIRRORED_REPEAT
```

For pixel art **always** use `TextureOptions::NEAREST`. Wrapping should be `ClampToEdge` (the default) to avoid bleeding at tile edges; the `NEAREST` constant uses this by default.

### Loading a texture (do this once, cache the handle)

```rust
struct CanvasState {
    texture: Option<egui::TextureHandle>,
}

impl CanvasState {
    fn ensure_texture(&mut self, ctx: &egui::Context, pixels: &[u8], w: usize, h: usize) {
        if self.texture.is_none() {
            let image = egui::ColorImage::from_rgba_unmultiplied([w, h], pixels);
            self.texture = Some(ctx.load_texture(
                "canvas",
                image,
                TextureOptions::NEAREST,
            ));
        }
    }
}
```

**WARNING:** `ctx.load_texture()` is NOT safe to call every frame. It allocates GPU memory each call. Call once, store the handle.

Signature:
```rust
pub fn load_texture(
    &self,
    name: impl Into<String>,
    image: impl Into<ImageData>,   // ColorImage, GrayImage, or FontImage
    options: TextureOptions,
) -> TextureHandle
```

`ImageData` is an enum: `Color(Arc<ColorImage>)` or `Font(Arc<FontImage>)`. `ColorImage` implements `Into<ImageData>` automatically.

### TextureHandle

```rust
pub fn id(&self) -> TextureId         // use this in Mesh / painter.image()
pub fn size(&self) -> [usize; 2]      // [width, height] in pixels
pub fn size_vec2(&self) -> Vec2       // same as Vec2
pub fn name(&self) -> String          // debug name

// Replace entire texture (must be same size or will panic/resize)
pub fn set(&mut self, image: impl Into<ImageData>, options: TextureOptions)

// Update a rectangular sub-region without re-uploading the whole texture
// pos: [x, y] top-left of the update region in texels
pub fn set_partial(&mut self, pos: [usize; 2], image: impl Into<ImageData>, options: TextureOptions)
```

RAII: texture freed when the last `TextureHandle` clone is dropped. Clone cheaply (`Arc<RwLock<TextureManager>>` internally).

### Incremental canvas update (critical for 8K canvas performance)

When only a dirty region changes, use `set_partial` to avoid uploading 256 MB of texture data every frame:

```rust
fn flush_dirty(&mut self, dirty: [usize; 4]) {
    // dirty = [x, y, w, h]
    let [x, y, w, h] = dirty;
    let sub_pixels: Vec<u8> = extract_subregion(&self.canvas_data, x, y, w, h);
    let sub_image = ColorImage::from_rgba_unmultiplied([w, h], &sub_pixels);
    if let Some(handle) = &mut self.texture {
        handle.set_partial([x, y], sub_image, TextureOptions::NEAREST);
    }
}
```

### Displaying a texture

Option A — convenience (auto-sizes to texture):
```rust
ui.image((handle.id(), handle.size_vec2()));
// or
ui.add(egui::Image::new(SizedTexture::from_handle(&handle)));
```

Option B — painter, full control over rect and UV:
```rust
painter.image(
    handle.id(),
    canvas_rect,
    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
    Color32::WHITE,
);
```

Option C — Mesh (when you need custom UV crops or per-vertex tinting):
```rust
let mut mesh = Mesh { texture_id: handle.id(), ..Default::default() };
mesh.add_rect_with_uv(canvas_rect, uv_rect, Color32::WHITE);
painter.add(Shape::Mesh(Arc::new(mesh)));
```

### SizedTexture

```rust
pub struct SizedTexture {
    pub id:   TextureId,
    pub size: Vec2,   // logical points, not pixels
}
impl SizedTexture {
    pub fn new(id: impl Into<TextureId>, size: impl Into<Vec2>) -> Self
    pub fn from_handle(handle: &TextureHandle) -> Self
}
```

### Registering an external wgpu texture as egui TextureId

If you render to a `wgpu::TextureView` in your custom render pass and want to display the result as an egui image:

```rust
// renderer is egui_wgpu::Renderer (from RenderState)
let tex_id: TextureId = renderer.register_native_texture(
    device,
    &texture_view,          // must be Rgba8Unorm format
    wgpu::FilterMode::Nearest,
);
// Update each frame after re-rendering:
renderer.update_native_texture(device, &texture_view, texture_filter, tex_id);
// Use tex_id with painter.image() or Mesh
```

FLAG: `update_native_texture` signature not confirmed — check Renderer docs directly if needed.

---

## 4. Custom GPU rendering — embedding wgpu inside egui

This is the make-or-break feature for Pixhaus. The egui_wgpu crate provides `CallbackTrait` + `Callback` to inject raw wgpu draw calls into egui's render pass.

### Architecture overview

```
UI code (Solid/Tauri)
    -> ui.allocate_painter(size, sense)
    -> painter.add(egui_wgpu::Callback::new_paint_callback(rect, MyRenderer))
    -> egui_wgpu::Renderer sees Shape::Callback during render
    -> calls MyRenderer::prepare() before the egui render pass
    -> calls MyRenderer::paint() inside the egui render pass
```

### CallbackTrait (egui_wgpu crate)

```rust
pub trait CallbackTrait: Send + Sync {
    // Called for all callbacks BEFORE the main egui render pass.
    // Use to write buffers, build command encoders for secondary passes.
    // The Vec<CommandBuffer> returned is submitted BEFORE the egui pass.
    fn prepare(
        &self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _screen_descriptor: &ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        _callback_resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        Vec::new()  // default: no-op
    }

    // Called after ALL prepare() calls finish.
    // CommandBuffers returned here are submitted AFTER those from prepare().
    fn finish_prepare(
        &self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _egui_encoder: &mut wgpu::CommandEncoder,
        _callback_resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        Vec::new()  // default: no-op
    }

    // REQUIRED. Called inside the egui render pass for issuing draw commands.
    // render_pass has 'static lifetime because egui_wgpu manages it via RenderPass<'static>
    fn paint(
        &self,
        info: PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &CallbackResources,
    );
}
```

### Callback (the Shape wrapper)

```rust
// Construct the callback shape (returns epaint::PaintCallback, which Into<Shape>)
pub fn new_paint_callback(
    rect: Rect,
    callback: impl CallbackTrait + 'static,
) -> epaint::PaintCallback

// Usage:
let cb = egui_wgpu::Callback::new_paint_callback(canvas_rect, my_canvas_renderer);
painter.add(cb);  // epaint::PaintCallback: Into<Shape> via Shape::Callback variant
```

The `rect` becomes `PaintCallbackInfo::viewport` and is used to set the wgpu render pass viewport/scissor before calling `paint()`. The backend restores previous state after.

### CallbackResources

```rust
pub type CallbackResources = TypeMap;
// TypeMap is a heterogeneous map keyed by TypeId.
```

Use it to store GPU resources (pipelines, bind groups, buffers) that are shared across prepare/paint without needing Arc/Mutex:

```rust
// Initialize once (e.g., in tauri setup or first prepare call):
callback_resources.insert(CanvasGpuResources {
    pipeline: render_pipeline,
    vertex_buffer,
    bind_group,
    uniform_buffer,
});

// In prepare/paint:
let res: &mut CanvasGpuResources = callback_resources
    .get_mut::<CanvasGpuResources>()
    .expect("CanvasGpuResources not initialized");
```

TypeMap methods (via anymap2 or equivalent):
```rust
fn insert<T: 'static + Send + Sync>(&mut self, val: T) -> Option<T>
fn get<T: 'static + Send + Sync>(&self) -> Option<&T>
fn get_mut<T: 'static + Send + Sync>(&mut self) -> Option<&mut T>
fn remove<T: 'static + Send + Sync>(&mut self) -> Option<T>
fn contains<T: 'static + Send + Sync>(&self) -> bool
```

FLAG: The concrete TypeMap implementation used internally is not publicly documented; the above methods are the standard TypeMap interface. Verify against `egui_wgpu::CallbackResources` docs if exact method names differ.

### ScreenDescriptor

```rust
pub struct ScreenDescriptor {
    pub size_in_pixels: [u32; 2],  // physical pixels [width, height]
    pub pixels_per_point: f32,     // HiDPI scale (e.g., 2.0 for Retina)
}
// Copy + Send + Sync
```

Logical size: `[size_in_pixels[0] as f32 / pixels_per_point, ...]`.

### RenderState (egui_wgpu)

```rust
pub struct RenderState {
    pub device:        wgpu::Device,
    pub queue:         wgpu::Queue,
    pub target_format: wgpu::TextureFormat,
    pub renderer:      Arc<RwLock<egui_wgpu::Renderer>>,
}
```

Access in Tauri:
```rust
// In your tauri command or setup closure:
let render_state: egui_wgpu::RenderState = app.state::<egui_wgpu::RenderState>().inner().clone();
// or via eframe/tauri-egui integration handles
```

To initialize resources in CallbackResources before the first frame (using Tauri + eframe-style integration):
```rust
// After wgpu device creation, before first render:
let mut renderer = render_state.renderer.write();
renderer.callback_resources.insert(CanvasGpuResources::new(
    &render_state.device,
    render_state.target_format,
));
```

### Canonical pattern — full canvas render pass

```rust
use egui_wgpu::{CallbackTrait, CallbackResources, ScreenDescriptor, Callback};

struct CanvasResources {
    pipeline:       wgpu::RenderPipeline,
    vertex_buf:     wgpu::Buffer,
    index_buf:      wgpu::Buffer,
    uniform_buf:    wgpu::Buffer,
    bind_group:     wgpu::BindGroup,
    dirty_uniforms: bool,
}

// Thin per-frame struct; cheap to construct each UI tick
struct CanvasCallback {
    transform: [[f32; 4]; 4],  // camera MVP
    tile_range: (u32, u32),
}

impl CallbackTrait for CanvasCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let res = resources.get_mut::<CanvasResources>().unwrap();
        // Write updated uniforms (camera, zoom, palette)
        queue.write_buffer(&res.uniform_buf, 0, bytemuck::cast_slice(&[self.transform]));
        Vec::new()
    }

    fn paint(
        &self,
        info: egui_wgpu::PaintCallbackInfo,
        rpass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        let res = resources.get::<CanvasResources>().unwrap();
        // viewport is already set by egui_wgpu to info.viewport_in_pixels()
        rpass.set_pipeline(&res.pipeline);
        rpass.set_bind_group(0, &res.bind_group, &[]);
        rpass.set_vertex_buffer(0, res.vertex_buf.slice(..));
        rpass.set_index_buffer(res.index_buf.slice(..), wgpu::IndexFormat::Uint32);
        let (start, end) = self.tile_range;
        rpass.draw_indexed(start * 6..end * 6, 0, 0..1);
    }
}

// Inside the egui Ui closure each frame:
fn show_canvas(ui: &mut egui::Ui, state: &CanvasState) {
    let (response, painter) = ui.allocate_painter(
        ui.available_size(),
        egui::Sense::click_and_drag(),
    );
    let canvas_rect = response.rect;

    let cb = Callback::new_paint_callback(
        canvas_rect,
        CanvasCallback {
            transform: state.camera.mvp(),
            tile_range: state.visible_tile_range(canvas_rect),
        },
    );
    painter.add(cb);
}
```

### PaintCallbackInfo

Available in `paint()`. Key methods:
```rust
// The rect passed to new_paint_callback(), in logical points
info.viewport: Rect

// The viewport in physical pixels (use for wgpu viewport commands if needed)
// FLAG: exact method name may be viewport_in_pixels() — verify at runtime
info.clip_rect: Rect    // the active scissor region
```

The backend sets the wgpu viewport/scissor to `info.viewport` before calling `paint()`. You should NOT call `rpass.set_viewport()` yourself unless you intentionally override it.

### Resource initialization timing

Resources must be in `CallbackResources` before `prepare()` is first called. Two patterns:

Pattern 1 — lazy init in `prepare()`:
```rust
fn prepare(&self, device: &wgpu::Device, queue: &wgpu::Queue, ..., resources: &mut CallbackResources) -> Vec<wgpu::CommandBuffer> {
    if !resources.contains::<CanvasResources>() {
        resources.insert(CanvasResources::new(device));
    }
    // ...
}
```

Pattern 2 — explicit init at app startup (preferred for large pipelines):
```rust
// In eframe App::new() or tauri setup:
let wgpu_state = cc.wgpu_render_state.as_ref().unwrap();
wgpu_state.renderer.write().callback_resources.insert(
    CanvasResources::new(&wgpu_state.device, wgpu_state.target_format),
);
```

### Renderer::register_native_texture (alternate approach for read-back)

If you do the wgpu rendering to your own `wgpu::Texture` and then want to show it as a software egui image (simpler but requires GPU readback):

```rust
let tex_id = renderer.register_native_texture(
    device,
    &canvas_texture_view,   // wgpu::TextureView, format must be Rgba8Unorm
    wgpu::FilterMode::Nearest,
);
// Draw as egui image:
painter.image(tex_id, canvas_rect, Rect::from_min_max(pos2(0.,0.), pos2(1.,1.)), Color32::WHITE);
```

This avoids implementing `CallbackTrait` but means the canvas texture is sampled by egui's own shader. Good for a "blit" pattern where the wgpu pass writes to a texture and egui displays it.

---

## 5. Text and fonts

### FontId

```rust
pub struct FontId {
    pub size:   f32,       // points (logical, not physical pixels)
    pub family: FontFamily,
}
impl FontId {
    pub fn new(size: f32, family: FontFamily) -> Self
    pub fn proportional(size: f32) -> Self   // FontFamily::Proportional
    pub fn monospace(size: f32) -> Self      // FontFamily::Monospace
}
```

### FontFamily

```rust
pub enum FontFamily {
    Proportional,          // variable-width (default body text)
    Monospace,             // fixed-width
    Name(Arc<str>),        // custom: FontFamily::Name("my_font".into())
}
```

### TextStyle

Predefined semantic styles (resolve to `FontId` via `Style`):
- `TextStyle::Small`
- `TextStyle::Body` (default)
- `TextStyle::Monospace`
- `TextStyle::Button`
- `TextStyle::Heading`
- `TextStyle::Name(Arc<str>)` — custom named style

Convert to FontId: `ui.style().text_styles[&TextStyle::Body].clone()`.

### RichText — builder for styled widget text

```rust
RichText::new("text") -> RichText
    .size(f32)
    .color(impl Into<Color32>)
    .background_color(impl Into<Color32>)
    .font(FontId)
    .text_style(TextStyle)
    .strong()          // bolder/brighter
    .weak()            // fainter
    .monospace()
    .code()            // monospace + gray background
    .underline()
    .strikethrough()
    .italics()
    .heading()         // large
    .small()           // smaller
    .small_raised()    // small + superscript
    .raised()          // superscript
```

`RichText` implements `Into<WidgetText>` so it passes to `ui.label()`, `ui.button()`, etc.

### ctx.fonts — low-level font access

```rust
// Font data is only valid after the first Context::run() call (pixels_per_point is unknown before)
ctx.fonts(|fonts: &FontsView<'_>| -> R {
    // fonts is a temporary read-guard
    let galley: Arc<Galley> = fonts.layout_no_wrap(
        "my text".into(),
        FontId::proportional(14.0),
        Color32::WHITE,
    );
    // or with wrapping:
    let galley2: Arc<Galley> = fonts.layout(
        "wrap me".into(),
        FontId::proportional(14.0),
        Color32::WHITE,
        200.0,  // wrap_width in points
    );
    galley
})
```

### Painter text methods

```rust
// Simple one-shot: lay out and draw in one call, returns bounding rect
let text_rect: Rect = painter.text(
    pos,
    Align2::LEFT_TOP,
    "Canvas: 1024x1024",
    FontId::monospace(12.0),
    Color32::WHITE,
);

// Two-step: layout then draw (reuse galley for hit-testing, measuring, etc.)
let galley: Arc<Galley> = painter.layout_no_wrap(
    "Layer 1".into(),
    FontId::proportional(13.0),
    Color32::from_rgba_unmultiplied(200, 200, 200, 255),
);
painter.galley(layer_name_pos, galley, Color32::WHITE);  // fallback_color unused if no PLACEHOLDER colors

// Override all colors in galley:
painter.galley_with_override_text_color(pos, galley, Color32::RED);
```

### Custom fonts

```rust
let mut fonts = egui::FontDefinitions::default();
fonts.font_data.insert(
    "pixel_font".to_owned(),
    std::sync::Arc::new(egui::FontData::from_static(
        include_bytes!("../assets/PixelFont.ttf"),
    )),
);
// Set as highest-priority proportional font:
fonts.families
    .get_mut(&egui::FontFamily::Proportional)
    .unwrap()
    .insert(0, "pixel_font".to_owned());
ctx.set_fonts(fonts);
```

---

## 6. Pitfalls and idioms for pixel art editors

| Concern | Rule |
|---|---|
| Texture filtering | Always `TextureOptions::NEAREST`; never `LINEAR` for the pixel canvas |
| Texture wrap | Use default (ClampToEdge) to prevent edge bleeding on tiles |
| Texture upload cost | `set_partial` dirty-region updates only; never full re-upload per frame |
| `load_texture` timing | Call once; cache `TextureHandle`; never call in immediate-mode hot path |
| Alpha construction | `Color32::from_rgba_unmultiplied` for user-facing colors; `from_rgba_premultiplied` only if you manually premultiplied |
| CornerRadius on canvas rects | Always `CornerRadius::ZERO` to prevent sub-pixel blurring at tile boundaries |
| wgpu callback state | Store GPU resources in `CallbackResources` type-map; initialize at startup not per-frame |
| RenderPass lifetime | `paint()` receives `&mut wgpu::RenderPass<'static>` — egui_wgpu owns the pass; do not end it or begin sub-passes |
| Viewport in paint() | egui_wgpu already sets viewport/scissor to the callback rect; only call `set_viewport` if you need a sub-viewport |
| Font access timing | `ctx.fonts(|f| ...)` is invalid before first `ctx.run()` frame |
| Text recreation | `Shape::Text` / `TextShape` stores a `Galley` which references `pixels_per_point`; recreate galleys when dpi changes |
| `Rounding` rename | In egui 0.34.2, use `CornerRadius` not `Rounding`; `Rounding` may compile as deprecated alias but will warn |

---

## 7. Dependency declaration

```toml
[dependencies]
egui       = "0.34"
egui-wgpu  = { version = "0.34", features = ["winit"] }
wgpu       = "29"          # egui-wgpu 0.34 uses wgpu 29.x
eframe     = { version = "0.34", features = ["wgpu"] }  # if using eframe
```

For Tauri + egui (non-eframe):
```toml
tauri-egui = "..."  # check community crate version for egui 0.34 compat
# or integrate egui_wgpu::Renderer manually into your Tauri wgpu surface
```

---

## 8. Unverified / flagged items

- **`TextureWrapMode` variants**: Existence confirmed (it's a field on `TextureOptions`), but exact variant names (`ClampToEdge`, `Repeat`, `MirroredRepeat`) not confirmed from docs.rs 0.34.2 pages — inferred from convention.
- **`CallbackResources` TypeMap exact API**: Confirmed as `TypeMap` alias; specific method names (`get`, `get_mut`, `insert`, `remove`, `contains`) are standard TypeMap interface but not verified against the exact crate version.
- **`PaintCallbackInfo::viewport_in_pixels()`**: Method name not confirmed; `info.viewport` (Rect in points) is confirmed. Physical pixel conversion may require manual multiplication by `pixels_per_point`.
- **`Renderer::update_native_texture()`**: Existence not confirmed; `register_native_texture` and `register_native_texture_with_sampler_options` are confirmed.
- **`Rounding` deprecation path**: Confirmed `CornerRadius` exists in 0.34.2 with verified signatures. Whether `Rounding` is fully removed or still a deprecated re-export was not directly confirmed — treat `CornerRadius` as the only correct type.
- **`Mesh::add_rect_with_uv()`**: Method exists on `Mesh` in epaint; signature not extracted directly — standard usage is `mesh.add_rect_with_uv(rect, uv, color)`.
- **eframe vs Tauri integration path for `CallbackResources` initialization**: The `cc.wgpu_render_state` access path shown is eframe-specific. Tauri integration varies by crate used.
