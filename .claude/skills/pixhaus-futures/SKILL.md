---
name: pixhaus-futures
description: >
  Use when composing, transforming, or combining async values in Pixhaus with the
  `futures` crate — the combinator toolkit on top of `std::future`. Covers the
  extension traits (`FutureExt`, `TryFutureExt`, `StreamExt`, `TryStreamExt`,
  `SinkExt`), the `Stream` and `Sink` traits, the `join!` / `try_join!` / `select!`
  / `select_biased!` / `pin_mut!` macros, bounded-concurrency adapters
  (`buffer_unordered`, `for_each_concurrent`, `FuturesUnordered`/`FuturesOrdered`),
  the `futures::channel` mpsc/oneshot channels, cancellation (`Abortable`,
  `select!`), and the no-unsafe waker path (`ArcWake`, `AtomicWaker`). Trigger this
  for ANY "run these futures concurrently", "race two futures / first one wins",
  "process this stream", "map/filter/buffer an async sequence", "limit how many
  run at once", "cancel this future", "send the result back to the UI loop", "build
  a Stream", "next() on a stream", "join!/select! won't compile", "the future isn't
  Unpin / FusedFuture", or "build a Waker without unsafe" task, even when the user
  doesn't say "futures". The load-bearing fact: `futures` is a toolkit, NOT a
  runtime — Pixhaus runs futures on tokio and bridges sync with pollster, so use
  `futures` for its combinators, traits, and channels, but reach for the
  `pixhaus-tokio` / `pixhaus-pollster` skills to spawn, run, do I/O, time, or lock.
---

# futures for Pixhaus

`futures` is the combinator layer that sits on top of the standard `Future` trait:
the extension methods (`FutureExt`, `StreamExt`, ...), the `Stream` and `Sink`
traits that `std` does not define, the concurrency macros (`join!`, `select!`), and
a handful of primitives (channels, `Abortable`, `ArcWake`). It is the de-facto
vocabulary the whole Rust async ecosystem shares — tokio's own types implement
`futures`' `Stream`/`Sink` traits and work with its adapters.

It is not a runtime. That distinction is the spine of this skill. Pixhaus already
owns one runtime — tokio, in the binary — and bridges sync→async with pollster in
the render crate. So you use `futures` for *composition* (combine, transform, race,
bound concurrency) and lean on the `pixhaus-tokio` and `pixhaus-pollster` skills for
*execution* (spawn, run, time, I/O, most locks). Mixing those up is the recurring
mistake: introducing `futures::executor::ThreadPool` as a second runtime, or
`futures::io` traits that don't talk to tokio.

## Version, features, license

```toml
# Recommended: drop the bundled executor — Pixhaus runs on tokio + pollster.
futures = { version = "0.3", default-features = false, features = ["std", "async-await"] }
```

| Crate | Version | MSRV | License |
|---|---|---|---|
| `futures` | 0.3.32 | 1.56 (verify) | `MIT OR Apache-2.0` |

The dual license includes MIT, so `futures` clears the [[project-v2-native-restart]]
MIT lock and `cargo deny`. Feature notes:

- Default features are `std`, `async-await`, `executor`. The `executor` feature
  pulls in `futures::executor` (`block_on`, `LocalPool`, `ThreadPool`) — a second
  runtime. Drop it (`default-features = false`) and keep `std` + `async-await`.
  `std` already enables `alloc`, the `futures-io` traits, `channel`, and `sink`;
  `async-await` enables the `join!` / `select!` macros.
- `boxed` / `BoxFuture` / `BoxStream` need an allocator — covered by `std` (or
  `alloc` on no-std). Pixhaus is desktop-only, so `std` is always on.
- Add `"bilock"` only if you actually use `futures::lock::BiLock`. Skip `compat`
  (futures 0.1 interop) and `thread-pool` (second runtime).

When you bump `futures`, re-verify the reference files against docs.rs — see
[[feedback-dep-upgrades]].

## The mental model: toolkit, not runtime

Three facts decide almost every call:

1. **`futures` composes; tokio runs.** A future or stream built from `futures`
   adapters is inert until something polls it. In Pixhaus that "something" is a
   tokio task (`tokio::spawn`) or, in render-crate tests/benches, `pollster::block_on`.
   `futures` gives you the verbs (`.map()`, `.buffer_unordered()`, `join!`); the
   runtime supplies the engine. Never reach for `futures::executor` to run them —
   that is a parallel runtime fighting the one you own.

2. **Extension methods need their trait in scope.** `.next()`, `.map()`,
   `.try_next()`, `.send()`, `.boxed()` live on `StreamExt` / `FutureExt` /
   `TryStreamExt` / `TryFutureExt` / `SinkExt`, not on the base traits. A "method
   not found" on a stream or future almost always means a missing
   `use futures::StreamExt;` (or `prelude::*`). The base `Stream`/`Sink` traits give
   you only `poll_next` / `poll_ready`; the ergonomic surface is on the `*Ext`
   traits.

3. **`select!` and `next()` impose `Unpin` + `FusedFuture`/`FusedStream`.** `select!`
   polls its branches repeatedly, so each must be `Unpin` (pin it with `pin_mut!` or
   build it `Box`ed) and `FusedFuture`/`FusedStream` so a completed branch isn't
   polled again (`.fuse()` on futures, `select_next_some()` or a `Fuse` stream on
   streams). `StreamExt::next()` borrows `&mut self`, so the stream must be `Unpin`
   or pinned. Most "doesn't implement Unpin / FusedFuture" errors trace back here.

## Rules that prevent the recurring bugs

- **Bound concurrency explicitly.** Spawning an unbounded number of in-flight
  futures (a naive `join_all` over thousands of tiles, or an unbounded
  `FuturesUnordered`) blows memory and starves the runtime. Use
  `stream::iter(work).buffer_unordered(N)` or `for_each_concurrent(N, ...)` with a
  concrete limit `N`, or feed a `FuturesUnordered` while capping its length. The
  [[8k-perf-constraint]] makes this concrete: tiling an 8K canvas yields thousands
  of work items, so the limit is not optional. See `references/streams.md`.
- **`buffered` keeps order; `buffer_unordered` does not.** `buffered(N)` yields
  results in input order (back-pressure on the slowest); `buffer_unordered(N)`
  yields each as it finishes. Pick `buffer_unordered` unless downstream order
  matters — for independent tile/layer work it is faster and the default choice.
- **Don't block the egui thread, and don't run `futures` on it.** The update loop
  is single-threaded and owns the document. `block_on` (pollster's or anyone's) on
  that thread freezes the frame. Background async work runs on a tokio task and
  returns over a channel the loop drains each frame, then `ctx.request_repaint()`.
  This is the same rule the `pixhaus-egui` and `pixhaus-pollster` skills state — the
  `futures` channels are one way to build that return path (see below).
- **Prefer tokio's channels and locks when you're already on tokio.** Use
  `futures::channel` when the consumer is a non-tokio poll loop or generic `Stream`
  consumer; otherwise `tokio::sync` is simpler and is the runtime you own. Same for
  locks: `futures::lock::Mutex` is an async mutex for runtime-agnostic library code
  only — short critical sections want `parking_lot` (see `pixhaus-parking-lot`), and
  guards held across `.await` want `tokio::sync::Mutex`. Never hold any guard across
  `.await` (workspace rule in `pixhaus-rust-conventions`).
- **Cancel with `Abortable` or `select!`, not by leaking the task.** Dropping a
  future stops it, but a spawned tokio task keeps running. To cancel in-flight work
  (a superseded AI request, a stale preview render), wrap it in
  `future::abortable` and keep the `AbortHandle`, or race it against a cancel signal
  with `select!`. See `references/future-combinators.md`.
- **Build wakers with `ArcWake`, never `RawWaker`.** The workspace forbids `unsafe`,
  and `RawWaker` requires it. `futures::task::ArcWake` + `waker(Arc::new(..))` is the
  sanctioned path; `AtomicWaker` is the primitive for "many producers wake one
  consumer" — exactly the shape of a custom egui-loop waker. See
  `references/sink-task-and-utilities.md`.
- **Propagate errors; `unwrap`/`expect` only in tests.** `TryFutureExt` /
  `TryStreamExt` (`map_err`, `try_for_each`, `try_collect`) thread `Result` through a
  pipeline — use them with `?` and a `thiserror` variant in library code rather than
  unwrapping mid-stream. See `pixhaus-thiserror` and `pixhaus-rust-conventions`.

## Pixhaus applications

Where `futures` lands in a native pixel-art editor on tokio:

- **Fan out per-tile / per-layer work with bounded concurrency.** An export,
  filter, or batch op over many tiles is `stream::iter(tiles).map(process).buffer_unordered(N)`
  collected on a tokio task. `N` caps memory and CPU; results stream back as they
  finish.
- **Race the AI backends or apply a timeout.** `select!` (or `future::select`) takes
  the first backend to answer and aborts the rest; `select!` against a
  `tokio::time::sleep` gives a timeout. The multi-backend adapter dispatch behind
  `Arc<dyn Backend>` is in the `pixhaus-async-trait` skill; `futures` supplies the
  racing and cancellation around it.
- **Return one result to the UI loop with `oneshot`.** "Spawn a task, get one value
  back" is a `futures::channel::oneshot` (or `tokio::sync::oneshot`): the tokio task
  sends, the egui loop polls/`try_recv`s the receiver each frame. For a stream of
  progress updates, an `mpsc` receiver is a `Stream` you drain per frame. See
  `references/channels.md`.
- **Collect many concurrent results as they complete.** `FuturesUnordered` drives a
  set of futures and yields outputs in completion order without a runtime of its
  own — good for "kick off N independent loads, react to each as it lands."
- **Adapt a callback or polled source into a `Stream`.** `stream::unfold` /
  `poll_fn` build a custom `Stream` from state plus an async step — e.g. wrapping a
  paginated backend or a frame-by-frame generator.

## References

Open the file for the area you're working in; each is a dense API reference for
`futures` 0.3.32, with load-bearing signatures checked against docs.rs.

| File | Covers |
|---|---|
| `references/future-combinators.md` | `future` module — `FutureExt` / `TryFutureExt` adapters, the constructors (`ready`/`ok`/`err`/`pending`/`poll_fn`/`lazy`/`join_all`/`try_join_all`/`select`/`select_all`), the types (`BoxFuture`, `Either`, `Shared`, `Abortable`/`AbortHandle`, `RemoteHandle`, `MaybeDone`, `OptionFuture`), and the macros (`join!`, `try_join!`, `select!`, `select_biased!`, `poll!`, `pending!`, `ready!`, `pin_mut!`) with the Unpin+FusedFuture rules |
| `references/streams.md` | `Stream`/`FusedStream` traits, `StreamExt` / `TryStreamExt` adapters, the `buffered` vs `buffer_unordered` vs `for_each_concurrent` vs `FuturesUnordered` concurrency comparison, `FuturesOrdered`, `select_all` / `stream_select!`, the constructors (`iter`/`once`/`unfold`/`poll_fn`/...), `BoxStream` |
| `references/channels.md` | `oneshot` (single value, `Canceled` on drop) and `mpsc` bounded + unbounded (`Sender`/`Receiver`/`Unbounded*`, `SendError`/`TrySendError`), receivers as `Stream`, and the `futures::channel` vs `tokio::sync` decision for the egui drain loop |
| `references/sink-task-and-utilities.md` | `Sink`/`SinkExt`; `task` (`ArcWake`, `AtomicWaker`, noop wakers, `Spawn`/`SpawnExt`, `FutureObj`); `lock` (async `Mutex`, `BiLock`); `executor` and `io` (both with bold "use tokio/pollster instead" guidance); `pin_mut!` |

A standing caution: the references record the 0.3.32 API faithfully, but a few deep
signatures were flagged during research as unverifiable from the rendered docs
(noted inline as "(verify)"). When one is load-bearing for what you're building,
confirm it against https://docs.rs/futures/0.3.32/futures/ before depending on it.

## Decision shortcut

```
Working with an async value or sequence in Pixhaus?
├─ Need to RUN it (spawn, block, time, sleep)?
│    └─ Not futures. tokio task / pollster::block_on. See pixhaus-tokio / pixhaus-pollster.
├─ Need file/socket I/O?
│    └─ tokio::io, not futures::io (incompatible trait families). Bridge via tokio_util::compat.
├─ Need a lock?
│    └─ Short section → parking_lot. Across .await → tokio::sync::Mutex. futures::lock only for runtime-agnostic libs.
├─ Composing/transforming (map, filter, race, join, bound concurrency)?
│    └─ futures combinators: FutureExt/StreamExt/TryStreamExt, join!/select!, buffer_unordered.
├─ Sending results between a task and the egui loop?
│    └─ oneshot for one value, mpsc (a Stream) for many. tokio::sync if both ends are on tokio.
└─ Building a Waker?
     └─ ArcWake / AtomicWaker (no-unsafe). Never RawWaker.
```
