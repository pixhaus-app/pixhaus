# Finding where the brush-drawing time actually goes

The rule before you change anything: don't guess, measure. "Laggy at 8K, smooth at 256" is a hypothesis about *what* is slow, not a finding about *where* the time goes. The fact that it scales with canvas size is a strong tell (it points at a full-canvas pass somewhere instead of dirty-region work), but a tell isn't proof. Profile first, then fix the box the profiler shows you. Here's the order I'd run it.

## 0. Make sure you're measuring the real thing

One trap up front: if you've been judging "laggy" from a `cargo run` (dev build) or from whatever `bacon` happened to compile, throw that impression out. A dev build is unoptimized — bounds checks and un-inlined calls dominate, and you'll end up "fixing" code that release would have erased. `bacon` is a correctness loop, not a perf number. Every timing and profile below comes from an optimized build.

Also confirm the slowness is real at the size that hurts. You already have that: 8192x8192 is exactly the size the canvas has to scale to. Reproduce there, not on a toy.

## 1. Build the right binary

The workspace defines three profiles. For *timing* you want `--release`. For *profiling* you want `release-with-debug`, which keeps release optimizations but also keeps debug symbols and skips `strip`, so the profiler can name functions instead of showing you a wall of `??`.

```powershell
# build the profilable binary
cargo build --profile release-with-debug
```

If you just want to confirm the lag is genuine and not a dev-build artifact, first run:

```powershell
cargo run --release
```

If it's smooth in `--release`, you were profiling the debug build's missing optimizations and you're done. I doubt that's the case at 8K, but it's a 30-second check that saves you from chasing a ghost.

## 2. Profile it with samply

Use `samply` here. It's cross-platform and works the same on your Windows machine as on macOS/Linux, needs no admin/perf setup, and opens the Firefox Profiler UI with a flamegraph plus a timeline. (`cargo flamegraph` works too, but on Windows its backend goes through ETW/blondie — more friction for no benefit.)

```powershell
cargo install samply
cargo build --profile release-with-debug
samply record .\target\release-with-debug\<binary>.exe
```

Then actually do the thing that's slow: open an 8192x8192 canvas and drag the brush around for several seconds. You want the profiler to capture the painful path under realistic load, so paint enough to fill a representative sample. Stop the recording and the UI opens.

## 3. Read the flamegraph

- X-axis is total CPU time — wider box = more time, either slower per call or called more often.
- Y-axis is stack depth — `main` at the bottom, leaves on top.
- Box color is random; ignore it.
- Hunt the **widest box near the top.** That's the leaf burning your cycles.

Compare mentally against the 256x256 case in your head: the box that's hair-thin at 256 and enormous at 8K is your culprit. That width-scales-with-canvas signature is the whole diagnosis.

## 4. What you'll most likely see (and where each leads)

In a canvas tool the bottleneck is almost always one of a short list. Match what samply shows to one of these — I'm naming the fix skills but **don't apply any of them until the profile actually points there:**

- **A per-pixel or per-scanline loop is the wide box.** First question, before any micro-opt: should that loop run over the whole canvas at all? A brush move only touches a small area. If the loop is iterating all 67 million pixels per drag event, the fix is algorithmic — bound the work to the dirty rectangle, not a faster full-canvas pass. That single change is what makes it scale. (8K-perf-constraint.) Only if it genuinely must touch every pixel — an export, a full-image filter — do you parallelize it across scanline chunks with rayon (pixhaus-rayon).
- **A buffer `clone` / allocation is hot.** An 8192x8192 RGBA buffer is ~256 MB. Cloning it once per brush move *is* the lag, and it'll be invisible at 256x256 (256 KB). pixhaus-rust-conventions covers borrowing over cloning, `Cow`, `mem::take`.
- **The UI stutters but total CPU is low / there are gaps in the timeline.** The heavy work is sitting on the egui frame thread and blocking it. Move it off-thread (`spawn_blocking`, return over a channel the frame loop drains) — pixhaus-tokio. The frame thread must never block.
- **Live compositing is CPU-bound at all.** The live canvas should composite on the GPU; CPU compositing is for export and thumbnails. pixhaus-wgpu.

## 5. Once you know the function, pin a number on it

If the hot path is a tight, isolatable function, add a criterion benchmark at a realistic size *before* you touch it, so your "before" is a number, not a memory.

```toml
[dev-dependencies]
criterion = "0.7"   # confirm current version on crates.io

[[bench]]
name = "brush"
harness = false
```

```rust
use std::hint::black_box;
use criterion::{Criterion, criterion_group, criterion_main};

fn bench_brush(c: &mut Criterion) {
    let mut canvas = PixelBuffer::filled(8192, 8192, Rgba::TRANSPARENT);
    c.bench_function("brush_dab_8192", |b| {
        b.iter(|| apply_brush(black_box(&mut canvas), black_box(stroke)));
    });
}
criterion_group!(benches, bench_brush);
criterion_main!(benches);
```

Use `std::hint::black_box` (not the deprecated `criterion::black_box`) so the optimizer can't fold the work away. Run it, make the change, run it again — criterion prints the delta and a confidence interval. Keep the change only if it clears a real margin (~5%+). If criterion can't distinguish your "win" from noise, it isn't a win — revert it, because you spent readability and bought nothing.

## What not to do while investigating

- Don't `#[inline]` or hand-unroll loops on a hunch. LTO already inlines aggressively; this is wasted readability before you've confirmed the bottleneck.
- Don't swap data structures speculatively. Profile first.
- Don't reach for `unsafe` for speed — it's forbidden workspace-wide, and the real wins here (dirty-rect bounding, off-thread, GPU) don't need it.
- Don't trust one noisy run. Background load skews timings; let criterion's intervals decide, or take the best of several `--release` runs.

## The short version

1. `cargo run --release` to confirm the lag survives an optimized build.
2. `cargo build --profile release-with-debug`, then `samply record` while you actually paint on the 8K canvas.
3. Read the flamegraph; find the widest box near the top that's huge at 8K but tiny at 256.
4. Given the size-scaling signature, the likely answer is a full-canvas pass or a full-buffer clone per brush move that should be bounded to the dirty rectangle — but let the profile name it, don't assume.
5. Pin the hot function with a criterion bench before changing it, and only keep the change if criterion shows a real, above-noise improvement.
