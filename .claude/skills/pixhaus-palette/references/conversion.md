# palette 0.7.6 — conversion, gamma↔linear, and the `cast` module

## The four conversion trait families

All live in `palette::convert`; the clamped pair (`FromColor`/`IntoColor`) and the
`Mut` pair are re-exported at the crate root, so `use palette::FromColor;` works. Every
family has a `From*`/`Into*` pair where `Into*` is a blanket impl over `From*` — you
only ever implement the `From*` direction.

### `FromColorUnclamped` / `IntoColorUnclamped` — raw math, no gamut check

```rust
pub trait FromColorUnclamped<T>: Sized { fn from_color_unclamped(val: T) -> Self; }
pub trait IntoColorUnclamped<T>: Sized { fn into_color_unclamped(self) -> T; }
```

The base layer everything builds on. The result may fall outside the target space's
valid range. Use when you'll do further math (don't want intermediate clamping to
discard out-of-gamut info) or will check bounds yourself.

```rust
let rgb = Srgb::from_color_unclamped(Lch::new(50.0f32, 100.0, -175.0));
assert!(!rgb.is_within_bounds());   // out of gamut, NOT clamped
```

### `FromColor` / `IntoColor` — clamped, the default you want

```rust
pub trait FromColor<T>: Sized { fn from_color(t: T) -> Self; }
pub trait IntoColor<T>: Sized { fn into_color(self) -> T; }

// pure blanket impl — you cannot implement FromColor directly:
impl<T, U> FromColor<T> for U where U: FromColorUnclamped<T> + Clamp { /* runs unclamped then clamps */ }
```

`from_color` runs `from_color_unclamped` then **gamut-clamps** into the target's valid
range. The lossy "just give me a valid color" path — the default for one-shot
conversions you store or display. (`IntoColor` is **not** dyn-compatible.)

```rust
let rgb = Srgb::from_color(Lch::new(50.0f32, 100.0, -175.0));
assert!(rgb.is_within_bounds());    // clamped into gamut
```

There is also `TryFromColor`/`TryIntoColor` returning `Result` when you need to *detect*
out-of-gamut instead of silently clamping.

### `FromColorMut` / `IntoColorMut` — in-place, zero-alloc

```rust
pub trait FromColorMut<T> where T: ?Sized + FromColorMut<Self> {
    fn from_color_mut(color: &mut T) -> FromColorMutGuard<'_, Self, T>;
}
pub trait IntoColorMut<T>: FromColorMut<T> where T: ?Sized + FromColorMut<Self> {
    fn into_color_mut(&mut self) -> FromColorMutGuard<'_, T, Self>;
}
```

Converts a color **or a `[T]` slice** in place, reusing the same memory, returning a
guard that converts back to the original type on drop. Source and target must share
memory layout (same component type + count, enforced via `ArrayCast`). This is the
8K-canvas path — transform a big buffer without allocating a parallel `Vec`. See
[[project-8k-perf-constraint]].

```rust
let mut rgb = [Srgb::new(1.0, 0.0, 0.0), Srgb::new(0.0, 1.0, 0.0)];
{
    let mut hsv = <[Hsv]>::from_color_mut(&mut rgb); // in place, no alloc
    hsv.shift_hue_assign(60.0);
} // guard drops -> rgb is Srgb again
```

`FromColorMutGuard::then_into_color_mut::<[Hsl]>()` chains conversions without bouncing
back through the original on each step. (verify the exact `then_into_color_mut` signature.)

**Rule of thumb:** `into_color` for normal one-shot conversions; `into_color_unclamped`
for raw math or when you'll clamp later; `*_mut` for bulk in-place buffer transforms.

## THE gotcha: gamma encoding vs linear math

`Srgb<u8>` and `Srgb<f32>` are gamma-encoded (non-linear). The u8/f32 split is only the
storage format — both still carry the sRGB transfer function. **Arithmetic — blending,
compositing, averaging, mix, lightness — directly on gamma values is wrong** ("the
midpoint looks too dark"). Move to a *linear* space first (`LinSrgb` for plain blends;
`Xyz`/`Oklab`/`Lab` for perceptual work).

Canonical pipeline — file bytes in, math, bytes out:

```rust
use palette::{Srgb, LinSrgb, Mix};

// 1. raw bytes from a PNG / pixel buffer: Srgb<u8> (gamma-encoded)
let a: Srgb<u8> = Srgb::new(r, g, b);

// 2. u8 -> f32 (still gamma), then 3. remove gamma -> linear
let a_lin: LinSrgb<f32> = a.into_format::<f32>().into_linear();
//   into_format::<f32>() only rescales 0..=255 -> 0.0..=1.0 (NO gamma change)
//   into_linear()        applies the sRGB EOTF (gamma -> linear)
// equivalently: a.into_format().into_color()

// 4. do math in linear space
let mixed: LinSrgb<f32> = a_lin.mix(b_lin, 0.5);

// 5. linear -> gamma -> u8, back to a buffer
let out: Srgb<u8> = Srgb::from_linear(mixed).into_format::<u8>();
```

`into_format` = "reinterpret the same color at a new precision" (no gamma touch).
`into_linear`/`from_linear` (and `into_color` between `Srgb` and `LinSrgb`) = "apply/
remove the transfer function". See `components-and-alpha.md` for the full method set.

## XYZ as the conversion hub

palette routes conversions through CIE XYZ rather than implementing N×N direct paths.
A conversion like `Hsl -> Lab` expands to roughly `Hsl -> (Lin)Rgb -> Xyz -> Lab`. For a
custom type, `#[derive(FromColorUnclamped)]` needs only `impl FromColorUnclamped<Xyz>
for YourType` and generates the rest by chaining through XYZ.

Derive knobs: `skip_derives(Rgb, Luma)` (edges you implement by hand), `component = "T"`
(default `f32`), `rgb_standard = "..."` (default `Srgb`), `white_point = "..."` (default
`D65`); field attr `#[palette(alpha)]`. Limitation: a single conversion generally can't
change the component type **and** the meta types (white point / RGB standard) at once,
and some spaces pin a white point (Oklab requires D65) — split into multiple
`into_color` steps when you need to change more than one axis.

## The `cast` module — bridging palette and raw `Vec<u8>` pixel buffers

How to move between typed colors and flat component buffers without copying.

**Marker traits:** `ArrayCast` (representable as a fixed-size array), `UintCast`
(representable as an unsigned integer). There is **no `ComponentCast` trait** —
component casting is `FromComponents`/`IntoComponents`/`ComponentsAs`/`TryComponentsAs`
(+ `*Mut`). Error types: `SliceCastError`, `VecCastError`, `BoxedSliceCastError`.

### Single color ↔ array

```rust
let color = palette::cast::from_array::<Srgb<u8>>([23, 198, 76]);
let array: [u8; 3] = palette::cast::into_array(color);
// reference views (no copy): from_array_ref / into_array_ref
```

### Color slice ↔ component slice — the pixel-buffer bridge (zero-copy)

```rust
use palette::{Srgb, cast};

// raw buffer (len must be a multiple of 3) -> &[Srgb<u8>], no copy
let buf: &[u8] = /* RGB pixel data */;
let pixels: &[Srgb<u8>] = cast::try_from_component_slice(buf)?; // Result; prefer this
// from_component_slice(buf) PANICS on length mismatch — avoid per [[pixhaus-rust-conventions]]

let bytes: &[u8] = cast::into_component_slice(pixels);          // back the other way
```

Verified names: `from_component_slice` (panics), `try_from_component_slice` (Result),
`into_component_slice`, plus `*_mut`, `from_component_vec`/`try_from_component_vec`,
`from_component_slice_box`, `into_component_vec`, `into_component_slice_box`. The trait
form is `buf.components_as()` / `buf.try_components_as()` (`ComponentsAs`/`TryComponentsAs`).

For an `[[u8; 4]]`-shaped buffer use `from_array_slice`/`into_array_slice` (+ `_mut`,
`_box`, `_vec`). In-place mapping: `map_vec_in_place`, `map_slice_box_in_place`.

### Packed `u32`

`palette::cast::Packed` is a color packed into a compact int, parameterized by a
`ComponentOrder` so you control channel order. Concrete aliases `PackedArgb` /
`PackedRgba` live in **`palette::rgb`** (not `cast`).

```rust
use palette::{Srgba, rgb::PackedArgb, cast};

let raw: u32 = 0xFF7F0080;                 // order is ComponentOrder-dependent (ARGB here)
let packed: PackedArgb = cast::from_uint(raw);
let color:  Srgba<u8>  = Srgba::from(packed);
// back: let raw2: u32 = cast::into_uint(/* a UintCast color */);
```

Functions: `from_uint`/`into_uint` (+ `_ref`, `_mut`, `_slice`, `_vec`, `_box`,
`_array`). Verify the byte order against the specific `Packed*` alias before relying on
a literal like `0xFF7F0080`.

## Verification notes

- The four trait signatures and the `FromColor` blanket impl are quoted from rendered
  0.7.6 docs. Crate-root URLs for these traits 404; canonical pages are under
  `palette/convert/`. The full `cast` function/trait/struct name list is verified.
- Not pinned from rendered docs: the exact associated-type spelling inside
  `from_array`/`into_array`, the `then_into_color_mut` signature, and the literal byte
  order of the `Packed*` example — confirm against source if load-bearing.
