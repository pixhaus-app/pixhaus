# glam 0.33 — other numeric families

What changes when you leave `f32`. glam mirrors the same `Vec2/3/4` (and
`Mat*`/`Affine*`/`Quat` for `f64`) shape across every numeric family. The type
names change by prefix, the element type changes, and a handful of methods
appear or disappear depending on whether the element type can represent
fractions and sign. This file covers the integer, unsigned, bool, and `f64`
families and — most importantly for Pixhaus — the cross-family `as_*` casts that
move pixel coords (`UVec2`/`IVec2`) to and from world coords (`Vec2`).

For the full `f32` vector/matrix/quat API, see the vectors, matrices, and quat
references. Those signatures apply unchanged to `f64` with `f64` elements.

## Family roster

Each family is `Vec2`/`Vec3`/`Vec4` over one element type. `f32` and `f64` add
matrices, a quaternion, and affines; the integer/bool families are vectors only.

| Family | Element | Module | Vector names | Extra types | Always on? |
| --- | --- | --- | --- | --- | --- |
| f32 | `f32` | `glam::f32` | `Vec2` `Vec3` `Vec3A` `Vec4` | `Mat2` `Mat3` `Mat3A` `Mat4` `Quat` `Affine2` `Affine3` `Affine3A` | yes (always) |
| f64 | `f64` | `glam::f64` | `DVec2` `DVec3` `DVec4` | `DMat2` `DMat3` `DMat4` `DQuat` `DAffine2` `DAffine3` | yes (default `float-types`) |
| bool | `bool` | `glam::bool` | `BVec2` `BVec3` `BVec4` (+ `BVec3A` `BVec4A`) | mask type, no math | yes (always) |
| i32 | `i32` | `glam::i32` | `IVec2` `IVec3` `IVec4` | — | `integer-types` |
| u32 | `u32` | `glam::u32` | `UVec2` `UVec3` `UVec4` | — | `integer-types` |
| i8 | `i8` | `glam::i8` | `I8Vec2` `I8Vec3` `I8Vec4` | — | `integer-types` |
| i16 | `i16` | `glam::i16` | `I16Vec2` `I16Vec3` `I16Vec4` | — | `integer-types` |
| i64 | `i64` | `glam::i64` | `I64Vec2` `I64Vec3` `I64Vec4` | — | `integer-types` |
| u8 | `u8` | `glam::u8` | `U8Vec2` `U8Vec3` `U8Vec4` | — | `integer-types` |
| u16 | `u16` | `glam::u16` | `U16Vec2` `U16Vec3` `U16Vec4` | — | `integer-types` |
| u64 | `u64` | `glam::u64` | `U64Vec2` `U64Vec3` `U64Vec4` | — | `integer-types` |
| isize | `isize` | `glam::isize` | `ISizeVec2` `ISizeVec3` `ISizeVec4` | — | `size-types` |
| usize | `usize` | `glam::usize` | `USizeVec2` `USizeVec3` `USizeVec4` | — | `size-types` |

Feature gating (verify against your `Cargo.toml` features):

- `f32` and `bool` are always compiled — no feature flag.
- `f64` is the `float-types` feature, which is on in the default feature set.
- All integer families (`i8`/`i16`/`i32`/`i64`, `u8`/`u16`/`u32`/`u64`) live
  behind `integer-types`. The names `IVec*`/`UVec*` are not special-cased; they
  gate with the rest.
- `isize`/`usize` (`ISizeVec*`/`USizeVec*`) live behind `size-types`.
- `all-types` turns on every optional family at once (`float-types` +
  `integer-types` + `size-types`).

If you reach for `IVec2` and the compiler says it does not exist, you are
missing `integer-types` in your glam features. Pixhaus needs both float and
integer families, so enable at least `integer-types` (`float-types` comes by
default).

## Integer vectors — `IVec*` (signed) and `UVec*` (unsigned)

Integer vectors carry the arithmetic and ordering API but drop everything that
needs a fraction. The split is mechanical: anything whose result is generally
not an integer is gone.

### What carries over from `f32`

Using `IVec2` as the reference; `IVec3`/`IVec4` and the `u32` equivalents match
modulo dimension and signedness.

```rust
// Arithmetic — element-wise, plus scalar variants against the element type.
fn add(self, rhs: Self) -> Self          // + - * / % and *Assign, vs Self and vs i32
// Ordering / bounds
fn min(self, rhs: Self) -> Self
fn max(self, rhs: Self) -> Self
fn clamp(self, min: Self, max: Self) -> Self
fn min_element(self) -> i32
fn max_element(self) -> i32
fn min_position(self) -> usize           // (verify) index of the min element
fn max_position(self) -> usize           // (verify)
// Sign — SIGNED ONLY (IVec*). Absent on UVec*.
fn abs(self) -> Self
fn signum(self) -> Self
fn is_negative_bitmask(self) -> u32
// Products
fn dot(self, rhs: Self) -> i32
fn dot_into_vec(self, rhs: Self) -> Self
fn length_squared(self) -> i32           // squared only — no float root needed
fn distance_squared(self, rhs: Self) -> i32
fn element_sum(self) -> i32
fn element_product(self) -> i32
// Euclidean division / remainder
fn div_euclid(self, rhs: Self) -> Self
fn rem_euclid(self, rhs: Self) -> Self
// Integer-grid distances (return unsigned)
fn manhattan_distance(self, rhs: Self) -> u32
fn checked_manhattan_distance(self, rhs: Self) -> Option<u32>
fn chebyshev_distance(self, rhs: Self) -> u32
// 2D-only rotation helpers (IVec2/UVec2)
fn perp(self) -> Self                    // (verify; signed)
fn perp_dot(self, rhs: Self) -> i32      // (verify; signed)
fn rotate(self, rhs: Self) -> Self       // (verify)
// 3D-only
fn cross(self, rhs: Self) -> Self        // IVec3/UVec3 only
```

Associated constants: `ZERO`, `ONE`, `NEG_ONE` (signed only), `MIN`, `MAX`, `X`,
`Y` (`Z`, `W` per dimension), `NEG_X`/`NEG_Y`/... (signed only), `AXES`.

### What is GONE versus `f32`

These need floating point and do not exist on integer vectors. Cast to a float
family first (see `as_*` below).

- `length` (only `length_squared` exists)
- `normalize`, `normalize_or_zero`, `try_normalize`, `is_normalized`
- `lerp`, `slerp`, `move_towards`
- `distance` (only `distance_squared`)
- `floor`, `ceil`, `round`, `fract`, `trunc` — integers have no fraction
- `recip`, `powf`, `exp`, trig, `is_finite`, `is_nan`
- `abs`/`signum`/`NEG_*` are also absent on the *unsigned* `UVec*` family.

### Wrapping / saturating / checked arithmetic

Integer vectors expose the same overflow-discipline methods as scalar integers,
element-wise. Present on both `IVec*` and `UVec*`:

```rust
fn wrapping_add(self, rhs: Self) -> Self     // + sub, mul, div
fn saturating_add(self, rhs: Self) -> Self   // + sub, mul, div
fn checked_add(self, rhs: Self) -> Option<Self>  // + sub, mul, div
```

Cross-sign helpers differ by family:

```rust
// On signed IVec* — combine with the matching UVec*:
fn checked_add_unsigned(self, rhs: UVec2) -> Option<Self>
fn checked_sub_unsigned(self, rhs: UVec2) -> Option<Self>
fn wrapping_add_unsigned(self, rhs: UVec2) -> Self
fn wrapping_sub_unsigned(self, rhs: UVec2) -> Self
fn saturating_add_unsigned(self, rhs: UVec2) -> Self
fn saturating_sub_unsigned(self, rhs: UVec2) -> Self

// On unsigned UVec* — combine with the matching IVec*:
fn checked_add_signed(self, rhs: IVec2) -> Option<Self>
fn wrapping_add_signed(self, rhs: IVec2) -> Self
fn saturating_add_signed(self, rhs: IVec2) -> Self
```

For pixel coordinates this matters: a brush stroke that walks `UVec2` pixel
positions toward the canvas edge should use `saturating_sub` / `checked_sub` so
you do not wrap a small `u32` past zero into `u32::MAX`.

### Bit operators

Integer vectors implement the bitwise operator traits element-wise (and their
`*Assign` forms), against both `Self` and the scalar element type:

- `BitAnd` / `BitOr` / `BitXor` (`& | ^`) and `BitAndAssign` / `BitOrAssign` / `BitXorAssign`
- `Shl` / `Shr` (`<< >>`) and `ShlAssign` / `ShrAssign`, against integer scalars and vectors
- `Not` (`!`)

### Comparison and `select` produce / consume a `BVec`

The `cmp*` family returns a boolean mask, never a `bool`:

```rust
fn cmpeq(self, rhs: Self) -> BVec2       // also cmpne, cmpge, cmpgt, cmple, cmplt
fn select(mask: BVec2, if_true: Self, if_false: Self) -> Self
```

`select` is associated (called as `IVec2::select(mask, a, b)`), and `mask` is a
`BVec` of the matching dimension. This is the bridge between the comparison
methods and branch-free blending. The same `cmp*` / `select` pair exists on every
numeric family including `f32`/`f64`.

## bool vectors — `BVec*` (the mask type)

`BVec2`/`BVec3`/`BVec4` are not math vectors. They are the masks returned by
`cmp*` and consumed by `select` across every numeric family. No arithmetic, no
`as_*` casts.

```rust
// Constants
const TRUE: Self;                        // all lanes true
const FALSE: Self;                       // all lanes false
// Construction
const fn new(x: bool, y: bool) -> Self           // BVec3: (x, y, z); BVec4: (x, y, z, w)
const fn splat(v: bool) -> Self
const fn from_array(a: [bool; 2]) -> Self        // length matches dimension (verify on BVec2)
// Packing / reduction
fn bitmask(self) -> u32                  // lane -> bit; x = bit 0, y = bit 1, ...
fn any(self) -> bool                     // true if any lane is true
fn all(self) -> bool                     // true if every lane is true
// Per-lane access (index panics if out of range: >1 for BVec2, >2 for BVec3)
fn test(self, index: usize) -> bool
fn set(&mut self, index: usize, value: bool)
```

`bitmask` packs lanes low-bit-first: for `BVec3`, `x` is bit 0, `y` is bit 1,
`z` is bit 2 — handy for switching on which comparisons passed.

How it ties together — float and int vectors hand you a `BVec` from a comparison
and take it back in `select`:

```rust
let mask: BVec2 = a.cmplt(b);            // where is a < b, per lane?
let picked = Vec2::select(mask, a, b);   // lanewise min, branch-free
if mask.any() { /* at least one lane of a is smaller */ }
```

SIMD mask variants `BVec3A` and `BVec4A` exist. They are the masks used by the
SIMD-backed `Vec3A` / `Vec4` (and `f64` SIMD) types; same `new`/`splat`/`bitmask`/
`any`/`all`/`test`/`set` surface, wider alignment. Use the plain `BVec3`/`BVec4`
unless a `Vec3A`/`Vec4` comparison hands you the `A` variant.

## f64 family — `DVec*`, `DMat*`, `DQuat`, `DAffine*`

The `f64` family is the `f32` API with `f64` elements and a `D` prefix. Naming:
`DVec2`/`DVec3`/`DVec4`, `DMat2`/`DMat3`/`DMat4`, `DQuat`, `DAffine2`/`DAffine3`.
There is no `DVec3A` analog — `Vec3A`/`Vec4` SIMD packing is `f32`-only — but
`DVec3` does provide `as_vec3a()` to cross into the `f32` SIMD type.

Everything in the vectors, matrices, and quat references applies unchanged with
`f64` substituted for `f32`: `length`, `normalize`, `lerp`, `dot`,
`cross`, `mul_vec*`, `from_rotation_*`, `select(mask: BVec2, ...)`, and so on.
Reach for `f64` only where `f32` precision is the bottleneck; Pixhaus stays on
`f32` for the render path.

## Cross-family conversions

Two routes between families: `as_*` for lossy numeric casts (always available,
never fails), and `From`/`TryFrom` for the lossless / fallible subset.

### `as_*` — lossy element-wise cast (the workhorse)

Every numeric vector has an `as_<family>vecN(self) -> <Family>VecN` for each
other numeric family of the same dimension. The cast is element-wise `as`
between the scalar types: float to int truncates toward zero, narrowing wraps /
saturates per Rust's `as` rules, no allocation, no `Option`. `BVec*` has none of
these — masks do not cast.

The casts you actually reach for in a pixel-art editor:

| From | Method | To | Note |
| --- | --- | --- | --- |
| `IVec2` | `as_vec2()` | `Vec2` | pixel coord -> world/render coord |
| `IVec2` | `as_uvec2()` | `UVec2` | signed -> unsigned (negatives wrap) |
| `IVec2` | `as_dvec2()` | `DVec2` | -> f64 |
| `UVec2` | `as_vec2()` | `Vec2` | unsigned pixel coord -> world coord |
| `UVec2` | `as_ivec2()` | `IVec2` | unsigned -> signed |
| `Vec2` | `as_ivec2()` | `IVec2` | world coord -> pixel, truncates toward zero |
| `Vec2` | `as_uvec2()` | `UVec2` | truncates; negatives wrap — clamp first |
| `Vec2` | `as_dvec2()` | `DVec2` | f32 -> f64 |
| `Vec3` | `as_dvec3()` | `DVec3` | f32 -> f64 |
| `DVec2` | `as_vec2()` | `Vec2` | f64 -> f32 |
| `IVec3` | `as_vec3()` / `as_vec3a()` | `Vec3` / `Vec3A` | |

The full per-vector menu (shown for the 2D row; 3D/4D mirror it with the
matching dimension, and `as_vec3a` exists on the 3-element vectors):

```rust
// On any numeric Vec2 (here IVec2):
fn as_vec2(self) -> Vec2
fn as_dvec2(self) -> DVec2
fn as_i8vec2(self) -> I8Vec2
fn as_u8vec2(self) -> U8Vec2
fn as_i16vec2(self) -> I16Vec2
fn as_u16vec2(self) -> U16Vec2
fn as_uvec2(self) -> UVec2       // (as_ivec2 on the unsigned/float vectors)
fn as_i64vec2(self) -> I64Vec2
fn as_u64vec2(self) -> U64Vec2
fn as_isizevec2(self) -> ISizeVec2
fn as_usizevec2(self) -> USizeVec2
// 3-element vectors additionally have:
fn as_vec3a(self) -> Vec3A
```

A vector does not emit an `as_*` for its own family (no `IVec2::as_ivec2`); use
the value directly.

### `From` / `TryFrom` — lossless and fallible

Where a cast cannot lose information, glam provides `From` (infallible); where it
can overflow, `TryFrom` (returns `Result`). Use these when you want the compiler
to reject lossy conversions instead of silently truncating.

- Tuples/arrays: `From<(i32, i32)>`, `From<[i32; 2]>`, and `Into` back to
  `(i32, i32)` / `[i32; 2]`.
- Widening within signedness is `From`: e.g. `IVec2: From<I8Vec2>`,
  `From<I16Vec2>`, `From<U8Vec2>`, `From<U16Vec2>` (smaller types fit in `i32`).
- `IVec2: From<BVec2>` — true -> 1, false -> 0 (verify direction).
- Narrowing or sign-crossing is `TryFrom`: e.g. `IVec2: TryFrom<I64Vec2>`,
  `TryFrom<U64Vec2>`, `TryFrom<ISizeVec2>` — fails if any lane is out of range.
- Int -> float widening to a larger element is `From` (e.g. `IVec2 -> DVec2`);
  `IVec2 -> Vec2` is exposed as `as_vec2` (an `i32` can exceed `f32`'s exact
  integer range, so it is a cast, not lossless `From`). (verify)

Rule of thumb: reach for `as_*` for the render-path pixel<->world casts where you
have already reasoned about range, and `TryFrom` at trust boundaries (loading a
project file, parsing user input) where an out-of-range coordinate should be an
error, not a wrap.

## Common patterns — pixel-art editor

```rust
use glam::{IVec2, UVec2, Vec2};

// Canvas size and a pixel address live in unsigned integer space.
let canvas_size: UVec2 = UVec2::new(8192, 8192);
let pixel: UVec2 = UVec2::new(12, 7);

// Render: pixel coord -> world/screen-space f32 for the wgpu viewport.
let world: Vec2 = pixel.as_vec2();                 // exact for in-range coords
let scaled: Vec2 = world * 2.0 + Vec2::new(0.5, 0.5);

// Input: floor a floating-point mouse position back to integer pixel coords.
// Use IVec2 (signed) so positions left/above the canvas are negative, not wrapped.
let mouse: Vec2 = Vec2::new(34.8, -2.3);
let hovered: IVec2 = mouse.floor().as_ivec2();     // (34, -3); floor before cast
// as_ivec2 alone truncates toward zero (-2 for -2.3); floor first to round down.

// Convert to addressable UVec2 only after a bounds check — as_uvec2 wraps negatives.
let in_bounds = hovered.cmpge(IVec2::ZERO).all()
    && hovered.cmplt(canvas_size.as_ivec2()).all();
if in_bounds {
    let addr: UVec2 = hovered.as_uvec2();
    let _index = addr.y * canvas_size.x + addr.x;  // row-major pixel index
}

// Edge-safe neighbor walk: saturating_sub keeps a u32 coord from wrapping past 0.
let left_neighbor: UVec2 = pixel.saturating_sub(UVec2::new(1, 0));

// Branch-free clamp into the canvas using cmp* + select (works on int vectors too).
let max_addr = canvas_size - UVec2::ONE;
let clamped = pixel.min(max_addr).max(UVec2::ZERO);
```

Takeaways: keep pixel coords in `UVec2`/`IVec2`, cast to `Vec2` with `as_vec2()`
for rendering, and when going the other way `floor()` the `Vec2` before
`as_ivec2()` (the cast truncates toward zero, which is wrong for negatives).
Bounds-check in signed space before `as_uvec2()`, because the unsigned cast wraps
negatives instead of clamping.
