---
name: pixhaus-async-trait
description: >
  Use when putting an `async fn` in a trait that has to be used as a `dyn` trait
  object in Pixhaus — above all the AI multi-backend adapter (`Anthropic`,
  `OpenAI`, `Replicate`, `Ollama`, `ComfyUI`, `Stability`) held behind
  `Arc<dyn Backend>` / `Vec<Box<dyn Backend>>` and dispatched with
  `backend.run(...).await`. Trigger this for ANY "async fn in a trait", "trait
  object with an async method", "the backend / adapter / verb trait", "registry
  of backends", "the trait isn't dyn compatible / not object safe (E0038)", "why
  isn't my async trait a trait object", or "the future isn't Send" request, and
  whenever you see `#[async_trait]`, `async_trait::async_trait`,
  `#[async_trait(?Send)]`, or a hand-rolled `Pin<Box<dyn Future>>` return in a
  trait method. async-trait has two traps worth stopping for — you usually do NOT
  need it (native `async fn` in traits has worked since Rust 1.75 for static
  dispatch, and the workspace is on a far newer toolchain), and when you do, its
  Send-by-default boxing quietly forces every argument and `Self` to be `Send` /
  `Sync` — so reach for this skill rather than relying on memory.
---

# async-trait for Pixhaus

`async-trait` is a single attribute macro, `#[async_trait]`, that rewrites
`async fn` methods in a trait into methods returning `Pin<Box<dyn Future>>`. That
boxing is the one thing it does, and it exists for exactly one reason: to make a
trait with async methods usable as a **`dyn` trait object**. Native `async fn` in
traits is stable and zero-cost for static dispatch, but it does not give you
`dyn Trait`. async-trait buys `dyn` at the cost of a heap allocation per call.

So the first question is never "how do I write this with async-trait" — it's
"do I need it at all". Most of the time you don't. Reach for it only when you are
holding a heterogeneous set of async implementations behind a trait object.

For the exhaustive surface — the full desugaring, every supported trait feature,
the generated lifetime names, the edge-case limitations — read
`references/api-reference.md`. This file is the decision and the patterns.

## Version and license

| Crate | Version | License |
|---|---|---|
| `async-trait` | 0.1 (0.1.89) | `MIT OR Apache-2.0` |

The dual license includes MIT, so it passes the workspace MIT lock and
`cargo deny`. It is a proc-macro crate with no runtime dependency and contains no
`unsafe`, so it sits fine under the workspace `unsafe`-forbidden rule — the macro
expands to ordinary safe `Box::pin`.

```toml
async-trait = "0.1"
```

## Decision 1: native async fn in traits first, async-trait only for `dyn`

This is the whole game, and getting it wrong is the most common async-trait
mistake. Rust 1.75 stabilized `async fn` in traits; the workspace toolchain is far
past that (edition 2024, see `pixhaus-rust-conventions`). For static dispatch —
generic bounds, a single impl, anything monomorphized — **write the native form
and add nothing**:

```rust
// GOOD — native, zero allocation, monomorphized. No async-trait, no macro.
trait Filter {
    async fn apply(&self, frame: Frame) -> Result<Frame>;
}

// Generic call site: static dispatch, the future is whatever the impl returns.
async fn run_filter(f: &impl Filter, frame: Frame) -> Result<Frame> {
    f.apply(frame).await
}
```

You reach for `#[async_trait]` only when you genuinely need a **trait object** —
a `dyn Trait` — because native `async fn` in a trait is not `dyn`-compatible. The
compiler tells you so loudly:

```
error[E0038]: the trait `Backend` is not dyn compatible
   = note: method `run` is `async`
```

That error is the signal — and the only good reason to add the crate. If you are
not storing the trait behind `Box<dyn _>` / `Arc<dyn _>` and you are not calling
it through a `&dyn _`, you do not need async-trait. Deleting it makes the code
faster and simpler.

```
About to add #[async_trait]?
├─ Is the trait used as dyn Trait (Box<dyn _>, Arc<dyn _>, &dyn _, a Vec of them)?
│    ├─ no  → use native `async fn` in the trait. Add nothing. Stop here.
│    └─ yes → is it a heterogeneous registry you dispatch at runtime? → async-trait. Read on.
└─ Is this on a hot per-pixel / per-frame path? → neither; the per-call Box::pin alloc is wrong there.
```

## Decision 2: the canonical Pixhaus use — the AI backend registry

The one place in Pixhaus that earns async-trait is the multi-backend AI runtime.
CLAUDE.md describes it as "a multi-backend runtime via an adapter pattern —
Anthropic, OpenAI, Replicate, Ollama, ComfyUI, Stability". That is a textbook
trait object: one `Backend` trait, many concrete adapters, chosen at runtime and
held in a registry. They cannot be a generic parameter because the set is
heterogeneous and resolved from config, so they live behind `Arc<dyn Backend>` —
and that is precisely what async-trait is for.

```rust
use async_trait::async_trait;
use tokio::sync::mpsc;

#[async_trait]
pub trait Backend: Send + Sync {
    /// Run a verb to completion, streaming chunks back over `tx`.
    async fn run(&self, inputs: VerbInputs, tx: mpsc::Sender<VerbChunk>) -> Result<()>;

    /// Non-async methods coexist freely — no boxing, no macro effect.
    fn id(&self) -> &'static str;
}

#[async_trait]
impl Backend for AnthropicBackend {
    async fn run(&self, inputs: VerbInputs, tx: mpsc::Sender<VerbChunk>) -> Result<()> {
        let mut stream = self.client.stream(inputs).await?;
        while let Some(chunk) = stream.next().await {
            tx.send(chunk?).await.map_err(|_| Error::ReceiverDropped)?;
        }
        Ok(())
    }

    fn id(&self) -> &'static str { "anthropic" }
}
```

Now they compose as trait objects — the registry the conventions skill already
hands to `invoke_streaming`:

```rust
let backend: Arc<dyn Backend> = registry.get(kind)?;   // dyn — needs async-trait
tokio::spawn(async move { backend.run(inputs, tx).await });
```

`Arc<dyn Backend>` is the shape to prefer over `Box<dyn Backend>` here: backends
are shared (the registry keeps one, tasks clone the `Arc`) and immutable after
construction. That matches the `Arc<dyn Backend>` already used in the streaming
example in `pixhaus-rust-conventions`.

## Decision 3: keep the default `Send`; use `?Send` only for thread-pinned futures

By default `#[async_trait]` boxes the future as `Pin<Box<dyn Future + Send>>` and
adds the bounds that make that hold. **Keep the default.** The Pixhaus binary owns
a multi-threaded Tokio runtime (CLAUDE.md), and anything you `tokio::spawn` must be
`Send`. A backend future is spawned, so it must be `Send` — the default is exactly
right, which is why the trait above is `Backend: Send + Sync`.

The Send-by-default has a consequence that surprises people: because every
argument is captured into the boxed future, **every argument must be `Send`, a
`&T` argument forces `T: Sync`, and `Self` must be `Send`/`Sync`** for the future
to stay `Send`. If a non-`Send` value (an `Rc`, a raw egui/wgpu handle, a
`RefCell`) is passed in or held in `self`, you get a "future cannot be sent
between threads safely" error pointing at the generated method. The fix is almost
always to not pass the non-`Send` thing across the boundary, not to drop `Send`.

Use `#[async_trait(?Send)]` only when the future is deliberately pinned to one
thread and never spawned onto the multi-thread runtime — e.g. a trait whose impls
touch `!Send` UI state and run only on the egui update thread. When you do, it
must appear on **both the trait and every impl**, or they won't match:

```rust
#[async_trait(?Send)]                 // future is NOT required to be Send
trait LocalTask {
    async fn step(&mut self);         // may capture !Send state
}

#[async_trait(?Send)]                 // same annotation on the impl — mandatory
impl LocalTask for UiAnimation {
    async fn step(&mut self) { /* touches !Send egui handles */ }
}
```

If you find yourself writing `?Send` to silence a Send error on something you also
`tokio::spawn`, stop — that's the lock-across-await class of bug, not a reason to
weaken the bound. See the async rules in `pixhaus-rust-conventions`.

## The cost, and where it must not go

Each async-trait call is one `Box::pin` heap allocation plus a dynamic dispatch
through the vtable. For a network-bound AI backend that's about to do an HTTP
round-trip, the allocation is free in relative terms — correct trade.

It is the wrong trade on a hot path. Pixhaus has an 8K-pixel performance ceiling
(see the project memory and CLAUDE.md): per-brush-move and per-frame work must be
bounded and allocation-light. Do not put `#[async_trait]` — or async at all — on
pixel-blending, compositing, or dirty-region code. Those are synchronous CPU
loops; if one needs to run off-thread, it goes through `spawn_blocking`, not an
async trait. async-trait belongs at the I/O boundary, never in the inner loop.

## Testing async traits with mockall

The testing stack mocks traits with `mockall` (trait-then-mock; see
`pixhaus-testing-conventions`). mockall understands async-trait, but **attribute
order matters**: `#[automock]` goes *above* `#[async_trait]` so mockall sees the
original async signatures before the macro rewrites them.

```rust
use mockall::automock;
use async_trait::async_trait;

#[automock]            // outer — must come first
#[async_trait]         // inner — rewrites after automock has read the trait
pub trait Backend: Send + Sync {
    async fn run(&self, inputs: VerbInputs, tx: mpsc::Sender<VerbChunk>) -> Result<()>;
    fn id(&self) -> &'static str;
}

// In a test: program the mock's async method with returning, as usual.
let mut backend = MockBackend::new();
backend.expect_run().returning(|_, _| Box::pin(async { Ok(()) }));
```

If the mock won't compile, the order is the first thing to check.

## Gotchas worth internalizing

- **Elided lifetimes need to be spelled out.** async-trait inherits the
  async-fn rule: a lifetime elided anywhere other than directly on `&` / `&mut`
  must be written `'_` or named. `async fn f(x: Cow<str>)` is rejected; write
  `async fn f(x: Cow<'_, str>)`. Plain `&self` / `&Foo` are fine.
- **Put `#[async_trait]` on the trait AND every impl.** It is not inherited. A
  trait annotated but an impl not (or vice versa) gives a mismatched-signature
  error. Same for the `?Send` flavor — both sides or neither.
- **`Self: Sized` methods stay un-boxed.** A method with `where Self: Sized` is
  not part of the vtable and is left as a normal `async fn` — handy for
  constructors or helpers you don't call through `dyn`.
- **Recursion is fine here.** Because the future is already boxed, an
  async-trait method that calls itself doesn't need the usual
  `Box::pin`-the-recursion dance — the macro has already done it.
- **The generated method has a hidden `'async_trait` lifetime.** You won't write
  it, but it shows up in error messages and in any manual `impl` you try to hand
  off; that's expected, not a bug in your code.

## Decision shortcut

```
Async method in a trait?
├─ Used only via generics / a single impl (static dispatch)?
│    └─ native `async fn` in the trait. No async-trait. No macro. Done.
├─ Used as a trait object — Arc<dyn _> / Box<dyn _> / &dyn _ (the AI backend registry)?
│    └─ #[async_trait] on the trait and every impl. Prefer Arc<dyn Backend> for shared backends.
│       Keep the default Send (it's spawned on the multi-thread runtime).
│       Trait bound: `: Send + Sync`. Every arg and Self must be Send/Sync.
├─ Future is pinned to one thread and never spawned (UI-thread-only state)?
│    └─ #[async_trait(?Send)] on both sides.
├─ Mocking it in a test?
│    └─ #[automock] ABOVE #[async_trait].
└─ Hot pixel / frame / dirty-region path?
     └─ none of the above — keep it sync; off-thread work goes to spawn_blocking.
```
