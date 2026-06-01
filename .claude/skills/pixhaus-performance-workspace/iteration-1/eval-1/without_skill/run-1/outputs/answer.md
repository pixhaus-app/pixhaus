# Benchmarking flood-fill in the core crate

The goal is a stable baseline you can compare against before and after your change. The standard tool for Rust micro/throughput benchmarks is **Criterion** — it runs the function many times, does statistical analysis, and tells you whether a change is a real regression or just noise. That's exactly the "don't make it slower" question you're asking.

Here's the full setup.

## 1. Make sure the `core` crate exists

The workspace is a clean slate (`members = []`). If you haven't created `core` yet, do that first so there's something to benchmark:

```bash
cargo new --lib core
```

Then add it to the root `Cargo.toml`:

```toml
[workspace]
members = ["core"]
```

Your flood-fill should be a `pub fn` in `core` so the benchmark can call it across the crate boundary. Something like:

```rust
// core/src/fill.rs
/// Flood-fill `buffer` (RGBA8, row-major, `width * height * 4` bytes) starting
/// at `(x, y)`, replacing the contiguous region of the start color with `fill`.
pub fn flood_fill(
    buffer: &mut [u8],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    fill: [u8; 4],
) {
    // ...
}
```

## 2. Add Criterion as a dev-dependency

Criterion is dev-only — it never ships in the library. In `core/Cargo.toml`:

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "flood_fill"
harness = false
```

`harness = false` is the important line — it tells Cargo not to use the built-in `libtest` bench harness (which is unstable/nightly-only) and lets Criterion own `main`.

Criterion is MIT/Apache-2.0, so it's clean against the repo's `cargo deny` license gate. As a dev-dependency it doesn't affect the shipped binary's dependency tree.

## 3. Write the benchmark

Create `core/benches/flood_fill.rs`:

```rust
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use pixhaus_core::flood_fill; // match your crate's actual package name

/// Build a `width x height` RGBA8 buffer filled with one color so a fill from
/// the corner has to traverse the whole canvas — the worst case for flood-fill.
fn solid_canvas(width: u32, height: u32) -> Vec<u8> {
    vec![0u8; (width as usize) * (height as usize) * 4]
}

fn bench_flood_fill(c: &mut Criterion) {
    let mut group = c.benchmark_group("flood_fill_full_canvas");

    // Cover small sprites up to the 8K worst case the project cares about.
    for &size in &[256u32, 1024, 4096, 8192] {
        let pixels = (size as u64) * (size as u64);
        group.throughput(Throughput::Elements(pixels)); // gives "Melem/s"

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            // Per-iteration setup: clone a fresh buffer so each run starts from
            // the same state. iter_batched keeps the clone out of the timed region.
            b.iter_batched(
                || solid_canvas(size, size),
                |mut buf| {
                    flood_fill(
                        black_box(&mut buf),
                        black_box(size),
                        black_box(size),
                        black_box(0),
                        black_box(0),
                        black_box([255, 0, 0, 255]),
                    );
                },
                criterion::BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

criterion_group!(benches, bench_flood_fill);
criterion_main!(benches);
```

A few things that matter for getting an honest number:

- **`iter_batched` with per-iteration setup.** Flood-fill mutates the buffer in place, so once it's filled the second run has nothing to do. You must hand each iteration a fresh buffer, and the clone must be outside the timed region — that's what `iter_batched` does (setup closure isn't timed, only the second closure is). Don't use plain `b.iter(|| ...)` with a pre-filled buffer; you'd be timing a no-op.
- **`black_box` on inputs.** Stops the optimizer from constant-folding or hoisting the call when arguments are known at compile time.
- **`Throughput::Elements`.** Reports millions of pixels per second, which is far more meaningful across canvas sizes than raw wall-clock, and it's the number that maps to the "scales to 8K" concern.
- **Cover the size range.** A flood-fill change can be neutral at 256x256 and a disaster at 8192x8192 (or vice versa — e.g. an algorithm with worse constant factors but better cache behavior). Bench the whole range so a regression can't hide.
- **Pick representative shapes, not just a solid canvas.** A solid canvas is the worst case (every pixel gets visited). Also consider a maze/checkerboard input if your algorithm's behavior depends on region shape — scanline vs. naive stack fill diverge a lot there. Add more `bench_with_input` cases as needed.

## 4. Capture the baseline, change the code, compare

Criterion's headline feature is automatic before/after comparison — it saves results under `target/criterion/` and diffs each run against the last.

```bash
# On the current (unchanged) code, record the baseline:
cargo bench -p core

# Now make your flood-fill change, then run again:
cargo bench -p core
```

The second run prints something like `change: [-1.2% +0.4%] (p = 0.31 > 0.05) No change in performance detected` or `Performance has regressed.` That's your answer.

For a cleaner workflow, name the baseline explicitly so a later edit doesn't overwrite it:

```bash
# Before the change, save a named baseline:
cargo bench -p core -- --save-baseline before

# After the change, compare against it (this run does NOT overwrite "before"):
cargo bench -p core -- --baseline before
```

Open `target/criterion/report/index.html` for the plots if you want to see the distribution.

## 5. Keep the measurement trustworthy

- **Run in release.** `cargo bench` compiles in release mode by default, so don't worry — but never judge perf from `cargo test` or a debug build.
- **Quiet the machine.** Close other heavy work; CPU frequency scaling and background load are the usual sources of noisy `p`-values. If results are jittery, bump sample size or measurement time: `group.measurement_time(std::time::Duration::from_secs(10))`.
- **Watch for allocation in the setup.** Allocating an 8192x8192x4 buffer is ~256 MB per iteration. `BatchSize::LargeInput` tells Criterion to run few iterations per batch so it doesn't try to hold thousands of these at once. If you still hit memory pressure, drop to `BatchSize::PerIteration`.
- **Compare apples to apples.** Run before and after on the same machine, same power state, ideally back-to-back. A baseline recorded on your laptop on battery vs. plugged in isn't comparable.

## 6. Note on the repo's gates

The repo's Stop hook runs `cargo test`/`clippy`/`fmt`, not `cargo bench` — benchmarks aren't part of the gate, so they won't block your session, but `clippy --all-targets` *does* lint bench files. Keep the bench code clean (no `unwrap` outside the bench body is the safe habit, though benches are test-like code). Benchmarks are run on demand, by you, when you want a number.

## Quick recap

1. Create/confirm the `core` crate with `flood_fill` as a `pub fn`.
2. Add `criterion` as a dev-dependency and a `[[bench]]` with `harness = false`.
3. Write `core/benches/flood_fill.rs` using `iter_batched` (fresh buffer per run), `black_box` on inputs, `Throughput::Elements`, and a 256→8192 size sweep.
4. `cargo bench -p core -- --save-baseline before`, make your change, then `cargo bench -p core -- --baseline before` to get a pass/regress verdict.
