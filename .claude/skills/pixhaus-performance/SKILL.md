---
name: pixhaus-performance
description: >
  Use when making Pixhaus Rust code faster, or deciding whether it needs to be — the
  measurement workflow that turns "this feels slow" into evidence before anyone touches
  a hot loop. Trigger this for ANY "is this fast enough", "make this faster", "optimize
  this", "why is this slow", "this lags on a 4K/8K canvas", "profile this", "where's the
  bottleneck", "set up a benchmark", "cargo bench / criterion", "flamegraph", "samply",
  "is this clone/alloc expensive", "did my change regress perf", or "should I rewrite
  this for speed" request, even when the user never says "performance" or "benchmark".
  The rule this skill enforces is the one CLAUDE.md states and agents keep breaking:
  don't guess, measure — no optimization lands without a benchmark or profile showing
  the current code is the bottleneck. This skill owns the HOW of measuring (the right
  build profile, clippy's perf lints, criterion micro-benchmarks, samply/flamegraph
  profiling, reading the result) and points at the skills that own the fixes:
  [[pixhaus-rust-conventions]] for cloning/allocation/iterator micro-opts,
  [[pixhaus-rayon]] for parallelism, [[pixhaus-tokio]] for keeping work off the UI
  thread, [[pixhaus-wgpu]] for moving pixels to the GPU. Reach for it before optimizing,
  not after a guess turns out wrong.
---

# Performance for Pixhaus

The golden rule, and the one agents break most: **don't guess, measure.** Rust compiled
in release is usually already fast. "This looks slow" is a hypothesis, not a finding —
optimizing on a hunch trades readable code for speed you can't prove you gained, and
often the hunch is wrong (the time was somewhere else entirely). CLAUDE.md says it
plainly: rewrite for performance *only* with a benchmark showing the current code is the
bottleneck. This skill is how you get that benchmark or profile.

This skill owns the **measurement workflow**. It does not re-teach the micro-fixes —
those live in skills that already cover them, and this skill links to them:

- Cloning, allocation, `Cow`, `Vec<u8>` vs `Box<[u8]>`, iterators vs index loops,
  `Vec<Vec<T>>` flattening, `Box<dyn>` vs generics → [[pixhaus-rust-conventions]].
- Parallelizing a hot per-pixel/per-scanline loop across cores → [[pixhaus-rayon]].
- Keeping heavy work off the egui frame thread (`spawn_blocking`, channels) →
  [[pixhaus-tokio]].
- Moving live compositing/drawing onto the GPU instead of the CPU → [[pixhaus-wgpu]].

Use this skill to decide *whether* to reach for any of those, and to prove the change
helped after you do.

## The loop

1. **Reproduce the slowness on a realistic input.** A pixel-art editor's perf story is
   dominated by canvas size — a path that's instant at 256x256 can stall at
   8192x8192 ([[8k-perf-constraint]]). Measure at the size that hurts, not a toy.
2. **Build with optimizations.** Profiling or timing a `dev` build tells you nothing
   real — see the profile section below. This is the single most common false alarm:
   "Rust is slow" almost always means "I forgot `--release`".
3. **Locate the cost with a profiler** (samply / flamegraph). Find the widest box.
4. **If it's a tight code path, pin it with a criterion benchmark** so the "before" is a
   number, not a memory.
5. **Make the change** — reach for the right fix skill above.
6. **Re-run the benchmark.** Keep the change only if it clears a real margin (the
   chapter's rule of thumb: ~5%+). Sub-noise wins aren't worth less readable code.
7. **Bound the work, don't just speed it up.** The durable fix for canvas-scale paths is
   usually algorithmic — touch only the dirty region — not a faster full-canvas pass.

## Build profiles — the part everyone gets wrong

The workspace already defines the three profiles you need. Use the right one:

| Doing | Profile | Why |
|---|---|---|
| Timing / "is it fast enough" | `release` (`--release`) | full `opt-level = 3`, `lto = "thin"`. The only honest speed number. |
| Profiling (samply, flamegraph) | `release-with-debug` (`--profile release-with-debug`) | release optimizations **plus** debug symbols and no `strip`, so the profiler can name functions instead of showing `??`. |
| Day-to-day editing | `dev` | unoptimized; never quote its timings. |

Profiling a `dev` build inverts the picture — bounds checks and un-inlined calls dominate
a profile that release would erase, so you "fix" code that was never hot. Profiling a
plain stripped `release` build gives accurate timings but anonymous stack frames.
`release-with-debug` exists precisely to give you both; reach for it whenever you profile.

```bash
# honest timing
cargo run --release

# build the profilable binary
cargo build --profile release-with-debug
```

`bacon` runs a background check/clippy loop while you edit — it's for correctness
feedback, not perf numbers. Quote timings from a deliberate `--release` run, not from
whatever `bacon` happened to compile.

## clippy already catches the easy ones

`clippy::perf` is on by default, and the Stop gate runs
`cargo clippy --workspace --all-targets -- -D warnings`. So the chapter's
`cargo clippy -- -D clippy::perf` advice is, in this repo, already enforced on every
session — needless `.clone()`, `.collect()` then re-iterate, `format!` where a `write!`
fits, and friends fail the gate before you profile anything. Don't treat clippy output as
optional perf trivia; it's a wall you've already agreed to. If you want to look
specifically at perf lints while iterating:

```bash
cargo clippy --workspace --all-targets -- -W clippy::perf
```

But understand these are *micro*-lints. They never find the real bottleneck — an
O(n²) over every pixel, a full-canvas pass that should be a dirty-rect pass, a clone of a
multi-megabyte buffer in a loop. Clippy is the floor; the profiler is how you find what
actually costs.

## Benchmarking with criterion

`cargo bench` with stable Rust needs a benchmark harness — use `criterion`. It runs your
code many times, accounts for noise, and reports a confidence interval, so a 3% "win"
that's really measurement jitter shows up as overlapping intervals instead of a false
victory.

Add it as a dev-dependency on the crate whose hot path you're measuring (confirm the
current version on crates.io; `criterion` is `MIT OR Apache-2.0`, so it clears the
`cargo deny` license gate — [[feedback-dep-upgrades]]):

```toml
[dev-dependencies]
criterion = "0.7"

[[bench]]
name = "blend"
harness = false   # required: criterion provides its own main()
```

A benchmark lives in `benches/` and compares the thing you care about at a realistic
size. Wrap inputs and outputs in `std::hint::black_box` so the optimizer can't fold the
work away as dead code (use `std::hint::black_box`, not the deprecated
`criterion::black_box`):

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

Run it, change the code, run it again. criterion remembers the previous run and prints
the delta ("change: -18.3% [...] Performance has improved."). That delta — not a
gut feeling — is what justifies keeping the change. If criterion reports the change as
within noise, revert it: you spent readability and bought nothing. Snapshot/regression
testing conventions live in [[pixhaus-testing-conventions]]; benches are for speed, tests
are for correctness — keep them separate.

## Profiling with a flamegraph

A profiler tells you *where* the time goes so you don't benchmark the wrong function. A
flamegraph is the readable form: the x-axis is total time on the CPU (wider = more time,
either slower per call or called more often), the y-axis is call-stack depth (`main` at
the bottom, leaves on top). Box color is random — ignore it. You're hunting the widest
box near the top: that's the leaf burning the cycles. Thick stack = heavy CPU, thin stack
= cheap.

Two tools. **Prefer `samply`** in this repo — it's cross-platform (it works the same on
the Windows dev machine, macOS, and Linux), needs no `sudo`/`perf` setup, and opens the
Firefox Profiler UI with a flamegraph plus a timeline:

```bash
cargo install samply
cargo build --profile release-with-debug
samply record ./target/release-with-debug/<binary>
```

`cargo flamegraph` produces a static SVG and is fine too, but its backend is
platform-specific (Linux `perf`, macOS `dtrace` with `sudo`, Windows ETW via `blondie`),
which is exactly the cross-platform friction `samply` avoids:

```bash
cargo install flamegraph
cargo flamegraph --profile release-with-debug --bin <binary>
# profile a specific bench or test instead:
cargo flamegraph --profile release-with-debug --bench blend -- --bench
```

Always profile the `release-with-debug` profile (above). A profile of a `dev` build is a
profile of missing optimizations, not of your algorithm.

## What the profile usually tells you in a pixel editor

The bottleneck in a canvas tool is almost always one of a short list, and each maps to a
fix skill — that's why this skill stays workflow-only and delegates the how:

- **A per-pixel loop dominates a wide box.** First ask if it should run over the whole
  canvas at all — bound it to the dirty rectangle ([[8k-perf-constraint]]). If it
  genuinely must touch every pixel (export, a full filter), parallelize it with
  [[pixhaus-rayon]] over scanline chunks.
- **A buffer `clone` or allocation shows up hot.** A `PixelBuffer` is megabytes; cloning
  one per brush move is the cost. See [[pixhaus-rust-conventions]] for `Cow`, `mem::take`,
  borrowing over cloning, and avoiding `Vec<Vec<T>>`.
- **The UI stutters but total CPU is low.** The heavy work is on the egui frame thread.
  Move it to `spawn_blocking` and return the result over a channel the frame loop drains
  — [[pixhaus-tokio]]. The frame thread must never block.
- **Live drawing/compositing is CPU-bound at all.** The live canvas should composite on
  the GPU; CPU compositing is for export and thumbnails. See [[pixhaus-wgpu]].

## Things not to do

- **Don't `#[inline]` on a hunch.** Rust and LTO inline aggressively already. Add
  `#[inline]` only when a benchmark shows it helps across a crate boundary — otherwise
  it's noise that can even hurt by bloating code.
- **Don't micro-tune before profiling.** Hand-unrolling a loop or swapping a `HashMap`
  for a `Vec` before you've confirmed it's the bottleneck is wasted effort and lost
  readability. Profile first.
- **Don't optimize for a canvas size that doesn't hurt.** A 64x64 sprite doesn't need
  rayon; the splitting overhead loses to a plain loop. Match the effort to the size that
  actually stalls.
- **Don't trust a single noisy run.** Background load skews timings. Let criterion's
  intervals decide, or take the best of several `--release` runs.
- **Don't reach for `unsafe` for speed.** The workspace forbids it
  ([[project-v2-native-restart]]). The safe paths above — release profile, rayon, GPU,
  dirty-rect bounding — cover the real wins. If you think you've found a case that
  genuinely needs `unsafe`, escalate to a maintainer rather than adding it.

## When in doubt

- "Is this fast enough?" — Measure it at the size that would hurt (4K/8K). If it's
  imperceptible there, you're done; don't optimize.
- "Should I optimize this?" — Only after a profile or benchmark names it the bottleneck.
  A guess is not a reason.
- "Is my speedup real?" — If criterion can't distinguish it from noise, it isn't. Revert.
- "It's still slow after optimizing the loop." — The fix is probably algorithmic (bound
  to the dirty region) or architectural (GPU, off-thread), not a faster inner loop.
