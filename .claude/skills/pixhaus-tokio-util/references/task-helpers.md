# tokio_util::task — managing a fleet of spawned tasks

Feature: **`rt`** for `TaskTracker`, `AbortOnDropHandle`, `LocalPoolHandle`,
`JoinQueue`. **`join-map`** (which pulls in `rt` + `hashbrown`) for `JoinMap`.
tokio-util 0.7.18. No free `spawn_pinned` function and no module-level `Spawn`
trait — `spawn_pinned` is a method on `LocalPoolHandle`.

## The drop-behavior cheat sheet (the thing people get wrong)

| Type | On drop | `wait`/await semantics |
|---|---|---|
| `tokio::task::JoinHandle` | **detaches** (task keeps running) | `.await` the handle for its output |
| `TaskTracker` | **detaches** — tasks keep running | `wait()` resolves only after `close()` AND empty |
| `AbortOnDropHandle` | **aborts** the task | `.await` it directly (`Future<Output = Result<T, JoinError>>`) |
| `JoinMap` | **aborts all** its tasks | `join_next().await` for `(key, result)` |

Pick by intent: graceful drain at shutdown → `TaskTracker`; task dies with its
owner → `AbortOnDropHandle`; keyed "latest wins" set → `JoinMap`.

---

## TaskTracker

> Tracks spawned tasks and lets you await until all of them exit.

```rust
pub fn new() -> Self

pub fn spawn<F>(&self, task: F) -> JoinHandle<F::Output>
    where F: Future + Send + 'static, F::Output: Send + 'static
pub fn spawn_on<F>(&self, task: F, handle: &Handle) -> JoinHandle<F::Output> /* same bounds */
pub fn spawn_blocking<F, T>(&self, task: F) -> JoinHandle<T>
    where F: FnOnce() -> T + Send + 'static, T: Send + 'static
pub fn spawn_blocking_on<F, T>(&self, task: F, handle: &Handle) -> JoinHandle<T> /* same */
pub fn spawn_local<F>(&self, task: F) -> JoinHandle<F::Output> /* !Send ok, needs LocalSet */
pub fn spawn_local_on<F>(&self, task: F, local_set: &LocalSet) -> JoinHandle<F::Output>

// Track a future you run yourself (e.g. inside select!) without spawning it.
pub fn track_future<F: Future>(&self, future: F) -> TrackedFuture<F>

pub fn close(&self) -> bool       // true if THIS call flipped open -> closed
pub fn reopen(&self) -> bool      // true if THIS call flipped closed -> open
pub fn wait(&self) -> TaskTrackerWaitFuture<'_>

pub fn len(&self) -> usize
pub fn is_empty(&self) -> bool
pub fn is_closed(&self) -> bool
pub fn token(&self) -> TaskTrackerToken
pub fn ptr_eq(left: &TaskTracker, right: &TaskTracker) -> bool
```

The contract that trips everyone: **`wait()` resolves only when the tracker is
both `close()`d and empty.** If you never `close()`, `wait()` never returns. You
may `close()` then `reopen()`; a pending `wait()` future keeps waiting. `Clone`
shares the same tracker. Dropping a `TaskTracker` does **not** abort its tasks —
it only tracks.

```rust
use tokio_util::task::TaskTracker;

let tracker = TaskTracker::new();
for i in 0..10 {
    tracker.spawn(async move { work(i).await });
}
tracker.close();       // no more tasks will be added
tracker.wait().await;  // resolves once all 10 have exited
```

Pixhaus use: own one `TaskTracker` for the app's background work and drain it in
`eframe`'s `on_exit` (`close(); wait().await` inside a `block_on`/handle) so a
save or export in flight isn't dropped on quit. See `pixhaus-eframe`.

---

## AbortOnDropHandle\<T\>

> Wraps a `JoinHandle<T>` and aborts the task when the wrapper is dropped.

```rust
pub fn new(handle: JoinHandle<T>) -> Self
pub fn abort(&self)
pub fn is_finished(&self) -> bool
pub fn abort_handle(&self) -> AbortHandle
pub fn detach(self) -> JoinHandle<T>    // recover the inner handle, disarming abort-on-drop
```

Impls: `Future<Output = Result<T, JoinError>>` (await it directly),
`AsRef<JoinHandle<T>>`, `Debug`, `Drop`. The inverse of a bare `JoinHandle`
(which detaches on drop). Tie a task's life to a parent value:

```rust
use tokio_util::task::AbortOnDropHandle;

struct PreviewPanel {
    // the render task is aborted automatically when the panel is dropped
    _render: AbortOnDropHandle<()>,
}

impl PreviewPanel {
    fn new() -> Self {
        let handle = tokio::spawn(async { /* re-render loop */ });
        Self { _render: AbortOnDropHandle::new(handle) }
    }
}
```

---

## LocalPoolHandle

> Cloneable handle to a pool of worker threads, each running a `LocalSet`, for
> spawning `!Send` tasks.

```rust
pub fn new(pool_size: usize) -> LocalPoolHandle   // panics if pool_size == 0
pub fn num_threads(&self) -> usize
pub fn get_task_loads_for_each_worker(&self) -> Vec<usize>

pub fn spawn_pinned<F, Fut>(&self, create_task: F) -> JoinHandle<Fut::Output>
    where F: FnOnce() -> Fut + Send + 'static,
          Fut: Future + 'static,            // the future itself need NOT be Send
          Fut::Output: Send + 'static
pub fn spawn_pinned_by_idx<F, Fut>(&self, create_task: F, idx: usize) -> JoinHandle<Fut::Output>
    /* same bounds, forces a specific worker index */
```

`spawn_pinned` is how you run a `!Send` future on a multi-thread runtime: the
*closure* that builds the future must be `Send + 'static` (it crosses thread
boundaries to reach the chosen worker), but the future need not be. Output must
be `Send` to return over the `JoinHandle`. The task is pinned to its worker and
never migrates. `spawn_pinned` picks the least-loaded worker; `_by_idx` forces
one. Pixhaus rarely needs this — most background work is `Send`. Reserve it for a
genuinely `!Send` dependency.

---

## JoinMap\<K, V, S = RandomState\>

> A set of tasks keyed by a hash key; `join_next` returns each result with its
> key. Features: `rt` + `join-map`.

```rust
pub fn new() -> Self                         where K: Hash + Eq, V: 'static
pub fn with_capacity(capacity: usize) -> Self
pub fn with_hasher(hash_builder: S) -> Self  where S: BuildHasher
pub fn with_capacity_and_hasher(capacity: usize, hash_builder: S) -> Self

// spawning (require K: Hash + Eq, S: BuildHasher)
pub fn spawn<F>(&mut self, key: K, task: F)
    where F: Future<Output = V> + Send + 'static, V: Send
pub fn spawn_on<F>(&mut self, key: K, task: F, handle: &Handle) /* same */
pub fn spawn_blocking<F>(&mut self, key: K, f: F)
    where F: FnOnce() -> V + Send + 'static, V: Send
pub fn spawn_blocking_on<F>(&mut self, key: K, f: F, handle: &Handle) /* same */
pub fn spawn_local<F>(&mut self, key: K, task: F)  where F: Future<Output = V> + 'static
pub fn spawn_local_on<F>(&mut self, key: K, task: F, local_set: &LocalSet)

// awaiting — yields the key with each result; None when the map is empty
pub async fn join_next(&mut self) -> Option<(K, Result<V, JoinError>)>

// cancellation
pub fn abort<Q>(&mut self, key: &Q) -> bool   where Q: ?Sized + Hash + Eq, K: Borrow<Q>
pub fn abort_matching(&mut self, predicate: impl FnMut(&K) -> bool)
pub fn abort_all(&mut self)
pub async fn shutdown(&mut self)              // abort all + drain

// inspection
pub fn len(&self) -> usize
pub fn is_empty(&self) -> bool
pub fn capacity(&self) -> usize
pub fn contains_key<Q>(&self, key: &Q) -> bool   where Q: ?Sized + Hash + Eq, K: Borrow<Q>
pub fn contains_task(&self, task: &Id) -> bool
pub fn keys(&self) -> JoinMapKeys<'_, K, V>

// memory (verify exact where-clauses against rustdoc if used verbatim)
pub fn reserve(&mut self, additional: usize)
pub fn shrink_to_fit(&mut self)
pub fn shrink_to(&mut self, min_capacity: usize)
pub fn detach_all(&mut self)
```

The defining behavior: **spawning a key that already has a running task aborts
and replaces the old one.** "If a task previously existed in the `JoinMap` for
this key, that task will be cancelled and replaced with the new one." That's
exactly right for "only the latest request per key matters" — a live thumbnail or
layer preview that re-renders as the user drags a slider, keyed by layer id.

`join_next` always hands back which key finished; the `Result` is `Err` on panic
or abort (`JoinError::is_cancelled` / `is_panic`). `&mut self` for spawn/join —
`JoinMap` is single-owner, lives on the loop that drains it (fits the
drain-each-frame model). Dropping it aborts all tasks (built on `JoinSet`).

`JoinMapKeys<'a, K, V>`: iterator over current keys (arbitrary order), from
`keys()`.

---

## JoinQueue\<T\> (peripheral)

A FIFO queue of tasks spawned on the runtime — results come out in spawn order
rather than completion order. Feature `rt`. Rarely needed; reach for it only when
ordering of results must match submission.

## task_tracker submodule

Houses `TaskTracker` plus `TaskTrackerToken`, `TaskTrackerWaitFuture<'a>`, and
`TrackedFuture<F>` (returned by `track_future`).
