---
name: pixhaus-bytemuck
description: >
  Use when reinterpreting bytes in Pixhaus — deriving Pod/Zeroable for wgpu vertex,
  uniform, instance, or push-constant structs; turning typed data into the &[u8] that
  queue.write_buffer / create_buffer_init want; viewing a Vec<u8> pixel buffer as
  typed pixels or vice versa; or reading raw bytes out of a buffer. Trigger this for
  ANY "get the bytes of", "cast this slice", "upload to the GPU", "&[u8] for the
  buffer", "reinterpret these pixels", "Pod/Zeroable", or "it won't derive Pod /
  there are padding bytes" task, even when the user doesn't say "bytemuck". bytemuck
  is the safe byte-casting crate; in Pixhaus the workspace forbids unsafe, so the
  derive macros are the only way to mark a type castable — reach for this skill rather
  than hand-writing an unsafe impl (which will not compile) or guessing a signature.
---

# bytemuck for Pixhaus

bytemuck reinterprets one plain-data type as another — most importantly, any `T` as
`&[u8]` and back — without a copy and without you writing `unsafe`. In Pixhaus it sits on
exactly one seam: the boundary where typed Rust data (a vertex, a uniform block, a row of
pixels) becomes the flat byte slice that `wgpu` uploads to the GPU, and where bytes coming
back become typed data again. wgpu's buffer APIs (`queue.write_buffer`,
`wgpu::util::DeviceExt::create_buffer_init`) take `&[u8]`; your data is structs and
`Vec<u8>`; bytemuck is the bridge that stays on the CPU side and costs nothing at runtime.

This skill is the floor for byte-casting work. The mental model that prevents the
recurring bugs, the one Pixhaus-specific rule that overrides everything you remember about
bytemuck, the everyday derive-and-cast API, and how it maps onto the renderer. When you
need an exact signature or the full trait/function surface, open the matching file in
`references/` — derived from docs.rs 1.25.0 and load-bearing calls verified against source.

## The one rule that is different in Pixhaus

The workspace sets `unsafe_code = "forbid"` (`Cargo.toml`, `[workspace.lints.rust]`).
bytemuck's traits are `unsafe trait`s, so the way you'd reach for first — hand-writing
`unsafe impl Pod for Vertex {}` — **does not compile here**. `forbid` cannot be locally
overridden; the compiler rejects it with "implementation of an `unsafe` trait ... requested
... with `-F unsafe-code`".

The derive macros are the way through, and they are not a workaround — they are better.
`#[derive(Pod, Zeroable)]` generates the same `unsafe impl` inside the `bytemuck_derive`
proc-macro crate, where proc-macro hygiene keeps the generated `unsafe` token from tripping
the lint, **and** it generates compile-time assertions that the type actually upholds the
contract (right `repr`, every field also `Pod`, no padding). Hand-written `unsafe impl`
asserts the contract on your honor; the derive proves it. Both facts above are verified by
compiling against this workspace's lint config, not recalled.

```rust
// Right — compiles under forbid(unsafe_code), and the macro checks the layout for you.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex { pos: [f32; 2], uv: [f32; 2], color: [f32; 4] }

// Wrong here — hard compile error, regardless of whether the type is actually sound.
unsafe impl bytemuck::Pod for Vertex {}
```

So: **mark types castable with the derives, never with `unsafe impl`.** If a type can't be
derived (it has padding, a non-`Pod` field, or is generic over a non-`Pod` parameter under
`#[repr(C)]`), the answer is to fix the type — add explicit padding fields, swap the field,
make it `#[repr(transparent)]` — not to reach for unsafe. See `references/derives.md`.

## Versions

bytemuck is post-1.0 and stable; it does not move in lockstep with the egui/wgpu stack.

| Crate | Version |
|---|---|
| `bytemuck` | 1.25 (`bytemuck_derive` 1.10 pulled in by the `derive` feature) |

```toml
# In the crate that talks to wgpu (render/), not the workspace backbone.
bytemuck = { version = "1.25", features = ["derive", "extern_crate_alloc"] }
```

- `derive` — the macros. You always want this in Pixhaus; it's the only legal path (above).
- `extern_crate_alloc` — unlocks the `allocation` module (`zeroed_vec`, `cast_vec`,
  `cast_slice_box`, …). Worth having: pixel buffers are `Vec<u8>` and a zeroed canvas of
  typed pixels is a natural `zeroed_vec`.
- Leave the SIMD/atomic/`must_cast`/`const_zeroed` features off unless a specific need
  appears; `references/alloc-and-checked.md` notes what each unlocks.

If you hold `glam` types (`Vec3`, `Mat4`, …) in a GPU struct, enable glam's own `bytemuck`
feature so those types are `Pod`/`Zeroable` — see the `pixhaus-glam` skill.

## The mental model: bit patterns, two directions

Every bytemuck trait is a claim about bit patterns. Two questions decide which trait a
type can have, and they are independent:

1. **Can you read its bytes out safely?** Yes when the type has **no padding/uninit
   bytes** — there are no undefined bytes to leak. That is `NoUninit`. This is the
   **write side**: `&T` → `&[u8]`, `bytes_of`, casting an immutable reference *out*.
2. **Can you build it from arbitrary bytes safely?** Yes when **every bit pattern is a
   valid value** — no illegal patterns like a `bool` that isn't 0/1 or a `char` that isn't
   a scalar value. That is `AnyBitPattern`. This is the **read side**: `&[u8]` → `&T`,
   `from_bytes`, filling a value *in*.

```
        NoUninit  (no padding)            AnyBitPattern  (any bits valid)
        WRITE side: &T -> &[u8]           READ side: &[u8] -> &T
                 \                          /
                  \                        /
                   \                      /
                    Pod  =  both at once  (also Copy + 'static)
                   reinterpret freely, including &mut T -> &mut U
```

- **`Pod`** ("plain old data") is the everyday trait: it is `NoUninit + AnyBitPattern`
  (plus `Copy + 'static`), so it casts in both directions, including mutable references.
  Your vertex/uniform/instance structs want `Pod`.
- **`Zeroable`** is weaker and orthogonal: "all-zero bytes is a valid value." Every `Pod`
  type is `Zeroable`, but plenty of non-`Pod` types are too. wgpu and bytemuck's
  allocation helpers ask for it (`zeroed`, `zeroed_vec`).
- **`CheckedBitPattern`** is the escape hatch for types with *some* illegal patterns
  (`bool`, `char`, fieldless enums): casting validates at runtime via
  `bytemuck::checked::*` and fails instead of inviting UB. Don't put a bare `enum` in a
  GPU struct and derive `Pod` — derive `CheckedBitPattern`, or use a plain integer field.

Full hierarchy, exact safety contracts, and the niche traits (`Contiguous`,
`TransparentWrapper`, `PodInOption`) are in `references/traits.md`.

## The everyday API

Two derives and three or four functions cover almost all Pixhaus use.

```rust
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Uniforms { view_proj: [[f32; 4]; 4], canvas_size: [f32; 2], _pad: [f32; 2] }

// typed -> bytes (write side). The slice borrows the data; no copy.
let verts: &[Vertex] = &mesh;
let bytes: &[u8] = bytemuck::cast_slice(verts);
queue.write_buffer(&vbuf, 0, bytes);

let u = Uniforms { /* … */ };
queue.write_buffer(&ubuf, 0, bytemuck::bytes_of(&u));   // single value -> &[u8]

// bytes -> typed (read side).
let px: &[Rgba8] = bytemuck::cast_slice(&pixel_bytes);  // &[u8] -> &[Rgba8]
let header: &Header = bytemuck::from_bytes(&buf[..size_of::<Header>()]);
```

- `cast_slice` / `cast_slice_mut` — `&[T]` ↔ `&[U]`, no copy, length recomputed from byte
  span. The slice-level workhorse for GPU upload and pixel views.
- `bytes_of` / `bytes_of_mut` — a single `&T` ↔ `&[u8]`. For one uniform block.
- `from_bytes` / `cast` — bytes → `&T` / value → value.
- Prefer the plain functions for code you control; they **panic** on a size or alignment
  mismatch, which during development is a loud, correct failure. Reach for the `try_*`
  variants (returning `Result<_, PodCastError>`) only when the bytes come from outside and
  a mismatch is an expected, recoverable case — then handle it with `thiserror`, per
  `pixhaus-rust-conventions`. Exact panic-vs-error conditions per function are in
  `references/functions.md`.

## Pixhaus applications

- **wgpu buffer structs.** Vertices, instances, uniforms, push constants: `#[repr(C)]` +
  `#[derive(Copy, Clone, Pod, Zeroable)]`, then `cast_slice`/`bytes_of` to upload. The
  renderer stays UI-agnostic (per the repo layout), and bytemuck is a clean fit there —
  no egui or app types involved.
- **The uniform padding trap.** WGSL/std140 layout aligns `vec3` and larger to 16 bytes,
  so a uniform struct often *needs* trailing or interior padding to match the shader. But
  `#[derive(Pod)]` **rejects implicit padding**. Resolve both at once by adding *explicit*
  padding fields (`_pad: [f32; 2]`, `_pad: u32`) so the layout matches the shader and the
  struct has no implicit padding. This is the single most common "won't derive Pod" cause.
  Details and a worked example in `references/wgpu-and-pixhaus.md`.
- **Pixel buffers.** Pixel data is `Vec<u8>` with explicit stride (per the repo memory
  rule). `cast_slice::<u8, Rgba8>` views it as typed pixels for blend/transform code;
  `cast_slice::<Rgba8, u8>` (or `bytes_of`) views it back as bytes for texture upload. A
  fresh transparent canvas of `N` pixels is `bytemuck::zeroed_vec::<Rgba8>(n)` (needs
  `extern_crate_alloc`). Zero-copy matters at 8K — never round-trip pixels through an owned
  copy just to change their type. See `references/wgpu-and-pixhaus.md`.
- **Not for the file format.** bytemuck does *raw, native-endian* byte reinterpretation; it
  performs no endianness conversion and encodes no schema. The `.pixhaus` format is
  MessagePack + zstd (`rmp-serde`), which handles endianness and versioning. Do not
  `bytes_of` a struct to disk — that file would be byte-order- and layout-dependent. Keep
  bytemuck on the in-memory GPU seam; keep serde on the I/O seam.

## Rules that prevent the recurring bugs

- **Alignment is checked, and reference casts can panic on it.** `cast_slice`/`from_bytes`
  reinterpret in place, so the source bytes must already be aligned for the target type;
  a `&[u8]` from an arbitrary offset is only 1-aligned and casting it to `&[u32]` panics
  with `AlignmentMismatch`. When you must read a value out of bytes that may be misaligned,
  use `pod_read_unaligned` (it copies, so only size matters). `Vec<u8>`/`Box<[u8]>` data is
  max-aligned at the start, so slicing from offset 0 is fine.
- **Slop is checked too.** Casting `&[u8]` to `&[T]` fails (`OutputSliceWouldHaveSlop`) if
  the byte length isn't a whole multiple of `size_of::<T>()`. Slice exactly, or size the
  buffer to the type.
- **`bool`/`char`/bare enums are not `Pod`.** They have illegal bit patterns. Don't derive
  `Pod` for a struct containing them; use a `u32` flag field, or `CheckedBitPattern`.
- **A `&mut` cast needs both directions.** `cast_slice_mut` requires the target be both
  readable-from-any-bits and writable-without-leaking — in practice both types `Pod`.
- **Fix the type, don't go unsafe.** Every derive failure names a real layout problem.
  Adding `#[repr(C)]`, an explicit `_pad` field, or swapping a field is the fix. `unsafe
  impl` is not available (it won't compile) and wouldn't make the underlying layout sound
  anyway.

## References

Open the file for what you're doing; each is a dense, version-pinned API reference.

| File | Covers |
|---|---|
| `references/traits.md` | Every marker trait, exact safety contracts, the read/write hierarchy, std implementors, `Contiguous`/`TransparentWrapper`/`*InOption` |
| `references/derives.md` | All 9 derive macros: required `repr`, field bounds, helper attributes, compile-failure conditions, the `forbid(unsafe_code)` interaction |
| `references/functions.md` | Every top-level cast/zero function with exact signature, panic vs `try_*` error, and the `PodCastError` variants |
| `references/alloc-and-checked.md` | `allocation` (Box/Vec/Rc/Arc, `zeroed_*`), `checked` (runtime bit-pattern validation), `must_cast`, `offset_of!`, error enums, feature gates |
| `references/wgpu-and-pixhaus.md` | Vertex/uniform/instance/pixel patterns end to end, the std140 padding trap, glam interop, the IO boundary |

A standing caution: the references record the 1.25.0 API faithfully and the load-bearing
signatures were checked against source, but if a deep signature is load-bearing for what
you're building, confirm it with `cargo doc -p bytemuck --open` once the crate is vendored.
