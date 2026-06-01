---
name: pixhaus-zstd
description: >
  Use when compressing or decompressing data with the `zstd` crate in Pixhaus —
  above all the `.pixhaus` project format, which is MessagePack (`rmp-serde`)
  piped through zstd, plus any PNG/sprite-sheet or cache bytes you want to shrink.
  Trigger this for ANY "save/load the project file", "compress this buffer",
  "wrap the file writer in a zstd encoder", "decompress these bytes", "pick a
  compression level", "stream vs in-memory compression", "train/use a zstd
  dictionary", "guard a decompress against a zip bomb / untrusted .pixhaus", "is
  zstd ok under our MIT license / will cargo deny flag it", or "why is my zstd
  output truncated / corrupt" request, and whenever you see `zstd::encode_all`,
  `zstd::decode_all`, `zstd::stream`, `zstd::bulk`, `Encoder`, `Decoder`,
  `window_log_max`, or `.finish()` on a zstd stream. zstd's
  streaming `Encoder` silently truncates the frame if you drop it without
  finishing, and the API splits across `stream`/`bulk`/`dict` in non-obvious
  ways — reach for this skill to get the boundary right rather than guessing.
---

# zstd for Pixhaus

zstd is the compression layer of the `.pixhaus` file format: the document model
is serialized to MessagePack with `rmp-serde`, then that byte stream is run
through zstd. The Rust `zstd` crate is a thin, safe wrapper over Facebook's C
library, exposing three ways to compress and the same three to decompress. Most
Pixhaus code wants exactly one of them; the trick is choosing.

This skill is the compression half of the format; the serialization half lives in
the `pixhaus-rmp-serde` skill. The save/load examples below call `rmp_serde`
directly — pair the two skills when touching the `io` crate's format code.

## Version and license

| Crate | Version | License | cargo deny |
|---|---|---|---|
| `zstd` | 0.13 | `MIT` | passes the MIT lock |
| `zstd-safe` | 7.x | `MIT/Apache-2.0` | passes |
| `zstd-sys` | 2.0 | `MIT/Apache-2.0` | passes |

The crate metadata is permissive, so zstd clears the workspace MIT lock. The
vendored C library (`facebook/zstd`) is dual BSD-3-Clause / GPL-2.0; under a dual
license you take the permissive BSD arm, and `cargo deny` reads the crate's
declared `MIT/Apache-2.0` metadata, not the C source headers. No action needed —
just don't be surprised if you grep the vendored sources and see GPL text.

```toml
# default features (arrays, legacy, zdict_builder) cover the project format
# and dictionary training. Add nothing unless you need the extras below.
zstd = "0.13"
```

Default features: `arrays`, `legacy` (decode old zstd frames), `zdict_builder`
(dictionary training — `dict::from_samples` etc.). Off by default and rarely
needed here: `zstdmt` (multithreaded compression), `experimental` (unstable C
APIs), `bindgen`/`pkg-config` (link a system libzstd instead of the vendored
one). Don't add `zstdmt` reflexively — see the threading note at the bottom.

## The three APIs — pick by data shape

```
Have the whole input as bytes, want the whole output as bytes, one shot?
  └─ zstd::encode_all / decode_all              (one-liners, the default choice)

Streaming through a Read/Write (a file, a socket) without buffering it all?
  └─ zstd::stream::{Encoder, Decoder}           (the large-.pixhaus path)

Compressing MANY small independent chunks, reusing one context/dictionary?
  └─ zstd::bulk::{Compressor, Decompressor}     (tiles, thumbnails, packets)
```

### 1. One-shot: `encode_all` / `decode_all`

The simplest path, and correct whenever the data already lives in a `Vec<u8>`.

```rust
// source: anything Read (a &[u8] is Read). level: 1..=22, or 0 for the default (3).
pub fn encode_all<R: Read>(source: R, level: i32) -> io::Result<Vec<u8>>
pub fn decode_all<R: Read>(source: R) -> io::Result<Vec<u8>>
```

```rust
let packed = zstd::encode_all(&msgpack_bytes[..], 3)?; // Vec<u8>, full zstd frame
let raw     = zstd::decode_all(&packed[..])?;           // back to the original bytes
```

`encode_all` returns a complete, self-contained frame — no separate finish step,
nothing to truncate. Use it for small-to-medium buffers you already hold.

### 2. Streaming: `stream::write::Encoder` / `stream::read::Decoder` — the project-file path

For a full `.pixhaus` save, prefer streaming: serialize MessagePack *directly
into* a zstd encoder that writes *directly to* the file. Neither the full
uncompressed MessagePack buffer nor the full compressed buffer is ever
materialized in memory — it matters at 8K, where the uncompressed document is
large (see the 8K perf constraint).

```rust
use std::fs::File;
use std::io::BufWriter;

// Save: document -> MessagePack -> zstd -> file, fully streamed.
let file = BufWriter::new(File::create(path)?);
let mut encoder = zstd::stream::write::Encoder::new(file, 3)?;
rmp_serde::encode::write(&mut encoder, &project)?; // writes through the encoder
encoder.finish()?;                                 // REQUIRED — see the gotcha below
```

```rust
use std::fs::File;
use std::io::BufReader;

// Load: file -> zstd -> MessagePack -> document, fully streamed.
let file = BufReader::new(File::open(path)?);
let decoder = zstd::stream::read::Decoder::new(file)?;
let project: Project = rmp_serde::decode::from_read(decoder)?;
```

Constructor signatures worth knowing (`W: Write`, `R: BufRead`; `Decoder::new`
takes any `Read` and wraps it in a `BufReader` for you):

```rust
// write::Encoder<'a, W: Write>
pub fn new(writer: W, level: i32) -> io::Result<Self>
pub fn auto_finish(self) -> AutoFinishEncoder<'a, W>
pub fn finish(self) -> io::Result<W>           // flushes the epilogue, returns the writer
pub fn try_finish(self) -> Result<W, (Self, io::Error)>  // recover the encoder on error
pub fn multithread(&mut self, n_workers: u32) -> io::Result<()> // needs `zstdmt` feature

// read::Decoder<'a, R: BufRead>
pub fn new(reader: R) -> io::Result<Self>      // R: Read; wraps in BufReader internally
pub fn with_buffer(reader: R) -> io::Result<Self> // R: BufRead; you supply the buffering
pub fn window_log_max(&mut self, log_distance: u32) -> io::Result<()> // cap decode memory
```

### 3. Bulk: `bulk::Compressor` / `bulk::Decompressor` — many small independent chunks

When you compress lots of *small, independent* blobs (cached tiles, thumbnails,
network-ish packets), build one `Compressor` and reuse it — it keeps the zstd
context and any dictionary alive across calls instead of re-allocating per blob.
Each `compress` call still produces a standalone frame.

```rust
pub fn new(level: i32) -> Result<Compressor>
pub fn compress(&mut self, data: &[u8]) -> Result<Vec<u8>>
pub fn compress_to_buffer<C: WriteBuf + ?Sized>(&mut self, source: &[u8], dest: &mut C) -> Result<usize>
pub fn set_compression_level(&mut self, level: i32) -> Result<()>

pub fn new() -> Result<Decompressor<'static>>
pub fn decompress(&mut self, data: &[u8], capacity: usize) -> Result<Vec<u8>>
pub fn decompress_to_buffer<C: WriteBuf + ?Sized>(&mut self, source: &[u8], dest: &mut C) -> Result<usize>
pub fn upper_bound(data: &[u8]) -> Option<usize>  // read the frame's declared content size
```

`Decompressor::decompress` needs a `capacity` — the max bytes you'll accept. It
errors rather than growing past it, which is exactly the bound you want against a
hostile or corrupt blob. `Decompressor::upper_bound(data)` reads the size the
frame *claims*; treat it as a hint to size the buffer, not as trusted truth.

## The gotcha that bites everyone: finish the encoder

A streaming `Encoder` buffers a final epilogue (the frame's content checksum and
end marker). That epilogue is written only when you call `finish()` (or
`try_finish()`). **If the `Encoder` is just dropped — including via an early
`return`, a `?` that bails further down, or a panic — the frame is truncated and
the file won't decompress.** This is the single most common zstd bug.

Two ways to be safe:

- **Call `finish()` explicitly** and let `?` propagate its result. This is the
  clearest choice in a save function, because `finish()` is also where late I/O
  errors (a full disk on the final flush) surface — you want to see them.
- **Use `auto_finish()`** to get an `AutoFinishEncoder` whose `Drop` calls
  `finish` for you. Convenient when the encoder's lifetime is tangled in other
  control flow — but `Drop` can't return a `Result`, so a finish error there is
  swallowed. Don't use `auto_finish()` on the project save path where reporting
  a write failure matters.

`decode_all` and `Decoder` have no matching footgun — there's nothing to finish
on the read side.

## Compression level

```rust
pub const DEFAULT_COMPRESSION_LEVEL: i32 = 3;   // what level 0 maps to
pub fn compression_level_range() -> RangeInclusive<i32>;  // the accepted range, 1..=22
```

Levels run 1 (fastest, least ratio) to 22 (slowest, smallest); `0` means "use
the default," which is `3`. Higher levels above ~19 are the "ultra" tier and cost
disproportionately more time and memory for thin gains. For Pixhaus:

- **Project saves: 3 (the default).** Saves should feel instant; 3 is the
  ratio/speed sweet spot the format was built around. Don't crank it to 19
  "to make files smaller" without measuring — you'll stall the save for a few
  percent.
- **Don't hardcode a magic number.** Define the level as a named constant in the
  `io` crate so the format has one source of truth, rather than sprinkling `3`
  across call sites.

## Untrusted input

Decompression can amplify a tiny input into a huge output (a "zip bomb").
Pixhaus is single-user and file-based, but users open `.pixhaus` files from other
people and from plugins, so the bytes aren't automatically trustworthy. Two
guards:

- On the bulk path, `Decompressor::decompress(data, capacity)` already caps the
  output — pick a `capacity` from a sane project-size ceiling, not from the
  frame's self-reported size.
- On the streaming path, `Decoder::window_log_max(n)` caps the decompression
  window (and thus peak memory) and rejects frames that demand more. Set it when
  decoding anything you didn't write yourself.

## Dictionaries (`dict`, default `zdict_builder` feature)

Dictionaries pay off when you compress *many small, similar* payloads where
per-frame overhead dominates — think a pile of like-shaped tiles or thumbnails,
not one big project file (a large frame already finds its own redundancy). Train
once, then reuse a prepared dictionary across encoders/decoders:

```rust
// Train from samples (each &[u8] is one sample). dict_size in bytes, e.g. 110 * 1024.
pub fn from_samples<S: AsRef<[u8]>>(samples: &[S], dict_size: usize) -> io::Result<Vec<u8>>;

// Prepare once, reuse many times — cheaper than re-parsing raw dict bytes per call.
let cdict = zstd::dict::EncoderDictionary::copy(&dict_bytes, level); // 'static, owns a copy
let ddict = zstd::dict::DecoderDictionary::copy(&dict_bytes);

let mut c = zstd::bulk::Compressor::with_prepared_dictionary(&cdict)?;
let mut d = zstd::bulk::Decompressor::with_prepared_dictionary(&ddict)?;
```

The encode and decode sides must use the *same* dictionary bytes, so a dictionary
becomes part of your on-disk format — version it deliberately. If you're not sure
you need one, you don't.

## Threading: keep compression off the UI thread

zstd compression is CPU-bound, and a large project save can take real time. Per
the workspace async rules (`pixhaus-rust-conventions`), CPU-bound work runs on
`tokio::task::spawn_blocking`, and the egui update loop must never block on it —
hand the save/load to a blocking task and deliver the result back over a channel
the loop drains each frame. Do not call `encode_all` or `encoder.finish()` inline
in a `ui`/`logic` path on a multi-megabyte buffer; that freezes the frame.

The `zstdmt` feature (`Encoder::multithread`, `Compressor::multithread`)
parallelizes *one* compression across worker threads. That's a different axis from
spawn_blocking and usually unnecessary for a single project file — reach for it
only if profiling shows save time on a huge document is the bottleneck, and keep
it off the UI thread regardless.

## Errors

The `zstd` crate surfaces failures as `std::io::Error` (its `Result` alias is
`io::Result`). In the `io` library crate, map these into the crate's `thiserror`
error type with `#[from]` rather than leaking `io::Error` through public APIs;
`anyhow` stays in the binary only. Never `unwrap()` a compress/decompress result
outside tests — a corrupt file is a user-facing error to report, not a panic.

## Decision shortcut

```
Compressing/decompressing in Pixhaus?
├─ It's the .pixhaus project file (msgpack + zstd, possibly large at 8K)?
│    └─ stream: write::Encoder + rmp_serde::encode::write + finish()  (read: read::Decoder)
│       and run it on spawn_blocking, never the UI thread.
├─ Data already a Vec<u8>, small/medium, one shot?
│    └─ zstd::encode_all(bytes, 3) / zstd::decode_all(bytes)
├─ Many small independent blobs, reusing a context or dictionary?
│    └─ bulk::Compressor / Decompressor (decompress needs a capacity bound)
└─ Decoding bytes you didn't write?  → add window_log_max / a capacity cap first.
```
