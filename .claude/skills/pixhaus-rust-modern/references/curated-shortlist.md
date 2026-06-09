# The curated shortlist: highest-value 1.85-1.96 features for Pixhaus

The ~15 features that actually pay off in this codebase, each with the exact spot it earns its keep — the long form of the SKILL.md shortlist. Part of the `pixhaus-rust-modern` skill; start at its `SKILL.md` for the shortlist and the per-version cheat sheet.

This is the shortlist to actually reach for. The 1.85→1.96 window stabilized hundreds of items; most are SIMD intrinsics, target features, and FFI plumbing that never touch a pixel-art editor. What follows is the ~15 that pay off in *this* code — the egui/wgpu shell, `Vec<u8>` pixel buffers, the `Command`/undo model, tokio-owned async, and the `Arc<dyn Backend>` AI runtime — with the exact spot each one earns its keep.

The rule up front: adopt a feature where it removes a real footgun or a layer of indirection in this codebase, not because it is new. Working code that already reads clearly is not a migration target. Several items below are edition-gated; the workspace is on edition 2024, so they are live.

A note on the window: the bottom of the range (1.85) is the floor because edition 2024 itself stabilized there. The workspace pins 1.96, so the whole window — including the recent 1.95 and 1.96 items — compiles today. `rust-toolchain.toml` stays the source of truth, so check it before assuming a future feature past 1.96 is available.

## 1. Edition 2024 (1.85)

The whole window sits on this. Edition 2024 is what unlocks let-chains, the new RPIT capture rules, and the temporary-scope changes the rest of this list depends on. It is per-crate and opt-in; 2024 crates interoperate with older ones, so there is no ecosystem split to fear.

```rust
// Cargo.toml — already set across the Pixhaus workspace
edition = "2024"
```

How to apply: it is already on. The payoff is everything below that says "edition-gated." If you spin up a new crate in `crates/` or `modules/`, set `edition = "2024"` so it inherits the same surface.

## 2. let-chains in `if`/`while` (1.88, edition 2024)

This is the single highest-frequency cleanup in the codebase. Tool-interaction code and command `apply`/`undo` paths constantly need "this `Option`/`Result` matched *and* a bool holds." The old shape is a pyramid of nested `if let`. let-chains flatten it.

```rust
// OLD: nested ifs to combine a match and a guard
fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
    if let Some(sprite) = doc.sprite_mut(self.sprite_id) {
        if let Some(cel) = sprite.active_cel_mut() {
            if cel.is_unlocked() {
                self.paint_into(cel);
            }
        }
    }
    Ok(())
}
```

```rust
// NEW (1.88): one condition, no pyramid
fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
    if let Some(sprite) = doc.sprite_mut(self.sprite_id)
        && let Some(cel) = sprite.active_cel_mut()
        && cel.is_unlocked()
    {
        self.paint_into(cel);
    }
    Ok(())
}
```

When not to reach for it: if a missing case needs its own `else` (an error return, a warn log), keep the explicit `match`/`if let`-`else`. let-chains have no per-arm else, so forcing one in defeats the readability win.

## 3. `if let` guards on match arms (1.95)

The match-arm sibling of let-chains. The dispatcher that turns a tool intent or a job result into a command is a `match`; arms there often need to both pattern-match the variant and pull a value out of a fallible lookup. Before, you matched the variant, then re-did the fallible work inside the arm (often with an `.unwrap()` the no-unwrap rule forbids).

```rust
// OLD: re-do the fallible lookup inside the arm
match intent {
    ToolIntent::ApplyAt(pos) if backend_for(pos).is_some() => {
        let backend = backend_for(pos).expect("just checked"); // banned by no-unwrap
        backend.run();
    }
    _ => {}
}
```

```rust
// NEW (1.95): bind in the guard, use it in the body
match intent {
    ToolIntent::ApplyAt(pos) if let Some(backend) = backend_for(pos) => {
        backend.run(); // bound once, no unwrap, no double lookup
    }
    _ => {}
}
```

Live on the 1.96 pin.

## 4. async closures and the `AsyncFn*` traits (1.85)

This matters at the AI-runtime boundary. When a job needs to run a per-item async operation — generate N reference frames, retry a backend call — you want to pass an `async` closure that can *borrow* from its captures across the returned future. A plain `|| async move { ... }` forces a move and cannot lend out borrowed state per call.

```rust
// OLD: closure returning an async block — must move captures in
let run = |req: Request| async move { backend.dispatch(req).await };
```

```rust
// NEW (1.85): async closure, bound by AsyncFn, can borrow per call
async fn for_each_frame<F>(reqs: Vec<Request>, run: F)
where
    F: AsyncFn(Request) -> Result<Frame, BackendError>,
{
    for req in reqs {
        let _ = run(req).await;
    }
}

for_each_frame(reqs, async |req| backend.dispatch(req).await).await;
```

The load-bearing detail: bound the parameter with `AsyncFn`/`AsyncFnMut`/`AsyncFnOnce` (in the prelude on all editions), not `Fn() -> impl Future`. The trait bound is what gives you the borrowing behavior. Available on all editions, not gated to 2024.

## 5. `async fn` in traits — and when it is NOT enough (window-wide)

Worth stating because it shapes the `Backend` trait. Native `async fn` in traits works for *static* dispatch. But the AI runtime holds backends as `Arc<dyn Backend>` and dispatches dynamically — and a `dyn`-compatible async trait method is still not something native `async fn` gives you across this window. So the rule for Pixhaus:

- Backend used through a generic (`<B: Backend>`)? Native `async fn` in the trait, no macro.
- Backend stored as `Arc<dyn Backend>` / `Vec<Box<dyn Backend>>`? You still need the `#[async_trait]` boxing (or a hand-rolled `Pin<Box<dyn Future>>`), because the trait must be object-safe.

This is exactly the seam the `pixhaus-async-trait` and `pixhaus-generics-dispatch` skills cover; load them before touching the `Backend` trait. The point here is just: do not assume the stabilized native `async fn` in traits removed the need for the macro at the `dyn` boundary. It did not, across this window.

## 6. `Vec::extract_if` and the `extract_if` family (1.87 Vec, 1.88 HashMap/HashSet, 1.91 BTreeMap/BTreeSet)

Built for "remove the entries matching a predicate and do something with each removed one" — which is the layer/frame/cel deletion shape, and pruning the AI-result cache or the job registry. Before, you drained into a temp `Vec`, filtered, and reassigned, or you index-walked backwards.

```rust
// OLD: partition by hand
let mut kept = Vec::new();
for layer in std::mem::take(&mut doc.layers) {
    if layer.id == target { freed.push(layer); } else { kept.push(layer); }
}
doc.layers = kept;
```

```rust
// NEW: Vec::extract_if (1.87) — removed items yielded for cleanup
let removed: Vec<_> = doc.layers.extract_if(.., |layer| layer.id == target).collect();
for layer in removed {
    self.captured_for_undo.push(layer); // capture before drop, for undo
}
```

```rust
// NEW: HashMap::extract_if (1.88) — prune the result cache
let evicted = cache.extract_if(|_id, entry| entry.is_stale()).count();
```

Note `Vec::extract_if` takes a range argument (`..` for the whole vec). The `HashMap`/`HashSet` form (1.88) and the `BTreeMap`/`BTreeSet` form (1.91) take just the predicate. Reach for it wherever you see a remove-and-collect hand-roll.

## 7. `Vec::pop_if` and `VecDeque::pop_front_if` / `pop_back_if` (1.86 Vec, 1.93 VecDeque)

Small but exact for the undo stack and the job queue. The history is a stack; "pop the last command only if it can coalesce with the incoming one" was an `if let Some(last) = stack.last()` guard followed by a separate `pop()`.

```rust
// OLD: peek, test, then pop
if let Some(top) = self.undo_stack.last() {
    if top.can_merge_with(&incoming) {
        let top = self.undo_stack.pop().expect("just peeked"); // banned
        merged = top.merge(incoming);
    }
}
```

```rust
// NEW: Vec::pop_if (1.86) — one call, no unwrap
if let Some(top) = self.undo_stack.pop_if(|top| top.can_merge_with(&incoming)) {
    merged = top.merge(incoming);
}
```

`VecDeque::pop_front_if`/`pop_back_if` (1.93) do the same for a deque-backed job queue: "take the next job only if the lane is free."

## 8. Disjoint mutable access: `<[_]>::get_disjoint_mut` and `HashMap::get_disjoint_mut` (1.86)

This is a real unlock for compositing and for swap-style commands. Blending two scanlines, or swapping two layers/frames by index, needs two `&mut` into the same slice at once — which the borrow checker rejects through plain indexing, and which previously forced `split_at_mut` gymnastics or an index-and-copy dance.

```rust
// OLD: split_at_mut to get two disjoint &mut into one Vec
let (a, b) = doc.layers.split_at_mut(hi);
let src = &mut a[lo];
let dst = &mut b[0];
std::mem::swap(src, dst);
```

```rust
// NEW (1.86): ask for the two indices directly
if let Ok([src, dst]) = doc.layers.get_disjoint_mut([lo, hi]) {
    std::mem::swap(src, dst); // checked disjoint, no split math
}
```

It returns a `Result` (`GetDisjointMutError`) when indices overlap or are out of bounds, so it stays inside the no-unwrap rule with a clean `if let Ok`. `HashMap::get_disjoint_mut` does the same for two values by key — useful when a command edits two cels keyed by id.

## 9. `<[T]>::as_chunks` and `as_rchunks` (1.88)

Direct hit on the RGBA8 pixel loop. A `PixelBuffer` is a flat `Vec<u8>`; iterating it as 4-byte pixels was `chunks_exact(4)` yielding `&[u8]` you then index `[0..3]`. `as_chunks::<4>()` yields `&[[u8; 4]]` — fixed-size arrays the compiler knows are length 4, which both reads cleaner and helps the optimizer.

```rust
// OLD: chunks_exact yields &[u8], index each channel
for px in buf.as_bytes().chunks_exact(4) {
    let (r, g, b, a) = (px[0], px[1], px[2], px[3]);
    // ...
}
```

```rust
// NEW (1.88): as_chunks yields &[[u8; 4]], plus the ragged remainder
let (pixels, rest) = buf.as_bytes().as_chunks::<4>();
debug_assert!(rest.is_empty(), "RGBA8 buffer length is a multiple of 4");
for &[r, g, b, a] in pixels {
    // r,g,b,a are u8 by destructuring, no per-channel indexing
}
```

Use the safe `as_chunks`, not `as_chunks_unchecked` — the unchecked form is `unsafe`, which is forbidden workspace-wide. `as_rchunks` chunks from the end if you ever need bottom-up row processing.

## 10. The `MaybeUninit` slice toolkit and zeroed allocators (1.93 slice methods, 1.92 `new_zeroed`)

For large GPU staging and import buffers, allocating then immediately zero-filling a multi-megabyte `Vec<u8>` for an 8K layer is wasted work. `Box::new_zeroed_slice` (1.92) gives you a zeroed allocation the allocator can hand over without a separate memset, and the 1.93 slice methods let you fill an uninitialized buffer and then view it as initialized — all without `unsafe`-by-hand byte work beyond the one documented `assume_init` call.

```rust
// NEW (1.92): zeroed staging buffer for an 8K RGBA layer, no double-zero
let len = (width as usize) * (height as usize) * 4;
let staging: Box<[u8]> = {
    // new_zeroed_slice yields Box<[MaybeUninit<u8>]>; zeroed bytes are valid u8
    let buf = Box::<[u8]>::new_zeroed_slice(len);
    unsafe { buf.assume_init() } // the one sanctioned assume_init; see note
};
```

Caveat, and it is the important one: this is the rare place a single `unsafe { assume_init }` is justified, and the workspace forbids `unsafe`. So in Pixhaus, prefer the *safe* write-then-view path — `<[MaybeUninit<T>]>::write_copy_of_slice` then `assume_init_ref` (both 1.93) — when you are copying known bytes in, and keep the zeroed-alloc trick for a follow-up discussion rather than slipping `unsafe` past the lint. 1.92 also documented `MaybeUninit`'s layout as a *guarantee* (same size/align/ABI as `T`), which is what makes `[MaybeUninit<u8>; N]` staging buffers officially sound for the GPU-upload path the `bytemuck`/`wgpu` skills cover.

## 11. `Vec::into_raw_parts` and `String::into_raw_parts` (1.93)

Niche but exact at the `wgpu`/FFI seam. When you hand a pixel buffer's backing allocation to a C-ABI boundary or reconstruct a `Vec` after a round-trip, `into_raw_parts` gives `(ptr, len, capacity)` in one call instead of three separate `as_ptr`/`len`/`capacity` reads that risk drift. Reach for it only at a genuine raw boundary; everywhere else, pass the `Vec`/slice. This is the kind of thing the `bytemuck` skill handles for the common case — `into_raw_parts` is for when you are past what safe casting covers.

## 12. `Vec::push_mut` / `insert_mut` (1.95)

Builder-shaped code in `core` — assembling a document's layers, a palette's swatches, a frame's cels — repeatedly pushed an element and then re-fetched it by `last_mut().unwrap()` to keep configuring it. `push_mut` returns the `&mut` to the just-inserted element directly.

```rust
// OLD: push then re-fetch with an unwrap (banned)
doc.layers.push(Layer::new(id));
let layer = doc.layers.last_mut().expect("just pushed");
layer.set_blend(BlendMode::Normal);
```

```rust
// NEW (1.95): push_mut hands back the reference
let layer = doc.layers.push_mut(Layer::new(id));
layer.set_blend(BlendMode::Normal);
```

`insert_mut` is the positional sibling (insert a layer at an index, keep editing it). `VecDeque` and `LinkedList` gained `push_front_mut`/`push_back_mut`/`insert_mut` in the same release.

## 13. `{integer}::midpoint` and `{float}::midpoint` (int 1.85, signed int 1.87, float 1.85)

Coordinate and color math, where `(a + b) / 2` silently overflows on large canvas dimensions or wraps for `u8` channel averages. `midpoint` computes the average without the intermediate overflow.

```rust
// OLD: overflows for large coords / wraps for u8 channels
let cx = (left + right) / 2;
let avg_r = ((c0.r as u16 + c1.r as u16) / 2) as u8; // widen-to-avoid-overflow dance
```

```rust
// NEW: midpoint, no overflow, no widening
let cx = u32::midpoint(left, right);   // 1.85
let avg_r = u8::midpoint(c0.r, c1.r);  // exact channel average, no u16 detour
let center = f32::midpoint(view_min, view_max); // 1.85, for the camera/zoom math
```

This deletes the widen-then-narrow pattern all over color and viewport code. Signed-integer `midpoint` landed slightly later (1.87) if you average signed offsets.

## 14. `RwLockWriteGuard::downgrade` (1.92)

For the shared caches and registries that genuinely live behind a lock across threads (the `egui-wgpu` `Renderer` is `Arc<RwLock<_>>`; tile/thumbnail caches touched off the UI thread). When a writer finishes mutating and wants to keep reading without a window where another writer can sneak in, `downgrade` converts the write guard to a read guard atomically.

```rust
// OLD: drop the write guard, re-acquire a read guard — a gap opens between
let mut w = cache.write();
w.insert(key, value);
drop(w);
let r = cache.read(); // another writer could have run in the gap
```

```rust
// NEW (1.92): atomic write -> read, no gap
let mut w = cache.write();
w.insert(key, value);
let r = RwLockWriteGuard::downgrade(w); // keep reading, no re-contend
```

Caveat for Pixhaus: this is `std::sync::RwLockWriteGuard`. The workspace's lock crate is `parking_lot`, whose guards already have `downgrade` and are `!Send` (so they cannot cross an `.await`). Use whichever the lock at hand is from, and check the `pixhaus-parking-lot` skill — the std stabilization is relevant mainly where a dependency hands you a std `RwLock`.

## 15. `cfg(true)` / `cfg(false)` and `cfg_select!` (cfg literals 1.88, `cfg_select!` 1.95)

Small ergonomic wins for the dev toggles this codebase already has. `#[cfg(false)]` is a clearer "compiled out" marker than the `#[cfg(any())]` trick. `cfg_select!` (1.95) picks one token tree among several cfg conditions — cleaner than a stack of `#[cfg]`/`#[cfg(not(...))]` pairs when, say, selecting a GPU-capability path or a platform default-directory branch.

```rust
// OLD: the any() trick for "never compile this"
#[cfg(any())]
fn scratch() {}
```

```rust
// NEW (1.88): say what you mean
#[cfg(false)]
fn scratch() {}
```

```rust
// NEW (1.95): cfg_select! instead of paired cfg/cfg(not)
let backend = cfg_select! {
    target_os = "macos" => MetalHint,
    target_os = "windows" => DxHint,
    _ => VulkanHint,
};
```

Do not over-rotate on these — they are convenience, not capability. Use them when you are already editing cfg-gated code, not as a reason to go rewrite working gates.

## What this list deliberately leaves out

The window also stabilized large AVX-512 / NEON / RISC-V / s390x intrinsic and target-feature batches (1.89, 1.94), C-style variadics (1.91, 1.93), naked functions (1.88), `asm_goto` (1.87), and pointer-provenance APIs (1.89, 1.91). None of these belong in a no-`unsafe`, raster-editor codebase — SIMD here goes through `wgpu` shaders and the optimizer, FFI is minimal, and inline assembly is forbidden by the unsafe ban. If a perf hot path ever needs explicit SIMD, that is a deliberate dependency-and-`unsafe` discussion to surface, not a feature to reach for off this list.

Two adoption rules to close on. First, the 1.96 pin makes every item on this list live today; `rust-toolchain.toml` stays the source of truth, so it is the thing to check before reaching for anything stabilized past 1.96. Second, none of this is a reason to churn working code — adopt a feature at the next *edit* of a spot it improves, where it deletes a real `.unwrap()`, a nesting level, or a widen-narrow dance, and leave the rest alone.
