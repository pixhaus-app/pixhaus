# Storing a lazily-built, globally-shared default palette

## Short answer

Use a `static` backed by **`std::sync::LazyLock`**, with the closure doing the
expensive build from the embedded asset:

```rust
use std::sync::LazyLock;

static DEFAULT_PALETTE: LazyLock<Palette> = LazyLock::new(|| {
    // Embedded at compile time — no I/O, no path lookup at runtime.
    const RAW: &[u8] = include_bytes!("../assets/default_palette.bin");
    Palette::decode(RAW)
        .expect("embedded default palette is a build-time invariant")
});
```

Read it from anywhere, on any thread, by deref:

```rust
pub fn default_palette() -> &'static Palette {
    &DEFAULT_PALETTE
}

// in a worker thread, a panel, the renderer — all fine:
let p: &Palette = default_palette();
let first = p.colors().first();
```

That single declaration satisfies every requirement in the question:

- **Computed once** — `LazyLock` runs the closure exactly once, on the first
  access, and caches the result for the program's lifetime.
- **Not built at startup if unused** — initialization is deferred to first
  touch. If no code path reads `DEFAULT_PALETTE`, the closure never runs, so you
  pay nothing for the expensive build.
- **Read from multiple threads** — `LazyLock<T>` is `Sync` when `T: Sync`, and
  the one-time init is internally synchronized: concurrent first-callers block
  until the winner finishes, then all get the same `&'static T`. No data race,
  no double-build.
- **Cheap to read after init** — every access after the first is a relaxed
  atomic load plus a deref. No lock is taken on the hot path.

## Why `LazyLock` specifically

`LazyLock` is in `std` (stabilized in Rust 1.80), so no extra dependency. It is
purpose-built for "global, initialized once, on first use." The two properties
that matter here:

1. **Laziness** is the whole point of the question. A plain `static` requires a
   `const` initializer — you can't run an expensive decode in it. `LazyLock`
   lets the initializer be arbitrary code that runs at first access.
2. **`&'static` access** means call sites just borrow. No cloning, no reference
   counting, no lifetimes to thread through APIs. A function returning
   `&'static Palette` is the simplest possible signature for "read it from all
   over the app."

The data is immutable after build, so you want a *shared reference*, not shared
ownership. `&'static T` is exactly that and costs nothing to hand around.

### One real constraint: `T: Sync`

Because the static is shared across threads, `Palette` must be `Sync` (and
`Send` is implied for a `'static` value used this way). A palette is almost
certainly a `Vec` of color structs, which is `Sync`. The trap is interior
mutability: if the palette ever held a `Cell`, `RefCell`, or a non-atomic
counter, it would not be `Sync` and this wouldn't compile. Keep the cached value
plain immutable data — which is what a *default* palette is.

### Panics in the initializer

The closure here uses `.expect(...)`. That is deliberate and correct for an
*embedded* asset: a malformed `include_bytes!` blob is a build-time bug, not a
runtime condition, so failing loud is right. Note that in this repo the
no-`unwrap`/no-`panic` rule is clippy-enforced, so gate it with an
`#[allow(clippy::expect_used)]` and a comment explaining it's a compile-time
invariant — or, cleaner, move the parse into a `#[test]` that asserts the
embedded bytes decode, and keep `expect` as the belt-and-suspenders guard. If
the source could genuinely fail at runtime (it can't, since it's embedded), you
would not use `LazyLock` at all — see the rejects below.

## What I'd reject

- **`once_cell::sync::Lazy`** — the crate that `LazyLock` was lifted from. Fine,
  but now redundant: prefer the `std` version and drop the dependency. Only
  reach for `once_cell` if you're pinned to a pre-1.80 toolchain, which this
  workspace is not.

- **`lazy_static!`** — the old macro. Works, but it's a macro wrapping the same
  idea, predates `LazyLock`, and produces a deref-newtype that's slightly
  awkward in error messages. No reason to add it today.

- **`static mut` + manual init** — requires `unsafe` to touch, which is
  forbidden workspace-wide, and you'd hand-roll the once-only synchronization
  that `LazyLock` already gives you correctly. Hard no.

- **`Arc<Palette>` cloned around the app** — wrong tool for *immutable global*
  data. `Arc` is for shared *ownership* with a runtime-determined lifetime; here
  the lifetime is the whole program, so `&'static` is both simpler and cheaper
  (no atomic refcount bumps on every clone). Use `Arc` only if the palette's
  identity must change at runtime (e.g. the user swaps the active palette) — but
  that's a different object than the built-in default, which never changes.

- **`OnceLock` with an explicit `get_or_init` call site** — `OnceLock` is the
  right primitive when the *initializing value* isn't known at the definition
  site (e.g. it depends on runtime config). Here the build is a pure function of
  an embedded constant, so the init closure belongs with the declaration, and
  `LazyLock` is `OnceLock` + that closure bundled together. Use `LazyLock`.
  (`lazy_static`/`once_cell` would be the same call; `LazyLock` wins on being in
  `std`.)

- **`thread_local!`** — defeats the requirement. You'd build the palette *once
  per thread*, paying the expensive cost N times and storing N copies of
  identical immutable data. Thread-local is for per-thread *mutable* scratch, not
  shared read-only globals.

- **Plain `const` / `static` with a `const fn` builder** — only works if the
  build is `const`-evaluable, and an expensive decode of a binary blob generally
  isn't (and even if it were, you'd lose the laziness — `const` data is baked at
  compile time, so "don't build it if unused" no longer applies the way you
  want). `LazyLock` is the move.

## Summary

`static DEFAULT_PALETTE: LazyLock<Palette> = LazyLock::new(|| ...)`, read via a
`-> &'static Palette` accessor. `std`-only, lazy (skips the build when unused),
thread-safe, single build, zero-cost reads after init. Reject `Arc`-cloning
(ownership you don't need), `static mut`/`unsafe` (forbidden and unnecessary),
`thread_local!` (rebuilds per thread), and the third-party once-init crates
(superseded by `std`).
