# rayon 1.12.0 — thread pools and configuration

By default, rayon runs every `par_iter`/`join`/`scope` on a single global pool sized to
the CPU count. You rarely need more. Build an explicit `ThreadPool` only to (a) cap or
size threads for one subsystem, (b) set a thread name / stack size / panic handler, or
(c) keep one workload's tasks isolated from another's.

## The global pool

Created lazily on first use, sized to the logical CPU count, overridable by the
`RAYON_NUM_THREADS` environment variable. To configure it programmatically, call
**once, early** in `main`:

```rust
rayon::ThreadPoolBuilder::new()
    .num_threads(8)
    .build_global()
    .expect("global rayon pool already initialized");  // in the binary, anyhow/expect is fine
```

`build_global()` succeeds at most once; a second call returns `Err`. Don't call it from a
library crate — leave the choice to the binary, the same way the tokio runtime is owned by
the binary ([[pixhaus-tokio]]).

## `ThreadPoolBuilder`

```rust
ThreadPoolBuilder::new() -> ThreadPoolBuilder            // = ThreadPoolBuilder<DefaultSpawn>

fn build(self) -> Result<ThreadPool, ThreadPoolBuildError>
fn build_global(self) -> Result<(), ThreadPoolBuildError>          // init the global pool (once)
fn build_scoped<W, F, R>(self, wrapper: W, with_pool: F) -> Result<R, ThreadPoolBuildError>
    where W: Fn(ThreadBuilder) + Sync, F: FnOnce(&ThreadPool) -> R  // pool lives only for `with_pool`

fn num_threads(self, n: usize) -> Self                   // 0/unset => RAYON_NUM_THREADS, else CPU count
fn thread_name<F>(self, f: F) -> Self                    where F: FnMut(usize) -> String + 'static
fn stack_size(self, bytes: usize) -> Self
fn use_current_thread(self) -> Self                      // calling thread becomes worker index 0
fn panic_handler<H>(self, h: H) -> Self                  where H: Fn(Box<dyn Any + Send>) + Send + Sync + 'static
fn start_handler<H>(self, h: H) -> Self                  where H: Fn(usize) + Send + Sync + 'static  // per thread at start
fn exit_handler<H>(self, h: H) -> Self                   where H: Fn(usize) + Send + Sync + 'static  // per thread at exit
fn spawn_handler<F>(self, f: F) -> ThreadPoolBuilder<CustomSpawn<F>>
    where F: FnMut(ThreadBuilder) -> Result<(), std::io::Error>    // custom thread creation
fn breadth_first(self) -> Self                           // DEPRECATED — use scope_fifo / spawn_fifo
```

- `num_threads(0)` (or never calling it) means "honor `RAYON_NUM_THREADS`, else CPU
  count." Set an explicit number only when you have a reason (leave a core for the UI
  thread, cap memory).
- `panic_handler` catches panics that have no caller to propagate to — chiefly from
  detached `spawn`. **Without one, an unhandled pool panic aborts the process.** For an
  editor that should survive a botched filter, install a handler that logs and continues.
- `start_handler`/`exit_handler` run per worker thread for thread-local setup/teardown.
- `spawn_handler` swaps in custom thread creation (e.g. to set affinity or register the
  thread with a profiler); the default uses `std::thread::Builder`.

## `ThreadPool`

```rust
fn install<OP, R>(&self, op: OP) -> R                    where OP: FnOnce() -> R + Send, R: Send
fn join<A, B, RA, RB>(&self, a: A, b: B) -> (RA, RB)     // same bounds as rayon::join
fn scope<'s, OP, R>(&self, op: OP) -> R                  where OP: FnOnce(&Scope<'s>) -> R + Send, R: Send
fn scope_fifo<'s, OP, R>(&self, op: OP) -> R
fn in_place_scope<'s, OP, R>(&self, op: OP) -> R         where OP: FnOnce(&Scope<'s>) -> R
fn in_place_scope_fifo<'s, OP, R>(&self, op: OP) -> R
fn spawn<OP>(&self, op: OP)                              where OP: FnOnce() + Send + 'static
fn spawn_fifo<OP>(&self, op: OP)                         where OP: FnOnce() + Send + 'static
fn spawn_broadcast<OP>(&self, op: OP)                    where OP: Fn(BroadcastContext) + Send + Sync + 'static
fn broadcast<OP, R>(&self, op: OP) -> Vec<R>             where OP: Fn(BroadcastContext) -> R + Sync, R: Send

fn current_num_threads(&self) -> usize
fn current_thread_index(&self) -> Option<usize>
fn current_thread_has_pending_tasks(&self) -> Option<bool>
fn yield_now(&self) -> Option<Yield>
fn yield_local(&self) -> Option<Yield>
```

`install` is the key method: anything inside its closure — including parallel iterators —
runs on *this* pool instead of the global one.

```rust
let pool = rayon::ThreadPoolBuilder::new().num_threads(4).build()?;
let result = pool.install(|| {
    big_buffer.par_chunks_mut(width * 4).for_each(process_row);   // runs on the 4-thread pool
    summarize(&big_buffer)
});
```

Caveats from the docs: inside `install`, the *calling* thread's thread-local data isn't
visible; and if the calling thread already belongs to a different pool, it yields to that
pool while waiting (which can let unrelated work run there).

## Introspection / free functions

```rust
rayon::current_num_threads() -> usize          // threads in the current pool (or global if outside one)
rayon::current_thread_index() -> Option<usize> // this worker's index, or None if not a rayon thread
rayon::max_num_threads() -> usize              // hard ceiling for one pool (target-dependent)
```

`current_num_threads` is what the parallel iterators use to decide split granularity. Use
it to size a per-thread buffer pool. `current_thread_index` returns `None` off the pool —
handy to assert you're (not) running where you expect.

## Errors

`ThreadPoolBuildError` implements `Error`/`Display`/`Debug`. In the binary, propagate with
`?`/`anyhow` or `expect` at startup; a library that builds a pool should surface it as a
`thiserror` variant rather than panic (see [[pixhaus-rust-conventions]], [[pixhaus-thiserror]]).

## When to build a pool in Pixhaus

- **Default global pool** for canvas/pixel work — simplest, correct, sized to the machine.
- **A capped pool** if you want to reserve a core for the egui/render thread so a heavy
  filter never makes the UI janky: `num_threads(num_cpus - 1)`.
- **A named pool with a panic handler** for long verbs, so a panic in a filter logs and
  the app survives instead of aborting.
- Don't spin up a pool per operation — building threads is expensive. Build once, reuse
  via `install`.
