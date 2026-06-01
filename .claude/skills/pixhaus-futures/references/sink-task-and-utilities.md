# futures: sink, task, lock, executor, io, pin_mut

The "everything else" reference for the `futures` crate (0.3.32) — the parts
outside `Future`/`Stream`/`StreamExt`. Each area carries a guidance note steering
toward the runtime primitives Pixhaus standardizes on (tokio, pollster,
parking_lot) instead of the `futures` equivalents, which are usually the wrong
choice in this codebase.

## Contents

- [sink](#sink) — `Sink` + `SinkExt`
- [task](#task) — wakers, `ArcWake`, `AtomicWaker`, spawn traits, `FutureObj`
- [lock](#lock) — async `Mutex`, `BiLock`
- [executor](#executor) — `block_on`, `LocalPool`, `ThreadPool`
- [io](#io) — `futures-io` traits (not tokio's)
- [pin_mut](#pin_mut) — stack pinning

## sink

`Sink<Item>` is the write-side dual of `Stream`: a value you push items into
asynchronously, with backpressure via `poll_ready`. `SinkExt` adds the
combinators and the `.await`-able adapters.

### `Sink<Item>` trait

Required associated type and four methods. `cx` is `&mut Context<'_>`.

| Item | Signature |
| --- | --- |
| `type Error` | associated error type |
| `poll_ready` | `fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>` |
| `start_send` | `fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error>` |
| `poll_flush` | `fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>` |
| `poll_close` | `fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>` |

Protocol: call `poll_ready` until `Ready(Ok(()))`, then `start_send` exactly
once, then `poll_flush` to push through. `start_send` must not be called before
`poll_ready` returns ready.

### `SinkExt`

Provided methods on any `Sink`. The future-returning ones (`send`, `feed`,
`send_all`, `close`, `flush`) are what you `.await`.

| Method | Signature | Notes |
| --- | --- | --- |
| `send` | `fn send(&mut self, item: Item) -> Send<'_, Self, Item>` | `poll_ready` + `start_send` + flush, all in one await |
| `feed` | `fn feed(&mut self, item: Item) -> Feed<'_, Self, Item>` | like `send` but does NOT flush — batch then `flush` once |
| `send_all` | `fn send_all<'a, St>(&'a mut self, stream: &'a mut St) -> SendAll<'a, Self, St>` | drains a `TryStream`/`Stream` into the sink |
| `close` | `fn close(&mut self) -> Close<'_, Self>` | drives `poll_close` |
| `flush` | `fn flush(&mut self) -> Flush<'_, Self>` | drives `poll_flush` |
| `with` | `fn with<U, Fut, F, E>(self, f: F) -> With<Self, Item, U, Fut, F>` | map each incoming `U` through async `f` before sending |
| `with_flat_map` | `fn with_flat_map<U, St, F>(self, f: F) -> WithFlatMap<Self, Item, U, St, F>` | one input expands to a stream of items |
| `buffer` | `fn buffer(self, capacity: usize) -> Buffer<Self, Item>` | buffer up to `capacity` items before backpressuring |
| `fanout` | `fn fanout<Si>(self, other: Si) -> Fanout<Self, Si>` | send every item to both sinks |
| `drain` | (module fn) `fn drain<Item>() -> Drain<Item>` | sink that discards everything (Infallible error) |
| `sink_map_err` | `fn sink_map_err<E, F>(self, f: F) -> SinkMapErr<Self, F>` | map the error type |
| `sink_err_into` | `fn sink_err_into<E>(self) -> SinkErrInto<Self, Item, E>` | `Into`-convert the error |
| `left_sink` | `fn left_sink<Si2>(self) -> Either<Self, Si2>` | for branching two sink types into one |
| `right_sink` | `fn right_sink<Si1>(self) -> Either<Si1, Self>` | other branch |

Module-level constructors: `sink::drain() -> Drain<T>` and
`sink::unfold(init, f) -> Unfold<...>` (build a sink from an async per-item
closure).

Connecting streams and sinks: `StreamExt::forward(sink)` pumps a stream into a
sink to completion; `StreamExt::split()` splits a type that is both `Stream` and
`Sink` into a `(SplitStream, SplitSink)` pair.

```rust
use futures::sink::SinkExt;

// feed a batch, then flush once
for item in batch {
    sink.feed(item).await?;
}
sink.flush().await?;
```

**Guidance:** `Sink` matters for channel-backed pipelines and codecs. For tokio
channels and `tokio_util::codec` framed I/O, the relevant `Sink` impls come from
those crates — see `pixhaus-tokio` and `pixhaus-tokio-util`. Don't pull in a
second framing stack.

## task

Wakers, the executor-abstraction traits, and owned future objects. Most of this
is for generic library code; Pixhaus spawns through tokio and drives its own egui
loop, so the load-bearing items here are `ArcWake`, `AtomicWaker`, and the
no-op wakers for tests.

### Re-exports from `core::task`

| Item | One line |
| --- | --- |
| `Context` | carries the `Waker`; passed to every `poll` |
| `Poll<T>` | `Ready(T)` or `Pending` |
| `Waker` | clone-able handle that schedules a task to be polled again |
| `RawWaker` | raw waker pointer + vtable; constructing one is `unsafe` |
| `RawWakerVTable` | function table (`clone`/`wake`/`wake_by_ref`/`drop`) behind a `RawWaker` |

Note: `core::task::Wake` (the safe `Arc`-based std trait) is NOT re-exported here;
`futures::task` ships its own `ArcWake` for the same job. (verify whether a `Wake`
re-export exists — not seen in the module index.)

### `ArcWake` — the no-unsafe waker path

**Guidance:** the Pixhaus workspace forbids `unsafe`, so `ArcWake` + `waker()`
is the sanctioned way to build a `Waker`. Hand-rolling a `RawWaker`/
`RawWakerVTable` requires `unsafe` and will not pass the workspace lint — do not
go that route.

```rust
pub trait ArcWake: Send + Sync {
    fn wake_by_ref(arc_self: &Arc<Self>);
    fn wake(self: Arc<Self>) { Self::wake_by_ref(&self) } // provided
}
```

| Function | Signature | Use |
| --- | --- | --- |
| `waker` | `fn waker<W: ArcWake + 'static>(wake: Arc<W>) -> Waker` | owned `Waker` from your `ArcWake` |
| `waker_ref` | `fn waker_ref<W: ArcWake>(wake: &Arc<W>) -> WakerRef<'_>` | borrowed, avoids an `Arc` clone when polling in a loop |

```rust
use std::sync::Arc;
use futures::task::{self, ArcWake};

struct LoopWaker { /* e.g. an egui repaint signal */ }
impl ArcWake for LoopWaker {
    fn wake_by_ref(_arc_self: &Arc<Self>) { /* request a repaint */ }
}
let waker = task::waker(Arc::new(LoopWaker { /* .. */ }));
```

### `AtomicWaker`

Single-consumer wake cell that multiple producers can write. The standard
primitive for "many producers want to nudge one task." A natural fit for a custom
egui-loop waker that background threads signal.

| Method | Signature |
| --- | --- |
| `new` | `fn new() -> AtomicWaker` (const) |
| `register` | `fn register(&self, waker: &Waker)` — consumer stores its waker before returning `Pending` |
| `wake` | `fn wake(&self)` — producer wakes the stored waker |
| `take` | `fn take(&self) -> Option<Waker>` — remove and return the stored waker |

### No-op wakers (tests, manual poll)

| Function | Signature |
| --- | --- |
| `noop_waker` | `fn noop_waker() -> Waker` |
| `noop_waker_ref` | `fn noop_waker_ref() -> &'static Waker` |

```rust
use futures::task::noop_waker_ref;
use std::task::Context;
let mut cx = Context::from_waker(noop_waker_ref());
// fut.as_mut().poll(&mut cx)  // drive a future by hand in a test
```

### Spawn traits

Executor abstraction. **Guidance:** in Pixhaus, spawning goes through the tokio
runtime owned by the binary (`pixhaus-tokio`); these traits are for runtime-
generic library code only. Don't introduce a `futures` executor just to satisfy
a `Spawn` bound — take a tokio handle instead.

| Trait | Required method |
| --- | --- |
| `Spawn` | `fn spawn_obj(&self, future: FutureObj<'static, ()>) -> Result<(), SpawnError>` |
| `LocalSpawn` | `fn spawn_local_obj(&self, future: LocalFutureObj<'static, ()>) -> Result<(), SpawnError>` |

| Ext trait | Methods |
| --- | --- |
| `SpawnExt` | `spawn<Fut: Future<Output=()> + Send + 'static>(&self, future: Fut) -> Result<(), SpawnError>`; `spawn_with_handle<Fut>(&self, future: Fut) -> Result<RemoteHandle<Fut::Output>, SpawnError>` |
| `LocalSpawnExt` | `spawn_local<Fut: Future<Output=()> + 'static>(&self, future: Fut) -> Result<(), SpawnError>`; `spawn_local_with_handle<Fut>(...) -> Result<RemoteHandle<Fut::Output>, SpawnError>` |

### `FutureObj` / `LocalFutureObj`

Owned `dyn Future` wrappers for executors that want to avoid an allocation
(custom no-alloc executors store the future inline). `FutureObj` is the `Send`
variant; `LocalFutureObj` is not `Send`. Both: `fn new<F>(f: F) -> Self`
(`F: Future<Output=T> + Send` for `FutureObj`). Rarely needed in app code.

## lock

### `Mutex<T>` — async mutex

`lock()` returns a future, not a guard. That is the whole point: a task awaiting
the lock yields instead of blocking the thread.

| Method | Signature |
| --- | --- |
| `new` | `const fn new(t: T) -> Mutex<T>` |
| `lock` | `fn lock(&self) -> MutexLockFuture<'_, T>` — `.await` yields `MutexGuard<'_, T>` |
| `try_lock` | `fn try_lock(&self) -> Option<MutexGuard<'_, T>>` |
| `lock_owned` | `fn lock_owned(self: Arc<Mutex<T>>) -> OwnedMutexLockFuture<T>` — guard with no borrow |
| `try_lock_owned` | `fn try_lock_owned(self: &Arc<Mutex<T>>) -> Option<OwnedMutexGuard<T>>` |
| `get_mut` | `fn get_mut(&mut self) -> &mut T` — no locking, needs `&mut self` |
| `into_inner` | `fn into_inner(self) -> T` |

`MappedMutexGuard<T, U>`: a guard projected to a field of the locked value via
`MutexGuard::map(guard, f)` (verify exact `map` signature — not shown in the
struct index excerpt).

**Guidance:** `futures::lock::Mutex` is an ASYNC mutex. In Pixhaus you almost
never want it. For short, non-async critical sections use `parking_lot::Mutex`
or `std::sync::Mutex` (see `pixhaus-parking-lot`). If a guard must survive an
`.await` inside tokio code, use `tokio::sync::Mutex` (see `pixhaus-tokio`).
Reach for `futures::lock::Mutex` only in runtime-agnostic library code with no
tokio dependency. Workspace rule regardless of which mutex: never hold a guard
across `.await`.

### `BiLock<T>`

Splits one value into exactly two owners, each of which can lock it. Cheaper than
a general `Mutex` because contention is bounded to two parties — the canonical use
is splitting a duplex stream into independent read and write halves.

| Method | Signature |
| --- | --- |
| `new` | `fn new(t: T) -> (BiLock<T>, BiLock<T>)` — returns the pair |
| `lock` | `fn lock(&self) -> BiLockAcquire<'_, T>` — `.await` yields `BiLockGuard<'_, T>` |
| `poll_lock` | `fn poll_lock(&self, cx: &mut Context<'_>) -> Poll<BiLockGuard<'_, T>>` |
| `reunite` | `fn reunite(self, other: BiLock<T>) -> Result<T, ReuniteError<T>> where T: Unpin` |

## executor

`futures`' built-in executors and the sync→async bridge.

| Item | Signature / note |
| --- | --- |
| `block_on` | `fn block_on<F: Future>(f: F) -> F::Output` — run a future to completion on the current thread |
| `block_on_stream` | `fn block_on_stream<S: Stream + Unpin>(stream: S) -> BlockingStream<S>` — turn a stream into a blocking `Iterator` |
| `LocalPool` | `new()`, `run(&mut self)`, `run_until<F: Future>(&mut self, f: F) -> F::Output`, `spawner(&self) -> LocalSpawner` — single-thread pool for `!Send` futures |
| `LocalSpawner` | handle implementing `Spawn`/`LocalSpawn` for a `LocalPool` |
| `ThreadPool` | `new() -> Result<ThreadPool, io::Error>`, `builder() -> ThreadPoolBuilder` — multi-thread pool |
| `ThreadPoolBuilder` | `pool_size`, `name_prefix`, `stack_size`, `create()` |
| `enter` | `fn enter() -> Result<Enter, EnterError>` — mark the thread as inside an executor (prevents nested `block_on`) |
| `Enter` | RAII guard returned by `enter` |

**Guidance:** Pixhaus does NOT use `futures::executor`. The binary owns a tokio
runtime; sync→async bridging in the render crate's tests and benches uses
`pollster::block_on` (see `pixhaus-pollster`, also the standard way to await
`wgpu` device/adapter requests). `futures::executor::block_on` is a fine
standalone alternative, but the project standardizes on pollster + tokio — do not
add `ThreadPool` or `LocalPool` as a second runtime.

## io

The `futures-io` async I/O trait family, mirroring `std::io`.

| Core trait | Companion ext trait |
| --- | --- |
| `AsyncRead` | `AsyncReadExt` |
| `AsyncWrite` | `AsyncWriteExt` |
| `AsyncBufRead` | `AsyncBufReadExt` |
| `AsyncSeek` | `AsyncSeekExt` |

Key combinators (on the ext traits): `read`, `read_exact`, `read_to_end`,
`read_to_string` (`AsyncReadExt`); `write`, `write_all`, `flush`, `close`
(`AsyncWriteExt`); `fill_buf`, `read_until`, `read_line`, `lines`
(`AsyncBufReadExt`); `seek` (`AsyncSeekExt`); module fns `copy`, `copy_buf`,
`empty`, `repeat`, `sink`. Useful structs: `Cursor`, `BufReader`, `BufWriter`,
`AllowStdIo` (wrap a sync `std::io` type), `ReadHalf`/`WriteHalf`.

**Guidance:** these are the `futures-io` traits, which are DIFFERENT from and
incompatible with `tokio::io::AsyncRead`/`AsyncWrite` — tokio defines its own
trait family. In Pixhaus, file and socket I/O goes through tokio, so use
`tokio::io` (see `pixhaus-tokio`). If you must adapt between the two families,
bridge with `tokio_util::compat` (`.compat()` / `.compat_write()`, see
`pixhaus-tokio-util`). Do not reach for `futures::io` for application I/O.

## pin_mut

`pin_mut!(x)` takes ownership of `x` and rebinds the same name to a
`Pin<&mut T>` anchored to the stack, so a `!Unpin` value can be polled or passed
where pinning is required — for example a future before `select!`, or before a
manual `poll`.

```rust
macro_rules! pin_mut { ($($x:ident),* $(,)?) => { ... }; }
```

```rust
use futures::pin_mut;

let fut = some_async_fn();      // not Unpin
pin_mut!(fut);                  // fut is now Pin<&mut _>
// fut.poll(&mut cx) / select!(fut, ...)
```

Note: soft-deprecated since Rust 1.68 in favor of the std `std::pin::pin!`
macro, which does the same thing. Prefer `std::pin::pin!` in new code; `pin_mut!`
remains for compatibility.
