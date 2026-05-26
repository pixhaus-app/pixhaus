# glam 0.33.0 — Quat, Affine2, Affine3A, EulerRot

Reference for `glam::f32`. Signatures verbatim from docs.rs 0.33.0. Items marked
"(verify)" need a doc check before relying on them.

## Quat

Orientation as a unit quaternion (3D rotation). Stored as `xyzw`. A `Quat` is only
a valid rotation while normalized — most constructors return a unit quaternion, but
arithmetic (`+`, `-`, `* f32`, `lerp`) does not preserve unit length, so renormalize
after blending. `q * vec3` rotates the vector; `q1 * q2` composes rotations and
applies right-to-left (`q2` first, then `q1`).

### Constants

```rust
pub const IDENTITY: Self  // no rotation, (0,0,0,1)
pub const NAN: Self
```

### Constructors

```rust
pub const fn from_xyzw(x: f32, y: f32, z: f32, w: f32) -> Self
pub const fn from_array(a: [f32; 4]) -> Self      // [x, y, z, w]
pub const fn from_vec4(v: Vec4) -> Self
pub fn from_slice(slice: &[f32]) -> Self           // reads 4, panics if shorter

pub fn from_axis_angle(axis: Vec3, angle: f32) -> Self   // axis must be normalized
pub fn from_scaled_axis(v: Vec3) -> Self                 // axis * angle (rad)
pub fn from_rotation_x(angle: f32) -> Self
pub fn from_rotation_y(angle: f32) -> Self
pub fn from_rotation_z(angle: f32) -> Self
pub fn from_euler(euler: EulerRot, a: f32, b: f32, c: f32) -> Self

pub fn from_mat3(mat: &Mat3) -> Self
pub fn from_mat3a(mat: &Mat3A) -> Self
pub fn from_mat4(mat: &Mat4) -> Self               // uses upper 3x3
pub fn from_rotation_axes(x_axis: Vec3, y_axis: Vec3, z_axis: Vec3) -> Self

pub fn from_rotation_arc(from: Vec3, to: Vec3) -> Self        // shortest arc, unit inputs
pub fn from_rotation_arc_colinear(from: Vec3, to: Vec3) -> Self
pub fn from_rotation_arc_2d(from: Vec2, to: Vec2) -> Self     // rotation about Z

pub fn from_affine3a(a: &Affine3A) -> Self                    // (verify) extracts rotation
```

### Methods

```rust
pub fn normalize(self) -> Self
pub fn is_normalized(self) -> bool
pub fn conjugate(self) -> Self            // negates xyz; inverse for unit quats
pub fn inverse(self) -> Self              // assumes normalized
pub fn dot(self, rhs: Self) -> f32
pub fn length(self) -> f32
pub fn length_squared(self) -> f32
pub fn length_recip(self) -> f32

pub fn mul_quat(self, rhs: Self) -> Self  // == self * rhs, compose (rhs applied first)
pub fn mul_vec3(self, rhs: Vec3) -> Vec3  // == self * rhs, rotate vector
pub fn mul_vec3a(self, rhs: Vec3A) -> Vec3A

pub fn lerp(self, end: Self, s: f32) -> Self    // nlerp; renormalizes
pub fn slerp(self, end: Self, s: f32) -> Self   // spherical, constant angular velocity
pub fn rotate_towards(self, rhs: Self, max_angle: f32) -> Self

pub fn to_euler(self, order: EulerRot) -> (f32, f32, f32)
pub fn to_array(self) -> [f32; 4]
pub fn to_axis_angle(self) -> (Vec3, f32)
pub fn to_scaled_axis(self) -> Vec3
pub fn xyz(self) -> Vec3

pub fn angle_between(self, rhs: Self) -> f32
pub fn is_near_identity(self) -> bool
pub fn is_finite(self) -> bool
pub fn is_nan(self) -> bool
pub fn abs_diff_eq(self, rhs: Self, max_abs_diff: f32) -> bool
pub fn write_to_slice(self, slice: &mut [f32])
```

### Operators

```rust
Quat * Quat   -> Quat   // compose, right-to-left
Quat * Vec3   -> Vec3   // rotate vector
Quat * Vec3A  -> Vec3A
Quat * f32    -> Quat   // scales components, breaks unit length
Quat + Quat   -> Quat   // component add, breaks unit length
Quat - Quat   -> Quat
Quat / f32    -> Quat
-Quat         -> Quat   // negation; represents the same rotation
```

`MulAssign`, `AddAssign`, `SubAssign`, `DivAssign` mirror the above. Ref variants
(`&Quat`, `&f32`, `&Vec3`, ...) exist for every `Mul`/`Add`/`Sub`/`Div`/`Neg`.
`Deref<Target = Vec4>` exposes `.x/.y/.z/.w`.

## Affine2

2D affine transform: `matrix2: Mat2` (linear part — scale/rotate/shear) plus
`translation: Vec2`. Cheaper and more compact than a full `Mat3` for 2D.

```rust
pub struct Affine2 {
    pub matrix2: Mat2,
    pub translation: Vec2,
}
```

### Constants

```rust
pub const IDENTITY: Self
pub const ZERO: Self
pub const NAN: Self
```

### Constructors

```rust
pub const fn from_cols(x_axis: Vec2, y_axis: Vec2, z_axis: Vec2) -> Self
                                                  // z_axis is the translation column
pub fn from_cols_array(m: &[f32; 6]) -> Self
pub fn from_cols_array_2d(m: &[[f32; 2]; 3]) -> Self
pub fn from_cols_slice(slice: &[f32]) -> Self

pub fn from_scale(scale: Vec2) -> Self
pub fn from_angle(angle: f32) -> Self                     // CCW rotation (rad)
pub fn from_translation(translation: Vec2) -> Self
pub fn from_angle_translation(angle: f32, translation: Vec2) -> Self
pub fn from_mat2(matrix2: Mat2) -> Self
pub fn from_mat2_translation(matrix2: Mat2, translation: Vec2) -> Self
pub fn from_scale_angle_translation(scale: Vec2, angle: f32, translation: Vec2) -> Self
                                                  // applied scale -> rotate -> translate
pub fn from_mat3(m: Mat3) -> Self                 // (verify) drops projective row
pub fn from_mat3a(m: Mat3A) -> Self               // (verify)
```

### Methods

```rust
pub fn transform_point2(&self, rhs: Vec2) -> Vec2    // applies matrix2 then + translation
pub fn transform_vector2(&self, rhs: Vec2) -> Vec2   // matrix2 only, ignores translation
pub fn inverse(&self) -> Self
pub fn to_scale_angle_translation(&self) -> (Vec2, f32, Vec2)
pub fn to_cols_array(&self) -> [f32; 6]
pub fn to_cols_array_2d(&self) -> [[f32; 2]; 3]
pub fn write_cols_to_slice(&self, slice: &mut [f32])
pub fn abs_diff_eq(&self, rhs: Self, max_abs_diff: f32) -> bool
pub fn is_finite(&self) -> bool
pub fn is_nan(&self) -> bool
pub fn as_daffine2(&self) -> DAffine2
```

Use `transform_point2` for positions (translated), `transform_vector2` for
directions/deltas (not translated).

### Operators

```rust
Affine2 * Affine2 -> Affine2   // compose, right-to-left
Affine2 * Mat3    -> Mat3
Affine2 * Mat3A   -> Mat3A
```

`MulAssign<Affine2>` for `Affine2`, `Mat3`, `Mat3A`. Ref variants exist.

## Affine3A

3D affine transform: `matrix3: Mat3A` (linear part) plus `translation: Vec3A`.
Cheaper than `Mat4` for any non-projective transform — it omits the perspective
row, so it cannot represent a perspective projection. Use it for model/world
transforms; reach for `Mat4` only when you need projection.

```rust
pub struct Affine3A {
    pub matrix3: Mat3A,
    pub translation: Vec3A,
}
```

### Constants

```rust
pub const IDENTITY: Self
pub const ZERO: Self
pub const NAN: Self
```

### Constructors

```rust
pub const fn from_cols(x_axis: Vec3A, y_axis: Vec3A, z_axis: Vec3A, w_axis: Vec3A) -> Self
                                                  // w_axis is the translation column

pub fn from_scale(scale: Vec3) -> Self
pub fn from_quat(rotation: Quat) -> Self
pub fn from_axis_angle(axis: Vec3, angle: f32) -> Self
pub fn from_rotation_x(angle: f32) -> Self
pub fn from_rotation_y(angle: f32) -> Self
pub fn from_rotation_z(angle: f32) -> Self
pub fn from_translation(translation: Vec3) -> Self
pub fn from_mat3(mat3: Mat3) -> Self
pub fn from_mat3_translation(mat3: Mat3, translation: Vec3) -> Self
pub fn from_scale_rotation_translation(scale: Vec3, rotation: Quat, translation: Vec3) -> Self
                                                  // TRS: scale -> rotate -> translate
pub fn from_rotation_translation(rotation: Quat, translation: Vec3) -> Self
pub fn from_mat4(m: Mat4) -> Self                 // drops projective row

pub fn look_to_lh(eye: Vec3, dir: Vec3, up: Vec3) -> Self
pub fn look_to_rh(eye: Vec3, dir: Vec3, up: Vec3) -> Self
pub fn look_at_lh(eye: Vec3, center: Vec3, up: Vec3) -> Self
pub fn look_at_rh(eye: Vec3, center: Vec3, up: Vec3) -> Self
```

### Methods

```rust
pub fn transform_point3(&self, rhs: Vec3) -> Vec3      // matrix3 then + translation
pub fn transform_vector3(&self, rhs: Vec3) -> Vec3     // matrix3 only, no translation
pub fn transform_point3a(&self, rhs: Vec3A) -> Vec3A
pub fn transform_vector3a(&self, rhs: Vec3A) -> Vec3A
pub fn inverse(&self) -> Self
pub fn to_scale_rotation_translation(&self) -> (Vec3, Quat, Vec3)
pub fn to_cols_array(&self) -> [f32; 12]
pub fn to_cols_array_2d(&self) -> [[f32; 3]; 4]
pub fn abs_diff_eq(&self, rhs: Self, max_abs_diff: f32) -> bool
pub fn is_finite(&self) -> bool
pub fn is_nan(&self) -> bool
```

`transform_point3` translates; `transform_vector3` does not.

### Operators

```rust
Affine3A * Affine3A -> Affine3A   // compose, right-to-left
Affine3A * Mat4     -> Mat4
Mat4     * Affine3A -> Mat4
```

All four owned/ref combinations exist for each `Mul`.

## EulerRot

Parameterizes the rotation order for `Quat::from_euler` / `Quat::to_euler` (and the
matrix `from_euler`/`to_euler`). Three angles `(a, b, c)` apply about the axes named
by the variant, in left-to-right order. Variants with no suffix are intrinsic
(each rotation is about the moving body's axes); variants ending in `Ex` are
extrinsic (each rotation is about the original fixed axes). Three-axis variants
(e.g. `XYZ`) cover all distinct axes; two-axis variants (e.g. `ZYZ`) repeat the
first axis last.

Default: `EulerRot::YXZ` — yaw (Y), pitch (X), roll (Z).

```rust
pub enum EulerRot {
    // intrinsic, three-axis
    ZYX, ZXY, YXZ, YZX, XYZ, XZY,
    // intrinsic, two-axis
    ZYZ, ZXZ, YXY, YZY, XYX, XZX,
    // extrinsic, three-axis
    ZYXEx, ZXYEx, YXZEx, YZXEx, XYZEx, XZYEx,
    // extrinsic, two-axis
    ZYZEx, ZXZEx, YXYEx, YZYEx, XYXEx, XZXEx,
}

impl Default for EulerRot {
    fn default() -> Self { EulerRot::YXZ }
}
```

## Common patterns

```rust
use glam::{Affine2, Affine3A, EulerRot, Mat2, Quat, Vec2, Vec3};

// Compose a rotation from Euler angles (yaw, pitch, roll), then renormalize.
let yaw = 0.5_f32;
let pitch = 0.2_f32;
let roll = 0.0_f32;
let rot = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll).normalize();

// Slerp between two orientations.
let a = Quat::from_rotation_y(0.0);
let b = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
let mid = a.slerp(b, 0.5); // halfway, constant angular velocity

// Compose: apply `rot` then `mid`. Right-to-left, so `rot` is applied first.
let combined = mid * rot;

// Build an Affine3A TRS and transform a point.
let trs = Affine3A::from_scale_rotation_translation(
    Vec3::splat(2.0),     // scale
    rot,                  // rotation
    Vec3::new(1.0, 0.0, 3.0), // translation
);
let world = trs.transform_point3(Vec3::new(1.0, 0.0, 0.0)); // scaled, rotated, translated
let dir = trs.transform_vector3(Vec3::X);                   // rotated + scaled, not translated

// 2D sprite transform: scale, rotate about center, place on screen.
let sprite = Affine2::from_scale_angle_translation(
    Vec2::new(32.0, 32.0),         // scale (e.g. to pixel size)
    std::f32::consts::FRAC_PI_4,   // 45 deg CCW
    Vec2::new(120.0, 80.0),        // screen position
);
let corner = sprite.transform_point2(Vec2::new(0.5, 0.5)); // unit-quad corner -> screen px
let inv = sprite.inverse();                                 // screen px -> sprite-local
```
