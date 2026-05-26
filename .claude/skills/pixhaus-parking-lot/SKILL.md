---
name: pixhaus-parking-lot
description: >
  Use when reaching for a lock in Pixhaus — any `Mutex`, `RwLock`, `Condvar`,
  `Once`, or `ReentrantMutex` over state that is genuinely shared across threads
  (the Tokio task pool, a `rayon`/`spawn_blocking` worker writing back to shared
  state, a cache or registry touched off the UI thread). parking_lot is the
  workspace's lock crate: smaller, faster, no poisoning, and — the part that
  matters most here — its guards are `!Send` by default, so holding one across
  `.await` is a compile error instead of a deadlock. Trigger this for ANY
  "should I use a Mutex / RwLock", "lock().unwrap() won't compile", "my future
  isn't Send", "Arc<Mutex<…>> here?", "read/write lock", "upgradable read",
  "lock poisoning", or "this lock deadlocks" question, and whenever you see
  `parking_lot::`, `.lock()`, `.read()`, `.write()`, or `Arc<Mutex<…>>` in the
  tree. Reach for this rather than std::sync or memory: parking_lot's API and
  guarantees differ from `std` in ways that change how you write the call site.
---

# parking_lot for Pixhaus

parking_lot reimplements `Mutex`, `RwLock`, `Condvar`, and `Once` to be smaller,
faster, and more flexible than `std::sync`. A `Mutex<T>` is one byte plus the
data, statically constructible, with an inline uncontended fast path and adaptive
spinning before it parks the thread. There is no poisoning: a panic while the lock
is held releases it normally instead of poisoning it.

It is the workspace lock crate. Use it everywhere you'd otherwise reach for
`std::sync::Mutex`/`RwLock`. But first re-read the rule it serves: per CLAUDE.md,
**every piece of mutable state has a single owner; avoid `Arc<Mutex<>>` except
where state is genuinely shared across threads.** parking_lot makes the lock
cheaper — it does not make the lock the right design. The egui update loop owns the
document directly and needs no lock at all (see `pixhaus-egui`,
`pixhaus-rust-conventions`). A lock is for state two threads actually touch.

## Version and license

| Crate | Version | License |
|---|---|---|
| `parking_lot` | 0.12 | `MIT OR Apache-2.0` |

The dual license includes MIT, so parking_lot passes the workspace MIT lock and
`cargo deny`. It's already in the tree transitively — egui-wgpu holds the renderer
as `Arc<RwLock<Renderer>>` (see `pixhaus-egui-wgpu`).

```toml
parking_lot = "0.12"
```

### Cargo features — none are on by default

| Feature | What it does | Use in Pixhaus |
|---|---|---|
| `send_guard` | Makes guards `Send` (removes the `!Send` guard rail) | **Don't.** The `!Send` default is the feature you want here — see below. |
| `deadlock_detection` | Runtime deadlock detector via `parking_lot_core` | Reasonable behind a debug/dev cfg when chasing a hang; not in release. |
| `arc_lock` | `lock_arc` / `read_arc` etc. returning `Arc`-bound guards | Only if a guard must outlive the borrow of an `Arc<Mutex<…>>`. |
| `serde` | Serialize/deserialize the inner `T` through the lock | Project files go through rmp-serde on owned data, not locks — unlikely. |
| `hardware-lock-elision`, `nightly` | Perf experiments needing nightly | Not on stable; out of scope. |

Default to plain `parking_lot = "0.12"`. Adding a feature is a deliberate call you
should be able to justify in the PR.

## The headline difference: guards are `!Send`, and that's the point

parking_lot's `MutexGuard`, `RwLockReadGuard`, and `RwLockWriteGuard` are **`!Send`
by default** (their raw locks carry `GuardNoSend`). A guard must be dropped on the
thread that acquired it. The practical consequence is the one that matters for this
codebase:

> A `!Send` guard held across an `.await` point makes the whole future `!Send`, so
> `tokio::spawn` **rejects it at compile time.**

That turns CLAUDE.md's "never hold a lock across `.await`" from a runtime deadlock
you'd hunt for into a borrow you can't even compile. Don't defeat it:

- **Never enable `send_guard`** to make an `await`-holding future compile. The
  error is correct — restructure so the guard drops before the await.
- The fix is always the same shape: copy/clone what you need out of the lock, drop
  the guard (end the scope or call `drop(guard)`), then `.await`.

```rust
// WRONG — guard is alive across .await; future is !Send, won't spawn.
let g = state.lock();
do_async(&g).await;            // compile error, by design

// RIGHT — take what you need, release, then await.
let snapshot = { state.lock().frame.clone() };  // guard dropped at `}`
do_async(&snapshot).await;
```

## No poisoning: `lock()` returns the guard, not a `Result`

`std::sync::Mutex::lock()` returns `LockResult` and the idiomatic-but-banned
`.lock().unwrap()` follows. parking_lot has no poisoning, so:

```rust
let mut g = mutex.lock();   // MutexGuard<T> directly — no Result, no .unwrap()
*g += 1;                    // Deref/DerefMut to T
```

This lines up with the workspace no-`unwrap`/no-`panic!` rule: there's nothing to
unwrap. If a panic happens while the lock is held, the data is *not* marked
poisoned — which means a panicked thread can leave the protected invariant
half-updated and the next locker sees it. Keep critical sections short and
panic-free; don't rely on poisoning to fence off corrupted state.

## The API you'll actually use

### Mutex

```rust
let m = parking_lot::Mutex::new(0u32);

m.lock();              // -> MutexGuard<T>, blocks
m.try_lock();          // -> Option<MutexGuard<T>>, never blocks
m.try_lock_for(dur);   // -> Option<MutexGuard<T>>, time-bounded
m.get_mut();           // -> &mut T, no locking — compiler proves exclusivity
m.is_locked();         // -> bool, advisory only (racy)
m.into_inner();        // -> T, consume the lock

// static / const context:
static COUNTER: parking_lot::Mutex<u32> = parking_lot::Mutex::new(0);
```

Guard helpers (from `lock_api`, available on the parking_lot alias):

- `MutexGuard::map(g, |t| &mut t.field)` -> `MappedMutexGuard` — narrow the guard to
  a field while holding the lock. `try_map` for the fallible case.
- `guard.unlocked(|| { … })` — temporarily release the lock to run a closure, then
  reacquire. Use to avoid holding the lock across an expensive unrelated call.
- `MutexGuard::unlock_fair(g)` and `guard.bump()` — see fairness below.

### RwLock

Many concurrent readers or one writer. Reach for it over `Mutex` only when reads
genuinely dominate and overlap; otherwise `Mutex` is simpler and often faster.

```rust
let rw = parking_lot::RwLock::new(state);

rw.read();              // -> RwLockReadGuard<T>, shared
rw.write();             // -> RwLockWriteGuard<T>, exclusive
rw.try_read();          // -> Option<…>
rw.try_write();         // -> Option<…>
rw.try_read_for(dur);   // time-bounded
rw.upgradable_read();   // -> RwLockUpgradableReadGuard<T>
rw.get_mut();           // -> &mut T, no locking
```

`upgradable_read()` is the "read now, maybe write" pattern done right: it's a read
lock that blocks other upgradable/write holders but lets plain readers in, and you
can promote it without releasing:

```rust
let g = rw.upgradable_read();
if needs_update(&g) {
    let mut w = parking_lot::RwLockUpgradableReadGuard::upgrade(g); // -> write guard
    *w = recompute();
}
```

Only one upgradable-read guard can exist at a time, which is what makes the
read→write promotion deadlock-free for that holder.

### The rest

- `Condvar` — `wait` / `wait_for` / `wait_until` / `notify_one` / `notify_all`.
  `wait` takes `&mut MutexGuard`, not the mutex.
- `Once` — one-time init; `call_once`. For most "init once" needs prefer
  `std::sync::OnceLock` / `LazyLock` unless you specifically want parking_lot's.
- `ReentrantMutex<T>` — same thread may lock recursively; gives `&T` (not `&mut T`)
  because reentrancy and `&mut` aliasing are incompatible. A reentrant mutex is
  usually a smell that ownership is tangled — prefer restructuring.
- `FairMutex` — always-fair `Mutex` variant (see fairness).

## Fairness: eventual by default, on demand when you need it

A normal `unlock` is *unfair* — the unlocking thread may immediately re-grab the
lock, which is fast but can starve a waiter. parking_lot applies **eventual
fairness**: roughly every 0.5 ms it hands the lock to a waiter instead. That's the
right default. Two escape hatches when a hot loop reacquires in a tight cycle and
you see a waiter starving:

- `MutexGuard::unlock_fair(g)` — drop this guard fairly, handing off to a waiter.
- `guard.bump()` — yield to a waiting thread mid-section, then reacquire; cheaper
  than `unlock_fair` + re-lock when nobody's waiting.

Reach for these only with evidence of starvation, not preemptively.

## Recursive read deadlock — the one RwLock footgun

`RwLock::read()` is **not** reentrant. If a thread holds a read guard and a writer
is queued, a second `read()` on the same thread deadlocks: the second read waits
behind the writer, the writer waits for the first read to drop, and the first read
waits for your stuck second read. `read_recursive()` skips the writer queue to
avoid that — but using it means writers can be starved indefinitely. Treat
`read_recursive` as a code smell pointing at a reentrant-read design; fix the
nesting instead of reaching for it.

## Rules that prevent the recurring bugs

- **Lock per genuinely-shared state, not by reflex.** Single-owner state on the
  egui thread needs no lock. A lock is for state crossing the Tokio/`spawn_blocking`
  boundary. See `pixhaus-rust-conventions`.
- **Keep critical sections tiny and panic-free.** No poisoning means a panic
  mid-update leaves the next locker reading a half-written value. Clone out, release,
  then do the slow/fallible work.
- **Never hold a guard across `.await`.** The `!Send` default makes this a compile
  error on spawned futures — keep it that way; don't reach for `send_guard`.
- **No `.unwrap()` on a lock** — there's no `Result`. `lock()`/`read()`/`write()`
  give the guard directly.
- **`Mutex` first, `RwLock` only when reads dominate and overlap.** An `RwLock`
  used like a `Mutex` is slower and adds the recursive-read footgun for nothing.
- **Use `get_mut()` when you have `&mut self`.** No atomic op, no contention — the
  borrow checker already proved exclusivity.
- **Don't pre-tune fairness.** Eventual fairness is fine until a profile shows
  starvation.

## Decision shortcut

```
Need shared mutable state behind a lock?
├─ Is the state actually owned by one thread (e.g. the egui loop)?
│    └─ yes → no lock. Own it directly. (pixhaus-egui / rust-conventions)
├─ Will a guard be alive across an `.await`?
│    └─ must not — clone out, drop the guard, then await. (!Send enforces this.)
├─ Do reads massively dominate and overlap, with rare writes?
│    ├─ yes → RwLock (upgradable_read for read-then-maybe-write)
│    └─ no  → Mutex
└─ One-time init only? → std OnceLock/LazyLock, or parking_lot::Once if you need it.
```
