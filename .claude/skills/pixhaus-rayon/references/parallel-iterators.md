# rayon 1.12.0 — `ParallelIterator` and `IndexedParallelIterator`

The two central traits. `ParallelIterator` is the base; `IndexedParallelIterator`
extends it for sources whose exact length is known up front (slices, ranges of small
integers, `Vec`), which unlocks indexed operations (`zip`, `enumerate`, `collect_into_vec`,
`position_*`, `rev`). Both are brought into scope by `use rayon::prelude::*;`.

```rust
pub trait ParallelIterator: Sized + Send {
    type Item: Send;
    // one required method for implementors:
    fn drive_unindexed<C>(self, consumer: C) -> C::Result where C: UnindexedConsumer<Self::Item>;
    // everything below is provided.
}

pub trait IndexedParallelIterator: ParallelIterator {
    fn len(&self) -> usize;                       // required of implementors
    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result;          // required
    fn with_producer<CB: ProducerCallback<Self::Item>>(self, cb: CB) -> CB::Output; // required
}
```

`Item: Send` always. Every closure bound is `Fn(...) + Sync + Send` (it runs on many
threads at once) — this is why captured state must be thread-safe.

## Table of contents

- [Adaptors (return another parallel iterator)](#adaptors)
- [The fold family (adaptors, NOT terminals)](#the-fold-family)
- [Terminal consumers (return a value)](#terminal-consumers)
- [`fold` vs `reduce` — read this](#fold-vs-reduce)
- [`_with` / `_init` per-job state](#_with--_init-per-job-state)
- [find / position ordering](#find--position-ordering)
- [IndexedParallelIterator-only methods](#indexedparalleliterator-only-methods)
- [Granularity controls](#granularity-controls)

## Adaptors

Return another `ParallelIterator`; lazy, compose freely. `R: Send` on anything that
produces new items.

```rust
fn map<F, R>(self, f: F) -> Map<Self, F>
    where F: Fn(Self::Item) -> R + Sync + Send, R: Send
fn map_with<T, F, R>(self, init: T, f: F) -> MapWith<Self, T, F>
    where F: Fn(&mut T, Self::Item) -> R + Sync + Send, T: Send + Clone, R: Send
fn map_init<INIT, T, F, R>(self, init: INIT, f: F) -> MapInit<Self, INIT, F>
    where F: Fn(&mut T, Self::Item) -> R + Sync + Send, INIT: Fn() -> T + Sync + Send, R: Send
fn cloned<'a, T>(self) -> Cloned<Self>   where T: 'a + Clone + Send, Self: ParallelIterator<Item = &'a T>
fn copied<'a, T>(self) -> Copied<Self>   where T: 'a + Copy + Send,  Self: ParallelIterator<Item = &'a T>
fn inspect<OP>(self, op: OP) -> Inspect<Self, OP>   where OP: Fn(&Self::Item) + Sync + Send
fn update<F>(self, f: F) -> Update<Self, F>          where F: Fn(&mut Self::Item) + Sync + Send
fn filter<P>(self, p: P) -> Filter<Self, P>          where P: Fn(&Self::Item) -> bool + Sync + Send
fn filter_map<P, R>(self, p: P) -> FilterMap<Self, P> where P: Fn(Self::Item) -> Option<R> + Sync + Send, R: Send
fn flat_map<F, PI>(self, f: F) -> FlatMap<Self, F>    where F: Fn(Self::Item) -> PI + Sync + Send, PI: IntoParallelIterator
fn flat_map_iter<F, SI>(self, f: F) -> FlatMapIter<Self, F> where F: Fn(Self::Item) -> SI + Sync + Send, SI: IntoIterator<Item: Send>
fn flatten(self) -> Flatten<Self>          where Self::Item: IntoParallelIterator
fn flatten_iter(self) -> FlattenIter<Self> where Self::Item: IntoIterator<Item: Send>
fn chain<C>(self, c: C) -> Chain<Self, C::Iter>   where C: IntoParallelIterator<Item = Self::Item>
fn intersperse(self, element: Self::Item) -> Intersperse<Self>   where Self::Item: Clone
fn while_some<T>(self) -> WhileSome<Self>   where Self: ParallelIterator<Item = Option<T>>, T: Send  // stops at first None
fn panic_fuse(self) -> PanicFuse<Self>      // makes other jobs bail faster after a panic
fn take_any(self, n: usize) -> TakeAny<Self>        // any n items (nondeterministic) — works on unindexed iterators
fn skip_any(self, n: usize) -> SkipAny<Self>
fn take_any_while<P>(self, p: P) -> TakeAnyWhile<Self, P> where P: Fn(&Self::Item) -> bool + Sync + Send
fn skip_any_while<P>(self, p: P) -> SkipAnyWhile<Self, P> where P: Fn(&Self::Item) -> bool + Sync + Send
```

`flat_map` flattens into a *parallel* sub-iterator (more splittable); `flat_map_iter`
flattens into a sequential one (cheaper when each item maps to a short `std` iterator).
Note `take_any`/`skip_any` (unordered) live here; the ordered `take`/`skip` are indexed-only.

## The fold family

These return adaptors that yield **per-job partial accumulators**, not a final value.

```rust
fn fold<T, ID, F>(self, identity: ID, fold_op: F) -> Fold<Self, ID, F>
    where F: Fn(T, Self::Item) -> T + Sync + Send, ID: Fn() -> T + Sync + Send, T: Send
fn fold_with<T, F>(self, init: T, fold_op: F) -> FoldWith<Self, T, F>
    where F: Fn(T, Self::Item) -> T + Sync + Send, T: Send + Clone
fn try_fold<T, R, ID, F>(self, identity: ID, fold_op: F) -> TryFold<Self, R, ID, F>
    where F: Fn(T, Self::Item) -> R + Sync + Send, ID: Fn() -> T + Sync + Send, R: Try<Output = T> + Send
fn try_fold_with<T, R, F>(self, init: T, fold_op: F) -> TryFoldWith<Self, R, F>
    where F: Fn(T, Self::Item) -> R + Sync + Send, R: Try<Output = T> + Send, T: Clone + Send
```

You almost always chain a terminal afterward: `.fold(id, op).reduce(id, combine)` or
`.fold(id, op).sum()`. `fold`'s accumulator `T` can differ from `Self::Item`; `reduce`'s
cannot — that's the main reason to choose `fold`.

## Terminal consumers

Drive the computation and **block the calling thread** until done; return a value.

```rust
fn for_each<OP>(self, op: OP)                 where OP: Fn(Self::Item) + Sync + Send
fn for_each_with<T, OP>(self, init: T, op: OP) where OP: Fn(&mut T, Self::Item) + Sync + Send, T: Send + Clone
fn for_each_init<INIT, T, OP>(self, init: INIT, op: OP) where OP: Fn(&mut T, Self::Item) + Sync + Send, INIT: Fn() -> T + Sync + Send
fn try_for_each<OP, R>(self, op: OP) -> R     where OP: Fn(Self::Item) -> R + Sync + Send, R: Try<Output = ()> + Send
fn try_for_each_with<T, OP, R>(self, init: T, op: OP) -> R       where OP: Fn(&mut T, Self::Item) -> R + Sync + Send, T: Send + Clone, R: Try<Output = ()> + Send
fn try_for_each_init<INIT, T, OP, R>(self, init: INIT, op: OP) -> R where OP: Fn(&mut T, Self::Item) -> R + Sync + Send, INIT: Fn() -> T + Sync + Send, R: Try<Output = ()> + Send

fn count(self) -> usize
fn reduce<OP, ID>(self, identity: ID, op: OP) -> Self::Item
    where OP: Fn(Self::Item, Self::Item) -> Self::Item + Sync + Send, ID: Fn() -> Self::Item + Sync + Send
fn reduce_with<OP>(self, op: OP) -> Option<Self::Item>   // None on empty
    where OP: Fn(Self::Item, Self::Item) -> Self::Item + Sync + Send
fn try_reduce<T, OP, ID>(self, identity: ID, op: OP) -> Self::Item   // Self::Item is a Try type
    where OP: Fn(T, T) -> Self::Item + Sync + Send, ID: Fn() -> T + Sync + Send, Self::Item: Try<Output = T>
fn try_reduce_with<T, OP>(self, op: OP) -> Option<Self::Item>
    where OP: Fn(T, T) -> Self::Item + Sync + Send, Self::Item: Try<Output = T>

fn sum<S>(self) -> S     where S: Send + Sum<Self::Item> + Sum<S>
fn product<P>(self) -> P where P: Send + Product<Self::Item> + Product<P>

fn min(self) -> Option<Self::Item>  where Self::Item: Ord
fn min_by<F>(self, f: F) -> Option<Self::Item>      where F: Sync + Send + Fn(&Self::Item, &Self::Item) -> Ordering
fn min_by_key<K, F>(self, f: F) -> Option<Self::Item> where K: Ord + Send, F: Sync + Send + Fn(&Self::Item) -> K
fn max(self) -> Option<Self::Item>  where Self::Item: Ord
fn max_by<F>(self, f: F) -> Option<Self::Item>      where F: Sync + Send + Fn(&Self::Item, &Self::Item) -> Ordering
fn max_by_key<K, F>(self, f: F) -> Option<Self::Item> where K: Ord + Send, F: Sync + Send + Fn(&Self::Item) -> K

fn find_any<P>(self, p: P) -> Option<Self::Item>    where P: Fn(&Self::Item) -> bool + Sync + Send
fn find_first<P>(self, p: P) -> Option<Self::Item>  where P: Fn(&Self::Item) -> bool + Sync + Send
fn find_last<P>(self, p: P) -> Option<Self::Item>   where P: Fn(&Self::Item) -> bool + Sync + Send
fn find_map_any<P, R>(self, p: P) -> Option<R>      where P: Fn(Self::Item) -> Option<R> + Sync + Send, R: Send
fn find_map_first<P, R>(self, p: P) -> Option<R>    where P: Fn(Self::Item) -> Option<R> + Sync + Send, R: Send
fn find_map_last<P, R>(self, p: P) -> Option<R>     where P: Fn(Self::Item) -> Option<R> + Sync + Send, R: Send
fn any<P>(self, p: P) -> bool   where P: Fn(Self::Item) -> bool + Sync + Send
fn all<P>(self, p: P) -> bool   where P: Fn(Self::Item) -> bool + Sync + Send

fn collect<C>(self) -> C   where C: FromParallelIterator<Self::Item>
fn unzip<A, B, FromA, FromB>(self) -> (FromA, FromB)
    where Self: ParallelIterator<Item = (A, B)>, FromA: Default + Send + ParallelExtend<A>, FromB: Default + Send + ParallelExtend<B>, A: Send, B: Send
fn partition<A, B, P>(self, p: P) -> (A, B)
    where A: Default + Send + ParallelExtend<Self::Item>, B: Default + Send + ParallelExtend<Self::Item>, P: Fn(&Self::Item) -> bool + Sync + Send
fn partition_map<A, B, P, L, R>(self, p: P) -> (A, B)
    where A: Default + Send + ParallelExtend<L>, B: Default + Send + ParallelExtend<R>, P: Fn(Self::Item) -> Either<L, R> + Sync + Send, L: Send, R: Send
fn collect_vec_list(self) -> LinkedList<Vec<Self::Item>>   // low-level; rarely needed
fn opt_len(&self) -> Option<usize>
```

## fold vs reduce

The distinction that trips everyone:

| | returns | accumulator type | use when |
|---|---|---|---|
| `reduce(id, op)` | `Self::Item` (a value) | same as `Item` | combining items into one of the same type, op is associative |
| `reduce_with(op)` | `Option<Self::Item>` | same as `Item` | same, but tolerate empty (no identity) |
| `fold(id, op)` | `Fold<…>` (an **iterator** of partials) | any `T` | accumulator differs from item, or you want to amortize per-job setup |

```rust
// histogram: accumulator (HashMap) differs from item (&Pixel) -> fold, then reduce to merge
let hist: HashMap<[u8; 4], u64> = pixels
    .par_iter()
    .fold(HashMap::new, |mut acc, px| { *acc.entry(px.rgba()).or_default() += 1; acc })
    .reduce(HashMap::new, |mut a, b| { for (k, v) in b { *a.entry(k).or_default() += v; } a });

// sum of weights: same type throughout -> reduce (or just .sum())
let total: u64 = weights.par_iter().copied().reduce(|| 0, |a, b| a + b);
```

`fold` alone computes nothing you can read — it yields per-job partials. Forgetting the
trailing `reduce`/`sum` is the classic bug.

## `_with` / `_init` per-job state

For mutable scratch state that must NOT be shared across threads, give each parallel job
its own copy instead of wrapping a shared `Mutex`:

- **`map_with(init, f)` / `for_each_with` / `fold_with` / `try_*_with`** — `init: T` must
  be `Clone`. rayon clones it once **per job** (not per item) and passes `&mut T`. Good
  for a cheap-to-clone seed: a channel `Sender`, a small config, an RNG seed.
- **`map_init(make, f)` / `for_each_init` / `try_*_init`** (and `fold`'s identity closure)
  — `make: Fn() -> T` is called once **per job** to construct fresh state. Good when the
  state isn't cheaply `Clone`: a reusable scratch buffer, a fresh `ThreadRng`.

rayon makes no promise about how many times init runs — it tracks the work split, not the
thread count. Don't rely on a specific count.

```rust
// each job gets its own scratch line buffer, allocated once, reused across its pixels
rows.par_chunks_mut(width * 4)
    .for_each_init(|| vec![0u8; width * 4], |scratch, row| blur_row(row, scratch));
```

## find / position ordering

- `_any` (`find_any`, `find_map_any`, `position_any`): returns *some* match — whichever a
  worker hits first. **Nondeterministic**, fastest (any match lets every job bail).
- `_first` (`find_first`, `find_map_first`, `position_first`): the lowest-index match.
  **Deterministic.** Slower — a job can only cancel work that's *after* the current best.
- `_last`: the highest-index match. Deterministic.

Reach for `_any` when you don't care which match you get; reach for `_first` when output
must be reproducible. rayon's own docs steer you to `_any` by default for the speed.

## IndexedParallelIterator-only methods

Available only when the length is known exactly (slices, `Vec`, small-int ranges):

```rust
fn collect_into_vec(self, target: &mut Vec<Self::Item>)         // reuse an allocation; placed by index
fn unzip_into_vecs<A, B>(self, left: &mut Vec<A>, right: &mut Vec<B>) where Self: IndexedParallelIterator<Item = (A, B)>, A: Send, B: Send

fn zip<Z>(self, other: Z) -> Zip<Self, Z::Iter>        where Z: IntoParallelIterator<Iter: IndexedParallelIterator>
fn zip_eq<Z>(self, other: Z) -> ZipEq<Self, Z::Iter>   where Z: IntoParallelIterator<Iter: IndexedParallelIterator>  // panics if lengths differ
fn interleave<I>(self, other: I) -> Interleave<Self, I::Iter>            where I: IntoParallelIterator<Item = Self::Item, Iter: IndexedParallelIterator>
fn interleave_shortest<I>(self, other: I) -> InterleaveShortest<Self, I::Iter> where I: IntoParallelIterator<Item = Self::Item, Iter: IndexedParallelIterator>

fn chunks(self, chunk_size: usize) -> Chunks<Self>     // yields Vec<Item> chunks
fn fold_chunks<T, ID, F>(self, n: usize, id: ID, f: F) -> FoldChunks<Self, ID, F>      where ID: Fn() -> T + Send + Sync, F: Fn(T, Self::Item) -> T + Send + Sync, T: Send
fn fold_chunks_with<T, F>(self, n: usize, init: T, f: F) -> FoldChunksWith<Self, T, F> where T: Send + Clone, F: Fn(T, Self::Item) -> T + Send + Sync

fn enumerate(self) -> Enumerate<Self>
fn step_by(self, step: usize) -> StepBy<Self>
fn skip(self, n: usize) -> Skip<Self>          // ordered (vs skip_any)
fn take(self, n: usize) -> Take<Self>          // ordered (vs take_any)
fn rev(self) -> Rev<Self>

fn position_any<P>(self, p: P) -> Option<usize>    where P: Fn(Self::Item) -> bool + Sync + Send
fn position_first<P>(self, p: P) -> Option<usize>  where P: Fn(Self::Item) -> bool + Sync + Send
fn position_last<P>(self, p: P) -> Option<usize>   where P: Fn(Self::Item) -> bool + Sync + Send
fn positions<P>(self, p: P) -> Positions<Self, P>  where P: Fn(Self::Item) -> bool + Sync + Send  // adaptor yielding usize

// lexicographic comparison against another indexed iterator (terminal):
fn cmp<I>(self, other: I) -> Ordering          where I: IntoParallelIterator<Item = Self::Item, Iter: IndexedParallelIterator>, Self::Item: Ord
fn partial_cmp<I>(self, other: I) -> Option<Ordering>  where I: IntoParallelIterator<Iter: IndexedParallelIterator>, Self::Item: PartialOrd<I::Item>
fn eq / ne / lt / le / gt / ge  // same shape, return bool
```

`collect_into_vec` reuses a caller-owned `Vec` (no fresh allocation) — worth it for a
buffer you fill every frame. `zip_eq` panics on a length mismatch; plain `zip` truncates
to the shorter.

## Granularity controls

```rust
fn with_min_len(self, min: usize) -> MinLen<Self>   // don't split below this many items per job
fn with_max_len(self, max: usize) -> MaxLen<Self>   // force splitting at least this fine
fn by_uniform_blocks(self, block_size: usize) -> UniformBlocks<Self>     // fixed sequential blocks; better cache locality
fn by_exponential_blocks(self) -> ExponentialBlocks<Self>                // growing blocks; good for left-to-right short-circuit
```

`with_min_len` is the lever against over-parallelizing small dirty regions: raise it so
each job carries enough work to outweigh the split/steal overhead. When two iterators are
combined (`zip`), the **greater** min and the **lesser** max win.
