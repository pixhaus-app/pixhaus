---
name: pixhaus-glam
description: >
  Use when writing, reviewing, or debugging any vector, matrix, quaternion, or
  transform math in Pixhaus — building the canvas view/projection matrix, mapping
  mouse position to pixel coordinates, panning/zooming the viewport, color math on
  RGBA, packing uniforms for the wgpu renderer, or any 2D/3D geometry. glam is the
  linear-algebra crate (Vec2/Vec3/Vec4, Mat3/Mat4, Quat, Affine2/Affine3A, plus
  integer IVec/UVec and f64 DVec families). Trigger this for ANY math-on-coordinates
  work even when the user says "the camera", "zoom to fit", "where did they click",
  "the transform", "rotate the selection", or "send this to the shader" without
  naming glam. glam is column-major and its SIMD types have alignment rules that bite
  on GPU upload, so reach for this skill rather than guessing signatures or layout.
---

# glam for Pixhaus

glam is the math layer under the wgpu canvas: vectors for positions and colors,
matrices for the view/projection that maps the document into clip space, and the
integer vector families for pixel coordinates. It's fast (SIMD f32) and the
de-facto standard in the Rust graphics ecosystem, so it interoperates cleanly with
`wgpu`, `bytemuck`, and `encase`.

This skill is the floor for math work in Pixhaus: the handful of facts that prevent
the recurring bugs (column-major order, SIMD alignment on GPU upload, quaternion
normalization), the version and feature pin, and how the types map onto a pixel-art
editor. When you need the full method surface for a type, open the matching file in
`references/` — don't guess signatures from memory; glam's API has shifted across
releases and the references are derived from docs.rs 0.33.0.

## Version and features — pin these

```toml
glam = { version = "0.33", features = ["bytemuck", "serde"] }
```

- `bytemuck` gives glam types `Pod`/`Zeroable`, so a `#[repr(C)]` uniform struct of
  glam types uploads to a wgpu buffer with `bytemuck::bytes_of`. This is the GPU path.
- `serde` gives `Serialize`/`Deserialize` — needed for the `.pixhaus` MessagePack
  format (camera state, layer transforms).
- Default features already include every type family (f32, f64, all integer widths,
  isize/usize). Don't disable them without a reason; the build cost is small.
- Add `encase` later if a uniform struct outgrows hand-packing — it handles
  std140/std430 padding via `ShaderType`. Add `approx` in dev-dependencies for
  `abs_diff_eq` assertions in tests.

MSRV is 1.68.2. License is MIT OR Apache-2.0 — clears the [[project-v2-native-restart]]
MIT lock. When you bump glam, re-verify the references against docs.rs — see
[[feedback-dep-upgrades]].

## The mental model: three facts that cause most bugs

1. **Matrices are column-major and multiply on the left of the vector.** A transform
   is `m * v`, never `v * m`. Composition reads right-to-left: `view * model * point`
   applies `model` first, then `view`. `to_cols_array()` emits in column order, which
   is exactly what WGSL's `mat4x4<f32>` expects — no transpose on upload. Get the
   order wrong and geometry collapses or mirrors with no compile error.

2. **The "A" types are 16-byte aligned; plain `Vec3` is not.** `Vec3` is 12 bytes,
   4-byte aligned (tight storage). `Vec3A`, `Vec4`, `Mat4` are 16-byte aligned (SIMD).
   This matters on GPU upload: WGSL `vec3<f32>` aligns to 16 bytes under std140/std430,
   so a host `Vec3` field in a uniform struct will misalign the fields after it. The
   fix: use `Vec4` and `Mat4` in uniform structs (already 16-aligned), or pad `Vec3`
   fields manually, or use the `encase` feature. Pick `Vec3` for CPU-side document
   data, `Vec3A`/`Vec4` for hot loops and GPU buffers. See
   `references/features-and-interop.md`.

3. **Quaternions and normals must stay normalized.** `Quat` represents rotation only
   while unit-length; repeated multiplication drifts it off the unit sphere. Re-normalize
   after accumulating rotations. `normalize` panics/►NaNs on a zero vector — use
   `normalize_or_zero` or `try_normalize` when the input might be degenerate (a
   zero-length drag delta, a click that didn't move).

## Rules that prevent the recurring bugs

- **Use the non-`_gl` projection constructors for wgpu.** wgpu/Metal/DX clip space
  uses NDC depth `0..1`; OpenGL uses `-1..1`. `orthographic_rh` / `perspective_rh` are
  correct; `orthographic_rh_gl` / `perspective_rh_gl` are wrong for our renderer and
  will clip or invert depth. The 2D canvas wants `Mat4::orthographic_rh`.
- **Convert between pixel and world coordinates with `as_*`, not `as`.** Pixel
  coordinates are `UVec2`/`IVec2`; world/screen positions are `Vec2`. Go up with
  `pixel.as_vec2()` and back with `world.floor().as_ivec2()` — floor *before* the cast
  so you land in the correct pixel cell, not toward zero. A bare `x as i32` truncates
  toward zero and puts negative coordinates in the wrong cell.
- **Read `.changed()`-style results, don't recompute geometry every frame** if the
  inputs didn't move — but glam ops are cheap, so favor clarity over caching unless a
  profile says otherwise. The [[8k-perf-constraint]] is about per-pixel work, not
  per-vector math.
- **Swizzle traits must be in scope.** `v.xy()`, `v.zyx()` come from `Vec2Swizzles` /
  `Vec3Swizzles` / `Vec4Swizzles`. They're re-exported at the glam root, so
  `use glam::Vec3Swizzles;` (or `use glam::*;`) brings them in. glam has no `rgba`
  aliases — color reorders use the positional letters (`color.zyxw()` is BGRA).
- **`transform_point` applies translation; `transform_vector` does not.** Use
  `transform_point2`/`transform_point3` for positions, `transform_vector2`/
  `transform_vector3` for directions and deltas. Transforming a drag delta as a point
  adds the translation twice.

## Pixhaus applications

Where the types land in a pixel-art editor on wgpu:

- **The canvas view matrix is a `Mat4::orthographic_rh`** sized to the document, post-
  multiplied by pan/zoom. Build it once per frame, push it as a uniform. Pan is a
  translation, zoom is a scale; compose with `Mat4::from_scale_rotation_translation`
  or chain `*`. See `references/matrices.md` for the MVP pattern.
- **Pixel coordinates are `IVec2`/`UVec2`.** Mouse position arrives as `Vec2` (egui
  `Pos2` → `Vec2`); map it through the inverse view matrix, then `floor().as_ivec2()`
  to get the pixel under the cursor. Coordinate conversions are in
  `references/other-families.md`.
- **Colors are `Vec4` (RGBA, 0..1) for math, `[u8; 4]` for storage.** Premultiplied
  alpha, blending, and tint math run on `Vec4` (`lerp`, component-wise `*`); convert at
  the buffer edge. Note egui uses premultiplied `Color32` — keep the conventions
  straight at the boundary.
- **Uniform structs use `Vec4` and `Mat4`, derive `Pod`/`Zeroable`.** A
  `#[repr(C)] #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]` struct of
  glam types is `bytemuck::bytes_of`'d straight into a wgpu buffer. Keep every field
  16-aligned. Example in `references/features-and-interop.md`.
- **Layer/selection transforms are `Affine2`.** A 2D affine (rotate/scale/translate a
  sprite or selection) is cheaper and clearer than a `Mat4` — no projection row.
  `Affine2::from_scale_angle_translation` then `transform_point2`. See
  `references/quat-and-affine.md`. Reserve `Quat`/`Affine3A` for any 3D verb (mesh
  deformation) — most of the editor is 2D.

## References

Open the file for the area you're working in; each is a dense API reference for glam
0.33.0, with load-bearing signatures checked against docs.rs.

| File | Covers |
|---|---|
| `references/vectors.md` | `Vec2`/`Vec3`/`Vec3A`/`Vec4` — constructors, constants, geometry (dot, cross, length, normalize, lerp, project), component-wise ops, comparison masks, operators, conversions |
| `references/matrices.md` | `Mat2`/`Mat3`/`Mat3A`/`Mat4` — column-major model, all constructors, the wgpu vs GL projection split, transform methods, the MVP and 2D-affine patterns |
| `references/quat-and-affine.md` | `Quat`, `Affine2`, `Affine3A`, `EulerRot` — rotation construction, slerp, TRS compose, transform_point vs transform_vector |
| `references/swizzles.md` | `Vec2/3/4Swizzles` traits — the naming rule, scope requirement, `truncate`/`extend`/`with_*`, color reorders |
| `references/other-families.md` | Integer (`IVec`/`UVec`), bool (`BVec` masks), f64 (`DVec`/`DMat`) families; what integers keep vs drop; the `as_*` cross-family cast tables for pixel↔world |
| `references/features-and-interop.md` | Cargo feature table, SIMD size/alignment, the std140/std430 GPU-buffer trap, bytemuck vs encase upload paths, serde/mint/approx, `FloatExt` |

A standing caution: the references record the 0.33.0 API faithfully, but a few deep
signatures were flagged during research as unverifiable from the rendered docs (noted
inline as "(verify)"). When one is load-bearing for what you're building, confirm it
against https://docs.rs/glam/0.33.0/glam/ or the source before depending on it.
