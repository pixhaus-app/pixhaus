# Export formats

Pixhaus exports animations through three formats. Each has different quality
knobs and constraints. Pick based on your target use case.

---

## GIF

**Module:** `pixhaus_io::animated::gif`  
**Entry point:** `encode_gif(frames, options, existing_palette, writer)`

GIF is limited to 256 colours per frame and no alpha channel (transparency is
binary — a pixel is either fully transparent or opaque). It is the only format
with universal browser and messaging-app support.

### Palette modes

| Mode | Description |
|------|-------------|
| `PaletteMode::GlobalQuantize` (default) | Runs NeuQuant across all frames and builds one shared 256-colour palette. Best compression; colour accuracy degrades when the sprite uses many colours across frames. |
| `PaletteMode::PerFrameQuantize` | Each frame gets its own 256-colour palette via the `gif` crate's built-in quantizer. Better per-frame colour accuracy at the cost of larger files. |
| `PaletteMode::ExistingPalette` | Uses the project's palette as-is. Fails with `Error::PaletteExceedsGifMax` if the palette has more than 256 entries. Recommended when the sprite was painted with a deliberately limited palette. |

### Dithering

| Mode | Description |
|------|-------------|
| `DitherMode::Off` (default) | Each pixel snaps to its nearest palette colour. Fastest; visible banding on gradients. |
| `DitherMode::FloydSteinberg` | Error-diffusion dithering. Best visual quality; temporal noise between frames when each frame is quantized independently. |
| `DitherMode::Bayer8x8` | Ordered 8×8 Bayer threshold dithering. Stable cross-hatch pattern across frames; friendlier to GIF LZW compression than Floyd-Steinberg. |

Dithering is applied before quantization. When `palette_mode` is
`PerFrameQuantize`, the `gif` crate handles its own internal quantization and
dithering is not applied (the `dither` field is ignored).

### Loop count

`LoopCount::Infinite` (default) loops forever.  
`LoopCount::Count(n)` plays the animation `n` times then stops.

### Frame timing

GIF stores frame delays in centiseconds (1/100 s). Pixhaus converts
`duration_ms` to centiseconds and floors to a minimum of 1 cs (10 ms). Very
short durations (< 10 ms) are rounded up; players that ignore sub-10 ms delays
will not stall.

---

## Animated WebP

**Module:** `pixhaus_io::animated::webp`  
**Entry point:** `encode_webp(frames, options)`

Animated WebP preserves full RGBA colour, supports per-pixel alpha, and is
typically 30–50% smaller than GIF for pixel art. Supported in all major
browsers (Chrome 32+, Firefox 65+, Safari 14+).

### Options

| Field | Default | Description |
|-------|---------|-------------|
| `lossless` | `true` | Use VP8L lossless codec. Exact colour reproduction. Recommended for pixel art. When `true`, `quality` is ignored. |
| `quality` | `90.0` | Lossy VP8 quality, `0.0`–`100.0`. Higher = larger file, fewer artefacts. Only effective when `lossless` is `false`. |
| `method` | `4` | Compression effort, `0`–`6`. `0` is fastest (larger file); `6` is slowest (smallest file). `4` is the libwebp default. |
| `loop_count` | `0` | Number of loops. `0` = infinite. |

### When to use lossy

Lossy WebP produces smaller files but introduces block artefacts. This is
rarely desirable for pixel art. Use it only when file size is critical and the
target display is small enough that artefacts are invisible.

---

## MP4

**Module:** `pixhaus_io::animated::mp4`  
**Entry point:** `encode_mp4(frames, options)`

MP4/H.264 is the standard for video sharing — social platforms, messaging
apps, and email clients that reject GIF will accept MP4. Frame timing is
converted to a constant framerate (average of all per-frame durations).
Per-frame timing variance is flattened; use GIF or WebP if per-frame timing
precision matters.

**Requires `ffmpeg` on `PATH`.** If not found, returns
`Error::FfmpegNotFound`. Install via your system package manager or from
[ffmpeg.org](https://ffmpeg.org/download.html).

### Options

| Field | Default | Description |
|-------|---------|-------------|
| `crf` | `23` | Constant-rate factor, `0`–`51`. Lower = higher quality, larger file. `18`–`28` is the typical range. `0` is lossless (very large). |
| `pix_fmt` | `"yuv420p"` | Pixel format passed to ffmpeg via `-pix_fmt`. `yuv420p` ensures maximum browser and player compatibility. Change only if you know your target supports it. |
| `extra_args` | `[]` | Additional raw ffmpeg arguments appended after the standard flags. Invalid arguments surface as `Error::FfmpegFailed`. |

### Framerate

The average frame duration across all frames is used to compute a constant
framerate (`fps = 1000 / avg_ms`). A single-duration animation (all frames
the same duration) will produce the exact framerate you expect. Mixed-duration
animations will be slightly off for individual frames but correct on average.

If all frames have `duration_ms = 0`, the framerate defaults to 60 fps. If the
frame list is empty, `encode_mp4` returns `Error::NoAnimationFrames` before
touching the filesystem.

### Pipeline

1. Each frame is written as a PNG file to a temporary directory under the
   system temp path.
2. `ffmpeg` is invoked with the PNG sequence as input and `output.mp4` as
   output.
3. The MP4 is read back into memory as `Vec<u8>`.
4. The temporary directory is deleted (via RAII drop guard).

Temp directory names include a nanosecond timestamp to avoid collisions when
multiple exports run concurrently.
