# Modern Rust: iterator, slice, array, Vec, and collection APIs (1.85-1.96)

Newly stabilized methods on Vec, slices, arrays, iterators, and the map/set/deque/list collections — extract_if, get_disjoint_mut, as_chunks, pop_if, push_mut and the rest — grouped by type. Part of the `pixhaus-rust-modern` skill; start at its `SKILL.md` for the shortlist and the per-version cheat sheet.

The toolchain pins 1.96, so every method below is callable today. This section is the reference of record for the sequence-and-collection APIs that landed across the 1.85-1.96 window. The rule: reach for the stable method instead of hand-rolling the loop, because the std version is the one the borrow checker, the optimizer, and the next reader already understand. But do not rewrite working code just to spend a new API — adopt these at the next touch, not in a churn pass.

Grouped by the type the method hangs off. `const` is called out per item, because a `const fn` is the difference between computing a value at startup and baking it into the binary — relevant for the stride tables, palette sizes, and layout math we set up once.

## Vec

`Vec::pop_if` (1.86) pops the last element only when a predicate says so, replacing the last-then-check-then-maybe-pop dance. Use it where the document model keeps a stack — a redo stack you trim, or a coalescing command buffer.

```rust
// OLD: peek, test, then conditionally pop — two indexing steps and a branch.
if let Some(last) = redo_stack.last() {
    if last.is_stale() {
        redo_stack.pop();
    }
}

// NEW (1.86): one call, predicate gets the element by &mut.
redo_stack.pop_if(|cmd| cmd.is_stale());
```

`Vec::extract_if` (1.87) is the in-place filter-and-collect: it yields removed elements through a draining iterator and leaves the survivors behind, in one pass. Reach for it when reaping entries from a layer list or a cel set by a condition — `retain` throws the removed items away, `extract_if` hands them to you (so you can, say, push them onto the undo record).

```rust
// OLD: build a keep-list and a removed-list by hand.
let mut removed = Vec::new();
let mut i = 0;
while i < layers.len() {
    if layers[i].is_empty() {
        removed.push(layers.remove(i)); // O(n) shift each time
    } else {
        i += 1;
    }
}

// NEW (1.87): one drain pass, removed elements yielded as you go.
let removed: Vec<Layer> = layers.extract_if(.., |l| l.is_empty()).collect();
```

`Vec::into_raw_parts` (1.93) decomposes a `Vec` into `(ptr, len, capacity)`. This is for the FFI seam only — handing a pixel buffer to a C-ABI encoder that wants the three parts, then rebuilding with `Vec::from_raw_parts`. Do not use it to "optimize" ordinary buffer passing; a `&[u8]` or `&mut [u8]` slice is the right currency for almost everything we do with `Vec<u8>` pixel data.

`Vec::push_mut` and `Vec::insert_mut` (1.95) push or insert and hand back a `&mut` to the element just placed, saving the `last_mut().unwrap()` that the no-unwrap rule bans anyway.

```rust
// OLD: push, then re-fetch the element to finish initializing it.
frames.push(Frame::blank(size));
let f = frames.last_mut().expect("just pushed"); // an unwrap we'd rather not write
f.set_duration(default_ms);

// NEW (1.95): the reference comes straight back.
let f = frames.push_mut(Frame::blank(size));
f.set_duration(default_ms);
```

Several `Vec` accessors became `const` in 1.87: `Vec::len`, `Vec::is_empty`, `Vec::capacity`, `Vec::as_ptr`, `Vec::as_mut_ptr`, `Vec::as_slice`, `Vec::as_mut_slice`. That lets a `const fn` over a `Vec` field assert sizes at compile time.

## Slices (`[T]`)

The disjoint-mutable-access family lands in 1.86: `<[_]>::get_disjoint_mut` returns several `&mut` to non-overlapping indices at once, checked for overlap, with `slice::GetDisjointMutError` on conflict (and `get_disjoint_unchecked_mut` when you've already proven disjointness). This is the clean way to swap or blend two cels' pixels held in one buffer without the borrow checker rejecting two `&mut` into the same slice.

```rust
// OLD: split_at_mut gymnastics to get two non-overlapping &mut into one buffer.
let (lo, hi) = pixels.split_at_mut(boundary);
let a = &mut lo[ai];
let b = &mut hi[bi - boundary]; // index math you have to get right by hand

// NEW (1.86): ask for both indices; overlap is a checked error.
let [a, b] = pixels.get_disjoint_mut([ai, bi]).map_err(Error::Overlap)?;
```

The split-off family (1.87) reshapes a slice without arithmetic: `split_off`/`split_off_mut` carve out a subrange, and `split_off_first`/`split_off_first_mut`/`split_off_last`/`split_off_last_mut` peel one end and advance the slice in place — the ergonomic way to walk a row buffer head-first.

The chunking-as-arrays family (1.88) is the one to know for stride work. `<[T]>::as_chunks::<N>()` returns `(&[[T; N]], &[T])` — a slice of fixed-size arrays plus the leftover remainder — and `as_chunks_mut` is its mutable twin. For an RGBA8 buffer, `N = 4` turns a flat `&[u8]` into per-pixel `[u8; 4]` arrays the compiler can keep in registers, no per-iteration bounds check. `as_rchunks`/`as_rchunks_mut` chunk from the end. The `as_chunks_unchecked`/`as_chunks_unchecked_mut` variants are `unsafe` and forbidden workspace-wide — never reach for them; the checked forms cost nothing once the length is known.

```rust
// OLD: index four bytes at a time, every access bounds-checked.
for px in 0..(rgba.len() / 4) {
    let base = px * 4;
    let r = rgba[base];
    let a = rgba[base + 3];
    // ...
}

// NEW (1.88): typed [u8; 4] pixels, remainder handled explicitly.
let (pixels, tail) = rgba.as_chunks::<4>();
debug_assert!(tail.is_empty(), "RGBA8 buffer must be 4-aligned");
for [r, g, b, a] in pixels {
    // r/g/b/a are u8, no per-access bounds check
}
```

Two more slice constructors view a prefix as a fixed array: `<[T]>::as_array::<N>()` and `as_mut_array::<N>()` (1.93) return `Option<&[T; N]>` — handy to read a 4-byte header or a fixed palette stride off the front of a buffer without slicing-and-`try_into`.

`<[T]>::array_windows::<N>()` (1.94) iterates overlapping fixed-size windows as `&[T; N]`, the typed cousin of `windows()`. Use it for a 3-wide neighborhood pass over a scanline. `<[T]>::element_offset` (1.94) gives the index of an element from a reference into the slice — useful when a selection holds a `&Pixel` and you need its position back.

Const slice ops worth noting: `<[T]>::reverse` is `const` in 1.90; `<[T; N]>::as_mut_slice` is `const` in 1.89; `<[T]>::rotate_left`/`rotate_right` are `const` in 1.92; `<[[T; N]]>::as_flattened`/`as_flattened_mut` (flatten a slice of arrays to a flat slice) are `const` in 1.87; `<[T]>::copy_from_slice` is `const` in 1.87.

## Arrays (`[T; N]`)

`core::array::repeat` (1.91) builds `[T; N]` by cloning a value N times — the array analog of `vec![x; n]`, for a fixed-size tool palette or a default-kernel array.

```rust
// OLD: from_fn ignoring the index just to clone a value N times.
let kernel: [f32; 9] = std::array::from_fn(|_| weight);

// NEW (1.91): say what you mean.
let kernel: [f32; 9] = std::array::repeat(weight);
```

`<[T; N]>::each_ref` and `each_mut` are `const` as of 1.91 — turn `[T; N]` into `[&T; N]`/`[&mut T; N]` in const context.

## Iterators

`core::iter::chain` (1.91) is the free-function form of `Iterator::chain` — `iter::chain(a, b)` reads left-to-right when you're joining two frame ranges or two layer sources, instead of burying the second operand in a method call.

```rust
// OLD: method form hides the second sequence at the tail.
let all = base_layers.iter().chain(overlay_layers.iter());

// NEW (1.91): both operands sit side by side.
let all = std::iter::chain(&base_layers, &overlay_layers);
```

`Peekable::next_if_map` and `next_if_map_mut` (1.94) consume-and-map the next item only when a closure returns `Some` — one step where you'd otherwise `peek`, test, map, then `next`. Reach for it in a token/command stream parser that pulls a value only when the lookahead matches.

`Extend` and `FromIterator` for tuples of arity 1-12 (1.85) let you `collect()` an iterator of tuples straight into a tuple of collections, or `extend` several collections in one pass — fan an iterator of `(LayerId, Cel)` into parallel `Vec`s without a manual loop.

```rust
// OLD: unzip by hand into two vectors.
let mut ids = Vec::new();
let mut cels = Vec::new();
for (id, cel) in source {
    ids.push(id);
    cels.push(cel);
}

// NEW (1.85): collect into a tuple of collections directly.
let (ids, cels): (Vec<LayerId>, Vec<Cel>) = source.collect();
```

## HashMap / HashSet

`HashMap::get_disjoint_mut` (1.86) returns several `&mut` values for distinct keys at once (with `get_disjoint_unchecked_mut` when disjointness is already proven) — the map counterpart to the slice method, for mutating two cache entries together.

`HashMap::extract_if` and `HashSet::extract_if` (1.88) drain matching entries and yield them, the same filter-and-recover pattern as the `Vec` version — evict stale GPU-texture cache entries and get the evicted set back to free their handles.

```rust
// NEW (1.88): drain stale entries, recover them to release GPU handles.
let evicted: Vec<_> = texture_cache
    .extract_if(|_, tex| tex.last_used < cutoff)
    .collect();
for (_, tex) in evicted {
    tex.release();
}
```

`BuildHasherDefault::new` (1.85, also `const`) constructs the default `BuildHasher` without a map in hand, for wiring a custom-hashed map's hasher up front.

## BTreeMap / BTreeSet

`BTreeMap::extract_if` and `BTreeSet::extract_if` (1.91) bring the draining filter to the ordered collections — same shape, ordered traversal.

`btree_map::Entry::insert_entry` and `VacantEntry::insert_entry` (1.92) insert through the entry API and return the `OccupiedEntry` instead of a bare `&mut V`, so you keep the entry handle for follow-up work in the same lookup.

## VecDeque

`VecDeque::pop_front_if` and `pop_back_if` (1.93) are the deque analogs of `Vec::pop_if` — pop an end only if the predicate holds. Fits a job queue you drain conditionally from either end. `VecDeque::push_front_mut`, `push_back_mut`, and `insert_mut` (1.95) return a `&mut` to the inserted element, same win as the `Vec` versions.

## LinkedList

`LinkedList::extract_if` (1.87) and `LinkedList::push_front_mut`/`push_back_mut` (1.95) round out the same patterns. A note on altitude: we almost never want `LinkedList` — a `Vec` or `VecDeque` wins on cache behavior for nearly everything in this codebase. Mentioned for completeness, not as a nudge to use it.

## When not to reach for these

- `into_raw_parts` (Vec/String) and the `*_unchecked` slice/chunk methods are FFI-and-unsafe territory. `unsafe` is forbidden workspace-wide, so the unchecked chunkers are off the table; the raw-parts methods belong only at a genuine C-ABI boundary.
- Do not swap a clear, working `retain` for `extract_if` unless you actually need the removed elements. Same for replacing a plain `chain` method call with `iter::chain` — that's churn, not improvement.
- `get_disjoint_mut` earns its keep only when you genuinely need two-plus simultaneous `&mut` into one container. For a single mutable element, ordinary indexing or `get_mut` stays clearer.

