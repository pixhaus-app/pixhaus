---
name: pixhaus-png
description: >
  Use when reading or writing PNG files in Pixhaus with the `png` crate — the
  primary import/export path for sprites and references, sprite-sheet PNGs,
  indexed-PNG export, and animated PNG (APNG) for animations. Trigger this for
  ANY "load/import a PNG", "export/save as PNG", "decode PNG to pixels", "encode
  this RGBA buffer to PNG", "write a sprite sheet", "indexed/paletted PNG",
  "PNG with transparency / tRNS", "animated PNG / APNG", "set the bit depth /
  color type / compression / filter", or "why is my PNG truncated / wrong
  colors" task, even when the user doesn't say "png". png is the standalone PNG
  codec (no `image` wrapper). Reach for this skill above all because png 0.18
  RENAMED the API your training data remembers: `FilterType` +
  `AdaptiveFilterType` merged into one `Filter` enum, `output_buffer_size`
  now returns `Option<usize>`, and `Compression::Default/Best` became
  `Balanced/High` — writing the old names will not compile. Two more traps bite
  hard: `Writer::finish()` is mandatory or the file is truncated, and
  `Transformations::EXPAND` does NOT guarantee an RGBA8 buffer.
---

# png for Pixhaus

`png` is a pure-Rust, standalone PNG encoder/decoder — no C libpng. It is the
low-level codec the `image` crate itself sits on top of. In Pixhaus's `io` crate
(per the repo layout) its job is the PNG-specific control surface: fine bit-depth
and color-type choice, indexed/paletted output with explicit `PLTE` + `tRNS`,
APNG frame control, gamma/sRGB/text chunks, row streaming, and the
compression/filter/limits knobs.

The two halves of the crate are independent: `Decoder` -> `Reader` pulls pixels
out of a PNG, and `Encoder` -> `Writer` pushes a pixel buffer into one. Pixhaus's
in-memory pixel buffer is RGBA8 `Vec<u8>` with explicit stride (the repo memory
rule), so the job on both sides is bridging that buffer to PNG's wider world of
color types, bit depths, and palettes. See `pixhaus-bytemuck` for the same buffer
as typed pixels, `pixhaus-color-quant` for building the palette an indexed PNG
needs, and `pixhaus-rust-conventions` for the async/error rules this skill leans on.

## png vs the image crate — pick the right layer

Pixhaus also depends on `image` (see `pixhaus-image`), which wraps `png` and adds
format detection, JPEG/GIF/etc., and offline ops (resize, crop, rotate). They
overlap on PNG, so be deliberate:

- **Reach for `image` first** for the everyday path: "open this file / save the
  layer as a PNG", importing a reference of unknown format, or anything that pairs
  PNG IO with a resize/crop/flip. It's the higher-level, more ergonomic layer and
  handles the RGBA8 conversion for you (`to_rgba8()`).
- **Drop to `png` (this skill)** when you need PNG-specific control `image`
  doesn't surface cleanly: writing **indexed/paletted** PNG with your own palette,
  authoring **APNG** with per-frame dispose/blend ops, setting gamma/sRGB/text
  chunks, **streaming** rows for an 8K export, or tuning compression/filter/limits.
  `image` delegates PNG to `png` anyway, so this is dropping a layer, not bringing
  in a parallel codec.

If a task is plain "load/save a PNG" with no PNG-specific requirement, prefer
`pixhaus-image` and don't reach here.

## Version and license

| Crate | Version | License | cargo deny |
|---|---|---|---|
| `png` | 0.18 | `MIT OR Apache-2.0` | clears the MIT lock |

MIT-or-Apache clears the workspace MIT lock — no GPL concern. Default features
are correct; there is no decode/encode feature to opt into (the `benchmarks` and
`unstable` features are for the crate's own benches, not consumers). When you bump
it, re-verify the signatures below against docs.rs (see [[feedback-dep-upgrades]]).

```toml
png = "0.18"
```

## 0.18 renamed the API — your memory is wrong

This is why the skill exists. `png` 0.18 changed names that older examples and
training data still use. Writing the old names is a compile error, not a warning,
so get these right the first time:

| You might reach for (0.17 and earlier) | 0.18 reality |
|---|---|
| `FilterType` + `AdaptiveFilterType`, `set_adaptive_filter(...)` | one `Filter` enum; `set_filter(Filter::Adaptive)` |
| `Compression::Default` / `Best` / `Huffman` / `Rle` | `NoCompression`, `Fastest`, `Fast`, `Balanced` (default), `High` |
| `output_buffer_size() -> usize` | `output_buffer_size() -> Option<usize>` |
| `output_line_size(w) -> usize` | `output_line_size(w) -> Option<usize>` |
| `set_srgb(...)` | `set_source_srgb(...)` |

`Filter` is `#[non_exhaustive]`, so always match with a `_ =>` arm or just
construct the variant you want.

## Decode: PNG file -> RGBA8 buffer

`Decoder<R>` needs `R: BufRead + Seek`, so wrap a `File` in `BufReader` (a
`Cursor<&[u8]>` works for in-memory bytes). Set transformations and limits on the
`Decoder` *before* `read_info`; `read_info` consumes it and hands back a `Reader`.

```rust
use std::io::BufReader;

let file = std::fs::File::open(path)?;            // map IO error into your io-crate enum
let mut decoder = png::Decoder::new(BufReader::new(file));

// Normalize palette/low-bit/16-bit down toward 8-bit before we ever see pixels.
decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);

let mut reader = decoder.read_info()?;            // -> Result<Reader<R>, DecodingError>

// output_buffer_size() is Option: None means the image is too large to address.
let size = reader
    .output_buffer_size()
    .ok_or(/* your "image too large" error */)?;
let mut buf = vec![0u8; size];

let info = reader.next_frame(&mut buf)?;          // -> OutputInfo
let pixels = &buf[..info.buffer_size()];          // valid bytes; see footgun below
```

`OutputInfo` has public fields `width: u32`, `height: u32`, `color_type:
ColorType`, `bit_depth: BitDepth`, `line_size: usize`, and the method
`buffer_size() -> usize` (= `line_size * height`).

### Footgun 1: `EXPAND` does not guarantee RGBA

`Transformations::EXPAND` does three specific things — expand palette to RGB,
expand sub-8-bit grayscale to 8-bit, and turn a `tRNS` chunk into an alpha
channel. It does **not** add an alpha channel to an opaque RGB or grayscale
image, and it does not turn grayscale into RGB. So after `EXPAND | STRIP_16` the
output is one of four 8-bit shapes — `Grayscale`, `GrayscaleAlpha`, `Rgb`,
`Rgba` — and Pixhaus wants RGBA8. The crate has no single "give me RGBA8"
transform; you finish the conversion by matching `output_color_type()`:

```rust
let (color, _depth) = reader.output_color_type();
let rgba: Vec<u8> = match color {
    png::ColorType::Rgba => pixels.to_vec(),
    png::ColorType::Rgb => pixels.chunks_exact(3)
        .flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
    png::ColorType::GrayscaleAlpha => pixels.chunks_exact(2)
        .flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
    png::ColorType::Grayscale => pixels.iter()
        .flat_map(|&g| [g, g, g, 255]).collect(),
    png::ColorType::Indexed => unreachable!("EXPAND removes Indexed"),
};
```

### Footgun 2: use `buffer_size()`, not `buf.len()`

`next_frame` requires `buf.len() >= output_buffer_size()`, but the number of
*valid* bytes it writes is `OutputInfo::buffer_size()`, which can be smaller than
the buffer you allocated. Always slice `&buf[..info.buffer_size()]`; reading the
whole `buf` would feed trailing zeros into your pixel data.

Keep the palette/transparency around when you need it: `reader.info()` returns
`&Info`, whose `palette: Option<Cow<[u8]>>` and `trns: Option<Cow<[u8]>>` fields
carry the original palette and per-index alpha if you decoded without `EXPAND`.

## Encode: RGBA8 buffer -> PNG file

```rust
let file = std::fs::File::create(path)?;
let writer = std::io::BufWriter::new(file);

let mut encoder = png::Encoder::new(writer, width, height);
encoder.set_color(png::ColorType::Rgba);
encoder.set_depth(png::BitDepth::Eight);
encoder.set_compression(png::Compression::Balanced);  // the default; raise to High for smaller files

let mut writer = encoder.write_header()?;             // consumes encoder -> Writer
writer.write_image_data(&rgba)?;                      // rgba.len() must == width*height*4
writer.finish()?;                                     // MANDATORY — see footgun 3
```

### Footgun 3: `finish()` is mandatory

`Writer` (and `StreamWriter`) implement `Drop`, but dropping does **not** produce
a valid file — `finish()` is what writes the final IDAT and the IEND chunk and
flushes. Skip it and you ship a truncated PNG, and because `Drop` can't return a
`Result`, the error from the missing final flush is silently swallowed. Always
end with `writer.finish()?` and propagate its error.

### Footgun 4: `write_image_data` length is exact

The slice length must match `width * height * bytes_per_pixel` for the color type
and depth you set — for RGBA8 that's `width * height * 4`. A mismatch is an
`EncodingError::Parameter`, not a silent crop. And only certain color/depth pairs
are legal:

| ColorType | Legal bit depths | Bytes/pixel at 8-bit |
|---|---|---|
| `Grayscale` | 1, 2, 4, 8, 16 | 1 |
| `Rgb` | 8, 16 | 3 |
| `Indexed` | 1, 2, 4, 8 (never 16) | 1 (index) |
| `GrayscaleAlpha` | 8, 16 | 2 |
| `Rgba` | 8, 16 | 4 |

For large canvases where you don't want to materialize the whole filtered image,
`writer.stream_writer()?` (or `into_stream_writer()`) gives a `Write` you push
rows into; finish it the same way: `sw.write_all(row)?; ... sw.finish()?`.

## Indexed PNG export

Pixel art is a natural fit for indexed (paletted) PNG — smaller files, and it
preserves the palette. Build the palette + index buffer with
`pixhaus-color-quant` (`color_map_rgb` and `index_of`), then:

```rust
let mut encoder = png::Encoder::new(writer, width, height);
encoder.set_color(png::ColorType::Indexed);
encoder.set_depth(png::BitDepth::Eight);              // or Four for <=16 colors
encoder.set_palette(palette_rgb);                     // flat RGB, 3 bytes per entry (PLTE)
encoder.set_trns(palette_alpha);                      // optional per-index alpha (tRNS)
let mut writer = encoder.write_header()?;
writer.write_image_data(&indices)?;                   // one byte per pixel = palette index
writer.finish()?;
```

`set_palette`/`set_trns` take anything `Into<Cow<[u8]>>` — a `Vec<u8>` or `&[u8]`
both work. The palette is RGB (3 bytes/entry, the PLTE chunk); transparency is a
separate parallel alpha array (the tRNS chunk), and it may be shorter than the
palette (entries past its end are fully opaque).

## Animated PNG (APNG) for animations

Pixhaus animations can export as a single APNG. Declare the animation on the
encoder, then each `write_image_data` call emits one frame.

```rust
let mut encoder = png::Encoder::new(writer, frame_w, frame_h);
encoder.set_color(png::ColorType::Rgba);
encoder.set_depth(png::BitDepth::Eight);
encoder.set_animated(num_frames, num_plays)?;         // num_plays = 0 -> loop forever
encoder.set_frame_delay(1, 10)?;                      // default 1/10 s per frame
let mut writer = encoder.write_header()?;

for frame in &frames {
    // optional per-frame overrides BEFORE writing: writer.set_frame_delay(n, d)?,
    // set_frame_position(x, y)?, set_frame_dimension(w, h)?, set_dispose_op(..)?, set_blend_op(..)?
    writer.write_image_data(&frame.rgba)?;            // one call == one frame
}
writer.finish()?;
```

Read APNG back by looping `next_frame` `animation_control().num_frames` times;
`reader.info().frame_control()` gives the just-read frame's `x_offset`,
`y_offset`, `delay_num/den`, `dispose_op`, and `blend_op` so you can composite.
Note Unity is the engine target and consumes sprite *sheets*, not APNG — APNG is
for previews and interchange, so reach for a packed sheet (a plain PNG laid out in
a grid) when the destination is the engine, and APNG when it's "share this loop."

## Threading: decode/encode off the egui thread

Decoding or encoding a PNG is CPU-bound (DEFLATE + per-row filtering) plus disk
IO. At the [[8k-perf-constraint]] an 8192x8192 RGBA image is 256 MB and the
filter/compress pass is far from a per-frame cost. Per `pixhaus-rust-conventions`,
that work runs on `tokio::task::spawn_blocking` and the egui update loop never
blocks on it — kick off the load/save on a blocking task and deliver the decoded
buffer (or the save result) back over a channel the loop drains each frame. Don't
call `read_info`/`next_frame`/`write_image_data` inline in a `ui`/`logic` path on
a full-size image; that freezes the frame.

## Limits: the decompression-bomb knob

Untrusted PNGs can declare an enormous size to force a huge allocation. `png`
caps intermediate decoder allocations via `Limits { bytes }`, default 64 MiB:

```rust
let decoder = png::Decoder::new_with_limits(reader, png::Limits { bytes: 1 << 30 }); // 1 GiB
// or decoder.set_limits(png::Limits { bytes: ... }) before read_info
```

`Limits` bounds the decoder's *internal* buffers, not the output buffer you
allocate, and an over-large declared image fails with `DecodingError::LimitsExceeded`
*before* allocating. For a trusted 8K editor canvas, raise it deliberately; for
imports of unknown provenance, keep it tight. This is the CVE-class protection
knob — don't disable it by setting it absurdly high on the untrusted path.

## Errors

`png` returns `DecodingError` (`IoError`, `Format`, `Parameter`, `LimitsExceeded`)
and `EncodingError` (`IoError`, `Format`, `Parameter`, `LimitsExceeded`). Both
implement `std::error::Error` and `From<std::io::Error>`. Per
`pixhaus-thiserror`, wrap them into the `io` crate's error enum with `#[from]`
rather than surfacing the raw types in a public signature:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("decoding PNG: {0}")]
    Decode(#[from] png::DecodingError),
    #[error("encoding PNG: {0}")]
    Encode(#[from] png::EncodingError),
}
```

No `unwrap()` anywhere in this flow (clippy-enforced): `output_buffer_size()`
returns `Option` — map `None` to an error, don't unwrap. `next_frame`,
`write_header`, `write_image_data`, and `finish` all return `Result` — propagate
with `?`.

## Decision shortcut

```
Reading or writing a PNG in Pixhaus?
├─ Import a PNG to the RGBA8 canvas?
│    └─ Decoder + set_transformations(EXPAND | STRIP_16) -> read_info -> next_frame
│       then convert output_color_type() to RGBA8 (EXPAND alone won't — footgun 1),
│       slice &buf[..info.buffer_size()] (footgun 2)
├─ Export the canvas as a normal PNG?
│    └─ Encoder::new -> set_color(Rgba)/set_depth(Eight) -> write_header
│       -> write_image_data(&rgba) -> finish()   (finish is mandatory — footgun 3)
├─ Export indexed/paletted PNG (smaller, pixel-art friendly)?
│    └─ build palette+indices with pixhaus-color-quant; set_color(Indexed),
│       set_palette(rgb), set_trns(alpha), write_image_data(&indices)
├─ Export an animation as one file?
│    └─ set_animated(frames, plays) then one write_image_data per frame (APNG);
│       but a sprite SHEET (grid PNG) is what Unity consumes
└─ Always: on spawn_blocking not the egui thread; set Limits on untrusted input;
   and the API is 0.18 — Filter (not FilterType), Compression::Balanced (not Default),
   output_buffer_size() -> Option.
```
