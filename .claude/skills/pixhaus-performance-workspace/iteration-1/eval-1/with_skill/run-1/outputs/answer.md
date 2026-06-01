# Benchmarking the flood-fill before you change it

Good instinct: in this repo no perf change lands without a benchmark or profile showing the
current code is the bottleneck. For flood-fill the move is to pin the *current* behavior as a
number first, so after your rewrite criterion can tell you whether you got faster, stayed flat,
or regressed. Here's the full setup.

## 1. Add criterion as a dev-dependency on the core crate

In `core/Cargo.toml` (the crate that owns flood-fill — confirm the path; the workspace is a
clean slate so this may be the first crate you add):

```toml
[dev-dependencies]
criterion = "0.7"   # confirm current version on crates.io; MIT OR Apache-2.0, clears cargo deny

[[bench]]
name = "flood_fill"
harness = false      # required — criterion provides its own main()
```

`harness = false` is not optional. Leave it off and `cargo bench` tries to use the built-in
nightly bench harness and your `criterion_main!` never runs.

## 2. Write the benchmark in `benches/`

Create `core/benches/flood_fill.rs`. The point of a flood-fill benchmark is to measure it at a
size that actually hurts and on inputs that exercise the real cost — this is a canvas tool, so
its perf story is dominated by canvas size (the 8K constraint). Benchmark at 4K/8K, not a toy
256x256 that's instant either way.

Flood-fill cost also depends heavily on *what gets filled*, so benchmark more than one shape:

- **Whole-canvas fill** — worst case, the fill visits every pixel. This is the case your
  rewrite most needs to not regress.
- **A bounded region** — a fill that hits a boundary early. Catches algorithms that scan the
  whole canvas regardless of how much they actually fill.

```rust
use std::hint::black_box;
use criterion::{Criterion, criterion_group, criterion_main};
// adjust these imports to the real types/fn once they exist
use core::{flood_fill, PixelBuffer, Rgba};

fn bench_flood_fill(c: &mut Criterion) {
    // Worst case: an empty canvas where the fill spreads to every pixel.
    c.bench_function("flood_fill_whole_4096", |b| {
        let base = PixelBuffer::filled(4096, 4096, Rgba::TRANSPARENT);
        b.iter_batched(
            || base.clone(),                       // fresh buffer per iter — fill mutates
            |mut buf| {
                flood_fill(
                    black_box(&mut buf),
                    black_box((0, 0)),             // seed
                    black_box(Rgba::RED),          // fill color
                );
                buf
            },
            criterion::BatchSize::LargeInput,
        );
    });

    // Bounded case: fill stops at a boundary, touching only part of the canvas.
    c.bench_function("flood_fill_bounded_4096", |b| {
        let base = bordered_region(4096, 4096);    // your helper: a fenced-off area
        b.iter_batched(
            || base.clone(),
            |mut buf| {
                flood_fill(black_box(&mut buf), black_box((10, 10)), black_box(Rgba::RED));
                buf
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

criterion_group!(benches, bench_flood_fill);
criterion_main!(benches);
```

Two details that matter:

- **`iter_batched` with a fresh clone per iteration**, not plain `b.iter`. Flood-fill mutates
  the buffer in place — once it's filled, a second run fills nothing and you'd be timing a
  no-op. `iter_batched` rebuilds the input each time; the clone happens in the setup closure,
  outside the measured section.
- **`black_box` on inputs and the returned buffer**, using `std::hint::black_box` (not the
  deprecated `criterion::black_box`). Without it the optimizer can prove the fill result is
  unused and delete the whole call — you'd "benchmark" nothing and see absurdly fast numbers.

## 3. Capture the baseline, change the code, re-run

criterion remembers the previous run and prints the delta, so the workflow is:

```bash
# baseline, on the CURRENT flood-fill
cargo bench -p core --bench flood_fill

# ... make your change ...

# same command — criterion compares against the saved baseline
cargo bench -p core --bench flood_fill
```

The second run prints something like `change: -18.3% [...] Performance has improved.` That
delta is what justifies the change. The rule of thumb: keep it only if it clears a real margin
(~5%+). If criterion reports the change as within noise (overlapping confidence intervals), the
rewrite bought you nothing measurable — revert it rather than ship less-readable code for an
imaginary win. Don't trust a single noisy run; criterion's intervals exist precisely so
background load doesn't fool you.

## 4. If you want to know *where* the time goes, profile

A benchmark tells you whether you got faster; a profiler tells you *what* to change. If you're
not sure the inner loop is even the bottleneck, profile before rewriting. Build the profilable
binary with the right profile and record with samply (preferred here — cross-platform, works
the same on your Windows machine, no sudo/perf setup):

```bash
cargo install samply
cargo build --profile release-with-debug
samply record ./target/release-with-debug/<binary>   # then drive a flood-fill in the app
```

Use `release-with-debug`, never `dev`. A `dev`-build profile is a profile of missing
optimizations (bounds checks, un-inlined calls) — you'd "fix" code that release would erase.
`release-with-debug` gives you release speed plus the debug symbols samply needs to name
functions instead of showing `??`.

## Build profile — the thing that's easy to get wrong

| Doing | Profile |
|---|---|
| `cargo bench` timings | criterion builds optimized automatically — don't second-guess it |
| Profiling with samply/flamegraph | `--profile release-with-debug` |
| Quoting any "is it fast" number by hand | `--release` |
| Day-to-day editing / `bacon` | `dev` — never quote its timings |

`bacon` is a correctness loop (check/clippy), not a perf number — quote timings from a
deliberate `cargo bench` or `--release` run, not from whatever bacon happened to compile.

## A note specific to flood-fill

When you read the result, the durable win for a canvas-scale path is usually algorithmic, not a
faster inner loop. If the profile shows flood-fill burning time proportional to total canvas
size even when it fills a small area, the fix is to bound the work to the region actually
touched (and track the dirty rect for the redraw) rather than micro-tuning the scan. If it
genuinely must visit every pixel (a true whole-canvas fill), that's where parallelizing across
scanline chunks with rayon earns its keep — but only after the benchmark or profile names it
the bottleneck, and only at a size big enough that the split overhead pays off. A 64x64 sprite
doesn't need any of this.

And don't reach for `unsafe` to speed up the fill — it's forbidden workspace-wide. The safe
paths (release profile, dirty-rect bounding, rayon) cover the real wins.

## TL;DR

1. Add `criterion` dev-dep + `[[bench]]` with `harness = false` on `core`.
2. Write `core/benches/flood_fill.rs`: realistic 4K/8K sizes, both whole-canvas and bounded
   cases, `iter_batched` for the per-iteration clone, `std::hint::black_box` on inputs/output.
3. `cargo bench -p core --bench flood_fill` to capture the baseline, change the code, run it
   again, read the delta. Keep the change only if it clears ~5%; revert if it's within noise.
4. Unsure where the time goes? `cargo build --profile release-with-debug` + `samply record`
   first.
