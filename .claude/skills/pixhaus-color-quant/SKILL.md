---
name: pixhaus-color-quant
description: >
  Use when reducing an image to a 256-or-fewer color palette in Pixhaus with the
  `color_quant` crate — generating a palette from an imported photo or reference,
  building an indexed-color image for GIF or indexed-PNG export, a "reduce
  palette" / posterize verb, or seeding a palette swatch panel from existing
  pixels. Trigger this for ANY "quantize this image", "extract a palette from",
  "reduce to N colors", "build an indexed image", "nearest palette color",
  "NeuQuant", "color map for the GIF/PNG", or "map every pixel to a palette index"
  task, even when the user doesn't say "color_quant". color_quant is the NeuQuant
  neural quantizer — the same one the `image` crate uses for GIF. It only ever
  reads RGBA bytes, its `samplefac` runs BACKWARDS (1 is best quality, not worst),
  it wants `colors >= 64`, and training is O(pixels) CPU work that must stay off
  the egui thread. Reach for this skill rather than guessing those, and to know
  when NeuQuant is the wrong tool for a tiny pixel-art palette.
---

# color_quant for Pixhaus

`color_quant` is a single-purpose crate: take an RGBA image and find a palette of
up to 256 colors that represents it well, using the NeuQuant algorithm (Anthony
Dekker's 1994 Kohonen-network quantizer). It's the same quantizer the `image`
crate reaches for when encoding a GIF, so it's well-trodden and battle-tested.

The whole crate is one struct, `NeuQuant`. You build it on a pixel buffer (which
trains the network), then ask it two kinds of question: "what's the palette?"
(`color_map_*`) and "which palette slot is closest to this pixel?" (`index_of` /
`map_pixel`). That's the entire surface.

In Pixhaus its jobs are palette generation (derive a swatch set from an imported
image) and indexed export (turn an RGBA layer into palette + index buffer for GIF
or indexed PNG). The export side pairs with the `io` crate's format code.

## Version and license

| Crate | Version | License | cargo deny |
|---|---|---|---|
| `color_quant` | 2.0 | `MIT` | passes the MIT lock |

MIT clears the workspace MIT lock — no GPL concern. It's a tiny crate with no
runtime dependencies. When you bump it, re-verify the signatures below against
docs.rs (see [[feedback-dep-upgrades]]).

```toml
color_quant = "2.0"
```

No feature flags. Nothing to configure.

## The whole API

```rust
// Build + train. samplefac in 1..=30, colors should be >= 64 (max 256),
// pixels is a flat RGBA byte buffer (len must be a multiple of 4).
pub fn new(samplefac: i32, colors: usize, pixels: &[u8]) -> NeuQuant

// The palette, three views. Length is colors * channels.
pub fn color_map_rgba(&self) -> Vec<u8>   // len colors*4, RGBA
pub fn color_map_rgb(&self)  -> Vec<u8>   // len colors*3, RGB
pub fn color_map_alpha(&self) -> Vec<u8>  // len colors,   alpha only (PNG tRNS chunk)

// Map a single RGBA pixel (a 4-byte slice) to its nearest palette slot.
pub fn index_of(&self, pixel: &[u8]) -> usize        // -> palette index
pub fn map_pixel(&self, pixel: &mut [u8])            // overwrites in place with the palette color
pub fn lookup(&self, idx: usize) -> Option<[u8; 4]>  // palette index -> RGBA, None if out of range

// Re-train on different data (new already calls this for you; rarely needed).
pub fn init(&mut self, pixels: &[u8])
```

`NeuQuant` has no `Clone`, `Debug`, `Serialize`, or `Deserialize`. It is a
transient computation, not document state: build it, pull the palette and indices
out, drop it. Don't try to stash it in the `.pixhaus` model — store the resulting
`Vec<u8>` palette instead.

## The canonical flow: image to palette + indices

```rust
use color_quant::NeuQuant;

// `rgba` is a flat RGBA8 buffer — exactly Pixhaus's pixel-buffer shape.
let nq = NeuQuant::new(10, 256, &rgba);

// The palette: 256 RGBA entries, flat. Hand this to a swatch panel or an encoder.
let palette: Vec<u8> = nq.color_map_rgba();

// The indexed image: one palette index per pixel.
let indices: Vec<u8> = rgba
    .chunks_exact(4)
    .map(|px| nq.index_of(px) as u8)
    .collect();
```

`chunks_exact(4)` is the right iterator — it walks one RGBA pixel per step and
drops any trailing partial pixel rather than panicking. The `as u8` cast is safe
because `colors <= 256`, so every index fits a byte; that's the whole point of an
indexed image. For a "quantized preview" that stays RGBA (every pixel snapped to
its nearest palette color, no index buffer), map in place instead:

```rust
let mut preview = rgba.clone();
for px in preview.chunks_exact_mut(4) {
    nq.map_pixel(px); // overwrites the 4 bytes with the chosen palette color
}
```

## Three facts that bite

1. **Input is always RGBA, never RGB.** `new` and `index_of` read 4 bytes per
   pixel. Feed a 3-byte-per-pixel RGB buffer and the channels misalign — every
   "pixel" is read off by one byte and the palette comes out garbage, with no
   error. Pixhaus pixel buffers are already RGBA8, so this is usually free; the
   trap is when you've stripped alpha somewhere upstream. Expand back to RGBA
   first.

2. **`samplefac` runs backwards from the intuition.** It's not "more samples =
   better." It's the divisor: `1` trains on every pixel (best quality, slowest),
   `30` trains on a thin sample (fastest, roughest). `10` is the documented
   speed/quality compromise and a fine default. Lower it toward `1` for a final
   export where quality matters; raise it for a live preview where you re-quantize
   on every slider drag. Out-of-range values (`< 1` or `> 30`) are not what the
   algorithm expects — clamp to `1..=30` before calling.

3. **`colors` should be `>= 64`.** NeuQuant is built around a network of at least
   64 neurons; ask for fewer and the result degrades in ways the algorithm wasn't
   tuned for. If a user wants a 16-color palette, generating 256 and then reducing
   (or reaching for a different quantizer — see below) beats passing `colors = 16`
   directly. The hard ceiling is 256.

## When NeuQuant is the wrong tool

NeuQuant shines on photographic or many-colored source images — importing a
reference photo and deriving a working palette from it. It's a neural averager,
which is exactly what you want there.

It's a poor fit for small target palettes and hard-edged pixel art. The network
blends nearby colors toward cluster centers, so two deliberately-distinct flat
colors can collapse into one averaged slot, and you can't faithfully hit a 16-color
target (see fact 3). When the job is "give me a tight 8/16/32-color palette that
keeps the distinct hues," median-cut or k-means quantization preserves intent
better. color_quant doesn't offer those — note the limitation to the user and
treat a different quantizer as the right call rather than forcing `colors` low.

Also: NeuQuant produces a palette, not a dithered image. Mapping each pixel to its
nearest palette color (`index_of` / `map_pixel`) with no dithering shows visible
banding on gradients. Dithering is a separate step the crate doesn't do.

## Threading: train off the egui thread

`new` (i.e. `init`) is O(pixels) CPU work, and at the [[8k-perf-constraint]] an
8192x8192 image is 268 MB of RGBA and tens of millions of training samples — that
is not a per-frame operation. Per the workspace async rules
(`pixhaus-rust-conventions`), CPU-bound work runs on `tokio::task::spawn_blocking`
and the egui update loop never blocks on it. Quantize on a blocking task and
deliver the palette + index buffer back over a channel the loop drains each frame.
Do not call `NeuQuant::new` inline in a `ui`/`logic` path on a full-size image;
that freezes the frame.

The `index_of`/`map_pixel` mapping pass is also O(pixels) — keep it on the same
blocking task, not the UI thread. For a live preview, raising `samplefac` and/or
quantizing a downscaled copy keeps it responsive; do the full-quality pass once on
commit.

## Errors

`color_quant` has no `Result`-returning API — `new` always builds, `index_of`
always returns an index, `lookup` returns `Option` for an out-of-range index.
There's nothing to map into a `thiserror` enum here. The failure modes are
upstream and your responsibility: validate that the input length is a multiple of
4 (RGBA) and that `colors` is in `64..=256` before calling, and surface a bad
input as the `io`/`core` crate's error rather than feeding the quantizer a
malformed buffer. No `unwrap()` is needed anywhere in this crate's flow.

## Decision shortcut

```
Reducing an image to a palette in Pixhaus?
├─ Source is photographic / many-colored, want a derived working palette?
│    └─ NeuQuant::new(10, 256, &rgba) -> color_map_rgba()   (this crate, the sweet spot)
├─ Need an indexed image for GIF / indexed-PNG export?
│    └─ palette = color_map_rgba(); indices = rgba.chunks_exact(4).map(|p| nq.index_of(p) as u8)
│       (color_map_alpha() for the PNG tRNS chunk)
├─ Want a tight 8/16/32-color palette that preserves distinct flat colors?
│    └─ NOT NeuQuant (colors must be >=64, and it averages hues) — use median-cut / k-means
└─ Whatever the case: input is RGBA (multiple of 4), samplefac 1..=30 (1=best),
   and run new() + the mapping pass on spawn_blocking, never the egui thread.
```
