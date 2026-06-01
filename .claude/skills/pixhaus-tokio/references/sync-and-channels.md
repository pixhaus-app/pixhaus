# Sync and channels

Verified against tokio 1.52.3. This file covers moving data between tasks and the UI
thread, and the synchronization primitives. For spawning see `runtime-and-tasks.md`; for
`select!` and shutdown see `time-and-coordination.md`.

## Contents

- The channel-choice matrix
- `mpsc` — worker results to the UI
- `oneshot` — one request, one reply
- `watch` — latest value / progress
- `broadcast` — fan-out to many
- `tokio::sync::Mutex` / `RwLock` vs parking_lot
- `Semaphore` — bound concurrency
- `Notify` — wake with no data

## The channel-choice matrix

Pick by the shape of the communication, not by habit.

| Need | Use | Why |
|---|---|---|
| Many results streaming from worker(s) to the UI loop | `mpsc` | multi-producer, single-consumer; the UI owns the one receiver |
| One reply to one request (an AI call's result) | `oneshot` | exactly one value; `send` is sync so it works from anywhere |
| Latest-only state: progress %, "is generating" | `watch` | keeps only the newest value; cheap repeated reads via `borrow` |
| One event to many independent listeners | `broadcast` | every receiver sees every value (rare in Pixhaus) |
| "Wake up, something changed" with no payload | `Notify` | signal-only; pairs with state behind a parking_lot lock |

The default in Pixhaus is **`mpsc` for the worker→UI results stream** and **`oneshot` for
a single request's reply**. Reach for the others only when the shape genuinely fits.

## `mpsc` — worker results to the UI

```rust
pub fn channel<T>(buffer: usize) -> (Sender<T>, Receiver<T>)          // bounded; panics if buffer == 0
pub fn unbounded_channel<T>() -> (UnboundedSender<T>, UnboundedReceiver<T>)

// bounded Sender
pub async fn send(&self, value: T) -> Result<(), SendError<T>>       // waits when full (backpressure)
pub fn try_send(&self, message: T) -> Result<(), TrySendError<T>>
pub fn blocking_send(&self, value: T) -> Result<(), SendError<T>>    // from sync/blocking code
// Receiver
pub async fn recv(&mut self) -> Option<T>                            // None when all senders dropped
pub fn try_recv(&mut self) -> Result<T, TryRecvError>                // the UI-thread drain
pub fn blocking_recv(&mut self) -> Option<T>
```

- **`Sender` is `Clone`** (each clone is an `Arc`-like refcount); **`Receiver` is not** —
  that's the single-consumer half. The UI holds the one `Receiver`; every spawn site
  clones a `Sender`.
- **The UI drains with `try_recv` in `logic`**, never `recv().await` (which would block the
  frame). Loop `while let Ok(v) = rx.try_recv() { ... }`; `Empty` is the normal idle case.
- **Lifecycle signals are free:** when all senders drop, `recv` drains the buffer then
  yields `None`; when the receiver drops, `send` errors. A send error on a worker almost
  always means the window closed — `let _ =` it.
- **Bounded vs unbounded:** prefer **bounded** (`channel(n)`) so a fast producer can't grow
  memory without limit — backpressure makes the producer wait. Use `unbounded_channel`
  only when the producer must never await on send (e.g. sending from a sync callback) and
  the volume is naturally small. A flood of progress updates is better modeled with
  `watch` than an unbounded mpsc.

```rust
// build once in the app constructor:
let (tx, rx) = mpsc::channel::<JobResult>(64);
// each spawn: let tx = tx.clone(); move tx into the task; tx.send(..).await
// each frame in logic: while let Ok(r) = rx.try_recv() { self.apply(r); }
```

## `oneshot` — one request, one reply

```rust
pub fn channel<T>() -> (Sender<T>, Receiver<T>)
pub fn send(self, value: T) -> Result<(), T>   // SYNC, consumes self; Err returns the unsent value
// Receiver implements Future:  rx.await -> Result<T, RecvError>
```

- **`send` is synchronous** — it takes `self` by value and returns immediately. That's why
  oneshot works from non-async code and across runtimes. On failure it hands back the
  value you tried to send.
- The `Receiver` *is* a future — `rx.await` yields `Ok(T)` or `Err(RecvError)` (sender
  dropped without sending).
- The actor reply pattern: send a command over an `mpsc` together with a `oneshot::Sender`
  for the answer; the handler does the work and `tx.send(answer)`s. In Pixhaus this is the
  clean way to ask a background service for one result without a long-lived channel.

## `watch` — latest value / progress

```rust
pub fn channel<T>(init: T) -> (Sender<T>, Receiver<T>)
// Sender:   pub fn send(&self, value: T) -> Result<(), SendError<T>>
// Receiver: pub fn borrow(&self) -> Ref<'_, T>                  // does NOT mark seen
//           pub fn borrow_and_update(&mut self) -> Ref<'_, T>   // marks seen
//           pub async fn changed(&mut self) -> Result<(), RecvError>
```

- Keeps only the **most recent** value; slow readers see the latest, not every step.
  Ideal for a progress fraction or a "generating / idle" status the UI polls each frame
  with `borrow()`.
- **`Receiver` is `Clone`** — many watchers, including across threads.
- `changed().await` resolves when an unseen value lands; it errors once all senders drop.
  For the UI you usually just `borrow()` in `logic` rather than awaiting `changed`.

## `broadcast` — fan-out to many

```rust
pub fn channel<T: Clone>(cap: usize) -> (Sender<T>, Receiver<T>)
// Sender:   send(&self, value: T) -> Result<usize, SendError<T>>   // count of live receivers
//           subscribe(&self) -> Receiver<T>
// Receiver: recv(&mut self) -> Result<T, RecvError>                // Lagged(n) | Closed
```

- Every receiver sees every value (`T: Clone`); new receivers come from `subscribe()` and
  only see values sent after they subscribe.
- The ring buffer of size `cap` overwrites the oldest unread value when full; a receiver
  that fell behind gets `RecvError::Lagged(n)` and resumes from the oldest still-held
  value. Recoverable — call `recv` again. Rarely needed in a single-window editor; reach
  for it only when several independent subsystems each need the full event stream.

## `tokio::sync::Mutex` / `RwLock` vs parking_lot

The rule from CLAUDE.md: **never hold a lock across `.await`.** The tokio docs say the same
from the other side — the *only* thing the async mutex buys you over a blocking mutex is
holding the guard across an await, and it's slower, so prefer the blocking mutex wherever
you can.

```rust
// tokio async mutex — only when the guard must live across an .await
pub async fn lock(&self) -> MutexGuard<'_, T>
pub fn try_lock(&self) -> Result<MutexGuard<'_, T>, TryLockError>
pub fn blocking_lock(&self) -> MutexGuard<'_, T>   // from sync code; never on a runtime worker
```

- **Default to `parking_lot::Mutex` / `RwLock`** for short, synchronous critical sections
  (see [[pixhaus-parking-lot]]). Their guards are `!Send`, so the compiler stops you from
  accidentally holding one across an await — the error is a feature.
- **Use `tokio::sync::Mutex` only when crossing an await inside the critical section is
  genuinely unavoidable.** Its guard is `Send`, keeping the future `Send`. If you find
  yourself wanting it, first check whether you can drop the lock before the await and
  re-acquire after — usually you can, and that's faster and clearer.
- Don't call `blocking_lock` on a runtime worker thread — it can deadlock the worker. It's
  for sync/blocking contexts only.
- **Most Pixhaus state needs no lock at all.** The document is owned by the UI thread.
  Tasks get the data they need by value (copy the region/slice into the spawned closure)
  or over a channel — single ownership beats `Arc<Mutex<_>>`. See [[pixhaus-rust-conventions]].

## `Semaphore` — bound concurrency

```rust
pub fn new(permits: usize) -> Self
pub async fn acquire(&self) -> Result<SemaphorePermit<'_>, AcquireError>          // Err only if closed
pub async fn acquire_owned(self: Arc<Self>) -> Result<OwnedSemaphorePermit, AcquireError>
pub fn try_acquire(&self) -> Result<SemaphorePermit<'_>, TryAcquireError>
pub fn add_permits(&self, n: usize)
```

- A permit is returned to the semaphore when its guard drops. Hold the permit for the
  duration of the work to keep concurrency capped at N.
- **This is how Pixhaus bounds fan-out** (the 8K perf constraint, [[project_8k_perf_constraint]]):
  one large operation can split into many region jobs, but spawning them all at once
  saturates the CPU and the blocking pool. Gate them.

```rust
let sem = Arc::new(Semaphore::new(num_cpus));       // cap = core count
for region in regions {
    let permit = sem.clone().acquire_owned().await?;   // waits if N are already running
    let tx = results_tx.clone();
    rt.spawn_blocking(move || {
        let _permit = permit;                           // held until the job finishes
        let out = process_region(region);
        let _ = tx.blocking_send(out);
    });
}
```

- Use `acquire_owned` (needs `Arc<Semaphore>`) when the permit must move into a spawned
  task; use `acquire` for a permit held within the current scope.

## `Notify` — wake with no data

```rust
pub fn new() -> Notify
pub fn notified(&self) -> Notified<'_>   // a future — must be awaited
pub fn notify_one(&self)
pub fn notify_waiters(&self)
```

- Signal-only wakeup; pair it with state behind a parking_lot lock when the "what changed"
  lives elsewhere. `notify_one` **stores one permit** if nobody is waiting, so the next
  `notified().await` returns immediately; `notify_waiters` wakes only those *already*
  waiting and stores nothing.
- The ordering trap: with `notify_waiters`, register your `notified()` future *before* the
  point where a signal could arrive, or you'll miss it. For a single drain loop,
  `notify_one` is usually the safer choice because of its stored permit.
- In Pixhaus this is niche — the channel + `request_repaint` pattern already wakes the UI.
  Reach for `Notify` for background-task-to-background-task coordination where there's no
  value to carry.
