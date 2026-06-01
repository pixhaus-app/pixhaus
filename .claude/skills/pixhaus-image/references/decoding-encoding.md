# Decoding, encoding, formats — image 0.25.10

Signatures verified against docs.rs 0.25.10. Pixhaus relevance noted inline.

## Table of contents

- Feature flags
- Top-level free functions (open / load / save)
- `ImageReader` — the configurable reader, and the untrusted-input guard
- `ImageFormat`
- `ImageDecoder` / `ImageEncoder` traits
- Per-codec encoders and options (PNG, JPEG, GIF)
- Animation (`Frame`, `Frames`, `Delay`)

## Feature flags

22 features; 17 on by default (`default-formats` meta-feature + `rayon`). Pixhaus
should set `default-features = false` and opt in (see SKILL.md).

| Feature | Default | Notes |
|---|---|---|
| `png` `jpeg` `gif` `bmp` `ico` `tiff` `webp` `tga` `dds` `hdr` `exr` `pnm` `ff` `avif` `qoi` | ✓ | every format; `default-formats` enables all 15 |
| `rayon` | ✓ | parallel encode/decode where the codec supports it |
| `avif-native` | ✗ | AVIF **decode** via libdav1d (C lib) — off unless asked |
| `nasm` | ✗ | NASM-accelerated AVIF encode |
| `serde` | ✗ | derives on some public types |
| `color_quant` | ✗ | auto-on with `gif` |

`avif` (default) is encode-only; WebP encode is lossless-only. `default-features =
false` removes all formats and rayon — re-enable explicitly.

## Top-level free functions

```rust
pub fn open<P: AsRef<Path>>(path: P) -> ImageResult<DynamicImage>
pub fn load<R: BufRead + Seek>(r: R, format: ImageFormat) -> ImageResult<DynamicImage>
pub fn load_from_memory(buffer: &[u8]) -> ImageResult<DynamicImage>
pub fn load_from_memory_with_format(buf: &[u8], format: ImageFormat) -> ImageResult<DynamicImage>
pub fn guess_format(buffer: &[u8]) -> ImageResult<ImageFormat>
pub fn image_dimensions<P: AsRef<Path>>(path: P) -> ImageResult<(u32, u32)>  // dims without full decode

pub fn save_buffer(
    path: impl AsRef<Path>, buf: &[u8], width: u32, height: u32,
    color: impl Into<ExtendedColorType>,
) -> ImageResult<()>
pub fn save_buffer_with_format(
    path: impl AsRef<Path>, buf: &[u8], width: u32, height: u32,
    color: impl Into<ExtendedColorType>, format: ImageFormat,
) -> ImageResult<()>
pub fn write_buffer_with_format<W: Write + Seek>(
    buffered_writer: &mut W, buf: &[u8], width: u32, height: u32,
    color: impl Into<ExtendedColorType>, format: ImageFormat,
) -> ImageResult<()>
```

`open` infers format from contents/extension. `save_buffer*` take the raw bytes +
`ExtendedColorType` directly — handy when you already have a flat buffer and don't
want to wrap it in an `ImageBuffer` first. `image_dimensions` reads just the header,
useful for a cheap "is this file too big" pre-check before committing to a decode.

Pixhaus import: `image::open(path)?.into_rgba8().into_raw()`. In-memory:
`image::load_from_memory(&bytes)?`. **But for untrusted files, use `ImageReader`
with `Limits` instead — the bare functions apply default limits but the reader lets
you tighten them and sniff the format.**

## `ImageReader<R>` — the configurable reader

```rust
pub struct ImageReader<R: Read + Seek> { /* ... */ }

pub fn new(buffered_reader: R) -> Self                       // format unknown until set/guessed
pub fn with_format(buffered_reader: R, format: ImageFormat) -> Self
pub fn open<P: AsRef<Path>>(path: P) -> ImageResult<Self>    // path-backed

pub fn format(&self) -> Option<ImageFormat>
pub fn set_format(&mut self, format: ImageFormat)
pub fn clear_format(&mut self)
pub fn limits(&mut self, limits: Limits)                     // set decode limits
pub fn no_limits(&mut self)                                  // disable all limits (opt-out)
pub fn into_inner(self) -> R

// on impl<'a, R: 'a + BufRead + Seek>:
pub fn with_guessed_format(self) -> ImageResult<Self>        // sniff format from content
pub fn into_dimensions(self) -> ImageResult<(u32, u32)>
pub fn decode(self) -> ImageResult<DynamicImage>
pub fn into_decoder(self) -> ImageResult<impl ImageDecoder + 'a>  // for streaming / metadata
```

Note `0.24`'s `image::io::Reader` is now `image::ImageReader` (top-level). There is
no `with_limits` constructor — set limits after construction with `limits()`.

### The untrusted-input pattern (do this for user/plugin files)

```rust
use image::{ImageReader, Limits};
use std::io::{BufReader, Cursor};

// Tighten limits to a sane project ceiling. default() caps alloc ~512 MiB; this is
// NOT unlimited, but for a hostile file you may want stricter dimension caps too.
let mut limits = Limits::default();
limits.max_image_width = Some(16_384);
limits.max_image_height = Some(16_384);

let reader = ImageReader::new(BufReader::new(Cursor::new(bytes)))
    .with_guessed_format()?;            // sniff magic bytes, don't trust extension
let mut reader = reader;
reader.limits(limits);
let dynamic = reader.decode()?;          // errors with ImageError::Limits if the file exceeds caps
```

`with_guessed_format` reads magic bytes rather than trusting a (possibly lying)
extension — the right call for anything you didn't write. Run the whole thing on
`spawn_blocking` (SKILL.md threading note).

## `ImageFormat`

Variants: `Png Jpeg Gif WebP Pnm Tiff Tga Dds Bmp Ico Hdr OpenExr Farbfeld Avif Qoi`.

```rust
pub fn from_extension<S: AsRef<OsStr>>(ext: S) -> Option<Self>
pub fn from_path<P: AsRef<Path>>(path: P) -> ImageResult<Self>
pub fn from_mime_type<M: AsRef<str>>(mime_type: M) -> Option<Self>
pub fn to_mime_type(&self) -> &'static str
pub fn extensions_str(self) -> &'static [&'static str]
pub fn can_read(&self) -> bool          // format is readable in principle
pub fn can_write(&self) -> bool
pub fn reading_enabled(&self) -> bool    // AND the feature is compiled in
pub fn writing_enabled(&self) -> bool
pub fn all() -> impl Iterator<Item = ImageFormat>
```

`can_read` answers "does the crate support this format at all"; `reading_enabled`
answers "is the feature compiled into *this* build". With `default-features = false`,
check `reading_enabled` before offering a format in an open dialog. (No `all_enabled`
method — filter `all()` by `reading_enabled`.)

## `ImageDecoder` / `ImageEncoder` traits

```rust
pub trait ImageDecoder {
    fn dimensions(&self) -> (u32, u32);
    fn color_type(&self) -> ColorType;                       // concrete in-memory layout
    fn read_image(self, buf: &mut [u8]) -> ImageResult<()> where Self: Sized;
    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()>;
    // provided:
    fn original_color_type(&self) -> ExtendedColorType { /* wire format */ }
    fn icc_profile(&mut self) -> ImageResult<Option<Vec<u8>>>;
    fn exif_metadata(&mut self) -> ImageResult<Option<Vec<u8>>>;
    fn xmp_metadata(&mut self) -> ImageResult<Option<Vec<u8>>>;
    fn iptc_metadata(&mut self) -> ImageResult<Option<Vec<u8>>>;
    fn orientation(&mut self) -> ImageResult<Orientation>;   // EXIF orientation
    fn total_bytes(&self) -> u64;                            // size the read_image buffer to this
    fn set_limits(&mut self, limits: Limits) -> ImageResult<()>;
}

pub trait ImageEncoder {
    fn write_image(self, buf: &[u8], width: u32, height: u32,
                   color_type: ExtendedColorType) -> ImageResult<()>;
}
```

Most code goes through `DynamicImage`/`ImageBuffer` and never touches these
directly. Drop to the decoder when you need metadata (`orientation`, `icc_profile`)
or want to stream into your own buffer (`total_bytes` then `read_image`). `DynamicImage::from_decoder(decoder)` builds an image straight from any decoder.

## Per-codec encoders and options

Each codec submodule is gated behind its feature. The encoders implement
`ImageEncoder` (so `write_image` consumes self); some also have inherent
`encode`/`encode_image` taking `&mut self` for reuse.

### PNG — `codecs::png` (the main Pixhaus export format)

```rust
// PngEncoder<W: Write>
pub fn new(w: W) -> PngEncoder<W>
pub fn new_with_quality(w: W, compression: CompressionType, filter: FilterType) -> PngEncoder<W>
// write_image via ImageEncoder

// CompressionType (#[non_exhaustive], Default = Fast)
Default | Fast | Best | Uncompressed | Level(u8)   // Level 1..=9

// FilterType (#[non_exhaustive], default Adaptive) — PNG row filter, NOT the resize filter!
NoFilter | Sub | Up | Avg | Paeth | Adaptive

// PngDecoder<R: BufRead + Seek>
pub fn new(r: R) -> ImageResult<PngDecoder<R>>
pub fn with_limits(r: R, limits: Limits) -> ImageResult<PngDecoder<R>>
pub fn is_apng(&self) -> ImageResult<bool>
pub fn apng(self) -> ImageResult<ApngDecoder<R>>     // animated PNG -> AnimationDecoder
```

Watch the name collision: `png::FilterType` is the PNG byte-row filter
(compression), totally unrelated to `imageops::FilterType` (resize sampling). For
pixel-art PNG export, `CompressionType::Best` + `FilterType::Adaptive` gives the
smallest file; `Fast` (default) is fine for autosaves. `image` wraps the lower-level
`png` crate — when you need direct control over PNG chunks, interlacing, or streaming
rows without `image`'s buffer types, drop to [[pixhaus-png]].

```rust
use image::codecs::png::{PngEncoder, CompressionType, FilterType};
let encoder = PngEncoder::new_with_quality(writer, CompressionType::Best, FilterType::Adaptive);
rgba_image.write_with_encoder(encoder)?;
```

### JPEG — `codecs::jpeg` (reference-image import/export, never for pixel art)

```rust
// JpegEncoder<W: Write>
pub fn new(w: W) -> JpegEncoder<W>
pub fn new_with_quality(w: W, quality: u8) -> JpegEncoder<W>   // 1..=100
pub fn encode(&mut self, image: &[u8], width: u32, height: u32, color_type: ExtendedColorType) -> ImageResult<()>
pub fn encode_image<I: GenericImageView>(&mut self, image: &I) -> ImageResult<()>
    where I::Pixel: PixelWithColorType
// JpegDecoder<R: BufRead + Seek>::new
```

JPEG is lossy with no alpha — wrong for sprites, fine for importing a photo
reference. `encode`/`encode_image` take `&mut self` (reusable); `write_image`
consumes self.

### GIF — `codecs::gif` (animation export, 256-color)

```rust
// GifEncoder<W: Write>
pub fn new(w: W) -> GifEncoder<W>
pub fn new_with_speed(w: W, speed: i32) -> GifEncoder<W>       // 1 (best/slowest) ..= 30 (worst/fastest)
pub fn set_repeat(&mut self, repeat: Repeat) -> ImageResult<()>
pub fn encode(&mut self, data: &[u8], width: u32, height: u32, color: ExtendedColorType) -> ImageResult<()>
pub fn encode_frame(&mut self, img_frame: Frame) -> ImageResult<()>
pub fn encode_frames<F: IntoIterator<Item = Frame>>(&mut self, frames: F) -> ImageResult<()>
// Repeat: Finite(u16) | Infinite
// GifDecoder::new<R: Read>(r) -> ImageResult<GifDecoder<R>>  (ImageDecoder/AnimationDecoder need BufRead+Seek)
```

GIF is 256 colors per frame, so the encoder quantizes — pair with
[[pixhaus-color-quant]] / [[pixhaus-palette]] when you want control over the
palette. `set_repeat(Repeat::Infinite)` for a looping animation.

## Animation — `Frame`, `Frames`, `Delay`, `AnimationDecoder`

```rust
pub trait AnimationDecoder<'a> { fn into_frames(self) -> Frames<'a>; }

// Frame — one animation frame: an RgbaImage plus placement and timing.
pub fn new(buffer: RgbaImage) -> Frame
pub fn from_parts(buffer: RgbaImage, left: u32, top: u32, delay: Delay) -> Frame
pub fn delay(&self) -> Delay
pub fn left(&self) -> u32
pub fn top(&self) -> u32
pub fn buffer(&self) -> &RgbaImage
pub fn buffer_mut(&mut self) -> &mut RgbaImage
pub fn into_buffer(self) -> RgbaImage

// Delay — frame duration as an exact rational of milliseconds.
pub fn from_numer_denom_ms(numerator: u32, denominator: u32) -> Self
pub fn from_saturating_duration(duration: Duration) -> Self
pub fn numer_denom_ms(self) -> (u32, u32)
```

`Frames<'a>` is an iterator of `ImageResult<Frame>` tied to the decoder's lifetime.
Decode every frame of a GIF:

```rust
use image::{codecs::gif::GifDecoder, AnimationDecoder};
let frames = GifDecoder::new(reader)?.into_frames().collect_frames()?;  // Vec<Frame>
for frame in &frames {
    let (num, den) = frame.delay().numer_denom_ms();    // per-frame timing in ms
    let rgba: &RgbaImage = frame.buffer();
    // ... hand rgba.as_raw() to a Pixhaus animation timeline
}
```

`Frame` frames are always `RgbaImage` — they map straight onto Pixhaus's RGBA8
buffers via `.into_buffer().into_raw()`. (`Frames::collect_frames` is a convenience
that drains the iterator into a `Vec<Frame>`, surfacing the first decode error.)
