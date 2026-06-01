---
name: pixhaus-tokio
description: >
  Use when writing, reviewing, or debugging any async / background-task code in the
  Pixhaus binary — running an AI backend request, encoding a PNG, loading or saving a
  project, or any work that must happen off the egui update thread; choosing between
  `tokio::spawn` and `spawn_blocking`; getting a result back to the UI over an mpsc /
  oneshot / watch channel and calling `ctx.request_repaint()`; bounding parallel work
  with a Semaphore; graceful shutdown with a CancellationToken; or reaching for
  `select!` / `join!`. Trigger this for ANY "spawn a task", "run this in the
  background", "this freezes the UI / the window hangs", "the AI request", "send the
  result back to the loop", "where do I create the runtime", "Arc<Mutex> across an
  await", "tokio runtime panics", or "graceful shutdown" work, even when the user
  doesn't say "tokio". The binary owns exactly one Tokio runtime and the egui loop must
  never block; getting that boundary wrong freezes the window or spawns a second
  runtime. For blocking a single self-contained future in a crate with NO runtime (the
  `render` crate's tests/benches), that's pixhaus-pollster, not this.
---

# tokio for Pixhaus

Tokio is the async runtime for the Pixhaus binary: a multi-threaded work-stealing
scheduler plus the channels, timers, and synchronization that move work off the UI
thread and results back onto it. This skill is the floor for async work in Pixhaus —
where the runtime lives, how background work reaches the egui frame, and the handful of
rules that keep the window responsive.

The job tokio does *not* do: it is not a general "make this concurrent" hammer, and it
is not the only executor in the tree. The `render` crate drives wgpu setup with pollster
(see [[pixhaus-pollster]]) because it has no runtime and wants one future blocked to
completion. tokio is the binary's runtime for *many* concurrent tasks that must not
block the frame. Two executors, two non-overlapping jobs — don't nest one in the other.

## Versions and license

| Crate | Version | License |
|---|---|---|
| `tokio` | 1.x (docs verified at 1.52.3) | MIT |
| `tokio-util` | 0.7.x (docs verified at 0.7.18) | MIT |

Both are MIT, so they pass the workspace MIT lock and `cargo deny`. The workspace pins
them in `[workspace.dependencies]`:

```toml
tokio = { version = "1", features = ["full"] }
tokio-util = { version = "0.7", features = ["rt"] }
```

`full` is the pragmatic choice for the binary because it owns the runtime and pulls in
the HTTP/AI client stack that needs the I/O and time drivers. A desktop editor rarely
touches `tokio::net`, `tokio::fs`, `tokio::process`, or `tokio::signal` directly — see
"What not to reach for" below. Don't add `tokio` to `core`, `io`, or `render`: those
crates stay runtime-agnostic, and the binary is the single owner. When you bump tokio,
re-verify against docs.rs — see [[feedback-dep-upgrades]].

## The mental model: one runtime, one UI thread, channels between them

Three facts drive almost every correct decision at this layer.

1. **The binary owns exactly one runtime.** Build it once in `main`, before the eframe
   loop starts, and keep it alive for the program's life. No `#[tokio::main]` (it hides
   the runtime and fights eframe's control of the main thread), no second runtime, no
   `Runtime::new()` deep in a module. Spawning needs the runtime *context*: either you're
   inside a task, or you hold a `Handle` and called `handle.spawn(...)` / entered it with
   `handle.enter()`. A cheap `Handle` clone is what you hand to code that needs to spawn.

2. **The egui update thread is sacred — never block it.** eframe runs `logic` then `ui`
   on one thread each frame (see [[pixhaus-eframe]]). Any `.await` that waits, any
   `block_on`, any lock held while slow work happens, freezes the whole window. Long work
   goes onto the runtime; the UI thread only ever *starts* tasks and *drains* their
   results.

3. **Results cross back over a channel, and a repaint wakes the loop.** A spawned task
   sends its result down a tokio channel. The egui `logic` method drains that channel
   each frame with `try_recv` (never `recv().await` — that would block). Because eframe
   sleeps when idle, the worker also holds a cloned `egui::Context` and calls
   `ctx.request_repaint()` when it has something, so the loop wakes to drain. This is the
   one pattern that connects async work to the immediate-mode frame.

```
  egui update thread                         tokio runtime
  ------------------                         -------------
  ui: user clicks "Generate"
    handle.spawn(async { ... })  ───────────▶  task runs (HTTP, decode, ...)
                                                  tx.send(result)
                                                  ctx.request_repaint()  ◀── wakes loop
  logic (next frame):
    while let Ok(r) = rx.try_recv() { apply(r) }
```

## The canonical pattern: spawn, channel, drain, repaint

```rust
use tokio::sync::mpsc;

struct Pixhaus {
    rt: tokio::runtime::Handle,        // a clone of the binary's runtime handle
    results_tx: mpsc::Sender<JobResult>,
    results_rx: mpsc::Receiver<JobResult>,
    // ... document, tools, etc.
}

impl Pixhaus {
    // called from `ui` when the user kicks off background work
    fn start_generate(&self, ctx: &egui::Context, prompt: String) {
        let tx = self.results_tx.clone();
        let ctx = ctx.clone();              // Context is cheap to clone; it wakes the loop
        self.rt.spawn(async move {
            let outcome = run_ai_request(prompt).await;   // async I/O, e.g. reqwest
            // ignore send error: it only fails if the UI dropped the receiver (app closing)
            let _ = tx.send(JobResult::Generated(outcome)).await;
            ctx.request_repaint();
        });
    }

    // called every frame from eframe `App::logic` — drains without blocking
    fn drain_jobs(&mut self) -> bool {
        let mut got_any = false;
        while let Ok(result) = self.results_rx.try_recv() {
            self.apply(result);
            got_any = true;
        }
        got_any
    }
}
```

`try_recv` returns `Err(TryRecvError::Empty)` when nothing is waiting — that's the normal
case, not an error to log. Build the channel once when constructing the app; the UI owns
the single `Receiver`, and each spawn point clones a `Sender`.

## spawn vs spawn_blocking — the decision that bites most

This is the call agents get wrong. The rule follows from what each thread pool is for.

- **`tokio::spawn`** runs a *future* on a worker thread. Use it for genuinely async work:
  HTTP to an AI backend (reqwest), awaiting a channel, anything built from `.await`s. The
  future and its output must be `Send + 'static`. A future that does CPU-bound work
  *between* awaits still hogs a worker thread — don't put image processing here.

- **`spawn_blocking`** runs a *closure* on tokio's separate blocking pool (default up to
  512 threads). Use it for synchronous work that would otherwise stall a worker:
  CPU-bound pixel/image processing, PNG/zstd encode and decode, `std::fs` reads and
  writes, calling a sync library. The closure is `FnOnce() -> R + Send + 'static`.

```rust
// async I/O: spawn
self.rt.spawn(async move { let bytes = reqwest_get(url).await?; /* ... */ });

// CPU-bound / sync blocking: spawn_blocking
self.rt.spawn_blocking(move || {
    let png = encode_png(&pixels)?;     // pure CPU, no .await
    std::fs::write(&path, &png)         // sync std::fs, not tokio::fs
});
```

Mixing them up is the classic mistake: encoding a 4K PNG inside `tokio::spawn` blocks a
scheduler worker for the whole encode and starves other async tasks; calling
`reqwest`'s async API inside `spawn_blocking` wastes a blocking thread parked on a future
it can't drive. Match the tool to the work.

**Bound CPU fan-out with a Semaphore.** The 8K perf constraint ([[project_8k_perf_constraint]])
means a large operation can fan out into many region jobs. Spawning hundreds of
`spawn_blocking` closures at once saturates the blocking pool and the CPU. Gate them with
a `tokio::sync::Semaphore` sized to the core count so only N run at once. Details in
`references/sync-and-channels.md`.

## Locks: prefer parking_lot, never hold one across an await

Pixhaus uses `parking_lot` for synchronous locks, and CLAUDE.md is explicit: never hold a
lock across `.await`. The reasoning is concrete — a `parking_lot`/`std` guard is `!Send`,
so holding it across an await makes the future `!Send` and `tokio::spawn` rejects it; even
where it compiles, you'd be holding a lock while the task is parked, which serializes the
runtime and risks deadlock.

- **Short, non-async critical section** (mutate a shared counter, read a flag): use
  `parking_lot::Mutex` / `RwLock` (see [[pixhaus-parking-lot]]). Acquire, touch the data,
  drop the guard — all between awaits, never across one.
- **Must hold state locked across an `.await`**: that, and only that, is what
  `tokio::sync::Mutex` is for. Its guard is `Send`, so the future stays `Send`. It's
  slower than `parking_lot`, so reach for it only when the await-in-critical-section is
  unavoidable — usually it isn't, and restructuring to drop the lock first is better.
- **Most state isn't shared at all.** The document lives on the UI thread and is owned
  directly (see CLAUDE.md memory rules). Don't wrap it in `Arc<Mutex<>>` to "share with a
  task" — copy the slice the task needs into the spawned closure, or send it over a
  channel. Single ownership beats a lock.

See [[pixhaus-rust-conventions]] for the locks-and-async rules and the no-`unwrap` rule
that applies to every `Result` a task returns.

## Graceful shutdown

A pixel editor with in-flight AI requests and a save in progress needs to wind tasks down
on exit, not abort mid-write. Use a `tokio_util::sync::CancellationToken`: hold a root
token in the app, hand clones (or `child_token()`s) to long-lived tasks, and have each
task `select!` between its real work and `token.cancelled()`. On close, call
`token.cancel()` once and every task's cancel branch fires. Pair this with eframe's
close-confirmation flow ([[pixhaus-eframe]]). The token and the `select!` semantics are in
`references/time-and-coordination.md`.

## Rules that prevent the recurring bugs

- **Dropping a `JoinHandle` does not cancel the task — it detaches it.** The task keeps
  running in the background; you just lose the result. To actually stop on drop, call
  `abort()`, or use a `JoinSet` (which aborts its tasks on drop). Fire-and-forget UI
  triggers can drop the handle deliberately; just know that's what's happening.
- **`spawn_blocking` tasks can't be aborted once started**, and they block runtime
  shutdown until they finish. Keep them bounded; for a truly long-lived loop use
  `std::thread::spawn` instead of permanently occupying a pool thread.
- **`try_recv` in `logic`, never `recv().await`.** Awaiting a channel on the UI thread
  freezes the frame. Drain in a `while let Ok(..)` loop and return whether you got
  anything so `logic` can `request_repaint` if more may be coming.
- **A send failing is the app closing, not a bug.** `tx.send(..)` errors only when the
  receiver dropped. On a worker that usually means the window closed; `let _ =` it rather
  than unwrapping.
- **`select!` drops the losing branches.** Only put cancel-safe futures in a `select!`
  loop. Channel `recv`, `Interval::tick`, and `CancellationToken::cancelled` are
  cancel-safe; `Mutex::lock`, `Semaphore::acquire`, and the buffered `AsyncRead`/`Write`
  helpers are not. See the not-cancel-safe list in `references/time-and-coordination.md`.
- **No nested runtimes.** `Runtime::block_on` and `Handle::block_on` panic inside an async
  context, and `block_in_place` panics on a current-thread runtime. If you think you need
  to block on a future, you're either on the UI thread (spawn + channel instead) or in a
  no-runtime crate (pollster instead).

## What not to reach for

- **`tokio::fs` for the editor's own files.** Pixhaus's file work is bursty and
  CPU-adjacent (serialize, compress, write). Use `spawn_blocking` with `std::fs` —
  simpler, and it doesn't pretend disk I/O is cheaply async. See [[pixhaus-zstd]] and
  [[pixhaus-rmp-serde]] for the encode side.
- **`tokio::net` directly.** AI backends go through an HTTP client (reqwest), which owns
  its own tokio-based networking. You spawn the request future; you don't open sockets.
- **`#[tokio::main]` / `#[tokio::test]` in the binary.** The runtime is built explicitly
  in `main`. For async unit tests in a crate that legitimately uses tokio, `#[tokio::test]`
  is fine; for the `render` crate's runtime-free tests, use `#[pollster::test]`.

## References

Open the file for the area you're working in. Each is a dense tokio 1.x reference verified
against docs.rs.

| File | Covers |
|---|---|
| `references/runtime-and-tasks.md` | `Runtime` / `Builder` / `Handle`, building the one runtime in `main`, `tokio::spawn`, `spawn_blocking`, `block_in_place`, `JoinHandle` (abort vs detach), `JoinSet`, `LocalSet` / `spawn_local`, `yield_now`, the feature flags |
| `references/sync-and-channels.md` | the channel-choice matrix, `mpsc` (bounded + unbounded), `oneshot`, `watch`, `broadcast`, `tokio::sync::Mutex` / `RwLock` vs parking_lot, `Semaphore` for bounded fan-out, `Notify` |
| `references/time-and-coordination.md` | `sleep` / `interval` / `MissedTickBehavior` / `timeout`, `select!` and its cancellation-safety rules, `join!` / `try_join!`, `CancellationToken` and the graceful-shutdown pattern |

A standing caution: signatures were verified at tokio 1.52.3 / tokio-util 0.7.18. The 1.x
API is stable, but when a deep signature is load-bearing, confirm it against
https://docs.rs/tokio/latest/ before depending on it.
