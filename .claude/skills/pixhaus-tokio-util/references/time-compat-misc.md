# tokio_util — time, compat, net, context, either, and the feature map

tokio-util 0.7.18, MIT. The smaller modules, plus the complete feature table.

## Table of contents

- `time::DelayQueue` — per-item delayed yield (debounce, expiry)
- `compat` — `tokio::io` ↔ `futures-io` bridge
- `net::Listener` — listener abstraction
- `context` — run a future under tokio's driver from another executor
- `either::Either` — one-of-two without boxing
- Full feature table

---

## time::DelayQueue\<T\> (feature `time`)

A queue where each inserted element carries its own delay and is yielded once its
deadline passes. **Not** a single timer — a set of independent timers multiplexed
over one `tokio::time` driver. Drive it with `poll_expired`, or via its
`Stream<Item = Expired<T>>` impl.

```rust
pub fn new() -> DelayQueue<T>
pub fn with_capacity(capacity: usize) -> DelayQueue<T>

pub fn insert(&mut self, value: T, timeout: Duration) -> Key
pub fn insert_at(&mut self, value: T, when: Instant) -> Key

pub fn remove(&mut self, key: &Key) -> Expired<T>            // PANICS on a stale/unknown key
pub fn try_remove(&mut self, key: &Key) -> Option<Expired<T>>

pub fn reset(&mut self, key: &Key, timeout: Duration)
pub fn reset_at(&mut self, key: &Key, when: Instant)

pub fn poll_expired(&mut self, cx: &mut Context<'_>) -> Poll<Option<Expired<T>>>

pub fn len(&self) -> usize
pub fn is_empty(&self) -> bool
pub fn clear(&mut self)
pub fn capacity(&self) -> usize
pub fn reserve(&mut self, additional: usize)
pub fn deadline(&self, key: &Key) -> Instant
```

- `Instant`/`Duration` are `tokio::time` types; the queue needs a runtime with
  the time driver. Enabling tokio-util's `time` flips on `tokio/time`.
- `poll_expired` returns `Poll::Ready(None)` when the queue is **empty** — `None`
  means "nothing left", not "wait". In a long-lived loop, re-insert before
  treating it as terminal.
- `remove` panics on a stale key; prefer `try_remove` when the key may already
  have expired.

`delay_queue` submodule: `Key` (identifies a stored value), `Expired<T>`
(`into_inner()`, `get_ref()`, `key()`, `deadline()`), and a `DelayQueue`
re-export. A `time::FutureExt` trait is also present (the `timeout` adapter).

```rust
use tokio_util::time::DelayQueue;
use std::time::Duration;
use futures::future::poll_fn;

let mut q: DelayQueue<&str> = DelayQueue::new();
let dirty = q.insert("save", Duration::from_millis(500));
// ...on each new edit, push the save out instead of piling up:
q.reset(&dirty, Duration::from_millis(500));   // debounce
while !q.is_empty() {
    match poll_fn(|cx| q.poll_expired(cx)).await {
        Some(e) => trigger(e.into_inner()),
        None => break,
    }
}
```

Pixhaus use: debounce autosave (reset the key on each stroke so a burst collapses
to one save), throttle expensive recomputes, and expire cache entries
(thumbnails, AI results).

---

## compat (feature `compat`) — tokio I/O ↔ futures-io I/O

Bridges tokio's `AsyncRead`/`AsyncWrite` and `futures-io`'s same-named traits.
Needed to feed a tokio stream into a crate built on `futures-io` (e.g. the
futures variants of `async-compression`, `async-tungstenite`) or the reverse.

```rust
pub struct Compat<T> { /* into_inner / get_ref / get_mut */ }

// tokio I/O -> futures-io I/O
pub trait TokioAsyncReadCompatExt: AsyncRead  { fn compat(self) -> Compat<Self>; }
pub trait TokioAsyncWriteCompatExt: AsyncWrite { fn compat_write(self) -> Compat<Self>; }
// futures-io I/O -> tokio I/O
pub trait FuturesAsyncReadCompatExt: AsyncRead  { fn compat(self) -> Compat<Self>; }
pub trait FuturesAsyncWriteCompatExt: AsyncWrite { fn compat_write(self) -> Compat<Self>; }
```

Read traits expose `compat()`, write traits expose `compat_write()` — so a
read+write type converts on each axis without a name collision. Import the
extension trait matching the source ecosystem; disambiguate with the trait name
if both are in scope. (`compat_write` name reported from the module index —
verify if used verbatim.)

```rust
use tokio_util::compat::FuturesAsyncReadCompatExt;
let tokio_reader = futures_io_reader.compat();   // now usable as tokio::io::AsyncRead
```

---

## net (features `net` + `codec`) — listener abstraction

```rust
pub trait Listener {
    type Io: AsyncRead + AsyncWrite;
    type Addr;
    fn poll_accept(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<(Self::Io, Self::Addr)>>;
    fn local_addr(&self) -> io::Result<Self::Addr>;
    fn accept(&mut self) -> ListenerAcceptFut<'_, Self> { ... }   // provided
}
```

Lets you write code generic over `TcpListener` (`Io = TcpStream`, `Addr =
SocketAddr`) and `UnixListener` (`Io = UnixStream`). `ListenerAcceptFut<'a, L>` is
the `accept()` future. Submodule `net::unix` holds UDS helpers. The `udp` module
(also `net` + `codec`) provides `UdpFramed`. Pixhaus is a single-user desktop
editor with no server, so this is unlikely to come up — present for
completeness.

---

## context (feature `rt`) — tokio driver under a foreign executor

For running a tokio-dependent future on a non-tokio runtime while still giving it
tokio's timer/IO drivers. Provides the *driver context*, not the executor — the
foreign runtime still polls it.

```rust
pub struct TokioContext<F> { /* wraps F, enters a Handle's context per poll */ }
pub fn new(future: F, handle: Handle) -> TokioContext<F>
pub fn into_inner(self) -> F

pub trait RuntimeExt {                       // on tokio::runtime::Handle
    fn wrap_future<F>(&self, fut: F) -> TokioContext<F>;
}
```

Pixhaus owns its tokio runtime in the binary, so this is essentially never
needed — it exists for embedding tokio code inside another async runtime.
(Signatures reported from the module index — verify if used.)

---

## either::Either (no feature — always available)

> Combines two futures, streams, or sinks with the same associated types into one
> type.

```rust
pub enum Either<L, R> { Left(L), Right(R) }
```

Forwards a trait to whichever variant is held, but only when **both** `L` and `R`
implement it with **matching** associated types:

- `Future` (same `Output`), `Stream` (same `Item`), `Sink<Item>` (same
  `Item`/`Error`)
- `AsyncRead`, `AsyncWrite`, `AsyncSeek`, `AsyncBufRead`

Return one of two concrete async types from a function without `Box<dyn>`:

```rust
use tokio_util::either::Either;

fn pick(stream_a: A, stream_b: B, use_a: bool) -> impl Stream<Item = T> {
    if use_a { Either::Left(stream_a) } else { Either::Right(stream_b) }
}
```

Mismatched associated types won't compile. Picking a variant is a runtime branch
carrying both layouts — cheaper than boxing, at the cost of size. Inherent
`accept`/`local_addr` methods exist when both sides are `Listener` (needs `net` +
`codec`).

---

## Full feature table

Default is empty — pull only what you use. Always-compiled (no feature): `sync`,
`either`, `future`, the `bytes` re-export.

| Feature | Unlocks | Pulls in |
|---|---|---|
| `rt` | `task::{TaskTracker, AbortOnDropHandle, LocalPoolHandle, JoinQueue}`, `context` | `tokio/rt`, `tokio/sync`, `futures-util` |
| `join-map` | `task::JoinMap`, `JoinMapKeys` | `rt`, `hashbrown` |
| `io` | `io::{ReaderStream, StreamReader, InspectReader/Writer, CopyToBytes, SinkWriter}`, `read_buf`, `poll_*_buf`, `simplex` | — |
| `io-util` | adds `io::SyncIoBridge`, `read_exact_arc` | `io`, `tokio/io-util`, `tokio/rt` |
| `codec` | `codec::*` | — |
| `time` | `time::DelayQueue` | `tokio/time`, `slab` |
| `net` | `net::Listener`, `udp::UdpFramed` (UdpFramed also needs `codec`) | `tokio/net` |
| `compat` | `compat::*` | `futures-io` |
| `full` | all of the above | (does NOT add `tracing`) |
| `tracing` | internal instrumentation | `tracing` |

Enabling `rt`/`io-util`/`time`/`net` turns on the matching `tokio` feature, so
your `tokio` dependency must carry it too. A likely Pixhaus line:

```toml
tokio-util = { version = "0.7", features = ["rt", "io-util", "time", "codec"] }
```

Drop `codec` if no AI backend frames a socket; drop `time` if you debounce
elsewhere. Don't use `full` — it drags in listeners and codecs a desktop editor
never touches.
