# futures::stream API reference

API reference for the `stream` module of the `futures` crate, version 0.3.32. A
`Stream` is the async analogue of `Iterator`: it produces a sequence of values
over time, one per `.await`. Streams are lazy — they do nothing until polled —
and the `StreamExt` / `TryStreamExt` extension traits supply the adapters
(`map`, `filter`, `buffered`, `for_each_concurrent`, etc.).

## Contents

- [Core traits: Stream, FusedStream](#core-traits)
- [Type aliases: BoxStream, LocalBoxStream](#type-aliases)
- [StreamExt adapters](#streamext-adapters)
- [Concurrency: buffered vs buffer_unordered vs for_each_concurrent vs FuturesUnordered](#concurrency)
- [TryStreamExt adapters](#trystreamext-adapters)
- [FuturesUnordered](#futuresunordered)
- [FuturesOrdered](#futuresordered)
- [select_all / SelectAll / stream_select!](#select)
- [Constructors](#constructors)
- [Gotchas](#gotchas)

## Core traits

### Stream

The async `Iterator`. `poll_next` returns `Poll::Ready(Some(item))` for a value,
`Poll::Ready(None)` when exhausted, `Poll::Pending` when not yet ready.

```rust
pub trait Stream {
    type Item;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>>;
    fn size_hint(&self) -> (usize, Option<usize>) { (0, None) }
}
```

Relationship to `Iterator`: same shape, but `poll_next` takes `Pin<&mut Self>`
plus a `Context` and can return `Pending`. `Item` plays the role of
`Iterator::Item`; `size_hint` matches `Iterator::size_hint` (lower bound,
optional upper bound). You rarely call `poll_next` directly — use `StreamExt`
methods like `next().await`.

### FusedStream

Marks a stream that tracks its own termination, so it is safe to poll after it
has returned `None`.

```rust
pub trait FusedStream: Stream {
    fn is_terminated(&self) -> bool;
}
```

`is_terminated()` returns `true` once the stream will never produce another
value. Required by `select_next_some` and by `select!`-style combinators that
must avoid re-polling a finished stream. Get one from `StreamExt::fuse`.

## Type aliases

| Alias | Definition | Use |
|-------|-----------|-----|
| `BoxStream<'a, T>` | `Pin<Box<dyn Stream<Item = T> + Send + 'a>>` | Type-erased stream that can cross threads. Produced by `boxed()`. |
| `LocalBoxStream<'a, T>` | `Pin<Box<dyn Stream<Item = T> + 'a>>` | Same, without `Send`. Produced by `boxed_local()`. |

## StreamExt adapters

Import `use futures::stream::StreamExt;` (or `futures::StreamExt`) to call these.
Adapters are lazy combinators; the terminal ones (`next`, `collect`, `fold`,
`for_each`, `count`, `concat`, `unzip`) return a `Future` you must `.await`.

### Terminal and access methods

| Method | Signature | What it does |
|--------|-----------|--------------|
| `next` | `fn next(&mut self) -> Next<'_, Self> where Self: Unpin` | Future for the next item; resolves to `Option<Self::Item>`. |
| `into_future` | `fn into_future(self) -> StreamFuture<Self> where Self: Sized + Unpin` | Future yielding `(Option<Item>, Self)` so you get the tail back. |
| `collect` | `fn collect<C>(self) -> Collect<Self, C> where C: Default + Extend<Self::Item>, Self: Sized` | Drains the stream into a collection `C`. |
| `unzip` | `fn unzip<A, B, FromA, FromB>(self) -> Unzip<Self, FromA, FromB> where FromA: Default + Extend<A>, FromB: Default + Extend<B>, Self: Sized + Stream<Item = (A, B)>` | Splits a stream of pairs into two collections. |
| `concat` | `fn concat(self) -> Concat<Self> where Self: Sized, Self::Item: Extend<<Self::Item as IntoIterator>::Item> + IntoIterator + Default` | Concatenates all items (each `Item` is itself extendable, e.g. `Vec`). |
| `count` | `fn count(self) -> Count<Self> where Self: Sized` | Counts items; resolves to `usize`. |
| `fold` | `fn fold<T, Fut, F>(self, init: T, f: F) -> Fold<Self, Fut, T, F> where F: FnMut(T, Self::Item) -> Fut, Fut: Future<Output = T>, Self: Sized` | Async accumulate over the stream. |
| `for_each` | `fn for_each<Fut, F>(self, f: F) -> ForEach<Self, Fut, F> where F: FnMut(Self::Item) -> Fut, Fut: Future<Output = ()>, Self: Sized` | Runs an async closure per item, sequentially. |
| `for_each_concurrent` | `fn for_each_concurrent<Fut, F>(self, limit: impl Into<Option<usize>>, f: F) -> ForEachConcurrent<Self, Fut, F> where F: FnMut(Self::Item) -> Fut, Fut: Future<Output = ()>, Self: Sized` | Runs the closure with up to `limit` futures in flight at once (`None` = unbounded). |
| `by_ref` | `fn by_ref(&mut self) -> &mut Self` | Borrow so a partial consume doesn't move the stream. |

### Mapping and filtering

| Method | Signature | What it does |
|--------|-----------|--------------|
| `map` | `fn map<T, F>(self, f: F) -> Map<Self, F> where F: FnMut(Self::Item) -> T, Self: Sized` | Sync transform of each item. |
| `then` | `fn then<Fut, F>(self, f: F) -> Then<Self, Fut, F> where F: FnMut(Self::Item) -> Fut, Fut: Future, Self: Sized` | Async transform; awaits each future before the next item. |
| `filter` | `fn filter<Fut, F>(self, f: F) -> Filter<Self, Fut, F> where F: FnMut(&Self::Item) -> Fut, Fut: Future<Output = bool>, Self: Sized` | Keeps items whose async predicate is `true`. |
| `filter_map` | `fn filter_map<Fut, T, F>(self, f: F) -> FilterMap<Self, Fut, F> where F: FnMut(Self::Item) -> Fut, Fut: Future<Output = Option<T>>, Self: Sized` | Filter + map; keeps `Some`. |
| `scan` | `fn scan<S, B, Fut, F>(self, initial_state: S, f: F) -> Scan<Self, S, Fut, F> where F: FnMut(&mut S, Self::Item) -> Fut, Fut: Future<Output = Option<B>>, Self: Sized` | Stateful map; ends when the closure returns `None`. |
| `inspect` | `fn inspect<F>(self, f: F) -> Inspect<Self, F> where F: FnMut(&Self::Item), Self: Sized` | Side-effect peek, item passes through unchanged. |
| `enumerate` | `fn enumerate(self) -> Enumerate<Self> where Self: Sized` | Yields `(index, item)`. |
| `cloned` | `fn cloned<'a, T>(self) -> Cloned<Self> where Self: Sized + Stream<Item = &'a T>, T: Clone + 'a` | Clones each `&T` to `T`. (verify) |
| `copied` | `fn copied<'a, T>(self) -> Copied<Self> where Self: Sized + Stream<Item = &'a T>, T: Copy + 'a` | Copies each `&T` to `T`. (verify) |

### Flattening

| Method | Signature | What it does |
|--------|-----------|--------------|
| `flatten` | `fn flatten(self) -> Flatten<Self> where Self::Item: Stream, Self: Sized` | Flattens a stream-of-streams sequentially (drains one fully before the next). |
| `flat_map` | `fn flat_map<U, F>(self, f: F) -> FlatMap<Self, U, F> where F: FnMut(Self::Item) -> U, U: Stream, Self: Sized` | `map` then `flatten`, sequential. |
| `flatten_unordered` | `fn flatten_unordered(self, limit: impl Into<Option<usize>>) -> FlattenUnorderedWithFlowController<Self, ()> where Self::Item: Stream + Unpin, Self: Sized` | Polls up to `limit` inner streams concurrently; items in completion order. |
| `flat_map_unordered` | `fn flat_map_unordered<U, F>(self, limit: impl Into<Option<usize>>, f: F) -> FlatMapUnordered<Self, U, F> where U: Stream + Unpin, F: FnMut(Self::Item) -> U, Self: Sized` | `map` then `flatten_unordered`. |

### Slicing and combining

| Method | Signature | What it does |
|--------|-----------|--------------|
| `take` | `fn take(self, n: usize) -> Take<Self> where Self: Sized` | First `n` items, then ends. |
| `take_while` | `fn take_while<Fut, F>(self, f: F) -> TakeWhile<Self, Fut, F> where F: FnMut(&Self::Item) -> Fut, Fut: Future<Output = bool>, Self: Sized` | Items while async predicate is `true`. |
| `take_until` | `fn take_until<Fut>(self, fut: Fut) -> TakeUntil<Self, Fut> where Fut: Future, Self: Sized` | Items until the given future resolves. |
| `skip` | `fn skip(self, n: usize) -> Skip<Self> where Self: Sized` | Drops the first `n` items. |
| `skip_while` | `fn skip_while<Fut, F>(self, f: F) -> SkipWhile<Self, Fut, F> where F: FnMut(&Self::Item) -> Fut, Fut: Future<Output = bool>, Self: Sized` | Drops items while async predicate is `true`. |
| `zip` | `fn zip<St>(self, other: St) -> Zip<Self, St> where St: Stream, Self: Sized` | Pairs items; ends when either ends. |
| `chain` | `fn chain<St>(self, other: St) -> Chain<Self, St> where St: Stream<Item = Self::Item>, Self: Sized` | Yields all of `self`, then all of `other`. |
| `peekable` | `fn peekable(self) -> Peekable<Self> where Self: Sized` | Allows `.peek().await` without consuming. |
| `chunks` | `fn chunks(self, capacity: usize) -> Chunks<Self> where Self: Sized` | Batches items into `Vec`s of up to `capacity` (waits to fill). |
| `ready_chunks` | `fn ready_chunks(self, capacity: usize) -> ReadyChunks<Self> where Self: Sized` | Batches only items already ready, up to `capacity` (no extra waiting). |
| `cycle` | `fn cycle(self) -> Cycle<Self> where Self: Sized + Clone` | Repeats the stream forever. |

### Concurrency adapters

| Method | Signature | What it does |
|--------|-----------|--------------|
| `buffered` | `fn buffered(self, n: usize) -> Buffered<Self> where Self::Item: Future, Self: Sized` | Stream of futures; runs up to `n` at once, yields outputs in stream order. |
| `buffer_unordered` | `fn buffer_unordered(self, n: usize) -> BufferUnordered<Self> where Self::Item: Future, Self: Sized` | Runs up to `n` at once, yields outputs in completion order. |
| `forward` | `fn forward<S>(self, sink: S) -> Forward<Self, S> where S: Sink<Self::Ok, Error = Self::Error>, Self: Sized + TryStream` | Drives all items into a `Sink` (the stream is a `TryStream`). |
| `split` | `fn split<Item>(self) -> (SplitSink<Self, Item>, SplitStream<Self>) where Self: Sized + Sink<Item>` | Splits a `Stream + Sink` into independently owned halves. |

### Trait-object and termination helpers

| Method | Signature | What it does |
|--------|-----------|--------------|
| `boxed` | `fn boxed<'a>(self) -> BoxStream<'a, Self::Item> where Self: Sized + Send + 'a` | Erase to `Pin<Box<dyn Stream + Send>>`. |
| `boxed_local` | `fn boxed_local<'a>(self) -> LocalBoxStream<'a, Self::Item> where Self: Sized + 'a` | Erase to `Pin<Box<dyn Stream>>` (no `Send`). |
| `fuse` | `fn fuse(self) -> Fuse<Self> where Self: Sized` | Wrap so polling after `None` stays `None`; yields a `FusedStream`. |
| `select_next_some` | `fn select_next_some(&mut self) -> SelectNextSome<'_, Self> where Self: Unpin + FusedStream` | Future for the next `Some`, skipping the terminated case; pairs with `select!`. |

## Concurrency

This is the most error-prone corner. All of these run multiple futures at once;
they differ in ordering and in who owns the futures.

- `buffered(n)` — input is a stream of futures. Runs up to `n` concurrently and
  yields results in the original stream order. A slow future at the head blocks
  later results from being emitted (head-of-line blocking), even if they finished.
- `buffer_unordered(n)` — same `n`-at-a-time concurrency, but yields each result
  the moment it completes, in completion order. No head-of-line blocking. Reach
  for this when order does not matter.
- `for_each_concurrent(limit, f)` — does not collect results; runs the per-item
  async closure with up to `limit` in flight (`None` = unbounded). Use it for
  side-effecting work where you only care that everything ran.
- `FuturesUnordered` — the manual version. You `push` futures yourself and poll
  the set as a stream; it yields outputs as they complete (like
  `buffer_unordered`) but with no fixed concurrency cap and no backing stream.
  Use it when futures are created dynamically rather than driven from one source
  stream.

Rule of thumb: `buffer_unordered(n)` / `for_each_concurrent(Some(n), f)` bound how
many futures run at once when the work comes from a stream; `FuturesUnordered` is
what you build by hand when it doesn't. Pick `buffered` only when downstream
consumers truly need original order.

```rust
use futures::stream::{self, StreamExt};

// Ordered: results come out in input order, max 4 in flight.
let ordered: Vec<_> = stream::iter(urls.clone())
    .map(|u| fetch(u))
    .buffered(4)
    .collect()
    .await;

// As-completed: same concurrency, results in finish order.
let as_done: Vec<_> = stream::iter(urls.clone())
    .map(|u| fetch(u))
    .buffer_unordered(4)
    .collect()
    .await;

// Side effects only, bounded concurrency.
stream::iter(urls)
    .for_each_concurrent(4, |u| async move { let _ = fetch(u).await; })
    .await;
```

## TryStreamExt adapters

For `Stream<Item = Result<T, E>>` (a `TryStream`). Import
`use futures::stream::TryStreamExt;`. `Self::Ok` is `T`, `Self::Error` is `E`.
Most adapters short-circuit: the first `Err` stops the stream and is propagated.

| Method | Signature | What it does |
|--------|-----------|--------------|
| `try_next` | `fn try_next(&mut self) -> TryNext<'_, Self> where Self: Unpin` | Future resolving to `Result<Option<Ok>, Error>`. |
| `try_for_each` | `fn try_for_each<Fut, F>(self, f: F) -> TryForEach<Self, Fut, F> where F: FnMut(Self::Ok) -> Fut, Fut: TryFuture<Ok = (), Error = Self::Error>, Self: Sized` | Async closure per `Ok`, stops on first error. |
| `try_for_each_concurrent` | `fn try_for_each_concurrent<Fut, F>(self, limit: impl Into<Option<usize>>, f: F) -> TryForEachConcurrent<Self, Fut, F> where F: FnMut(Self::Ok) -> Fut, Fut: Future<Output = Result<(), Self::Error>>, Self: Sized` | Concurrent (up to `limit`), errors propagate immediately. |
| `try_collect` | `fn try_collect<C>(self) -> TryCollect<Self, C> where C: Default + Extend<Self::Ok>, Self: Sized` | Collects `Ok`s into `C`; resolves to `Result<C, Error>`. |
| `try_concat` | `fn try_concat(self) -> TryConcat<Self> where Self: Sized, Self::Ok: Extend<<Self::Ok as IntoIterator>::Item> + IntoIterator + Default` | Concatenates all `Ok` items, short-circuiting on error. |
| `try_filter` | `fn try_filter<Fut, F>(self, f: F) -> TryFilter<Self, Fut, F> where Fut: Future<Output = bool>, F: FnMut(&Self::Ok) -> Fut, Self: Sized` | Async predicate over `Ok`; errors pass through. |
| `try_filter_map` | `fn try_filter_map<Fut, F, T>(self, f: F) -> TryFilterMap<Self, Fut, F> where Fut: TryFuture<Ok = Option<T>, Error = Self::Error>, F: FnMut(Self::Ok) -> Fut, Self: Sized` | Filter + map returning `Result<Option<T>, E>`. |
| `try_flatten` | `fn try_flatten(self) -> TryFlatten<Self> where Self::Ok: TryStream, <Self::Ok as TryStream>::Error: From<Self::Error>, Self: Sized` | Flattens a try-stream of try-streams. |
| `try_fold` | `fn try_fold<T, Fut, F>(self, init: T, f: F) -> TryFold<Self, Fut, T, F> where F: FnMut(T, Self::Ok) -> Fut, Fut: TryFuture<Ok = T, Error = Self::Error>, Self: Sized` | Fallible async accumulate. |
| `try_skip_while` | `fn try_skip_while<Fut, F>(self, f: F) -> TrySkipWhile<Self, Fut, F> where F: FnMut(&Self::Ok) -> Fut, Fut: TryFuture<Ok = bool, Error = Self::Error>, Self: Sized` | Skip `Ok`s while predicate is `true`. |
| `try_take_while` | `fn try_take_while<Fut, F>(self, f: F) -> TryTakeWhile<Self, Fut, F> where F: FnMut(&Self::Ok) -> Fut, Fut: TryFuture<Ok = bool, Error = Self::Error>, Self: Sized` | Take `Ok`s while predicate is `true`. |
| `map_ok` | `fn map_ok<T, F>(self, f: F) -> MapOk<Self, F> where Self: Sized, F: FnMut(Self::Ok) -> T` | Map the success value. |
| `map_err` | `fn map_err<E, F>(self, f: F) -> MapErr<Self, F> where Self: Sized, F: FnMut(Self::Error) -> E` | Map the error value. |
| `and_then` | `fn and_then<Fut, F>(self, f: F) -> AndThen<Self, Fut, F> where F: FnMut(Self::Ok) -> Fut, Fut: TryFuture<Error = Self::Error>, Self: Sized` | Async fallible step on each `Ok`. |
| `or_else` | `fn or_else<Fut, F>(self, f: F) -> OrElse<Self, Fut, F> where F: FnMut(Self::Error) -> Fut, Fut: TryFuture<Ok = Self::Ok>, Self: Sized` | Async recovery on each `Err`. |
| `inspect_ok` | `fn inspect_ok<F>(self, f: F) -> InspectOk<Self, F> where F: FnMut(&Self::Ok), Self: Sized` | Peek at `Ok`, pass through. |
| `inspect_err` | `fn inspect_err<F>(self, f: F) -> InspectErr<Self, F> where F: FnMut(&Self::Error), Self: Sized` | Peek at `Err`, pass through. |
| `into_stream` | `fn into_stream(self) -> IntoStream<Self> where Self: Sized` | View a `TryStream` as a plain `Stream<Item = Result<..>>`. |
| `try_buffered` | `fn try_buffered(self, n: usize) -> TryBuffered<Self> where Self::Ok: TryFuture<Error = Self::Error>, Self: Sized` | `Ok`s are futures; run up to `n`, yield in order, stop on error. |
| `try_buffer_unordered` | `fn try_buffer_unordered(self, n: usize) -> TryBufferUnordered<Self> where Self::Ok: TryFuture<Error = Self::Error>, Self: Sized` | Same but yields in completion order. |
| `try_chunks` | `fn try_chunks(self, capacity: usize) -> TryChunks<Self> where Self: Sized` | Chunks `Ok`s into `Vec`s of up to `capacity`. |

## FuturesUnordered

`futures::stream::FuturesUnordered<Fut>` — an unbounded set of futures driven
concurrently with no executor or runtime of its own. It implements `Stream`,
yielding `<Fut as Future>::Output` in completion order (not push order). Futures
are polled only when they signal a wake-up, so a large idle set is cheap.

```rust
impl<Fut: Future> FuturesUnordered<Fut> {
    pub fn new() -> FuturesUnordered<Fut>;
    pub fn push(&self, future: Fut);          // note: &self, not &mut self
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn iter(&self) -> Iter<'_, Fut> where Fut: Unpin;
    pub fn iter_mut(&mut self) -> IterMut<'_, Fut> where Fut: Unpin;
    pub fn clear(&mut self);
}
```

`push` takes `&self`, so you can add while holding a shared reference. Also
implements `FromIterator<Fut>` and `Extend<Fut>` (build one with
`.collect()` / `extend`). The Stream impl returns `Ready(None)` when empty.

Common pattern — drive a dynamic set to completion by polling it as a stream:

```rust
use futures::stream::{FuturesUnordered, StreamExt};

let mut tasks = FuturesUnordered::new();
for id in ids {
    tasks.push(async move { work(id).await });
}
while let Some(result) = tasks.next().await {
    handle(result); // arrives as each future finishes
}
```

## FuturesOrdered

`futures::stream::FuturesOrdered<Fut>` — same concurrent driving as
`FuturesUnordered`, but results come out strictly in the order the futures were
pushed (FIFO), regardless of which finishes first. Implements `Stream` and
`FusedStream`. A future that completes early is held until all earlier ones have
been yielded.

```rust
impl<Fut: Future> FuturesOrdered<Fut> {
    pub fn new() -> FuturesOrdered<Fut>;
    pub fn push_back(&mut self, future: Fut);   // add to the tail (was `push`)
    pub fn push_front(&mut self, future: Fut);  // add to the head
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

Also `FromIterator<Fut>` and `Extend<Fut>`. Use it when you need concurrency but
must preserve submission order in the output — the manual analogue of `buffered`.

## select

### SelectAll and select_all

`futures::stream::SelectAll<St>` — an unbounded set of *streams* polled together,
yielding items as any of them becomes ready. It does not poll a pushed stream
until `SelectAll` itself is polled.

```rust
impl<St: Stream + Unpin> SelectAll<St> {
    pub fn new() -> SelectAll<St>;   // empty: poll returns Ready(None)
    pub fn push(&mut self, stream: St);
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn iter(&self) -> Iter<'_, St>;
}

// Free function:
pub fn select_all<I>(streams: I) -> SelectAll<I::Item>
where
    I: IntoIterator,
    I::Item: Stream + Unpin;
```

Implements `FromIterator` and `Extend`. Use `select_all(iter_of_streams)` to
merge a runtime-sized collection of same-`Item` streams into one.

### stream_select!

`futures::stream_select!` — macro that merges a statically known number of
streams (all yielding the same `Item`) into one stream, without boxing. Unlike
`select_all` it keeps each stream inline, so the streams may be different
concrete types. Requires the `std` and `async-await` features. When several are
ready at once, one is chosen pseudo-randomly.

```rust
use futures::{stream, StreamExt, stream_select};

let s1 = stream::iter(vec![1i32]).fuse();
let s2 = stream::iter(vec![2i32]).fuse();
let mut combined = stream_select!(s1, s2);
while let Some(v) = combined.next().await { /* 1 or 2 */ }
```

Use `stream_select!` for a fixed set of differently-typed streams; use
`select_all` for a dynamic, homogeneous collection.

## Constructors

Free functions in `futures::stream` that build a stream from scratch.

| Function | Signature | When to use |
|----------|-----------|-------------|
| `iter` | `pub fn iter<I>(i: I) -> Iter<I::IntoIter> where I: IntoIterator` | Lift an existing iterator into a stream (always ready). |
| `once` | `pub fn once<Fut>(future: Fut) -> Once<Fut> where Fut: Future` | A one-item stream from a single future. |
| `repeat` | `pub fn repeat<T: Clone>(item: T) -> Repeat<T>` | Infinite stream of the same clone. Pair with `take`. |
| `repeat_with` | `pub fn repeat_with<A, F>(repeater: F) -> RepeatWith<F> where F: FnMut() -> A` | Infinite stream, fresh value each time (no `Clone` needed). |
| `empty` | `pub fn empty<T>() -> Empty<T>` | Yields nothing; always `Ready(None)`. |
| `pending` | `pub fn pending<T>() -> Pending<T>` | Never yields; always `Pending`. Never completes. |
| `poll_fn` | `pub fn poll_fn<T, F>(f: F) -> PollFn<F> where F: FnMut(&mut Context<'_>) -> Poll<Option<T>>` | Hand-write `poll_next` as a closure. |
| `unfold` | `pub fn unfold<T, F, Fut, Item>(init: T, f: F) -> Unfold<T, F, Fut> where F: FnMut(T) -> Fut, Fut: Future<Output = Option<(Item, T)>>` | Build a stream from a seed + async closure. The go-to for custom async streams. |
| `try_unfold` | `pub fn try_unfold<T, F, Fut, Item>(init: T, f: F) -> TryUnfold<T, F, Fut> where F: FnMut(T) -> Fut, Fut: TryFuture<Ok = Option<(Item, T)>>` | Like `unfold` but the closure can fail; produces a `TryStream`. |

`unfold` is the inverse of `fold`: the closure takes the current state, awaits,
and returns `Some((item, next_state))` to emit and continue, or `None` to end.

```rust
use futures::stream::{self, StreamExt};

// Custom stream from state + async step.
let s = stream::unfold(0u32, |state| async move {
    if state < 3 {
        Some((state * 2, state + 1)) // emit `state*2`, advance state
    } else {
        None                         // terminate
    }
});
assert_eq!(s.collect::<Vec<_>>().await, vec![0, 2, 4]);
```

## Gotchas

- `next()` needs `StreamExt` in scope and the stream to be `Unpin`. If it is not
  `Unpin`, pin it first: `futures::pin_mut!(stream);` or
  `let mut stream = Box::pin(stream);`, then `stream.next().await`.
- `select_next_some()` requires `Self: Unpin + FusedStream`. Call `.fuse()` (or
  use `FuturesUnordered`/`SelectAll`, which are already fused) before using it,
  typically inside a `select!` loop.
- `chunks(0)` / `ready_chunks(0)` panic — capacity must be non-zero.
- Streams are lazy: a combinator chain does nothing until the final future is
  awaited (via `next`, `collect`, `for_each`, etc.). Building the chain has no
  effect on its own.
- `buffered`/`buffer_unordered` need `Self::Item: Future`; the upstream stream
  must yield futures, e.g. `stream.map(|x| async move { ... }).buffered(n)`.
- `FuturesUnordered::push` takes `&self`; `FuturesOrdered` uses `push_back` /
  `push_front` and takes `&mut self`.
- `repeat`, `repeat_with`, `pending`, and `iter` over an infinite iterator never
  terminate — bound them with `take`/`take_while` before a terminal like
  `collect`, or you will loop or OOM.
