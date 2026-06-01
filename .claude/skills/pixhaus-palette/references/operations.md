# palette 0.7.6 — operations: mix, adjust, blend, difference

All trait methods are generic over a scalar `Self::Scalar` (`T`, usually `f32`). Methods
consume `self` and return `Self` unless marked `_assign` (in place, `&mut self`).

## Mixing / interpolation — `Mix`, `MixAssign`

```rust
fn mix(self, other: Self, factor: Self::Scalar) -> Self;       // Mix
fn mix_assign(&mut self, other: Self, factor: Self::Scalar);   // MixAssign
```

- `factor` in `[0.0, 1.0]`: `0.0` → `self`, `1.0` → `other`, `0.5` → midpoint. Per-
  component lerp. Clamping of out-of-range `factor` is **unspecified** — clamp `t`
  yourself.
- **The result is only as good as the space — `mix` lerps the components of whatever
  space `Self` is, so the space *is* the algorithm.**
  - `Oklab`/`Oklch` (or `Lab`/`LinSrgb`) → perceptually smooth gradients.
  - gamma `Srgb` → **wrong** (muddy/dark midtones). Convert to linear/perceptual first.
  - a hue space (`Hsl`/`Oklch`/`Lch`) → interpolates the hue around the circle (shortest
    arc) — right for a hue sweep, not for a plain A→B blend.

`Mix` is the only built-in interpolation; there is **no `Gradient` type** — for multi-
stop ramps see `gradients-and-named.md`.

## Lightness / saturation — `Lighten`/`Darken`, `Saturate`/`Desaturate`

```rust
fn lighten(self, factor) -> Self;        // RELATIVE: scales toward max lightness
fn lighten_fixed(self, amount) -> Self;  // ABSOLUTE: adds to lightness, independent of current
fn darken(self, factor) -> Self;
fn darken_fixed(self, amount) -> Self;
fn lighten_assign(&mut self, factor);    // + _fixed_assign variants
// Saturate/Desaturate mirror this: saturate / saturate_fixed / desaturate / desaturate_fixed
```

- **Relative vs absolute** is the key distinction. At 50% lightness: `lighten(0.5)` → 75%
  (halfway to max); `lighten_fixed(0.5)` → 100% (direct add). `darken(0.5)` → 25%;
  `darken_fixed(0.5)` → 0%. Use `_fixed` for a constant-step brush, the relative form for
  a multiplicative tweak.
- `Darken`/`Desaturate` are blanket impls over `Lighten`/`Saturate`.
- `Lighten` is implemented on spaces with a lightness channel: `Hsl`, `Hsv`, `Hwb`,
  `Lab`, `Lch`, `Luv`, `Lchuv`, `Oklab`, `Oklch`, `Okhsl`, `Okhsv`, `Okhwb`, `Hsluv`,
  `Luma`, `Rgb`, `Xyz`, `Yxy`, `Alpha<C>`. `Saturate` only where there's a
  saturation/chroma channel: `Hsl`, `Hsv`, `Okhsl`, `Okhsv`, `Hsluv`, `Lch`, `Lchuv`,
  `Alpha<C>` (note: not `Hwb`/`Oklch`).

## Hue ops (cylindrical spaces)

```rust
fn shift_hue(self, amount: Self::Scalar) -> Self;  // ShiftHue; amount in degrees (120.0 shifts 120°->240°)
fn get_hue(&self) -> Self::Hue;                     // GetHue — returns the hue directly, NOT Option (grays -> 0)
fn set_hue(&mut self, hue: H);                      // SetHue — in place, generic over H
fn with_hue(self, hue: H) -> Self;                  // WithHue — copy with new hue
```

- `get_hue` returns the hue type directly; gray returns `0`. `set_hue`/`with_hue` are
  generic over `H` and convert into the space's native hue (`RgbHue` for Hsl/Hsv/Hwb,
  `OklabHue` for Ok*, `LabHue`/`LuvHue` for Lch/Lchuv/Hsluv).
- `ShiftHue` on `Hsl`, `Hsv`, `Hwb`, `Oklch`, `Okhsl`, `Okhsv`, `Okhwb`, `Lch`, `Lchuv`,
  `Hsluv`, `Alpha<C>`. No `shift_hue_assign`. `GetHue` is also on non-cylindrical spaces
  (`Rgb`, `Lab`, `Oklab`) since it computes a hue.

## `blend` module — compositing and blend modes

Two families. **`Compose`** = Porter-Duff alpha compositing (how layers stack by
coverage). **`Blend`** = separable blend modes (how layer colors combine). Both operate
on premultiplied alpha — see `PreAlpha`.

```rust
// Compose
fn over(self, other) -> Self;    // normal layer-on-layer
fn inside(self, other) -> Self;
fn outside(self, other) -> Self;
fn atop(self, other) -> Self;
fn xor(self, other) -> Self;
fn plus(self, other) -> Self;

// Blend — the full 0.7.6 set (11 separable modes)
fn multiply / screen / overlay / darken / lighten / dodge / burn
   / hard_light / soft_light / difference / exclusion (self, other) -> Self;
```

Note: modes are `dodge`/`burn` (not `color_dodge`/`color_burn`), and there are **no**
non-separable modes (hue/saturation/color/luminosity) in 0.7.6.

### `PreAlpha` — premultiplied alpha (required for blending)

```rust
pub struct PreAlpha<C: Premultiply> {
    pub color: C,            // components already multiplied by alpha
    pub alpha: C::Scalar,    // 0.0 transparent .. 1.0 opaque
}
PreAlpha::new(color, alpha); PreAlpha::new_opaque(color);
impl<C> From<C> for PreAlpha<C>;                   // opaque
impl<C> From<Alpha<C, C::Scalar>> for PreAlpha<C>; // premultiplies
fn unpremultiply(self) -> Alpha<C, C::Scalar>;     // back to straight alpha
```

- **Blending uses premultiplied alpha.** Docs: "a completely transparent resultant color
  will become black" — round-tripping loses color info in fully-transparent cases, and
  clamps alpha to `[0, 1]`.
- **Correctness rule the docs leave to you: composite in a *linear* space**, not gamma
  `Srgb`. Convert to `LinSrgb`, wrap in `PreAlpha`, blend/compose, `unpremultiply`,
  re-encode. (The 0.7.6 module docs state the premultiplied requirement but not the
  linear-space one.)

`Compose` is implemented for `PreAlpha<C>`, `Alpha<C, C::Scalar>`, and generically for
`C: Premultiply`.

## `color_difference` — seven metrics

| Trait | Method | Use | Best space |
|---|---|---|---|
| `Ciede2000` | `fn difference(self, other) -> Self::Scalar` | De-facto perceptual ΔE; accurate, expensive | `Lab`, `Lch` |
| `ImprovedCiede2000` | `fn improved_difference(self, other) -> Self::Scalar` | CIEDE2000 + Huang correction; same cost, better | `Lab`, `Lch` |
| `DeltaE` | `fn delta_e(self, other) -> Self::Scalar` | The space's native ΔE; low cost | `Lab`, `Lch`, `Cam16Ucs*` |
| `ImprovedDeltaE` | `fn improved_delta_e(self, other) -> Self::Scalar` | `DeltaE` + Huang; medium cost/accuracy | same |
| `EuclideanDistance` | `fn distance(self, other) -> Self::Scalar` and `fn distance_squared(self, other) -> Self::Scalar` | Quick; valid in uniform spaces | `Lab`, `Oklab` |
| `HyAb` | `fn hybrid_distance(self, other) -> Self::Scalar` | Euclidean+Manhattan hybrid; cheap, strong for medium-large diffs | `Lab` (best) |
| `Wcag21RelativeContrast` | see below | Accessibility/legibility (not nearness) | sRGB `Rgb`/`Luma` |

```rust
// Wcag21RelativeContrast
fn relative_contrast(self, other) -> Self::Scalar;           // the contrast ratio
fn has_min_contrast_text(self, other) -> Mask;              // >= 4.5:1  (AA normal text)
fn has_min_contrast_large_text(self, other) -> Mask;        // >= 3:1   (AA large text)
fn has_enhanced_contrast_text(self, other) -> Mask;         // >= 7:1   (AAA normal text)
fn has_enhanced_contrast_large_text(self, other) -> Mask;   // >= 4.5:1 (AAA large text)
fn has_min_contrast_graphics(self, other) -> Mask;          // >= 3:1   (graphics/UI)
```

- WCAG thresholds: 4.5:1 = AA normal text; 3:1 = AA large text (18pt+, or 14pt+ bold) and
  graphics; 7:1 / 4.5:1 = AAA normal/large. Input colors are sRGB-encoded; the impl
  converts to linear luma at D65 internally.
- `EuclideanDistance::distance_squared` skips the sqrt — use it when only **ranking**
  candidates (nearest-swatch), not when you need the actual distance value.

### Editor mapping

- **Nearest-palette-match / eyedropper-snap / indexed export:** convert pixels and
  swatches to `Lab`, rank with `Ciede2000`/`ImprovedCiede2000` (best perceptual) or
  `Oklab` + `distance_squared` (fast inner loop). `HyAb` is a good cheap middle ground for
  large palettes. Pairs with [[pixhaus-color-quant]] (which *builds* the palette).
- **Layer blend modes:** `Blend`/`Compose` in `LinSrgb` via `PreAlpha`.
- **Brushes:** `Lighten`/`Darken`/`Saturate`/`ShiftHue` (use `_fixed` for predictable steps).
- **Contrast checks:** `Wcag21RelativeContrast` on UI/text colors.

## Verification notes

All method names/signatures are from rendered 0.7.6 docs. `get_hue` returns the hue
directly (not `Option`) — confirmed. The "blend in linear space" rule is a real
correctness requirement not stated in the 0.7.6 blend module docs — treat it as caller
responsibility. `Blend` exposes exactly the 11 separable modes listed; no
`shift_hue_assign` exists.
