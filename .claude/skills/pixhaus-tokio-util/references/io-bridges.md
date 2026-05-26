# tokio_util::io — sync ↔ async and Stream ↔ AsyncRead bridges

Feature: **`io`** for most items; **`io-util`** (implies `io`) adds
`SyncIoBridge` and `read_exact_arc`. tokio-util 0.7.18. Buffers are
`bytes::Bytes`/`BytesMut`.

This module is the seam between Pixhaus's sync encoders (`zstd`, the `image`
crate — `std::io::Read`/`Write`) and the async runtime, and between an async byte
stream (an AI backend's HTTP body) and an `AsyncRead`.

## Table of contents

- `SyncIoBridge` — async I/O driven synchronously (the spawn_blocking rule)
- `ReaderStream` — `AsyncRead` → `Stream<Item = Result<Bytes, io::Error>>`
- `StreamReader` — `Stream` of `Bytes`-like → `AsyncRead`
- `InspectReader` / `InspectWriter` — tap bytes as they flow
- `CopyToBytes` / `SinkWriter` — `Sink<Bytes>` → `AsyncWrite`
- Free functions: `read_buf`, `poll_read_buf`, `poll_write_buf`, `read_exact_arc`

---

## SyncIoBridge\<T\> — the one with the sharp edge (`io` + `io-util`)

> Wraps a `tokio::io::AsyncRead` so it can be used as `std::io::Read` (and
> `AsyncWrite` as `Write`, `AsyncBufRead` as `BufRead`, `AsyncSeek` as `Seek`).

```rust
pub struct SyncIoBridge<T> { /* private */ }

pub fn new(src: T) -> Self where T: Unpin              // captures Handle::current(); PANICS if no runtime
pub fn new_with_handle(src: T, rt: Handle) -> Self where T: Unpin  // explicit handle, build off-runtime
pub fn into_inner(self) -> T
pub fn shutdown(&mut self) -> std::io::Result<()> where T: AsyncWrite + Unpin
pub fn is_write_vectored(&self) -> bool where T: AsyncWrite
```

Blocking trait impls delegate to the inner async type: `Read` (for
`AsyncRead + Unpin`), `Write` (for `AsyncWrite + Unpin`), `BufRead` (for
`AsyncBufRead + Unpin`), `Seek` (for `AsyncSeek + Unpin`), plus `AsRef`/`AsMut`.

**The rule that prevents the deadlock:** every blocking call internally does
`Handle::block_on` on the captured runtime. Calling `block_on` from an async
worker thread panics or deadlocks. So a `SyncIoBridge` **must run on a
`spawn_blocking` thread**, never directly on a runtime worker. `new()` also
panics if no runtime is entered — use `new_with_handle()` to construct from a
non-async context.

```rust
use tokio_util::io::SyncIoBridge;

// reader: impl tokio::io::AsyncRead + Unpin + Send + 'static
let mut sync_reader = SyncIoBridge::new(reader);     // build on the runtime thread
let decoded = tokio::task::spawn_blocking(move || {
    // off the runtime, so block_on inside the bridge is legal
    let mut out = Vec::new();
    let mut zstd = zstd::stream::read::Decoder::new(&mut sync_reader)?; // sync API, async source
    std::io::copy(&mut zstd, &mut out)?;
    Ok::<_, std::io::Error>(out)
})
.await??;
```

This is the `.pixhaus` load path (zstd + rmp-serde) and the PNG/sprite-sheet
decode path. See `pixhaus-zstd` and `pixhaus-rmp-serde`.

---

## ReaderStream\<R\> — `AsyncRead` → `Stream` (`io`)

```rust
pub struct ReaderStream<R> { /* private */ }
pub fn new(reader: R) -> Self where R: AsyncRead            // default capacity 4096 (not guaranteed)
pub fn with_capacity(reader: R, capacity: usize) -> Self where R: AsyncRead
// impl Stream for ReaderStream<R: AsyncRead>: Item = Result<Bytes, std::io::Error>
```

Yields `Result<Bytes, io::Error>` chunks; `None` at EOF. Each chunk can be an
error — handle it, don't `unwrap`.

```rust
use tokio_stream::StreamExt;
use tokio_util::io::ReaderStream;

let mut stream = ReaderStream::new(&data[..]);
while let Some(chunk) = stream.next().await {
    let bytes = chunk?;            // Result<Bytes, io::Error>
}
```

---

## StreamReader\<S, B\> — `Stream` of `Bytes`-like → `AsyncRead` (`io`)

The inverse of `ReaderStream`. Turns a stream of buffers (an HTTP body) into an
`AsyncRead`/`AsyncBufRead`.

```rust
pub struct StreamReader<S, B> { /* private */ }
// where S: Stream<Item = Result<B, E>>, B: Buf, E: Into<std::io::Error>

pub fn new(stream: S) -> Self
pub fn into_inner(self) -> S                          // discards any buffered partial chunk
pub fn into_inner_with_chunk(self) -> (S, Option<B>)  // also returns leftover buffered data
pub fn get_ref(&self) -> &S
pub fn get_mut(&mut self) -> &mut S
pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut S>
// impl AsyncRead + AsyncBufRead
```

The bound to remember: the stream item must be `Result<B, E>` with `B: Buf` and
`E: Into<std::io::Error>`. A `reqwest::Error` doesn't satisfy that — map it
first.

```rust
use tokio::io::AsyncReadExt;
use tokio_util::io::StreamReader;

let stream = response
    .bytes_stream()                                              // Stream<Item = Result<Bytes, reqwest::Error>>
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)); // -> E: Into<io::Error>
let mut reader = StreamReader::new(stream);
let mut head = [0u8; 8];
reader.read_exact(&mut head).await?;   // now an AsyncRead — feed PNG sniffing, framing, etc.
```

---

## InspectReader / InspectWriter (`io`)

Tap on bytes without changing them — hashing, checksums, progress, logging.

```rust
pub struct InspectReader<R, F> { /* private */ }
pub fn new(reader: R, f: F) -> InspectReader<R, F> where R: AsyncRead, F: FnMut(&[u8])
pub fn into_inner(self) -> R          // impl AsyncRead: calls f with each chunk read

pub struct InspectWriter<W, F> { /* private */ }
pub fn new(writer: W, f: F) -> InspectWriter<W, F> where W: AsyncWrite, F: FnMut(&[u8])
pub fn into_inner(self) -> W          // impl AsyncWrite: calls f with each chunk written (never empty)
```

---

## CopyToBytes / SinkWriter (`io`)

Adapt a `Sink<Bytes>` into an `AsyncWrite`. `SinkWriter` needs a `Sink<&[u8]>`,
which is rarely implemented directly, so the standard pattern wraps the byte sink
in `CopyToBytes` first.

```rust
pub struct CopyToBytes<S> { /* private */ }     // Sink<&[u8]> over a Sink<Bytes> (copies each slice)
pub fn new(inner: S) -> Self
pub fn get_ref(&self) -> &S
pub fn get_mut(&mut self) -> &mut S
pub fn into_inner(self) -> S

pub struct SinkWriter<S> { /* private */ }      // AsyncWrite over a Sink<&[u8]>
pub fn new(sink: S) -> Self
pub fn get_ref(&self) -> &S
pub fn get_mut(&mut self) -> &mut S
pub fn into_inner(self) -> S
// impl AsyncWrite where for<'a> S: Sink<&'a [u8], Error = E>, E: Into<std::io::Error>

// usual composition:
let writer = SinkWriter::new(CopyToBytes::new(sink_of_bytes));
```

---

## Free functions

```rust
// io — read from an AsyncRead into a BufMut (async convenience)
pub async fn read_buf<R, B>(read: &mut R, buf: &mut B) -> std::io::Result<usize>
    where R: AsyncRead + Unpin, B: BufMut;

// io — poll-based read into a BufMut, for manual future/poll impls
pub fn poll_read_buf<T: AsyncRead + ?Sized, B: BufMut>(
    io: Pin<&mut T>, cx: &mut Context<'_>, buf: &mut B,
) -> Poll<std::io::Result<usize>>;

// io — poll-based write from a Buf into an AsyncWrite; advances the buffer cursor
pub fn poll_write_buf<T: AsyncWrite + ?Sized, B: Buf>(
    io: Pin<&mut T>, cx: &mut Context<'_>, buf: &mut B,
) -> Poll<std::io::Result<usize>>;

// io-util — read exactly `len` bytes into a fresh Arc<[u8]>
pub async fn read_exact_arc<R: AsyncRead>(read: R, len: usize) -> std::io::Result<Arc<[u8]>>;
```

There is no `write_buf` free function — only `read_buf`, the two `poll_*_buf`
poll-style functions, and `read_exact_arc`.

## Also present: `simplex`

The `simplex` submodule provides an in-memory unidirectional byte pipe
(`simplex()` → a `SimplexStream` pair) — useful in tests to connect an
`AsyncWrite` producer to an `AsyncRead` consumer without real I/O.
