# Optimizing `blend_normal`

Short answer: I'm not going to rewrite this on the strength of "feels slow." The rule in this repo (CLAUDE.md, and the perf workflow) is **don't guess, measure** — no optimization lands without a benchmark or profile showing this code is actually the bottleneck. So let's do this in the order that produces evidence, and I'll tell you exactly what I'd change and why once we have it. That said, this function has two specific problems I'm confident about *on inspection* (not on a timing hunch), and one structural one that matters more than the inner loop. Let me separate "almost certainly true regardless of measurement" from "needs a number."

## First: are you even timing a release build?

This is the single most common false alarm. If "feels slow" came from `cargo run` (a `dev` build), throw the observation out — it tells you nothing real. Bounds checks and un-inlined calls dominate a debug build in ways `release` erases entirely.

```bash
# honest timing — full opt-level 3, thin LTO
cargo run --release
```

If it's still slow in release, now we have a real problem to chase. Half the time, this step ends the investigation.

## The two things wrong on inspection (not hunches)

### 1. `.unwrap()` in a per-pixel loop violates the no-unwrap rule and adds a branch per pixel

`src.pixel(x, y).unwrap()` and `dst.pixel(x, y).unwrap()` run on every single pixel. At 8192×8192 that's ~67 million `Option` unwraps, each a bounds check plus a panic branch — and `.unwrap()` outside tests is clippy-banned here anyway. The coordinates are loop-derived from `0..height`/`0..width`, so they're provably in bounds; the per-pixel `Option` plumbing is pure overhead. This is a correctness-convention fix that also happens to remove work, so it's worth doing independent of any benchmark.

### 2. You're doing per-pixel 2D indexing instead of iterating the buffer

`pixel(x, y)` / `set_pixel(x, y)` recompute `y * stride + x` (and bounds-check) on every access, three times per pixel (two reads, one write). CLAUDE.md mandates pixel buffers are flat `Vec<u8>` with explicit stride. The idiomatic, faster form iterates the byte slices directly — let the iterator carry the position so the optimizer can elide bounds checks and vectorize the channel math. Prefer iterators over indexing (a stated convention, and it's what lets the autovectorizer work).

Roughly (adjust to your real `PixelBuffer` API — assuming RGBA8, 4 bytes/pixel, matching dimensions):

```rust
/// Composite `src` over `dst` in place (normal/source-over).
/// Requires identical dimensions; callers slice to the overlap first.
pub fn blend_normal(src: &PixelBuffer, dst: &mut PixelBuffer) {
    debug_assert_eq!(src.width, dst.width);
    debug_assert_eq!(src.height, dst.height);

    for (s, d) in src
        .pixels()        // &[u8; 4] per pixel, or chunks_exact(4)
        .zip(dst.pixels_mut())
    {
        *d = alpha_over(*s, *d);
    }
}
```

If `PixelBuffer` doesn't expose pixel iterators yet, `src.bytes().chunks_exact(4).zip(dst.bytes_mut().chunks_exact(4))` gets you there over the flat `Vec<u8>`. `chunks_exact` is the right call — it gives the optimizer a known stride and no remainder branch in the hot path.

Note I have **not** added `#[inline]` to `alpha_over`. LTO inlines aggressively already; adding it on a hunch is noise. If a benchmark later shows it matters across the crate boundary, then add it — not before.

## The thing that matters more than the inner loop: bound the work

Here's the contrarian-with-cause part. Making this loop 3x faster is the *wrong fix* if it's running over the whole canvas during live painting. The 8K perf constraint in this repo is explicit: per-brush-move work must be bounded by the **dirty region**, not total canvas size. A brush stroke touches a few hundred pixels; compositing all 67M every move is the actual bug, and no amount of iterator tuning or SIMD saves you from doing 67M pixels of pointless work.

So the question that decides everything: **where is `blend_normal` called from?**

- **Live drawing / per-frame layer compositing** → the durable fix is *not* a faster full-canvas pass. It's (a) blend only the dirty rectangle each move, and ideally (b) composite the live canvas on the **GPU**, not the CPU. CPU compositing is for export and thumbnails. That's a [[pixhaus-wgpu]] conversation, and it's where the real 8K win lives.
- **Export / a one-shot full-canvas filter** → it genuinely must touch every pixel, so the full pass is correct. *Then* parallelize it across scanline chunks with rayon ([[pixhaus-rayon]]) — `par_chunks_mut` over the destination, zipped with source. That's the case where rayon's splitting overhead pays off, because the work is genuinely O(whole canvas).
- **UI stutters but total CPU is low** → the work is on the egui frame thread blocking it. Move it to `spawn_blocking` and return over a channel the frame loop drains ([[pixhaus-tokio]]). The frame thread must never block.

I can't pick between these for you without seeing the call site, and the right answer is completely different for each. That's the measurement.

## So here's what I'd actually do, in order

1. **Confirm it's slow in `--release`**, not dev. (`cargo run --release`.) If it's fine here, we're done.
2. **Profile the slow scenario at 8K** with samply on a profilable build — release optimizations *plus* symbols so the profiler names functions instead of showing `??`:
   ```bash
   cargo build --profile release-with-debug
   samply record ./target/release-with-debug/<binary>
   ```
   Reproduce the slow paint/export, then read the flamegraph: hunt the widest box near the top. If `blend_normal` isn't a wide box, the time is somewhere else and we just avoided "fixing" the wrong function.
3. **If `blend_normal` is the wide box, pin it with a criterion benchmark** at a realistic size so "before" is a number, not a memory. Add to the core crate:
   ```toml
   [dev-dependencies]
   criterion = "0.7"   # confirm latest on crates.io; MIT OR Apache-2.0, clears cargo deny

   [[bench]]
   name = "blend"
   harness = false
   ```
   ```rust
   use std::hint::black_box;
   use criterion::{Criterion, criterion_group, criterion_main};

   fn bench_blend(c: &mut Criterion) {
       let src = PixelBuffer::filled(2048, 2048, Rgba::RED);
       let mut dst = PixelBuffer::filled(2048, 2048, Rgba::BLUE);
       c.bench_function("blend_normal_2048", |b| {
           b.iter(|| blend_normal(black_box(&src), black_box(&mut dst)));
       });
   }
   criterion_group!(benches, bench_blend);
   criterion_main!(benches);
   ```
4. **Apply the iterator + no-unwrap rewrite above**, re-run the bench. criterion prints the delta against the stored baseline ("change: -X% ... Performance has improved.").
5. **Keep it only if it clears ~5%** outside the noise interval. If criterion can't distinguish it from noise, revert — you'd be spending readability for nothing. (The no-unwrap fix stays regardless; that's a convention fix, not a perf one.)
6. **Then go after the structural fix** the profile pointed at — dirty-rect bounding, GPU compositing, off-thread, or rayon — per the call-site analysis above. That's almost always a bigger win than the inner loop.

## What I won't do

- Won't reach for `unsafe` for speed — it's forbidden workspace-wide. The safe paths (release profile, iterators, rayon, GPU, dirty-rect) cover the real wins.
- Won't `#[inline]` or hand-unroll on a hunch before the profiler names the bottleneck.
- Won't optimize for a canvas size that doesn't actually stall — a 64×64 sprite doesn't need any of this.

Tell me where `blend_normal` is called from (live paint vs export) and whether the slowness reproduces under `--release`, and I'll take it the rest of the way — including wiring up the benchmark and doing the rewrite.
