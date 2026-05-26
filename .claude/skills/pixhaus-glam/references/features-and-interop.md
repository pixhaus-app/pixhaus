# glam 0.33.0 — features, memory layout, and GPU interop

Reference for `glam` 0.33.0 as used in a `wgpu` + `bytemuck` + `serde` graphics
app. MSRV is **1.68.2**.

## Default feature set

The default features are `std` and `all-types`. `all-types` pulls in
`float-types`, `integer-types`, and `size-types`, so a default build has every
vector/matrix type for f32, f64, all integer widths, and isize/usize. There are
35 feature flags total; 16 are on by default.

To trim the build, set `default-features = false` and re-add only what you use
(e.g. `std`, `float-types` for f32+f64).

## Cargo features

| Group | Feature | Default | What it does |
|-------|---------|:-------:|--------------|
| core | `std` | yes | Standard library. No extra deps. Disable for `no_std`. |
| core | `nostd-libm` | no | `no_std` math via `libm` ^0.2. Use instead of `std` on bare-metal. |
| core | `libm` | no | Pulls `libm` ^0.2 for math fns even with `std`. |
| type selection | `all-types` | yes | Enables `float-types` + `integer-types` + `size-types`. |
| type selection | `float-types` | yes (via all-types) | f64 types (DVec/DMat/DQuat). f32 is always present. |
| type selection | `integer-types` | yes (via all-types) | Enables `i8 i16 i32 i64 u8 u16 u32 u64` vector types. |
| type selection | `size-types` | yes (via all-types) | Enables `isize`/`usize` vector types. |
| type selection | `f64` | yes (via float-types) | f64 vectors/matrices. |
| type selection | `i8` `i16` `i32` `i64` | yes (via integer-types) | Signed integer vectors of that width. No extra deps. |
| type selection | `u8` `u16` `u32` `u64` | yes (via integer-types) | Unsigned integer vectors of that width. No extra deps. |
| type selection | `isize` `usize` | yes (via size-types) | Pointer-width integer vectors. |
| math behavior | `scalar-math` | no | Disables SIMD; uses native scalar alignment. Vec3A/Vec4 lose 16-byte SIMD layout. |
| math behavior | `fast-math` | no | Platform-specific float optimizations; relaxes some IEEE guarantees. |
| math behavior | `core-simd` | no | Portable SIMD (`core::simd`). Nightly only. Also flips bytemuck's `nightly_portable_simd`. |
| math behavior | `cuda` | no | CUDA-compatible alignment for types. |
| math behavior | `glam-assert` | no | Runtime parameter validation (e.g. normalized inputs). Off in release builds you ship. |
| math behavior | `debug-glam-assert` | no | Same asserts, but only active in debug builds. |
| interop / serde | `approx` | no | `approx` ^0.5 — `AbsDiffEq`/`RelativeEq`/`UlpsEq` impls for tests. |
| interop / serde | `arbitrary` | no | `arbitrary` ^1.4.2 — fuzzing input generation. |
| interop / serde | `bytemuck` | no | `bytemuck` ^1.9 — `Pod`/`Zeroable` impls. The GPU-upload path. |
| interop / serde | `encase` | no | `encase` ^0.12 — `ShaderType` impls with std140/std430 layout handling. |
| interop / serde | `mint` | no | `mint` ^0.5.8 — interop type conversions with other math crates. |
| interop / serde | `rand` | no | `rand` ^0.10 — sample random vectors/quaternions. |
| interop / serde | `rkyv` | no | `rkyv` ^0.8 — zero-copy archive serialization. |
| interop / serde | `bytecheck` | no | Enables `rkyv` ^0.8 `bytecheck` validation. Pair with `rkyv`. |
| interop / serde | `serde` | no | `serde_core` ^1.0 — `Serialize`/`Deserialize`. |
| interop / serde | `speedy` | no | `speedy` ^0.8 — fast binary serialization. |
| interop / serde | `zerocopy` | no | `zerocopy` ^0.8 + `zerocopy-derive` ^0.8 — zero-copy byte casts. |

Note on overlap: `bytemuck` and `zerocopy` both give you raw byte casts;
`encase` gives you correct GPU layout. They are not redundant — see below.

## Memory layout and the GPU buffer problem

This is the part that matters for the `wgpu` canvas renderer.

### Sizes and alignment of the common types

| Type | Size | Align | Notes |
|------|-----:|------:|-------|
| `Vec2` | 8 | 4 | Tightly packed `[f32; 2]`. |
| `Vec3` | 12 | 4 | Tightly packed `[f32; 3]`. No padding. |
| `Vec3A` | 16 | 16 | SIMD-backed. `Vec3` data + 4 bytes padding. 16-byte aligned. |
| `Vec4` | 16 | 16 | SIMD-backed, 16-byte aligned. |
| `Quat` | 16 | 16 | SIMD-backed, 16-byte aligned. |
| `Mat2` | 16 | 16 | |
| `Mat3` | 36 | 4 | Three `Vec3` columns; no SIMD padding. |
| `Mat3A` | 48 | 16 | Three `Vec3A` columns; 16-byte aligned. |
| `Mat4` | 64 | 16 | Four `Vec4` columns; 16-byte aligned. |

Types with an `A` suffix (`Vec3A`, `Mat3A`) are SIMD alternatives to the scalar
type. From the docs: "SIMD vector types are used for storage on supported
platforms for better performance than the Vec3 type." On x86_64 `Vec3A` reports
`USES_SSE2 = true`. With the `scalar-math` feature these fall back to native
scalar alignment and lose the 16-byte guarantee — do not assume 16-byte
alignment if you enable `scalar-math`.

Convert with `From`/`Into`: `Vec3A::from(v3)` and `Vec3::from(v3a)`.

### Why alignment matters: std140 / std430

GPU buffer layout rules are stricter than Rust's `#[repr(C)]`:

- **std140** (uniform buffers): a `vec3` is aligned to 16 bytes, and the stride
  of any array element rounds up to a multiple of 16. A `mat4` is 4 column
  vectors each 16-byte aligned (64 bytes total).
- **std430** (storage buffers): looser — `vec3` is still 16-byte aligned, but
  array strides are not force-padded to 16 the way std140 does.

The trap: glam's **host** layout does not always match the **GPU** layout.
`Vec3` is 12 bytes on the host but occupies a 16-byte slot in std140/std430. If
you put a `Vec3` in a `#[repr(C)]` struct and `bytemuck`-cast it straight into a
uniform buffer, every field after that `Vec3` lands at the wrong offset and the
shader reads garbage. `Vec3A`, `Vec4`, and `Mat4` already align to 16, so they
match — `Vec3` is the one that bites.

### Two ways to upload, and when to use each

1. **`encase` (`ShaderType`)** — preferred for uniform buffers. The `encase`
   feature gives glam types `encase::ShaderType` impls, and `encase` computes
   std140/std430 padding for you at the buffer level. You write into a
   `UniformBuffer`/`StorageBuffer` wrapper; it inserts the padding. Use this when
   the struct has `vec3`s, nested structs, or arrays — exactly the cases manual
   padding gets wrong.

2. **`bytemuck` (`Pod`/`Zeroable`)** — direct byte cast, fastest path, but *you*
   own correctness. Safe only when your struct's host layout already equals the
   GPU layout. Keep uniform structs built from `Mat4` and `Vec4` (both 16-byte
   aligned), avoid bare `Vec3`, and pad any sub-16 tail manually. Then
   `bytemuck::bytes_of` / `cast_slice` into the buffer is correct and copy-free.

Practical rule for Pixhaus uniform structs: **use `Mat4` and `Vec4`, not
`Vec3`/`Vec3A`.** If you need a 3-component value, store it as `Vec4` and ignore
`.w`, or add an explicit `_pad: f32`. Reach for `encase` the moment a struct
gets a `Vec3` or an array you would otherwise hand-pad.

## bytemuck: deriving Pod for a uniform struct

With the `bytemuck` feature, glam's types implement `Pod` and `Zeroable`, so a
`#[repr(C)]` struct of glam types can derive them too and be cast to bytes.

```rust
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec4};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CanvasUniform {
    // 64 bytes, 16-byte aligned — matches std140 mat4.
    view_proj: Mat4,
    // 16 bytes — packs an RGB tint into xyz, opacity into w.
    tint: Vec4,
    // 16 bytes — pixel grid params. Keep the struct a multiple of 16.
    grid: Vec4,
}
// size = 96, align = 16. No bare Vec3, so no hidden std140 offset bug.

// Upload:
let data = CanvasUniform { /* ... */ };
queue.write_buffer(&uniform_buffer, 0, bytemuck::bytes_of(&data));
// or at creation: contents: bytemuck::cast_slice(&[data])
```

If you must include a `Vec3`, either promote it to `Vec4` or insert an explicit
`_pad: f32` after it — do not rely on the 12-byte `Vec3` lining up with the
shader's 16-byte `vec3`.

## serde: the .pixhaus format

With `serde`, every glam type is `Serialize` + `Deserialize`. This is what lets
transforms, camera state, and tilemap offsets ride along in the `.pixhaus`
MessagePack (`rmp-serde`) + zstd format without hand-written codecs. glam
serializes vectors/matrices as flat sequences, which round-trips cleanly through
MessagePack.

## Other interop features

- **`mint`** — conversion types for crossing into other math libraries (e.g. a
  physics or engine crate that speaks `mint`). Convert via the `From` impls on
  the `mint::*` types; glam does not expose `mint` types in its own API surface.
- **`approx`** — `abs_diff_eq!` / `relative_eq!` for float-tolerant assertions.
  Use in tests instead of `==` on `Vec`/`Mat`, since exact float equality is
  fragile after transforms.
- **`rand`** — sample random vectors and quaternions; handy for proptest input
  and procedural placement.

## FloatExt

`FloatExt` is an extension trait implemented for both `f32` and `f64`, adding
shader-style scalar helpers:

| Method | Signature | What it does |
|--------|-----------|--------------|
| `lerp` | `fn lerp(self, rhs: Self, s: Self) -> Self` | Linear interpolation from `self` to `rhs` by `s`. |
| `inverse_lerp` | `fn inverse_lerp(a: Self, b: Self, v: Self) -> Self` | Inverse of `lerp`: where `v` sits in `[a, b]`, normalized. |
| `remap` | `fn remap(self, in_start, in_end, out_start, out_end) -> Self` | Map `self` from one range to another. |
| `fract_gl` | `fn fract_gl(self) -> Self` | GLSL-style fract: `self - floor(self)` (differs from Rust's `fract` for negatives). |
| `step` | `fn step(self, value: Self) -> Self` | `0.0` if `value < self`, else `1.0` (GLSL `step`). |
| `saturate` | `fn saturate(self) -> Self` | Clamp to `[0.0, 1.0]`. |

Bring it into scope with `use glam::FloatExt;` to call these on plain `f32`/`f64`.

## Recommended dependency line

```toml
glam = { version = "0.33", features = ["bytemuck", "serde"] }
```

Add `encase` the first time a uniform/storage struct needs std140/std430 padding
it cannot get from `Mat4`/`Vec4` alone:

```toml
glam = { version = "0.33", features = ["bytemuck", "serde", "encase"] }
```

### For Pixhaus

The `render` crate uploads camera and canvas uniforms to `wgpu` every frame, so
`bytemuck` is the baseline — derive `Pod`/`Zeroable` on `#[repr(C)]` uniform
structs built from `Mat4` and `Vec4` (both 16-byte aligned, matching std140) and
`bytemuck::bytes_of` them into the buffer; never put a bare `Vec3` in a uniform
struct without explicit padding, since glam's 12-byte host `Vec3` does not match
the shader's 16-byte `vec3` and silently corrupts later fields. Add `encase` only
when a buffer's layout outgrows hand-packing (arrays, nested structs, or an
unavoidable `vec3`). `serde` covers persisting transforms and view state into the
`.pixhaus` MessagePack format. Keep the default `all-types` set unless a crate
proves it needs only f32 — the integer vector types are cheap and useful for
tile coordinates and pixel indices.
