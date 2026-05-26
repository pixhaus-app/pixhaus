---
name: pixhaus-palette
description: >
  Use when writing, reviewing, or debugging any color-space or color-correctness
  work in Pixhaus — building HSL/HSV/Oklch color pickers, perceptual gradients,
  the "nearest palette color" / eyedropper-snap, layer blend modes, lighten/darken/
  saturate brushes, WCAG contrast checks, or converting between Srgb<u8> file bytes
  and the linear RGBA the wgpu canvas blends. palette is the color-management crate
  (Srgb/LinSrgb, Hsl/Hsv/Hwb, Lab/Lch/Luv, Oklab/Oklch/Okhsl, Xyz, the Alpha wrapper,
  Mix/Blend/Compose, color_difference, named CSS colors). Trigger this for ANY
  "convert this color", "mix/blend two colors", "make a gradient", "rotate the hue",
  "is this color out of gamut", "match to the nearest swatch", "check contrast", or
  "why does my blend look muddy/dark" task even when the user doesn't say "palette".
  palette's 0.7 API differs sharply from older examples — the standalone Gradient
  type was REMOVED, the serde feature is named `serializing`, and gamma-vs-linear is
  a type-level distinction that bites every blend — so reach for this skill rather
  than guessing signatures or assuming an API that no longer exists.
---

# palette for Pixhaus

palette is the color-correctness layer: typed color spaces so the compiler stops
you mixing sRGB with linear light, conversions that apply the right transfer
function, and the perceptual spaces (Oklab/Oklch, Lab/Lch) that make gradients,
recoloring, and palette-matching look right instead of muddy. It is the crate
behind color pickers, the gradient tool, blend modes, eyedropper-snap, and
accessibility checks.

This skill is the floor for color work in Pixhaus: the handful of facts that prevent
the recurring bugs (gamma vs linear, the missing Gradient type, the `serializing`
feature name, out-of-gamut after conversion), the version and feature pin, and how
the spaces map onto a pixel-art editor. When you need the full type or method surface,
open the matching file in `references/` — don't guess from memory; palette's API
changed hard at 0.6/0.7 and the references are derived from docs.rs 0.7.6.

## palette vs glam — which color type

Both crates touch color; they don't compete. Keep the boundary clear:

- **glam `Vec4` / `[u8; 4]`** — the GPU and storage path. Linear RGBA the shader
  blends, the bytes in a pixel buffer, uniforms. Fast, dumb, no color-space meaning.
  See [[pixhaus-glam]].
- **palette `Srgb`/`Oklch`/`Lab`/…** — anywhere a color has *meaning*: picker UIs,
  perceptual gradients, palette nearest-match, contrast checks, file import/export
  where gamma encoding matters. Convert to a plain `[f32; 4]`/`Vec4` at the edge
  when handing off to the renderer.

Rule of thumb: if you're doing math *about color* (perception, gamut, hue,
contrast), use palette. If you're moving pixels to the GPU, use glam/bytemuck.

## Version and features — pin these

```toml
palette = { version = "0.7.6", features = ["serializing", "bytemuck"] }
```

- `serializing` is the serde feature — **not** `serde`. It enables `Serialize`/
  `Deserialize` on color types (it implies `std`), so a stored swatch or palette
  drops straight into the `.pixhaus` MessagePack file. See [[pixhaus-rmp-serde]].
  Writing `features = ["serde"]` technically enables the raw optional dep but
  `serializing` is the documented public switch — use it.
- `bytemuck` gives color types `Pod`/`Zeroable`, so `Srgba<u8>` / `LinSrgb<f32>` cast
  to `&[u8]` for a wgpu buffer or texture upload. This is the GPU bridge. See
  [[pixhaus-bytemuck]].
- `named` and `named_from_str` (the CSS color constants + string lookup) are **already
  default** — don't add them. `std`, `approx` are default too.
- Add `wide` only when you do SIMD pixel batching; add `enterpolation` (a separate
  crate) when you need multi-stop gradients — see the gradient note below.

MSRV is 1.60.0 (declared; CI tests 1.71.0). License is MIT OR Apache-2.0 — clears the
[[project-v2-native-restart]] MIT lock. When you bump palette, re-verify the references
against docs.rs — see [[feedback-dep-upgrades]].

## The mental model: four facts that cause most bugs

1. **Gamma vs linear is a type, and math belongs in linear.** `Srgb<u8>` and
   `Srgb<f32>` are *gamma-encoded* — non-linear. Averaging, blending, lightness math,
   or filtering on gamma values is wrong (the classic "midpoint looks too dark"). Move
   to a linear space first: `LinSrgb` for plain blends, `Oklab`/`Oklch` for perceptual
   ones. palette keeps `Srgb` and `LinSrgb` as distinct types precisely so the compiler
   tracks this. The canonical loop is store-u8 → lift-to-f32 → decode-to-linear →
   do-math → clamp → re-encode → quantize-to-u8. See `references/conversion.md`.

2. **`into_format` only rescales the number; `into_linear` changes the encoding.**
   `Srgb<u8>::into_format::<f32>()` maps `0..=255` to `0.0..=1.0` and does **nothing**
   to gamma — it's still gamma-encoded sRGB. `into_linear()` is what applies the sRGB
   transfer function (gamma → linear). Conflating the two is the single most common
   palette bug. Cross integer/float with `into_format` (or `FromStimulus`), never an
   `as` cast — `255u8 as f32` gives `255.0`, not `1.0`. See `references/components-and-alpha.md`.

3. **There is no `Gradient` type in palette 0.7.** The standalone `palette::gradient::
   Gradient` was removed in the 0.6 rework — `palette::gradient::*` does not exist and
   will not compile. Two-color interpolation is the `Mix` trait (`a.mix(b, t)`).
   Multi-stop gradients use the separate **`enterpolation`** crate (palette dev-depends
   on it and uses it in its own examples). Do not write any `Gradient::new`/`.take(n)`/
   `.get(t)` from memory. See `references/gradients-and-named.md`.

4. **Conversion can leave you out of gamut, and palette won't auto-fix it.** A valid
   Oklch or Lab value may have no representable sRGB equivalent. `from_color`/
   `into_color` clamp into the target gamut; `from_color_unclamped`/`into_color_unclamped`
   do the raw math and may return invalid components. Use `is_within_bounds()` to detect
   and `clamp()`/`clamp_assign()` to fix — always before quantizing back to `u8`. See
   `references/conversion.md` and `references/components-and-alpha.md`.

## Rules that prevent the recurring bugs

- **Pick the space by intent, then mix/blend there.** `mix` lerps the components of
  whatever space `Self` is, so the space *is* the algorithm. Oklab/Oklch for smooth
  perceptual gradients and recoloring; `LinSrgb` for physically-correct light blends;
  a hue space (Hsl/Oklch) when you want to travel around the hue circle. Never `mix`
  in gamma `Srgb`.
- **Blend and composite in linear, premultiplied.** The `blend` module's `Compose`
  (Porter-Duff: `over`, `atop`, `xor`, `plus`, …) and `Blend` (the 11 separable modes:
  `multiply`, `screen`, `overlay`, `darken`, `lighten`, `dodge`, `burn`, `hard_light`,
  `soft_light`, `difference`, `exclusion`) operate on `PreAlpha` (premultiplied). Convert
  to a linear space, wrap in `PreAlpha`, blend, `unpremultiply`, re-encode. See
  `references/operations.md`.
- **Nearest-palette-match runs in a perceptual space.** Convert pixels and swatches to
  `Lab` and rank with `Ciede2000` (best) or `EuclideanDistance`/`distance_squared`
  (fast — skip the sqrt when only ranking). Euclidean distance in gamma `Srgb` is the
  wrong metric and produces visibly wrong matches. See `references/operations.md`.
- **Relative vs absolute adjustment is `_fixed`.** `lighten(0.5)` moves *halfway to max*
  (relative); `lighten_fixed(0.5)` *adds 0.5* to lightness (absolute). Same for
  `saturate`/`saturate_fixed`. Use `_fixed` for a constant-step brush, the relative
  form for a multiplicative tweak. `Lighten`/`Saturate` live only on spaces with the
  matching channel.
- **Read hue with `into_positive_degrees()` for UI.** Hue newtypes (`RgbHue`,
  `OklabHue`, `LabHue`, `LuvHue`) normalize to `(-180, 180]`; `into_positive_degrees()`
  gives `[0, 360)` for a picker slider. `get_hue()` returns the hue directly (grays
  return 0), **not** an `Option`.
- **Bridge raw buffers with `cast`, not by hand.** `cast::try_from_component_slice(&[u8])
  -> Result<&[Srgb<u8>], _>` views a flat RGB buffer as typed colors with no copy
  (`from_component_slice` panics on length mismatch; prefer the `try_` form per the
  no-unwrap rule, see [[pixhaus-rust-conventions]]). `cast::into_component_slice` goes
  back. See `references/conversion.md`.

## Pixhaus applications

Where the spaces land in a pixel-art editor on wgpu:

- **Color pickers** are `Hsv`/`Hsl`/`Hwb` (familiar) or `Okhsl`/`Okhsv` (perceptually
  uniform axes — better behaved sliders). Read/write hue via the `*Hue` newtype. Build
  the picker in the chosen space, convert to `Srgb<u8>` for the swatch and to a linear
  `[f32; 4]` for the brush.
- **The gradient tool** interpolates in `Oklch` (hue-travel) or `Oklab` (straight
  blend) for smooth ramps, then converts each sample back to `Srgb`. Two stops →
  `Mix`; N stops → `enterpolation::linear::Linear` over palette colors. See
  `references/gradients-and-named.md`.
- **Layer blend modes** map onto `blend::Blend` / `blend::Compose` in `LinSrgb` via
  `PreAlpha`. "Normal" = `over`; "Multiply"/"Screen"/"Overlay"/"Dodge"/"Burn" are the
  separable modes. Premultiply, blend in linear, unpremultiply, encode.
- **Eyedropper-snap and indexed/GIF export** convert to `Lab` and use `Ciede2000` (or
  `Oklab` + `distance_squared`) to find the nearest swatch. This pairs with the
  NeuQuant quantizer — see [[pixhaus-color-quant]] for building the palette itself;
  palette's color_difference does the per-pixel *matching*. Per-pixel work over an 8K
  canvas is the [[project-8k-perf-constraint]] path: keep it off the egui thread and
  consider the in-place `FromColorMut` conversion to avoid allocating a parallel buffer.
- **Lighten/darken/saturate/hue-rotate adjustments** are the `Lighten`/`Darken`/
  `Saturate`/`Desaturate`/`ShiftHue` traits on `Hsl`/`Oklch`/`Lab`. Use the `_fixed`
  forms for predictable brush steps.
- **Contrast checks** (UI text/legibility on a chosen palette) use
  `Wcag21RelativeContrast`: `relative_contrast()` for the ratio, `has_min_contrast_text()`
  for the 4.5:1 AA gate. Input colors are sRGB-encoded.
- **The `.pixhaus` file** stores swatches/palettes as serde-serialized palette colors
  (the `serializing` feature) — typically `Srgb<u8>` for compactness.

## References

Open the file for the area you're working in; each is a dense API reference for palette
0.7.6, with load-bearing signatures checked against docs.rs.

| File | Covers |
|---|---|
| `references/color-spaces.md` | Every color type — `Rgb`/`Srgb`/`LinSrgb`, `Hsl`/`Hsv`/`Hwb`, `Lab`/`Lch`/`Luv`/`Lchuv`/`Hsluv`, `Oklab`/`Oklch`/`Okhsl`/`Okhsv`/`Okhwb`, `Xyz`/`Yxy`/`Luma` — fields, exact numeric ranges, generic params (`S` standard, `Wp` white point), aliases, the `*Hue` newtypes, when to reach for each |
| `references/conversion.md` | `FromColor`/`IntoColor` (clamped) vs `*Unclamped` vs `*Mut` (in-place); the gamma↔linear pipeline and why; XYZ as the conversion hub; the `cast` module for `Vec<u8>` pixel buffers and packed `u32` |
| `references/components-and-alpha.md` | Component type `T` (u8/u16/f32/f64); the `Stimulus`/`FromStimulus` trait and 255↔1.0; `into_format` vs `into_linear`/`from_linear`/`into_encoding`; the `Alpha<C,T>` wrapper + `WithAlpha`; `Clamp`/`IsWithinBounds`; precision tradeoffs |
| `references/operations.md` | `Mix`/`MixAssign`; `Lighten`/`Darken`/`Saturate`/`Desaturate` (relative vs `_fixed`); hue ops (`ShiftHue`/`GetHue`/`SetHue`/`WithHue`); the `blend` module (`Compose`, `Blend`'s 11 modes, `PreAlpha`); `color_difference` (Ciede2000, DeltaE, EuclideanDistance, HyAb, Wcag21RelativeContrast) |
| `references/gradients-and-named.md` | The no-`Gradient` reality and the `enterpolation` path; which space to interpolate in; the `named` CSS-color module (`named::RED`, `from_str`), constant type `Srgb<u8>` |
| `references/features-and-interop.md` | Exact Cargo feature list and defaults; `serializing` vs `serde`; `bytemuck` GPU casting and the `Pod`/alignment caveat; `wide` SIMD; no_std/`libm`; MSRV/license; recommended Cargo.toml |

A standing caution: the references record the 0.7.6 API faithfully, but a few deep
signatures were flagged during research as unverifiable from the rendered docs (noted
inline as "(verify)"). When one is load-bearing for what you're building, confirm it
against https://docs.rs/palette/0.7.6/palette/ or the source before depending on it.
