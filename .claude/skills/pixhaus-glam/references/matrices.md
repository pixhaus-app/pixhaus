# glam 0.33.0 f32 matrices

API reference for `glam::f32::{Mat2, Mat3, Mat3A, Mat4}`. Signatures verbatim from
docs.rs. Anything marked "(verify)" was inferred, not confirmed on the page.

## Orientation

glam matrices are column-major: stored as column vectors (`x_axis`, `y_axis`, ...),
each column a vector type. Vectors are treated as columns, so the multiply order is
`matrix * vector` (`m * v`), and composition reads right-to-left: `a * b` applies `b`
first, then `a`. `to_cols_array` emits in column order — the layout GPU APIs expect
for `std140`/uniform upload, no transpose needed for wgpu/WGSL `mat4x4<f32>`.

Type roles:

- `Mat2` — 2D linear maps only: rotation, scale, shear. No translation. 2x2, columns
  are `Vec2`.
- `Mat3` — two roles. As a 2D affine transform it carries translation in the third
  column and is applied with `transform_point2` / `transform_vector2`. As a 3D map it
  is a linear/rotation matrix (no translation). 3x3, columns are `Vec3`.
- `Mat3A` — same math as `Mat3` but columns are `Vec3A` (16-byte aligned, SIMD). Faster
  for batched 3D linear transforms; pays alignment padding. Pick `Mat3` for tight
  storage / 2D affine, `Mat3A` for hot 3D transform loops. Interconvertible via
  `From`.
- `Mat4` — 3D affine (rotation + scale + translation) and projection. 4x4, columns are
  `Vec4`. The matrix type you upload to a shader as the MVP.

wgpu depth note: wgpu/Metal/DX clip space uses NDC depth `0..1`. Use the non-`_gl`
projection constructors (`perspective_rh`, `orthographic_rh`, ...). The `_gl` variants
produce OpenGL's `-1..1` depth and are wrong for wgpu.

---

## Mat2

2x2 column-major, columns `Vec2`. 2D linear transforms (rotation, scale, shear); no
translation.

### Constants

```rust
pub const ZERO: Self;     // all 0.0
pub const IDENTITY: Self; // diagonal 1.0, off-diagonal 0.0
pub const NAN: Self;      // all NaN
```

### Constructors

```rust
pub const fn from_cols(x_axis: Vec2, y_axis: Vec2) -> Self
pub const fn from_cols_array(m: &[f32; 4]) -> Self
pub const fn from_cols_array_2d(m: &[[f32; 2]; 2]) -> Self
pub const fn from_diagonal(diagonal: Vec2) -> Self
pub fn from_scale_angle(scale: Vec2, angle: f32) -> Self  // angle in radians
pub fn from_angle(angle: f32) -> Self                     // rotation, radians
pub fn from_mat3(m: Mat3) -> Self                         // top-left 2x2
pub fn from_mat3_minor(m: Mat3, i: usize, j: usize) -> Self
pub fn from_mat3a(m: Mat3A) -> Self
pub fn from_mat3a_minor(m: Mat3A, i: usize, j: usize) -> Self
pub const fn from_cols_slice(slice: &[f32]) -> Self       // slice len >= 4
```

### Methods

```rust
pub fn transpose(&self) -> Self
pub fn determinant(&self) -> f32
pub fn inverse(&self) -> Self                 // panics if not invertible
pub fn try_inverse(&self) -> Option<Self>     // (verify) presence
pub fn inverse_or_zero(&self) -> Self          // (verify) presence
pub fn diagonal(&self) -> Vec2
pub fn mul_vec2(&self, rhs: Vec2) -> Vec2
pub fn mul_transpose_vec2(&self, rhs: Vec2) -> Vec2
pub fn mul_mat2(&self, rhs: &Self) -> Self
pub fn add_mat2(&self, rhs: &Self) -> Self
pub fn sub_mat2(&self, rhs: &Self) -> Self
pub fn mul_scalar(&self, rhs: f32) -> Self
pub fn mul_diagonal_scale(&self, scale: Vec2) -> Self  // (verify)
pub fn div_scalar(&self, rhs: f32) -> Self
pub fn recip(&self) -> Self
pub fn abs(&self) -> Self
pub fn col(&self, index: usize) -> Vec2
pub fn col_mut(&mut self, index: usize) -> &mut Vec2
pub fn row(&self, index: usize) -> Vec2
pub fn is_finite(&self) -> bool
pub fn is_nan(&self) -> bool
pub fn abs_diff_eq(&self, rhs: Self, max_abs_diff: f32) -> bool
pub const fn to_cols_array(&self) -> [f32; 4]
pub const fn to_cols_array_2d(&self) -> [[f32; 2]; 2]
pub fn write_cols_to_slice(&self, slice: &mut [f32])
pub fn as_dmat2(&self) -> DMat2
```

### Operators

```rust
Mul<Mat2> for Mat2  -> Mat2   // matrix product
Mul<Vec2> for Mat2  -> Vec2   // m * v, column transform
Mul<f32>  for Mat2  -> Mat2   // and Mul<Mat2> for f32 -> Mat2
Div<f32>  for Mat2  -> Mat2   // and Div<Mat2> for f32 -> Mat2
Add       for Mat2  -> Mat2
Sub       for Mat2  -> Mat2
Neg       for Mat2  -> Mat2
// All also impl'd for &-borrowed lhs/rhs combinations.
// AddAssign, SubAssign, MulAssign<Mat2|f32>, DivAssign<f32>. Sum, Product.
```

---

## Mat3

3x3 column-major, columns `Vec3`. Two roles: 2D affine (translation in 3rd column, use
`transform_point2`) or 3D linear/rotation.

### Constants

```rust
pub const ZERO: Self;
pub const IDENTITY: Self;
pub const NAN: Self;
```

### Constructors

```rust
pub const fn from_cols(x_axis: Vec3, y_axis: Vec3, z_axis: Vec3) -> Self
pub const fn from_cols_array(m: &[f32; 9]) -> Self
pub const fn from_cols_array_2d(m: &[[f32; 3]; 3]) -> Self
pub const fn from_diagonal(diagonal: Vec3) -> Self
pub fn from_mat4(m: Mat4) -> Self                 // top-left 3x3
pub fn from_mat4_minor(m: Mat4, i: usize, j: usize) -> Self
pub fn from_quat(rotation: Quat) -> Self
pub fn from_axis_angle(axis: Vec3, angle: f32) -> Self     // axis normalized
pub fn from_euler(order: EulerRot, a: f32, b: f32, c: f32) -> Self
pub fn from_rotation_x(angle: f32) -> Self
pub fn from_rotation_y(angle: f32) -> Self
pub fn from_rotation_z(angle: f32) -> Self
pub fn from_translation(translation: Vec2) -> Self          // 2D affine translation
pub fn from_angle(angle: f32) -> Self                       // 2D rotation, radians
pub fn from_scale_angle_translation(scale: Vec2, angle: f32, translation: Vec2) -> Self
pub fn from_scale(scale: Vec2) -> Self                      // 2D affine scale
pub fn from_mat2(m: Mat2) -> Self
pub const fn from_cols_slice(slice: &[f32]) -> Self
pub fn look_to_lh(dir: Vec3, up: Vec3) -> Self
pub fn look_to_rh(dir: Vec3, up: Vec3) -> Self
pub fn look_at_lh(eye: Vec3, center: Vec3, up: Vec3) -> Self
pub fn look_at_rh(eye: Vec3, center: Vec3, up: Vec3) -> Self
```

### Methods

```rust
pub fn col(&self, index: usize) -> Vec3
pub fn col_mut(&mut self, index: usize) -> &mut Vec3
pub fn row(&self, index: usize) -> Vec3
pub fn transpose(&self) -> Self
pub fn determinant(&self) -> f32
pub fn diagonal(&self) -> Vec3
pub fn inverse(&self) -> Self
pub fn try_inverse(&self) -> Option<Self>
pub fn inverse_or_zero(&self) -> Self
pub fn is_finite(&self) -> bool
pub fn is_nan(&self) -> bool
pub fn mul_vec3(&self, rhs: Vec3) -> Vec3
pub fn mul_vec3a(&self, rhs: Vec3A) -> Vec3A
pub fn mul_transpose_vec3(&self, rhs: Vec3) -> Vec3
pub fn mul_mat3(&self, rhs: &Self) -> Self
pub fn add_mat3(&self, rhs: &Self) -> Self
pub fn sub_mat3(&self, rhs: &Self) -> Self
pub fn mul_scalar(&self, rhs: f32) -> Self
pub fn mul_diagonal_scale(&self, scale: Vec3) -> Self
pub fn div_scalar(&self, rhs: f32) -> Self
pub fn recip(&self) -> Self
pub fn transform_point2(&self, rhs: Vec2) -> Vec2   // applies translation (3rd col)
pub fn transform_vector2(&self, rhs: Vec2) -> Vec2  // ignores translation
pub fn to_cols_array(&self) -> [f32; 9]
pub fn to_cols_array_2d(&self) -> [[f32; 3]; 3]
pub fn write_cols_to_slice(&self, slice: &mut [f32])
pub fn to_euler(&self, order: EulerRot) -> (f32, f32, f32)
pub fn abs_diff_eq(&self, rhs: Self, max_abs_diff: f32) -> bool
pub fn abs(&self) -> Self
pub fn as_dmat3(&self) -> DMat3
```

### Operators

```rust
Mul<Mat3>    for Mat3 -> Mat3
Mul<Vec3>    for Mat3 -> Vec3
Mul<Vec3A>   for Mat3 -> Vec3A
Mul<f32>     for Mat3 -> Mat3   // and Mul<Mat3> for f32 -> Mat3
Mul<Affine2> for Mat3 -> Mat3   // and Mul<Mat3> for Affine2 -> Mat3
Div<f32>     for Mat3 -> Mat3   // and Div<Mat3> for f32 -> Mat3
Add          for Mat3 -> Mat3
Sub          for Mat3 -> Mat3
Neg          for Mat3 -> Mat3
// &-borrowed combinations for all of the above.
// MulAssign<Mat3|f32|Affine2>, AddAssign, SubAssign, DivAssign<f32>. Sum, Product.
// From<Affine2>, From<Mat3A>.
```

---

## Mat3A

Same math as `Mat3`, columns are `Vec3A` (16-byte aligned, SIMD). Use for hot 3D
linear-transform loops; `Mat3` for tight storage / 2D affine. Convert via `From`.

### Constants

```rust
pub const ZERO: Self;
pub const IDENTITY: Self;
pub const NAN: Self;
```

### Constructors

Same set and signatures as `Mat3`, except `from_cols` takes `Vec3A` columns:

```rust
pub const fn from_cols(x_axis: Vec3A, y_axis: Vec3A, z_axis: Vec3A) -> Self
pub const fn from_cols_array(m: &[f32; 9]) -> Self
pub const fn from_cols_array_2d(m: &[[f32; 3]; 3]) -> Self
pub const fn from_diagonal(diagonal: Vec3) -> Self
pub fn from_mat4(m: Mat4) -> Self
pub fn from_mat4_minor(m: Mat4, i: usize, j: usize) -> Self
pub fn from_quat(rotation: Quat) -> Self
pub fn from_axis_angle(axis: Vec3, angle: f32) -> Self
pub fn from_euler(order: EulerRot, a: f32, b: f32, c: f32) -> Self
pub fn from_rotation_x(angle: f32) -> Self
pub fn from_rotation_y(angle: f32) -> Self
pub fn from_rotation_z(angle: f32) -> Self
pub fn from_translation(translation: Vec2) -> Self
pub fn from_angle(angle: f32) -> Self
pub fn from_scale_angle_translation(scale: Vec2, angle: f32, translation: Vec2) -> Self
pub fn from_scale(scale: Vec2) -> Self
pub fn from_mat2(m: Mat2) -> Self
pub const fn from_cols_slice(slice: &[f32]) -> Self
pub fn look_to_lh(dir: Vec3, up: Vec3) -> Self
pub fn look_to_rh(dir: Vec3, up: Vec3) -> Self
pub fn look_at_lh(eye: Vec3, center: Vec3, up: Vec3) -> Self
pub fn look_at_rh(eye: Vec3, center: Vec3, up: Vec3) -> Self
```

### Methods

```rust
pub const fn to_cols_array(&self) -> [f32; 9]
pub const fn to_cols_array_2d(&self) -> [[f32; 3]; 3]
pub fn col(&self, index: usize) -> Vec3A
pub fn col_mut(&mut self, index: usize) -> &mut Vec3A
pub fn row(&self, index: usize) -> Vec3A
pub fn is_finite(&self) -> bool
pub fn is_nan(&self) -> bool
pub fn transpose(&self) -> Self
pub fn diagonal(&self) -> Vec3A
pub fn determinant(&self) -> f32
pub fn inverse(&self) -> Self
pub fn try_inverse(&self) -> Option<Self>
pub fn inverse_or_zero(&self) -> Self
pub fn transform_point2(&self, rhs: Vec2) -> Vec2
pub fn transform_vector2(&self, rhs: Vec2) -> Vec2
pub fn mul_vec3(&self, rhs: Vec3) -> Vec3
pub fn mul_vec3a(&self, rhs: Vec3A) -> Vec3A
pub fn mul_transpose_vec3(&self, rhs: Vec3) -> Vec3
pub fn mul_transpose_vec3a(&self, rhs: Vec3A) -> Vec3A
pub fn mul_mat3(&self, rhs: &Self) -> Self
pub fn add_mat3(&self, rhs: &Self) -> Self
pub fn sub_mat3(&self, rhs: &Self) -> Self
pub fn mul_scalar(&self, rhs: f32) -> Self
pub fn mul_diagonal_scale(&self, scale: Vec3) -> Self
pub fn div_scalar(&self, rhs: f32) -> Self
pub fn recip(&self) -> Self
pub fn abs_diff_eq(&self, rhs: Self, max_abs_diff: f32) -> bool
pub fn abs(&self) -> Self
pub fn to_euler(&self, order: EulerRot) -> (f32, f32, f32)
pub fn write_cols_to_slice(&self, slice: &mut [f32])
pub fn as_dmat3(&self) -> DMat3
```

### Operators

```rust
Mul<Mat3A>  for Mat3A -> Mat3A
Mul<Vec3>   for Mat3A -> Vec3
Mul<Vec3A>  for Mat3A -> Vec3A
Mul<f32>    for Mat3A -> Mat3A   // and Mul<Mat3A> for f32 -> Mat3A
Div<f32>    for Mat3A -> Mat3A   // and Div<Mat3A> for f32 -> Mat3A
Add         for Mat3A -> Mat3A
Sub         for Mat3A -> Mat3A
Neg         for Mat3A -> Mat3A
// AddAssign, SubAssign, MulAssign, DivAssign. Sum, Product.
```

---

## Mat4

4x4 column-major, columns `Vec4`. 3D affine (rotation + scale + translation) and
projection. The MVP matrix type.

### Constants

```rust
pub const ZERO: Self;
pub const IDENTITY: Self;
pub const NAN: Self;
```

### Constructors

```rust
pub const fn from_cols(x_axis: Vec4, y_axis: Vec4, z_axis: Vec4, w_axis: Vec4) -> Self
pub const fn from_cols_array(m: &[f32; 16]) -> Self
pub const fn from_cols_array_2d(m: &[[f32; 4]; 4]) -> Self
pub const fn from_diagonal(diagonal: Vec4) -> Self
pub fn from_quat(rotation: Quat) -> Self
pub fn from_axis_angle(axis: Vec3, angle: f32) -> Self
pub fn from_euler(order: EulerRot, a: f32, b: f32, c: f32) -> Self
pub fn from_rotation_x(angle: f32) -> Self
pub fn from_rotation_y(angle: f32) -> Self
pub fn from_rotation_z(angle: f32) -> Self
pub fn from_rotation_translation(rotation: Quat, translation: Vec3) -> Self
pub fn from_scale_rotation_translation(scale: Vec3, rotation: Quat, translation: Vec3) -> Self
pub fn from_translation(translation: Vec3) -> Self
pub fn from_scale(scale: Vec3) -> Self
pub fn from_mat3(m: Mat3) -> Self
pub fn from_mat3a(m: Mat3A) -> Self
pub fn from_mat3_translation(mat3: Mat3, translation: Vec3) -> Self
pub fn from_cols_slice(slice: &[f32]) -> Self
```

### Projection and view constructors

```rust
pub fn perspective_rh(fov_y_radians: f32, aspect_ratio: f32, z_near: f32, z_far: f32) -> Self
pub fn perspective_lh(fov_y_radians: f32, aspect_ratio: f32, z_near: f32, z_far: f32) -> Self
pub fn perspective_rh_gl(fov_y_radians: f32, aspect_ratio: f32, z_near: f32, z_far: f32) -> Self
pub fn perspective_infinite_rh(fov_y_radians: f32, aspect_ratio: f32, z_near: f32) -> Self
pub fn perspective_infinite_lh(fov_y_radians: f32, aspect_ratio: f32, z_near: f32) -> Self
pub fn perspective_infinite_reverse_rh(fov_y_radians: f32, aspect_ratio: f32, z_near: f32) -> Self
pub fn perspective_infinite_reverse_lh(fov_y_radians: f32, aspect_ratio: f32, z_near: f32) -> Self
pub fn orthographic_rh(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Self
pub fn orthographic_lh(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Self
pub fn orthographic_rh_gl(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Self
pub fn look_at_rh(eye: Vec3, center: Vec3, up: Vec3) -> Self
pub fn look_at_lh(eye: Vec3, center: Vec3, up: Vec3) -> Self
pub fn look_to_rh(eye: Vec3, dir: Vec3, up: Vec3) -> Self
pub fn look_to_lh(eye: Vec3, dir: Vec3, up: Vec3) -> Self
pub fn frustum_rh(left: f32, right: f32, bottom: f32, top: f32, z_near: f32, z_far: f32) -> Self
pub fn frustum_lh(left: f32, right: f32, bottom: f32, top: f32, z_near: f32, z_far: f32) -> Self
pub fn frustum_rh_gl(left: f32, right: f32, bottom: f32, top: f32, z_near: f32, z_far: f32) -> Self
```

Depth range (NDC z):

| Function | Depth range | Use with wgpu |
|----------|-------------|---------------|
| `perspective_rh`, `perspective_lh` | `0..1` | yes |
| `perspective_rh_gl` | `-1..1` (OpenGL) | no |
| `perspective_infinite_rh` / `_lh` | `0..1`, far = inf | yes |
| `perspective_infinite_reverse_rh` / `_lh` | `0..1` reversed (1 near, 0 far) | yes, good z-precision |
| `orthographic_rh`, `orthographic_lh` | `0..1` | yes |
| `orthographic_rh_gl` | `-1..1` (OpenGL) | no |
| `frustum_rh`, `frustum_lh` | `0..1` | yes |
| `frustum_rh_gl` | `-1..1` (OpenGL) | no |

wgpu clip space is `0..1` depth: use the non-`_gl` constructors. The `_gl` variants are
for OpenGL's `-1..1` NDC and will misclip on wgpu.

### Methods

```rust
pub fn transpose(&self) -> Self
pub fn determinant(&self) -> f32
pub fn inverse(&self) -> Self
pub fn try_inverse(&self) -> Option<Self>
pub fn inverse_or_zero(&self) -> Self
pub fn transform_point3(&self, rhs: Vec3) -> Vec3    // applies translation (w=1)
pub fn transform_vector3(&self, rhs: Vec3) -> Vec3   // ignores translation (w=0)
pub fn transform_point3a(&self, rhs: Vec3A) -> Vec3A
pub fn transform_vector3a(&self, rhs: Vec3A) -> Vec3A
pub fn project_point3(&self, rhs: Vec3) -> Vec3      // transform + perspective divide
pub fn project_point3a(&self, rhs: Vec3A) -> Vec3A
pub fn mul_vec4(&self, rhs: Vec4) -> Vec4
pub fn mul_transpose_vec4(&self, rhs: Vec4) -> Vec4
pub fn mul_mat4(&self, rhs: &Self) -> Self
pub fn add_mat4(&self, rhs: &Self) -> Self
pub fn sub_mat4(&self, rhs: &Self) -> Self
pub fn mul_scalar(&self, rhs: f32) -> Self
pub fn mul_diagonal_scale(&self, scale: Vec4) -> Self
pub fn div_scalar(&self, rhs: f32) -> Self
pub fn recip(&self) -> Self
pub fn col(&self, index: usize) -> Vec4
pub fn col_mut(&mut self, index: usize) -> &mut Vec4
pub fn row(&self, index: usize) -> Vec4
pub fn diagonal(&self) -> Vec4
pub fn is_finite(&self) -> bool
pub fn is_nan(&self) -> bool
pub fn abs_diff_eq(&self, rhs: Self, max_abs_diff: f32) -> bool
pub fn abs(&self) -> Self
pub fn to_cols_array(&self) -> [f32; 16]      // column order, ready for GPU upload
pub fn to_cols_array_2d(&self) -> [[f32; 4]; 4]
pub fn to_euler(&self, order: EulerRot) -> (f32, f32, f32)
pub fn to_scale_rotation_translation(&self) -> (Vec3, Quat, Vec3)
pub fn write_cols_to_slice(&self, slice: &mut [f32])
pub fn as_dmat4(&self) -> DMat4
```

### Operators

```rust
Mul<Mat4>  for Mat4 -> Mat4   // matrix product; a * b applies b first
Mul<Vec4>  for Mat4 -> Vec4   // m * v
Mul<f32>   for Mat4 -> Mat4   // and Mul<Mat4> for f32 -> Mat4
Div<f32>   for Mat4 -> Mat4   // and Div<Mat4> for f32 -> Mat4
Add        for Mat4 -> Mat4
Sub        for Mat4 -> Mat4
Neg        for Mat4 -> Mat4
// &-borrowed combinations for all of the above.
// MulAssign<Mat4|f32>, AddAssign, SubAssign, DivAssign<f32>.
// From<Affine3A> for Mat4, From<Affine3> for Mat4 (verify Affine3 name).
```

---

## Common patterns

```rust
use glam::{Mat3, Mat4, Quat, Vec2, Vec3};

// --- 3D MVP for wgpu (0..1 NDC depth) ---
// Right-handed view-space, wgpu clip space. Use the non-_gl projection.
let proj = Mat4::perspective_rh(
    60.0_f32.to_radians(), // fov_y
    width / height,        // aspect
    0.1,                   // z_near
    1000.0,                // z_far
);
let view = Mat4::look_at_rh(
    Vec3::new(0.0, 2.0, 5.0), // eye
    Vec3::ZERO,               // target
    Vec3::Y,                  // up
);
// Object transform: scale, then rotate, then translate (single call composes them).
let model = Mat4::from_scale_rotation_translation(
    Vec3::splat(1.0),
    Quat::from_rotation_y(0.5),
    Vec3::new(0.0, 0.0, 0.0),
);
// Right-to-left: model applied first, then view, then projection.
let mvp = proj * view * model;
// Upload column-major; no transpose needed for WGSL mat4x4<f32>.
let mvp_cols: [f32; 16] = mvp.to_cols_array();

// Transform a model-space point into world space.
let world = model.transform_point3(Vec3::new(1.0, 0.0, 0.0));

// --- 2D pixel canvas for wgpu (orthographic, 0..1 depth) ---
// Map a (w x h) pixel rect to clip space. Top-left origin: top < bottom flips Y.
let canvas = Mat4::orthographic_rh(
    0.0,    // left
    width,  // right
    height, // bottom
    0.0,    // top  (top < bottom => y grows downward, screen convention)
    -1.0,   // near
    1.0,    // far
);

// --- 2D affine via Mat3 ---
// Scale + rotate + translate in one call; apply with transform_point2.
let xform = Mat3::from_scale_angle_translation(
    Vec2::new(2.0, 2.0),     // scale
    0.25,                    // angle, radians
    Vec2::new(100.0, 50.0),  // translation, pixels
);
let p = xform.transform_point2(Vec2::new(10.0, 10.0)); // includes translation
let d = xform.transform_vector2(Vec2::new(1.0, 0.0));  // direction, no translation
```
