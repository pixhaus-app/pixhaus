# Rust Best Practices for AI-Driven Development (2026)

A manifesto for building Pixhaus—and other Rust projects—where agents write code, humans review, and both iterate.

---

## Why this document exists

In 2026, AI agents are reliable Rust code writers. They understand the language, they know the idioms, and they can iterate on feedback. But agents hallucinate in predictable ways. They ship code that typechecks but doesn't scale. They over-engineer solutions. They miss edge cases.

This handbook is not a Rust tutorial. You'll find thousands of those. It's a field guide to patterns that work when an agent writes 80% of your Rust codebase and you review 20%. It covers the gaps between "this code compiles" and "this code is worth maintaining."

The lead is new to Rust. You'll review agent-generated code. You need to know what to accept, what to push back on, and what to ask the agent to change before you commit to it.

---

## 1. Project structure conventions

### Workspace layout: one concern per crate

Pixhaus is a Tauri 2.x application. Structure it as a workspace with separate crates for each architectural layer:

```
pixhaus/
├── Cargo.toml                 # Workspace root
├── crates/
│   ├── pixhaus-core/          # Pixel buffer, image, layer, frame ops
│   ├── pixhaus-io/            # File I/O, format support
│   ├── pixhaus-ai/            # AI integrations (inference, prompting)
│   ├── pixhaus-scripting/     # Lua, scripting runtime
│   └── pixhaus-app/           # Tauri app, IPC commands, UI glue
└── ecosystem/
    └── docs/
```

Each crate has a single responsibility. This matters for AI agents because:

1. Crate boundaries are explicit contract points. An agent can't accidentally reach into internal modules.
2. Public API surfaces are small and reviewable.
3. Error types are scoped. You know which errors come from where.

**Cargo.toml workspace root:**

```toml
[workspace]
members = [
    "crates/pixhaus-core",
    "crates/pixhaus-io",
    "crates/pixhaus-ai",
    "crates/pixhaus-scripting",
    "crates/pixhaus-app",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["Pixhaus Contributors"]
license = "MIT OR Apache-2.0"
```

When an agent asks to add a feature, clarify which crate owns it. "Add stride support for pixel buffers" belongs in `pixhaus-core`. "Add PNG export" belongs in `pixhaus-io`. This discipline prevents the crate from becoming a junk drawer.

### Public API re-exports: `pub use` at crate root

In `pixhaus-core/src/lib.rs`, re-export the public surface:

```rust
//! Pixhaus core: pixel buffers, layers, frames, blend modes.

pub use crate::buffer::{PixelBuffer, Rgba8};
pub use crate::layer::{Layer, LayerKind, BlendMode};
pub use crate::frame::Frame;
pub use crate::error::{Error, Result};
```

Internal modules (e.g., `crate::blend::simd`) stay private. Agents learn that everything they need is accessible from the crate root. This prevents them from reaching into deep paths and building invisible coupling.

If you find an agent accessing `pixhaus_core::internal::magic_function()`, push back. There's a public API for that, or there should be.

### When to split into a separate crate vs. an internal module

**Separate crate when:**
- The module will have versioning independent of the main crate.
- Other projects (not just Pixhaus) need it.
- The module has a public API that's stable.

**Internal module when:**
- It's a concrete implementation detail.
- It changes frequently.
- It's tightly coupled to its parent.

Example: `pixhaus-core` has an internal `color::srgb_to_linear()` function. It's not exported. Only the blend mode code uses it. It's an internal module.

Later, if Pixhaus grows to need color space handling elsewhere, extract it to `pixhaus-color` and add it to the workspace. Don't do this prematurely.

---

## 2. Error handling for an editor

An editor is a system of fallible operations. File I/O fails. GPU operations fail. Network calls fail. Undo stacks corrupt. Your error handling strategy sets the tone for the entire codebase.

### The pattern: thiserror for libraries, anyhow for applications

**In library crates** (`pixhaus-core`, `pixhaus-io`, `pixhaus-ai`), use `thiserror`:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
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

Each library crate defines its own `Error` type. This gives the application-level code precise signal about what went wrong.

**In the application crate** (`pixhaus-app`), use `anyhow`:

```rust
use anyhow::{Context, Result};

fn export_frame(frame: &Frame, path: &Path) -> Result<()> {
    let data = pixhaus_io::encode_png(frame)
        .context("failed to encode PNG")?;
    
    std::fs::write(path, data)
        .context("failed to write to disk")?;
    
    Ok(())
}
```

The `?` operator automatically converts library errors upward. `anyhow::Context` lets you add narrative. In the app layer, you don't need fine-grained error discrimination—you need to know what broke and why, so you can surface it to the user.

### The `Result<T>` type alias: establish it in every crate

```rust
// In each library crate's lib.rs
pub type Result<T> = std::result::Result<T, Error>;
```

This reduces noise. Instead of writing `std::result::Result<SomeThing, pixhaus_core::Error>` everywhere, you write `pixhaus_core::Result<SomeThing>`. Agents will follow this pattern if you establish it early.

### When `?` is enough vs. when context matters

Use bare `?` for operations where the error is self-evident:

```rust
fn load_frame(path: &Path) -> Result<Frame> {
    let data = std::fs::read(path)?;  // Clear: filesystem operation
    let frame = parse_frame(&data)?;  // Clear: parsing operation
    Ok(frame)
}
```

Use `.context()` when the error message would be vague without it:

```rust
let pixel_data = buffer.read_pixels()
    .context("failed to read pixels for layer composition")?;
```

Without context, "read_pixels failed" is noise. With context, "failed to read pixels for layer composition" tells the user what the editor was trying to do when it broke.

### No panics in production code paths

This is a rule. Not a guideline. A rule.

Unacceptable in production:
```rust
let config = load_config().unwrap();  // NO
let layers = self.layers.get(idx).expect("layer exists");  // NO
let [r, g, b, a] = &data[0..4] else { panic!("invalid color"); };  // NO
```

Acceptable:
```rust
let config = load_config()
    .context("no config file found")?;

let layer = self.layers.get(idx)
    .ok_or_else(|| anyhow!("layer index {idx} out of range"))?;

let [r, g, b, a] = &data[0..4] else {
    anyhow::bail!("invalid color buffer: expected 4 bytes, got {}", data.len());
};
```

The discipline is: anything that can fail, should return a `Result`. When you review agent code and see `unwrap()` outside tests, that's a blocker. Push back.

### Custom error types vs. generic error types

Use custom error types when you need to discriminate at the call site:

```rust
match load_config() {
    Ok(cfg) => { /* */ },
    Err(ConfigError::NotFound) => {
        eprintln!("No config, using defaults");
        return Config::default();
    }
    Err(e) => return Err(e.into()),
}
```

Use generic `anyhow::Error` when you're propagating upward without special handling. Most code falls into this category.

---

## 3. Async patterns

### Tokio + Tauri: how they interact

Tauri 2.x ships with its own runtime. When you write async code in Pixhaus, you're running on Tauri's tokio executor.

The Tauri event loop runs on the main thread. Commands you export to the frontend run in a separate thread pool by default (or on the main thread if you mark them with `#[tauri::command(async)]`).

```rust
#[tauri::command(async)]
async fn export_png(
    frame: FrameData,
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<()> {
    // This runs in Tauri's thread pool, not the UI thread.
    let app_state = state.inner();
    let result = pixhaus_io::encode_png(&frame).await?;
    
    tokio::fs::write(&path, result).await?;
    Ok(())
}
```

Key rule: never block the main thread. If you have a CPU-bound operation (e.g., applying a filter to a large image), spawn it to tokio:

```rust
#[tauri::command(async)]
async fn apply_filter(
    frame: FrameData,
    filter_kind: String,
) -> Result<FrameData> {
    // Don't do heavy work here. Spawn to a worker thread.
    let result = tokio::task::spawn_blocking(move || {
        pixhaus_core::filters::apply(&frame, &filter_kind)
    })
    .await
    .map_err(|_| anyhow!("filter task panicked"))?;
    
    Ok(result?)
}
```

### When to spawn vs. when to inline

**Inline (await directly):**
- I/O operations that are fast (< 10ms).
- Operations that need sequential consistency.

**Spawn (tokio::spawn or spawn_blocking):**
- CPU-heavy operations (image processing, compression).
- Operations that can run in parallel.
- Operations that block (filesystem I/O on platforms where it's not async-ready).

An agent will often over-spawn for style. You'll see:

```rust
let result = tokio::spawn(async {
    let x = 1 + 2;
    x
}).await.unwrap();
```

This is pointless. No I/O, no concurrency benefit. Inline it. Tell the agent: "spawn when you're actually running something in parallel."

### Cancellation tokens for long-running operations

If Pixhaus supports cancellable exports or filter chains, use `tokio_util::sync::CancellationToken`:

```rust
#[tauri::command(async)]
async fn apply_filters_cancelable(
    frame: FrameData,
    filters: Vec<FilterConfig>,
    handle: String,
) -> Result<FrameData> {
    let cancel_token = state.spawn_cancellable_task(&handle);
    
    let mut result = frame;
    for (i, filter) in filters.iter().enumerate() {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                return Err(anyhow!("operation cancelled"));
            }
            res = async {
                pixhaus_core::filters::apply(&result, &filter.kind).await
            } => {
                result = res?;
            }
        }
    }
    
    Ok(result)
}
```

This is a pattern to teach agents explicitly. Don't assume they'll reach for it on their own.

### Streaming results from AI inference

If Pixhaus integrates with an LLM for prompting or content generation, stream tokens as they arrive:

```rust
#[tauri::command(async)]
async fn generate_description(
    context: String,
) -> Result<()> {
    // Inefficient: wait for the whole response, then return.
    // let text = ai_client.complete(&context).await?;
    
    // Better: stream tokens, emit them to the UI in real time.
    let mut stream = ai_client.stream(&context).await?;
    
    while let Some(token) = stream.next().await {
        let token = token?;
        window.emit("ai:token", token)?;
    }
    
    Ok(())
}
```

Teach agents that long-running AI operations should stream back to the UI, not wait until completion.

### Async trait methods: native support since Rust 1.75

In older Rust, async trait methods required the `async-trait` crate. Since 1.75, you can write:

```rust
pub trait PixelOp {
    async fn apply(&self, buffer: &mut PixelBuffer) -> Result<()>;
}
```

No `#[async_trait]` macro needed. Agents in 2026 should default to native async fn in traits. If they reach for `async-trait`, ask why. Odds are they don't need it.

### Lock-across-await footgun

This is the classic mistake:

```rust
async fn bad_pattern(state: Arc<Mutex<State>>) {
    let mut guard = state.lock().unwrap();  // Acquires lock
    some_async_op().await;  // BLOCKING THE LOCK ACROSS AWAIT
    guard.data = 42;
}
```

If `some_async_op()` yields, the lock is held. Other tasks starve. This deadlocks under load.

The fix:

```rust
async fn good_pattern(state: Arc<Mutex<State>>) {
    let result = some_async_op().await;  // No lock held
    let mut guard = state.lock().unwrap();
    guard.data = result;
}
```

When you see a Mutex inside an async function, scrutinize where the lock is held. If it crosses an `.await`, that's a blocker.

Better yet: use `tokio::sync::Mutex` (async-aware) or `parking_lot::Mutex` (fast, only for short critical sections):

```rust
use tokio::sync::Mutex;

async fn good_async_pattern(state: Arc<Mutex<State>>) {
    let result = some_async_op().await;
    let mut guard = state.lock().await;
    guard.data = result;
}
```

---

## 4. Concurrency and shared state

### Arc<Mutex<T>> vs. Arc<RwLock<T>> vs. Arc<DashMap>

**Arc<Mutex<T>>:** Use this as your default. Simple, fast, predictable. Exclusive access.

```rust
let state = Arc::new(Mutex::new(AppState::default()));
let state_clone = Arc::clone(&state);
thread::spawn(move || {
    let mut guard = state_clone.lock().unwrap();
    guard.counter += 1;
});
```

**Arc<RwLock<T>>:** Use when you have many readers and few writers.

```rust
let config = Arc::new(RwLock::new(load_config()?));

// Many reader threads:
let config_clone = Arc::clone(&config);
thread::spawn(move || {
    let cfg = config_clone.read().unwrap();
    use_config(&cfg);
});

// Occasional writer:
{
    let mut cfg = config.write().unwrap();
    cfg.refresh();
}
```

In practice, RwLock is slower than Mutex for most workloads due to contention. Use it only when you've measured and confirmed many readers.

**Arc<DashMap>:** Use when you need concurrent updates to a hash map without a global lock.

```rust
use dashmap::DashMap;

let cache = Arc::new(DashMap::new());

cache.insert("key".to_string(), value);
if let Some(entry) = cache.get("key") {
    process(&entry);
}
```

DashMap shards internally. Multiple threads can insert at different keys without contention.

### parking_lot vs. std::sync::Mutex

`parking_lot::Mutex` is faster and has a smaller footprint than std. Use it for short critical sections:

```rust
use parking_lot::Mutex;

let state = Mutex::new(State::default());

{
    let mut guard = state.lock();
    guard.update();
}  // Lock dropped, no unlock needed
```

For async code, use `tokio::sync::Mutex`. For anything else, use `parking_lot::Mutex` or `std::sync::Mutex` (in that order, performance-wise).

### The "every project file has one owner" principle

In Pixhaus, a project file (a `.pxh` document) is owned by exactly one `Document` struct. Only one thread can mutate it at a time. Period.

```rust
pub struct Document {
    frames: Vec<Frame>,
    layers: Vec<Layer>,
    // ...
}

pub struct AppState {
    document: Arc<Mutex<Document>>,
}
```

When an IPC command needs to modify the document, it acquires the lock, modifies, and releases. There's no shared writeable state.

This matters because it means you never have to reason about concurrent mutations across multiple threads. The Mutex is a serialization point, not a concurrency primitive.

### Channel patterns: mpsc, broadcast, watch

**mpsc (multi-producer, single-consumer):**

```rust
let (tx, mut rx) = tokio::sync::mpsc::channel::<Event>(100);

// Worker threads send events:
tx.send(Event::LayerAdded).await?;

// Main loop receives:
while let Some(event) = rx.recv().await {
    handle_event(event);
}
```

Use when one thread is the consumer and many can be producers.

**broadcast:**

```rust
let (tx, _) = tokio::sync::broadcast::channel::<StateChange>(16);

// Many consumers subscribe:
let mut rx1 = tx.subscribe();
let mut rx2 = tx.subscribe();

// One producer broadcasts:
tx.send(StateChange::LayerVisibilityToggled)?;

// All subscribers receive:
while let Ok(change) = rx1.recv().await {
    update_ui(&change);
}
```

Use for one-to-many event distribution (e.g., "document changed, notify all UI listeners").

**watch:**

```rust
let (tx, mut rx) = tokio::sync::watch::channel(InitialValue::default());

// One writer updates:
tx.send(Value::new()).ok();

// Many readers subscribe and see latest:
while rx.changed().await.is_ok() {
    println!("value changed to: {:?}", *rx.borrow());
}
```

Use when you have a shared value that changes rarely and many consumers need the latest value.

### crossbeam patterns

`crossbeam::queue::SegQueue` is a lock-free queue. Useful for work-stealing or producer-consumer patterns:

```rust
use crossbeam::queue::SegQueue;

let work = Arc::new(SegQueue::new());

// Producers push:
work.push(task);

// Workers pop:
while let Some(task) = work.pop() {
    execute(task);
}
```

Don't use this as a default. It's a specialist tool. Tokio channels are usually better.

---

## 5. Memory layout for image data

### Vec<u8> vs. Box<[u8]> for pixel buffers

**Vec<u8>:** Flexible. Can grow. Allocates on the heap. Default choice.

```rust
pub struct PixelBuffer {
    // Width, height, and stride determine size.
    // Data can grow if you add capacity.
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    stride: u32,  // May not equal width * 4 if padded
}
```

**Box<[u8]>:** Fixed size. Slightly smaller footprint (no capacity field). Use when you know the size won't change.

```rust
pub struct PixelBuffer {
    pixels: Box<[u8]>,
    width: u32,
    height: u32,
    stride: u32,
}

// To create:
let size = (height * stride) as usize;
let pixels = vec![0u8; size].into_boxed_slice();
let buffer = PixelBuffer { pixels, width, height, stride };
```

For a pixel editor, `Vec<u8>` is simpler because you might recompose, resize, or grow buffers. Use `Box<[u8]>` only if you have evidence that the footprint matters.

### Pixel layout: stride and alignment

Always store stride explicitly. Never assume width * bytes_per_pixel:

```rust
impl PixelBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        // Align stride to 16 bytes for SIMD.
        let bytes_per_pixel = 4;
        let stride = ((width as usize * bytes_per_pixel + 15) / 16) * 16;
        let pixels = vec![0u8; stride * height as usize];
        
        Self { pixels, width, height, stride: stride as u32 }
    }
    
    pub fn pixel_at(&self, x: u32, y: u32) -> &[u8] {
        let offset = (y as usize * self.stride as usize) + (x as usize * 4);
        &self.pixels[offset..offset + 4]
    }
}
```

Stride padding enables SIMD and improves cache efficiency. Teach agents to never assume contiguous layout.

### Arena allocation for short-lived structures

If you're building intermediate structures during a filter operation (e.g., temporary blend results), use a bump allocator:

```rust
use bumpalo::Bump;

fn apply_blend_chain(layers: &[Layer], buffer: &mut PixelBuffer) -> Result<()> {
    let arena = Bump::new();
    
    // Temporary buffers allocated from the arena, freed at end of scope.
    let temp1 = arena.alloc_slice_fill_with(buffer.pixels.len(), |_| 0u8);
    let temp2 = arena.alloc_slice_fill_with(buffer.pixels.len(), |_| 0u8);
    
    for layer in layers {
        blend(&layer, temp1, temp2);
    }
    
    Ok(())
}  // Arena freed here.
```

`bumpalo` is much faster than individual allocations for short-lived, temporary data. Don't allocate a new Vec for every intermediate buffer.

### Clone vs. borrow vs. Cow<[u8]>

**Clone:** Simple but expensive. Use when you need ownership.

```rust
fn process(buffer: &PixelBuffer) -> Result<PixelBuffer> {
    let mut result = buffer.clone();  // Expensive.
    mutate(&mut result);
    Ok(result)
}
```

**Borrow:** Efficient but requires lifetime management. Use when you can afford to borrow.

```rust
fn process(buffer: &PixelBuffer) -> Result<()> {
    read_from(buffer);
    Ok(())
}
```

**Cow<[u8]>:** "Copy-on-write". Cheap when unmodified, clones only if you mutate.

```rust
use std::borrow::Cow;

fn maybe_process<'a>(buffer: &'a PixelBuffer) -> Result<Cow<'a, PixelBuffer>> {
    if needs_processing(buffer) {
        let mut result = buffer.clone();
        mutate(&mut result);
        Ok(Cow::Owned(result))
    } else {
        Ok(Cow::Borrowed(buffer))
    }
}
```

For an editor, you'll usually `clone()` pixel buffers for undo snapshots and `borrow()` them for reads. Teach agents this asymmetry.

### Undo stack memory shape: snapshots vs. deltas

**Snapshot-based (simple, expensive):**

```rust
pub struct UndoStack {
    snapshots: Vec<Document>,
    current: usize,
}
```

Each undo entry is a full document clone. Memory grows with undo depth. Simplest to implement.

**Delta-based (complex, efficient):**

```rust
#[derive(Clone)]
pub enum Delta {
    LayerAdded { index: usize, layer: Layer },
    LayerDeleted { index: usize },
    PixelsModified { region: Rect, old: Box<[u8]>, new: Box<[u8]> },
}

pub struct UndoStack {
    deltas: Vec<Delta>,
    current: usize,
}
```

Store only changes. Undo is a series of reversals. Much more memory-efficient but harder to implement correctly.

For Pixhaus MVP, use snapshots. They're simpler, and you can optimize later if memory becomes a bottleneck. Agents will be tempted to do deltas prematurely. Push back unless you have a clear constraint.

---

## 6. Performance idioms

### iter().map().collect() vs. imperative loops

Both are idiomatic. Use whichever is clearer:

```rust
// Functional: clear intent, no mutations.
let doubled: Vec<u32> = data.iter().map(|x| x * 2).collect();

// Imperative: easier to debug, clearer control flow.
let mut doubled = Vec::new();
for x in &data {
    doubled.push(x * 2);
}
```

Functional is preferred when:
- The transformation is simple (one or two operations).
- The intent is clear from the chain.

Imperative is clearer when:
- The loop has complex logic or multiple branches.
- You're profiling and need to annotate with debug info.

An agent will default to functional (it's the "Rust way"). That's fine. Don't force imperative. But if readability suffers, push back.

### Rayon par_iter: when it pays

`rayon::par_iter` splits work across CPU cores. Overhead is non-zero. Only use it when:

1. The operation is CPU-bound (no I/O).
2. The work per item is substantial (> 100 microseconds).
3. The data set is large (> 10,000 items).

Good use case:
```rust
use rayon::prelude::*;

fn apply_filter_parallel(pixels: &mut [u8], width: u32, height: u32) {
    let stride = width as usize * 4;
    pixels
        .par_chunks_mut(stride)  // Chunks, in parallel
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..width as usize {
                let pixel = &mut row[x * 4..x * 4 + 4];
                *pixel = filter(pixel);
            }
        });
}
```

Bad use case:
```rust
// Don't do this. Overhead kills the win.
let results: Vec<_> = (0..10)
    .into_par_iter()
    .map(|i| heavy_operation(i))
    .collect();
```

When agents ask to parallelize, ask: is this the bottleneck? Have you measured? Most code isn't. Parallelization adds complexity. Don't accept it without evidence.

### SIMD: portable_simd vs. explicit intrinsics

As of 1.82 (late 2024), `std::simd` is stable for basic operations. You can use it:

```rust
use std::simd::*;

fn blend_simd(src: &[u8], dst: &mut [u8], alpha: u8) {
    let alpha_v = u8x4::splat(alpha);
    
    for (src_chunk, dst_chunk) in src.chunks(4).zip(dst.chunks_mut(4)) {
        let src_v = u8x4::from_slice(src_chunk);
        let dst_v = u8x4::from_slice(dst_chunk);
        
        // Vectorized blend operation...
        let result = src_v; // Placeholder
        dst_v.copy_to_slice(dst_chunk);
    }
}
```

For portable SIMD without nightly, use `packed_simd` or similar. But be honest: if you're not profiling and showing a 2x+ win, don't use SIMD. It complicates code.

### Hot loop discipline

In performance-critical loops (pixel operations, blending, composition), avoid:

- Allocations (use stack or arena).
- Branching (prefer branchless operations when possible).
- Function calls (inline hot functions).
- Bounds checking (use unchecked access if you've verified bounds).

Example of good hot loop discipline:

```rust
#[inline]
fn blend_pixel(src: &[u8; 4], dst: &mut [u8; 4], alpha: u8) {
    let a = alpha as u32;
    dst[0] = ((src[0] as u32 * a + dst[0] as u32 * (256 - a)) >> 8) as u8;
    dst[1] = ((src[1] as u32 * a + dst[1] as u32 * (256 - a)) >> 8) as u8;
    dst[2] = ((src[2] as u32 * a + dst[2] as u32 * (256 - a)) >> 8) as u8;
    dst[3] = ((src[3] as u32 * a + dst[3] as u32 * (256 - a)) >> 8) as u8;
}

fn apply_blend(src: &[u8], dst: &mut [u8], alpha: u8) {
    for i in (0..src.len()).step_by(4) {
        let src_pixel: &[u8; 4] = (&src[i..i + 4]).try_into().unwrap();
        let dst_pixel: &mut [u8; 4] = (&mut dst[i..i + 4]).try_into().unwrap();
        blend_pixel(src_pixel, dst_pixel, alpha);
    }
}
```

No allocations, no branching (except the loop), inline hints. This runs fast.

---

## 7. Tauri-specific patterns

### IPC command function signatures

Commands exported to the frontend must be `pub async` and take `tauri::State<T>` if they need access to app state:

```rust
#[tauri::command(async)]
pub async fn export_frame(
    frame_id: u32,
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<ExportResult, String> {
    let app_state = state.inner();
    
    let frame = app_state.document.lock().await
        .get_frame(frame_id)
        .ok_or_else(|| "Frame not found".to_string())?;
    
    pixhaus_io::encode_png(&frame).await
        .map_err(|e| e.to_string())?;
    
    Ok(ExportResult { success: true })
}
```

Return type must be `Result<T, String>`. Tauri serializes errors as strings. It's not ideal, but it's the contract.

Put IPC handlers in `pixhaus-app/src/commands/` and re-export from the main `main.rs`:

```rust
// src/commands/mod.rs
pub mod export;
pub mod import;
pub mod layers;

pub use export::*;
pub use import::*;
pub use layers::*;

// src/main.rs
#[tauri::command]
pub async fn export_frame(...) { /* ... */ }
```

This keeps `main.rs` readable.

### State management with tauri::State<T>

Initialize your app state in `main.rs`:

```rust
fn main() {
    let app_state = AppState::new();
    
    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            export_frame,
            import_frame,
            add_layer,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run app");
}
```

Commands receive `State<AppState>` automatically. The state is guaranteed to outlive the command.

```rust
#[tauri::command]
async fn add_layer(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let doc = state.document.lock().await;
    doc.add_layer();
    Ok(())
}
```

### Event emission patterns

Emit events from the app to update the UI:

```rust
#[tauri::command]
async fn add_layer(
    window: tauri::Window,
    state: tauri::State<'_, AppState>,
) -> Result<u32, String> {
    let mut doc = state.document.lock().await;
    let layer_id = doc.add_layer();
    
    // Notify the UI.
    window.emit("layer:added", LayerEvent { id: layer_id })
        .map_err(|e| e.to_string())?;
    
    Ok(layer_id)
}
```

On the frontend (Solid.js):

```typescript
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export function useLayerEvents() {
    onMount(async () => {
        await listen("layer:added", (event) => {
            console.log("Layer added:", event.payload);
            // Update UI state
        });
    });
}
```

### Window management

Create windows dynamically if needed:

```rust
#[tauri::command]
async fn open_export_dialog(
    app: tauri::AppHandle,
) -> Result<(), String> {
    let export_window = tauri::window::WindowBuilder::new(
        &app,
        "export",
        tauri::WindowUrl::App("export.html".into()),
    )
    .build()
    .map_err(|e| e.to_string())?;
    
    Ok(())
}
```

Most commands operate on the main window. Only open new windows for long-running dialogs or separate panels.

### Avoid cross-thread issues with the main thread

The Tauri event loop runs on the main thread. Commands run in a thread pool. State access must be thread-safe.

Never store `Window` in app state. It's not `Send`. Store window handles (which are `Send`) and create new handles from `AppHandle`:

```rust
// BAD: Window is not Send
pub struct AppState {
    window: Window,
}

// GOOD: AppHandle is Send
pub struct AppState {
    app_handle: AppHandle,
}

#[tauri::command]
async fn notify_ui(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    app.get_window("main")
        .unwrap()
        .emit("update", ())
        .map_err(|e| e.to_string())
}
```

---

## 8. Serde patterns

### Derive vs. custom impl

Use `#[derive(Serialize, Deserialize)]` unless you need custom logic:

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub opacity: u8,
}
```

Custom impl when you need to:
- Handle versioning (old format -> new format).
- Skip or rename fields.
- Validate on deserialize.

```rust
impl<'de> Deserialize<'de> for Layer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct LayerV1 {
            name: String,
            visible: bool,
            opacity: u8,
        }
        
        let v1 = LayerV1::deserialize(deserializer)?;
        Ok(Layer {
            name: v1.name,
            visible: v1.visible,
            opacity: v1.opacity,
        })
    }
}
```

### Versioning serialized structures

Always include a version field:

```rust
#[derive(Serialize, Deserialize)]
pub struct ProjectFile {
    pub version: u32,  // Bump this when schema changes.
    pub metadata: Metadata,
    pub frames: Vec<Frame>,
}

const CURRENT_VERSION: u32 = 1;

impl ProjectFile {
    pub fn load(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let file: ProjectFile = serde_json::from_str(&data)?;
        
        match file.version {
            1 => Ok(file),
            2 => migrate_v1_to_v2(file),
            _ => Err(anyhow!("unsupported version: {}", file.version)),
        }
    }
}
```

### Schema migration patterns

When you bump the version, provide a migration function:

```rust
fn migrate_v1_to_v2(mut v1: ProjectFile) -> Result<ProjectFile> {
    // In v2, we added an "author" field to metadata.
    // Default it to "unknown".
    v1.metadata.author = "unknown".to_string();
    v1.version = 2;
    Ok(v1)
}
```

Store migration functions in a separate module:

```rust
// src/project/migration.rs
pub fn v1_to_v2(mut file: ProjectFile) -> Result<ProjectFile> {
    file.metadata.author = "unknown".to_string();
    file.version = 2;
    Ok(file)
}

pub fn apply_migrations(mut file: ProjectFile) -> Result<ProjectFile> {
    while file.version < CURRENT_VERSION {
        file = match file.version {
            1 => v1_to_v2(file)?,
            _ => return Err(anyhow!("unknown version")),
        };
    }
    Ok(file)
}
```

### ts-rs / specta for TypeScript binding generation

If you want to auto-generate TypeScript types from Rust, use `specta`:

```rust
use specta::{Type, TypeScriptify};

#[derive(Type, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub opacity: u8,
}

// In your build.rs or a standalone generator:
// specta::export::<Layer>("bindings/");
```

This generates `bindings/Layer.ts`:

```typescript
export interface Layer {
    name: string;
    visible: boolean;
    opacity: number;
}
```

Keep types in sync without manual copy-paste. If an agent adds a field to a Rust struct, the TypeScript type updates automatically.

---

## 9. Testing posture

### Structure: inline tests vs. tests/ directory

**Inline tests (unit tests):**

```rust
// In src/lib.rs or src/blend.rs
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_alpha_blend() {
        let src = [255, 0, 0, 255];    // Red, fully opaque
        let mut dst = [0, 255, 0, 255]; // Green, fully opaque
        
        blend_pixel(&src, &mut dst, 128);
        
        // Result should be 50% red, 50% green.
        assert!(dst[0] > 100 && dst[0] < 150);
        assert!(dst[1] > 100 && dst[1] < 150);
    }
}
```

Inline tests are tight to the code. Use them for public API contracts and obvious behavior.

**Integration tests (tests/ directory):**

```rust
// tests/integration.rs
use pixhaus_io::*;

#[test]
fn test_png_roundtrip() {
    let buffer = PixelBuffer::new(100, 100);
    let encoded = encode_png(&buffer).unwrap();
    let decoded = decode_png(&encoded).unwrap();
    
    assert_eq!(decoded.width(), 100);
    assert_eq!(decoded.height(), 100);
}
```

Integration tests exercise entire subsystems. Use them for end-to-end flows.

Rule of thumb: every public function has at least one test. Integration tests cover major workflows. Internal functions tested via their public consumers.

### Property-based tests for image operations

For pixel operations, use `proptest`:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_blend_idempotent(
        r in 0u8..=255,
        g in 0u8..=255,
        b in 0u8..=255,
    ) {
        let src = [r, g, b, 255];
        let mut dst = [0, 0, 0, 255];
        
        blend_pixel(&src, &mut dst, 0);  // Alpha = 0 should not change dst.
        
        assert_eq!(dst, [0, 0, 0, 255]);
    }
}
```

Property tests generate random inputs. If your code has edge cases in alpha blending or color space, property tests find them. Agents rarely write these without prompting. Ask for them.

### Snapshot tests for blend mode output

Some operations are hard to assert with numbers (color accuracy is subjective). Use snapshot tests:

```rust
// tests/blend_snapshots.rs
#[test]
fn test_multiply_blend_snapshot() {
    let src = generate_test_image();
    let mut dst = generate_test_image();
    
    apply_blend_mode(&src, &mut dst, BlendMode::Multiply);
    
    // Save to .golden file on first run, compare on subsequent runs.
    insta::assert_snapshot!(format!("{:?}", dst));
}
```

Use `insta` for this. First run creates a `.golden` file. Subsequent runs compare. If output changes (intentionally), you update the golden file in code review.

### Visual regression for canvas rendering

For Tauri apps with canvas rendering, capture screenshots:

```rust
// tests/visual_regression.rs (if you have headless rendering)
#[test]
#[ignore]  // Run manually or in CI with display server.
fn test_checkerboard_pattern() {
    let canvas = render_checkerboard(100, 100);
    let image = canvas.to_png();
    
    // Compare to baseline.png, allowing X% pixel difference.
    let baseline = load_baseline("checkerboard.png");
    assert_images_similar(&image, &baseline, 0.01);  // 1% tolerance
}
```

Visual regression is fragile (font rendering, AA, etc.). Use high tolerances and run in CI only.

### Every public function has a test

This is a rule, not a guideline. When an agent submits code, check: does every public function have at least one test?

If not, that's a blocker. Doesn't have to be exhaustive, but it has to exist.

```rust
pub fn new(width: u32, height: u32) -> Self {
    // Must have a test:
    // #[test]
    // fn test_new_creates_correct_size() { }
}
```

### Mock vs. real for AI backend tests

When testing AI integration, mock the LLM:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    struct MockAiClient;
    
    #[async_trait]
    impl AiClient for MockAiClient {
        async fn complete(&self, prompt: &str) -> Result<String> {
            Ok("mocked response".to_string())
        }
    }
    
    #[tokio::test]
    async fn test_describe_generates_text() {
        let client = MockAiClient;
        let result = describe_image("test prompt", &client).await.unwrap();
        
        assert!(!result.is_empty());
    }
}
```

Don't make tests hit real APIs. They'll be slow, flaky, and expensive. Mock the boundary.

---

## 10. Documentation conventions

### Crate-level docs with `//!`

Every crate has a `lib.rs` with a doc comment:

```rust
//! Pixhaus core: pixel buffer management, layers, frames, and composition.
//!
//! # Overview
//!
//! The core crate provides the fundamental types for image editing:
//! - `PixelBuffer`: Raw pixel data with stride support.
//! - `Layer`: A composition unit with blend modes and opacity.
//! - `Frame`: A container for multiple layers.
//!
//! # Example
//!
//! ```
//! use pixhaus_core::{PixelBuffer, Layer};
//!
//! let mut buffer = PixelBuffer::new(100, 100);
//! let layer = Layer::new("Background");
//! ```

pub use crate::buffer::PixelBuffer;
pub use crate::layer::Layer;
pub use crate::frame::Frame;
pub use crate::error::Result;
```

This is the entry point for `cargo doc`. Make it count.

### Item-level docs with `///`

Every public item gets a doc comment:

```rust
/// A 2D pixel buffer with explicit stride.
///
/// # Fields
/// - `pixels`: Raw RGBA data, row-major layout.
/// - `width`: Width in pixels.
/// - `height`: Height in pixels.
/// - `stride`: Bytes per row (may be > width * 4 for alignment).
///
/// # Example
///
/// ```
/// let buffer = PixelBuffer::new(100, 100);
/// assert_eq!(buffer.width(), 100);
/// ```
pub struct PixelBuffer {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    stride: u32,
}

/// Blends two colors with the given alpha.
///
/// Returns the blended result in RGBA format.
///
/// # Panics
///
/// None. This function always succeeds.
pub fn blend(src: &[u8; 4], dst: &[u8; 4], alpha: u8) -> [u8; 4] {
    // ...
}
```

Agents sometimes skip docs. You push back. Every public item. No exceptions.

### Cargo doc discipline

Run `cargo doc --open` regularly. Fix warnings.

```bash
cargo doc --no-deps --open
```

This builds and opens the HTML. If docs are missing, cargo warns. If examples don't compile, cargo fails. Keep it green.

### Rustdoc tests

Examples in doc comments are runnable tests:

```rust
/// # Example
/// ```
/// let buffer = PixelBuffer::new(100, 100);
/// assert_eq!(buffer.width(), 100);
/// ```
```

`cargo test` runs these. They'll fail if the API changes. This keeps examples in sync with code.

---

## 11. Common patterns agents get wrong in Rust

### Over-using Box<dyn Trait> when generics are right

Agents often default to `Box<dyn Trait>` for flexibility. Don't let them.

**Wrong:**
```rust
fn apply_filter(buffer: &mut PixelBuffer, filter: Box<dyn Filter>) {
    filter.apply(buffer);
}
```

**Right (usually):**
```rust
fn apply_filter<F: Filter>(buffer: &mut PixelBuffer, filter: F) {
    filter.apply(buffer);
}
```

Generics are monomorphized at compile time. No runtime dispatch, no allocation. Dyn traits are slower and harder to reason about.

When you call `apply_filter(buffer, SomeConcreteFilter)`, the compiler generates a copy of the function with `F = SomeConcreteFilter`. No virtual dispatch.

Use `Box<dyn Trait>` only when:
- You actually need heterogeneous collections of different types.
- The type is unknown at compile time.

### Returning &str from a function that owns the String

Classic mistake:

```rust
// WRONG
fn get_name() -> &str {
    let name = String::from("Alice");
    &name  // Dangling reference; name dropped at end of scope
}

// RIGHT
fn get_name() -> String {
    String::from("Alice")
}

// ALSO RIGHT (if you can borrow from a caller)
fn get_name<'a>(buffer: &'a str) -> &'a str {
    &buffer[0..5]
}
```

Lifetimes are hard for agents. You'll see borrowed references where owned values are needed. When the borrow checker rejects code, don't just `.clone()` as a reflex. Understand whether the value should be owned or borrowed.

### Premature Arc<Mutex<>> when single-owner is fine

Agents sometimes wrap things in Arc<Mutex<>> for "flexibility":

```rust
// WRONG: single owner, no concurrency
pub struct Document {
    data: Arc<Mutex<DocumentData>>,
}

// RIGHT: single owner, no Mutex needed
pub struct Document {
    data: DocumentData,
}

// MUTEX ONLY if multiple threads need to mutate:
pub struct AppState {
    document: Arc<Mutex<Document>>,
}
```

Arc<Mutex<>> has real costs: allocations, lock overhead, less compiler help. Use only when you need it.

### Unnecessary .clone() calls

```rust
// WRONG
let x = vec.clone();
process(&x);

// RIGHT
process(&vec);
```

Teach agents to borrow by default. Clone only when ownership is needed. Rust's borrow checker will tell you when borrowing isn't enough.

### Using Vec<Vec<T>> for 2D data when flat Vec is right

```rust
// WRONG: cache-unfriendly, complex indexing
let grid: Vec<Vec<u8>> = (0..height)
    .map(|_| vec![0u8; width])
    .collect();
grid[y][x] = value;

// RIGHT: cache-friendly, simple indexing
let grid = vec![0u8; width * height];
grid[y * width + x] = value;

// EVEN BETTER for image data: explicit stride
let stride = (width * 4 + 15) / 16 * 16;  // Aligned for SIMD
let buffer = vec![0u8; stride * height];
```

2D pixel data should be flat. Vec<Vec> fragments memory and destroys cache locality. For image data, always use a flat buffer with stride.

### Re-implementing standard iterators

```rust
// WRONG
for i in 0..data.len() {
    process(&data[i]);
}

// RIGHT
for item in data {
    process(item);
}

// ALSO RIGHT if you need indices
for (i, item) in data.iter().enumerate() {
    process(i, item);
}
```

Agents sometimes index explicitly. Show them `iter()`, `enumerate()`, `zip()`. These are cleaner and often faster.

### Misusing async

```rust
// WRONG: spawn unnecessary tasks
let result = tokio::spawn(async { 1 + 2 }).await.unwrap();

// WRONG: block inside async
let x = std::thread::sleep(Duration::from_secs(1));  // Blocks executor!

// RIGHT: await directly
let result = some_async_op().await;

// RIGHT: use tokio::time
tokio::time::sleep(Duration::from_secs(1)).await;
```

Agents will spawn tasks unnecessarily and sometimes block the executor with std::thread::sleep or blocking I/O. Review async code carefully.

### Wrong choice of String vs. &str vs. Cow<str>

```rust
// WRONG: takes ownership unnecessarily
fn process(name: String) {
    println!("{}", name);
}

// RIGHT: borrow
fn process(name: &str) {
    println!("{}", name);
}

// OKAY: returns owned value
fn build_string() -> String {
    format!("Hello, {}", name)
}

// GOOD: flexible, clones only when mutated
fn maybe_modify<'a>(input: &'a str) -> Cow<'a, str> {
    if needs_modification(input) {
        Cow::Owned(input.to_uppercase())
    } else {
        Cow::Borrowed(input)
    }
}
```

API design rule: accept `&str`, return `String` (unless the caller owns the data and you can borrow it).

### Premature optimization with unsafe

```rust
// WRONG: unsafe without proof of safety
unsafe {
    std::ptr::copy_nonoverlapping(src, dst, count);
}

// RIGHT: use safe APIs
dst.copy_from_slice(src);

// UNSAFE OKAY only when absolutely necessary and documented
unsafe {
    // SAFETY: We've verified that src.len() == dst.len() == count.
    // Both pointers are valid and properly aligned.
    std::ptr::copy_nonoverlapping(src, dst, count);
}
```

Agents will reach for `unsafe` thinking it's faster. It's not. It's dangerous and wrong. Push back hard.

### Misusing mem::take and mem::replace

```rust
// WRONG: cloning when moving
let old_state = state.clone();
state = new_state;

// RIGHT: swap without cloning
let old_state = mem::take(&mut state);
state = new_state;

// Also right
let old_state = mem::replace(&mut state, new_state);
```

`mem::take` and `mem::replace` are low-level. Use them only when you need to avoid a clone and the performance matters. They're not idiomatic for normal code.

---

## 12. Rust idioms to teach agents explicitly

### Builder pattern

```rust
pub struct FrameBuilder {
    width: u32,
    height: u32,
    layers: Vec<Layer>,
    metadata: Metadata,
}

impl FrameBuilder {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            layers: Vec::new(),
            metadata: Metadata::default(),
        }
    }
    
    pub fn with_layer(mut self, layer: Layer) -> Self {
        self.layers.push(layer);
        self
    }
    
    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = metadata;
        self
    }
    
    pub fn build(self) -> Result<Frame> {
        // Validate and construct
        Frame::new(self.width, self.height, self.layers, self.metadata)
    }
}

// Usage
let frame = FrameBuilder::new(100, 100)
    .with_layer(layer1)
    .with_layer(layer2)
    .with_metadata(meta)
    .build()?;
```

Builders let you construct complex objects with optional configuration. Teach agents to recognize when a struct has many optional fields, a builder is the right pattern.

### Newtype wrappers for type safety

```rust
// Wrong: easy to mix up
fn set_opacity(frame_id: u32, opacity: u32) { }

// Right: impossible to confuse
#[derive(Copy, Clone)]
pub struct FrameId(u32);

#[derive(Copy, Clone)]
pub struct Opacity(u8);  // 0-255

fn set_opacity(frame_id: FrameId, opacity: Opacity) { }

// Now these are type errors:
// set_opacity(Opacity(128), FrameId(1));  // COMPILE ERROR
// set_opacity(1, 128);                     // COMPILE ERROR
```

Newtypes prevent semantic errors at compile time. Use them for IDs, handles, and domain concepts.

### Sealed traits

Prevent external implementations of a trait:

```rust
pub trait Layer: sealed::Sealed {
    fn render(&self) -> PixelBuffer;
}

mod sealed {
    pub trait Sealed {}
    
    impl Sealed for crate::layers::RasterLayer {}
    impl Sealed for crate::layers::TextLayer {}
    // External crates can't implement Layer
}

pub struct RasterLayer { /* ... */ }
impl sealed::Sealed for RasterLayer {}
impl Layer for RasterLayer { /* ... */ }
```

This lets you add new trait methods without breaking external implementations.

### Type-state pattern for invalid-state-elimination

```rust
pub struct Loaded;
pub struct Ready;

pub struct Canvas<State> {
    pixels: Vec<u8>,
    state: std::marker::PhantomData<State>,
}

impl Canvas<Loaded> {
    pub fn new(width: u32, height: u32) -> Self {
        Canvas { pixels: vec![0; width * height * 4], state: PhantomData }
    }
    
    pub fn validate(self) -> Result<Canvas<Ready>> {
        // Validate...
        Ok(Canvas { pixels: self.pixels, state: PhantomData })
    }
}

impl Canvas<Ready> {
    pub fn render(&self) { /* ... */ }
}

// Usage
let canvas = Canvas::new(100, 100);
// canvas.render();  // COMPILE ERROR: Canvas<Loaded> has no render()
let ready = canvas.validate()?;
ready.render();     // OK
```

Type-state eliminates invalid states. Use it when certain operations are only valid after preconditions are met.

### The Drop trait for resource cleanup

```rust
pub struct TempFile {
    path: PathBuf,
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

// Automatically cleans up when dropped
{
    let temp = TempFile { path: "/tmp/tmp_file".into() };
    // ... use temp ...
}  // drop() called automatically
```

Use Drop for resource cleanup. It's guaranteed to run (unless the program panics or aborts).

### From/Into for conversions

```rust
impl From<u32> for Rgba8 {
    fn from(value: u32) -> Self {
        let r = ((value >> 24) & 0xFF) as u8;
        let g = ((value >> 16) & 0xFF) as u8;
        let b = ((value >> 8) & 0xFF) as u8;
        let a = (value & 0xFF) as u8;
        Rgba8::new(r, g, b, a)
    }
}

// From impl automatically gives you Into
let rgba: Rgba8 = 0xFF0000FFu32.into();
```

Implement From for your types. Into comes free. This makes APIs flexible.

### Display vs. Debug

```rust
use std::fmt;

#[derive(Debug)]
pub struct Color { r: u8, g: u8, b: u8 }

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

// Debug: {:?}, derive-able
println!("{:?}", color);  // Color { r: 255, g: 0, b: 0 }

// Display: {}, implement manually
println!("{}", color);    // #ff0000
```

Debug is for developers. Display is for users. Implement Display for user-facing types.

### Constructors in module, methods in impl

```rust
// src/layer.rs
pub struct Layer { /* ... */ }

// Constructor at module level (or impl)
pub fn new(name: &str) -> Layer { Layer { /* ... */ } }

// Methods in impl block
impl Layer {
    pub fn name(&self) -> &str { /* ... */ }
    pub fn set_opacity(&mut self, opacity: u8) { /* ... */ }
}
```

This is style, not law. But it's conventional. Keep constructors together, methods together.

---

## 13. Code review checklist for AI-generated Rust

When an agent submits a Rust module, run through this checklist before merging:

- **Error paths covered?** Does every fallible operation return a Result? No unwrap() or expect() in production code?
- **Test coverage adequate?** Does every public function have at least one test? Are edge cases tested?
- **Documentation present?** Does every public item have a `///` comment with an example?
- **No panics in production code?** Search for `unwrap()`, `expect()`, `panic!()` outside of tests.
- **Async correctness?** Are locks held across `.await`? Are blocking operations marked with `spawn_blocking()`?
- **Public API surface minimal?** Is only what should be public marked `pub`? Are internal modules private?
- **Allocations in hot paths?** Any `Vec::new()` or `String::from()` in tight loops?
- **Unsafe usage justified?** Is there a SAFETY comment explaining why it's safe?
- **Lifetimes sensible?** Are there unnecessary lifetime parameters? Does the code borrow when it should own?
- **No Over-cloning?** Are there unnecessary `.clone()` calls?
- **Concurrency safe?** If there's shared state, is it properly protected with Arc/Mutex? No raw pointers?
- **Dependencies justified?** Did the agent add new crate dependencies? Are they necessary?
- **Compilation warnings?** Run `cargo clippy` and address all warnings.

---

## 14. Pixhaus-specific idioms

### Pixel buffer ownership

In Pixhaus, a PixelBuffer is owned by exactly one Layer. A Layer is owned by exactly one Frame. A Frame is owned by the Document.

```rust
pub struct PixelBuffer { /* ... */ }

pub struct Layer {
    buffer: PixelBuffer,
    blend_mode: BlendMode,
    opacity: u8,
}

pub struct Frame {
    layers: Vec<Layer>,
    width: u32,
    height: u32,
}

pub struct Document {
    frames: Vec<Frame>,
    current_frame: usize,
}
```

No shared ownership of pixel data. No Arc<Mutex<PixelBuffer>>. Single owner simplifies reasoning and avoids synchronization overhead.

When you need to read pixels for blending, borrow the buffer:

```rust
impl Frame {
    pub fn composite(&self) -> Result<PixelBuffer> {
        let mut result = PixelBuffer::new(self.width, self.height);
        
        for layer in &self.layers {
            if layer.visible {
                layer.composite_onto(&mut result)?;
            }
        }
        
        Ok(result)
    }
}

impl Layer {
    fn composite_onto(&self, target: &mut PixelBuffer) -> Result<()> {
        // Borrow the layer's buffer, blend into target
        blend_pixels(&self.buffer, target, self.opacity)
    }
}
```

### Layer stack representation

Layers are a Vec in order (bottom to top):

```rust
pub struct Frame {
    layers: Vec<Layer>,  // layers[0] is bottom
}

impl Frame {
    pub fn add_layer_at(&mut self, index: usize, layer: Layer) {
        self.layers.insert(index, layer);
    }
    
    pub fn remove_layer(&mut self, index: usize) -> Option<Layer> {
        if index < self.layers.len() {
            Some(self.layers.remove(index))
        } else {
            None
        }
    }
    
    pub fn move_layer(&mut self, from: usize, to: usize) {
        if from < self.layers.len() && to < self.layers.len() {
            let layer = self.layers.remove(from);
            self.layers.insert(to, layer);
        }
    }
}
```

Simple, straightforward. No fancy data structures needed.

### Frame timeline data structure

Frames are stored in a Vec. Current frame index is tracked:

```rust
pub struct Document {
    frames: Vec<Frame>,
    current_frame: usize,
}

impl Document {
    pub fn current(&self) -> &Frame {
        &self.frames[self.current_frame]
    }
    
    pub fn current_mut(&mut self) -> &mut Frame {
        &mut self.frames[self.current_frame]
    }
    
    pub fn next_frame(&mut self) {
        if self.current_frame + 1 < self.frames.len() {
            self.current_frame += 1;
        }
    }
    
    pub fn prev_frame(&mut self) {
        if self.current_frame > 0 {
            self.current_frame -= 1;
        }
    }
}
```

For animation playback, iterate with timing:

```rust
pub struct Playback {
    document: Arc<Mutex<Document>>,
    current_time_ms: u64,
    fps: u32,
}

impl Playback {
    pub fn update(&mut self, dt_ms: u64) {
        self.current_time_ms += dt_ms;
        
        let doc = self.document.lock().unwrap();
        let frame_duration_ms = 1000 / self.fps as u64;
        let frame_idx = (self.current_time_ms / frame_duration_ms) as usize;
        
        drop(doc);  // Explicit drop to avoid holding lock
        
        if frame_idx < doc.frames.len() {
            doc.current_frame = frame_idx;
        }
    }
}
```

### Selection mask types

A selection is a set of pixels. Represent it efficiently:

```rust
#[derive(Clone)]
pub enum SelectionMask {
    All,
    None,
    Pixels(BitVec),  // One bit per pixel
    BoundingBox(Rect),  // Fast for rectangular selections
    Custom(Box<dyn Fn(u32, u32) -> bool>),  // Arbitrary predicate (slow)
}

impl SelectionMask {
    pub fn is_selected(&self, x: u32, y: u32) -> bool {
        match self {
            SelectionMask::All => true,
            SelectionMask::None => false,
            SelectionMask::Pixels(bits) => {
                let idx = y as usize * width + x as usize;
                bits[idx]
            }
            SelectionMask::BoundingBox(rect) => rect.contains(x, y),
            SelectionMask::Custom(pred) => pred(x, y),
        }
    }
}
```

Use BitVec for pixel-level selections (memory-efficient). Use rects for fast paths.

### Command pattern for undo

Every user action is a Command. Commands know how to apply themselves and undo:

```rust
pub trait Command {
    fn apply(&mut self, document: &mut Document) -> Result<()>;
    fn undo(&mut self, document: &mut Document) -> Result<()>;
    fn description(&self) -> &str;
}

pub struct AddLayerCommand {
    layer: Option<Layer>,  // None after apply
}

impl Command for AddLayerCommand {
    fn apply(&mut self, document: &mut Document) -> Result<()> {
        if let Some(layer) = self.layer.take() {
            document.add_layer(layer);
            Ok(())
        } else {
            Err(anyhow!("layer already applied"))
        }
    }
    
    fn undo(&mut self, document: &mut Document) -> Result<()> {
        if let Some(layer) = document.remove_layer(document.current_layer) {
            self.layer = Some(layer);
            Ok(())
        } else {
            Err(anyhow!("nothing to undo"))
        }
    }
    
    fn description(&self) -> &str {
        "Add Layer"
    }
}

pub struct UndoStack {
    commands: Vec<Box<dyn Command>>,
    current: usize,
}

impl UndoStack {
    pub fn apply(&mut self, mut command: Box<dyn Command>, document: &mut Document) -> Result<()> {
        command.apply(document)?;
        self.commands.truncate(self.current);
        self.commands.push(command);
        self.current += 1;
        Ok(())
    }
    
    pub fn undo(&mut self, document: &mut Document) -> Result<()> {
        if self.current > 0 {
            self.current -= 1;
            self.commands[self.current].undo(document)?;
            Ok(())
        } else {
            Err(anyhow!("nothing to undo"))
        }
    }
}
```

Commands own the data they manipulate. Apply moves it in, undo moves it back out. Simple, composable.

---

## 15. AI agent prompting patterns for Rust dev

When you ask an agent to write a Rust module, be specific:

### Brief example: Add a hue-saturation filter

Instead of:
> "Add a hue-saturation filter"

Say:
> Add a `HueSaturationFilter` to `pixhaus-core/src/filters/`.
> 
> **Signature:**
> ```rust
> pub struct HueSaturationFilter {
>     pub hue_shift: i16,      // -180 to 180
>     pub saturation: f32,     // 0.0 to 2.0
> }
> 
> impl HueSaturationFilter {
>     pub fn apply(&self, buffer: &mut PixelBuffer) -> Result<()>;
> }
> ```
> 
> **Behavior:**
> - Convert RGBA to HSV, shift hue, adjust saturation, convert back to RGBA.
> - Validate inputs: hue in [-180, 180], saturation in [0, 2].
> - Return `anyhow::Error` if validation fails.
> 
> **Tests:**
> - Test with neutral values (hue 0, saturation 1.0) should be identity.
> - Test with hue 180 should invert colors.
> - Test with saturation 0 should produce grayscale.
> 
> **Documentation:**
> - Doc comment with example usage.
> - Include SAFETY comment if any unsafe code.

This tells the agent:
- The public API.
- The behavior and edge cases.
- The error handling strategy.
- The test expectations.
- The documentation requirement.

### Iterative refinement loop

1. **Initial implementation:** Agent writes the code.
2. **Review:** You run `cargo clippy`, check tests, verify docs.
3. **Feedback:** You tell the agent what's wrong (if anything).
4. **Refinement:** Agent updates.
5. **Merge:** Once it passes review.

Example conversation:

> **You:** "The filter test is failing. Add a test for the identity case (hue 0, saturation 1.0). Make sure input and output are byte-identical."
>
> **Agent:** [Writes test, it fails]
>
> **You:** "The test is right. Now fix the apply() implementation. The HSV round-trip is losing precision somewhere."
>
> **Agent:** [Fixes implementation, test passes]

This is normal. Agents write code, humans tune it. Expect 3-5 iterations for complex features.

---

## Patterns of genuine uncertainty in 2026

These are areas where the ecosystem is still in flux or where best practices diverge:

### 1. Async-trait for library APIs

In 1.75+, native async fn in traits is stable. Most libraries have migrated. But some still use `async-trait` for compatibility or historical reasons.

**Our guidance:** Use native async fn. If you support MSRV (minimum supported Rust version) older than 1.75, then `async-trait`.

For Pixhaus (no external MSRV guarantee), use native async.

### 2. Error handling in library vs. application

The boundary is fuzzy. Some libraries prefer returning `anyhow::Error` (simple). Others prefer custom error types (precise).

**Our guidance:** Library crates (`pixhaus-core`, etc.) use custom error types. Application crate uses `anyhow`.

### 3. Concurrency abstraction (tokio vs. async-std vs. embassy)

Tokio dominates, but async-std and embassy exist. Tauri uses tokio, so we're locked in.

**Our guidance:** Tokio for Pixhaus. No choice.

### 4. Serialization (serde vs. bincode vs. custom)

Serde is standard. Bincode is faster but less human-readable. Custom is rare but sometimes optimal.

**Our guidance:** serde + serde_json for human-readable config/project files. Consider bincode if we profile and find serialization is a bottleneck.

### 5. SIMD maturity

Portable SIMD is stable (as of 1.82) for basic ops, but not all operations are available. Intrinsics or vendor-specific crates may be necessary.

**Our guidance:** Try std::simd first. Fall back to explicit intrinsics or `packed_simd` if needed. Don't premature-optimize.

---

## Summary: The five core rules

1. **Error handling:** Library crates use `thiserror`. Application uses `anyhow`. Every fallible operation returns `Result`. No panics in production.

2. **Async safety:** No locks across `.await`. No `std::thread::sleep()` in async code. Use `tokio::time::sleep()`. No blocking operations in the main task pool without `spawn_blocking()`.

3. **Shared state:** Arc<Mutex<>> only when multiple threads need writeable access. Default to single ownership. Use channels for cross-thread communication.

4. **Memory discipline:** Clone sparingly. Borrow by default. Use flat buffers with stride for pixel data, not Vec<Vec>. No allocations in hot loops.

5. **Testing and documentation:** Every public function has a test and a doc comment. Clippy passes. No warnings. Done is documented.

---

## Document statistics and key conventions

**Word count:** 7,200 words.

**Top 3 most important conventions:**

1. **Error handling orthodoxy** (thiserror for libraries, anyhow for apps, no panics in production code). This prevents silent failures and makes production code resilient.

2. **Async safety discipline** (no locks across await, no blocking main thread, explicit spawn_blocking for CPU work). Violating this causes deadlocks and UI freezes—deal-breakers in a desktop editor.

3. **Single ownership of mutable state** (every document owned by exactly one holder, no Arc<Mutex<>> except for IPC state). This simplifies reasoning about concurrency and eliminates entire classes of bugs.

**Patterns of genuine uncertainty in 2026:**

- **Async trait maturity:** Native async fn in traits is stable since 1.75, but ecosystem migration is ongoing. Some libraries still use `async-trait` for compatibility. Guidance: use native async for new code.

- **SIMD stability:** Portable SIMD is stable (1.82+) but incomplete. If edge cases arise, may need to fall back to vendor-specific intrinsics or packed_simd. Guidance: profile before optimizing.

- **Library error types vs. anyhow:** The boundary between "custom error enum" and "just use anyhow" is context-dependent. Guidance: custom errors for public library APIs, anyhow at app boundaries.

---

**End of document.**
