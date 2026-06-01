# rayon 1.12.0 — sources, the prelude, collecting, and the bridge

How data gets *into* a parallel iterator and *out* of one. All of these traits are in
`use rayon::prelude::*;`.

## The prelude — what you import

`use rayon::prelude::*;` brings thirteen extension traits into scope (all traits, no
types):

```
ParallelIterator              IndexedParallelIterator
IntoParallelIterator          IntoParallelRefIterator         IntoParallelRefMutIterator
FromParallelIterator          ParallelExtend
ParallelBridge
ParallelDrainFull             ParallelDrainRange
ParallelSlice                 ParallelSliceMut                ParallelString
```

If `.par_iter()` / `.par_sort()` / `.par_chunks_mut()` "doesn't exist," the prelude is
missing. This is the most common first error.

## Getting in: the `Into*` family

```rust
pub trait IntoParallelIterator {
    type Iter: ParallelIterator<Item = Self::Item>;
    type Item: Send;
    fn into_par_iter(self) -> Self::Iter;             // consumes self (owned items)
}

pub trait IntoParallelRefIterator<'data> {
    type Iter: ParallelIterator<Item = Self::Item>;
    type Item: Send + 'data;
    fn par_iter(&'data self) -> Self::Iter;           // borrows; items are &T
}

pub trait IntoParallelRefMutIterator<'data> {
    type Iter: ParallelIterator<Item = Self::Item>;
    type Item: Send + 'data;
    fn par_iter_mut(&'data mut self) -> Self::Iter;   // borrows mut; items are &mut T
}
```

`into_par_iter` is the root. `par_iter` / `par_iter_mut` are blanket impls that route
through `IntoParallelIterator` on `&T` / `&mut T` — exactly mirroring std's
`into_iter` / `iter` / `iter_mut`. The mental swap: take any working sequential chain
and change the leading `iter()` → `par_iter()`, `iter_mut()` → `par_iter_mut()`,
`into_iter()` → `into_par_iter()`.

**Which std types parallelize** (via `IntoParallelIterator` on the type and/or its
references): `Vec<T>`, `Box<[T]>`, `[T; N]`, `&[T]`, `&mut [T]`, `VecDeque<T>`,
`LinkedList<T>`, `BinaryHeap<T>`, `HashMap<K, V, S>`, `HashSet<T, S>`, `BTreeMap<K, V>`,
`BTreeSet<T>`, `Range<T>`, `RangeInclusive<T>`, `Option<T>`, `Result<T, E>`, and tuples up
to 12. `str`/`String` go through `ParallelString` (see `references/slices-and-strings.md`).

## Getting out: `collect`, `FromParallelIterator`, `ParallelExtend`

```rust
pub trait FromParallelIterator<T: Send> {
    fn from_par_iter<I>(par_iter: I) -> Self where I: IntoParallelIterator<Item = T>;
}
pub trait ParallelExtend<T: Send> {
    fn par_extend<I>(&mut self, par_iter: I) where I: IntoParallelIterator<Item = T>;
}
```

`.collect::<C>()` works for any `C: FromParallelIterator`: `Vec`, `VecDeque`,
`LinkedList`, `BinaryHeap`, `Box<[T]>`, `Arc<[T]>`, `Rc<[T]>`, `HashMap`, `HashSet`,
`BTreeMap`, `BTreeSet`, `String`, `Box<str>`, `OsString`, `Cow`, `Either`, and — usefully
— `Option<C>` / `Result<C, E>` (collect a fallible parallel computation, short-circuiting
to the first `None`/`Err`). Collecting into a `(FromA, FromB)` tuple unzips.

For an indexed iterator with a buffer you already own, prefer
`collect_into_vec(&mut vec)` over `collect()` — it reuses the allocation, which matters
for a per-frame fill. `par_extend` appends into an existing collection in parallel.

## The sequential-iterator bridge — `par_bridge`

```rust
pub trait ParallelBridge: Sized {
    fn par_bridge(self) -> IterBridge<Self>;
}
// blanket: impl for every `T: Iterator + Send` whose `T::Item: Send`.
```

Turns an ordinary sequential `Iterator` into a `ParallelIterator` by pulling items
`next()`-one-at-a-time (synchronized) and farming them to the pool. Caveats from the docs:

- **Order is not preserved** — items finish in nondeterministic order.
- **The `next()` pull is a serialization point** — if the source can't produce fast
  enough, it bottlenecks the pool.
- **It's less efficient than a native parallel source.** Use it only when the source is
  inherently sequential — a `channel` receiver, a file/socket reader, a generator. When
  the data is already a `Vec`/slice/range, use `par_iter`/`into_par_iter`, which split
  cleanly with no per-item lock.

## Draining

```rust
pub trait ParallelDrainFull {                       // whole collection
    type Iter: ParallelIterator<Item = Self::Item>;
    type Item: Send;
    fn par_drain(self) -> Self::Iter;
}
pub trait ParallelDrainRange<Idx = usize> {         // a range of an indexable collection
    type Iter: ParallelIterator<Item = Self::Item>;
    type Item: Send;
    fn par_drain<R: RangeBounds<Idx>>(self, range: R) -> Self::Iter;
}
```

`ParallelDrainFull` is implemented on `&mut HashMap` (`Item = (K, V)`), `&mut HashSet`,
`&mut BinaryHeap`. `ParallelDrainRange` on `&mut Vec<T>`, `&mut VecDeque<T>`,
`&mut String` (`Item = char`). Both keep the original capacity and remove the drained
items on drop even if you don't consume the iterator fully.

## Source free functions (`rayon::iter::*`)

```rust
pub fn split<D, S>(data: D, splitter: S) -> Split<D, S>
    where D: Send, S: Fn(D) -> (D, Option<D>) + Sync
pub fn empty<T: Send>() -> Empty<T>
pub fn once<T: Send>(item: T) -> Once<T>
pub fn repeat<T: Clone + Send>(element: T) -> Repeat<T>          // infinite — bound with zip/take
pub fn repeat_n<T: Clone + Send>(element: T, n: usize) -> RepeatN<T>
// repeatn(...) exists but is deprecated in favor of repeat_n

pub fn walk_tree<S, B, I>(root: S, children_of: B) -> WalkTree<S, B>
    where S: Send, B: Fn(&S) -> I + Send + Sync, I: IntoIterator<Item = S, IntoIter: DoubleEndedIterator>
pub fn walk_tree_prefix<S, B, I>(root: S, children_of: B) -> WalkTreePrefix<S, B>   // parent before children
    where S: Send, B: Fn(&S) -> I + Send + Sync, I: IntoIterator<Item = S, IntoIter: DoubleEndedIterator>
pub fn walk_tree_postfix<S, B, I>(root: S, children_of: B) -> WalkTreePostfix<S, B>  // children before parent
    where S: Send, B: Fn(&S) -> I + Send + Sync, I: IntoIterator<Item = S>
```

`split` is the general-purpose recursive divider: hand it arbitrary `data` and a closure
that halves it (returning `None` for the second half when it can't split further); rayon
recurses until it has enough pieces to feed the cores. Useful for parallelizing a custom
data structure that isn't a slice — e.g. a region of the canvas described by a rect, split
into quadrants. `walk_tree*` parallelizes tree traversal (a layer group hierarchy, a
quad-tree); use plain `walk_tree` unless you need pre/post order.

## Ranges and the wide-integer caveat

`a..b` parallelizes for all integer types and `char`. But:

> `zip` requires `IndexedParallelIterator`, which is **not** implemented for `u64`,
> `i64`, `u128`, or `i128`.

A range of those wide types can exceed `usize`, so it only implements `ParallelIterator`
— no `zip`, `enumerate`, `collect_into_vec`, or other indexed ops. Ranges of
`u8/i8/u16/i16/u32/i32/usize/isize` and `char` are fully indexed. For pixel coordinates
this is a non-issue (`usize`/`u32` are indexed); just don't reach for `(0u64..n).zip(...)`.

```rust
// fill pixel indices in parallel — usize range is indexed, so zip/enumerate work
(0..pixel_count).into_par_iter().for_each(|i| { /* ... */ });
```
