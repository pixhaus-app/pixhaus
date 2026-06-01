# Type-state for editor tools: don't, and drop the unsafe idea entirely

Short answer: type-state is the wrong tool here, and the `unsafe` part is a
red flag that's actually telling you so. Use a plain `enum` with a `match`.
Here's the reasoning, because the "why" matters more than the verdict.

## Why type-state doesn't fit tool selection

Type-state earns its verbosity only when **all** of these hold:

- The states are known at compile time and form a small, fixed set.
- Some operations are illegal in some states, and calling them in the wrong
  state is a real bug with a costly failure mode.
- The transitions are driven by **code you control**, not by runtime input
  you can't predict.

Tool selection fails the one that matters most: **the transition is driven by
runtime input.** The user clicks the toolbar. You don't know at compile time
whether the active tool is a Pencil, an Eraser, or a Bucket — that's decided
by a mouse event at runtime. Type-state can't represent "a value whose type I
won't know until the user clicks," because a type has to be fixed when the
code compiles. That's the textbook runtime-state boundary, and it's exactly
the case where type-state stops being applicable.

It also fails the "different operations per state" test more than it looks
like it does. Pencil, Eraser, and Bucket aren't a lifecycle where you call
methods in a forbidden order. They're three peers that all answer the same
question: "the user dragged here — paint." They share an interface
(`on_press`, `on_drag`, `on_release`), they don't transition into one another,
and there's no illegal call order to forbid. Type-state forbids method
*sequencing*; you don't have a sequencing problem, you have a *dispatch*
problem.

And the set churns. You'll add Line, Fill-Selection, Eyedropper, Move,
Rectangle-Select. Every new tool in a type-state design multiplies `impl`
blocks and forces a new marker type. A `match` arm is cheap; a type parameter
that has to thread through your whole app state is not.

So this is the case the skill flags directly: "Five states with mostly-shared
methods is a `match` wearing a costume." You have three-plus peers with a
fully shared interface. That's an enum, full stop.

## The `unsafe` idea: stop — it's the signal, not the solution

You said you'd "use unsafe to return the right tool type from the click
handler." That instinct — wanting `unsafe` to return a different *type* from
each branch of a runtime decision — is the precise tell that you've hit a
runtime-state boundary. A function has one return type. Branches that want to
return `Tool<Pencil>` from one arm and `Tool<Eraser>` from another can't, and
no `transmute` makes that sound: you'd be lying to the compiler about a type,
and the first method call reads the wrong layout. It's undefined behavior, not
a clever workaround.

It's also flatly against repo rules — `unsafe` is forbidden workspace-wide,
`cargo deny` / clippy enforce it, and there is no exception worth requesting
here because the safe version is *less* code. When you want `unsafe` to return
different types from one branch, the answer is an enum (or `Box<dyn Trait>`),
never a transmute.

## What to do instead

Two clean options. Pick by how the tools share code.

### Option A — enum + match (start here)

Best when the tool is mostly data plus a `match` on behavior. It's the
simplest thing that's correct, it keeps every tool's state in one owned value
(no `Arc<Mutex<>>`, no boxing), and adding a tool is one variant plus arms the
compiler forces you to fill in.

```rust
pub enum Tool {
    Pencil(Pencil),
    Eraser(Eraser),
    Bucket(Bucket),
}

pub struct Pencil { pub size: u32, pub color: [u8; 4] }
pub struct Eraser { pub size: u32 }
pub struct Bucket { pub tolerance: u8 }

pub struct Stroke<'a> { /* dirty-region accumulator, canvas view, etc. */ _p: &'a () }

impl Tool {
    // Toolbar click just reassigns the active tool. No unsafe, no type juggling.
    pub fn select_pencil(&mut self) { *self = Tool::Pencil(Pencil { size: 1, color: [0, 0, 0, 255] }); }
    pub fn select_eraser(&mut self) { *self = Tool::Eraser(Eraser { size: 1 }); }
    pub fn select_bucket(&mut self) { *self = Tool::Bucket(Bucket { tolerance: 0 }); }

    pub fn on_press(&mut self, stroke: &mut Stroke<'_>, x: i32, y: i32) {
        match self {
            Tool::Pencil(p) => { /* stamp p.color at (x, y) */ }
            Tool::Eraser(e) => { /* clear a brush of e.size */ }
            Tool::Bucket(b) => { /* flood fill within b.tolerance */ }
        }
    }
}
```

The toolbar handler becomes `self.tool.select_eraser()` — one line, no `unsafe`,
no return-type gymnastics. The operations valid for each tool live behind the
`match`, and the compiler still makes you handle every variant, so adding
`Tool::Line` won't silently skip a code path.

If you want each tool's logic in its own module rather than fattening one
`match`, give each its own struct with inherent methods and have the `match`
arms delegate (`Tool::Pencil(p) => p.on_press(stroke, x, y)`). Same enum, tidier
files.

### Option B — `Box<dyn ToolBehavior>` (if the tool set is open or plugin-fed)

If tools will eventually come from Lua/`extism` plugins, or the set is genuinely
open-ended, a trait object gives a uniform interface without a central enum you
have to edit for every tool:

```rust
pub trait ToolBehavior {
    fn on_press(&mut self, stroke: &mut Stroke<'_>, x: i32, y: i32);
    fn on_drag(&mut self, stroke: &mut Stroke<'_>, x: i32, y: i32);
    fn on_release(&mut self, stroke: &mut Stroke<'_>);
}

// App state holds the active tool uniformly:
struct Editor { tool: Box<dyn ToolBehavior> }

// Toolbar click swaps the boxed tool. Still no unsafe.
fn on_toolbar_click(editor: &mut Editor, picked: ToolKind) {
    editor.tool = match picked {
        ToolKind::Pencil => Box::new(Pencil { size: 1, color: [0, 0, 0, 255] }),
        ToolKind::Eraser => Box::new(Eraser { size: 1 }),
        ToolKind::Bucket => Box::new(Bucket { tolerance: 0 }),
    };
}
```

Per repo conventions, prefer the enum (Option A) until you actually have the
open-set / plugin requirement — "don't reach for `Box<dyn Trait>` when generics
fit," and here even simpler, when an enum fits. The trait is the right call the
day tools become plugin-supplied; not before.

## Where type-state *would* pay off near this code

So you know the tool, not just dismiss it: type-state fits a single stroke's
lifecycle if you ever need to forbid an illegal call order on it — e.g. a
`Stroke<Idle>` that only gains `.drag()` and `.commit()` after `.begin()`, so
"commit a stroke that never started" can't compile. That's a real
code-driven sequence with a forbidden order. Tool *selection* isn't; tool
*stroke lifecycle* might be. Reach for it there if the bug ever bites, not for
the toolbar.

## Bottom line

- Don't use type-state for tool switching — the active tool is chosen at
  runtime, the tools are peers sharing one interface, and the set will grow.
  All three point away from type-state.
- Drop the `unsafe` plan completely. Wanting `unsafe` to return different
  types from a runtime branch is the symptom that you're at a runtime-state
  boundary; the cure is an enum, and `unsafe` is forbidden here anyway.
- Use `enum Tool { Pencil, Eraser, Bucket }` + `match`. Move to
  `Box<dyn ToolBehavior>` only when tools become open-ended or plugin-supplied.
