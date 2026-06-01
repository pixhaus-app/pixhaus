# futures channels (0.3.32)

`futures::channel` provides async channels for passing values between tasks. Two
submodules: `oneshot` (single value, one sender, one receiver) and `mpsc`
(multi-producer, single-consumer, bounded or unbounded). The send halves
implement `Sink`, the receive halves implement `Future` (oneshot) or `Stream`
(mpsc) — which is the whole reason to reach for these over `tokio::sync` in a
non-tokio poll loop. Requires the `std` or `alloc` feature (`alloc` is on by
default).

## Contents

- [oneshot](#oneshot)
- [mpsc bounded](#mpsc-bounded)
- [mpsc unbounded](#mpsc-unbounded)
- [mpsc error types](#mpsc-error-types)
- [futures channels vs tokio::sync](#futures-channels-vs-tokiosync)

## oneshot

One value, one use. `Sender` consumes itself on `send`; `Receiver` is a `Future`
that resolves to `Result<T, Canceled>`.

```rust
use futures::channel::oneshot;

let (tx, rx) = oneshot::channel::<u32>();
tx.send(42).unwrap();          // consumes tx; Err(T) if the receiver is gone
let value = rx.await?;          // Result<u32, Canceled>
```

Constructor:

| fn | signature | what it does |
| --- | --- | --- |
| `channel` | `oneshot::channel<T>() -> (Sender<T>, Receiver<T>)` | new one-shot channel for a single value |

### `oneshot::Sender<T>`

| method | signature | what it does |
| --- | --- | --- |
| `send` | `send(self, t: T) -> Result<(), T>` | sends the value, consuming `self`; returns `Err(t)` if the receiver was dropped |
| `poll_canceled` | `poll_canceled(&mut self, cx: &mut Context<'_>) -> Poll<()>` | polls to detect whether the `Receiver` has been dropped |
| `cancellation` | `cancellation(&mut self) -> Cancellation<'_, T>` | future that resolves when the corresponding `Receiver` has hung up |
| `is_canceled` | `is_canceled(&self) -> bool` | true if the corresponding `Receiver` has been dropped |
| `is_connected_to` | `is_connected_to(&self, receiver: &Receiver<T>) -> bool` | true if this sender is connected to the given receiver |

Note: the method is `is_connected_to(&self, receiver)`, not `is_connected()`.
Impls: `Debug`, `Drop`, `Unpin`.

### `oneshot::Receiver<T>`

Implements `Future<Output = Result<T, Canceled>>` and `FusedFuture`. Resolves to
`Ok(T)` when the value arrives, `Err(Canceled)` when the sender drops first.

| method | signature | what it does |
| --- | --- | --- |
| `try_recv` | `try_recv(&mut self) -> Result<Option<T>, Canceled>` | attempts to receive outside a task context; `Ok(None)` means not yet sent |
| `close` | `close(&mut self)` | gracefully closes the receiver, preventing any further send |

Impls: `Future`, `FusedFuture`, `Debug`, `Drop`, `Unpin`. Not `Clone`.

### `oneshot::Canceled`

Unit struct (`pub struct Canceled;`). "Error returned from a `Receiver` when the
corresponding `Sender` is dropped." Impls: `Error`, `Display`, `Debug`, `Clone`,
`Copy`, `PartialEq`, `Eq`.

Semantics: single value, single use. Dropping the `Sender` without sending makes
the `Receiver` resolve to `Err(Canceled)`. Dropping the `Receiver` lets the
`Sender` learn via `poll_canceled` / `cancellation` / `is_canceled`.

## mpsc bounded

Multi-producer, single-consumer with backpressure. `channel(buffer)` returns a
bounded pair. Capacity is `buffer + num-senders`: each sender gets one guaranteed
slot beyond the shared `buffer` of first-come-first-serve slots, so backpressure
keeps senders from outpacing the receiver.

```rust
use futures::channel::mpsc;
use futures::{SinkExt, StreamExt};

let (mut tx, mut rx) = mpsc::channel::<Job>(16);
tx.send(job).await?;                 // Sink: awaits when full (backpressure)
while let Some(job) = rx.next().await { /* ... */ }   // Stream
```

Constructor:

| fn | signature | what it does |
| --- | --- | --- |
| `channel` | `mpsc::channel<T>(buffer: usize) -> (Sender<T>, Receiver<T>)` | bounded channel; total capacity `buffer + num-senders` |

### `mpsc::Sender<T>`

Implements `Sink<T, Error = SendError>` and `Clone` (clone to get more
producers). Use `SinkExt::send` for the awaiting send; the inherent methods below
are the non-awaiting / lower-level paths.

| method | signature | what it does |
| --- | --- | --- |
| `try_send` | `try_send(&mut self, msg: T) -> Result<(), TrySendError<T>>` | sends without awaiting; returns the message on error (full or disconnected) |
| `start_send` | `start_send(&mut self, msg: T) -> Result<(), SendError>` | sends a message on the channel (the `Sink::start_send` step) |
| `poll_ready` | `poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), SendError>>` | polls for guaranteed capacity to send at least one item |
| `is_closed` | `is_closed(&self) -> bool` | whether the channel is closed, without a context |
| `close_channel` | `close_channel(&mut self)` | closes the channel from the sender side, blocking new messages |
| `disconnect` | `disconnect(&mut self)` | disconnects this sender; closes the channel if it was the last sender |
| `same_receiver` | `same_receiver(&self, other: &Sender<T>) -> bool` | whether both senders feed the same receiver |
| `hash_receiver` | `hash_receiver<H: Hasher>(&self, hasher: &mut H)` | hashes the receiver identity into `hasher` |

For the awaiting send: `use futures::SinkExt;` then `tx.send(msg).await` (returns
`Result<(), SendError>`).

### `mpsc::Receiver<T>`

Implements `Stream<Item = T>` and `FusedStream`. Single consumer — not `Clone`.

| method | signature | what it does |
| --- | --- | --- |
| `try_recv` | `try_recv(&mut self) -> Result<T, TryRecvError>` | receives without blocking; errors if empty, or empty and closed |
| `recv` | `recv(&mut self) -> Recv<'_, Receiver<T>>` | future that waits for one message; `RecvError` if empty and closed |
| `try_next` | `try_next(&mut self) -> Result<Option<T>, TryRecvError>` (deprecated) | prefer `try_recv`; old non-notifying poll |
| `close` | `close(&mut self)` | closes the receiving half without dropping it; still drains buffered messages |
| `poll_next` | `poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>>` | `Stream` method; `None` when exhausted |

Impls: `Stream`, `FusedStream`, `Debug`, `Drop`, `Unpin`. Not `Clone`.

Note: `Receiver::try_recv` here returns `Result<T, TryRecvError>` (errors on
empty), unlike `oneshot::Receiver::try_recv` which returns
`Result<Option<T>, Canceled>`. The frame-drain pattern uses the stream side
(`poll_next`) — see the decision section.

## mpsc unbounded

No capacity bound, no backpressure. "A `send` on this channel will always succeed
as long as the receive half has not been closed." If the receiver falls behind,
messages buffer without limit and the process can exhaust memory.

```rust
use futures::channel::mpsc;

let (tx, mut rx) = mpsc::unbounded::<Event>();
tx.unbounded_send(event)?;            // &self, no await, no &mut
// rx is a Stream<Item = Event>
```

Constructor:

| fn | signature | what it does |
| --- | --- | --- |
| `unbounded` | `mpsc::unbounded<T>() -> (UnboundedSender<T>, UnboundedReceiver<T>)` | unbounded channel; sends never block |

### `mpsc::UnboundedSender<T>`

Implements `Sink<T>` (also `Sink for &UnboundedSender<T>`) and `Clone`. Key win:
`unbounded_send` takes `&self` and does not await — callable from any sync
context, including a non-async producer.

| method | signature | what it does |
| --- | --- | --- |
| `unbounded_send` | `unbounded_send(&self, msg: T) -> Result<(), TrySendError<T>>` | sends a message; needs only `&self`, no await |
| `poll_ready` | `poll_ready(&self, _: &mut Context<'_>) -> Poll<Result<(), SendError>>` | checks readiness (always ready unless closed) |
| `is_closed` | `is_closed(&self) -> bool` | whether the channel is closed, without a context |
| `close_channel` | `close_channel(&self)` | closes the channel from the sender side |
| `disconnect` | `disconnect(&mut self)` | disconnects this sender; closes if it was the last |
| `start_send` | `start_send(&mut self, msg: T) -> Result<(), SendError>` | the `Sink::start_send` step |
| `same_receiver` | `same_receiver(&self, other: &UnboundedSender<T>) -> bool` | whether both senders feed the same receiver |
| `hash_receiver` | `hash_receiver<H: Hasher>(&self, hasher: &mut H)` | hashes the receiver identity into `hasher` |

### `mpsc::UnboundedReceiver<T>`

Implements `Stream<Item = T>` and `FusedStream`. Single consumer — not `Clone`.

| method | signature | what it does |
| --- | --- | --- |
| `try_recv` | `try_recv(&mut self) -> Result<T, TryRecvError>` | receives without blocking; errors if empty or empty-and-closed |
| `recv` | `recv(&mut self) -> Recv<'_, UnboundedReceiver<T>>` | future that waits for one message |
| `try_next` | `try_next(&mut self) -> Result<Option<T>, TryRecvError>` (deprecated) | prefer `try_recv` |
| `close` | `close(&mut self)` | closes the receiving half without dropping it |
| `poll_next` | `poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>>` | `Stream` method |

Impls: `Stream`, `FusedStream`. Not `Clone`.

## mpsc error types

### `mpsc::SendError`

Returned by the `Sink` impl and `start_send`/`poll_ready`. Does not carry the
message back.

| method | signature | what it does |
| --- | --- | --- |
| `is_full` | `is_full(&self) -> bool` | true if the error is from the channel being full |
| `is_disconnected` | `is_disconnected(&self) -> bool` | true if the error is from the receiver being dropped |

Impls: `Error`, `Display`, `Debug`, `Clone`, `PartialEq`, `Eq`.

### `mpsc::TrySendError<T>`

Returned by `try_send` and `unbounded_send`. Carries the rejected message so the
caller can recover it.

| method | signature | what it does |
| --- | --- | --- |
| `into_inner` | `into_inner(self) -> T` | returns the message that failed to send |
| `is_full` | `is_full(&self) -> bool` | true if the error is from the channel being full |
| `is_disconnected` | `is_disconnected(&self) -> bool` | true if the error is from the receiver being dropped |

No `err()` method. Impls: `Error`, `Display`, `Debug`, `Clone`, `PartialEq`,
`Eq`.

## futures channels vs tokio::sync

Pixhaus's binary owns a tokio runtime, and the egui update loop drains channels
each frame. That makes the choice between `futures::channel` and `tokio::sync`
deliberate. For the tokio-side APIs (`tokio::sync::mpsc`/`oneshot`/`watch`/
`broadcast`), see the `pixhaus-tokio` skill — this section only covers when to
reach across to `futures::channel`.

The single most useful fact: a `futures::channel::mpsc::Receiver` implements
`Stream`, so it can be polled with `StreamExt` / `poll_next` from any context —
including a non-tokio poll loop — and composed with any generic `Stream`
consumer. A `tokio::sync::mpsc::Receiver` is driven by `recv().await` /
`try_recv()` and is tied to the tokio runtime.

Guidance for Pixhaus:

- If both ends live in tokio land (a tokio task sends, another tokio task
  awaits), prefer `tokio::sync`. It is the runtime you already own.
- Reach for `futures::channel` when the consumer is a non-tokio poll loop, a
  generic `Stream` consumer, or a runtime-agnostic library that should not pin
  itself to tokio.
- Draining each egui frame: `try_recv`-style draining works with either crate.
  `tokio::sync::mpsc::Receiver::try_recv` is the idiomatic "drain each frame"
  call; with `futures::channel` you would loop on the stream side (`poll_next` /
  `try_recv`) until it yields nothing. If you are already on tokio, the
  `tokio::sync` drain is the simpler frame loop.
- `futures::channel::oneshot` is a clean return path for "spawn a task, get one
  result back." The receiving side is just a `Future` you can poll, so it works
  without any runtime on the receiver — useful when a result lands back in the
  egui loop rather than in a tokio task.
