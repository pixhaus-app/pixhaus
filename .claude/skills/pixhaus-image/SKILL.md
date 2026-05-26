---
name: pixhaus-image
description: >
  Use when loading, saving, or transforming raster image files in Pixhaus with the
  `image` crate — above all the `io` crate's PNG/sprite-sheet/reference-image import
  and export, plus any offline pixel-buffer op (resize, crop, flip, rotate, overlay,
  color adjust, GIF/APNG animation frames). Trigger this for ANY "open/import a
  PNG/JPEG/GIF", "save/export the layer as PNG", "load a reference image", "decode
  these image bytes", "encode to a sprite sheet", "resize/scale/downscale an image",
  "crop/flip/rotate a buffer", "build a GIF", "turn a DynamicImage into RGBA8 bytes /
  a Vec<u8> into an image", "guess the image format", or "guard against a giant/hostile
  image file" task, even when the user doesn't say "image". `image` is the format
  decode/encode + offline ops layer; it is NOT the live wgpu canvas renderer. Two
  traps make it worth stopping for: scaling pixel art with anything other than
  `FilterType::Nearest` silently blurs every sprite, and `image::open` on untrusted
  files needs a `Limits` guard because the default is generous, not unlimited. Reach
  for this skill rather than guessing signatures — the API moved a lot across 0.24→0.25.
---

# image for Pixhaus

`image` is the file-format layer of the `io` crate: it decodes PNG/JPEG/GIF/etc.
into an in-memory buffer, encodes buffers back out, and offers a set of offline
raster operations (resize, crop, flip, rotate, overlay, color adjust). Pixhaus
uses it for import (open a PNG, load a reference photo, read a sprite sheet),
export (save a layer or the flattened canvas, write a GIF/APNG), and any one-shot
pixel transform that isn't the live viewport.

Draw the line clearly: **`image` is import/export + offline ops, not the live
canvas.** The interactive viewport — pan, zoom, brush strokes at 8K — runs on
`wgpu` (`pixhaus-wgpu`) against GPU textures. `image` touches CPU `Vec<u8>`
buffers at the edges of that world: when bytes enter from a file and when they
leave to one. Don't reach for `image::resize` to draw the zoomed canvas; that's a
shader's job.

This skill is the floor: the version pin, the one interop path that everything
else hangs off (raw `Vec<u8>` ↔ `RgbaImage`), the handful of facts that prevent
the recurring bugs, and how the pieces map onto a pixel-art editor. When you need
the full method surface for an area, open the matching file in `references/` —
don't guess signatures from memory; `image`'s API shifted hard across 0.24→0.25
and the references are derived from docs.rs 0.25.10.

## Version, features, and license

```toml
# Trim to the formats Pixhaus actually reads/writes rather than taking all 15.
image = { version = "0.25", default-features = false, features = [
    "png", "gif", "bmp", "tga", "qoi",   # core pixel-art import/export
    "jpeg", "webp",                       # reference-image import
    "rayon",                              # parallel codecs (keep on)
] }
```

`image` is `MIT OR Apache-2.0` — clears the [[project-v2-native-restart]] MIT lock.
Its default codec deps (png, gif via `color_quant`, jpeg, etc.) are permissive and
pass `cargo deny`. The one to watch is the non-default `avif-native` feature, which
links libdav1d (a C library) for AVIF *decoding* — leave it off unless a user asks
for AVIF, both to keep the dependency surface small and to avoid a C build. Default
AVIF support is encode-only.

Feature notes that bite:

- **`default-features = false` turns off every format and `rayon`.** You get a
  crate that decodes nothing until you re-enable features. Pick the formats Pixhaus
  supports explicitly (above) — it cuts compile time and the codec attack surface.
- **`rayon` (on by default) parallelizes encode/decode** where the codec supports
  it. Keep it on. It's compatible with our model: run codecs inside
  `tokio::task::spawn_blocking` (see threading below), and rayon parallelizes within
  that blocking task. See [[pixhaus-rayon]] for the threadpool interaction.
- WebP encoding is **lossless-only**. AVIF decode needs `avif-native`. GIF pulls in
  `color_quant` (the same quantizer behind [[pixhaus-color-quant]]).

When you bump `image`, re-verify the references against docs.rs — the 0.24→0.25
jump renamed `Reader`→`ImageReader`, moved `io::Reader`, and changed several
signatures. See [[feedback-dep-upgrades]].

## The one interop path: raw `Vec<u8>` ↔ `RgbaImage`

Pixhaus pixel buffers are flat `Vec<u8>` of RGBA8 with explicit stride (the
workspace memory rule). `image`'s matching type is `RgbaImage`, which is exactly
`ImageBuffer<Rgba<u8>, Vec<u8>>` — its backing container *is* a `Vec<u8>`. So the
bridge is move-only, no per-pixel copy:

```rust
use image::{RgbaImage, DynamicImage};

// Vec<u8> (w*h*4 RGBA8) -> RgbaImage. None if the buffer is too small. Moves, no copy.
let img: RgbaImage = RgbaImage::from_raw(width, height, bytes)
    .ok_or(/* your io-crate error: buffer length doesn't match w*h*4 */)?;

// RgbaImage -> Vec<u8>. Moves the container straight back out.
let bytes: Vec<u8> = img.into_raw();
```

Decoded files arrive as `DynamicImage` (an enum over every pixel layout). Pixhaus
works in RGBA8, so **canonicalize on the way in** and pull the `Vec<u8>` out:

```rust
let dynamic = image::open(path)?;            // -> DynamicImage, whatever the file's layout
let rgba: RgbaImage = dynamic.into_rgba8();  // reuses the buffer if already RGBA8, else converts
let (w, h) = rgba.dimensions();
let pixels: Vec<u8> = rgba.into_raw();        // the flat RGBA8 buffer Pixhaus owns
```

`into_rgba8()` is the consuming converter — free when the source is already
`ImageRgba8`, a conversion otherwise. Use `to_rgba8()` (borrowing) when you still
need the `DynamicImage` afterward. The reverse, building a file from a Pixhaus
buffer, is `RgbaImage::from_raw(w, h, bytes)` then `.save(path)` or
`.write_to(writer, ImageFormat::Png)`. Full method surface in
`references/buffers-and-images.md`.

## The facts that cause most bugs

1. **Scale pixel art with `FilterType::Nearest`, nothing else.** Every other
   filter (`Triangle`, `CatmullRom`, `Gaussian`, `Lanczos3`) interpolates between
   neighboring pixels — which softens and blurs a sprite, destroying the hard edges
   that *are* the art. `Nearest` is also the fastest. Use
   `image.resize(w, h, FilterType::Nearest)` for integer upscale/downscale of pixel
   art, and reserve the smoothing filters for downsizing photographic reference
   images. Also avoid `thumbnail()` for sprites — its downscale path averages source
   pixels. This is the single most common pixel-art mistake with this crate.

2. **`image::open` on untrusted files needs a `Limits` guard.** Users open
   `.png`/`.gif` files from other people and from plugins. A crafted file can
   declare enormous dimensions and blow up memory (a decompression bomb).
   `Limits::default()` caps allocation at ~512 MiB — generous but not unlimited;
   `Limits::no_limits()` is the opt-out, not the default. Decode untrusted input
   through `ImageReader` with explicit limits rather than the bare `open`/`load`
   free functions, and call `check_dimensions` before allocating. Pattern in
   `references/decoding-encoding.md`.

3. **`get_pixel` means two different things, and `unsafe_get_pixel` is banned.**
   The inherent `ImageBuffer::get_pixel` returns `&P` (a reference); the
   `GenericImageView::get_pixel` *trait* method returns `Self::Pixel` *by value*.
   They read the same data — just don't be surprised by the type. The trait's
   `unsafe_get_pixel`/`unsafe_put_pixel` are off-limits: the workspace forbids
   `unsafe` (`pixhaus-rust-conventions`). Use the checked/safe accessors; per-pixel
   bounds checks are not the bottleneck, full-buffer passes are.

4. **Most ops allocate a new buffer; a few mutate in place — know which.**
   `resize`, `crop_imm`, `blur`, `rotate90`, `grayscale`, the `imageops` free
   functions: new buffer returned. But `invert`, `crop` (the `&mut self` one),
   `imageops::overlay`/`replace`/`tile`, `imageops::dither`, and the `*_in_place`
   functions mutate their target. Reaching for the wrong one either wastes an
   allocation or silently fails to change anything. The split is tabulated in
   `references/imageops.md`.

## Pixhaus applications

Where the pieces land in a pixel-art editor:

- **Import (the `io` crate).** `image::open(path)` for a file path,
  `image::load_from_memory(bytes)` for an in-memory blob (a paste, a download),
  `ImageReader::new(reader).with_guessed_format()?.decode()?` when you have a
  generic reader and want sniffed-not-extension format detection. Canonicalize to
  `into_rgba8().into_raw()`. Guard untrusted sources with `Limits`. See
  `references/decoding-encoding.md`.
- **Export.** `RgbaImage::from_raw(w, h, &pixels)` then `.save("out.png")` (format
  from extension) or `.write_to(&mut writer, ImageFormat::Png)` to a buffer/stream.
  For tuned PNG output, build a `PngEncoder::new_with_quality(w, CompressionType,
  FilterType)` and `write_with_encoder`. JPEG quality is
  `JpegEncoder::new_with_quality(w, 1..=100)`.
- **Sprite sheets.** A sheet is one big `RgbaImage`; cut frames with
  `imageops::crop_imm(&sheet, x, y, fw, fh).to_image()` (a view → owned buffer),
  and assemble a sheet by `imageops::replace(&mut sheet, &frame, x, y)` (in place,
  same pixel type required).
- **Animation export (GIF/APNG).** Build `Frame::from_parts(rgba, left, top,
  Delay::from_numer_denom_ms(n, d))` and feed `GifEncoder::encode_frames`. GIF is
  256-color, so it pairs with palette quantization ([[pixhaus-color-quant]],
  [[pixhaus-palette]]). Decode animation with `GifDecoder::new(r)?.into_frames()`.
- **Indexed / palette export.** `imageops::index_colors(&buf, &color_map)` maps an
  image to a `Luma<u8>` index buffer against a `ColorMap`; `imageops::dither` does
  the same in place with Floyd–Steinberg. For palette *generation* see
  [[pixhaus-color-quant]] and [[pixhaus-palette]] — `image` consumes a palette, it
  doesn't pick a good small one for you.
- **Reference-image ops.** Resizing a photographic reference down to fit a panel is
  the one place the smoothing filters belong (`FilterType::Lanczos3` or
  `CatmullRom`). EXIF-rotated phone photos: read `decoder.orientation()?` and
  `dynamic.apply_orientation(orientation)` so the image isn't sideways.
- **To egui / wgpu.** `image` gives straight (un-premultiplied) RGBA8, which is
  what `egui::ColorImage::from_rgba_unmultiplied([w, h], &bytes)` wants for a
  preview texture, and what you upload to a `wgpu` texture. Keep premultiply/
  straight-alpha conventions straight at that boundary (see [[pixhaus-egui]],
  [[pixhaus-wgpu]]). Don't route the live canvas through `image`.

## Threading: decode/encode off the egui thread

Decoding and encoding are CPU- and IO-bound, and at the [[8k-perf-constraint]] an
8192×8192 RGBA image is 268 MB — decoding or encoding that is not a per-frame
operation. Per the workspace async rules (`pixhaus-rust-conventions`), run
`image::open`, `save`, `write_to`, and any full-size `imageops` pass on
`tokio::task::spawn_blocking`, and never block the egui update loop on it — deliver
the decoded buffer (or the save result) back over a channel the loop drains each
frame. The `rayon` feature parallelizes *within* a codec; that's a separate axis
from `spawn_blocking` and lives happily inside the blocking task. Do not call
`image::open` inline in a `ui`/`logic` path on a large file; that freezes the frame.

## Errors

`image` surfaces failures as `image::ImageError` (its `Result` alias is
`ImageResult<T>`), an enum over `Decoding`/`Encoding`/`Parameter`/`Limits`/
`Unsupported`/`IoError`. It implements `From<std::io::Error>`, so `?` on file I/O
folds in cleanly. In the `io` library crate, map `ImageError` into the crate's
`thiserror` enum with `#[from]` rather than leaking it through public APIs;
`anyhow` stays in the binary only ([[pixhaus-thiserror]]). A corrupt or
unsupported file is a user-facing error to report — never `unwrap()` a decode
result outside tests. `from_raw` returns `Option` (not `Result`); a `None` there
means your buffer length didn't match `w*h*channels`, which is a programmer error
worth its own clear message.

## References

Open the file for the area you're working in; each is a dense API reference for
`image` 0.25.10, with load-bearing signatures checked against docs.rs.

| File | Covers |
|---|---|
| `references/decoding-encoding.md` | `open`/`load`/`load_from_memory`, `ImageReader`, format guessing, `ImageFormat`, `ImageDecoder`/`ImageEncoder`, per-codec encoders + options (PNG compression/filter, JPEG quality, GIF speed/repeat), animation (`Frame`/`Frames`/`Delay`), the untrusted-input `Limits` guard, feature flags |
| `references/buffers-and-images.md` | `ImageBuffer` (construction, pixel access, iteration, raw access, save/write), the type aliases, `DynamicImage` (variants, `to_*`/`into_*`/`as_*` conversions, `as_bytes`, transform convenience methods), `GenericImageView`/`GenericImage` traits, `SubImage`, `FlatSamples` zero-copy interop |
| `references/imageops.md` | resize/thumbnail + `FilterType` (the pixel-art filter rule), blur/sharpen/convolution, crop views, overlay/replace/tile/gradients, flips/rotations (allocating, in-place, write-into), color adjust (brighten/contrast/huerotate/invert/grayscale), dither/`index_colors`/`ColorMap`, the `sample` submodule, and the allocates-vs-mutates table |
| `references/color-and-pixels.md` | `ColorType` vs `ExtendedColorType`, the `Rgb`/`Rgba`/`Luma`/`LumaA` newtypes and `[T; N]` layout, type aliases, the `Pixel`/`Primitive`/`PixelWithColorType` traits, the `error` module, `Limits`/`LimitSupport`, `metadata::Orientation` |

A standing caution: a few deep signatures were flagged during research as not
fully verifiable from the rendered docs (noted inline as "(verify)"). When one is
load-bearing for what you're building, confirm it against
https://docs.rs/image/0.25.10/image/ or the source before depending on it.

## Decision shortcut

```
Touching image files or offline pixel buffers in Pixhaus?
├─ Importing a file?
│    ├─ Trusted/own file, have a path?  -> image::open(path)?.into_rgba8().into_raw()
│    ├─ In-memory bytes (paste/download)? -> image::load_from_memory(&bytes)?
│    └─ Untrusted (user/plugin) file?  -> ImageReader + Limits (default 512MiB, not unlimited)
│                                          .with_guessed_format()?.decode()? then check_dimensions
├─ Exporting?  -> RgbaImage::from_raw(w,h,bytes)?.save(path)  (or PngEncoder for tuned output)
│                 GIF/APNG: GifEncoder::encode_frames(Frame::from_parts(..))
├─ Scaling PIXEL ART?     -> .resize(w, h, FilterType::Nearest)   (NEVER Triangle/Lanczos — they blur)
├─ Scaling a PHOTO/ref?   -> .resize(w, h, FilterType::Lanczos3)  (smoothing is correct here)
├─ Crop a frame from a sheet? -> imageops::crop_imm(&img, x,y,w,h).to_image()  (view -> owned)
├─ Composite frames?      -> imageops::overlay/replace(&mut dst, &src, x, y)   (in place, same pixel type)
└─ Whatever the case: run decode/encode/large ops on spawn_blocking, never the egui thread,
   and map ImageError into the io crate's thiserror enum (#[from]).
```
