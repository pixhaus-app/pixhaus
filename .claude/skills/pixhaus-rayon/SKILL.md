---
name: pixhaus-rayon
description: >
  Use when parallelizing CPU-bound work across cores in Pixhaus with the `rayon`
  crate — above all per-pixel and per-scanline ops on the big RGBA `Vec<u8>` buffers
  (blend, fill, filters, transforms, color reduction), plus histogram/palette builds,
  sprite-sheet slicing, and any "this loop is too slow on a 4K/8K canvas" hot path.
  Trigger this for ANY "parallelize this loop", "run this across all cores", "use a
  thread pool for the pixels", "speed up the blend / filter / composite", "par_iter /
  par_chunks_mut / par_sort", "split this work with join or scope", "fold vs reduce",
  "why doesn't .par_iter() exist" (missing prelude), "this isn't Send/Sync", or
  "it deadlocks / stalls the UI when I parallelize" request, even when the user never
  says "rayon". rayon is the data-parallelism layer; its closures run on pool threads
  (so Send/Sync bounds bite), `fold` is an adaptor not a terminal, and a parallel job
  run on the egui frame thread or a tokio worker stalls it — reach for this skill to
  get the boundary right rather than guessing. rayon is CPU parallelism; for async I/O
  (the AI backends) that is tokio's job, not rayon's.
---

# rayon for Pixhaus

rayon is the data-parallelism layer: it spreads CPU-bound work — the per-pixel and
per-scanline loops that dominate a 4K/8K canvas — across a work-stealing thread pool
with almost no ceremony. Take a sequential `iter()`/`iter_mut()` chain, swap in
`par_iter()`/`par_iter_mut()`, and rayon splits the work across cores; if no cores are
idle it just runs sequentially, so the overhead is small. It is the tool for the
[[8k-perf-constraint]] hot paths in `core` and `render`.

This skill is the floor for parallel work in Pixhaus: the handful of facts that prevent
the recurring bugs (the prelude requirement, `fold` vs `reduce`, Send/Sync on pool
threads, never blocking the frame), the version pin, and how the API maps onto a
pixel-art editor. When you need the full method surface for an area, open the matching
file in `references/` — don't guess signatures from memory; rayon's iterator traits are
broad and the `_with`/`_init`/`_first`/`_any`/`fold`/`reduce` families are easy to
confuse.

rayon owns a thread pool; tokio owns the async runtime. Keep them separate: rayon for
CPU data parallelism (pixels), tokio for async I/O (the AI backends — see
[[pixhaus-tokio]]). They are not interchangeable, and mixing them carelessly stalls one
or the other (see the rules below).

## Version and license — pin these

```toml
rayon = "1.12"
```

| Crate | Version | License | cargo deny |
|---|---|---|---|
| `rayon` | 1.12.0 | `MIT OR Apache-2.0` | passes the MIT lock |
| `rayon-core` | 1.13.0 | `MIT OR Apache-2.0` | passes |

- `rayon` is the iterator/algorithm layer; `rayon-core` is the runtime (the thread pool,
  `join`, `scope`, `ThreadPoolBuilder`, `spawn`). The core types are re-exported at the
  `rayon` root, so you depend only on `rayon` and write `rayon::join`, `rayon::scope`,
  `rayon::ThreadPoolBuilder`. Don't add `rayon-core` directly.
- No feature flags you need here. MSRV is 1.80.0 — far below the workspace toolchain.
- License clears the [[project-v2-native-restart]] MIT lock. rayon uses `unsafe`
  internally, but its public API is safe — the workspace `unsafe` ban applies to *our*
  crates, not to dependencies, so rayon is fine to use under `#![forbid(unsafe_code)]`.
- When you bump rayon, re-verify the references against docs.rs — see
  [[feedback-dep-upgrades]].

## The mental model: five facts that cause most bugs

1. **Nothing works without the prelude.** `par_iter`, `par_iter_mut`, `into_par_iter`,
   `par_chunks_mut`, `par_sort`, `par_bridge` are all *trait* methods. `use
   rayon::prelude::*;` brings the thirteen extension traits into scope. Without it the
   methods simply don't exist and you get a confusing "no method named `par_iter`"
   error on a `Vec` that obviously has one. The prelude is the first line of any module
   that touches rayon.

2. **`fold` is an adaptor, not a terminal — `reduce` is the terminal.** `reduce(id, op)`
   runs the whole computation and returns one value; its accumulator and items are the
   same type. `fold(id, op)` returns *another parallel iterator* of per-job partial
   accumulators (and its accumulator type `T` may differ from the item type) — it does
   not finish anything. The pattern is `.fold(id, op).reduce(id, combine)` (or
   `.fold(...).sum()`): fold collapses each job's slice into a partial, reduce combines
   the partials. Reaching for `fold` and expecting a value is the single most common
   rayon mistake. See `references/parallel-iterators.md`.

3. **Closures run on pool threads, so captures must be `Send`/`Sync`.** Everything you
   hand to a parallel iterator or `join`/`scope` executes on worker threads. `Rc`,
   `Cell`, `RefCell` are not `Send`/`Sync` and will not compile — share immutable state
   with `Arc`, mutable state with atomics or a lock (`parking_lot`, see
   [[pixhaus-parking-lot]]). For per-job scratch state that isn't shared, use `map_with`
   / `map_init` / `for_each_init` instead of a shared `Mutex` — they give each job its
   own cloned or freshly-constructed value, which is faster and lock-free.

4. **A parallel job blocks the thread that drives it — keep it off the egui frame thread
   and off tokio workers.** `par_iter().collect()`, `par_sort()`, `join`, and `scope`
   all block the calling thread until the work finishes. Call one directly in the egui
   `update`/`ui` loop and you stall the frame; call one inside an async task and you
   block a tokio worker (and risk starving the runtime). The right shape: a heavy verb
   runs on a background thread (a dedicated `std::thread` or tokio `spawn_blocking`) that
   uses rayon *internally*, and posts the result back over a channel the egui loop drains
   each frame — exactly the pattern CLAUDE.md prescribes for background work. Don't hold a
   lock across a rayon call any more than across an `.await`.

5. **Parallelism has fixed overhead — small work loses.** Splitting and stealing cost
   cycles. On a tiny dirty region, a sequential loop beats `par_iter`. Bound rayon work
   by the dirty rectangle, not the whole canvas ([[8k-perf-constraint]]), and use
   `with_min_len(n)` to stop rayon splitting below a payload that's worth a job. Measure
   before parallelizing a path you assume is hot.

## Rules that prevent the recurring bugs

- **Parallelize over the dirty region, never the whole buffer.** For per-pixel work use
  `buffer.par_chunks_mut(width * 4)` to hand each worker a band of RGBA scanlines, and
  slice to the dirty rows first. A brush dab on an 8K canvas should fan out over the few
  affected rows, not 8192 of them. `references/slices-and-strings.md` has the chunk APIs.
- **You cannot box a parallel iterator.** `ParallelIterator` is not dyn-compatible, so
  `Box<dyn ParallelIterator>` does not exist. Return `impl ParallelIterator<Item = …>`
  or stay generic — which aligns with the [[pixhaus-rust-conventions]] "no `Box<dyn>`
  when generics fit" rule anyway.
- **Prefer the unstable sorts.** `par_sort_unstable*` sorts in place with no allocation
  and is generally faster; `par_sort*` (stable) allocates a scratch buffer the size of
  the slice. Use stable only when equal elements must keep their order.
- **Pick the deterministic combinator when output must be reproducible.** A `.pixhaus`
  file or an exported image must be byte-identical for the same input. `reduce`,
  `find_first`, `position_first`, and indexed `collect` are order-deterministic;
  `find_any`, `reduce_with` on a non-associative op, and `fold`'s partial ordering are
  not. For floating-point sums, remember addition isn't associative — parallel order can
  shift the last bits. Choose the ordered variant when reproducibility matters; choose
  `_any` when you genuinely don't care, because it short-circuits hardest.
- **Initialize the global pool at most once, early.** `ThreadPoolBuilder::build_global()`
  succeeds only on the first call; a second call errors. Set it up at startup (or rely on
  the default, which uses the CPU count and honors `RAYON_NUM_THREADS`). For an isolated
  pool — to cap threads for one subsystem — build a `ThreadPool` and run work through
  `pool.install(...)`. See `references/thread-pools.md`.
- **`par_bridge` is the last resort, not the default.** It parallelizes an arbitrary
  sequential `Iterator`, but pulls items one at a time under a lock and yields them in
  nondeterministic order — slower than a native parallel source. Use it only when the
  source is inherently sequential (a channel, a file/network reader); when the data is
  already a `Vec`/slice/range, use `par_iter`/`into_par_iter` instead.

## Pixhaus applications

Where rayon lands in a native pixel-art editor on `egui` + `wgpu`:

- **Per-scanline pixel ops in `core`.** Fill, blend, tint, threshold, and other
  whole-pixel passes run as `dirty_rows.par_chunks_mut(width * 4).for_each(|row| …)` over
  the RGBA `Vec<u8>`. This is the bread-and-butter use and the reason rayon is in the
  tree. CPU-side compositing for export and thumbnails; the live canvas composites on the
  GPU (see [[pixhaus-wgpu]]).
- **Layer composite bounded to a rect.** Folding N layers into one output is
  `layers.par_iter()` paired with a per-pixel blend, or `fold`+`reduce` when accumulating.
  Bound to the dirty rectangle, not the document.
- **Histograms and palette work.** Building a color histogram is the textbook
  `fold(HashMap::new, accumulate).reduce(HashMap::new, merge)` — per-job maps merged at
  the end, no shared lock. Sorting a palette is `par_sort_unstable_by_key`. Pairs with
  [[pixhaus-color-quant]] for the quantizer.
- **Bulk import / sprite-sheet slicing in `io`.** Slicing a sheet into frames or decoding
  many cels is `frames.into_par_iter().map(decode).collect()`. Keep it off the egui
  thread (rule 4).
- **Separable filters (blur, sharpen).** Run the horizontal pass with
  `par_chunks_mut` over rows, then the vertical pass over columns (transpose or
  column-chunk). Use `map_init` to give each job a reusable scratch line buffer instead
  of allocating per pixel.
- **Custom recursive work via `join`/`scope`.** Most needs are met by parallel iterators;
  drop to `join` for divide-and-conquer (a quad-tree flood fill, a recursive region
  split) and `scope` when a loop spawns a dynamic number of tasks. See
  `references/task-parallelism.md`.

## References

Open the file for the area you're working in; each is a dense API reference for rayon
1.12.0, with signatures checked against docs.rs.

| File | Covers |
|---|---|
| `references/parallel-iterators.md` | `ParallelIterator` + `IndexedParallelIterator` — every adaptor and consumer with signatures; the `fold`-vs-`reduce` split; `find_first`/`_any`/`_last` and `position_*` ordering; `map_with`/`map_init`/`for_each_init` per-job state; granularity (`with_min_len`/`with_max_len`/`chunks`/blocks) |
| `references/sources-and-collecting.md` | Getting in and out: `IntoParallelIterator`/`par_iter`/`par_iter_mut`, the prelude's thirteen traits, `collect`/`FromParallelIterator`/`ParallelExtend`/`unzip`/`partition`, `par_bridge`, `par_drain`, source fns (`split`/`empty`/`once`/`repeat`/`repeat_n`/`walk_tree*`), and which std types parallelize (incl. the range `u64`/`i64`/`u128`/`i128` `zip` caveat) |
| `references/task-parallelism.md` | The fork-join primitives: `join`/`join_context`, `scope`/`scope_fifo`/`in_place_scope`, `spawn`/`spawn_fifo`, `broadcast`/`spawn_broadcast`; the work-stealing model; FIFO vs LIFO; `FnContext`, `Yield`, `yield_now`/`yield_local`; panic propagation |
| `references/thread-pools.md` | `ThreadPoolBuilder` (every method) and `ThreadPool` (every method); `build`/`build_global`/`build_scoped`; `install`; `num_threads` and `RAYON_NUM_THREADS`; `current_num_threads`/`current_thread_index`/`max_num_threads`; custom spawn/start/exit/panic handlers |
| `references/slices-and-strings.md` | `ParallelSlice`/`ParallelSliceMut` — `par_chunks`/`par_chunks_exact`/`par_rchunks`/`par_windows`/`par_chunk_by` (the pixel-band APIs) and the full `par_sort*` family with allocation behavior; `ParallelString` — `par_lines`/`par_split`/`par_chars`/etc. |
