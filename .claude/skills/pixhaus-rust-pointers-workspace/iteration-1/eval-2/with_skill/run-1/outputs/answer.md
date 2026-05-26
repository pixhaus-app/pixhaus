# Built-in default palette: use `LazyLock<Palette>`

## Recommended container

Store it as a `static LazyLock<Palette>`. Your three constraints line up exactly with what `LazyLock` does:

- **Computed once** — `LazyLock` runs its closure on the first access and caches the result; every access after is a plain read.
- **Read all over the app from multiple threads** — `LazyLock<T>` is `Sync` when `T: Send + Sync`, so the same `static` is readable from the egui thread and any background worker with no extra wrapping.
- **Expensive, skip it if unused** — initialization is deferred to first access. If nothing ever reads `DEFAULT_PALETTE`, the closure never runs. That is the whole point versus eager startup work.

No `Arc`, no lock. A `static` lives for the whole program, so there is nothing to keep alive with a refcount, and the value is built once then never mutated, so there is nothing to guard.

## Snippet

```rust
use std::sync::LazyLock;

// Built once on first read, then shared read-only for the life of the process.
static DEFAULT_PALETTE: LazyLock<Palette> = LazyLock::new(|| {
    // The embedded asset travels in the binary; decoding/building is the
    // expensive part we defer until something actually asks for the palette.
    static BYTES: &[u8] = include_bytes!("../assets/default_palette.bin");
    Palette::from_embedded(BYTES)
});

// Anywhere, on any thread — first caller builds it, the rest just borrow.
fn default_swatches() -> &'static [Rgba8] {
    DEFAULT_PALETTE.colors()
}
```

`include_bytes!` embeds the asset; the cost you care about (parsing/quantizing/constructing the `Palette`) sits inside the closure and is what gets deferred and cached.

One caveat the type system enforces: the closure must not fail. `LazyLock::new` takes `FnOnce() -> T`, not `-> Result<T, _>`. Since the asset is embedded and known-good at build time, build the `Palette` infallibly — and if the parse genuinely could fail, that is a build-time bug to catch in a test, not a runtime `unwrap()` in the closure (the no-`unwrap`/no-`panic` rule still applies inside that closure). If you can't make construction infallible, the right move is to encode the data so it can't be malformed, or to validate it in a test, not to panic in front of a user.

## Reasoning

This is the "lazily initialize a `static` once, then read forever" row of the decision table, and in Pixhaus the shared-across-threads case is the common one, so the thread-safe `*Lock` form is correct rather than the single-threaded `*Cell` form. `LazyLock` is the ergonomic default here because the value comes from a self-contained closure — load the embedded bytes, build the palette, done — which is exactly the shape `LazyLock::new(|| ...)` wants.

`LazyLock` is in `std`, so this needs no dependency. It replaces the old `lazy_static!` and `once_cell::sync::Lazy` patterns; don't pull in a crate for it.

## What I'd reject

- **`Arc<Palette>` (or `Arc<RwLock<Palette>>`).** The reflex on the word "shared" is `Arc`, and on "shared from threads" it's `Arc<lock<>>` — both wrong here. `Arc` is for a value with *no single owner* that several holders must keep alive independently. A `static` already lives forever and has no ownership question, so the refcount buys nothing. And the palette is read-only after construction — there is no shared *mutation*, so there is nothing for a `Mutex`/`RwLock` to guard. A lock here would just add contention on every read for no reason.

- **`OnceLock<Palette>` with manual `get_or_init`.** Not wrong — `OnceLock` is the same machinery underneath — but it's the tool for when something *else* decides the value and calls `set`/`get_or_init` at a call site, i.e. when init isn't a self-contained closure. Here init *is* a self-contained closure, so `LazyLock` says it once at the definition and every call site is just a read. Prefer it for the ergonomics.

- **`Mutex<Option<Palette>>` initialized lazily by hand.** This is the anti-pattern the table calls out: you'd be hand-rolling exactly what `LazyLock` gives you, plus a lock you don't need, plus an `Option` you have to keep unwrapping. Skip it.

- **Eager construction at startup (e.g. building it in `main` or the app constructor).** This violates the "skip it if nothing uses it" requirement directly — it pays the expensive build cost unconditionally. Laziness is a stated goal, so the lazy container is the point.

- **`LazyCell` / `OnceCell` (the single-threaded cells).** They're `!Sync`, so they can't back a `static` read from multiple threads. Ruled out by the cross-thread access requirement.

- **Raw pointers / `unsafe` static init tricks.** Off the table — the workspace is `#![forbid(unsafe_code)]`. `LazyLock` is the safe, `std` answer and needs none of that.
