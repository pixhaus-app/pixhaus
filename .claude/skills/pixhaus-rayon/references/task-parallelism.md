# rayon 1.12.0 — task parallelism: join, scope, spawn, broadcast

The lower-level fork-join layer beneath the parallel iterators. Reach for it when the
work doesn't fit a `par_iter` chain: divide-and-conquer recursion (`join`), a loop that
spawns a dynamic number of tasks (`scope`), fire-and-forget background work (`spawn`), or
running something on every worker thread (`broadcast`). All re-exported at the `rayon`
root.

## The work-stealing model

Rayon runs a fixed pool of worker threads (CPU count by default). A "potentially
parallel" call like `join(a, b)` runs `a` on the current thread and advertises `b` in a
local queue; if another thread is idle, it *steals* `b` and runs it in parallel — if not,
the current thread runs `b` itself after `a`. So parallelism is opportunistic and nearly
free when no core is idle. This is why per-task size matters: a task too small to be worth
stealing just adds bookkeeping.

**LIFO vs FIFO.** The current thread takes its *most recently* spawned task first (LIFO,
good for cache locality); other threads steal the *oldest* (FIFO). The `_fifo` variants
make same-thread ordering first-in-first-out within their scope — use them when earlier
tasks should start first (e.g. latency-sensitive ordering), not for raw throughput.

## `join` — fork two

```rust
pub fn join<A, B, RA, RB>(oper_a: A, oper_b: B) -> (RA, RB)
where A: FnOnce() -> RA + Send, B: FnOnce() -> RB + Send, RA: Send, RB: Send

pub fn join_context<A, B, RA, RB>(oper_a: A, oper_b: B) -> (RA, RB)
where A: FnOnce(FnContext) -> RA + Send, B: FnOnce(FnContext) -> RB + Send, RA: Send, RB: Send
```

Runs two closures potentially in parallel, returns both results, **blocks until both
finish**. Both always execute. Stack-allocated — no heap, no `'static` requirement, so
closures can borrow from the surrounding scope. The canonical divide-and-conquer shape:

```rust
fn sum_tree(node: &Node) -> u64 {
    match node {
        Leaf(v) => *v,
        Branch(l, r) => { let (a, b) = rayon::join(|| sum_tree(l), || sum_tree(r)); a + b }
    }
}
```

`join_context` passes each closure an `FnContext` whose `.migrated()` is `true` when the
closure ran on a different thread than the caller (i.e. `b` was stolen) — use it to decide
whether further splitting is worthwhile. **Panics propagate**: if a closure panics, `join`
re-panics with that value once the other side settles; if both panic, the first's value
wins.

## `scope` — spawn a dynamic number

```rust
pub fn scope<'scope, OP, R>(op: OP) -> R
    where OP: FnOnce(&Scope<'scope>) -> R + Send, R: Send
pub fn scope_fifo<'scope, OP, R>(op: OP) -> R
    where OP: FnOnce(&ScopeFifo<'scope>) -> R + Send, R: Send
pub fn in_place_scope<'scope, OP, R>(op: OP) -> R         // body runs on the CALLING thread
    where OP: FnOnce(&Scope<'scope>) -> R                  // (no Send/R: Send bound)
pub fn in_place_scope_fifo<'scope, OP, R>(op: OP) -> R
    where OP: FnOnce(&ScopeFifo<'scope>) -> R
```

```rust
impl<'scope> Scope<'scope> {
    fn spawn<BODY>(&self, body: BODY)            where BODY: FnOnce(&Scope<'scope>) + Send + 'scope;
    fn spawn_broadcast<BODY>(&self, body: BODY)  where BODY: Fn(&Scope<'scope>, BroadcastContext) + Send + Sync + 'scope;
}
impl<'scope> ScopeFifo<'scope> {
    fn spawn_fifo<BODY>(&self, body: BODY)       where BODY: FnOnce(&ScopeFifo<'scope>) + Send + 'scope;
    fn spawn_broadcast<BODY>(&self, body: BODY)  where BODY: Fn(&ScopeFifo<'scope>, BroadcastContext) + Send + Sync + 'scope;
}
```

`scope` creates a region, hands you `&Scope`, and **blocks until every task spawned into
it finishes**. Unlike `join`, spawned tasks are heap-allocated, so a loop can spawn an
unbounded number without recursion. Tasks may borrow data that outlives the scope
(`'scope`). The scope body runs *in the pool* (affects thread-locals); `in_place_scope`
runs the body on your calling thread and sends only the spawned tasks to the pool — handy
when the body itself shouldn't migrate. Panic semantics match `join`; all spawned tasks
still run, and which panic propagates is unspecified when several panic.

```rust
rayon::scope(|s| {
    for tile in dirty_tiles {
        s.spawn(move |_| process(tile));   // one task per tile, all joined at scope end
    }
});
```

## `spawn` — fire and forget

```rust
pub fn spawn<F>(func: F)       where F: FnOnce() + Send + 'static;
pub fn spawn_fifo<F>(func: F)  where F: FnOnce() + Send + 'static;
```

Queues a task in the global scope that is **not** joined — it runs whenever a worker is
free and may outlive the calling frame, hence `'static` (usually a `move` closure that
owns its data, e.g. holds an `Arc`). A panic with no join to catch it goes to the
`ThreadPoolBuilder::panic_handler` (default: abort the process). Use for detached
background work whose result you don't await; for results, prefer a channel.

## `broadcast` — run on every thread

```rust
pub fn broadcast<OP, R>(op: OP) -> Vec<R>            // blocks; collects one R per thread
    where OP: Fn(BroadcastContext) -> R + Sync, R: Send
pub fn spawn_broadcast<OP>(op: OP)                  // detached version
    where OP: Fn(BroadcastContext) + Send + Sync + 'static

pub struct BroadcastContext<'a> { /* .. */ }
impl BroadcastContext<'_> {
    fn index(&self) -> usize;        // 0..num_threads
    fn num_threads(&self) -> usize;
}
```

Runs `op` once on each thread of the current pool. Use to initialize thread-local state or
collect per-thread results. Rarely needed for pixel work.

## Cooperative yielding and context

```rust
pub fn yield_now() -> Option<Yield>     // run one pending item (may steal); None if not in a pool
pub fn yield_local() -> Option<Yield>   // run one item from THIS thread's queue only (no steal)
pub enum Yield { Executed, Idle }       // Executed = work ran, Idle = none found

pub struct FnContext { /* Copy */ }
impl FnContext { fn migrated(&self) -> bool; }
```

`yield_now`/`yield_local` let a thread that's about to block do useful pool work first.
Note: unlike `std::thread::yield_now`, they do *not* yield to the OS scheduler — in a
polling loop that returns `Idle`, OS-yield separately.

## Do / don't (Pixhaus)

- **Use `join` for divide-and-conquer** (recursive region split, quad-tree fill) and
  `scope` when a loop spawns a variable number of tasks over borrowed data. For flat
  data-parallel work over a slice, a `par_iter`/`par_chunks_mut` chain is simpler and
  usually faster — don't hand-roll with `join`.
- **All of `join`/`scope` block the calling thread.** Same rule as the parallel
  iterators: don't call them on the egui frame thread or inside a tokio task. Run the
  heavy verb on a background thread that uses them internally and posts the result over a
  channel (see [[pixhaus-tokio]]).
- **Never hold a lock across a `join`/`scope` call** — the same hazard as holding a lock
  across `.await`. A stolen task could try to take the same lock and deadlock.
- **Don't block or do I/O inside these closures.** A worker stuck on I/O starves the pool.
  CPU-bound pixel math is the intended use.
