---
name: pixhaus-pollster
description: >
  Use when blocking a synchronous thread on a single async future in Pixhaus —
  most often driving wgpu's async setup (`request_adapter`, `request_device`) to
  completion in a context that has no async runtime: the `render` crate's tests,
  benches, and examples, build/setup glue, or a `#[pollster::test]` async test.
  Trigger this for ANY "block on a future", "await this in a sync fn", "run this
  future to completion without tokio", or "wgpu init hangs / I need the device
  synchronously" request, and whenever you see `pollster::block_on`,
  `FutureExt::block_on`, `#[pollster::main]`, or `#[pollster::test]`. pollster is
  a separate executor from the binary's tokio runtime; reach for this skill to get
  the boundary right rather than guessing, because misusing it blocks the UI thread
  or fights the runtime.
---

# pollster for Pixhaus

pollster is "an incredibly minimal async executor" — roughly 100 lines, zero
dependencies, one job: block the current thread until a single future completes.
It does not spin; it parks the thread in a wait state until the future is polled
to completion. There is no reactor, no task scheduler, no I/O driver.

That minimalism is the whole point and the whole limit. Use pollster to cross from
sync into async exactly once, in a context that has no runtime. Do not reach for it
when you already have one.

## Version and license

| Crate | Version | MSRV | License |
|---|---|---|---|
| `pollster` | 0.4 | 1.69.0 | `Apache-2.0 OR MIT` |

The dual license includes MIT, so pollster passes the workspace MIT lock and
`cargo deny`. The attribute macros (`#[pollster::main]`, `#[pollster::test]`) are
behind the `macro` feature — off by default.

```toml
# block_on / FutureExt only (the common case)
pollster = "0.4"

# also want #[pollster::main] / #[pollster::test]
pollster = { version = "0.4", features = ["macro"] }
```

## The entire API

Three things. That's it.

```rust
// 1. Free function. Blocks until `fut` resolves, returns its output.
//    Takes anything that is IntoFuture, not just Future.
pub fn block_on<F: IntoFuture>(fut: F) -> F::Output

// 2. Extension trait — the same thing in suffix position.
//    Implemented for every F: Future.
use pollster::FutureExt as _;
let out = some_future.block_on();

// 3. Attribute macros (feature = "macro"), each just wraps block_on:
#[pollster::main]   // enables `async fn main()`
#[pollster::test]   // enables an `async fn` test
```

`block_on` and `FutureExt::block_on` are interchangeable — pick whichever reads
better at the call site. Prefer the free function when the future is built inline;
prefer the suffix `.block_on()` when chaining off an expression.

## The mental model: one bridge, no runtime

pollster blocks the calling thread and drives *that one future* to completion. It
provides no executor for futures the future itself spawns, no timer, no I/O
reactor. So:

- A self-contained future (wgpu setup, a channel recv, a CPU-bound async block)
  works.
- A future that calls `tokio::spawn`, `tokio::time::sleep`, or any
  `tokio::net`/`tokio::fs` I/O **panics or hangs** — those need Tokio's reactor
  running on the thread, and pollster isn't it. The pollster docs call this out:
  futures that require a specific runtime or reactor will not work.

This is the line that matters in Pixhaus, because the binary owns a Tokio runtime
(see CLAUDE.md). Two executors, two non-overlapping jobs.

## When to use it in Pixhaus — and when not to

**Use pollster when there is no async runtime in scope and you need one future's
result synchronously.** The canonical case is the `render` crate. It's UI-agnostic
and knows nothing about eframe or the binary's Tokio runtime, but wgpu's setup is
async. In `render`'s tests, benches, and examples, drive it with pollster:

```rust
// render crate test/bench/example — no runtime here, so bridge with pollster
let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
    .expect("no suitable wgpu adapter");
let (device, queue) = pollster::block_on(
    adapter.request_device(&wgpu::DeviceDescriptor::default()),
)?;
```

`#[pollster::test]` is the clean way to write an async test in a crate that has no
test runtime:

```rust
#[pollster::test]
async fn uploads_dirty_region() {
    let gpu = TestGpu::new().await; // self-contained async setup
    // ...assertions...
}
```

**Do not use pollster when:**

- **You're on the egui update thread.** `block_on` parks the thread until the
  future finishes — on the UI thread that freezes the frame and the whole window.
  Background work belongs on a Tokio task that returns its result over a channel
  the update loop drains each frame, then `ctx.request_repaint()` to wake it. See
  the `pixhaus-egui` skill and the async rules in the `pixhaus-rust-conventions`
  skill. This is the most likely misuse — resist "just block_on it for now" in any
  `ui`/`logic` path.
- **A Tokio runtime is already available.** Inside the binary or any
  `#[tokio::main]`/`tokio::spawn` context, use Tokio's own `block_on`,
  `block_in_place`, or `spawn_blocking`. Nesting pollster inside Tokio gives you two
  executors fighting over one thread.
- **The future needs a reactor.** Anything doing Tokio timers, sockets, or async
  file I/O. pollster has no reactor; it will hang or panic. Run it on Tokio.
- **The binary needs `async fn main`.** Pixhaus's runtime is Tokio and is owned by
  the binary, so the entry point is Tokio's, not `#[pollster::main]`. Reserve
  `#[pollster::main]` for small standalone helper binaries or examples that have no
  other runtime.

## Rules that prevent the recurring bugs

- **One bridge per thread, at the edge.** `block_on` is the sync→async boundary
  crossing. Call it at the outermost sync layer (test body, example `main`, setup
  fn), never deep inside async code. Calling `block_on` from within a future that's
  already being driven is a self-deadlock.
- **Don't swallow the output.** `block_on` returns the future's output by value —
  thread the `Result`/value out and handle it. Per the workspace no-`unwrap` rule,
  `unwrap`/`expect` are for test and example code only; in library code propagate
  with `?` or a `thiserror` variant.
- **Keep the blocked future self-contained.** If the future you pass needs to spawn
  concurrent tasks or wait on a timer, pollster is the wrong tool — that's a sign
  the work belongs on Tokio.
- **`IntoFuture`, not just `Future`.** `block_on` accepts any `IntoFuture`, so you
  can pass types that aren't literally futures yet (their `IntoFuture` impl is run
  first). Usually invisible; worth knowing when a type "just works" or doesn't.

## Decision shortcut

```
Need a future's result in a sync fn?
├─ Is a Tokio runtime already in scope (binary, #[tokio::main], a task)?
│    └─ yes → use Tokio (block_in_place / spawn_blocking / handle.block_on). NOT pollster.
├─ Are you on the egui UI thread?
│    └─ yes → never block. Spawn on Tokio, return over a channel, request_repaint.
├─ Does the future need timers / sockets / async file I/O (a reactor)?
│    └─ yes → run it on Tokio. pollster has no reactor.
└─ Otherwise (render-crate test/bench/example, self-contained setup) → pollster::block_on.
```
