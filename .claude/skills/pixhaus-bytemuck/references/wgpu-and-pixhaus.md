# bytemuck in the Pixhaus renderer

The applied patterns. For the wgpu APIs themselves (buffer creation, `write_buffer`,
bind groups, pipelines) use the `pixhaus-wgpu` skill; for the vector/matrix math that fills
these structs use `pixhaus-glam`; for the `.pixhaus` file format use `pixhaus-rmp-serde`.
This file is only the byte-casting seam between them.

## Vertex / instance buffers

A vertex or instance struct is `#[repr(C)]` + `#[derive(Copy, Clone, Pod, Zeroable)]`, then
`cast_slice` to the `&[u8]` the buffer wants.

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CanvasVertex { pos: [f32; 2], uv: [f32; 2] }   // 16 bytes, no padding

let verts: &[CanvasVertex] = &quad;
queue.write_buffer(&vertex_buf, 0, bytemuck::cast_slice(verts));
// or at creation: device.create_buffer_init(&BufferInitDescriptor {
//     contents: bytemuck::cast_slice(verts), usage: VERTEX, .. })
```

Keep these structs free of `bool`, `char`, bare enums, and pointers — none are `Pod`. For a
discrete flag use a `u32`; for an enum use a `#[repr(u32)]` integer field and convert with a
`Contiguous` enum (see traits.md), keeping the GPU struct itself all-`Pod`.

## Uniform buffers and the std140 padding trap

This is the failure you'll hit most. WGSL uniform layout aligns `vec3`/`vec4`/`mat` members
to 16 bytes, so a uniform struct usually needs padding to match the shader. But
`#[derive(Pod)]` **rejects implicit padding** — the compiler error and the shader layout
requirement point the same way: make the padding **explicit**.

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CanvasUniforms {
    view_proj: [[f32; 4]; 4], // 64 B, 16-aligned
    canvas_size: [f32; 2],    // 8 B
    zoom: f32,                // 4 B
    _pad: f32,                // 4 B explicit pad -> total 80 B, 16-aligned, no implicit padding
}

let u = CanvasUniforms { /* fill from glam, see pixhaus-glam */ };
queue.write_buffer(&uniform_buf, 0, bytemuck::bytes_of(&u));
```

Rules of thumb:
- Lay fields out large-to-small and pad the tail to a multiple of 16.
- A lone `vec3` in a uniform is a trap — it's 12 bytes in Rust but the shader wants it
  16-aligned. Use `[f32; 4]` (ignore the 4th component) or follow it with explicit padding.
- Initialize via `..Zeroable::zeroed()` if you want padding fields zeroed without naming
  them, but they must still exist as fields for the derive to accept the struct.
- If the struct holds glam types (`Mat4`, `Vec4`), enable glam's `bytemuck` feature so they
  are `Pod`; glam's 16-byte-aligned types (`Vec4`, `Mat4`, `Affine3A`) help the layout
  line up. See `pixhaus-glam`.

## Pixel buffers ↔ bytes

Pixel data is `Vec<u8>` with explicit stride (the repo memory rule). bytemuck gives you
zero-copy typed views in both directions:

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Rgba8 { r: u8, g: u8, b: u8, a: u8 }   // 4 bytes, no padding

let pixels: &[Rgba8]      = bytemuck::cast_slice(&buf);          // &[u8]   -> &[Rgba8]
let pixels_mut: &mut [Rgba8] = bytemuck::cast_slice_mut(&mut buf);
let back: &[u8]           = bytemuck::cast_slice(pixels);        // &[Rgba8] -> &[u8] for upload
queue.write_texture(/* .. */, bytemuck::cast_slice(pixels), /* layout .. */);
```

- The `&[u8] -> &[Rgba8]` cast needs `buf.len() % 4 == 0` (else `OutputSliceWouldHaveSlop`)
  and 4-byte alignment — guaranteed at the start of a `Vec<u8>`/`Box<[u8]>`, so view from
  offset 0, not from an arbitrary mid-buffer slice.
- This is the 8K-perf seam: at 8192×8192 a full RGBA buffer is 256 MB. Casting is a
  pointer reinterpret — **O(1)**. Never `.to_vec()` or map-collect pixels just to change
  their type; that copy is the thing the native rewrite exists to avoid. Bound work to the
  dirty region and cast in place.
- To *own* typed pixels, allocate `Vec<Rgba8>` (or `bytemuck::zeroed_vec::<Rgba8>(w*h)` for
  a transparent canvas) from the start — a `Vec<u8>` can't be `cast_vec`'d to `Vec<Rgba8>`
  because its allocation is only 1-aligned (see alloc-and-checked.md).

## The IO boundary — do not use bytemuck for the file format

bytemuck reinterprets raw, **native-endian** bytes and encodes no schema or version. The
`.pixhaus` format is MessagePack + zstd (`rmp-serde`), which is portable and versioned.

- Persisting a struct with `bytes_of` to disk would produce a file whose meaning depends on
  the writer's endianness and struct layout — unreadable on another target or after a field
  reorder. Don't.
- Keep bytemuck on the **in-memory → GPU** seam (this file). Keep `serde`/`rmp-serde` on the
  **in-memory ↔ disk** seam (`pixhaus-rmp-serde`). A type can derive both `Pod` and
  `Serialize`/`Deserialize`; use the bytemuck side for upload and the serde side for saving,
  never bytemuck for saving.
- The one safe cross-process use of raw bytes is the GPU upload itself, where "the other
  side" is your own shader reading the same native layout — which is exactly what the
  `#[repr(C)]` + matching WGSL struct contract guarantees.
