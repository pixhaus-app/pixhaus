# palette 0.7.6 — gradients and named colors

## The headline: there is NO `Gradient` type or `gradient` module in 0.7

Verified by direct fetch: `palette/gradient/index.html` returns **404**, and the crate
root lists no `gradient` module and no `Gradient` type anywhere. The old
`palette::gradient::Gradient` (present through ~0.5) was removed in the 0.6 rework and
never returned. Anything you "remember" about `Gradient::new`, `Gradient::with_domain`,
`gradient.get(t)`, `gradient.take(n)`, `gradient.colors()`, or named-gradient
constructors is **stale — none of it exists in 0.7.x and will not compile.** Do not
write it.

The full 0.7.6 top-level module list (no gradient):
`alpha, angle, blend, bool_mask, cam16, cast, chromatic_adaptation, color_difference,
color_theory, convert, encoding, hsl, hsluv, hsv, hues, hwb, lab, lch, lchuv, luma,
luv, named, num, okhsl, okhsv, okhwb, oklab, oklch, rgb, serde, stimulus, white_point,
xyz, yxy`.

## What palette gives you for interpolation: `Mix` (two colors)

```rust
pub trait Mix { type Scalar; fn mix(self, other: Self, factor: Self::Scalar) -> Self; }
```

`factor` in `0.0..=1.0`. One segment only. See `operations.md`. Out-of-range clamping is
unspecified — clamp `t` yourself. (`blend` is for compositing, not gradients.)

## Multi-stop gradients: bring the `enterpolation` crate

palette 0.7.6 carries `enterpolation ^0.2.0` as a **dev-dependency** and uses it in its
own gradient examples. That is the sanctioned way to do N-stop gradients now. It's a
separate crate you add yourself; it is not re-exported from palette.

```toml
enterpolation = "0.2"
```

Build a piecewise-linear gradient over palette colors:

```rust
use enterpolation::{linear::Linear, Curve};
use palette::Oklab;   // interpolate in a perceptual space — see below

let grad = Linear::builder()
    .elements([col_a, col_b, col_c])   // palette colors as control points (here Oklab)
    .equidistant::<f32>()              // or .knots([0.0, 0.4, 1.0]) for a custom domain
    .build()?;                         // returns Result — propagate with ?, don't unwrap

// N evenly spaced samples (the analog of the old gradient.take(n)):
let ramp: Vec<Oklab> = grad.take(steps).collect();
```

- `Curve::take(self, samples: usize)` is verified — "give me N evenly spaced values",
  returns an iterator; collect it.
- Single-point sampling at `t` is shown in examples as `grad.gen(t)` (the `Generator`
  trait). The exact `gen`/`sample` signature could **not** be verified from docs.rs
  (only `take` was fully documented) — confirm against the installed version before
  relying on it. (verify)
- Custom stops via `.knots([...])`; `Equidistant` backs `.equidistant()`.

## Which space to interpolate in (the load-bearing advice)

Interpolate in a perceptually uniform space, **not** gamma sRGB:

- **Best: `Oklab` / `Oklch`.** The modern default — perceptually uniform, no surprise
  hue shifts, cheap. Use `Oklch` to travel around the hue wheel, `Oklab` for a straight
  blend without hue drift.
- **Acceptable, physically correct, not perceptual: `LinSrgb`.** Better than gamma
  `Srgb` (which gives a muddy/dark midpoint), but a linear blend still doesn't track
  perceived lightness evenly.
- **Never interpolate in gamma `Srgb`** — the classic "gray dead zone" in a blue→yellow
  ramp.
- `Lab`/`Lch` are also fine; Oklab/Oklch are the newer, better-behaved choice.

Mechanics: convert endpoint colors into the working space, build/sample there, then
convert each sample back to `Srgb` for display. In the hot path you can use
`into_color_unclamped` and `clamp()` only the final display colors (see `conversion.md`).

## The `named` module — CSS/SVG color constants

- Gated behind the `named` feature — **already on by default** (see
  `features-and-interop.md`).
- 148 SVG/CSS3 named-color constants, `ALICEBLUE` through `YELLOWGREEN`, including `RED`,
  `REBECCAPURPLE`, `OLIVE`, `BLACK`, `WHITE`, `NAVY`. Access as `palette::named::RED`.
- **Constant type is `Srgb<u8>`** (8-bit gamma sRGB). Lift to working precision with
  `Srgb::<f32>::from_format(named::OLIVE)` (and `into_linear()` before math).
- Name lookup (verified):

  ```rust
  pub fn from_str(name: &str) -> Option<Srgb<u8>>   // Some(color) or None
  // palette::named::from_str("olive")
  ```

  Gated behind `named_from_str` (implies `named`) — also default.

## Verification notes

- The no-`Gradient` reality and the module list are verified by direct fetch (404 +
  crate-root listing). `Mix::mix` factor clamping is unspecified. `Curve::take` is
  verified; `enterpolation`'s single-point `gen`/`sample` signatures are not — confirm
  against the pinned version. `named` constants are `Srgb<u8>`; `from_str` and both
  feature flags are confirmed.
