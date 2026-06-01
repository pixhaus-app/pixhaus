# Runtime and tasks

Verified against tokio 1.52.3. This file covers building the one runtime, spawning onto
it, and the task handles. For channels see `sync-and-channels.md`; for timers, `select!`,
and shutdown see `time-and-coordination.md`.

## Contents

- Building the runtime in `main`
- `Runtime`
- `runtime::Builder`
- `Handle`
- `tokio::spawn`
- `JoinHandle` — abort vs detach
- `spawn_blocking`
- `block_in_place`
- `JoinSet`
- `LocalSet` / `spawn_local`
- `yield_now`
- Feature flags

## Building the runtime in `main`

The binary builds one runtime explicitly and holds a `Handle` clone wherever spawning is
needed. Do not use `#[tokio::main]` — it wraps `main` in a runtime and runs your code
inside `block_on`, which collides with eframe wanting to own the main thread and run the
winit event loop there.

```rust
fn main() -> eframe::Result {
    // one runtime for the whole program; multi-thread + I/O + time drivers enabled
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("pixhaus-worker")
        .build()
        .expect("build tokio runtime");   // main() may unwrap; library code may not

    let handle = rt.handle().clone();      // cheap to clone; hand to the app and to tasks

    // keep `rt` alive for the program's life — dropping it blocks until tasks finish.
    // eframe runs the winit loop on this thread; the app spawns onto `handle`.
    eframe::run_native(
        "Pixhaus",
        native_options(),
        Box::new(move |cc| Ok(Box::new(Pixhaus::new(cc, handle)))),
    )
}
```

The app stores the `Handle`, not the `Runtime`. The `Runtime` value stays in `main` so its
`Drop` (which blocks until tasks finish) runs at the very end, after eframe returns.

## `Runtime`

```rust
pub fn new() -> io::Result<Runtime>   // = Builder::new_multi_thread().enable_all().build()
pub fn block_on<F: Future>(&self, future: F) -> F::Output
pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
where F: Future + Send + 'static, F::Output: Send + 'static
pub fn handle(&self) -> &Handle
pub fn shutdown_background(self)
pub fn shutdown_timeout(self, duration: Duration)
```

- `Runtime::new()` is the quick multi-thread-with-everything constructor; use `Builder`
  when you want a thread name or to cap threads.
- `block_on` drives a future to completion on the calling thread. It **panics if called
  from within an async context** (another runtime). In Pixhaus the only legitimate
  `block_on` is the one eframe's loop implies — you don't call it yourself on the UI
  thread.
- **Drop blocks.** Dropping a `Runtime` waits for all spawned tasks with no timeout, which
  is why `rt` stays in `main` and shuts down last. If you ever need to tear a runtime down
  from inside async code, use `shutdown_background` or `shutdown_timeout` instead of
  letting `Drop` run there (it can deadlock).

## `runtime::Builder`

```rust
pub fn new_multi_thread() -> Builder      // work-stealing pool
pub fn new_current_thread() -> Builder    // single thread; tasks progress only in block_on
pub fn worker_threads(&mut self, val: usize) -> &mut Self   // panics if 0; default = num CPUs
pub fn enable_all(&mut self) -> &mut Self                   // = enable_io + enable_time
pub fn enable_io(&mut self) -> &mut Self
pub fn enable_time(&mut self) -> &mut Self
pub fn thread_name(&mut self, val: impl Into<String>) -> &mut Self  // default "tokio-runtime-worker"
pub fn max_blocking_threads(&mut self, val: usize) -> &mut Self     // panics if 0; default 512
pub fn build(&mut self) -> io::Result<Runtime>
```

- Pixhaus uses `new_multi_thread().enable_all()`. `enable_time()` is what powers
  `tokio::time`; `enable_io()` powers networking/process/signal and async I/O. `enable_all`
  covers both and is the safe default for the binary.
- `worker_threads` defaults to the CPU count — leave it unless profiling says otherwise.
- `max_blocking_threads` (default 512) caps the `spawn_blocking` pool. It's large because
  blocking I/O often has no async form; you rarely change it, but a runaway fan-out of
  `spawn_blocking` is what exhausts it — bound that with a Semaphore instead.

## `Handle`

```rust
pub fn current() -> Handle                              // panics outside a runtime context
pub fn try_current() -> Result<Handle, TryCurrentError> // never panics
pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
where F: Future + Send + 'static, F::Output: Send + 'static
pub fn spawn_blocking<F, R>(&self, func: F) -> JoinHandle<R>
where F: FnOnce() -> R + Send + 'static, R: Send + 'static
pub fn block_on<F: Future>(&self, future: F) -> F::Output
pub fn enter(&self) -> EnterGuard<'_>
```

- A `Handle` is a cheap, cloneable reference to the runtime. The app holds one and spawns
  through `self.rt.spawn(...)` / `self.rt.spawn_blocking(...)`.
- `Handle::current()` **panics** outside a runtime; use `try_current()` when you can't be
  sure one is active. In Pixhaus you generally pass the handle explicitly rather than
  relying on `current()`.
- `enter()` sets the thread's runtime context (returns a guard) so you can construct
  runtime-bound types — e.g. a `tokio::time` timer or an mpsc channel — from a non-task
  thread. Hold the guard; drop guards in reverse acquisition order or it panics.

## `tokio::spawn`

```rust
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where F: Future + Send + 'static, F::Output: Send + 'static
```

- Free-function form; needs a runtime context (panics without one). `self.rt.spawn(...)`
  via the stored `Handle` is the form to use in Pixhaus so you're never depending on an
  ambient context.
- `Send + 'static` on both future and output: the task may run on any worker and outlive
  the caller, so it can't borrow. Move owned data in; if you need data from the document,
  copy the slice the task needs before spawning rather than sharing a lock.
- **If the task panics, the runtime catches it** — it does not bring the runtime down. The
  panic surfaces when you await the `JoinHandle` as `Err(JoinError)` with `is_panic()`
  true. A fire-and-forget spawn whose handle you drop swallows the panic silently, so log
  inside the task if it matters.

## `JoinHandle` — abort vs detach

```rust
impl<T> Future for JoinHandle<T> { type Output = Result<T, JoinError>; }
pub fn abort(&self)
pub fn is_finished(&self) -> bool
pub fn abort_handle(&self) -> AbortHandle
// JoinError::is_panic() / is_cancelled() / into_panic()
```

- Awaiting yields `Result<T, JoinError>`: `Ok(T)` on success, `Err` if the task panicked
  or was aborted.
- **Dropping a `JoinHandle` does NOT cancel the task — it detaches it.** The task runs to
  completion in the background; you just lose the ability to observe its result. This is
  the single most-misremembered tokio fact. The Pixhaus "spawn, send result over a
  channel, drop the handle" pattern relies on detach being fine — the channel carries the
  result, not the handle. When you *do* want to cancel on drop, call `abort()` yourself or
  put the task in a `JoinSet`.
- `abort()` requests cancellation; the task stops at its next await point. `abort_handle()`
  hands out a cloneable cancel-only handle so another part of the app can cancel without
  owning the `JoinHandle`.

## `spawn_blocking`

```rust
pub fn spawn_blocking<F, R>(f: F) -> JoinHandle<R>
where F: FnOnce() -> R + Send + 'static, R: Send + 'static
```

- Runs a *synchronous closure* on the dedicated blocking pool so it doesn't stall a
  scheduler worker. This is where Pixhaus does CPU-bound pixel work, PNG/zstd encode and
  decode, and `std::fs` reads and writes.
- Gotchas: a running `spawn_blocking` task **cannot be aborted**, and it **blocks runtime
  shutdown** until it returns (bound shutdown with `shutdown_timeout` if needed). A
  *forever* loop should use `std::thread::spawn`, not `spawn_blocking` — it would
  permanently occupy a pool thread.
- For many CPU jobs at once (the 8K fan-out case), gate with a `Semaphore` so you cap how
  many run concurrently instead of flooding the pool. See `sync-and-channels.md`.

## `block_in_place`

```rust
pub fn block_in_place<F, R>(f: F) -> R where F: FnOnce() -> R   // no Send/'static bound
```

- Multi-thread runtime only. **Panics on a current-thread runtime.**
- Tells the scheduler the current worker is about to block so it can move sibling tasks
  elsewhere, then runs `f` inline on this thread. Cheaper than `spawn_blocking` (no thread
  hand-off, no `Send`/`'static`) but it **suspends anything else running in the same task**
  — e.g. other `join!` branches.
- Rarely the right tool in Pixhaus: from the UI thread you spawn instead of block, and
  `spawn_blocking` is clearer for offloading. Reach for `block_in_place` only inside an
  existing multi-thread task that must run one blocking call and has nothing concurrent to
  starve.

## `JoinSet`

```rust
pub fn new() -> Self
pub fn spawn<F>(&mut self, task: F) -> AbortHandle
where F: Future<Output = T> + Send + 'static, T: Send
pub fn spawn_blocking<F>(&mut self, f: F) -> AbortHandle
where F: FnOnce() -> T + Send + 'static, T: Send
pub async fn join_next(&mut self) -> Option<Result<T, JoinError>>
pub fn abort_all(&mut self)
pub async fn shutdown(&mut self)
pub fn len(&self) -> usize
```

- A set of tasks that all return the same `T`, consumed **in completion order** via
  `join_next().await` (`None` when empty). `join_next` is cancel-safe.
- **Dropping a `JoinSet` aborts all its tasks** — the opposite of a bare `JoinHandle`. Use
  it when you fan out work (e.g. one task per region) and want them cancelled together if
  the owner goes away, and want results as they finish.
- `abort_all()` aborts but leaves tasks in the set — still drain with `join_next` to
  observe the cancellations. `shutdown().await` is `abort_all` then drain-to-empty.

## `LocalSet` / `spawn_local`

```rust
pub fn spawn_local<F>(&self, future: F) -> JoinHandle<F::Output>
where F: Future + 'static, F::Output: 'static    // NO Send bound
```

- For `!Send` futures (holding `Rc`, `RefCell`, a non-`Send` FFI handle) that `tokio::spawn`
  rejects. Tasks stay on the current thread. Needs a current-thread runtime and is driven
  with `local_set.run_until(fut).await`.
- Pixhaus shouldn't normally need this: the binary runs a multi-thread runtime and the
  UI's non-`Send` state lives on the UI thread, not in tasks. If you reach for `spawn_local`
  to dodge a `Send` error, first check whether the data should be copied/owned into the
  task instead.

## `yield_now`

```rust
pub async fn yield_now()
```

- Cooperatively hands control back to the scheduler, then resumes. Use it to break up a
  long async loop that rarely hits a natural await so it doesn't monopolize a worker.
  CPU-bound work that never awaits belongs in `spawn_blocking`, not in an async loop with
  `yield_now` sprinkled in.

## Feature flags

The binary enables `full`. What the relevant flags gate:

| Flag | Gates |
|---|---|
| `full` | everything public except `test-util` and unstable APIs |
| `rt` | task spawning + current-thread scheduler |
| `rt-multi-thread` | the multi-thread work-stealing scheduler (implies `rt`) |
| `macros` | `#[tokio::main]`, `#[tokio::test]`, `select!`, `join!`, `try_join!` |
| `sync` | `Mutex`, `RwLock`, channels, `Notify`, `Semaphore` |
| `time` | `sleep`, `interval`, `timeout`, `Instant` |
| `io-util` | `AsyncReadExt` / `AsyncWriteExt` and friends |
| `net` / `fs` / `process` / `signal` | networking / async filesystem / child processes / OS signals |

Pixhaus rarely uses `net`/`fs`/`process`/`signal` directly (HTTP goes through reqwest;
file work is `spawn_blocking` + `std::fs`), but `full` enables them and they cost nothing
if unused.
