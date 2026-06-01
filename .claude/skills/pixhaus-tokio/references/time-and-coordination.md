# Time and coordination

Verified against tokio 1.52.3 and tokio-util 0.7.18. This file covers timers, the
`select!` / `join!` macros, and the graceful-shutdown pattern. For spawning see
`runtime-and-tasks.md`; for channels and locks see `sync-and-channels.md`.

## Contents

- `sleep` / `sleep_until`
- `interval` and `MissedTickBehavior`
- `timeout`
- `select!` and cancellation safety
- `join!` / `try_join!`
- `CancellationToken` and graceful shutdown

All `tokio::time` APIs need the `time` feature and a time driver in the runtime
(`enable_time()` / `enable_all()` — Pixhaus uses `enable_all`). `tokio::time::Instant` is
tokio's own monotonic instant, not `std::time::Instant`.

## `sleep` / `sleep_until`

```rust
pub fn sleep(duration: Duration) -> Sleep            // Sleep is a Future resolving to ()
pub fn sleep_until(deadline: Instant) -> Sleep
```

- `sleep(d)` == `sleep_until(Instant::now() + d)`. Cancel by dropping the future.
- Panic trap: a timer must first be polled inside the runtime. `rt.block_on(sleep(d))`
  panics, but `rt.block_on(async { sleep(d).await })` is fine — wrapping in `async` makes
  the future lazy so it's first polled on the runtime. You rarely write this in Pixhaus
  (you `.await` sleeps inside spawned tasks), but it explains the occasional "no timer
  running" panic.
- Timer granularity is platform-specific; Windows timers can be coarser than 1 ms. Don't
  build precise frame timing on `sleep` — egui's repaint scheduling drives the frame.

## `interval` and `MissedTickBehavior`

```rust
pub fn interval(period: Duration) -> Interval         // panics if period == 0
pub fn interval_at(start: Instant, period: Duration) -> Interval
// Interval:
pub async fn tick(&mut self) -> Instant               // cancel-safe
pub fn set_missed_tick_behavior(&mut self, behavior: MissedTickBehavior)
pub fn reset(&mut self)
```

- **The first `tick().await` completes immediately**; subsequent ticks wait `period`.
  Account for that if you don't want work to fire at t=0.
- `tick()` is **cancel-safe** — if it loses a `select!` race, no tick is consumed. That's
  what makes `interval` safe to drive from a `select!` loop (e.g. a periodic autosave or a
  progress poll that also watches a cancellation token).
- `MissedTickBehavior` (matters only when a tick is delayed past its slot):
  - **`Burst` (default)** — fire rapidly to catch up to the original schedule.
  - **`Delay`** — wait a full `period` from when the tick actually fired; schedule drifts.
  - **`Skip`** — drop missed ticks, realign to the next multiple of `period`.
  For a UI-adjacent periodic task, `Skip` or `Delay` avoids a catch-up burst after the app
  was busy or asleep.

## `timeout`

```rust
pub fn timeout<F: IntoFuture>(duration: Duration, future: F) -> Timeout<F::IntoFuture>
pub fn timeout_at<F: IntoFuture>(deadline: Instant, future: F) -> Timeout<F::IntoFuture>
// Timeout<T> yields Result<T, Elapsed>
```

- `Ok(value)` if the inner future finished in time, `Err(Elapsed)` if it timed out (the
  inner future is then dropped). Wrap AI requests and any network call in a `timeout` so a
  hung backend can't leave the UI showing "generating" forever.
- Trap: the deadline is checked *before* polling the inner future, so CPU-bound work that
  never yields can run past the deadline and still return `Ok`. `timeout` bounds *awaiting*,
  not *computing* — bound compute with chunking or by running it under `spawn_blocking`
  with its own guard.

## `select!` and cancellation safety

Needs the `macros` feature.

```rust
tokio::select! {
    biased;                                  // optional: poll top-to-bottom instead of random
    res = some_future => { /* handler */ }
    msg = rx.recv(), if enabled => { /* with precondition guard */ }
    else => { /* all branches disabled */ }
}
```

- Polls all branches concurrently *on the current task*; the first to complete with a
  matching pattern wins, its handler runs, and the macro evaluates to that handler.
- **The losing branches are dropped** — their futures stop where they are and lose any
  in-flight progress. This is the headline hazard.
- `if <cond>` is a *precondition*: when false, the branch's future is created but never
  polled. `else` runs when every branch is disabled; with no `else`, an all-disabled
  `select!` **panics**. `biased;` removes the random poll order (you then own fairness).

**Cancellation safety — the rule that prevents data loss.** Only loop a `select!` over
futures that are safe to drop and recreate. The tokio docs name these as **not**
cancel-safe; do not hold them across a `select!` iteration that might drop them:

- `Mutex::lock`, `RwLock::read`, `RwLock::write`
- `Semaphore::acquire`
- `Notify::notified`
- the buffered I/O helpers: `AsyncReadExt::read_exact` / `read_to_end` / `read_to_string`,
  `AsyncWriteExt::write_all`

Cancel-safe and fine in a `select!` loop: channel `recv` (`mpsc`/`broadcast`/`watch`),
`Interval::tick`, and `CancellationToken::cancelled`. The standard worker loop selects
between a cancel-safe work source and the cancellation token:

```rust
loop {
    tokio::select! {
        maybe = rx.recv() => match maybe {
            Some(job) => handle(job).await,
            None => break,                  // all senders dropped
        },
        () = token.cancelled() => break,    // graceful shutdown requested
    }
}
```

## `join!` / `try_join!`

Need the `macros` feature.

- `join!(a, b, ...)` waits for **all** branches concurrently on one task and returns a
  tuple of every result. Use it to run independent awaits in parallel within a task (two
  backend calls at once) rather than awaiting them in sequence.
- `try_join!(a, b, ...)` is for `Result`-returning futures: `Ok((t1, t2, ...))` when all
  succeed, or it **short-circuits on the first `Err`**, returning that error without
  waiting for the rest. Reach for it when any one failure should abandon the whole batch.
- Both poll fairly by default; `biased;` forces declaration order.

## `CancellationToken` and graceful shutdown

`CancellationToken` lives in `tokio-util`; for its full API and the `DropGuard` /
`run_until_cancelled` / parent-child details see [[pixhaus-tokio-util]]. The
pattern you need for Pixhaus:

```rust
// app holds a root token
let shutdown = tokio_util::sync::CancellationToken::new();

// each long-lived task gets a clone (or child_token()) and selects on it
let token = shutdown.clone();
rt.spawn(async move {
    tokio::select! {
        () = token.cancelled() => { /* clean up, flush, return */ }
        _  = do_long_running_work() => {}
    }
});

// on window close (after the unsaved-changes confirm in pixhaus-eframe):
shutdown.cancel();   // every task's cancelled() branch fires; they wind down
```

- `cancelled()` is **cancel-safe**, so it's the correct thing to put in a `select!` arm.
- `cancel()` is idempotent and wakes all current and future `cancelled()` awaits; calling
  it once on shutdown is enough.
- Tie this to the eframe close flow ([[pixhaus-eframe]]): veto the close, confirm unsaved
  changes, then `cancel()` and let tasks drain. Remember that `spawn_blocking` work in
  flight can't be aborted and will hold shutdown until it finishes — bound it, or accept
  the wait, rather than expecting `cancel()` to stop it.
