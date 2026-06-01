# Representing a nested layer hierarchy in the core crate

## Recommendation

Model the tree as a recursive `enum` where the recursive arm is held by an
already-heap-backed `Vec`. A layer is either a leaf or a group, and a group owns
its children outright. You do **not** need `Rc`, `Arc`, or any interior-mutability
cell for this. One owner, plain ownership, `&`/`&mut` to read and write.

```rust
/// A node in the layer tree: either a single drawable layer or a group that
/// owns nested children.
pub enum LayerNode {
    Leaf(Layer),
    Group(Group),
}

pub struct Group {
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    /// Children, top-to-bottom in z-order. Groups nest arbitrarily deep here.
    pub children: Vec<LayerNode>,
}

pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub pixels: PixelBuffer, // Vec<u8> + stride, per the memory convention
}
```

The whole document owns one `Vec<LayerNode>` (or a single root `Group`), and the
egui loop mutates it through `&mut self`. No pointer past `Vec`.

## Why this is the right shape

A type can't contain itself by value — the size would be infinite — so a recursive
structure needs a pointer somewhere to break the cycle and give the recursive arm
a known size. The key point the pointers skill makes: **`Vec<T>`, `String`, and the
like are already heap-allocated and owned, so they already break the recursion. You
don't add a `Box` on top.** A `Vec<LayerNode>` stores its elements behind a heap
pointer; the `Group` struct itself is a fixed size (pointer + len + cap), so
`LayerNode` is sized and compiles.

That's exactly the case the skill's recursive-types example calls out:

```rust
enum LayerNode {
    Leaf(LayerId),
    Group(Vec<LayerNode>),          // Vec is already heap-backed — no Box needed here
    Masked(Box<LayerNode>, MaskId), // a single recursive child does need the Box
}
```

A group's children are a *collection* (the `Vec` arm), so no `Box`. You'd only
reach for `Box<LayerNode>` if you had a node that owned exactly **one** recursive
child by value with no surrounding `Vec` — for instance a `Masked(Box<LayerNode>,
MaskId)` wrapper. A single recursive field has no heap-backed container to hide
behind, so that one needs the `Box`.

Ownership is clean: a group owns its children, the document owns the roots, and the
borrow checker proves soundness at compile time. Reading a subtree is `&LayerNode`;
editing one is `&mut LayerNode`. This is the Pixhaus default — the cheapest thing
that works — and it costs nothing at runtime beyond the heap the `Vec` already uses.

## Alternatives, and why I'd reject most of them

- **`Rc<RefCell<LayerNode>>` / `Arc<Mutex<LayerNode>>` for the children.** This is
  the reflex to resist. "A tree of shared nodes" sounds like it needs shared
  ownership, but a layer tree has a *single* owner: the document. No two parents
  share a child, and nothing outside the UI thread mutates the tree (background work
  gets a *copy of the slice it needs* and returns results over a channel, per the
  document-ownership rule). `Rc` is `!Send`/`!Sync` and almost never appears in
  Pixhaus; `RefCell` trades a compile error for a runtime panic in front of a user,
  which the no-`unwrap`/no-`panic` rule exists to keep out. `Arc<Mutex>` here is the
  classic over-share: a lock around data that has one owner, plus a second copy of
  the tree that would desync the undo stack. Reject all of these.

- **An arena: `Vec<LayerNode>` of flat nodes addressed by index/`LayerId`, with
  groups storing `Vec<LayerId>` instead of owning children directly.** This is the
  legitimate alternative, and the one I'd actually consider if requirements grow.
  The skill points to indices-into-a-`Vec` as the way to model graphs and
  self-referential structures without back-pointers or `unsafe`. It pays off when
  you need cheap reparenting/reordering, stable handles that survive tree edits
  (selection, the undo stack, references from the UI), or many cross-references into
  the tree. The cost is indirection: every traversal goes through the arena, and you
  hand-manage node removal. For a plain "groups own their children, nested deep"
  requirement, the owned recursive enum is simpler and I'd start there; promote to an
  arena (`LayerId` newtype + `Vec` slab) the moment stable ids or frequent reparenting
  show up. Note these aren't far apart — the enum already wants a `LayerId` newtype on
  leaves, so moving to an arena later is a contained refactor.

- **`Box<LayerNode>` on the children.** Wrong for the `Vec` arm — wrapping each
  element in a `Box` adds a pointer indirection per child for no reason, since the
  `Vec` already owns its elements on the heap. `Box` earns its place only for a
  single recursive field (the `Masked` case above) or to shrink a large rarely-used
  enum variant measured in hand.

- **A giant flat enum with no recursion** (e.g. encoding depth some other way).
  Fights the natural shape, makes traversal awkward, and buys nothing. The recursive
  enum *is* the data.

## Bottom line

Recursive `enum` with the children behind a `Vec`. No `Box` on the group's
children (the `Vec` already breaks the recursion), no `Rc`/`Arc`/`RefCell` (the
document is the single owner, mutated through `&mut self`). Add a `LayerId` newtype
on leaves now so that if stable handles or heavy reparenting arrive, switching to an
index-based arena is a contained change rather than a rewrite.
