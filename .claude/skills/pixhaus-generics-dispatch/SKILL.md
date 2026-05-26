---
name: pixhaus-generics-dispatch
description: Use when deciding HOW a Pixhaus type should be polymorphic — whether a function, struct, or collection should take a generic (`<T: Trait>` / `impl Trait`, static dispatch) or a trait object (`Box<dyn Trait>` / `Arc<dyn Trait>` / `&dyn Trait`, dynamic dispatch). Trigger this for ANY "should this be generic or a trait object", "make this work with any X", "store different X in one collection / registry", "plugin architecture", "the AI Backend registry", "blend mode / verb / render backend trait", "Box<dyn Trait> vs impl Trait", "monomorphization / code bloat / compile time", "vtable cost in the pixel loop", or "the trait isn't object safe / dyn compatible (E0038)" task, even when the user never says "generics" or "dispatch". The rule is "static where you can, dynamic where you must": prefer monomorphized generics so the 8K per-pixel hot path stays inlined with zero indirection, and reach for `dyn Trait` only at the genuine runtime-polymorphism boundaries — the multi-backend AI runtime, mlua/extism plugins, heterogeneous collections. Load this BEFORE choosing between a generic and a trait object so you don't reach for `Box<dyn>` by reflex (an agent tell) or pay a vtable call per pixel. Not for these adjacent calls: choosing `Rc` vs `Arc` vs `Box` for sharing or owning a SINGLE concrete type (that's `pixhaus-rust-pointers`); making an `async fn` in a trait `Send` or dyn-compatible (that's `pixhaus-async-trait`); or adding lifetime parameters to satisfy the borrow checker (that's borrowing, `pixhaus-rust-conventions`) — even though all three mention generics or pointers, the dispatch decision is not what's being asked.
---

# Generics and dispatch in Pixhaus

> Static where you can, dynamic where you must.

Rust gives you two ways to write polymorphic code, and the choice is a real
engineering decision in this repo, not a style preference:

- **Generics / static dispatch** (`<T: Trait>`, `impl Trait`) — resolved at
  compile time, monomorphized into one specialized copy per concrete type.
  Zero runtime overhead, inlines through the call. This is the default.
- **Trait objects / dynamic dispatch** (`dyn Trait` behind a pointer) — one
  shared copy of the code, the concrete type erased, calls routed through a
  vtable at runtime. Flexible, but every call is an indirect jump the compiler
  can't inline across.

`pixhaus-rust-conventions` states the headline rule ("no `Box<dyn Trait>` for
monomorphic call sites"). This skill is the full reasoning behind it and the
map of where each tool actually belongs in Pixhaus.

## The decision in one table

|                       | Static (`impl Trait` / `<T: Trait>`) | Dynamic (`dyn Trait`)              |
|-----------------------|--------------------------------------|------------------------------------|
| Per-call cost         | None — inlined                       | Vtable indirection, no inlining    |
| Compile time          | Slower — one copy per type           | Faster — code shared               |
| Binary size           | Larger — per-type codegen            | Smaller                            |
| Mix types in one `Vec`| No — one concrete type per instance  | Yes — that's the whole point       |
| Works in a `dyn` API  | N/A                                  | Only if the trait is dyn-compatible|
| Error messages        | Clearer, point at the real type      | Erased type can muddy the error    |

Read it top-down: static wins everything *except* runtime heterogeneity and
binary size. So you stay static until the design genuinely needs to hold
different concrete types behind one interface at runtime — then you switch,
deliberately, at that boundary and nowhere deeper.

## Why static is the default here: the 8K hot path

Pixhaus has to scale drawing and playback to 8192×8192 (see the 8K perf
constraint). A trait object call is an indirect jump through a vtable that the
optimizer cannot inline or vectorize across. Put one in a per-pixel loop and at
8K you've bought ~67 million un-inlinable indirect calls per operation. That is
exactly the kind of overhead the native rewrite exists to delete.

```rust
// BAD — a vtable hop per pixel, inlining and SIMD blocked
fn composite(dst: &mut [u8], src: &[u8], mode: &dyn BlendMode) {
    for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        let out = mode.blend(Rgba::from(s), Rgba::from(&*d)); // indirect call
        d.copy_from_slice(&out.to_bytes());
    }
}

// GOOD — monomorphized: the blend inlines into the loop body, vectorizes
fn composite<M: BlendMode>(dst: &mut [u8], src: &[u8], mode: &M) {
    for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        let out = mode.blend(Rgba::from(s), Rgba::from(&*d)); // inlined
        d.copy_from_slice(&out.to_bytes());
    }
}
```

The dynamic version is fine to *select* a blend mode (`match` on an enum, look
it up once). It is not fine to *call* per pixel. Resolve the `dyn` once at the
top of the operation, hand the concrete type to a generic inner loop.

## Static dispatch: `impl Trait` vs `<T: Trait>`

Both monomorphize identically. Pick on readability:

```rust
// impl Trait in argument position — the idiomatic default for a single bound
fn export_each(layers: &[Layer], sink: impl FrameSink) { ... }

// Named generic — use when you must name the type (return it, bound an
// associated type, repeat it across several arguments)
fn transform<T: PixelOp>(op: &T, buf: &mut PixelBuffer) -> T::Output { ... }
```

Rules of thumb:

- One bound, used once, don't need the name → `impl Trait`.
- Need to refer to the type again (return position, `T::Output`, `Vec<T>`) →
  named `<T: Trait>`.
- `impl Trait` in **return** position hides the concrete type from the caller
  while staying static — great for returning iterators and closures:
  `fn pixels(&self) -> impl Iterator<Item = Rgba> + '_`.

### Watch the monomorphization cost

Static dispatch isn't free — it's paid at compile time and in binary size. A
generic function instantiated over 12 concrete types is 12 copies of its
machine code. Usually irrelevant. It bites when a *large* function is generic
over a *small, hot* part. The fix is the "outer thin / inner fat" split: keep
the generic surface tiny and funnel into one non-generic function that does the
bulk work.

```rust
// The generic shell is tiny; the real work isn't duplicated per type.
pub fn save(path: &Path, fmt: impl Into<Format>) -> Result<()> {
    save_inner(path, fmt.into()) // one monomorphization point
}
fn save_inner(path: &Path, fmt: Format) -> Result<()> { /* the actual work */ }
```

## Dynamic dispatch: where it earns its place in Pixhaus

Reach for `dyn Trait` only when the type genuinely isn't known until runtime or
must vary within one collection. In this repo that's a short, specific list:

- **The multi-backend AI runtime.** Anthropic, OpenAI, Replicate, Ollama,
  ComfyUI, Stability all implement one `Backend` trait, the user picks one at
  runtime, and they live together in a registry. This is the textbook case:
  `Arc<dyn Backend + Send + Sync>` shared across the UI thread and tokio tasks,
  a `Vec<Box<dyn Backend>>` or map for the registry. (Async methods on this
  trait pull in `pixhaus-async-trait` — read it before adding one.)
- **Plugins.** `mlua` scripts and `extism` WASM plugins are loaded at runtime;
  their concrete types literally don't exist at compile time. Trait objects (or
  the plugin host's own handles) are the only option.
- **Heterogeneous collections.** A list that must hold genuinely different
  concrete types at once — a stack of effect layers of differing kinds, a queue
  of mixed verbs — needs `Vec<Box<dyn Verb>>`. One concrete type per element
  would be a generic, and a generic can't do "different per element".

If your case isn't one of these, you almost certainly want a generic or an
`enum`. A closed, known set of variants (the built-in blend modes, known export
formats) is an `enum` + `match`, not `dyn` — it's faster, exhaustively checked,
and needs no heap.

### Pointer choice: `&dyn` < `Box<dyn>` < `Arc<dyn>`

Pick the weakest one that works — ownership and sharing aren't free:

- `&dyn Trait` — you only borrow it for the call. Default for trait-object
  *parameters*. No allocation, no ownership.
- `Box<dyn Trait>` — you need to own one on the heap (store it in a struct,
  return it, put it in a `Vec`). Single owner.
- `Arc<dyn Trait + Send + Sync>` — shared across threads (UI thread + tokio
  task). The AI backend registry is the canonical case. Add `+ Send + Sync` so
  it can cross the spawn boundary.

```rust
fn describe(b: &dyn Backend) -> String { ... }          // just calling it
struct Registry { backends: Vec<Box<dyn Backend>> }     // owns them
let active: Arc<dyn Backend + Send + Sync> = ...;        // shared with tasks
```

### Don't box too early

The most common agent mistake is boxing inside a struct that has exactly one
backend type. That's a generic, not a trait object:

```rust
// BAD — premature boxing; this struct holds one concrete backend
struct Renderer { backend: Box<dyn RenderBackend> }

// GOOD — generic over the one backend it actually has
struct Renderer<B: RenderBackend> { backend: B }
```

Box at a real boundary (a registry of mixed types, a public API that must hide
the type), not by reflex. If a struct is generic over one backend for its whole
lifetime, keep it generic.

## Dyn compatibility (formerly "object safety")

You can only make `dyn Trait` from a trait that is **dyn-compatible** (the
current name for what older docs and Rust 1.79−1.82 errors call "object safe";
the rules are the same). A trait is dyn-compatible only if, roughly:

- no method is generic over type parameters (`fn make<T>()` can't be in a
  vtable — there's no single machine-code copy to point at),
- no method returns `Self` by value (the caller wouldn't know the size),
- methods take `&self`, `&mut self`, or `self`, and
- methods you want callable via `dyn` aren't gated on `where Self: Sized`.

```rust
// dyn-compatible — every method is a plain &self call
trait Verb { fn apply(&self, doc: &mut Document) -> Result<()>; }

// NOT dyn-compatible — generic method and a Self-returning method
trait Factory {
    fn create<T>(&self) -> T;   // generic method: no single vtable entry
    fn clone_box(&self) -> Self; // returns Self by value: unknown size
}
```

The escape hatch for a `-> Self` method is `fn clone_box(&self) -> Box<dyn
Trait>`. The escape hatch for a generic method is often to take `&dyn
OtherTrait` instead of `<T: OtherTrait>`. Native `async fn` in traits is not
dyn-compatible without boxing — that's the whole reason `pixhaus-async-trait`
exists; go there for the `Backend` trait.

If you hit `E0038 (... cannot be made into an object)`, the compiler is telling
you the trait can't be a trait object as written. Either fix the offending
method or step back and ask whether you wanted a generic in the first place.

## Decision flow

1. **Do different concrete types need to coexist at runtime** — in one `Vec`, a
   registry, a runtime-loaded plugin? If no → generic or `enum`, stop here.
2. **Is the set of types closed and known at compile time** (blend modes, export
   formats)? → `enum` + `match`. Faster than `dyn`, exhaustive, no heap.
3. **Otherwise you need `dyn`.** Resolve it once at the boundary, then hand the
   concrete value to a generic inner function so the hot path stays static.
4. **Pick the weakest pointer** that works: `&dyn` to call, `Box<dyn>` to own,
   `Arc<dyn + Send + Sync>` to share across threads.
5. **Check dyn compatibility.** Generic methods or `-> Self` mean you redesign
   the trait or accept it can only ever be a generic.

When you're truly unsure, start with a generic and a trait bound. It's cheap to
add `Box<dyn>` at a boundary later; it's expensive to claw back a vtable call
you scattered through a pixel loop.

## See also

- `pixhaus-rust-conventions` — the headline rule and the broader `Box<dyn>` /
  `Arc<Mutex>` / `Vec<Vec<T>>` anti-patterns.
- `pixhaus-async-trait` — async methods on a `dyn` trait (the AI `Backend`).
- `pixhaus-wgpu` — the render backend, the other place a backend trait appears.
