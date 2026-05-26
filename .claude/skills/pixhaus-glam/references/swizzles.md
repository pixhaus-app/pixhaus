# glam 0.33.0 swizzle traits

API reference for `Vec2Swizzles`, `Vec3Swizzles`, `Vec4Swizzles`.

## What swizzling is

Swizzling reorders or duplicates a vector's components into a new vector. The
method name is the sequence of component letters to read, in output order:

```rust
use glam::{Vec3, Vec3Swizzles};

let v = Vec3::new(1.0, 2.0, 3.0);
v.xy();   // Vec2(1.0, 2.0)   reorder + drop
v.zyx();  // Vec3(3.0, 2.0, 1.0)   reverse
v.xxxx(); // Vec4(1.0, 1.0, 1.0, 1.0)   duplicate, on a type that returns Vec4
```

The number of letters sets the output dimension, independent of the source: 2
letters -> a 2-component vector, 3 -> 3-component, 4 -> 4-component. A `Vec2` can
swizzle up to a `Vec4` (`v.xyxy()`), a `Vec4` can swizzle down to a `Vec2`
(`v.xy()`).

## Traits must be in scope

The methods are trait methods, not inherent methods. The trait must be imported
or the calls don't resolve:

```rust
use glam::Vec3Swizzles; // or use glam::*;
```

The traits are defined in `glam::swizzles` but re-exported at the crate root
(`pub use self::swizzles::Vec2Swizzles;` and the Vec3/Vec4 equivalents), so both
`use glam::Vec3Swizzles;` and `use glam::*;` bring them in. They do not appear in
the rustdoc Traits index, only under the `swizzles` module — that is a doc
quirk, not a path you need.

## Implemented for every numeric vector family

Not just the f32 `Vec*`. The swizzle return types are associated types, so each
implementing type returns its own family.

- `Vec2Swizzles` (16 impls): `Vec2`, `DVec2`, `I8Vec2`, `I16Vec2`, `IVec2`,
  `I64Vec2`, `ISizeVec2`, `U8Vec2`, `U16Vec2`, `UVec2`, `U64Vec2`, `USizeVec2`.
- `Vec3Swizzles` (13 impls): `Vec3`, `Vec3A`, `DVec3`, `I8Vec3`, `I16Vec3`,
  `IVec3`, `I64Vec3`, `ISizeVec3`, `U8Vec3`, `U16Vec3`, `UVec3`, `U64Vec3`,
  `USizeVec3`.
- `Vec4Swizzles` (13 impls): `Vec4`, `DVec4`, `I8Vec4`, `I16Vec4`, `IVec4`,
  `I64Vec4`, `ISizeVec4`, `U8Vec4`, `U16Vec4`, `UVec4`, `U64Vec4`, `USizeVec4`.

So `IVec3::new(1, 2, 3).xy()` returns `IVec2`, `DVec4::splat(0.0).xyz()` returns
`DVec3`, and so on.

## No rgba aliases

glam exposes only `x`/`y`/`z`/`w` letters. There is no `rgba()`, `rgb()`, `st()`,
or other named alias set. A "bgra" reorder is spelled with xyzw mapped by
position: treat x=r, y=g, z=b, w=a and write the swizzle in the target order,
e.g. b,g,r,a is `v.zyxw()`.

## Naming rule

A method named by letters `abc...` returns a vector whose components are
`(self.a, self.b, self.c, ...)`. Every permutation and repetition that fits the
source dimension exists, for each output dimension the trait supports. The full
sets below are exhaustive for 0.33.0.

## Vec2Swizzles

Associated types: `Vec3`, `Vec4`. Source letters: `{x, y}`.

```rust
// identity is a provided method
fn xy(self) -> Self;

// 2-letter -> Self (Vec2)
fn xx(self) -> Self;
fn yx(self) -> Self;
fn yy(self) -> Self;

// 3-letter -> Self::Vec3 (all 8 permutations of {x,y})
fn xxx(self) -> Self::Vec3; fn xxy(self) -> Self::Vec3;
fn xyx(self) -> Self::Vec3; fn xyy(self) -> Self::Vec3;
fn yxx(self) -> Self::Vec3; fn yxy(self) -> Self::Vec3;
fn yyx(self) -> Self::Vec3; fn yyy(self) -> Self::Vec3;

// 4-letter -> Self::Vec4 (all 16 permutations of {x,y})
fn xxxx(self) -> Self::Vec4; // ...all 16 permutations of {x,y}... fn yyyy(self) -> Self::Vec4;
```

`Vec2` has no `with_*` swizzle setters.

## Vec3Swizzles

Associated types: `Vec2`, `Vec4`. Source letters: `{x, y, z}`.

```rust
// identity is a provided method
fn xyz(self) -> Self;

// 2-letter -> Self::Vec2 (all 9 permutations of {x,y,z})
fn xx(self) -> Self::Vec2; fn xy(self) -> Self::Vec2; fn xz(self) -> Self::Vec2;
fn yx(self) -> Self::Vec2; fn yy(self) -> Self::Vec2; fn yz(self) -> Self::Vec2;
fn zx(self) -> Self::Vec2; fn zy(self) -> Self::Vec2; fn zz(self) -> Self::Vec2;

// 3-letter -> Self (all 27 permutations of {x,y,z}: xxx..zzz)
fn xxx(self) -> Self; // ...all 27 permutations of {x,y,z}... fn zzz(self) -> Self;

// 4-letter -> Self::Vec4 (all 81 permutations of {x,y,z}: xxxx..zzzz)
fn xxxx(self) -> Self::Vec4; // ...all 81 permutations of {x,y,z}... fn zzzz(self) -> Self::Vec4;

// 2-component setters: replace the named pair, return Self
fn with_xy(self, rhs: Self::Vec2) -> Self;
fn with_xz(self, rhs: Self::Vec2) -> Self;
fn with_yx(self, rhs: Self::Vec2) -> Self;
fn with_yz(self, rhs: Self::Vec2) -> Self;
fn with_zx(self, rhs: Self::Vec2) -> Self;
fn with_zy(self, rhs: Self::Vec2) -> Self;
```

## Vec4Swizzles

Associated types: `Vec2`, `Vec3`. Source letters: `{x, y, z, w}`.

```rust
// identity is a provided method
fn xyzw(self) -> Self;

// 2-letter -> Self::Vec2 (all 16 permutations of {x,y,z,w}: xx..ww)
fn xy(self) -> Self::Vec2; // ...all 16 permutations of {x,y,z,w}... fn ww(self) -> Self::Vec2;

// 3-letter -> Self::Vec3 (all 64 permutations of {x,y,z,w}: xxx..www)
fn xyz(self) -> Self::Vec3; // ...all 64 permutations of {x,y,z,w}... fn www(self) -> Self::Vec3;

// 4-letter -> Self (all 256 permutations of {x,y,z,w}: xxxx..wwww)
fn xxxx(self) -> Self; // ...all 256 permutations of {x,y,z,w}... fn wwww(self) -> Self;

// setters: replace the named group, return Self
fn with_xy(self, rhs: Self::Vec2) -> Self;   // ...all 2-letter groups...
fn with_xyz(self, rhs: Self::Vec3) -> Self;  // ...all 3-letter groups...
```

## Dropping a component, and the truncate/extend relationship

To drop a component, swizzle to the lower dimension. The leading-letters swizzle
is equivalent to the inherent `truncate` method, but swizzling also lets you
reorder while dropping.

```rust
let v4 = Vec4::new(1.0, 2.0, 3.0, 4.0);
v4.xyz();      // Vec3(1, 2, 3) -- same result as v4.truncate()
v4.truncate(); // Vec3(1, 2, 3) -- inherent: pub fn truncate(self) -> Vec3
v4.zyx();      // Vec3(3, 2, 1) -- reorder while dropping w; truncate can't do this

let v3 = Vec3::new(1.0, 2.0, 3.0);
v3.xy();       // Vec2(1, 2) -- same as v3.truncate()
v3.truncate(); // Vec2(1, 2) -- inherent: pub fn truncate(self) -> Vec2
```

Going up a dimension is the inverse: `extend` appends one component; swizzling
can build the higher vector from existing components only (no new value).

```rust
let v3 = Vec3::new(1.0, 2.0, 3.0);
v3.extend(1.0); // Vec4(1, 2, 3, 1) -- inherent: pub fn extend(self, w: f32) -> Vec4
v3.xyzz();      // Vec4(1, 2, 3, 3) -- w taken from an existing component, no new value

let v4 = Vec4::new(1.0, 2.0, 3.0, 4.0);
v4.with_w(1.0); // Vec4(1, 2, 3, 1) -- inherent setter: pub fn with_w(self, w: f32) -> Self
```

Rule of thumb: use `extend`/`truncate`/`with_w` when a component value comes from
outside the vector (a literal, a separate scalar). Use a swizzle when every
output component is one of the input components.

## Common patterns

```rust
use glam::{Vec2, Vec3, Vec4, Vec3Swizzles, Vec4Swizzles};

// Drop w from a homogeneous/world position (Vec4 -> Vec3).
let pos_h = Vec4::new(4.0, 5.0, 6.0, 1.0);
let pos = pos_h.xyz();          // Vec3(4, 5, 6); identical to pos_h.truncate()

// Coordinate-system change: reorder axes (e.g. y-up <-> z-up swap of y and z).
let yup = Vec3::new(1.0, 2.0, 3.0);
let zup = yup.xzy();            // Vec3(1, 3, 2)
let flip = yup.zyx();           // Vec3(3, 2, 1); full reverse

// Channel reorder, rgba -> bgra, using positional xyzw (x=r, y=g, z=b, w=a).
let rgba = Vec4::new(0.1, 0.2, 0.3, 1.0);
let bgra = rgba.zyxw();         // Vec4(0.3, 0.2, 0.1, 1.0)

// Build a Vec4 from a Vec3 position plus w = 1.0.
let p = Vec3::new(1.0, 2.0, 3.0);
let homogeneous = p.extend(1.0);        // Vec4(1, 2, 3, 1) -- w is a fresh scalar
// vs. swizzle, which can only reuse existing components:
let dup_w = p.xyzz();                   // Vec4(1, 2, 3, 3) -- w copied from z
```
