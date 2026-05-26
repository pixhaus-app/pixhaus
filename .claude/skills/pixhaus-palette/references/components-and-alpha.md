# palette 0.7.6 — component types, Stimulus, format conversion, Alpha, Clamp

## Component type `T`

Every color is generic over a per-channel scalar `T` — `f32` (default), `f64`, `u8`,
`u16`. The color space (`S`/`Wp`) and the numeric type (`T`) are orthogonal: changing
`T` does not change the space or its encoding.

For a pixel editor:
- **u8** — storage/textures. 1 byte/channel, matches GPU `Rgba8` and PNG. Bands; no
  headroom. Never blend or filter directly in u8.
- **u16** — high-bit storage where 8-bit banding shows.
- **f32** — math. The natural GPU shader type; 24-bit mantissa is ample for 8/16-bit
  pixels.
- **f64** — only for accuracy-sensitive color-science chains; 2× memory, overkill for
  per-pixel canvas work.

Practical loop: **store u8, lift to f32, decode to linear for math, re-encode, clamp,
quantize back to u8.**

## The `stimulus` module — normalized components, 255 ↔ 1.0

A *stimulus* is a color component encoding intensity from 0 (none) to a max (full /
"white"). The name is from "tristimulus" (XYZ, RGB).

```rust
pub trait Stimulus: Zero {
    fn max_intensity() -> Self;   // 255u8 / 65535u16 / 1.0f32 / 1.0f64
}

pub trait IntoStimulus<T> { fn into_stimulus(self) -> T; }
// FromStimulus method:        fn from_stimulus(other: T) -> Self;
```

Both "convert while performing the appropriate scaling, rounding and clamping." The
load-bearing point: **conversion preserves the normalized intensity — it is NOT a raw
cast.** `u8` 255 ↔ `f32` 1.0; `u8` 0 ↔ `f32` 0.0. `f32 → u8` multiplies by 255, rounds,
and clamps; `u8 → f32` divides by 255.

`value as f32` would give `255.0`, not `1.0`. **Always cross integer/float with the
stimulus conversions (`into_format`/`FromStimulus`), never `as`.** `StimulusColor` is a
marker for colors whose every component is a stimulus (RGB qualifies; Lab/Hsl do not).

## `into_format` / `from_format` (component type only) vs `into_linear` (encoding)

```rust
// component type T -> U; color space S AND encoding unchanged
pub fn into_format<U>(self) -> Rgb<S, U> where U: FromStimulus<T>;
pub fn from_format<U>(color: Rgb<S, U>) -> Self where T: FromStimulus<U>;
```

`into_format` changes **only** the numeric type, via `FromStimulus`.
`Srgb<u8>::into_format::<f32>()` keeps the gamma-encoded sRGB curve and the primaries —
it just rescales `0..=255` to `0.0..=1.0`.

```rust
// these DO change encoding (gamma decode/encode), not just the type:
pub fn into_linear<U>(self) -> Rgb<Linear<S::Space>, U>
    where S: RgbStandard, S::TransferFn: IntoLinear<U, T>;
pub fn from_linear<U>(color: Rgb<Linear<S::Space>, U>) -> Self
    where S: RgbStandard, S::TransferFn: FromLinear<U, T>;
```

`into_linear` runs the sRGB EOTF; the output is in `Linear<...>`, a different standard.
`into_linear`/`from_linear` are faster than chaining `into_format().into_color()`
because they fuse the retype and the decode. `into_encoding<U, St>` / `from_encoding<U,
St>` re-encode a *linear* color into a different `RgbStandard` (different gamma curve),
bounded by `St: RgbStandard<Space = S>`.

Mnemonic: **`into_format` = "same color, new precision"; `into_linear` = "do real math
on light".** Conflating them is the classic palette bug.

## `Alpha<C, T>` — the alpha wrapper

```rust
pub struct Alpha<C, T> {
    pub color: C,   // the inner color
    pub alpha: T,   // 0.0 / 0u8 = transparent, 1.0 / 255u8 = opaque
}
```

- `Srgba<S, T> = Alpha<Srgb<S, T>, T>`, `Rgba = Alpha<Rgb<S, T>, T>`. The alpha type can
  differ from the color's `T`.
- **Deref:** `Alpha` derefs to `C`, so `rgba.red` reaches `rgba.color.red`. In *generic*
  code, write `.color` explicitly to avoid move-out-of-deref errors.
- Generic constructor `Alpha::new(color, alpha)`; RGB-backed `Alpha` also gets the
  4-arg `Srgba::new(r, g, b, a)` / `Rgba::new(...)`.
- `premultiply(self) -> PreAlpha<C>` (see `operations.md` — blending needs this).

### `WithAlpha<A>` — attach/strip alpha generically

```rust
type Color;        // opaque color (no transparency)
type WithAlpha;    // = Alpha<Self::Color, A>

fn with_alpha(self, alpha: A) -> Self::WithAlpha;   // wrap/replace alpha
fn without_alpha(self) -> Self::Color;              // drop alpha
fn split(self) -> (Self::Color, A);                 // separate
fn opaque(self) -> Self::WithAlpha      where A: Stimulus;  // alpha = max_intensity()
fn transparent(self) -> Self::WithAlpha where A: Zero;      // alpha = 0
```

Implemented for both opaque colors and `Alpha` wrappers, so generic code adds/removes
transparency uniformly. (verify whether `opaque`/`transparent` are on the trait vs
inherent on `Alpha` — likely both via the impl; the bounds `A: Stimulus` / `A: Zero` are
reliable.)

## `Clamp` / `IsWithinBounds` — gamut handling

```rust
fn clamp(self) -> Self;                    // Clamp
fn clamp_assign(&mut self);                // ClampAssign (in place)
fn is_within_bounds(&self) -> Self::Mask;  // IsWithinBounds; Mask = bool for scalar colors
```

- `clamp()` restricts each component to its valid range for that space. sRGB
  `(1.3, 0.5, -3.0).clamp()` → `(1.0, 0.5, 0.0)`. `clamp_assign` is the no-copy form.
- `is_within_bounds()` returns true iff all components are inside their valid ranges. It
  only *reports* — it does not fix.
- Why out-of-gamut happens: valid bounds are per-space, and conversions / arithmetic can
  produce mathematically valid but unrepresentable values (an Oklab or linear-RGB result
  with no sRGB equivalent). palette does **not** auto-clamp on unclamped conversion — you
  detect with `is_within_bounds()` and correct with `clamp()`/`clamp_assign()` before
  display or `u8` quantization. Detection and correction are deliberately separate ops.

## `num` module (brief)

Granular traits abstracting over numeric types so palette runs over `f32`, `f64`, and
SIMD batches with one code path: `Real`, `Zero`, `One`, `Arithmetics`, `MulAdd`,
`Sqrt`, `Cbrt`, `Powf`, `Trigonometry`, `MinMax`, `PartialCmp`, `Clamp`, etc.
`Stimulus: Zero` ties in here — `Zero` is the 0% end, `max_intensity()` the 100% end.

## Verification notes

All signatures above are quoted from rendered 0.7.6 docs. High confidence on `Stimulus`,
`IntoStimulus`, `into_format`/`from_format`/`into_linear`/`from_linear`, `Alpha`
fields/`new`/Deref, `Clamp`/`ClampAssign`/`IsWithinBounds`. The `into_encoding<U, St>`
generic-parameter order and the exact placement of `opaque`/`transparent` are the two
details to glance at the rustdoc for if you write the bounds by hand.
