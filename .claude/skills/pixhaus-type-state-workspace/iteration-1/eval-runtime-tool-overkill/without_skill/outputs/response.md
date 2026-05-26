# Type-state for editor tools: skip it, and definitely skip the unsafe

Short answer: type-state is the wrong tool here, and the `unsafe` plan can't
work — not because the repo bans `unsafe` (it does), but because it wouldn't
give you what you want even if it were allowed.

## Why type-state doesn't fit

Type-state encodes a state machine in the type system so the *compiler* rejects
invalid operations at compile time. It shines when the state transitions are
known at the point you write the code — `Builder<NoUrl>` → `Builder<HasUrl>`,
or a `File<Open>` you can't `read()` until you've called `open()`. The win is
that calling a wrong-state method is a compile error.

Your tool switch is the opposite situation. Which tool is active is decided at
**runtime** by a user clicking the toolbar. The compiler has no idea at build
time whether the active tool is a `Pencil` or a `Bucket` — that's data, not a
type. When the deciding value only exists at runtime, no amount of type-state
machinery can move the check to compile time. You'd just end up storing the
tool in an enum or a trait object anyway, and writing a `match` (or dynamic
dispatch) to recover the concrete type — which is exactly the runtime dispatch
type-state was supposed to eliminate.

So the pattern buys you nothing for the thing you described. The "each tool only
exposes its valid operations" goal is real and worth keeping — but the active
tool is a runtime choice, so the natural Rust expression of "one of N tool
kinds, each with its own behavior" is an enum or a trait, not a type parameter.

## Why the `unsafe` idea is a dead end

The plan — "use `unsafe` to return the right tool type from the click handler" —
implies a function whose return type depends on a runtime value. Rust functions
have one fixed return type. `unsafe` doesn't change that; it only unlocks a
handful of specific superpowers (deref raw pointers, call `unsafe` fns, etc.).
None of them let a function return `Pencil` on one branch and `Bucket` on
another. To do that you *still* need an enum or a `Box<dyn Tool>` — and at that
point the `unsafe` is doing nothing except inviting UB if you ever transmute to
"reinterpret" one tool as another.

And in this repo it's a non-starter regardless: `unsafe` is forbidden
workspace-wide (clippy-enforced). There's no version of this design that needs
it.

## What to do instead

Two clean options. Pick by how the tools differ.

### Option A — enum + match (start here)

If tools share a small, fixed set of operations (begin a stroke, continue,
commit) and differ only in *how* they fill pixels, an enum is the simplest thing
that works and keeps all tools in one place:

```rust
pub enum Tool {
    Pencil(Pencil),
    Eraser(Eraser),
    Bucket(Bucket),
}

impl Tool {
    /// Called on pointer-down at a pixel. Each variant does its own thing.
    pub fn on_press(&mut self, ctx: &mut ToolCtx, at: PixelPos) {
        match self {
            Tool::Pencil(p) => p.stamp(ctx, at),
            Tool::Eraser(e) => e.stamp(ctx, at),
            Tool::Bucket(b) => b.flood(ctx, at),
        }
    }
}
```

The toolbar handler just assigns: `self.active = Tool::Bucket(Bucket::new());`.
No unsafe, no generics, exhaustive `match` so adding a fourth tool is a compile
error until you handle it everywhere. This is the right default for three tools.

### Option B — `trait Tool` + `Box<dyn Tool>` (if behaviors diverge a lot)

If tools grow their own config UI, cursors, option panels, and you want them
pluggable without touching a central enum, a trait is cleaner:

```rust
pub trait Tool {
    fn on_press(&mut self, ctx: &mut ToolCtx, at: PixelPos);
    fn on_drag(&mut self, ctx: &mut ToolCtx, at: PixelPos);
    fn name(&self) -> &'static str;
}

struct Pencil { /* ... */ }
struct Bucket { /* ... */ }
// impl Tool for each

// App state holds the active tool behind a trait object:
active: Box<dyn Tool>,

// Toolbar click swaps it:
self.active = Box::new(Bucket::new());
```

Per the repo's dispatch guidance, don't reach for `Box<dyn Trait>` prematurely —
with only three known tools, Option A is leaner and gives exhaustiveness
checking. Move to Option B when the per-tool behavior and state genuinely
diverge enough that the enum's `match` arms become unwieldy.

## On "only expose valid operations"

That instinct is good, just aim it at the right level. The way you guarantee
a `Bucket` can't be asked to do a pencil-only thing is: each tool is its own
struct with only its own methods, and the active-tool layer talks to them
through the *shared* `Tool` interface (the enum's methods or the trait). A
`Pencil` simply has no `flood_fill` method to call. You get the "invalid
operations don't exist" property from ordinary structs and a narrow shared
interface — no phantom type parameters required.

## Bottom line

- Don't use type-state — the active tool is a runtime decision, so it can't
  buy compile-time safety here.
- Drop the `unsafe` entirely — it can't return a runtime-chosen type, and it's
  banned in this workspace anyway.
- Use an enum + `match` now (Option A). Promote to a `Tool` trait +
  `Box<dyn Tool>` only if/when per-tool behavior diverges enough to justify it.
