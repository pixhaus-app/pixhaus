# palette 0.7.6 — color spaces

Every signature here is from docs.rs 0.7.6. Component type `T` defaults to `f32` on
every type. Two meta-generics recur:

- `S` (RGB family) — an `RgbStandard`: bundles primaries **and** the transfer function.
  Default `Srgb`. This is why `Srgb` (gamma) and `LinSrgb` (linear) are different types.
- `Wp` (CIE family) — a white point, default `D65` (2-degree standard observer). The
  Oklab family fixes D65 internally and has **no** `Wp` param.

## RGB family — storage, display, GPU

```rust
pub struct Rgb<S = Srgb, T = f32> {
    pub red: T,
    pub green: T,
    pub blue: T,
    pub standard: PhantomData<S>,
}
```

- Range: `0.0..=1.0` for `f32`, `0..=255` for `u8`.
- Reach for it for storage, display, and GPU upload. **Not** for perceptual mixing —
  convert to a linear/perceptual space first (see `conversion.md`).

Aliases (in `palette::rgb`, re-exported at root):

| Alias | Meaning |
|---|---|
| `Srgb<T> = Rgb<Srgb, T>` | gamma-encoded sRGB (the `S = Srgb` standard) |
| `LinSrgb<T> = Rgb<Linear<Srgb>, T>` | linear-light sRGB |
| `Srgba<T>` / `LinSrgba<T>` | the above + alpha (`a` suffix = `Alpha` wrapper) |
| `GammaSrgb` / `GammaSrgba` | gamma-2.2 encoded |
| `Rgba` | generic `Alpha<Rgb<S,T>, T>` |
| `PackedRgba`, `PackedArgb`, `PackedAbgr`, `PackedBgra` | byte-order-tagged packed forms (see `conversion.md`) |

Common concrete forms: `Srgb<u8>` (8-bit display/file), `Srgb` = `Srgb<f32>`,
`LinSrgb` = `LinSrgb<f32>` (the linear working type).

Constructor: `pub const fn new(red: T, green: T, blue: T) -> Rgb<S, T>`. RGB-backed
`Alpha` gets a 4-arg `Srgba::new(r, g, b, a)`.

## Cylindrical RGB spaces — color pickers

```rust
pub struct Hsl<S = Srgb, T = f32> { pub hue: RgbHue<T>, pub saturation: T, pub lightness: T, pub standard: PhantomData<S> }
pub struct Hsv<S = Srgb, T = f32> { pub hue: RgbHue<T>, pub saturation: T, pub value: T,     pub standard: PhantomData<S> }
pub struct Hwb<S = Srgb, T = f32> { pub hue: RgbHue<T>, pub whiteness: T,  pub blackness: T, pub standard: PhantomData<S> }
```

- `hue: RgbHue<T>` 0..360 deg. `saturation`/`lightness`/`value`/`whiteness`/`blackness`
  all `0.0..1.0`. Hsl: lightness 0 = black, 0.5 = pure color, 1 = white. Hsv: value
  0 = black, 1 = bright.
- Familiar picker axes but **not perceptually uniform** — equal slider steps don't look
  equal. For uniform sliders prefer the Ok* trio below.

## CIE Lab family — perceptual difference

```rust
pub struct Lab<Wp = D65, T = f32>   { pub l: T, pub a: T, pub b: T,            pub white_point: PhantomData<Wp> }
pub struct Lch<Wp = D65, T = f32>   { pub l: T, pub chroma: T, pub hue: LabHue<T>, pub white_point: PhantomData<Wp> }
pub struct Luv<Wp = D65, T = f32>   { pub l: T, pub u: T, pub v: T,            pub white_point: PhantomData<Wp> }
pub struct Lchuv<Wp = D65, T = f32> { pub l: T, pub chroma: T, pub hue: LuvHue<T>, pub white_point: PhantomData<Wp> }
pub struct Hsluv<Wp = D65, T = f32> { pub hue: LuvHue<T>, pub saturation: T, pub l: T, pub white_point: PhantomData<Wp> }
```

- `Lab`: `l` 0..100; `a` (green↔red) and `b` (blue↔yellow) roughly -128..127. Device-
  independent, perceptually-uniform-ish. The standard space for color difference (ΔE);
  docs note it wasn't designed for gamut manipulation — prefer Oklab for editing.
- `Lch`: polar Lab. `l` 0..100; `chroma` 0..~128 (saturated ≈ 128–181); `hue: LabHue` 0..360.
- `Luv`: `l` 0..100; `u`/`v` ranges vary with L (roughly `u` -84..176, `v` -135..108).
  Linear for fixed lightness — suited to additive mixing.
- `Lchuv`: polar Luv. `chroma` 0..~180.
- `Hsluv`: human-friendly Luv picker. **`saturation` is 0..100 here, not 0..1**; `l` 0..100.

## Oklab family — perceptual mixing / gradients (no `Wp`)

```rust
pub struct Oklab<T = f32> { pub l: T, pub a: T, pub b: T }
pub struct Oklch<T = f32> { pub l: T, pub chroma: T, pub hue: OklabHue<T> }
pub struct Okhsl<T = f32>  { pub hue: OklabHue<T>, pub saturation: T, pub lightness: T }
pub struct Okhsv<T = f32>  { pub hue: OklabHue<T>, pub saturation: T, pub value: T }
pub struct Okhwb<T = f32>  { pub hue: OklabHue<T>, pub whiteness: T, pub blackness: T }
```

- `Oklab`: `l` 0..1; `a`/`b` ≈ -0.4..0.4. The modern default for smooth gradients,
  perceptual grayscale, and saturation changes that preserve hue/lightness.
- `Oklch`: polar Oklab — the go-to for hue-preserving manipulation and hue-travel
  gradients. `l` 0..1; `chroma` formally unbounded, practical 0..~0.37 in sRGB gamut;
  `hue: OklabHue` 0..360.
- `Okhsl`/`Okhsv`/`Okhwb`: HSL/HSV/HWB-shaped views of Oklab — perceptually-uniform
  picker axes (`saturation`/`lightness`/`value`/etc. all 0..1). Prefer these over plain
  Hsl/Hsv when slider uniformity matters. All are `<T = f32>` only (no `S`, no `Wp`).

## CIE hub spaces

```rust
pub struct Xyz<Wp = D65, T = f32> { pub x: T, pub y: T, pub z: T,    pub white_point: PhantomData<Wp> }
pub struct Yxy<Wp = D65, T = f32> { pub x: T, pub y: T, pub luma: T, pub white_point: PhantomData<Wp> }
```

- `Xyz`: CIE 1931 — the hub every conversion routes through (see `conversion.md`).
  `y` = luminance 0..1; `x`/`z` scale with white point (D65 ≈ x 0..0.95, z 0..1.089).
- `Yxy`: luminance-chromaticity form. `x`,`y` are chromaticity coords 0..1; `luma` (Y)
  is brightness 0..1. Note field order is `x, y, luma`. Basis of chromaticity diagrams.

## Luma — grayscale

```rust
pub struct Luma<S = Srgb, T = f32> { pub luma: T, pub standard: PhantomData<S> }
```

- Single channel `luma`: 0.0..1.0 (`f32`), 0..255 (`u8`). "Basically the Y of CIE XYZ."
  Carries an `S` standard like RGB, so gamma vs linear matters: `SrgbLuma`, `LinLuma`
  (+ alpha variants). (verify the exact alias names against the `luma` module index.)

## Hue newtypes — `RgbHue` / `LabHue` / `OklabHue` / `LuvHue`

All four share one shape: a circular angle in degrees, `0 == 360`, normalized to
`(-180, 180]` internally, with wrapping arithmetic.

Construct:
- `RgbHue::new(angle)` / `OklabHue::new(angle)` — degrees for floats.
- `::from_degrees(deg)` (alias of `new`), `::from_radians(rad)`.

Read:
- `.into_degrees()` → `(-180, 180]`.
- `.into_positive_degrees()` → `[0, 360)` — **use this for UI display / sliders.**
- `.into_radians()` → `(-π, π]`.
- `.into_raw_degrees()` / `.into_raw_radians()` → unnormalized internal value.

Which space uses which hue: Hsl/Hsv/Hwb → `RgbHue`; Lch → `LabHue`; Lchuv/Hsluv →
`LuvHue`; Oklch/Okhsl/Okhsv/Okhwb → `OklabHue`.

## Verification notes

- All struct signatures are quoted verbatim from rendered docs.rs 0.7.6 except
  `LabHue`/`LuvHue` method lists, which are inferred by symmetry with the verified
  `RgbHue`/`OklabHue` (same type shape, same field usage).
- `a`/`b`, `u`/`v`, and Oklch `chroma` ranges are documented as approximate ("roughly",
  "varies with L"), not hard type bounds.
- `Luma` alpha-alias names (`SrgbLumaa`, etc.) follow palette's naming convention but
  were not read verbatim — confirm before quoting.
