# Buffers, dynamic images, views — image 0.25.10

Signatures verified against docs.rs 0.25.10. The raw-`Vec<u8>` bridge in SKILL.md
is the single most-used path; this file is the full surface behind it.

## Table of contents

- `ImageBuffer<P, Container>` and the type aliases
- `DynamicImage`
- `GenericImageView` / `GenericImage` traits
- `SubImage`
- `FlatSamples` (zero-copy foreign-buffer interop)

## `ImageBuffer<P, Container>`

```rust
pub struct ImageBuffer<P: Pixel, Container> { /* ... */ }
```

`P` is the pixel type (`Rgba<u8>`), `Container` the backing subpixel store
(`Vec<u8>`). `P::Subpixel` is the scalar (`u8`).

### Type aliases (re-exported at crate root) — use these names

```rust
pub type RgbImage       = ImageBuffer<Rgb<u8>,   Vec<u8>>;
pub type RgbaImage      = ImageBuffer<Rgba<u8>,  Vec<u8>>;   // <- the Pixhaus type
pub type GrayImage      = ImageBuffer<Luma<u8>,  Vec<u8>>;
pub type GrayAlphaImage = ImageBuffer<LumaA<u8>, Vec<u8>>;
pub type Rgb32FImage    = ImageBuffer<Rgb<f32>,  Vec<f32>>;
pub type Rgba32FImage   = ImageBuffer<Rgba<f32>, Vec<f32>>;
```

`RgbaImage`'s container *is* `Vec<u8>`, which is why `from_raw`/`into_raw` move
Pixhaus's pixel buffer in and out without copying.

### Construction (Vec-backed)

```rust
impl<P: Pixel> ImageBuffer<P, Vec<P::Subpixel>> {
    pub fn new(width: u32, height: u32) -> Self                       // zeroed
    pub fn from_pixel(width: u32, height: u32, pixel: P) -> Self       // filled
    pub fn from_fn<F: FnMut(u32, u32) -> P>(width: u32, height: u32, f: F) -> Self
    pub fn from_vec(width: u32, height: u32, buf: Vec<P::Subpixel>) -> Option<Self>
    pub fn into_vec(self) -> Vec<P::Subpixel>
}
```

### Construction / raw access (any `Deref` container)

```rust
impl<P: Pixel, Container: Deref<Target = [P::Subpixel]>> ImageBuffer<P, Container> {
    pub fn from_raw(width: u32, height: u32, buf: Container) -> Option<Self>  // None if buf too small
    pub fn into_raw(self) -> Container
    pub fn as_raw(&self) -> &Container
}
```

`from_raw`/`from_vec` return `None` when `buf.len() < width*height*channels` — they
never reallocate. For `RgbaImage`, `from_raw` and `from_vec` are interchangeable
(`Container = Vec<u8>`). This is THE bytes→image entry; `into_raw`/`into_vec` is the
image→bytes exit.

### Pixel access

```rust
pub fn get_pixel(&self, x: u32, y: u32) -> &P                        // panics out of bounds
pub fn get_pixel_checked(&self, x: u32, y: u32) -> Option<&P>
pub fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut P            // panics out of bounds
pub fn get_pixel_mut_checked(&mut self, x: u32, y: u32) -> Option<&mut P>
pub fn put_pixel(&mut self, x: u32, y: u32, pixel: P)
```

The inherent `get_pixel` returns `&P` (reference). The `GenericImageView` *trait*
method of the same name returns `P` by value — see the trait section. For
predictable bounds behavior in non-test code, prefer `get_pixel_checked`. Per-pixel
get/put is fine for sparse edits; for whole-buffer work iterate or slice instead
(it's the per-pixel function-call overhead, not bounds checks, that adds up).

### Iteration

```rust
pub fn pixels(&self) -> Pixels<'_, P>                       // &P
pub fn pixels_mut(&mut self) -> PixelsMut<'_, P>            // &mut P
pub fn enumerate_pixels(&self) -> EnumeratePixels<'_, P>    // (u32, u32, &P)
pub fn enumerate_pixels_mut(&mut self) -> EnumeratePixelsMut<'_, P>  // (u32, u32, &mut P)
pub fn rows(&self) -> Rows<'_, P>
pub fn rows_mut(&mut self) -> RowsMut<'_, P>
pub fn enumerate_rows(&self) -> EnumerateRows<'_, P>
pub fn enumerate_rows_mut(&mut self) -> EnumerateRowsMut<'_, P>
```

With the `rayon` feature: `par_pixels`, `par_pixels_mut`, `par_enumerate_pixels(_mut)`,
`from_par_fn` (bounds `P: Send + Sync`, `P::Subpixel: Send + Sync`). Use the parallel
variants for full-buffer passes inside a `spawn_blocking` task at 8K — see
[[pixhaus-rayon]]. (Iterator `Item` types above are the documented behavior; the
module index renders names only — verify on each struct page if a signature is
load-bearing.)

### Dimensions, raw, flat samples

```rust
pub fn dimensions(&self) -> (u32, u32)
pub fn width(&self) -> u32
pub fn height(&self) -> u32
pub fn sample_layout(&self) -> SampleLayout
pub fn as_flat_samples(&self) -> FlatSamples<&[P::Subpixel]>
pub fn as_flat_samples_mut(&mut self) -> FlatSamples<&mut [P::Subpixel]>   // Container: AsMut
pub fn into_flat_samples(self) -> FlatSamples<Container>                    // Container: AsRef
```

### Save / write / encode

```rust
impl<P, Container> ImageBuffer<P, Container>
where P: Pixel, [P::Subpixel]: EncodableLayout, Container: Deref<Target = [P::Subpixel]>
{
    pub fn save<Q: AsRef<Path>>(&self, path: Q) -> ImageResult<()> where P: PixelWithColorType;
    pub fn save_with_format<Q: AsRef<Path>>(&self, path: Q, format: ImageFormat) -> ImageResult<()>
        where P: PixelWithColorType;
    pub fn write_to<W: Write + Seek>(&self, writer: &mut W, format: ImageFormat) -> ImageResult<()>
        where P: PixelWithColorType;                                    // writer by &mut
    pub fn write_with_encoder<E: ImageEncoder>(&self, encoder: E) -> ImageResult<()>
        where P: PixelWithColorType;
}
```

`save` picks the format from the path extension. **Note `ImageBuffer::write_to`
takes `&mut W`; `DynamicImage::write_to` takes `W` by value** — a real difference.
`write_with_encoder` is how you pass a tuned `PngEncoder`/`JpegEncoder`.

### Whole-buffer pixel-type conversion — the `ConvertBuffer` trait

```rust
pub trait ConvertBuffer<T> { fn convert(&self) -> T; }
// let rgb: RgbImage = rgba_image.convert();
```

`convert` is NOT inherent on `ImageBuffer`; it comes from `ConvertBuffer`. Usually
you don't need it — `DynamicImage::into_rgba8` covers the common path.

## `DynamicImage`

The enum a decoded file lands in — one variant per pixel layout.

```rust
pub enum DynamicImage {
    ImageLuma8(GrayImage),                ImageLumaA8(GrayAlphaImage),
    ImageRgb8(RgbImage),                  ImageRgba8(RgbaImage),
    ImageLuma16(ImageBuffer<Luma<u16>, Vec<u16>>),   ImageLumaA16(ImageBuffer<LumaA<u16>, Vec<u16>>),
    ImageRgb16(ImageBuffer<Rgb<u16>, Vec<u16>>),     ImageRgba16(ImageBuffer<Rgba<u16>, Vec<u16>>),
    ImageRgb32F(Rgb32FImage),             ImageRgba32F(Rgba32FImage),
}
```

### Constructors

```rust
pub fn new(w: u32, h: u32, color: ColorType) -> DynamicImage
pub fn new_luma8 / new_luma_a8 / new_rgb8 / new_rgba8 (w, h) -> DynamicImage
pub fn new_luma16 / new_luma_a16 / new_rgb16 / new_rgba16 / new_rgb32f / new_rgba32f (w, h) -> DynamicImage
pub fn from_decoder(decoder: impl ImageDecoder) -> ImageResult<Self>
// also From<RgbaImage>, From<RgbImage>, ... : DynamicImage::from(rgba_image)
```

### Conversions — three flavors

```rust
// to_*  : borrow, ALWAYS allocates a converted copy. Keeps the DynamicImage.
pub fn to_rgba8(&self) -> RgbaImage      pub fn to_rgb8(&self) -> RgbImage
pub fn to_luma8(&self) -> GrayImage      pub fn to_luma_alpha8(&self) -> GrayAlphaImage
// ...16 and 32f variants: to_rgba16/to_rgb16/to_luma16/to_rgba32f/to_rgb32f/to_luma32f/...

// into_* : consume, REUSES the buffer if the variant already matches, else converts.
pub fn into_rgba8(self) -> RgbaImage     pub fn into_rgb8(self) -> RgbImage
pub fn into_luma8(self) -> GrayImage     pub fn into_luma_alpha8(self) -> GrayAlphaImage
// ...16 variants. (No into_*32f-luma forms — verify if you need 32f luma.)

// as_* : borrow the inner buffer if the variant matches, else None. Zero-copy.
pub fn as_rgba8(&self) -> Option<&RgbaImage>     pub fn as_mut_rgba8(&mut self) -> Option<&mut RgbaImage>
pub fn as_rgb8 / as_luma8 / as_luma_alpha8 (and 16/32f) -> Option<&...>
```

Pixhaus rule: import via `into_rgba8()` (free when already RGBA8). Use `to_rgba8()`
only when you still need the `DynamicImage`. `as_rgba8()` when you just want to peek
without converting and can handle the `None`.

### Bytes and metadata

```rust
pub fn as_bytes(&self) -> &[u8]      // raw subpixel bytes of the CURRENT variant
pub fn into_bytes(self) -> Vec<u8>   // consume to the current variant's bytes
pub fn color(&self) -> ColorType
pub fn width(&self) -> u32   pub fn height(&self) -> u32   pub fn has_alpha(&self) -> bool
```

`as_bytes`/`into_bytes` give whatever the current variant holds — for an
`ImageLuma8` that's 1 byte/pixel, not RGBA. Force the layout first
(`into_rgba8()`) if you need a guaranteed RGBA8 stream.

### Transform convenience methods (return a new `DynamicImage` unless noted)

```rust
pub fn crop(&mut self, x: u32, y: u32, width: u32, height: u32) -> DynamicImage   // &mut self
pub fn crop_imm(&self, x: u32, y: u32, width: u32, height: u32) -> DynamicImage
pub fn resize(&self, nw: u32, nh: u32, filter: FilterType) -> DynamicImage         // preserves aspect
pub fn resize_exact(&self, nw: u32, nh: u32, filter: FilterType) -> DynamicImage
pub fn resize_to_fill(&self, nw: u32, nh: u32, filter: FilterType) -> DynamicImage // crops to fill
pub fn thumbnail(&self, nw: u32, nh: u32) -> DynamicImage                          // fast, averages
pub fn thumbnail_exact(&self, nw: u32, nh: u32) -> DynamicImage
pub fn blur(&self, sigma: f32) -> DynamicImage          pub fn fast_blur(&self, sigma: f32) -> DynamicImage
pub fn unsharpen(&self, sigma: f32, threshold: i32) -> DynamicImage
pub fn filter3x3(&self, kernel: &[f32]) -> DynamicImage  // 9-element kernel
pub fn adjust_contrast(&self, c: f32) -> DynamicImage    pub fn brighten(&self, value: i32) -> DynamicImage
pub fn huerotate(&self, value: i32) -> DynamicImage      pub fn grayscale(&self) -> DynamicImage
pub fn invert(&mut self)                                  // IN PLACE, returns ()
pub fn flipv(&self) -> DynamicImage   pub fn fliph(&self) -> DynamicImage
pub fn rotate90(&self) -> DynamicImage   pub fn rotate180(&self) -> DynamicImage   pub fn rotate270(&self) -> DynamicImage
pub fn apply_orientation(&mut self, orientation: Orientation)   // EXIF fixup, in place
```

`resize` for pixel art → pass `FilterType::Nearest`. `crop` borrows `&mut self` and
returns the cropped copy; `crop_imm` borrows `&self`. `invert` and
`apply_orientation` mutate in place.

### I/O

```rust
pub fn write_to<W: Write + Seek>(&self, w: W, format: ImageFormat) -> ImageResult<()>  // W BY VALUE
pub fn write_with_encoder(&self, encoder: impl ImageEncoder) -> ImageResult<()>
pub fn save<Q: AsRef<Path>>(&self, path: Q) -> ImageResult<()>
pub fn save_with_format<Q: AsRef<Path>>(&self, path: Q, format: ImageFormat) -> ImageResult<()>
```

## `GenericImageView` / `GenericImage`

The traits that `imageops` and generic code are written against.

```rust
pub trait GenericImageView {
    type Pixel: Pixel;
    fn dimensions(&self) -> (u32, u32);                  // required
    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel;  // required — BY VALUE (not &Pixel)
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn in_bounds(&self, x: u32, y: u32) -> bool;
    unsafe fn unsafe_get_pixel(&self, x: u32, y: u32) -> Self::Pixel;   // BANNED here (no unsafe)
    fn pixels(&self) -> Pixels<'_, Self> where Self: Sized;
    fn view(&self, x: u32, y: u32, width: u32, height: u32) -> SubImage<&Self> where Self: Sized;
    fn try_view(&self, x: u32, y: u32, width: u32, height: u32)
        -> Result<SubImage<&Self>, ImageError> where Self: Sized;
}

pub trait GenericImage: GenericImageView {
    fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut Self::Pixel;    // required
    fn put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel);        // required
    fn blend_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel);      // required, alpha-composites
    unsafe fn unsafe_put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel);  // BANNED here
    fn copy_from<O>(&mut self, other: &O, x: u32, y: u32) -> ImageResult<()>
        where O: GenericImageView<Pixel = Self::Pixel>;                // same pixel type required
    fn copy_within(&mut self, source: Rect, x: u32, y: u32) -> bool;   // false if out of bounds
    fn sub_image(&mut self, x: u32, y: u32, width: u32, height: u32)
        -> SubImage<&mut Self> where Self: Sized;
}
```

`GenericImageView::get_pixel` returns the pixel by value (cheap — pixels are tiny
`[T; N]`). The `unsafe_*` methods exist but the workspace forbids `unsafe`
(`pixhaus-rust-conventions`): never call them. `copy_from` requires a matching pixel
type and errors if the source doesn't fit at the offset.

## `SubImage<I>` — a non-owning window

```rust
pub fn new(image: I, x: u32, y: u32, width: u32, height: u32) -> SubImage<I>
pub fn change_bounds(&mut self, x: u32, y: u32, width: u32, height: u32)
pub fn offsets(&self) -> (u32, u32)
// where I: Deref, I::Target: GenericImageView + 'static
pub fn to_image(&self) -> ImageBuffer<Pixel, Vec<Subpixel>>   // materialize the window -> owned buffer
pub fn view(&self, x: u32, y: u32, width: u32, height: u32) -> SubImage<&I::Target>
pub fn inner(&self) -> &I::Target
// where I: DerefMut, I::Target: GenericImage
pub fn sub_image(&mut self, ...) -> SubImage<&mut I::Target>
pub fn inner_mut(&mut self) -> &mut I::Target
```

A `SubImage` borrows another image and stores an offset + size — no pixel copy. It
implements `GenericImageView`/`GenericImage` (via deref), so you can read/write
through it. `imageops::crop_imm(&img, x, y, w, h)` returns one; `.to_image()`
copies the window into a fresh owned buffer. This is the natural way to slice a
frame out of a sprite sheet without allocating until you commit.

## `FlatSamples` — zero-copy foreign-buffer interop

```rust
pub struct FlatSamples<Buffer> {
    pub samples: Buffer,             // contiguous store (Vec<u8>, &[u8], &mut [u8])
    pub layout: SampleLayout,
    pub color_hint: Option<ColorType>,
}
#[repr(C)] pub struct SampleLayout {
    pub channels: u8, pub channel_stride: usize,
    pub width: u32,  pub width_stride: usize,
    pub height: u32, pub height_stride: usize,
}
impl SampleLayout {
    pub fn row_major_packed(channels: u8, width: u32, height: u32) -> Self
    pub fn column_major_packed(channels: u8, width: u32, height: u32) -> Self
}
// FlatSamples methods
pub fn as_view<P: Pixel>(&self) -> Result<View<&[P::Subpixel], P>, Error> where Buffer: AsRef<[P::Subpixel]>
pub fn as_view_mut<P: Pixel>(&mut self) -> Result<ViewMut<&mut [P::Subpixel], P>, Error> where Buffer: AsMut<...>
pub fn try_into_buffer<P>(self) -> Result<ImageBuffer<P, Buffer>, (Error, Self)>
    where P: Pixel + 'static, P::Subpixel: 'static, Buffer: Deref<Target = [P::Subpixel]>
```

When another library or FFI hands you a buffer described by strides (not necessarily
packed row-major), wrap it in `FlatSamples` and `try_into_buffer::<Rgba<u8>>()` to
get an `ImageBuffer` without a copy — it fails (returning the samples back) if the
layout isn't `ImageBuffer`-compatible. Pixhaus's own buffers are already packed
row-major RGBA8, so for those `RgbaImage::from_raw` is simpler; reach for
`FlatSamples` only at a foreign-stride boundary.
