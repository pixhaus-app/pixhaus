# imageops — image 0.25.10

Offline raster operations. Signatures verified against per-function docs.rs pages
(the module index page lists descriptions only). `colorops` and `sample` are
submodules whose items are re-exported, so each is reachable as `imageops::foo` or
`imageops::colorops::foo` / `imageops::sample::foo`.

Common shape: buffer-returning ops are generic over `I: GenericImageView` and return
`ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>>`. In-place ops take
`&mut I` where `I: GenericImage`.

## Allocates vs mutates — read this first

| Mutates in place (`&mut`) | Returns a new buffer / view |
|---|---|
| `invert` | `resize`, `thumbnail`, `blur`, `fast_blur`, `unsharpen`, `filter3x3` |
| `overlay`, `replace`, `tile` | `rotate90/180/270`, `flip_horizontal/vertical` |
| `vertical_gradient`, `horizontal_gradient` | `crop`, `crop_imm` (return a `SubImage` view) |
| `dither` | `index_colors`, `grayscale`, `brighten`, `contrast`, `huerotate` |
| `flip_*_in_place`, `rotate180_in_place` | `sample_*`, `interpolate_*` (return `Option<P>`) |

Picking the wrong half either wastes an allocation or silently no-ops. The
`DynamicImage` convenience methods (`img.resize(..)`, `img.invert()`) wrap these.

## 1. Resize / scale — and the pixel-art filter rule

```rust
pub fn resize<I: GenericImageView>(image: &I, nwidth: u32, nheight: u32, filter: FilterType)
    -> ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>>
    where I::Pixel: 'static, <I::Pixel as Pixel>::Subpixel: 'static;

pub fn thumbnail<I, P, S>(image: &I, new_width: u32, new_height: u32) -> ImageBuffer<P, Vec<S>>
    where I: GenericImageView<Pixel = P>, P: Pixel<Subpixel = S> + 'static, S: Primitive + Enlargeable + 'static;
```

`thumbnail` is a fast integer box filter (each source pixel → one target on
downscale); lower quality than `resize`, no filter choice. `resize` preserves the
chosen `FilterType`.

```rust
pub enum FilterType { Nearest, Triangle, CatmullRom, Gaussian, Lanczos3 }
```

| Filter | Kind | Use for |
|---|---|---|
| `Nearest` | nearest neighbor | **pixel art** — keeps hard edges, no blending; also fastest |
| `Triangle` | linear | mild smoothing |
| `CatmullRom` | cubic | good photo downscale |
| `Lanczos3` | windowed sinc | best photo downscale, slowest |
| `Gaussian` | gaussian | soft blur-on-resize |

Docs' own timing (release, one image): Nearest ~31 ms, Triangle ~414, CatmullRom
~817, Lanczos3 ~1170, Gaussian ~1180.

**The rule:** scale pixel art with `FilterType::Nearest`. Every other variant
interpolates and blurs the sprite. Use the smoothing filters (`Lanczos3`,
`CatmullRom`) only for photographic reference images. Avoid `thumbnail` for sprites
— its averaging downscale softens them too.

## 2. Blur / sharpen / convolution

```rust
pub fn blur<I: GenericImageView>(image: &I, sigma: f32)
    -> ImageBuffer<I::Pixel, Vec<...>> where I::Pixel: 'static;          // gaussian
pub fn fast_blur<P: Pixel>(input_buffer: &ImageBuffer<P, Vec<P::Subpixel>>, sigma: f32)
    -> ImageBuffer<P, Vec<P::Subpixel>>;                                 // approx gaussian, returns NEW buffer
pub fn unsharpen<I, P, S>(image: &I, sigma: f32, threshold: i32) -> ImageBuffer<P, Vec<S>>
    where I: GenericImageView<Pixel = P>, P: Pixel<Subpixel = S> + 'static, S: Primitive + 'static;
pub fn filter3x3<I, P, S>(image: &I, kernel: &[f32]) -> ImageBuffer<P, Vec<S>>  // kernel must be 9 elems
    where I: GenericImageView<Pixel = P>, P: Pixel<Subpixel = S> + 'static, S: Primitive + 'static;
```

Despite the name, `fast_blur` returns a new buffer in 0.25.10 (takes `&ImageBuffer`),
it does not mutate in place.

## 3. Crop — views, not copies

```rust
pub fn crop<I: GenericImageView>(image: &mut I, x: u32, y: u32, width: u32, height: u32) -> SubImage<&mut I>;
pub fn crop_imm<I: GenericImageView>(image: &I, x: u32, y: u32, width: u32, height: u32) -> SubImage<&I>;
```

Both return a `SubImage` borrowing the source — no allocation. Call `.to_image()` on
the result to get an owned `ImageBuffer`. `crop_imm` is the one to reach for when
slicing frames out of a sprite sheet:

```rust
let frame: RgbaImage = imageops::crop_imm(&sheet, fx, fy, fw, fh).to_image();
```

## 4. Overlay / replace / tile / gradients — in place, same pixel type

All four mutate `bottom` and require `J::Pixel == I::Pixel`.

```rust
pub fn overlay<I, J>(bottom: &mut I, top: &J, x: i64, y: i64)            // alpha-composite top onto bottom
    where I: GenericImage, J: GenericImageView<Pixel = I::Pixel>;
pub fn replace<I, J>(bottom: &mut I, top: &J, x: i64, y: i64)           // copy over, no blend
    where I: GenericImage, J: GenericImageView<Pixel = I::Pixel>;
pub fn tile<I, J>(bottom: &mut I, top: &J)                              // repeat top across bottom
    where I: GenericImage, J: GenericImageView<Pixel = I::Pixel>;
pub fn vertical_gradient<S, P, I>(img: &mut I, start: &P, stop: &P)     // fill existing image
    where I: GenericImage<Pixel = P>, P: Pixel<Subpixel = S> + 'static, S: Primitive + Lerp + 'static;
pub fn horizontal_gradient<S, P, I>(img: &mut I, start: &P, stop: &P)
    where I: GenericImage<Pixel = P>, P: Pixel<Subpixel = S> + 'static, S: Primitive + Lerp + 'static;
```

`x`/`y` are `i64`, so negative offsets (top hanging off the left/top edge) are fine.
`overlay` is the layer-compositing primitive; `replace` is the "stamp a frame into a
sheet" primitive. Gradients fill a pre-existing mutable image — they don't allocate
or take dimensions.

## 5. Flips and rotations

Allocating (`I: GenericImageView`, `I::Pixel: 'static`), return a new buffer:

```rust
pub fn rotate90<I>(image: &I) -> ImageBuffer<I::Pixel, Vec<...>>     // also rotate180, rotate270
pub fn flip_horizontal<I>(image: &I) -> ImageBuffer<I::Pixel, Vec<...>>   // also flip_vertical
```

In place (`I: GenericImage`):

```rust
pub fn flip_horizontal_in_place<I: GenericImage>(image: &mut I)
pub fn flip_vertical_in_place<I: GenericImage>(image: &mut I)
pub fn rotate180_in_place<I: GenericImage>(image: &mut I)
```

Only `rotate180` has an in-place form — `rotate90`/`rotate270` swap width and height
so they can't write back into the same buffer.

Write-into-destination (return `ImageResult<()>`, fill a caller-owned buffer):

```rust
pub fn rotate90_in<I, Container>(image: &I, destination: &mut ImageBuffer<I::Pixel, Container>) -> ImageResult<()>
    where I: GenericImageView, I::Pixel: 'static, Container: DerefMut<Target = [<I::Pixel as Pixel>::Subpixel]>;
// rotate180_in, rotate270_in, flip_horizontal_in, flip_vertical_in follow the same shape
```

The `_in` forms let you reuse an allocation across calls — useful in a hot path that
rotates many frames.

**No arbitrary-angle rotation exists in `imageops` for 0.25.10** —
`rotate_about_center` is gone. Only the 90/180/270 cardinal rotations. Arbitrary
rotation of a selection is a wgpu-shader or hand-rolled job.

## 6. Color adjustments (`colorops`)

```rust
pub fn brighten<I, P, S>(image: &I, value: i32) -> ImageBuffer<P, Vec<S>>   // -darken / +lighten
pub fn contrast<I, P, S>(image: &I, contrast: f32) -> ImageBuffer<P, Vec<S>>
pub fn huerotate<I, P, S>(image: &I, value: i32) -> ImageBuffer<P, Vec<S>>  // value in DEGREES; 0/360 = no-op
    // all three: I: GenericImageView<Pixel = P>, P: Pixel<Subpixel = S> + 'static, S: Primitive + 'static
pub fn invert<I: GenericImage>(image: &mut I)                               // IN PLACE
pub fn grayscale<I: GenericImageView>(image: &I)
    -> ImageBuffer<Luma<<I::Pixel as Pixel>::Subpixel>, Vec<...>>;          // drops alpha
pub fn grayscale_with_type<NewPixel, I>(image: &I) -> ImageBuffer<NewPixel, Vec<NewPixel::Subpixel>>
    where NewPixel: Pixel + FromColor<Luma<...>>, I: GenericImageView;
```

In-place twins `brighten_in_place` / `contrast_in_place` / `huerotate_in_place`, and
alpha-preserving `grayscale_alpha` / `grayscale_with_type_alpha`, also exist (pull
their exact generic headers from the per-fn pages if you need them). `invert` mutates
in place and returns `()`.

## 7. Dithering and color mapping (`colorops`)

```rust
pub trait ColorMap {
    type Color;
    fn index_of(&self, color: &Self::Color) -> usize;   // nearest palette index
    fn map_color(&self, color: &mut Self::Color);        // snap color to nearest palette entry, in place
    fn lookup(&self, index: usize) -> Option<Self::Color> { /* default None */ }
    fn has_lookup(&self) -> bool { /* default false */ }
}
pub struct BiLevel;   // ColorMap with Color = Luma<u8>: black/white two-color map

pub fn dither<Pix, Map>(image: &mut ImageBuffer<Pix, Vec<u8>>, color_map: &Map)   // IN PLACE, Floyd–Steinberg
    where Map: ColorMap<Color = Pix> + ?Sized, Pix: Pixel<Subpixel = u8> + 'static;
pub fn index_colors<Pix, Map>(image: &ImageBuffer<Pix, Vec<u8>>, color_map: &Map)
    -> ImageBuffer<Luma<u8>, Vec<u8>>                                              // NEW index image, no dither
    where Map: ColorMap<Color = Pix> + ?Sized, Pix: Pixel<Subpixel = u8> + 'static;
```

`dither` writes the palette-mapped result back into the image with error diffusion;
`index_colors` returns a `Luma<u8>` buffer where each pixel is its palette index
(for indexed-PNG / GIF export). Both fix `Subpixel = u8`. `color_map` is `?Sized` so
`&dyn ColorMap` works.

For *generating* a good palette to feed these, see [[pixhaus-color-quant]] (NeuQuant)
and [[pixhaus-palette]] — `image` consumes a `ColorMap`, it doesn't choose a small
palette for you. You can implement `ColorMap` over a Pixhaus palette and pass it to
`dither`/`index_colors`.

## 8. Sampling (`sample`)

All return `Option<P>` (`None` out of bounds), generic over `P: Pixel`.

```rust
pub fn sample_nearest<P: Pixel>(img: &impl GenericImageView<Pixel = P>, u: f32, v: f32) -> Option<P>
pub fn sample_bilinear<P: Pixel>(img: &impl GenericImageView<Pixel = P>, u: f32, v: f32) -> Option<P>
    // u, v are NORMALIZED [0, 1]
pub fn interpolate_nearest<P: Pixel>(img: &impl GenericImageView<Pixel = P>, x: f32, y: f32) -> Option<P>
pub fn interpolate_bilinear<P: Pixel>(img: &impl GenericImageView<Pixel = P>, x: f32, y: f32) -> Option<P>
    // x in [0, w-1], y in [0, h-1] (pixel coordinates)
```

`sample_*` take normalized UV; `interpolate_*` take pixel coordinates. For pixel-art
sampling use the `_nearest` forms; bilinear blends neighbors.
