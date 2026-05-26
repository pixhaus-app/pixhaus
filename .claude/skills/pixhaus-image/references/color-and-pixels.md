# Color, pixels, errors, limits — image 0.25.10

Signatures verified against docs.rs 0.25.10.

## Table of contents

- `ColorType` vs `ExtendedColorType`
- Pixel structs (`Rgb`/`Rgba`/`Luma`/`LumaA`) and type aliases
- `Pixel`, `Primitive`, `PixelWithColorType` traits
- The `error` module
- `Limits` / `LimitSupport` (untrusted-input guard)
- `metadata::Orientation`

## `ColorType` vs `ExtendedColorType`

Two enums, two jobs. **`ColorType` = what the crate stores and processes in memory.
`ExtendedColorType` = what a file format may declare on the wire.**

### `ColorType` — 10 concrete in-memory layouts

`L8 La8 Rgb8 Rgba8 L16 La16 Rgb16 Rgba16 Rgb32F Rgba32F` (L = luminance/grayscale).

```rust
pub fn bytes_per_pixel(self) -> u8
pub fn has_alpha(self) -> bool
pub fn has_color(self) -> bool      // false for L*/La*
pub fn bits_per_pixel(self) -> u16  // always a multiple of 8
pub fn channel_count(self) -> u8    // 1/2/3/4
```

Pixhaus works in `Rgba8`. `DynamicImage::color()` returns a `ColorType`; check it
when you care whether a decoded file had alpha.

### `ExtendedColorType` — 29 wire formats

Adds sub-byte depths (`L1 La1 Rgb1 Rgba1 L2 ... L4 ... Rgb5x1`), `Bgr8`/`Bgra8`,
`Cmyk8`/`Cmyk16`, `A8` (alpha-only), and `Unknown(u8)`. Used by the encode/decode
free functions (`save_buffer` takes `impl Into<ExtendedColorType>`) and to estimate
conversion cost.

```rust
pub fn channel_count(self) -> u8
pub fn bits_per_pixel(&self) -> u16   // note &self here (ColorType's takes self)
```

Conversions: `From<ColorType> for ExtendedColorType` always succeeds; reverse is
`TryFrom<ExtendedColorType> for ColorType` (fails for extended-only variants, error
`TryFromExtendedColorError` in `image::error`).

## Pixel structs and type aliases

All four are `#[repr(transparent)]` newtypes over `[T; N]`; field `.0` is the array.
The `Pixel` impls assume sRGB.

```rust
#[repr(transparent)] pub struct Luma<T>(pub [T; 1]);   // [luma]            CHANNEL_COUNT 1, no alpha
#[repr(transparent)] pub struct LumaA<T>(pub [T; 2]);  // [luma, alpha]     CHANNEL_COUNT 2, alpha
#[repr(transparent)] pub struct Rgb<T>(pub [T; 3]);    // [r, g, b]         CHANNEL_COUNT 3, no alpha
#[repr(transparent)] pub struct Rgba<T>(pub [T; 4]);   // [r, g, b, a]      CHANNEL_COUNT 4, alpha
```

Construct directly: `Rgba([255u8, 0, 0, 255])`. Index channels via `.0[i]` or
`.channels()`. Concrete aliases the crate uses widely: `Rgba<u8>` (the Pixhaus
pixel), `Rgb<u8>`, `Luma<u8>`, `LumaA<u8>`, and the `u16`/`f32` widths.

Pixel type → `ExtendedColorType` (the `PixelWithColorType::COLOR_TYPE` that drives
`save`'s format detection):

| `u8` | `u16` | `f32` |
|---|---|---|
| `Luma<u8>`→`L8` | `Luma<u16>`→`L16` | (no concrete ColorType for f32 luma) |
| `LumaA<u8>`→`La8` | `LumaA<u16>`→`La16` | — |
| `Rgb<u8>`→`Rgb8` | `Rgb<u16>`→`Rgb16` | `Rgb<f32>`→`Rgb32F` |
| `Rgba<u8>`→`Rgba8` | `Rgba<u16>`→`Rgba16` | `Rgba<f32>`→`Rgba32F` |

## `Pixel`, `Primitive`, `PixelWithColorType`

### `Pixel`

```rust
pub trait Pixel {
    type Subpixel: Primitive;
    const CHANNEL_COUNT: u8;
    const COLOR_MODEL: &'static str;
    const HAS_ALPHA: bool = false;

    fn channels(&self) -> &[Self::Subpixel];
    fn channels_mut(&mut self) -> &mut [Self::Subpixel];
    fn channels4(&self) -> (Self::Subpixel, Self::Subpixel, Self::Subpixel, Self::Subpixel);
    fn from_channels(a: Self::Subpixel, b: Self::Subpixel, c: Self::Subpixel, d: Self::Subpixel) -> Self;
    fn from_slice(slice: &[Self::Subpixel]) -> &Self;          // view a slice as one pixel
    fn from_slice_mut(slice: &mut [Self::Subpixel]) -> &mut Self;
    fn to_rgb(&self) -> Rgb<Self::Subpixel>;
    fn to_rgba(&self) -> Rgba<Self::Subpixel>;
    fn to_luma(&self) -> Luma<Self::Subpixel>;
    fn to_luma_alpha(&self) -> LumaA<Self::Subpixel>;
    fn map<F: FnMut(Self::Subpixel) -> Self::Subpixel>(&self, f: F) -> Self;
    fn apply<F: FnMut(Self::Subpixel) -> Self::Subpixel>(&mut self, f: F);
    fn map_with_alpha<F, G>(&self, f: F, g: G) -> Self;        // f over color, g over alpha
    fn apply_with_alpha<F, G>(&mut self, f: F, g: G);
    fn map2<F>(&self, other: &Self, f: F) -> Self;             // combine two pixels
    fn apply2<F>(&mut self, other: &Self, f: F);
    fn invert(&mut self);
    fn blend(&mut self, other: &Self);                         // alpha-composite other ONTO self
    // provided:
    fn alpha(&self) -> Self::Subpixel;
    fn map_without_alpha<F>(&self, f: F) -> Self;
    fn apply_without_alpha<F>(&mut self, f: F);
}
```

`blend(other)` mutates `self`. `from_slice`/`from_slice_mut` reinterpret a 4-element
`&[u8]` as a `&Rgba<u8>` without copying — handy when walking a flat buffer with
`chunks_exact(4)`.

### `Primitive`

```rust
pub trait Primitive: Copy + NumCast + Num + PartialOrd + Clone + Bounded {
    const DEFAULT_MAX_VALUE: Self;   // 1.0 for floats, type MAX for ints (255 for u8)
    const DEFAULT_MIN_VALUE: Self;   // 0.0 / 0
}
```

Implemented for `i8 i16 i32 i64 isize u8 u16 u32 u64 usize f32 f64`. The `Num*` /
`Bounded` bounds come from `num-traits`.

### `PixelWithColorType`

```rust
pub trait PixelWithColorType: Pixel + /* sealed */ {
    const COLOR_TYPE: ExtendedColorType;
}
```

Sealed and not dyn-compatible — you can't implement it for your own pixel type. It's
the bound on `save`/`write_to`/`write_with_encoder` (the crate needs to know the
on-disk color type). Implemented for the concrete aliases above.

## The `error` module

```rust
pub type ImageResult<T> = Result<T, ImageError>;

pub enum ImageError {
    Decoding(DecodingError),
    Encoding(EncodingError),
    Parameter(ParameterError),       // bad argument; kind ParameterErrorKind
    Limits(LimitError),              // a Limits cap exceeded; kind LimitErrorKind
    Unsupported(UnsupportedError),   // format/feature not built in; kind UnsupportedErrorKind
    IoError(std::io::Error),
}
```

`ImageError: std::error::Error` and `From<std::io::Error>`, so `?` on file I/O folds
into `ImageError::IoError`. `ImageFormatHint` is carried inside decode/encode/
unsupported errors as a best-effort format identifier. `TryFromExtendedColorError`
is the `ExtendedColorType` → `ColorType` conversion failure.

Pixhaus handling (`pixhaus-thiserror`, `pixhaus-rust-conventions`): in the `io`
library crate, give the crate's error enum a `#[from] image::ImageError` variant and
let `?` convert. Don't leak `ImageError` through public APIs; don't `unwrap()` a
decode result outside tests — a corrupt file is a reportable user error.
`ImageError::Limits` specifically means a hostile/huge file tripped your guard —
surface it as "image too large", not a generic failure.

## `Limits` / `LimitSupport` — untrusted-input guard

```rust
pub struct Limits {
    pub max_image_width: Option<u32>,
    pub max_image_height: Option<u32>,
    pub max_alloc: Option<u64>,
    // #[non_exhaustive] — build via default()/no_limits(), then set fields
}
pub fn default() -> Limits;     // NOT unlimited: max_alloc ~512 MiB (the bomb guard)
pub fn no_limits() -> Limits;   // disables ALL constraints (the opt-out)
pub fn check_dimensions(&self, width: u32, height: u32) -> ImageResult<()>;
pub fn check_support(&self, supported: &[LimitSupport]) -> ImageResult<()>;
pub fn reserve(&mut self, amount: u64) -> ImageResult<()>;
pub fn reserve_usize(&mut self, amount: usize) -> ImageResult<()>;
pub fn reserve_buffer(&mut self, width: u32, height: u32, color_type: ColorType) -> ImageResult<()>;
pub fn free(&mut self, amount: u64);
pub fn free_usize(&mut self, amount: usize);

#[non_exhaustive] pub struct LimitSupport {}   // empty forward-compat placeholder today
```

The trap: **`Limits::default()` is not unlimited** — it caps allocation at ~512 MiB,
which is the decompression-bomb defense. `no_limits()` is the deliberate opt-out (use
it only for files you produced). For untrusted input (user/plugin files), start from
`default()`, tighten `max_image_width`/`max_image_height` to a project ceiling, set
the limits on the `ImageReader` (or decoder via `set_limits`), and let `decode()`
return `ImageError::Limits` on a file that exceeds them. Decoding an 8K canvas is
legitimate — size the caps to the real max Pixhaus supports, not below it. (The
~512 MiB default is documented as a current value, not a stability guarantee — pin
your own cap rather than relying on it.)

## `metadata::Orientation` — EXIF fixup

Phone photos store an orientation flag instead of physically rotating pixels; a
reference image imported without honoring it shows up sideways.

```rust
pub enum Orientation {
    NoTransforms, Rotate90, Rotate180, Rotate270,
    FlipHorizontal, FlipVertical, Rotate90FlipH, Rotate270FlipH,
}
pub fn from_exif(exif_orientation: u8) -> Option<Self>;   // EXIF tag value 1..=8
pub fn to_exif(self) -> u8;
pub fn from_exif_chunk(chunk: &[u8]) -> Option<Self>;
pub fn remove_from_exif_chunk(chunk: &mut [u8]) -> Option<Self>;
```

Apply it: read `decoder.orientation()?` (the `ImageDecoder` provided method), then
`dynamic_image.apply_orientation(orientation)` to rotate/flip the pixels into the
intended layout. Do this on import of any photographic reference. (The `metadata`
module also carries CICP color-management types — `Cicp`, `CicpTransform`, etc. —
and `LoopCount` for animation; unrelated to orientation.)
