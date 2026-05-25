---
name: pixhaus-rust-conventions
description: Use when writing or reviewing Rust in the Pixhaus repo — covers error handling, async, locks, ownership, common agent mistakes, and the PR review checklist
---

# Pixhaus Rust conventions

The floor every Rust contribution reaches for. Read this before writing
or reviewing Rust in the Pixhaus repo. Every section has the pattern, the
counter-example, and the "why" so edge cases resolve themselves.

## Crate ownership

Each crate owns one concern; nothing reaches across that line. New crates
land as the work defines them, but the rule is constant: pure data and ops
live deepest, I/O and integration live at the edges, and the binary depends
on everything but is depended on by nothing.

The likely shape as the editor grows:

| Crate | Owns |
|---|---|
| `core` | pixel buffers, layers, frames, palettes, tilemap data, undo, selection — pure data + ops, no I/O |
| `io` | file formats in and out (the `.pixhaus` format, PNG, etc.) |
| `render` | the UI-agnostic `wgpu` viewport renderer — knows nothing about the UI toolkit |
| `shell` | the `eframe` + `egui` binary; owns app state, hosts panels, embeds `render` |

Before adding code, ask: which crate owns this? "Add stride support to pixel
buffers" → `core`. "Add PNG export" → `io`. "Draw the grid overlay" → `render`.

If two crates fight over a piece of code, the answer is usually that the
abstraction lives in the deeper crate and the concrete adapter lives in the
consumer. Surface the disagreement to a human reviewer rather than picking
arbitrarily.

## Error handling

### Library crates: `thiserror`

Library crates (`core`, `io`, `render`, and any future logic crate) define
their own error enum:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid pixel format: {0}")]
    InvalidPixelFormat(String),

    #[error("buffer size mismatch: expected {expected}, got {actual}")]
    SizeMismatch { expected: usize, actual: usize },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("PNG decode error: {0}")]
    PngDecode(#[from] png::DecodingError),
}

pub type Result<T> = std::result::Result<T, Error>;
```

Each library crate exports its own `Error` and `Result` from `lib.rs`. The
`#[from]` impls let `?` convert upstream errors automatically.

### The binary: `anyhow`

The `shell` binary uses `anyhow::Result` and adds context with `.context()`:

```rust
use anyhow::{Context, Result};

fn export_frame(frame: &Frame, path: &Path) -> Result<()> {
    let bytes = pixhaus_io::encode_png(frame)
        .context("failed to encode PNG for export")?;

    std::fs::write(path, bytes)
        .with_context(|| format!("failed to write {}", path.display()))?;

    Ok(())
}
```

`anyhow` carries a chain of `.context()` strings that surface to the user as
"failed to export frame: failed to encode PNG: invalid pixel format". That's
the diagnostic experience we want. Library code keeps its rich typed errors;
the binary is where they collapse into a context chain for display.

### No `unwrap()`, `expect()`, or `panic!` in non-test code

This is a clippy-enforced rule (`unwrap_used`, `expect_used`, `panic`
denied at workspace level). Every non-test path must surface failures.

```rust
// NO
let config = load_config().unwrap();
let layer = self.layers.get(idx).expect("layer exists");
panic!("never happens");

// YES
let config = load_config()
    .context("no config file found")?;
let layer = self.layers.get(idx)
    .ok_or_else(|| anyhow!("layer index {idx} out of range"))?;
return Err(anyhow!("invalid state: {state:?}"));
```

Test code is exempt. Tests can `unwrap` freely — the crate root sets:

```rust
#![cfg_attr(test, allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
))]
```

Don't add a per-function `#[allow(...)]` to dodge the lint in production
code. If you genuinely need a panic at startup, wire it through proper error
handling instead.

### When `?` is enough vs. when context matters

Use bare `?` when the error is self-evident from the type:

```rust
fn load_frame(path: &Path) -> Result<Frame> {
    let data = std::fs::read(path)?;   // io::Error speaks for itself
    let frame = parse_frame(&data)?;   // ParseError carries the location
    Ok(frame)
}
```

Use `.context()` when the failure mode would be vague without narrative:

```rust
let pixels = buffer.read_pixels()
    .context("failed to read pixels for layer composition")?;
```

Reach for `.with_context(|| format!(...))` to defer string formatting until
the error path actually fires.

## Async patterns

### Edition 2024 niceties

The workspace pins `edition = "2024"` and toolchain `1.95`. A few language
features stabilized in this window are worth reaching for by default:

```rust
// async closures — no more `move ||` + async block dance
let map = stream.then(async |x| process(x).await).collect::<Vec<_>>().await;

// let-chains — combine `if let` with extra conditions
if let Some(layer) = doc.layer(id) && layer.is_visible() && !layer.locked {
    paint(layer);
}

// gen blocks — ergonomic iterator construction without writing an Iterator impl
fn pixels_in(rect: Rect) -> impl Iterator<Item = (u32, u32)> {
    gen {
        for y in rect.y_range() {
            for x in rect.x_range() {
                yield (x, y);
            }
        }
    }
}
```

Reach for these when they make the call site clearer. Don't refactor working
code just to use them.

### The binary owns the runtime

The `shell` binary owns a single tokio runtime and drives it explicitly. The
UI loop runs on one thread; background work (file I/O, inference, decode) is
spawned onto the runtime and its results return over channels the UI drains.
Don't scatter `#[tokio::main]` or hidden runtimes across crates — one owner.

```rust
fn open_project(rt: &tokio::runtime::Handle, path: PathBuf) -> oneshot::Receiver<Result<ProjectMeta>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    rt.spawn(async move {
        let result = pixhaus_io::read(&path).await;
        let _ = tx.send(result);
    });
    rx
}
```

The UI polls `rx` (or drains an `mpsc`) each frame and applies the result
when it lands. Library code keeps its rich error types; the boundary that
hands results to the UI is where they collapse for display.

### Never block the UI thread

CPU-bound work goes through `tokio::task::spawn_blocking`:

```rust
async fn apply_filter(frame: Frame, filter_kind: String) -> Result<Frame> {
    let result = tokio::task::spawn_blocking(move || {
        pixhaus_core::filters::apply(&frame, &filter_kind)
    })
    .await
    .map_err(|e| anyhow!("filter task panicked: {e}"))??;

    Ok(result)
}
```

Heuristic: if the operation looks at every pixel in a buffer, it goes to
`spawn_blocking`. Async I/O (file reads, network) stays on tokio's reactor.

### The lock-across-await footgun

This deadlocks under load:

```rust
// BAD — std::sync::Mutex held across .await
async fn bad(state: Arc<std::sync::Mutex<State>>) {
    let mut guard = state.lock().expect("poisoned");
    some_async_op().await;          // lock still held
    guard.data = 42;
}
```

Two fixes:

1. Release the lock before awaiting:

```rust
async fn good(state: Arc<std::sync::Mutex<State>>) {
    let result = some_async_op().await;
    let mut guard = state.lock().expect("poisoned");
    guard.data = result;
}
```

2. Use an async-aware mutex:

```rust
use tokio::sync::Mutex;

async fn good(state: Arc<Mutex<State>>) {
    let result = some_async_op().await;
    let mut guard = state.lock().await;
    guard.data = result;
}
```

Rule: when you see `Mutex` inside an `async fn`, scan for `.await` between
the `lock()` and the drop. If they cross, it's a blocker — do not approve
the PR.

### Cancellation tokens

Long-running operations the user can cancel use
`tokio_util::sync::CancellationToken`:

```rust
use tokio::select;
use tokio_util::sync::CancellationToken;

pub async fn run_chain(
    mut input: Frame,
    steps: Vec<FilterConfig>,
    cancel: CancellationToken,
) -> Result<Frame> {
    for step in steps {
        select! {
            biased;
            _ = cancel.cancelled() => return Err(Error::Cancelled),
            res = apply(&input, &step) => { input = res?; }
        }
    }
    Ok(input)
}
```

Use `biased;` in `select!` so the cancellation branch wins ties — otherwise
the runtime may resolve in arrival order and you'll lose cancels under load.

### Streaming output

Operations that produce progressive results stream via `tokio::sync::mpsc`:

```rust
use tokio::sync::mpsc;

pub async fn invoke_streaming(
    backend: Arc<dyn Backend>,
    inputs: VerbInputs,
) -> Result<mpsc::Receiver<VerbChunk>> {
    let (tx, rx) = mpsc::channel(32);
    tokio::spawn(async move {
        if let Err(err) = backend.run(inputs, tx.clone()).await {
            let _ = tx.send(VerbChunk::Error(err.to_string())).await;
        }
    });
    Ok(rx)
}
```

Channel capacity matters: too small starves the producer, too large hides
backpressure. 16–64 is the right range for these streams.

## Concurrency and shared state

### One owner per piece of mutable state

The active document has a single owner. On the UI thread that owner is a
plain field mutated through `&mut self` — no lock needed, because only one
thread touches it. A lock appears only where state is genuinely shared across
threads (a background task and the UI), and then it's a serialization point,
not a concurrency optimization:

```rust
pub struct Shared {
    // shared across the UI thread and background tasks
    registry: Arc<parking_lot::RwLock<BackendRegistry>>,
}
```

Avoid second copies of the document floating around — they desync the undo
stack. For per-key concurrent access (caches, registries), `dashmap::DashMap`
outperforms `RwLock<HashMap>`.

### parking_lot vs std vs tokio mutexes

| Use | Pick |
|---|---|
| Short critical section, sync code | `parking_lot::Mutex` |
| Held across `.await` | `tokio::sync::Mutex` |
| Many readers, few writers, sync code | `parking_lot::RwLock` |
| Per-key concurrent access | `dashmap::DashMap` |
| Inter-task signal, single value | `tokio::sync::watch` |
| Inter-task fan-out events | `tokio::sync::broadcast` |
| Worker queue, many producers, one consumer | `tokio::sync::mpsc` |

`std::sync::Mutex` is the slowest of the sync options and its poison
semantics are awkward — prefer `parking_lot` unless you specifically need
poison handling.

## Memory and ownership

### `Vec<u8>` vs `Box<[u8]>`

Default to `Vec<u8>` for pixel buffers. `Box<[u8]>` saves the capacity
field (8 bytes) and signals "fixed size", but the ergonomics cost rarely
pays back in an editor where buffers grow on resize.

### Stride is explicit

Never assume `width * 4`. Pixel buffers carry `stride` separately so
SIMD-friendly padding is allowed:

```rust
pub struct PixelBuffer {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    stride: u32,   // bytes per row, may exceed width * bytes_per_pixel
}

impl PixelBuffer {
    pub fn pixel(&self, x: u32, y: u32) -> Option<&[u8]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let offset = (y as usize) * (self.stride as usize) + (x as usize) * 4;
        self.pixels.get(offset..offset + 4)
    }
}
```

Returning `Option<&[u8]>` instead of `&[u8]` keeps the bounds check
explicit and avoids `unwrap()` at call sites.

### `Cow` for "maybe modify"

When a function may or may not change a buffer, return
`Cow<'_, PixelBuffer>` to avoid the unconditional clone:

```rust
use std::borrow::Cow;

fn maybe_quantize<'a>(buf: &'a PixelBuffer, palette: &Palette) -> Cow<'a, PixelBuffer> {
    if buf.matches_palette(palette) {
        Cow::Borrowed(buf)
    } else {
        Cow::Owned(buf.quantize(palette))
    }
}
```

### Undo stack: snapshots first, deltas only when measured

Default to snapshot-based undo (clone the document into the stack). Reach
for delta-based undo only when memory profiling proves it's needed. Agents
will be tempted to do deltas immediately because they're "more efficient";
push back unless there's a measured constraint.

## Common agent mistakes (with corrections)

### `Box<dyn Trait>` overuse

Agent reflex when a function takes a "thing that does X":

```rust
// BAD — heap allocation, dynamic dispatch
fn render(layer: Box<dyn Renderable>, ctx: &mut Ctx) { ... }
```

Use `impl Trait` in argument position — under edition 2024 it's the idiomatic
choice for monomorphic call sites and reads more cleanly than the `<R: Trait>`
generic form for single-bound parameters:

```rust
// GOOD — monomorphized, zero overhead, terse
fn render(layer: &impl Renderable, ctx: &mut Ctx) { ... }

// Equivalent for single-bound cases; prefer when you need to name R inside:
fn render<R: Renderable>(layer: &R, ctx: &mut Ctx) -> R::Output { ... }
```

Reach for `Box<dyn Trait>` only when the collection genuinely contains
heterogeneous types: `Vec<Box<dyn Verb>>` for a verb registry is fine.
A function parameter is not.

### `Vec<Vec<T>>` for 2D data

```rust
// BAD — fragmented, slow, can't SIMD
struct Grid<T> { cells: Vec<Vec<T>> }
```

Use a flat buffer with explicit dims:

```rust
// GOOD — contiguous, indexable, SIMD-friendly
struct Grid<T> {
    cells: Vec<T>,
    width: usize,
    height: usize,
}

impl<T> Grid<T> {
    fn get(&self, x: usize, y: usize) -> Option<&T> {
        if x >= self.width || y >= self.height { return None; }
        self.cells.get(y * self.width + x)
    }
}
```

### Premature `Arc<Mutex<T>>`

Agent reflex when a value needs to be "shared":

```rust
// BAD — unnecessary heap, locking, contention
fn process(data: Arc<Mutex<PixelBuffer>>) { ... }
```

If only one task touches the data at a time, pass `&mut PixelBuffer`. If
multiple readers and no writer, pass `&PixelBuffer`. Reach for
`Arc<Mutex<>>` only when multiple owners genuinely need shared mutable
access across `.await` points or threads.

### `String` everywhere

```rust
// BAD — heap allocation per call
fn name(&self) -> String { String::from("layer") }
```

Use `&'static str` for constants, `&str` for borrowed names, `Cow<'_, str>`
when the answer is sometimes owned and sometimes static:

```rust
// GOOD
fn name(&self) -> &'static str { "layer" }
fn label(&self) -> &str { &self.label }
fn key(&self) -> Cow<'_, str> {
    if self.id.is_empty() { Cow::Borrowed("default") } else { Cow::Borrowed(&self.id) }
}
```

### `clone()` to satisfy the borrow checker

Cloning a `PixelBuffer` is dozens of KB to MB. If you find yourself cloning
to avoid a borrow conflict, restructure: hoist the borrow, use `mem::take`,
or return the new buffer from a function that consumes the old.

## Idioms worth knowing

### Newtype for unit safety

Wrap primitive types when they carry meaning:

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct LayerId(u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct FrameIndex(u32);

// LayerId can never be confused with FrameIndex at the type level.
fn pixel(layer: LayerId, frame: FrameIndex) -> Option<&Pixel> { ... }
```

The compiler refuses to swap them. Worth the boilerplate for IDs, indices,
counts that share a primitive type.

### Sealed traits

Stop downstream implementers from breaking your invariants:

```rust
mod sealed { pub trait Sealed {} }

pub trait BlendMode: sealed::Sealed {
    fn blend(&self, src: Rgba, dst: Rgba) -> Rgba;
}

pub struct Normal;
impl sealed::Sealed for Normal {}
impl BlendMode for Normal { fn blend(&self, ..) -> .. { ... } }
```

Use when you need a closed set of implementations (built-in blend modes,
known backend kinds). Plugin-extensible traits should not be sealed.

### Type-state for protocols

Encode request lifecycle in the type system:

```rust
pub struct Request<S> { id: String, state: PhantomData<S> }
pub struct Draft;
pub struct Submitted;

impl Request<Draft> {
    pub fn submit(self) -> Request<Submitted> {
        Request { id: self.id, state: PhantomData }
    }
}

impl Request<Submitted> {
    pub fn cancel(self) -> Result<()> { ... }
}
```

A `Request<Draft>` cannot be cancelled; a `Request<Submitted>` cannot be
re-submitted. The compiler enforces the protocol.

## PR review checklist

Run through this list when reviewing Rust PRs:

- [ ] No `unwrap()` / `expect()` / `panic!` outside tests
- [ ] Library crates use `thiserror`, the binary uses `anyhow`
- [ ] `?` propagates errors; `.context()` adds narrative where useful
- [ ] No `Mutex` held across `.await` (or it's `tokio::sync::Mutex`)
- [ ] CPU-bound work in `spawn_blocking`, not on the runtime
- [ ] No `Box<dyn Trait>` for monomorphic call sites
- [ ] No `Vec<Vec<T>>` for 2D data
- [ ] No premature `Arc<Mutex<>>` for single-owner data
- [ ] No `clone()` on large buffers to dodge the borrow checker
- [ ] Public functions have rustdoc; `# Errors` and `# Panics` sections where relevant
- [ ] Every public function has at least one test (`pixhaus-testing-conventions`)
- [ ] New crates added to the workspace `members` list
- [ ] `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` all green
- [ ] New dependencies pass `cargo deny check` (MIT-compatible license, no advisories)
- [ ] No `unsafe` (workspace forbids); if there's a real need, escalate to a maintainer

## When in doubt

- "Should this be a separate crate?" — Default no. Internal modules first.
  Promote to a crate when the API stabilizes and another consumer needs it.
- "Should this be `async`?" — Only if it does I/O or wraps something that
  does. Pure CPU-bound work shouldn't be `async`.
- "Should I use `unsafe`?" — No. The workspace forbids it. If you find a
  case that genuinely needs it, escalate.
- "Should I add a dependency?" — `cargo deny` gates licenses (MIT project, no
  GPL/LGPL/AGPL). If a crate is load-bearing or unusual, surface it for human
  review rather than adding silently.
- "Should I rewrite this for performance?" — Only with a benchmark showing
  the current code is the bottleneck.
