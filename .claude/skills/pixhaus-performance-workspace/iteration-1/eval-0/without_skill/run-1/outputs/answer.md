# Optimizing `blend_normal`

Short answer: yes, and the speedup is large. The current version is slow for three separate reasons, and they compound. Let me walk through each, then give you a rewrite.

## Why it's slow

1. **Per-pixel method calls with bounds checks and `Option`.** `src.pixel(x, y).unwrap()` and `dst.set_pixel(x, y, ...)` each recompute the index `y * width + x`, do a bounds check, wrap the result in `Option`, and then you `unwrap()` it. That's a lot of ceremony for "read four bytes." The compiler *might* inline and hoist some of it, but accessor methods that return `Option` rarely optimize down to a clean linear scan. You're paying an index multiply and a bounds check on every one of `width * height` iterations.

2. **Column-major access pattern.** Your inner loop varies `x` while `y` is fixed, which is the right order for a row-major buffer — good. But you're still addressing pixel-by-pixel through accessors, so you never let the CPU prefetcher and the compiler see that you're walking memory linearly. The whole point of a `Vec<u8>` with explicit stride is that a blend is a *streaming* operation over contiguous bytes.

3. **You blend the entire canvas every call.** This is the big one for painting. When the user drags a brush, you touch maybe a few hundred pixels, but `blend_normal` walks all `width * height` of them. On an 8192×8192 canvas that's 67 million pixels per brush move regardless of brush size. No amount of micro-optimizing the inner loop fixes an algorithm that does work proportional to canvas size instead of work proportional to what changed.

The order to fix these: **#3 first** (it's the one that actually bites when painting), then **#1 and #2** (which make the per-pixel cost cheap for whatever region you do touch).

## Fix #3: blend only the dirty region

Give blends a rectangle to operate on. The brush already knows the bounding box of what it just painted; pass that down. Everything else in this answer assumes you've bounded the work — a tight inner loop over 67M pixels is still 67M pixels.

```rust
/// Axis-aligned region in pixel coordinates, clamped to the buffer by the caller.
#[derive(Clone, Copy)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub fn blend_normal_in(src: &PixelBuffer, dst: &mut PixelBuffer, region: Rect) {
    // ... blend only rows region.y .. region.y + region.height,
    //     columns region.x .. region.x + region.width
}
```

If `src` is a small brush stamp at an offset rather than a full-canvas layer, even better — its dimensions *are* the dirty region and you never allocate a canvas-sized source.

## Fix #1 and #2: operate on byte slices, not accessors

Assuming RGBA8 (4 bytes per pixel) and a row stride in bytes, work directly on the backing `&[u8]` / `&mut [u8]` and let the iterator carry the bounds proof so the compiler drops per-pixel checks. Here's a region-aware version that processes one row at a time and chunks each row into 4-byte pixels:

```rust
pub fn blend_normal_in(src: &PixelBuffer, dst: &mut PixelBuffer, region: Rect) {
    const BPP: usize = 4; // bytes per pixel, RGBA8

    let dst_stride = dst.stride(); // bytes per row
    let src_stride = src.stride();

    let x0 = region.x as usize;
    let row_bytes = region.width as usize * BPP;

    for row in 0..region.height as usize {
        let dy = region.y as usize + row;

        let d_start = dy * dst_stride + x0 * BPP;
        let s_start = row * src_stride; // adjust if src has its own offset

        let dst_row = &mut dst.bytes_mut()[d_start..d_start + row_bytes];
        let src_row = &src.bytes()[s_start..s_start + row_bytes];

        for (s, d) in src_row
            .chunks_exact(BPP)
            .zip(dst_row.chunks_exact_mut(BPP))
        {
            let out = alpha_over_rgba([s[0], s[1], s[2], s[3]], [d[0], d[1], d[2], d[3]]);
            d.copy_from_slice(&out);
        }
    }
}
```

Why this is faster:

- **`chunks_exact` gives the optimizer a clean, bounds-check-free loop.** It knows the length is a multiple of 4 and that each chunk is exactly 4 bytes, so the per-pixel bounds checks vanish and it can autovectorize the arithmetic.
- **One slice index per row instead of two method calls per pixel.** The index multiply happens `height` times, not `width * height` times.
- **Linear memory walk** within each row keeps the prefetcher happy.

You'll need `bytes()` / `bytes_mut()` / `stride()` accessors on `PixelBuffer` if they don't exist. They're cheap to add and they're the right primitive for any streaming pixel op (blend, fill, copy, channel swap) — not just this one.

## Make the per-pixel blend math integer-only

If your `alpha_over` works in `f32` and divides by 255.0 per channel, that's another hidden cost. The standard integer "over" with rounding avoids floats entirely and stays autovectorizable:

```rust
#[inline]
fn alpha_over_rgba(src: [u8; 4], dst: [u8; 4]) -> [u8; 4] {
    let sa = src[3] as u32;
    let inv = 255 - sa;

    // out = src + dst * (1 - src_a), premultiplied-style with rounding.
    let blend = |s: u8, d: u8| -> u8 {
        let s = s as u32 * sa;
        let d = d as u32 * inv;
        let sum = s + d + 127; // round-to-nearest
        ((sum + (sum >> 8)) >> 8) as u8 // fast /255 approximation
    };

    let out_a = sa + (dst[3] as u32 * inv + 127 >> 8); // or compute the same way

    [
        blend(src[0], dst[0]),
        blend(src[1], dst[1]),
        blend(src[2], dst[2]),
        out_a.min(255) as u8,
    ]
}
```

Adjust to match whatever your `alpha_over` actually computes (straight vs. premultiplied alpha changes the formula). The point is: keep it `u32` integer math, mark it `#[inline]`, and the row loop above will vectorize across pixels.

## Don't reach for threads yet

A multi-row `rayon` split (`par_chunks_mut` over rows) is a real option, but only *after* you've bounded the work to the dirty region. For a brush-sized region the threading overhead would dominate. Save parallelism for whole-canvas operations — full-layer composite, filters, export — where the region genuinely is the canvas. When you do, split `dst.bytes_mut()` into row-stride chunks with `par_chunks_mut(stride)` so each thread owns disjoint rows; that keeps it data-race-free without locks.

## Order of operations

1. Add `bytes()`, `bytes_mut()`, `stride()` to `PixelBuffer` (and confirm RGBA8 / the actual layout).
2. Add the dirty-`Rect` parameter and have the brush pass its bounding box.
3. Rewrite the loop with `chunks_exact` over byte slices.
4. Make `alpha_over` integer-only and `#[inline]`.
5. Benchmark a realistic brush stroke (a `criterion` bench on a small region) before adding any parallelism.

## One caution

Match the exact blend math of your existing `alpha_over` before you ship — straight vs. premultiplied alpha, and the rounding, change pixel values by ±1, which a snapshot/`image-compare` test will flag. Keep a test that blends a known src over a known dst and asserts the exact output bytes, so the optimization can't silently change what users see.
