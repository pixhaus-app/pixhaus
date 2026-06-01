# tokio_util::sync — cancellation and poll-based primitives

The `sync` module is **always available** — there is no feature flag. A bare
`tokio-util = "0.7"` gives you everything here. tokio-util 0.7.18.

Public items: `CancellationToken`, `DropGuard`, `DropGuardRef`,
`WaitForCancellationFuture`, `WaitForCancellationFutureOwned`, `PollSender`,
`PollSendError`, `PollSemaphore`, `ReusableBoxFuture`. No traits, no free
functions.

## Table of contents

- `CancellationToken` — the headline type
- `DropGuard` / `DropGuardRef`
- `WaitForCancellationFuture` (the future kinds)
- `PollSender` — drive an mpsc `Sender` from a poll/`Sink` context
- `PollSemaphore`
- `ReusableBoxFuture`

---

## CancellationToken

> A token to signal a cancellation request to one or more tasks.

```rust
pub struct CancellationToken { /* private */ }
```

Cheaply `Clone` (clones share one underlying state), `Default` (== `new()`),
`Send + Sync + Unpin`. Tokens form a **tree**: cancelling a token cancels it and
all descendants; a child cancelling never affects its parent.

```rust
pub fn new() -> CancellationToken

// Cancel this token + all children. Wakes every task awaiting cancelled().
// NOTE: not atomic across the tree — children are visited one at a time, so
// is_cancelled() may briefly disagree between related tokens mid-cancel.
pub fn cancel(&self)

// Future that completes when cancellation is requested (immediately if already
// cancelled). CANCEL SAFE — safe to drop unpolled, safe in a select! branch.
pub fn cancelled(&self) -> WaitForCancellationFuture<'_>

// Same, but consumes self and returns a 'static future owning the token —
// for spawning a task that only waits on cancellation. Also cancel safe.
pub fn cancelled_owned(self) -> WaitForCancellationFutureOwned

pub fn is_cancelled(&self) -> bool

// Independently-cancellable child scope. Parent (or any ancestor) cancel flows
// down; child cancel does NOT flow up. If parent is already cancelled, the
// returned child is already cancelled.
pub fn child_token(&self) -> CancellationToken

// Consume into a DropGuard that cancels on drop unless disarmed.
pub fn drop_guard(self) -> DropGuard
// Borrowing variant — same drop-cancel, borrows instead of consuming.
pub fn drop_guard_ref(&self) -> DropGuardRef<'_>

// Run `fut`, racing it against cancellation.
//   Some(output) — fut finished first
//   None         — cancelled first (fut dropped at its next suspension point)
pub async fn run_until_cancelled<F: Future>(&self, fut: F) -> Option<F::Output>
pub async fn run_until_cancelled_owned<F: Future>(self, fut: F) -> Option<F::Output>
```

Key facts:

- `.clone()` shares the *same* token. To cancel one job without touching the
  rest, hand it a `.child_token()`.
- `cancelled()`/`cancelled_owned()` are futures, not blocking calls — `await`
  them on a tokio task or drop them in `select!`. They're cancel safe.
- `cancel()` is not atomic across the tree; don't rely on every related token
  flipping in the same instant.

### Idiomatic usage

```rust
use tokio_util::sync::CancellationToken;

let token = CancellationToken::new();

// Give a worker its own cancellable scope.
let child = token.child_token();
let handle = tokio::spawn(async move {
    // run_until_cancelled: None means we were cancelled.
    child
        .run_until_cancelled(async {
            loop {
                do_one_unit_of_work().await;
            }
        })
        .await // -> Option<()>, here always None (the loop never returns)
});

// Elsewhere, race other work against cancellation explicitly.
tokio::select! {
    _ = token.cancelled() => { /* shutdown requested */ }
    _ = some_other_future() => { /* finished first */ }
}

token.cancel();            // cancels token + child
let _ = handle.await;
```

Pixhaus use: one token (or a child off a root) per cancellable job — AI
generation, export, a slow file decode. Cancel it when the user hits Esc or
starts a newer request. Pair with the channel-drain loop in `pixhaus-egui` so
the cancel is requested from the UI but awaited on the task.

---

## DropGuard / DropGuardRef

> A wrapper for a `CancellationToken` that cancels it on drop.

```rust
pub struct DropGuard { /* private */ }       // owns the token
pub struct DropGuardRef<'a> { /* private */ } // borrows the token

// On DropGuard:
pub fn disarm(self) -> CancellationToken   // consume WITHOUT cancelling; return the token
```

On drop (any exit path — `return`, `?`, panic) the guard calls `cancel()` on its
token, cancelling the whole subtree. That's the point: "cancel everything if this
scope exits unexpectedly." If the scope completed successfully and you want the
token to stay live, `disarm()` before returning. Created via
`CancellationToken::drop_guard` / `drop_guard_ref`.

---

## WaitForCancellationFuture / WaitForCancellationFutureOwned

```rust
pub struct WaitForCancellationFuture<'a> { /* private */ }   // Future<Output = ()>
pub struct WaitForCancellationFutureOwned { /* private */ }  // Future<Output = ()>, 'static
```

Both resolve to `()` when their token is cancelled. The borrowed form is tied to
the token's lifetime; the owned form holds the token, so it's `'static` and can
move into a spawned task. Returned by `cancelled()` / `cancelled_owned()`. Cancel
safe to drop unpolled.

---

## PollSender\<T\>

> A wrapper around `tokio::sync::mpsc::Sender` that can be polled, implementing
> `Sink<T>`.

```rust
pub struct PollSender<T> { /* private */ }   // Sink<T, Error = PollSendError<T>> + Clone + Debug

pub fn new(sender: Sender<T>) -> Self

// Reserve a slot. Must reach Poll::Ready(Ok(())) BEFORE send_item.
pub fn poll_reserve(&mut self, cx: &mut Context<'_>)
    -> Poll<Result<(), PollSendError<T>>>

// Deposit into the reserved slot. PANICS if there was no successful prior reserve.
pub fn send_item(&mut self, value: T) -> Result<(), PollSendError<T>>

pub fn abort_send(&mut self) -> bool      // release a reserved-but-unused slot; true if one existed
pub fn get_ref(&self) -> Option<&Sender<T>>
pub fn close(&mut self)
pub fn clone(&self) -> PollSender<T>
```

The contract is **reserve, then send**: `poll_reserve` acquires the channel
permit; `send_item` fills it. Calling `send_item` without that panics. As a
`Sink`, `poll_ready` == `poll_reserve` and `start_send` == `send_item`, same
contract. `poll_reserve` returns `Poll::Ready(Err(_))` when the channel is
closed.

When to use it: only when you genuinely need a `Sink` or are writing a manual
`poll_*` adapter (e.g. forwarding a `Stream` into an mpsc with
`StreamExt::forward`). For ordinary "task computes a result, send it to the egui
loop," a plain `tokio::sync::mpsc::Sender` with `.send(...).await` is simpler and
has no panic footgun.

## PollSendError\<T\>

```rust
pub struct PollSendError<T>(/* private */);
pub fn into_inner(self) -> Option<T>   // the item that failed to send, if any
```

Returned when the channel is closed. `Display`; `Error`/`Debug` where `T: Debug`.

---

## PollSemaphore

> A wrapper around `tokio::sync::Semaphore` with a `poll_acquire` method.

```rust
pub struct PollSemaphore { /* private */ }   // Clone + Debug; wraps Arc<Semaphore>

pub fn new(semaphore: Arc<Semaphore>) -> Self

pub fn poll_acquire(&mut self, cx: &mut Context<'_>) -> Poll<Option<OwnedSemaphorePermit>>
pub fn poll_acquire_many(&mut self, cx: &mut Context<'_>, permits: u32)
    -> Poll<Option<OwnedSemaphorePermit>>

pub fn available_permits(&self) -> usize
pub fn add_permits(&self, n: usize)
pub fn close(&self)
pub fn clone_inner(&self) -> Arc<Semaphore>   // clone the inner Arc
pub fn into_inner(self) -> Arc<Semaphore>
pub fn clone(&self) -> PollSemaphore
```

Yields `Poll::Ready(Some(permit))` on success, `Poll::Ready(None)` if closed,
`Poll::Pending` otherwise. Yields an `OwnedSemaphorePermit` (it holds an
`Arc<Semaphore>`), so the permit is `'static`.

Gotcha: **single-waker** — only the most recent `Context`'s `Waker` is
registered. Don't poll one `PollSemaphore` from two places expecting both to
wake. Pixhaus use: bound concurrent AI requests or concurrent decode tasks to N
in flight. For the non-poll path, the underlying `Semaphore::acquire().await`
(via `clone_inner()`) is usually all you need.

---

## ReusableBoxFuture\<'a, T\>

> A reusable `Pin<Box<dyn Future<Output = T> + Send + 'a>>` — swap the stored
> future in place, reusing the allocation where possible.

```rust
pub struct ReusableBoxFuture<'a, T> { /* private */ }

pub fn new<F>(future: F) -> Self            where F: Future<Output = T> + Send + 'a
pub fn set<F>(&mut self, future: F)         where F: Future<Output = T> + Send + 'a
pub fn try_set<F>(&mut self, future: F) -> Result<(), F>
                                            where F: Future<Output = T> + Send + 'a
pub fn get_pin(&mut self) -> Pin<&mut (dyn Future<Output = T> + Send)>
pub fn poll(&mut self, cx: &mut Context<'_>) -> Poll<T>
```

- `set` reallocates iff the new future's `Layout` differs from the stored one;
  same-layout swaps reuse the box.
- `try_set` never reallocates — returns `Err(future)` (handing it back) on a
  layout mismatch, so you can fall back to `set`.

Niche: building a custom `Stream`/`poll_*` adapter that holds an in-flight future
between polls and replaces it on completion without per-iteration boxing. Most
Pixhaus code never needs this — prefer `async`/`await` and let the compiler size
the state machine.
