# futures::future API reference (futures 0.3.32)

The `futures::future` module plus the future/poll macros from the crate root.
Covers the `FutureExt` and `TryFutureExt` adapter traits, the free constructors and
combinators (`ready`, `join`, `select`, `abortable`, ...), the concrete future types
they produce, and the `join!` / `try_join!` / `select!` / `poll!` macro family. All
signatures below were taken from the docs.rs 0.3.32 rendered pages; anything not
confirmed there is marked `(verify)`.

## Contents

- [Feature gates](#feature-gates)
- [FutureExt](#futureext)
- [TryFutureExt](#tryfutureext)
- [Free functions and constructors](#free-functions-and-constructors)
- [Types](#types)
  - [BoxFuture / LocalBoxFuture](#boxfuture--localboxfuture)
  - [Either](#either)
  - [Shared / WeakShared](#shared--weakshared)
  - [Abortable / AbortHandle / AbortRegistration / Aborted](#abortable--aborthandle--abortregistration--aborted)
  - [RemoteHandle / Remote](#remotehandle--remote)
  - [MaybeDone / TryMaybeDone](#maybedone--trymaybedone)
  - [OptionFuture](#optionfuture)
  - [Constructor future types](#constructor-future-types)
- [Macros](#macros)
  - [join! / try_join!](#join--try_join)
  - [select! / select_biased!](#select--select_biased)
  - [poll! / pending! / ready! / pin_mut!](#poll--pending--ready--pin_mut)
- [Gotchas](#gotchas)

## Feature gates

Adapters that allocate or spawn are behind cargo features. With default features
(`std`) they are all on. Relevant gates:

- `catch_unwind` — `std` only.
- `boxed`, `boxed_local`, `BoxFuture`, `LocalBoxFuture` — `alloc` only.
- `shared`, `Shared`, `WeakShared` — `std`, or `alloc` + `spin`.
- `remote_handle`, `RemoteHandle`, `Remote` — `channel` + `std`.
- `join!` / `try_join!` / `select!` / `select_biased!` / `poll!` / `pending!` —
  `async-await` (on by default).

## FutureExt

Extension trait blanket-implemented for every `Future`. All adapters take `self` by
value and require `Self: Sized` (poll helpers take `&mut self`).

| method | signature | what it does |
| --- | --- | --- |
| `map` | `fn map<U, F>(self, f: F) -> Map<Self, F> where F: FnOnce(Self::Output) -> U` | Map the output through a closure. |
| `map_into` | `fn map_into<U>(self) -> MapInto<Self, U> where Self::Output: Into<U>` | `map` via `Into`. |
| `then` | `fn then<Fut, F>(self, f: F) -> Then<Self, Fut, F> where F: FnOnce(Self::Output) -> Fut, Fut: Future` | Chain a second future built from the output. |
| `left_future` | `fn left_future<B>(self) -> Either<Self, B> where B: Future<Output = Self::Output>` | Wrap as the `Left` arm of an `Either`. |
| `right_future` | `fn right_future<A>(self) -> Either<A, Self> where A: Future<Output = Self::Output>` | Wrap as the `Right` arm of an `Either`. |
| `into_stream` | `fn into_stream(self) -> IntoStream<Self>` | Single-item stream yielding the output. |
| `flatten` | `fn flatten(self) -> Flatten<Self> where Self::Output: Future` | Await the future, then await its future output. |
| `flatten_stream` | `fn flatten_stream(self) -> FlattenStream<Self> where Self::Output: Stream` | Await the future, then drive the stream it yields. |
| `fuse` | `fn fuse(self) -> Fuse<Self>` | Make it `FusedFuture`; polling after completion returns `Pending` forever. Required by `select!`. |
| `inspect` | `fn inspect<F>(self, f: F) -> Inspect<Self, F> where F: FnOnce(&Self::Output)` | Peek at the output without consuming it. |
| `catch_unwind` | `fn catch_unwind(self) -> CatchUnwind<Self> where Self: Sized + UnwindSafe` | Catch a panic; output becomes `Result<Output, Box<dyn Any + Send>>`. `std` only. |
| `shared` | `fn shared(self) -> Shared<Self> where Self::Output: Clone` | Cloneable, multi-await handle to one future. `std` (or `alloc`+`spin`). |
| `remote_handle` | `fn remote_handle(self) -> (Remote<Self>, RemoteHandle<Self::Output>)` | Split into a drivable `Remote` and a handle that awaits its output. `channel`+`std`. |
| `boxed` | `fn boxed<'a>(self) -> Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>> where Self: Sized + Send + 'a` | Type-erase into a `BoxFuture`. `alloc` only. |
| `boxed_local` | `fn boxed_local<'a>(self) -> Pin<Box<dyn Future<Output = Self::Output> + 'a>> where Self: Sized + 'a` | Type-erase into a `LocalBoxFuture` (no `Send`). `alloc` only. |
| `unit_error` | `fn unit_error(self) -> UnitError<Self>` | Wrap output into `Ok(output)` with `Err = ()`. |
| `never_error` | `fn never_error(self) -> NeverError<Self>` | Wrap output into `Ok(output)` with `Err = Never`. |
| `poll_unpin` | `fn poll_unpin(&mut self, cx: &mut Context<'_>) -> Poll<Self::Output> where Self: Unpin` | Poll an `Unpin` future without pinning by hand. |
| `now_or_never` | `fn now_or_never(self) -> Option<Self::Output>` | Poll once; `Some(out)` if ready now, else `None`. Drops the future when not ready. |

## TryFutureExt

Adapters for futures whose output is `Result<Ok, Error>` (the `TryFuture` trait). All
take `self` by value, `Self: Sized`.

| method | signature | what it does |
| --- | --- | --- |
| `map_ok` | `fn map_ok<T, F>(self, f: F) -> MapOk<Self, F> where F: FnOnce(Self::Ok) -> T` | Map the success value; leave errors alone. |
| `map_ok_or_else` | `fn map_ok_or_else<T, E, F>(self, e: E, f: F) -> MapOkOrElse<Self, F, E> where F: FnOnce(Self::Ok) -> T, E: FnOnce(Self::Error) -> T` | Map both arms to a common `T`; output is `T` (not `Result`). |
| `map_err` | `fn map_err<E, F>(self, f: F) -> MapErr<Self, F> where F: FnOnce(Self::Error) -> E` | Map the error value; leave success alone. |
| `err_into` | `fn err_into<E>(self) -> ErrInto<Self, E> where Self::Error: Into<E>` | Convert the error via `Into`. |
| `ok_into` | `fn ok_into<U>(self) -> OkInto<Self, U> where Self::Ok: Into<U>` | Convert the success via `Into`. |
| `and_then` | `fn and_then<Fut, F>(self, f: F) -> AndThen<Self, Fut, F> where F: FnOnce(Self::Ok) -> Fut, Fut: TryFuture<Error = Self::Error>` | On success, run another fallible future; short-circuits on error. |
| `or_else` | `fn or_else<Fut, F>(self, f: F) -> OrElse<Self, Fut, F> where F: FnOnce(Self::Error) -> Fut, Fut: TryFuture<Ok = Self::Ok>` | On error, run a recovery future; passes success through. |
| `unwrap_or_else` | `fn unwrap_or_else<F>(self, f: F) -> UnwrapOrElse<Self, F> where F: FnOnce(Self::Error) -> Self::Ok` | Collapse to `Self::Ok`, computing a fallback from the error. |
| `inspect_ok` | `fn inspect_ok<F>(self, f: F) -> InspectOk<Self, F> where F: FnOnce(&Self::Ok)` | Peek at the success value. |
| `inspect_err` | `fn inspect_err<F>(self, f: F) -> InspectErr<Self, F> where F: FnOnce(&Self::Error)` | Peek at the error value. |
| `try_flatten` | `fn try_flatten(self) -> TryFlatten<Self, Self::Ok> where Self::Ok: TryFuture<Error = Self::Error>` | Flatten a success that is itself a `TryFuture`. |
| `try_flatten_stream` | `fn try_flatten_stream(self) -> TryFlattenStream<Self> where Self::Ok: TryStream<Error = Self::Error>` | Flatten a success that is a `TryStream`. |
| `flatten_sink` | `fn flatten_sink<Item>(self) -> FlattenSink<Self, Self::Ok> where Self::Ok: Sink<Item, Error = Self::Error>` | Flatten a success that is a `Sink`. |
| `into_future` | `fn into_future(self) -> IntoFuture<Self>` | Adapt a `TryFuture` into a plain `Future<Output = Result<..>>`. |
| `try_poll_unpin` | `fn try_poll_unpin(&mut self, cx: &mut Context<'_>) -> Poll<Result<Self::Ok, Self::Error>> where Self: Unpin` | Poll an `Unpin` `TryFuture` without pinning by hand. |

## Free functions and constructors

| function | signature | what it does |
| --- | --- | --- |
| `ready` | `fn ready<T>(t: T) -> Ready<T>` | Future that is immediately ready with `t`. |
| `ok` | `fn ok<T, E>(t: T) -> Ready<Result<T, E>>` | Immediately ready `Ok(t)`. (verify exact form) |
| `err` | `fn err<T, E>(err: E) -> Ready<Result<T, E>>` | Immediately ready `Err(err)`. (verify exact form) |
| `pending` | `fn pending<T>() -> Pending<T>` | Future that never resolves. |
| `poll_fn` | `fn poll_fn<T, F>(f: F) -> PollFn<F> where F: FnMut(&mut Context<'_>) -> Poll<T>` | Build a future from a `poll` closure. |
| `poll_immediate` | `fn poll_immediate<F: Future>(f: F) -> PollImmediate<F>` | Wraps a future so polling yields `Option<F::Output>` (ready now or `None`). |
| `lazy` | `fn lazy<F, R>(f: F) -> Lazy<F> where F: FnOnce(&mut Context<'_>) -> R` | Run the closure the first time it is polled. |
| `always_ready` | `fn always_ready<T, F: Fn() -> T>(prod: F) -> AlwaysReady<T, F>` | Always-ready future that recomputes its value each poll. (verify) |
| `maybe_done` | `fn maybe_done<Fut: Future>(future: Fut) -> MaybeDone<Fut>` | Wrap a future so it can be polled to completion and the output read out later. |
| `try_maybe_done` | `fn try_maybe_done<Fut: TryFuture>(future: Fut) -> TryMaybeDone<Fut>` | `maybe_done` for `TryFuture`; short-circuits on error. |
| `join` | `fn join<A, B>(a: A, b: B) -> Join<A, B> where A: Future, B: Future` | Run two futures concurrently; output `(A::Output, B::Output)`. |
| `join3`/`join4`/`join5` | same shape, 3/4/5 futures | Tuple of all outputs. |
| `join_all` | `fn join_all<I>(iter: I) -> JoinAll<I::Item> where I: IntoIterator, I::Item: Future` | Output `Vec<Output>` in input order. For many futures prefer `FuturesOrdered`/`FuturesUnordered`. |
| `try_join` | `fn try_join<A, B>(a: A, b: B) -> TryJoin<A, B>` where both are `TryFuture` with the same `Error` | Output `Result<(A::Ok, B::Ok), Error>`; completes early on first error. |
| `try_join3`/`try_join4`/`try_join5` | same shape | As above for 3/4/5 futures. |
| `try_join_all` | `fn try_join_all<I>(iter: I) -> TryJoinAll<I::Item> where I: IntoIterator, I::Item: TryFuture` | Output `Result<Vec<Ok>, Error>`; first error short-circuits. |
| `select` | `fn select<A, B>(a: A, b: B) -> Select<A, B> where A: Future + Unpin, B: Future + Unpin` | First of two to finish; output `Either<(A::Output, B), (B::Output, A)>` (winner plus the loser future). |
| `select_all` | `fn select_all<I>(iter: I) -> SelectAll<I::Item> where I: IntoIterator, I::Item: Future + Unpin` | Output `(Item::Output, usize, Vec<Item>)` — winner output, its index, the remaining futures. |
| `select_ok` | `fn select_ok<I>(iter: I) -> SelectOk<I::Item> where I: IntoIterator, I::Item: TryFuture + Unpin` | First success; output `Result<(Ok, Vec<remaining>), Error>`. Errors are skipped until one succeeds or all fail. (verify exact output tuple) |
| `try_select` | `fn try_select<A, B>(a: A, b: B) -> TrySelect<A, B> where A: TryFuture + Unpin, B: TryFuture + Unpin` | Like `select` but resolves as soon as either finishes; output is `Result<Either<..>, Either<..>>` carrying winner + loser. (verify exact output) |
| `abortable` | `fn abortable<Fut>(future: Fut) -> (Abortable<Fut>, AbortHandle) where Fut: Future` | Wrap a future so an `AbortHandle` can cancel it. Output of the `Abortable` is `Result<Fut::Output, Aborted>`. (verify signature) |

## Types

### BoxFuture / LocalBoxFuture

Type aliases for owned, type-erased futures (the return types of `boxed` / `boxed_local`).
`alloc` feature.

```rust
type BoxFuture<'a, T>      = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
type LocalBoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;
```

Use `BoxFuture` to store heterogeneous futures (for example `Vec<BoxFuture<'static, T>>`)
or to name an `async fn`'s return type in a trait. `LocalBoxFuture` drops the `Send` bound.

### Either

```rust
pub enum Either<A, B> {
    Left(A),
    Right(B),
}
```

Combines two different futures/streams/sinks that share the same associated types into
one type. Implements `Future`/`Stream`/`Sink`/`AsyncRead`/`AsyncWrite`/`AsyncSeek`/
`AsyncBufRead` when both arms do. Produced by `FutureExt::left_future` / `right_future`,
and is the output type of `select`/`try_select`.

Methods:

| method | signature | what it does |
| --- | --- | --- |
| `as_pin_ref` | `fn as_pin_ref(self: Pin<&Either<A, B>>) -> Either<Pin<&A>, Pin<&B>>` | Project a pinned shared ref into each arm. |
| `as_pin_mut` | `fn as_pin_mut(self: Pin<&mut Either<A, B>>) -> Either<Pin<&mut A>, Pin<&mut B>>` | Project a pinned mut ref into each arm. |
| `factor_first` | `fn factor_first(self) -> (T, Either<A, B>)` on `Either<(T, A), (T, B)>` | Pull a shared first tuple element out. |
| `factor_second` | `fn factor_second(self) -> (Either<A, B>, T)` on `Either<(A, T), (B, T)>` | Pull a shared second tuple element out. |
| `into_inner` | `fn into_inner(self) -> T` on `Either<T, T>` | Collapse a homogeneous `Either` to its value. |

### Shared / WeakShared

`Shared<Fut>` is a `Clone` future: every clone awaits the same underlying future and each
gets a clone of the output, so `Fut::Output: Clone` is required. Produced by
`FutureExt::shared`. `std` (or `alloc` + `spin`).

| method | signature | what it does |
| --- | --- | --- |
| (impl) | `impl<Fut> Clone for Shared<Fut>` | Cloning gives another awaiter of the same future. |
| `peek` | `fn peek(&self) -> Option<&Fut::Output>` | The completed output, if the future has already resolved. (verify) |
| `downgrade` | `fn downgrade(&self) -> Option<WeakShared<Fut>>` | Weak handle that does not keep the shared state alive. (verify) |
| `strong_count` / `weak_count` | `fn strong_count(&self) -> Option<usize>` / `fn weak_count(&self) -> Option<usize>` | Reference counts on the shared state. (verify) |

`WeakShared<Fut>` upgrades back to a `Shared` like an `Arc` weak ref via
`fn upgrade(&self) -> Option<Shared<Fut>>`. (verify)

### Abortable / AbortHandle / AbortRegistration / Aborted

A future (or stream) that an external handle can short-circuit. Output of an
`Abortable` future is `Result<Fut::Output, Aborted>`.

```rust
// AbortHandle
fn new_pair() -> (AbortHandle, AbortRegistration)  // make a handle + its registration
fn abort(&self)                                     // request cancellation
fn is_aborted(&self) -> bool                        // true once abort() was called

// Abortable
fn new(future: Fut, reg: AbortRegistration) -> Abortable<Fut>  // (verify)
fn is_aborted(&self) -> bool                                   // (verify)
```

`AbortRegistration` is the half passed to `Abortable::new`. `Aborted` is a unit error
struct returned in the `Err` arm when the task was aborted. `abort()` does not interrupt a
poll already running on another thread — it takes effect at the next poll.

Two construction paths:

```rust
// One-shot: future + handle in one call.
let (fut, handle) = future::abortable(some_future);

// Manual: useful to share one registration across an Abortable future and stream.
let (handle, reg) = AbortHandle::new_pair();
let fut = Abortable::new(some_future, reg);
handle.abort();
```

### RemoteHandle / Remote

`remote_handle()` splits a future into:

- `Remote<Fut>` — the future that actually does the work; must be polled/spawned by an
  executor or its output never arrives.
- `RemoteHandle<Fut::Output>` — a future that resolves to that output. Dropping the handle
  cancels the remote; call `RemoteHandle::forget` to let the remote run detached. (verify
  `forget`). `channel` + `std` feature.

### MaybeDone / TryMaybeDone

```rust
pub enum MaybeDone<Fut: Future> {
    Future(Fut),       // not yet polled to completion
    Done(Fut::Output), // finished, output stored
    Gone,              // output already taken
}
```

`take_output(self: Pin<&mut Self>) -> Option<Fut::Output>` pulls the stored output out
(leaving `Gone`). `TryMaybeDone` is the `TryFuture` analogue and short-circuits on error.
Built with `maybe_done` / `try_maybe_done`; used to drive several futures to completion
before collecting results.

### OptionFuture

```rust
pub struct OptionFuture<Fut> { /* wraps Option<Fut> */ }
```

`From<Option<Fut>>`. A future over an optional inner future: output is `Option<Fut::Output>` —
`Some(out)` if the inner future was present, `None` immediately if it was `None`. Handy for
conditionally awaiting in `select!`.

### Constructor future types

Each constructor returns a named future type, useful when you need to store or name it:
`Ready<T>` (`ready`), `Pending<T>` (`pending`), `PollFn<F>` (`poll_fn`), `Lazy<F>` (`lazy`),
`PollImmediate<F>` (`poll_immediate`), and the combinator outputs `Map`, `MapInto`, `Then`,
`Flatten`, `FlattenStream`, `IntoStream`, `Fuse`, `Inspect`, `CatchUnwind`, `UnitError`,
`NeverError`, plus the join/try-join/select families (`Join`, `Join3..5`, `JoinAll`,
`TryJoin`, `TryJoin3..5`, `TryJoinAll`, `Select`, `SelectAll`, `SelectOk`, `TrySelect`) and
the try adapters (`MapOk`, `MapErr`, `MapOkOrElse`, `AndThen`, `OrElse`, `UnwrapOrElse`,
`OkInto`, `ErrInto`, `InspectOk`, `InspectErr`, `TryFlatten`, `TryFlattenStream`,
`FlattenSink`, `IntoFuture`). `FutureObj` / `LocalFutureObj` are no-alloc trait objects for
custom executors.

## Macros

### join! / try_join!

`async-await` feature, usable inside `async fn`/closure/block. Both are variadic and poll
all branches concurrently (unlike sequential `.await`). They pin the futures internally, so
the arguments do not need to be `Unpin`.

```rust
use futures::{join, try_join};

// join!: tuple of all outputs once all complete.
let a = async { 1 };
let b = async { 2 };
let c = async { 3 };
assert_eq!(join!(a, b), (1, 2));
assert_eq!(join!(a, b, c), (1, 2, 3)); // variadic

// try_join!: Result<tuple, E>. First Err short-circuits and is returned.
let a = async { Ok::<i32, i32>(1) };
let b = async { Ok::<i32, i32>(2) };
assert_eq!(try_join!(a, b), Ok((1, 2)));

let a = async { Ok::<i32, i32>(1) };
let b = async { Err::<u64, i32>(2) };
assert_eq!(try_join!(a, b), Err(2));
```

`join!(a, b)` is the concurrent form of `(a.await, b.await)`.

### select! / select_biased!

`async-await` feature, usable inside an `async` context. Polls every branch and runs the
arm of whichever future finishes first.

Branch grammar:

```text
select! {
    pattern = future_expr => body,   // bind the output, run the body
    // ...
    complete => body,                // all futures/streams exhausted
    default  => body,                // nothing was ready this poll (no .await happens)
}
```

- `complete =>` runs once every listed future/stream has finished (so the normal arms can
  no longer fire). It takes priority over `default` when both apply.
- `default =>` runs immediately if no branch is ready right now; without it `select!`
  suspends until a branch is ready.
- Requirements: each future must be `Unpin` **and** `FusedFuture`. A named binding passed
  by identifier must satisfy both yourself; an inline expression (for example
  `some_async_fn()`) is pinned automatically by the macro. Use `.fuse()` to satisfy
  `FusedFuture`, and `pin!`/`pin_mut!` to satisfy `Unpin` for a named future.
- Streams work too: pass `stream.next()` (which is fused-friendly via `select_next_some` or
  `.fuse()` on a `StreamExt` stream).

`select_biased!` is identical except polling order: when multiple branches are ready it
always picks the **first one written**, top to bottom. Plain `select!` chooses among ready
branches pseudo-randomly to avoid starvation.

```rust
use futures::{future, select, select_biased};
use futures::future::FutureExt;
use std::pin::pin;

// Named futures: fuse + pin to satisfy FusedFuture + Unpin.
let mut a = pin!(async { 1 }.fuse());
let mut b = pin!(async { 2 }.fuse());
let res = select! {
    x = a => x,
    y = b => y,
    complete => -1,   // both done already
    default  => 0,    // neither ready this poll
};

// select_biased! picks the first ready arm in declaration order.
let mut a = future::ready(4);
let mut b = future::pending::<()>();
let res = select_biased! {
    a_res = a => a_res + 1,
    _ = b => 0,
};
assert_eq!(res, 5);
```

### poll! / pending! / ready! / pin_mut!

```rust
use futures::{poll, pending, ready, pin_mut};
use std::task::Poll;
```

- `poll!(fut)` -> `Poll<T>`: polls a future (or `stream.next()`) exactly once in the current
  `async` context without awaiting. `async-await` feature; usable inside an `async` context.
  The polled value must be `Unpin` or already pinned.
- `pending!()`: yields to the executor once (returns `Poll::Pending` to it) and resumes on the
  next poll. You must ensure the waker is woken elsewhere or progress stalls. `async-await`
  feature; `async` context only.
- `ready!(expr)`: given a `Poll<T>`, evaluates to the inner `T` when `Ready`, otherwise
  `return Poll::Pending` early from the enclosing `poll` fn. Soft-deprecated since Rust 1.64
  in favor of `std::task::ready!`.
- `pin_mut!(a, b, ...)`: pins each named value to the stack, shadowing the binding with
  `Pin<&mut T>`. No heap allocation. Soft-deprecated since Rust 1.68 in favor of the std
  `pin!` macro (which returns a value rather than shadowing).

```rust
// ready! in a manual poll impl:
let value = ready!(inner.poll_unpin(cx)); // returns Poll::Pending early if not ready

// pin_mut! to make a named future select!-able:
let fut = async { 1 }.fuse();
pin_mut!(fut);                 // fut is now Pin<&mut impl FusedFuture>
let _: Poll<i32> = poll!(&mut fut);
```

## Gotchas

- `select!` / `select_biased!` require every branch future to be **both `Unpin` and
  `FusedFuture`**. Fix with `.fuse()` (for `FusedFuture`) and `pin!`/`pin_mut!` (for
  `Unpin`); inline expressions are auto-pinned, named bindings are not.
- `Shared` requires `Self::Output: Clone` — each awaiter gets a clone of the output.
- `boxed` / `boxed_local` need the `alloc` feature (on with default `std`); they return
  `Pin<Box<dyn Future ...>>` aliased as `BoxFuture` / `LocalBoxFuture`.
- `now_or_never` returns `Option<Output>` and **drops** the future if it is not ready —
  do not call it on a future you still need to drive.
- `catch_unwind` needs the `std` feature and `Self: UnwindSafe`; the output becomes a
  `Result` whose `Err` is the panic payload.
- `remote_handle`'s `Remote` half does nothing unless an executor polls it; dropping the
  `RemoteHandle` cancels the work unless you call `forget`.
- `join!` polls concurrently — it is not the same as awaiting in sequence, which matters for
  side-effect ordering.
- `try_join!` / `try_join` / `try_join_all` short-circuit on the **first** error; remaining
  futures are dropped.
- `ready!` and `pin_mut!` are soft-deprecated in favor of the std `ready!` / `pin!` macros;
  prefer std in new code.
