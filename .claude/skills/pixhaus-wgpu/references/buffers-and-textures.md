# wgpu 29.0.1 — buffers, textures, samplers, data upload

Signatures verified against docs.rs/wgpu/29.0.1. No `unsafe`; pixel data is `Vec<u8>` cast
to `&[u8]` via `bytemuck`.

## v29 renamed the copy types — use the `TexelCopy*` family

| Role | v29 name (use this) | Old name (do NOT use) |
|------|---------------------|------------------------|
| `write_texture` / copy destination | `TexelCopyTextureInfo<'a>` | `ImageCopyTexture` |
| data layout | `TexelCopyBufferLayout` | `ImageDataLayout` |
| buffer copy endpoint | `TexelCopyBufferInfo<'a>` | `ImageCopyBuffer` |

```rust
pub type TexelCopyTextureInfo<'a> = TexelCopyTextureInfoBase<&'a Texture>;
// fields: texture: &Texture, mip_level: u32, origin: Origin3d, aspect: TextureAspect
```

## Load-bearing signatures

```rust
// Queue — staged uploads, flushed at the next submit.
pub fn write_buffer(&self, buffer: &Buffer, offset: BufferAddress, data: &[u8])
pub fn write_texture(
    &self,
    texture: TexelCopyTextureInfo<'_>,
    data: &[u8],
    data_layout: TexelCopyBufferLayout,
    size: Extent3d,
)

// util::DeviceExt — create + fill in one call (import wgpu::util::DeviceExt)
fn create_buffer_init(&self, desc: &BufferInitDescriptor<'_>) -> Buffer;
fn create_texture_with_data(&self, queue: &Queue, desc: &TextureDescriptor<'_>,
                            order: TextureDataOrder, data: &[u8]) -> Texture;

// Texture
pub fn create_view(&self, desc: &TextureViewDescriptor<'_>) -> TextureView
pub fn as_image_copy(&self) -> TexelCopyTextureInfo<'_>  // whole-texture dest helper
pub fn width(&self) -> u32; pub fn height(&self) -> u32; pub fn size(&self) -> Extent3d

// Buffer mapping (readback only — never the paint hot path)
pub fn slice<S: RangeBounds<BufferAddress>>(&self, bounds: S) -> BufferSlice<'_>
// On BufferSlice:
pub fn map_async(&self, mode: MapMode,
    callback: impl FnOnce(Result<(), BufferAsyncError>) + WasmNotSend + 'static)
pub fn get_mapped_range(&self) -> BufferView
pub fn get_mapped_range_mut(&self) -> BufferViewMut
// Buffer::unmap(&self)
```

## Descriptor field layouts

```rust
// TextureDescriptor<'a>
label: Label<'a>, size: Extent3d, mip_level_count: u32, sample_count: u32,
dimension: TextureDimension, format: TextureFormat, usage: TextureUsages,
view_formats: &'a [TextureFormat]

// TextureViewDescriptor<'a> — ::default() gives a full-texture 2D view
label, format: Option<TextureFormat>, dimension: Option<TextureViewDimension>,
usage: Option<TextureUsages>, aspect: TextureAspect, base_mip_level: u32,
mip_level_count: Option<u32>, base_array_layer: u32, array_layer_count: Option<u32>

// SamplerDescriptor<'a>
label, address_mode_u/v/w: AddressMode, mag_filter: FilterMode, min_filter: FilterMode,
mipmap_filter: FilterMode, lod_min_clamp: f32, lod_max_clamp: f32,
compare: Option<CompareFunction>, anisotropy_clamp: u16, border_color: Option<SamplerBorderColor>

// util::BufferInitDescriptor<'a> { label: Label<'a>, contents: &'a [u8], usage: BufferUsages }
// TexelCopyBufferLayout { offset: u64, bytes_per_row: Option<u32>, rows_per_image: Option<u32> }
// Extent3d { width: u32, height: u32, depth_or_array_layers: u32 }
// Origin3d { x: u32, y: u32, z: u32 }   // const Origin3d::ZERO
```

## Enums

- `TextureFormat`: `Rgba8Unorm` (linear), `Rgba8UnormSrgb` (sRGB), `Bgra8Unorm`,
  `Bgra8UnormSrgb`. For an authored pixel-art canvas where stored bytes are the literal
  colors, prefer **`Rgba8Unorm`** so the GPU does not re-encode bytes on sample; pick a
  `*Srgb` variant only if you want hardware sRGB→linear. The surface format egui-wgpu hands
  you is often `Bgra8UnormSrgb` — your pipeline's color-target format must match it, so
  convert deliberately rather than assuming.
- `FilterMode`: `Nearest`, `Linear`. **Pixel art must use `Nearest`** for `mag_filter`,
  `min_filter`, and `mipmap_filter` — `Linear` blurs texels. This is the single most common
  pixel-art rendering bug.
- `AddressMode`: `ClampToEdge` (default for a single canvas), `Repeat`, `MirrorRepeat`,
  `ClampToBorder` (feature-gated).
- `TextureDimension`: `D1`/`D2`/`D3` — canvas is `D2`. `TextureAspect`: `All` for color.
- `MapMode`: `Read`, `Write`. `BufferUsages`: `MAP_READ`, `MAP_WRITE`, `COPY_SRC`,
  `COPY_DST`, `INDEX`, `VERTEX`, `UNIFORM`, `STORAGE`, `INDIRECT`. `TextureUsages`:
  `COPY_SRC`, `COPY_DST`, `TEXTURE_BINDING`, `STORAGE_BINDING`, `RENDER_ATTACHMENT`.

## The 256-byte row-alignment rule — and why dirty-rect uploads dodge it

```rust
pub const COPY_BYTES_PER_ROW_ALIGNMENT: u32 = 256;
```

This binds **buffer-backed copies** (`copy_buffer_to_texture` / `copy_texture_to_buffer`):
the buffer's `bytes_per_row` must be a multiple of 256.

`queue.write_texture` takes a plain `&[u8]`, not a GPU buffer, so wgpu stages it and the 256
rule does **not** bind your `bytes_per_row` — you pass the tight stride (`width * 4` for
RGBA8) and arbitrary widths just work. **This is exactly why the dirty-rect upload path uses
`queue.write_texture`, not a staging buffer + copy.** A 4097-px-wide dirty region uploads
with `bytes_per_row = Some(4097 * 4)` directly, no padding math.

You only meet the 256 rule on *readback* (export, `copy_texture_to_buffer`): pad each row up
to a 256-byte multiple, then strip the padding CPU-side.

```rust
let unpadded = width * 4;
let padded = unpadded.div_ceil(256) * 256; // readback buffer's bytes_per_row
```

## Worked example — canvas texture + nearest sampler + dirty-rect upload

```rust
use wgpu::*;

let canvas = device.create_texture(&TextureDescriptor {
    label: Some("canvas"),
    size: Extent3d { width: canvas_w, height: canvas_h, depth_or_array_layers: 1 },
    mip_level_count: 1, sample_count: 1,
    dimension: TextureDimension::D2,
    format: TextureFormat::Rgba8Unorm,                  // linear: bytes == sampled
    usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
    view_formats: &[],
});
let canvas_view = canvas.create_view(&TextureViewDescriptor::default());

let sampler = device.create_sampler(&SamplerDescriptor {
    label: Some("nearest"),
    address_mode_u: AddressMode::ClampToEdge,
    address_mode_v: AddressMode::ClampToEdge,
    address_mode_w: AddressMode::ClampToEdge,
    mag_filter: FilterMode::Nearest,
    min_filter: FilterMode::Nearest,
    mipmap_filter: FilterMode::Nearest,
    ..Default::default()
});

// After a brush stroke: upload ONLY the dirty sub-rectangle. Work is bounded by the dirty
// region, never by canvas size — this is the 8K perf path (see the 8k-perf-constraint memory).
// `dirty_pixels` is a tight RGBA8 Vec<u8>, dirty_w * dirty_h * 4 bytes, rows top-to-bottom.
queue.write_texture(
    TexelCopyTextureInfo {
        texture: &canvas,
        mip_level: 0,
        origin: Origin3d { x: dirty_x, y: dirty_y, z: 0 },
        aspect: TextureAspect::All,
    },
    &dirty_pixels,                                      // &[u8] — no GPU buffer, no 256 rule
    TexelCopyBufferLayout {
        offset: 0,
        bytes_per_row: Some(dirty_w * 4),               // tight stride; any width works
        rows_per_image: Some(dirty_h),                  // required when height > 1
    },
    Extent3d { width: dirty_w, height: dirty_h, depth_or_array_layers: 1 },
);
```

If `dirty_pixels` is a strided slice of a larger backing buffer, pass the full row stride in
`bytes_per_row` and an `offset` to the rect's first row — `write_texture` walks
`bytes_per_row` per row and ignores trailing bytes beyond `width * 4`.

## Worked example — vertex / index / uniform buffers

```rust
use wgpu::util::{DeviceExt, BufferInitDescriptor};
use bytemuck::{Pod, Zeroable, cast_slice};

#[repr(C)] #[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex { pos: [f32; 2], uv: [f32; 2] }

let vbuf = device.create_buffer_init(&BufferInitDescriptor {
    label: Some("quad-vertices"),
    contents: cast_slice(&vertices),                    // &[Vertex] -> &[u8], no unsafe
    usage: BufferUsages::VERTEX,
});
let ibuf = device.create_buffer_init(&BufferInitDescriptor {
    label: Some("quad-indices"),
    contents: cast_slice(&indices),
    usage: BufferUsages::INDEX,
});
// Uniform buffer: COPY_DST so it can be rewritten each frame.
let ubuf = device.create_buffer_init(&BufferInitDescriptor {
    label: Some("uniforms"),
    contents: cast_slice(&[uniforms]),                  // single struct -> 1-elem slice
    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
});

// Per-frame uniform update — cheap, staged, no map/unmap:
queue.write_buffer(&ubuf, 0, cast_slice(&[updated_uniforms]));
```

## bytemuck + uniform layout

- Vertex/uniform structs: `#[repr(C)] #[derive(Clone, Copy, Pod, Zeroable)]`, then
  `bytemuck::cast_slice(&[t])` for `&[u8]`. Safe, no `unsafe`.
- Keep uniform structs 16-byte aligned (pad fields) to match WGSL's std140-ish layout — a
  `vec3` is 16-aligned, a trailing scalar needs padding. The pixhaus-glam skill covers
  packing `Mat4`/`Vec*` into uniform-safe byte layouts; reach for it rather than hand-rolling
  matrix math here.
- Pixel buffers are already `Vec<u8>` — they pass straight to `write_texture` / `contents`.

## Gotchas

- Uploads are queued, visible only after the next `submit`. Nothing to `await` for an upload;
  mapping is the async API and is used only for readback.
- Set `rows_per_image` whenever `height > 1`; `None` is for a single row.
- `Texture::as_image_copy()` keeps the old method name even in v29 and returns a
  whole-texture `TexelCopyTextureInfo`; for a dirty rect, build the struct yourself to set
  `origin`.
- The brush hot loop maps nothing — it only `write_texture`s the dirty rect.
