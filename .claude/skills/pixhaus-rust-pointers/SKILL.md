---
name: pixhaus-rust-pointers
description: >
  Use when deciding which pointer, smart pointer, or interior-mutability container to
  reach for in Pixhaus Rust code — a plain `&T` / `&mut T` borrow, `Box<T>` for a
  recursive or heterogeneous type, `Rc` vs `Arc` for shared ownership, `Cell` / `RefCell`
  vs a lock for mutation through a shared handle, `Mutex` / `RwLock` for cross-thread
  state, or `OnceLock` / `LazyLock` for a lazily-initialized static. Trigger this for ANY
  "should this be Arc or Rc", "how do I share this across threads / tasks", "wrap it in
  Arc<Mutex>", "I need a recursive enum / tree / linked list", "interior mutability",
  "RefCell already borrowed (it panics)", "lazy static config", "global registry",
  "this needs &mut but I only have &", "is this type Send / Sync", or "can I use a raw
  pointer here" question, even when the user names no specific type. The default in
  Pixhaus is plain ownership with `&` / `&mut` and a single owner per piece of state — a
  heap pointer or a lock is a deliberate exception you justify, not a reflex. Raw pointers
  (`*const T` / `*mut T`) need `unsafe`, which the workspace forbids, so this skill also
  tells you what to use instead. For the deeper async/lock rules (lock-across-await,
  spawn_blocking, channels) see [[pixhaus-tokio]] and [[pixhaus-rust-conventions]]; this
  skill is the front door that picks the container.
---

# Pointers and shared state in Pixhaus

Higher-level languages hide this decision: Python and JavaScript make everything a
reference, garbage-collect the lifetimes, and you never choose. Rust makes the choice
explicit, and the choice carries real cost — a heap allocation, an atomic refcount, a
lock's contention, a panic on a double borrow. This skill is the decision procedure:
given what you're trying to do, which container do you reach for, and which ones are a
reflex you should resist.

The Pixhaus default is the cheapest thing that works: a value owned outright, lent out
as `&T` to readers and `&mut T` to the one writer. Every box, refcount, cell, and lock
past that is an exception you justify against a concrete need — shared ownership, a
recursive shape, mutation through a shared handle, cross-thread access. Agents reach for
`Arc<Mutex<T>>` the moment they see the word "share"; that reflex is wrong far more often
than it's right (see [[pixhaus-rust-conventions]] on premature `Arc<Mutex<>>`).

## Pick by what you're doing

Start here. Find the row that matches your situation; the sections below explain each.

| You want to… | Reach for | Not |
|---|---|---|
| Let several readers see a value | `&T` | `Rc`, clone |
| Let exactly one writer mutate it | `&mut T` | `RefCell`, a lock |
| Heap-allocate a recursive type (tree, AST, linked node) | `Box<T>` | `Rc` "to be safe" |
| Store mixed concrete types behind one trait | `Box<dyn Trait>` in a `Vec` | a giant enum, generics |
| Move a big value to the heap to keep a struct small | `Box<T>` | leaving it inline |
| Share ownership across threads / tasks, read-only | `Arc<T>` | `Arc<Mutex<T>>` |
| Share *and mutate* across threads, short critical section | `Arc<parking_lot::Mutex<T>>` | `std::sync::Mutex` |
| Share across threads, many readers + rare writer | `Arc<parking_lot::RwLock<T>>` | `Mutex` |
| Share *and mutate* across an `.await` point | `Arc<tokio::sync::Mutex<T>>` | parking_lot across await |
| Per-key concurrent access (cache, registry) | `dashmap::DashMap` | `RwLock<HashMap>` |
| Lazily initialize a `static` once, then read forever | `LazyLock<T>` (or `OnceLock<T>`) | `Mutex<Option<T>>` |
| Get a result from a background task to the UI | a channel (see [[pixhaus-tokio]]) | `Arc<Mutex<Option<T>>>` |
| Touch raw memory / FFI | nothing — `unsafe` is forbidden | `*const T` / `*mut T` |

Two notes the table can't carry: `Rc` and the single-threaded cells (`Cell`, `RefCell`,
`OnceCell`, `LazyCell`) almost never appear in Pixhaus, and there's a reason for each —
covered below. And the document itself, the central piece of mutable state, is none of
these: it's owned directly by the egui loop and mutated through `&mut self`. No pointer
at all (see "The document needs no pointer").

## References: the default, and they're free

`&T` is a shared borrow — any number of readers, no mutation, no allocation, no refcount.
`&mut T` is an exclusive borrow — exactly one writer at a time, also free. The borrow
checker proves at compile time that you never have a writer alongside any other accessor,
which is why these need no runtime machinery. Reach for them first and almost always.

```rust
// Many readers borrow the buffer; none of them can mutate it.
fn checksum(buf: &PixelBuffer) -> u32 { /* read pixels */ }

// One writer borrows it exclusively; the compiler forbids a second accessor meanwhile.
fn invert(buf: &mut PixelBuffer) { /* flip every pixel */ }
```

If a borrow gives you a fight — "cannot borrow as mutable", "already borrowed" — the
honest fix is usually to restructure ownership (hoist the borrow, split the struct, take
by value and return), not to escape into a `RefCell` or a clone. Cloning a `PixelBuffer`
is KBs to MBs; reaching for it to dodge the borrow checker is the mistake
[[pixhaus-rust-conventions]] calls out by name.

`&mut T` is not `Send`-hostile, but it is not `Send` itself — you cannot hand a `&mut T`
to another thread. That's the point: exclusive mutation is a single-thread idea. When you
genuinely need mutation visible across threads, that's a lock, below.

## Box: heap allocation with a single owner

`Box<T>` owns one heap allocation. Three reasons to reach for it, and they're the only
common ones in Pixhaus:

1. **Recursive types.** A type can't contain itself by value — the size would be
   infinite. Box breaks the cycle by putting the recursive arm behind a pointer of known
   size. This is the right tool for a node tree, an expression AST, a layer-group
   hierarchy:

   ```rust
   enum LayerNode {
       Leaf(LayerId),
       Group(Vec<LayerNode>),          // Vec is already heap-backed — no Box needed here
       Masked(Box<LayerNode>, MaskId), // a single recursive child does need the Box
   }
   ```

   Note `Vec<T>`, `String`, and the like are already heap-allocated and owned; don't wrap
   them in `Box` "for the heap". The Box earns its place only for a *single* recursive or
   large value.

2. **Heterogeneous trait objects in a collection.** When a list holds several concrete
   types that share a trait, `Vec<Box<dyn Trait>>` is the idiomatic store — a verb
   registry, a stack of effects. This is the *one* place `Box<dyn Trait>` is right;
   for a single function parameter, use `&impl Trait` instead (the `Box<dyn Trait>`
   overuse trap in [[pixhaus-rust-conventions]]).

   ```rust
   let verbs: Vec<Box<dyn Verb>> = vec![Box::new(Fill), Box::new(Outline)];
   ```

3. **Shrinking a large, rarely-used variant.** An enum is as big as its largest variant.
   If one arm is huge and uncommon, boxing it keeps every value of the enum small. Only
   do this with a size measurement in hand, not on a hunch.

## Rc and Arc: shared ownership

Use these only when a value has **no single owner** — when several independent holders
must each keep it alive and you genuinely can't express that with a borrow or by parking
the value with one owner and lending references.

- **`Rc<T>`** is a single-threaded reference count. It is `!Send` and `!Sync`, so it
  cannot cross a thread boundary. In Pixhaus it almost never appears: the place you'd
  reach for it — shared graph nodes, a parent pointer — is single-threaded UI state that
  usually has a cleaner owner (an arena, an index/`LayerId` instead of a pointer). If you
  think you need `Rc`, first ask whether an id-into-a-`Vec` models the same graph without
  the refcount. Reach for `Rc` only when that genuinely doesn't fit.

- **`Arc<T>`** is the atomic, thread-safe reference count: `Send + Sync` when `T: Send +
  Sync`. This is the one you actually use, and the common shape is **read-only shared
  data**: an `Arc<[Pixel]>` handed to several worker tasks, an `Arc<Palette>` shared
  across a render and the UI. Plain `Arc<T>` gives shared *reads*; it does not let you
  mutate the inside. The moment you need shared mutation, you add a lock — and that's the
  next section, the place to be most careful.

```rust
// Read-only fan-out: clone the Arc (cheap — just bumps the count), move it into each task.
let shared: Arc<[Pixel]> = pixels.into();
for region in regions {
    let shared = Arc::clone(&shared);
    rt.spawn_blocking(move || process(&shared, region));
}
```

`Arc::clone(&x)` copies the pointer and increments the count; it does **not** clone the
data. Prefer the explicit `Arc::clone(&x)` over `x.clone()` so readers see it's a refcount
bump, not a deep copy.

## Interior mutability: mutating through a shared handle

Interior mutability is for the case where you hold a *shared* reference (`&self`, an
`Arc`, an `Rc`) but still need to mutate what's inside. The container moves the
borrow-checking from compile time to runtime, or to an atomic operation. Which one
depends on whether the sharing crosses threads — and getting that wrong is where the
real bugs are.

### Single-threaded: Cell and RefCell — and why they're rare here

- **`Cell<T>`** holds a `Copy` value you swap wholesale with `get` / `set`. No borrow, no
  panic, but no in-place mutation and `Copy`-only. `!Sync`.
- **`RefCell<T>`** allows in-place mutation of any `T` behind a `&`, enforcing the
  borrow rules at *runtime*: `borrow()` and `borrow_mut()` hand out guards, and a
  conflicting pair **panics**.

  ```rust
  let cell = RefCell::new(0u32);
  *cell.borrow_mut() += 1;            // fine
  let _read = cell.borrow();          // shared borrow held...
  let _write = cell.borrow_mut();     // PANIC: already borrowed
  ```

These barely appear in Pixhaus, by design. A `RefCell` panic is a borrow-checker error
deferred to runtime — exactly the class of failure the no-`unwrap`/no-`panic` rule
(see [[pixhaus-rust-conventions]]) exists to keep out of production paths. The document is
owned by the UI loop and mutated through `&mut self`, so the usual reason to reach for
`RefCell` — "I'm behind `&self` and need to mutate" — usually means the ownership is
wrong, not that you need a cell. Fix the ownership first. Reach for `Cell`/`RefCell` only
in a contained, single-threaded structure where the borrow pattern is genuinely dynamic
and you can prove the borrows can't overlap; if you can't prove it, it'll panic in front
of a user.

### Cross-thread: Mutex and RwLock

When the mutation must be visible across threads or tasks, the container is a lock, and in
Pixhaus that means `parking_lot` for synchronous code:

- **`Mutex<T>`** — one accessor at a time, reader or writer. `Send + Sync` when `T: Send`.
- **`RwLock<T>`** — many concurrent readers *or* one writer. Use it when reads dominate
  and writes are rare (a backend registry the UI reads constantly and a config flow writes
  occasionally).

A lock is shared, so it travels inside an `Arc`: `Arc<parking_lot::RwLock<BackendRegistry>>`.
This is the *legitimate* `Arc<lock<>>` — genuinely shared mutable state across the UI
thread and background tasks — as opposed to the reflexive `Arc<Mutex<>>` wrapped around
data that has one owner.

Two rules carry over from [[pixhaus-rust-conventions]] and [[pixhaus-tokio]], and they're
non-negotiable:

- **Prefer `parking_lot` over `std::sync`** for sync locks — faster, no poisoning, and
  the guard API is cleaner. See [[pixhaus-parking-lot]].
- **Never hold a lock across `.await`.** A `parking_lot`/`std` guard is `!Send`, so a
  future holding one is `!Send` and `tokio::spawn` rejects it; even where it compiles you'd
  serialize the runtime and risk deadlock. If — and only if — you must hold state locked
  across an await, that's the single job of `tokio::sync::Mutex`, whose guard is `Send`.
  Usually the better fix is to drop the lock before the await. [[pixhaus-tokio]] has the
  full footgun.

For per-key concurrent access — a tile cache, a thumbnail map keyed by id — a single
`RwLock<HashMap>` serializes every access through one lock; `dashmap::DashMap` shards it
and outperforms (also in [[pixhaus-rust-conventions]]).

## One-time and lazy initialization

For a value computed once and then read forever — a static config, a compiled regex, a
lookup table:

| Need | Single-thread | Thread-safe (what you want for a `static`) |
|---|---|---|
| Set once, then read | `OnceCell<T>` | `OnceLock<T>` |
| Initialize lazily on first access via a closure | `LazyCell<T>` | `LazyLock<T>` |

In Pixhaus, a shared-across-threads `static` is the common case, so reach for the `*Lock`
forms. `LazyLock` is the ergonomic default: give it a closure, and the first access runs
it and caches the result; every access after is a plain read.

```rust
use std::sync::LazyLock;

static DEFAULT_PALETTE: LazyLock<Palette> = LazyLock::new(|| {
    Palette::load_builtin("dawnbringer-32")
});

// First read builds it; the rest just borrow.
let pal = &*DEFAULT_PALETTE;
```

Use `OnceLock` when initialization isn't a self-contained closure — when something else
decides the value and calls `set` once (`get_or_init` covers the lazy case without a
`static` initializer). Both replace the old `lazy_static!` / `once_cell` patterns;
they're in `std`, so don't add a crate for this.

The `*Cell` (single-thread) versions exist for completeness; you'd only pick one inside a
structure that is itself single-threaded and never shared across threads, which is rare
here for the same reason `Rc` is.

## Raw pointers: forbidden, here's the alternative

`*const T` and `*mut T` exist, and every dereference of one requires an `unsafe` block.
The Pixhaus workspace **forbids `unsafe`** (`#![forbid(unsafe_code)]` workspace-wide), so
raw pointers are off the table — code that dereferences one will not compile, and you
should not try to make it.

What people actually want when they reach for a raw pointer:

- **FFI / a C library** — wrap it in a vetted, safe crate from the ecosystem; don't hand-roll
  the `extern` block. If a binding genuinely needs `unsafe`, that's a maintainer
  conversation, not an inline `#[allow]`.
- **"Cast these bytes to that type"** for GPU upload or pixel reinterpretation — that's
  `bytemuck`'s safe `Pod`/`Zeroable` casts, not a pointer cast. See [[pixhaus-bytemuck]].
- **A self-referential or graph structure** — model it with indices into a `Vec` (an
  arena), not back-pointers. Same shape, no `unsafe`, no aliasing hazard.

If you find a case you believe truly needs `unsafe`, stop and escalate to a maintainer per
[[pixhaus-rust-conventions]]; don't weaken the lint.

## The document needs no pointer

The most important piece of mutable state in the editor — the open document — is owned
directly by the egui app struct and mutated through `&mut self` in the update loop. No
`Box`, no `Rc`, no `Arc`, no lock. One thread touches it, so the borrow checker alone
keeps it sound, and there's a single source of truth for the undo stack (a second copy
behind an `Arc` would desync undo — see [[pixhaus-rust-conventions]]).

Background work doesn't get a shared handle to the document. It receives a *copy of the
slice it needs* moved into the spawned closure, and returns its result over a channel that
the loop drains each frame; the loop applies the change through `&mut self`. That pattern
— copy in, channel out — is in [[pixhaus-tokio]], and it's why so little of Pixhaus needs
a pointer past `&` / `&mut` at all.

## Send and Sync, briefly

A pointer is thread-safe only if the data behind it is, and the compiler tracks this with
two auto-traits: **`Send`** (the value can move to another thread) and **`Sync`** (`&T`
can be shared across threads — equivalently, `&T: Send`). You rarely implement them; you
just need to know which containers carry them, because that's what `tokio::spawn`'s
`Send + 'static` bound checks.

| Container | `Send`? | `Sync`? | Consequence |
|---|---|---|---|
| `&T` / `&mut T` | if `T` is | `&T` if `T: Sync` | borrow rules, compile-time |
| `Box<T>` | if `T: Send` | if `T: Sync` | acts like the `T` it owns |
| `Rc<T>` | no | no | single-thread only — won't enter a task |
| `Arc<T>` | if `T: Send + Sync` | if `T: Send + Sync` | the cross-thread share |
| `Cell` / `RefCell` | if `T: Send` | **no** | can move, can't *share* across threads |
| `parking_lot::Mutex`/`RwLock` | if `T: Send` | if `T: Send` | the way to make `T` shareable+mutable |
| `OnceLock` / `LazyLock` | if `T: Send` | if `T: Send + Sync` | usable in a `static` |
| `*const T` / `*mut T` | no | no | moot — forbidden anyway |

The practical reading: if `tokio::spawn` complains your future isn't `Send`, something
inside it is `Rc`, a `RefCell`, or a `parking_lot` guard held across the await. The fix is
almost never to force `Send` — it's to pick the right container from the rows above, or to
restructure so the non-`Send` thing never crosses the await.

## When in doubt

- "Should this be shared?" — Default no. One owner, lend `&`/`&mut`. Add an `Arc` only
  when several holders must keep it alive independently.
- "`Arc<Mutex>` or just `Arc`?" — `Arc` alone if the shared data is read-only. Add the
  lock only when shared *mutation* is real. Most "shared" data is read-only fan-out.
- "`Mutex` or `RwLock`?" — `RwLock` when reads vastly outnumber writes; `Mutex` otherwise.
  Don't reach for `RwLock` reflexively — under write-heavy load it's slower than a `Mutex`.
- "Can I use `RefCell` to get around the borrow checker?" — Almost never. A `RefCell`
  trades a compile error for a runtime panic. Fix the ownership instead.
- "Raw pointer for performance?" — No. The workspace forbids `unsafe`. Indices, `bytemuck`,
  or a safe crate cover every legitimate case; a genuine exception is a maintainer call.
