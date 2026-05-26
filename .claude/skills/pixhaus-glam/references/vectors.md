# glam 0.33.0 f32 vector API reference

`Vec2`, `Vec3`, `Vec3A`, and `Vec4` are glam's single-precision float vectors. `Vec2`
and `Vec3` are `#[repr(C)]` structs with public `x`/`y`(`/z`) fields and no padding —
they're the right choice for storage, vertex buffers, and anything crossing an FFI or
GPU layout boundary. `Vec3A` and `Vec4` are 16-byte-aligned SIMD types (SSE2 on x86):
faster for arithmetic-heavy math but `Vec3A` costs 16 bytes per value (4 bytes of
padding) and its fields are private (access via `Deref` to `Vec3`, swizzles, or
`Index`). Pick `Vec3` when size/layout matters (storing many of them, uploading to the
GPU); pick `Vec3A` when a value is hot in a math loop and the extra 4 bytes don't hurt.
`Vec4` is always SIMD-aligned. Convert freely: `Vec3 <-> Vec3A` via `From`/`.to_vec3a()`/
`.to_vec3()`, and `Vec3A` derefs to `Vec3` so all `Vec3` methods are callable on a
`Vec3A`.

Conventions below: all methods take `self` by value (these are `Copy`). `Self` is the
vector's own type. Mask types are `BVec2`/`BVec3`/`BVec4` for `Vec2`/`Vec3`, but `Vec3A`
uses `BVec3A` and `Vec4` uses `BVec4A`.

## Vec2

`#[repr(C)] struct Vec2 { pub x: f32, pub y: f32 }`. 8 bytes, scalar math (no SIMD).
Mask type: `BVec2`.

### Constants

`ZERO ONE NEG_ONE MIN MAX NAN INFINITY NEG_INFINITY X Y NEG_X NEG_Y` are all `Self`.
`AXES: [Self; 2]` = `[X, Y]`.

### Fields and access

Direct: `v.x`, `v.y`. Indexed: `v[0]`, `v[1]` (`Index`/`IndexMut<usize> -> f32`).
Builders: `with_x(self, x: f32) -> Self`, `with_y(self, y: f32) -> Self`.

### Construction and conversion

```rust
const fn new(x: f32, y: f32) -> Self
const fn splat(v: f32) -> Self
const fn from_array(a: [f32; 2]) -> Self
const fn to_array(&self) -> [f32; 2]
const fn from_slice(slice: &[f32]) -> Self      // panics if len < 2
fn write_to_slice(self, slice: &mut [f32])       // panics if len < 2
const fn extend(self, z: f32) -> Vec3            // -> (x, y, z)
fn select(mask: BVec2, if_true: Self, if_false: Self) -> Self
fn map<F: Fn(f32) -> f32>(self, f: F) -> Self
fn from_angle(angle: f32) -> Self                // unit vector at angle (radians)
// as_*: as_dvec2 as_i8vec2 as_u8vec2 as_i16vec2 as_u16vec2 as_ivec2 as_uvec2
//       as_i64vec2 as_u64vec2 as_isizevec2 as_usizevec2
fn as_dvec2(self) -> DVec2
fn as_ivec2(self) -> IVec2
fn as_uvec2(self) -> UVec2
```

### Arithmetic and geometry

```rust
fn dot(self, rhs: Self) -> f32
fn dot_into_vec(self, rhs: Self) -> Self          // splat of dot
fn length(self) -> f32
fn length_squared(self) -> f32
fn length_recip(self) -> f32
fn distance(self, rhs: Self) -> f32
fn distance_squared(self, rhs: Self) -> f32
fn normalize(self) -> Self                        // panics/NaN if ~zero length
fn try_normalize(self) -> Option<Self>
fn normalize_or(self, fallback: Self) -> Self
fn normalize_or_zero(self) -> Self
fn normalize_and_length(self) -> (Self, f32)
fn is_normalized(self) -> bool
fn project_onto(self, rhs: Self) -> Self
fn reject_from(self, rhs: Self) -> Self
fn project_onto_normalized(self, rhs: Self) -> Self
fn reject_from_normalized(self, rhs: Self) -> Self
fn lerp(self, rhs: Self, s: f32) -> Self
fn move_towards(self, rhs: Self, d: f32) -> Self
fn midpoint(self, rhs: Self) -> Self
fn mul_add(self, a: Self, b: Self) -> Self         // self*a + b
fn reflect(self, normal: Self) -> Self             // normal must be normalized
fn refract(self, normal: Self, eta: f32) -> Self
fn div_euclid(self, rhs: Self) -> Self
fn rem_euclid(self, rhs: Self) -> Self
```

### Angle and 2D rotation helpers

```rust
fn perp(self) -> Self                  // 90deg CCW: (-y, x)
fn perp_dot(self, rhs: Self) -> f32    // 2D cross / wedge: x*rhs.y - y*rhs.x
fn rotate(self, rhs: Self) -> Self     // rotate rhs by the angle/scale of self (complex mul)
fn rotate_towards(self, rhs: Self, max_angle: f32) -> Self
fn to_angle(self) -> f32               // angle of self (radians, atan2)
fn angle_to(self, rhs: Self) -> f32    // signed angle self -> rhs
```

### Component-wise

```rust
fn min(self, rhs: Self) -> Self
fn max(self, rhs: Self) -> Self
fn clamp(self, min: Self, max: Self) -> Self      // panics if any min > max
fn min_element(self) -> f32
fn max_element(self) -> f32
fn min_position(self) -> usize
fn max_position(self) -> usize
fn element_sum(self) -> f32
fn element_product(self) -> f32
fn abs(self) -> Self
fn signum(self) -> Self
fn copysign(self, rhs: Self) -> Self
fn round(self) -> Self
fn floor(self) -> Self
fn ceil(self) -> Self
fn trunc(self) -> Self
fn fract(self) -> Self                 // self - trunc(self)
fn fract_gl(self) -> Self              // self - floor(self)
fn recip(self) -> Self
fn powf(self, n: f32) -> Self
fn exp(self) -> Self
fn exp2(self) -> Self
fn ln(self) -> Self
fn log2(self) -> Self
fn sqrt(self) -> Self
fn sin(self) -> Self
fn cos(self) -> Self
fn sin_cos(self) -> (Self, Self)
fn saturate(self) -> Self              // clamp to [0, 1]
fn step(self, rhs: Self) -> Self       // 0.0 where self < rhs else 1.0 (verify edge convention)
fn clamp_length(self, min: f32, max: f32) -> Self
fn clamp_length_max(self, max: f32) -> Self
fn clamp_length_min(self, min: f32) -> Self
```

### Comparison and masks

```rust
fn cmpeq(self, rhs: Self) -> BVec2
fn cmpne(self, rhs: Self) -> BVec2
fn cmpge(self, rhs: Self) -> BVec2
fn cmpgt(self, rhs: Self) -> BVec2
fn cmple(self, rhs: Self) -> BVec2
fn cmplt(self, rhs: Self) -> BVec2
fn abs_diff_eq(self, rhs: Self, max_abs_diff: f32) -> bool
fn is_finite(self) -> bool
fn is_finite_mask(self) -> BVec2
fn is_nan(self) -> bool
fn is_nan_mask(self) -> BVec2
fn is_negative_bitmask(self) -> u32    // bit i set if component i is negative
```

Use `BVec2::any()` / `BVec2::all()` to reduce a mask to a `bool`, and
`Vec2::select(mask, a, b)` to pick per-lane.

### Operators

| Op | rhs = `Vec2` | rhs = `f32` | `f32` lhs |
| --- | --- | --- | --- |
| `Add` `Sub` `Mul` `Div` `Rem` | yes -> `Vec2` | yes -> `Vec2` | yes -> `Vec2` |
| `AddAssign` `SubAssign` `MulAssign` `DivAssign` `RemAssign` | yes | yes | n/a |

Also: `Neg for Vec2` and `&Vec2`; `Mul<Vec2> for Mat2 -> Vec2`;
`Index`/`IndexMut<usize> -> f32`.

### From / Into

`From<[f32; 2]>`, `From<(f32, f32)>`, `From<BVec2>` build a `Vec2`. `Vec2` converts into
`[f32; 2]`, `(f32, f32)`, `DVec2`. `AsRef<[f32; 2]>` / `AsMut<[f32; 2]>`. To go up a
dimension use `extend`. To get a `Vec2` from `Vec3`/`Vec3A` use their `truncate()`.

## Vec3

`#[repr(C)] struct Vec3 { pub x: f32, pub y: f32, pub z: f32 }`. 12 bytes, scalar math.
Mask type: `BVec3`.

### Constants

`ZERO ONE NEG_ONE MIN MAX NAN INFINITY NEG_INFINITY X Y Z NEG_X NEG_Y NEG_Z` are `Self`.
`AXES: [Self; 3]` = `[X, Y, Z]`.

### Fields and access

Direct `v.x v.y v.z`; `v[0..=2]` (`Index`/`IndexMut<usize> -> f32`).
`with_x/with_y/with_z(self, _: f32) -> Self`.

### Construction and conversion

```rust
const fn new(x: f32, y: f32, z: f32) -> Self
const fn splat(v: f32) -> Self
const fn from_array(a: [f32; 3]) -> Self
const fn to_array(&self) -> [f32; 3]
const fn from_slice(slice: &[f32]) -> Self        // panics if len < 3
fn write_to_slice(self, slice: &mut [f32])         // panics if len < 3
fn extend(self, w: f32) -> Vec4                    // -> (x, y, z, w)
fn truncate(self) -> Vec2                          // drop z
fn to_vec3a(self) -> Vec3A
fn to_homogeneous(self) -> Vec4                    // (x, y, z, 1.0)
fn from_homogeneous(v: Vec4) -> Self               // perspective divide (verify)
fn select(mask: BVec3, if_true: Self, if_false: Self) -> Self
fn map<F: Fn(f32) -> f32>(self, f: F) -> Self
// as_*: as_dvec3 as_i8vec3 as_u8vec3 as_i16vec3 as_u16vec3 as_ivec3 as_uvec3
//       as_i64vec3 as_u64vec3 as_isizevec3 as_usizevec3
fn as_dvec3(self) -> DVec3
fn as_ivec3(self) -> IVec3
fn as_uvec3(self) -> UVec3
```

### Arithmetic and geometry

```rust
fn dot(self, rhs: Self) -> f32
fn dot_into_vec(self, rhs: Self) -> Self
fn cross(self, rhs: Self) -> Self                  // 3D cross product (Vec3/Vec3A only)
fn length(self) -> f32
fn length_squared(self) -> f32
fn length_recip(self) -> f32
fn distance(self, rhs: Self) -> f32
fn distance_squared(self, rhs: Self) -> f32
fn normalize(self) -> Self
fn try_normalize(self) -> Option<Self>
fn normalize_or(self, fallback: Self) -> Self
fn normalize_or_zero(self) -> Self
fn normalize_and_length(self) -> (Self, f32)
fn is_normalized(self) -> bool
fn project_onto(self, rhs: Self) -> Self
fn reject_from(self, rhs: Self) -> Self
fn project_onto_normalized(self, rhs: Self) -> Self
fn reject_from_normalized(self, rhs: Self) -> Self
fn lerp(self, rhs: Self, s: f32) -> Self
fn slerp(self, rhs: Self, s: f32) -> Self          // spherical, for direction vectors
fn move_towards(self, rhs: Self, d: f32) -> Self
fn midpoint(self, rhs: Self) -> Self
fn mul_add(self, a: Self, b: Self) -> Self
fn reflect(self, normal: Self) -> Self
fn refract(self, normal: Self, eta: f32) -> Self
fn div_euclid(self, rhs: Self) -> Self
fn rem_euclid(self, rhs: Self) -> Self
```

### Angle, rotation, orthogonal helpers

```rust
fn angle_between(self, rhs: Self) -> f32           // unsigned, [0, pi]
fn rotate_x(self, angle: f32) -> Self
fn rotate_y(self, angle: f32) -> Self
fn rotate_z(self, angle: f32) -> Self
fn rotate_axis(self, axis: Self, angle: f32) -> Self   // axis must be normalized (verify)
fn rotate_towards(self, rhs: Self, max_angle: f32) -> Self
fn any_orthogonal_vector(self) -> Self             // some vector perpendicular to self
fn any_orthonormal_vector(self) -> Self            // self must be normalized
fn any_orthonormal_pair(self) -> (Self, Self)      // self must be normalized
```

### Component-wise

Same set as `Vec2`: `min max clamp min_element max_element min_position max_position
element_sum element_product abs signum copysign round floor ceil trunc fract fract_gl
recip powf exp exp2 ln log2 sqrt sin cos sin_cos saturate step clamp_length
clamp_length_max clamp_length_min`. Signatures match `Vec2` with `Self` = `Vec3`.

### Comparison and masks

`cmpeq cmpne cmpge cmpgt cmple cmplt(self, rhs) -> BVec3`; `abs_diff_eq(self, rhs,
max_abs_diff) -> bool`; `is_finite -> bool`, `is_finite_mask -> BVec3`, `is_nan ->
bool`, `is_nan_mask -> BVec3`, `is_negative_bitmask -> u32`.

### Operators

Same matrix as `Vec2` (`Add Sub Mul Div Rem` with `Vec3` rhs, `f32` rhs, and `f32` lhs;
the `*Assign` forms; `Neg`). Linear-algebra products: `Mat3 * Vec3 -> Vec3`,
`Mat3A * Vec3 -> Vec3`, `Quat * Vec3 -> Vec3`. `Index`/`IndexMut<usize> -> f32`.

### From / Into

Build: `From<[f32; 3]>`, `From<(f32, f32, f32)>`, `From<(Vec2, f32)>`, `From<Vec3A>`,
`From<BVec3>`, `From<BVec3A>`. Convert into: `[f32; 3]`, `(f32, f32, f32)`, `DVec3`,
`Vec3A`. `AsRef`/`AsMut<[f32; 3]>`. From a `Vec4`, use `Vec4::truncate()`.

## Vec3A

`struct Vec3A(/* private */)`. 16-byte aligned, 16 bytes total (one f32 of padding),
SSE2 SIMD on x86. Fields are private. Mask type: `BVec3A`. Implements `Deref`/`DerefMut
-> Vec3`, so every `Vec3` method and field is reachable through a `Vec3A`.

### Constants

Identical names and meaning to `Vec3`: `ZERO ONE NEG_ONE MIN MAX NAN INFINITY
NEG_INFINITY X Y Z NEG_X NEG_Y NEG_Z` (`Self`), `AXES: [Self; 3]`.

### Access

No public fields; use `*v` / `Deref` (`v.x` works via deref), swizzles, or `v[0..=2]`
(`Index`/`IndexMut<usize> -> f32`). `with_x/with_y/with_z(self, _: f32) -> Self`.

### Construction and conversion

```rust
const fn new(x: f32, y: f32, z: f32) -> Self
const fn splat(v: f32) -> Self
const fn from_array(a: [f32; 3]) -> Self
const fn to_array(&self) -> [f32; 3]
const fn from_slice(slice: &[f32]) -> Self
fn write_to_slice(self, slice: &mut [f32])
fn from_vec4(v: Vec4) -> Self                      // drop w
fn extend(self, w: f32) -> Vec4
fn truncate(self) -> Vec2
fn to_vec3(self) -> Vec3
fn to_homogeneous(self) -> Vec4                     // (x, y, z, 1.0)
fn from_homogeneous(v: Vec4) -> Self               // (verify)
fn select(mask: BVec3A, if_true: Self, if_false: Self) -> Self
fn map<F: Fn(f32) -> f32>(self, f: F) -> Self
fn as_dvec3(self) -> DVec3
fn as_ivec3(self) -> IVec3
fn as_uvec3(self) -> UVec3
// plus as_i8vec3 as_u8vec3 as_i16vec3 as_u16vec3 as_i64vec3 as_u64vec3
//      as_isizevec3 as_usizevec3
```

### Methods

The full `Vec3` surface is available with `Self` = `Vec3A` and masks as `BVec3A`:
`dot dot_into_vec cross length length_squared length_recip distance distance_squared
normalize try_normalize normalize_or normalize_or_zero normalize_and_length
is_normalized project_onto reject_from project_onto_normalized reject_from_normalized
lerp slerp move_towards midpoint mul_add reflect refract div_euclid rem_euclid
angle_between any_orthogonal_vector any_orthonormal_vector any_orthonormal_pair
rotate_x rotate_y rotate_z rotate_axis rotate_towards min max clamp min_element
max_element min_position max_position element_sum element_product abs signum copysign
round floor ceil trunc fract fract_gl recip powf exp exp2 ln log2 sqrt sin cos sin_cos
saturate step clamp_length clamp_length_max clamp_length_min cmpeq cmpne cmpge cmpgt
cmple cmplt abs_diff_eq is_finite is_finite_mask is_nan is_nan_mask is_negative_bitmask`.
Comparison methods return `BVec3A`.

### Operators

`Add Sub Mul Div Rem` for `Vec3A`-rhs, `f32`-rhs, and `f32`-lhs (all `-> Vec3A`);
matching `*Assign`; `Neg`; and `&` reference variants of all of these. Products:
`Mat3 * Vec3A -> Vec3A`, `Mat3A * Vec3A -> Vec3A`, `Quat * Vec3A -> Vec3A`.
`Index`/`IndexMut<usize> -> f32`.

### From / Into

Build: `From<[f32; 3]>`, `From<(f32, f32, f32)>`, `From<(Vec2, f32)>`, `From<Vec3>`,
`From<BVec3>`, `From<BVec3A>`, `From<__m128>`. Convert into: `[f32; 3]`,
`(f32, f32, f32)`, `Vec3`, `__m128`. `Vec4 -> Vec3A` via `from_vec4`. `AsRef`/`AsMut<[f32;
3]>`. The `Vec3 <-> Vec3A` pair: `Vec3A::from(vec3)` / `vec3.to_vec3a()` and
`Vec3::from(vec3a)` / `vec3a.to_vec3()`.

## Vec4

`struct Vec4(/* private */)`. 16-byte aligned, 16 bytes, SSE2 SIMD on x86. Fields
private. Mask type for comparisons and `select`: `BVec4A`.

### Constants

`ZERO ONE NEG_ONE MIN MAX NAN INFINITY NEG_INFINITY X Y Z W NEG_X NEG_Y NEG_Z NEG_W`
are `Self`. `AXES: [Self; 4]` = `[X, Y, Z, W]`.

### Access

No public fields; use swizzles or `v[0..=3]` (`Index`/`IndexMut<usize> -> f32`).
`with_x/with_y/with_z/with_w(self, _: f32) -> Self`.

### Construction and conversion

```rust
const fn new(x: f32, y: f32, z: f32, w: f32) -> Self
const fn splat(v: f32) -> Self
const fn from_array(a: [f32; 4]) -> Self
const fn to_array(&self) -> [f32; 4]
const fn from_slice(slice: &[f32]) -> Self        // panics if len < 4
fn write_to_slice(self, slice: &mut [f32])         // panics if len < 4
fn truncate(self) -> Vec3                          // drop w -> (x, y, z)
fn project(self) -> Vec3                           // perspective divide (x/w, y/w, z/w)
fn select(mask: BVec4A, if_true: Self, if_false: Self) -> Self
fn map<F: Fn(f32) -> f32>(self, f: F) -> Self
fn as_dvec4(self) -> DVec4
fn as_ivec4(self) -> IVec4
fn as_uvec4(self) -> UVec4
// plus as_i8vec4 as_u8vec4 as_i16vec4 as_u16vec4 as_i64vec4 as_u64vec4
//      as_isizevec4 as_usizevec4
```

### Arithmetic and geometry

```rust
fn dot(self, rhs: Self) -> f32
fn dot_into_vec(self, rhs: Self) -> Self
fn length(self) -> f32
fn length_squared(self) -> f32
fn length_recip(self) -> f32
fn distance(self, rhs: Self) -> f32
fn distance_squared(self, rhs: Self) -> f32
fn normalize(self) -> Self
fn try_normalize(self) -> Option<Self>
fn normalize_or(self, fallback: Self) -> Self
fn normalize_or_zero(self) -> Self
fn normalize_and_length(self) -> (Self, f32)
fn is_normalized(self) -> bool
fn project_onto(self, rhs: Self) -> Self
fn reject_from(self, rhs: Self) -> Self
fn project_onto_normalized(self, rhs: Self) -> Self
fn reject_from_normalized(self, rhs: Self) -> Self
fn lerp(self, rhs: Self, s: f32) -> Self
fn move_towards(self, rhs: Self, d: f32) -> Self
fn midpoint(self, rhs: Self) -> Self
fn mul_add(self, a: Self, b: Self) -> Self
fn reflect(self, normal: Self) -> Self
fn refract(self, normal: Self, eta: f32) -> Self
fn div_euclid(self, rhs: Self) -> Self
fn rem_euclid(self, rhs: Self) -> Self
fn clamp_length(self, min: f32, max: f32) -> Self
fn clamp_length_max(self, max: f32) -> Self
fn clamp_length_min(self, min: f32) -> Self
```

No `cross`, no angle/rotation/orthogonal helpers, no `slerp` (those are `Vec3`/`Vec3A`
or `Vec2`-specific).

### Component-wise

Same set as the others (`Self` = `Vec4`): `min max clamp min_element max_element
min_position max_position element_sum element_product abs signum copysign round floor
ceil trunc fract fract_gl recip powf exp exp2 ln log2 sqrt sin cos sin_cos saturate
step`.

### Comparison and masks

`cmpeq cmpne cmpge cmpgt cmple cmplt(self, rhs) -> BVec4A`; `abs_diff_eq(self, rhs,
max_abs_diff) -> bool`; `is_finite -> bool`, `is_finite_mask -> BVec4A`, `is_nan ->
bool`, `is_nan_mask -> BVec4A`, `is_negative_bitmask -> u32` (4-bit).

### Operators

Same matrix as the others: `Add Sub Mul Div Rem` for `Vec4`-rhs, `f32`-rhs, `f32`-lhs;
matching `*Assign`; `Neg`. `Index`/`IndexMut<usize> -> f32`. (Matrix product
`Mat4 * Vec4 -> Vec4` lives on `Mat4`.)

### From / Into

Build: `From<[f32; 4]>`, `From<(f32, f32, f32, f32)>`, `From<(Vec3, f32)>`,
`From<(f32, Vec3)>`, `From<(Vec3A, f32)>`, `From<(f32, Vec3A)>`, `From<(Vec2, f32, f32)>`,
`From<(Vec2, Vec2)>`, `From<BVec4>`, `From<BVec4A>`, `From<Quat>`, `From<__m128>`.
Convert into: `[f32; 4]`, `(f32, f32, f32, f32)`, `DVec4`, `__m128`. `AsRef`/`AsMut<[f32;
4]>`. Down a dimension: `truncate()` or `project()`.

## Common patterns

```rust
use glam::{Vec2, Vec3, Vec3A, Vec4, vec3};

// Build a position (the vec3! free function is shorthand for Vec3::new).
let p = Vec3::new(1.0, 2.0, 3.0);
let q = vec3(0.0, 1.0, 0.0);

// Direction, length-safe normalize (never NaN on a zero vector).
let dir = (q - p).normalize_or_zero();

// Frame-rate-independent move toward a target.
let next = p.move_towards(q, speed * dt);

// Lerp and slerp.
let mid   = p.lerp(q, 0.5);                 // straight-line blend
let arc    = a.normalize().slerp(b.normalize(), t); // angular blend (Vec3)

// Dot / cross for lighting and basis building.
let ndl    = normal.dot(light_dir).max(0.0);
let bitan  = normal.cross(tangent);

// Component clamp into a box, then per-lane select on a mask.
let clamped = p.clamp(Vec3::ZERO, Vec3::splat(16.0));
let pick    = Vec3::select(p.cmplt(q), p, q); // min per lane, mask-driven

// Homogeneous round-trip: Vec3 -> Vec4 -> perspective divide -> Vec3.
let clip = p.extend(1.0);                    // Vec4 (x, y, z, 1)
let ndc   = (mvp * clip).project();           // Vec3 after /w

// SIMD hot loop: compute in Vec3A, store as Vec3.
let acc = Vec3A::from(p) + Vec3A::from(q);
let stored: Vec3 = acc.to_vec3();

// Swizzles (from Vec*Swizzles traits): reorder/duplicate lanes.
let v = Vec4::new(1.0, 2.0, 3.0, 4.0);
let xy: Vec2 = v.xy();
let bgra: Vec4 = v.zyxw();
```
