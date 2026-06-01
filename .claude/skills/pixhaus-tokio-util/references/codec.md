# tokio_util::codec — framing a byte stream into messages

Feature: **`codec`**. tokio-util 0.7.18. Buffers are `bytes::BytesMut`/`Bytes`,
so the `bytes` crate is in your public API surface.

A `Decoder` finds frame boundaries in incoming bytes; an `Encoder` serializes
outgoing frames; `Framed`/`FramedRead`/`FramedWrite` run the read/buffer/write
loop and expose the result as a `Stream`/`Sink`. In Pixhaus this matters only if
an AI backend speaks a framed wire protocol over a socket (newline-delimited JSON
from Ollama, length-prefixed frames from a ComfyUI-style socket). Pull frames
with `StreamExt::next`, push with `SinkExt::send` (extension traits from
`futures`/`tokio-stream` must be in scope, and the value pinned).

## Table of contents

- `Decoder` / `Encoder` — the traits you implement
- `Framed` / `FramedRead` / `FramedWrite` / `FramedParts`
- Built-in codecs: `LinesCodec`, `BytesCodec`, `AnyDelimiterCodec`,
  `LengthDelimitedCodec`
- `length_delimited::Builder`

---

## Decoder — what you implement to parse incoming frames

```rust
pub trait Decoder {
    type Item;
    type Error: From<std::io::Error>;

    // required: pull at most one frame out of `src`.
    //   Ok(Some(item)) + consume its bytes  -> a frame is ready
    //   Ok(None)                            -> need more bytes (loop reads, calls again)
    //   Err(_)                              -> malformed; terminates the stream
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error>;

    // provided: called at EOF. Default calls decode and errors if Ok(None) is
    // returned while bytes remain unconsumed. Override to emit a final
    // unterminated frame from trailing data.
    fn decode_eof(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> { ... }

    // provided: wrap an AsyncRead + AsyncWrite into a Framed using this codec.
    fn framed<T: AsyncRead + AsyncWrite + Sized>(self, io: T) -> Framed<T, Self> where Self: Sized { ... }
}
```

`decode` is called **repeatedly** — never assume one call per frame. Return
`Ok(None)` (don't error, don't block) when the frame isn't fully buffered;
consume exactly the frame's bytes when you return `Some`.

## Encoder\<Item\> — what you implement to serialize outgoing frames

```rust
pub trait Encoder<Item> {
    type Error: From<std::io::Error>;
    fn encode(&mut self, item: Item, dst: &mut BytesMut) -> Result<(), Self::Error>;
}
```

Generic over the item type, so one codec can `impl Encoder<A>` and
`impl Encoder<B>`. `encode` appends serialized bytes to `dst` (the
`FramedWrite`'s buffer). Both trait `Error` types must be `From<io::Error>`.

---

## Framed / FramedRead / FramedWrite

`Framed<T, U>` — `Stream<Item = Result<U::Item, U::Error>>` + `Sink<I, Error =
U::Error>` over an `AsyncRead + AsyncWrite`, one codec for both directions.
`FramedRead<T, D>` — read half (`Stream`). `FramedWrite<T, E>` — write half
(`Sink`).

```rust
// Framed
pub fn new(inner: T, codec: U) -> Framed<T, U>
pub fn with_capacity(inner: T, codec: U, capacity: usize) -> Framed<T, U>
pub fn from_parts(parts: FramedParts<T, U>) -> Framed<T, U>
pub fn get_ref(&self) -> &T            pub fn get_mut(&mut self) -> &mut T
pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut T>
pub fn codec(&self) -> &U              pub fn codec_mut(&mut self) -> &mut U
pub fn codec_pin_mut(self: Pin<&mut Self>) -> &mut U
pub fn map_codec<C, F>(self, map: F) -> Framed<T, C>   where F: FnOnce(U) -> C
pub fn read_buffer(&self) -> &BytesMut    pub fn read_buffer_mut(&mut self) -> &mut BytesMut
pub fn write_buffer(&self) -> &BytesMut   pub fn write_buffer_mut(&mut self) -> &mut BytesMut
pub fn into_inner(self) -> T
pub fn into_parts(self) -> FramedParts<T, U>

// FramedRead: new, with_capacity, get_ref/_mut/_pin_mut, into_inner,
//             decoder()/decoder_mut()/decoder_pin_mut(), map_decoder, read_buffer(_mut)
// FramedWrite: new, get_ref/_mut/_pin_mut, into_inner,
//              encoder()/encoder_mut()/encoder_pin_mut(), map_encoder, write_buffer(_mut)
```

Use `into_parts`/`from_parts` (not `into_inner`) when swapping codecs without
losing already-buffered bytes.

### FramedParts\<T, U\>

```rust
pub io: T,
pub codec: U,
pub read_buf: BytesMut,    // read-but-unprocessed bytes
pub write_buf: BytesMut,   // buffered-but-unwritten bytes
// #[non_exhaustive] — construct via new, then assign buffers if carrying bytes over
pub fn new<I>(io: T, codec: U) -> FramedParts<T, U>   where U: Encoder<I>
```

---

## Built-in codecs

### LinesCodec
Lines split on `\n` (all platforms). Decodes → `String`; encodes from any
`T: AsRef<str>`. Error `LinesCodecError` (a max-length variant + `Io(io::Error)`).

```rust
pub fn new() -> LinesCodec                       // max_length == usize::MAX (UNBOUNDED)
pub fn new_with_max_length(max_length: usize) -> Self
pub fn max_length(&self) -> usize
```

`new()` is a DoS risk on untrusted input — a peer that never sends `\n` forces
unbounded buffering. Prefer `new_with_max_length`; an over-length line yields a
`LinesCodecError`, then bytes are discarded to the limit until the next newline.

### BytesCodec
Pass-through, no framing. Zero-sized, `Copy`/`Clone`. Decodes → `BytesMut`;
`impl Encoder<Bytes> + Encoder<BytesMut>`; `Error = std::io::Error`.

```rust
pub fn new() -> BytesCodec
```

### AnyDelimiterCodec
Splits on any byte in a seek-delimiter set. Decodes → `Bytes`; encodes from
`T: AsRef<str>` (appends a terminator sequence). Error `AnyDelimiterCodecError`
(max-length variant + `Io`).

```rust
pub fn new(seek_delimiters: Vec<u8>, sequence_writer: Vec<u8>) -> AnyDelimiterCodec   // UNBOUNDED
pub fn new_with_max_length(seek_delimiters: Vec<u8>, sequence_writer: Vec<u8>, max_length: usize) -> Self
pub fn max_length(&self) -> usize
```

`seek_delimiters` = bytes that terminate an incoming chunk (any one matches);
`sequence_writer` = bytes appended after each encoded item. Same unbounded
caveat as `LinesCodec`.

### LengthDelimitedCodec (module `codec::length_delimited`)
Length-prefixed frames — whole frames without manual length bookkeeping.
Decodes → `BytesMut`; `impl Encoder<Bytes>` (writes the header then payload);
`Error = std::io::Error`.

```rust
pub fn new() -> Self                       // u32 big-endian length, 8 MB max
pub fn builder() -> Builder
pub fn max_frame_length(&self) -> usize
pub fn set_max_frame_length(&mut self, val: usize)   // applies to frames decoded AFTER the call
```

The 8 MB default cap makes this the safe choice against untrusted peers.

### length_delimited::Builder
Setters return `&mut Self` (chainable). Defaults in brackets.

```rust
pub fn new() -> Builder
pub fn big_endian(&mut self) -> &mut Self        // [default]
pub fn little_endian(&mut self) -> &mut Self
pub fn native_endian(&mut self) -> &mut Self
pub fn max_frame_length(&mut self, val: usize) -> &mut Self          // [8 MB]
pub fn length_field_type<T: LengthFieldType>(&mut self) -> &mut Self // [u32]; u16|u32|u64|usize
pub fn length_field_length(&mut self, val: usize) -> &mut Self       // [4] bytes, 1..=8
pub fn length_field_offset(&mut self, val: usize) -> &mut Self       // [0] header bytes before length
pub fn length_adjustment(&mut self, val: isize) -> &mut Self         // [0] encoded length vs payload delta
pub fn num_skip(&mut self, val: usize) -> &mut Self                  // [offset + length] bytes skipped before payload
pub fn new_codec(&self) -> LengthDelimitedCodec
pub fn new_read<T: AsyncRead>(&self, upstream: T) -> FramedRead<T, LengthDelimitedCodec>
pub fn new_write<T: AsyncWrite>(&self, inner: T) -> FramedWrite<T, LengthDelimitedCodec>
pub fn new_framed<T: AsyncRead + AsyncWrite>(&self, inner: T) -> Framed<T, LengthDelimitedCodec>
```

`length_field_type::<T>()` sets width and numeric type at once; the
offset/adjustment/skip trio handles headers where the length isn't at byte 0 or
doesn't equal the payload length.

---

## Example: a custom u32-be length-prefixed String codec

```rust
use bytes::{Buf, BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder, Framed};

struct MyCodec;

impl Decoder for MyCodec {
    type Item = String;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<String>, Self::Error> {
        if src.len() < 4 {
            return Ok(None);                         // need the length header
        }
        let len = u32::from_be_bytes(src[..4].try_into().expect("4 bytes")) as usize;
        if src.len() < 4 + len {
            src.reserve(4 + len - src.len());        // avoid repeated reallocs as the frame arrives
            return Ok(None);                         // need the full body
        }
        src.advance(4);                              // drop the header
        let body = src.split_to(len);
        String::from_utf8(body.to_vec())
            .map(Some)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

impl Encoder<String> for MyCodec {
    type Error = std::io::Error;
    fn encode(&mut self, item: String, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let bytes = item.as_bytes();
        dst.reserve(4 + bytes.len());
        dst.put_u32(bytes.len() as u32);
        dst.put_slice(bytes);
        Ok(())
    }
}

// wrap a socket and use it as Stream + Sink
async fn run(socket: tokio::net::TcpStream) -> std::io::Result<()> {
    use futures::{SinkExt, StreamExt};
    let mut framed = Framed::new(socket, MyCodec);
    framed.send("hello".to_string()).await?;         // Sink: encode + write
    if let Some(frame) = framed.next().await {        // Stream: read + decode
        let line: String = frame?;
    }
    Ok(())
}
```

One-liner with a built-in codec:

```rust
use tokio_util::codec::{FramedRead, LinesCodec};
// Stream<Item = Result<String, LinesCodecError>>, capped at 8 KiB:
let lines = FramedRead::new(reader, LinesCodec::new_with_max_length(8 * 1024));
```
