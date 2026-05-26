# Representing a recursive layer hierarchy in Rust

## Short answer

Use an enum with two variants — a leaf and a group — and have the group own a
`Vec` of the same enum. Recursion through an owned `Vec` is the idiomatic,
zero-`unsafe`, single-owner way to model a tree in Rust. Don't reach for `Rc`,
`RefCell`, or raw pointers; you don't need shared ownership or back-pointers for
a layer tree that one document owns.

```rust
/// A node in the layer tree. Either a single paintable layer or a group that
/// nests more nodes.
pub enum LayerNode {
    Leaf(Layer),
    Group(Group),
}

pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub blend: BlendMode,
    pub pixels: PixelBuffer, // Vec<u8> + stride, owned here
}

pub struct Group {
    pub id: LayerId,
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub blend: BlendMode,
    /// Children render bottom-to-top (index 0 = bottom). The Vec owns them,
    /// so the whole subtree drops when the group drops.
    pub children: Vec<LayerNode>,
}
```

`Vec<LayerNode>` is the recursive edge. Because `Vec<T>` stores its elements
behind a heap pointer, the type has a known, finite size even though it refers
to itself — so you do **not** need a `Box` here. (You'd only need `Box` if a
variant held a single `LayerNode` inline; `Vec`, `Box`, and the like already
provide the indirection that breaks the infinite-size cycle.)

## Why this shape

- **Single owner, matches the locked memory rule.** The document owns the root,
  each group owns its children outright via `Vec`. No `Arc<Mutex<>>`, no
  reference counting, no interior mutability. Dropping the document drops the
  whole tree. This is exactly the "every piece of mutable state has a single
  owner" guideline.
- **The enum makes "group vs leaf" a type-level fact.** A leaf cannot
  accidentally grow children; a group cannot accidentally hold pixels it never
  composites. The compiler forces every traversal to handle both cases via
  `match`, so adding a third node kind later (say, an adjustment layer) is a
  compile-error-driven refactor, not a runtime surprise.
- **Trivially serializable.** `rmp-serde` + `serde` derive `Serialize` /
  `Deserialize` on this without any custom code — recursive enums and `Vec` are
  handled out of the box. That matters for the `.pixhaus` format.
- **Traversal is plain recursion.** No pointer chasing, no lifetime gymnastics:

  ```rust
  impl LayerNode {
      pub fn visit(&self, depth: usize, f: &mut impl FnMut(&LayerNode, usize)) {
          f(self, depth);
          if let LayerNode::Group(g) = self {
              for child in &g.children {
                  child.visit(depth + 1, f);
              }
          }
      }
  }
  ```

  Mutable traversal is the same with `&mut self` and `&mut g.children`. The
  borrow checker is happy because at any moment you hold exactly one mutable
  path into the tree.

## The one real pitfall: addressing nodes

The recursive-owned tree has a known weakness — you can't cheaply hold a
"pointer to a layer" while still being able to mutate the tree, because a `&mut
LayerNode` borrows the whole tree for its lifetime. The fix is **not** to switch
to `Rc<RefCell<>>`. Instead, address nodes by a stable identity and resolve to a
borrow only at the moment you need it.

Two good options, pick based on your needs:

1. **Stable `LayerId` (a newtype over a `u64` or a generational id).** The UI,
   undo stack, and selection all store `LayerId`, never a borrow. To act on a
   node you walk the tree to find it. Cheap to copy, survives reorders, and
   serializes cleanly. This is the right default for an editor — undo/redo and
   the panel both need an identity that outlives any single borrow.

   ```rust
   #[derive(Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
   pub struct LayerId(pub u64);

   impl Group {
       pub fn find_mut(&mut self, target: LayerId) -> Option<&mut LayerNode> {
           for child in &mut self.children {
               match child {
                   LayerNode::Leaf(l) if l.id == target => return Some(child),
                   LayerNode::Group(g) if g.id == target => return Some(child),
                   LayerNode::Group(g) => {
                       if let Some(found) = g.find_mut(target) {
                           return Some(found);
                       }
                   }
                   LayerNode::Leaf(_) => {}
               }
           }
           None
       }
   }
   ```

2. **Path as `Vec<usize>` (index per depth).** Useful for "move this node here"
   operations where you need positional context. Cheaper to follow than an id
   search, but indices shift on insert/remove, so it's a transient handle, not
   something to store across edits.

For a pixel-art editor with undo/redo, go with the `LayerId` approach as the
canonical handle and keep the tree itself owned-and-recursive.

## Alternatives I considered and rejected

- **`Rc<RefCell<LayerNode>>` (or `Arc<Mutex<>>`) for every node.** This is the
  reflex port from a GC language, and it's wrong here. You'd trade compile-time
  borrow checking for runtime `borrow()`/`lock()` panics, lose `Send`/`Sync`
  clarity, make serialization awkward (you have to dedup or break cycles), and
  invite reference cycles that leak. You only need this if multiple owners
  genuinely mutate the same node concurrently — a single-user, file-based editor
  with a single-threaded egui update loop does not.

- **A flat arena / slotmap with id-based child lists** (e.g. every node in one
  `Vec<LayerNode>` or a `SlotMap`, children stored as `Vec<LayerId>`). This is a
  legitimate and arguably *better* choice at scale — it sidesteps the
  borrow-the-whole-tree problem entirely, makes ids stable for free, and is
  cache-friendly. The cost is one layer of indirection on every access and a bit
  more ceremony. I'd reach for this if traversal/mutation patterns get hot or if
  you find yourself fighting the borrow checker constantly. For a layer tree
  (tens to low hundreds of nodes, not a perf-critical inner loop), the plain
  owned enum is simpler and clearer; reserve the arena for the pixel data, which
  is where the real bytes and performance live. Mentioning it because if the tree
  ever needs back-pointers (child knows its parent), the arena is the clean way
  to get them without `Weak`.

- **Trait objects: `Vec<Box<dyn LayerNode>>` with a `Layer` / `Group` trait.**
  Tempting if you imagine many node kinds with shared behavior, but it buys
  nothing over the enum here and costs you: you can't easily match on the
  concrete kind, downcasting needs `Any`, and `serde` can't derive through
  `dyn`. The set of node kinds is closed and known (leaf, group, maybe
  adjustment) — that's the textbook case *for* an enum, *against* trait objects.
  Prefer the enum until the variant set is genuinely open and
  externally-extensible.

- **`Box<LayerNode>` inside the variant.** Unnecessary because the recursive edge
  goes through `Vec`, which is already a heap pointer. Adding `Box` would just be
  a wasted allocation per node.

## Recommendation

Recursive owned enum (`LayerNode { Leaf, Group }`, group owns `Vec<LayerNode>`),
addressed externally by a `Copy` `LayerId` newtype. It's the least machinery, it
matches the single-owner memory rule, it serializes for free, and it keeps the
borrow checker on your side. If profiling or constant borrow-checker friction
later says otherwise, migrate the tree to a slotmap arena keyed by `LayerId` —
the id-based public API means that swap won't ripple through callers.
