---
name: pixhaus-rmp-serde
description: >
  Use when serializing or deserializing anything to MessagePack in Pixhaus with
  `rmp-serde` — above all the `.pixhaus` project file format (MessagePack + zstd)
  in the `io` crate, but also any blob you persist or send. Trigger this for ANY
  "save/load the project", "write the file format", "serialize this struct",
  "round-trip to bytes", "encode the document", or "why is my saved file huge"
  request, and whenever you see `rmp_serde`, `to_vec`, `to_vec_named`,
  `from_slice`, `from_read`, `Serializer`, `serde_bytes`, or `#[serde(...)]` on a
  persisted type. rmp-serde has two traps that silently wreck a file format —
  structs default to position-dependent arrays (no field names, brittle to schema
  change) and `Vec<u8>` defaults to a sequence of integers (catastrophic for pixel
  buffers) — so reach for this skill rather than relying on memory.
---

# rmp-serde for Pixhaus

rmp-serde is the Serde data format for MessagePack — a compact, self-describing
binary encoding. It is how Pixhaus turns the in-memory document into bytes for the
`.pixhaus` file (MessagePack, then zstd) and back. The crate is small; the API you
touch is four functions and one builder. What's worth getting right is two
defaults that quietly sabotage a durable file format if you don't override them.

For the exhaustive API — every function signature, both `Error` enums, the
`config` types, the full MessagePack type mapping — read
`references/api-reference.md`. This file is the decisions and the patterns.

## Version and license

| Crate | Version | License |
|---|---|---|
| `rmp-serde` | 1.3 | MIT |
| `serde_bytes` | 0.11 | `MIT OR Apache-2.0` |

Both pass the workspace MIT lock and `cargo deny`. `serde_bytes` is a separate
crate you add explicitly; it is not bundled with rmp-serde.

```toml
rmp-serde  = "1.3"
serde      = { version = "1", features = ["derive"] }
serde_bytes = "0.11"   # for pixel buffers and any other byte blob
```

## Decision 1: named maps for anything that persists

rmp-serde gives you two struct encodings, and the convenient default is the wrong
one for a file format:

- **`to_vec` / `write` / `Serializer::new`** — structs serialize as **arrays**:
  just the field values, in declaration order, no names. Compact and fast. But the
  schema lives entirely in field *position*. Add a field, remove one, or reorder,
  and every previously written file decodes into garbage or fails outright. There
  is no way to be lenient because there are no names to match on.
- **`to_vec_named` / `write_named` / `Serializer::with_struct_map`** — structs
  serialize as **maps** keyed by field name. Bigger on the wire (every field name
  is repeated), but the decoder matches by name, so you can add fields, reorder
  them, and skip unknown ones.

**For the `.pixhaus` format and anything else that outlives one run, use the named
form.** A project file is read by future versions of Pixhaus; it has to survive the
document model growing new fields. Named maps plus `#[serde(default)]` give you
forward-compatible saves for free:

```rust
#[derive(Serialize, Deserialize)]
struct ProjectFile {
    version: u32,
    canvas: Canvas,
    layers: Vec<Layer>,
    #[serde(default)]          // older files lack this; decode them as Default
    onion_skin: OnionSkin,
}

// Save: named so the schema can grow.
let bytes = rmp_serde::to_vec_named(&project)?;

// Load: matches by field name, fills missing fields from #[serde(default)].
let project: ProjectFile = rmp_serde::from_slice(&bytes)?;
```

The named tax — repeated field names — is small relative to the bulk of a sprite
file (the pixel data), and zstd compresses those repeated keys hard. The size win
of arrays is not worth a file format you can never evolve.

Use compact arrays (`to_vec`) only for ephemeral, internal, schema-frozen data
where size dominates and nothing reads it later — a cache entry within one run, an
undo snapshot, an IPC frame between two builds of the same binary. When in doubt,
named.

## Decision 2: byte buffers must use serde_bytes

This is the one that turns a 4 MB layer into a 12 MB one and tanks save time.

Serde has no dedicated bytes concept, so a plain `Vec<u8>` serializes as a
**sequence of integers** — rmp-serde writes a marker byte (sometimes more) for
*every single byte* of your buffer. Pixhaus pixel buffers are `Vec<u8>` (RGBA, an
explicit stride — see CLAUDE.md), and at the 8192x8192 ceiling the project targets
that is 256 MB of pixels per layer encoded as a quarter-billion tiny integers. It
is slow to write, slow to read, and huge before zstd ever sees it.

Fix it by telling Serde the field is bytes, with `serde_bytes`:

```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct Layer {
    width: u32,
    height: u32,
    #[serde(with = "serde_bytes")]   // <- the whole game
    pixels: Vec<u8>,                 // now one MessagePack `bin` blob, not N ints
}
```

That annotation makes the field encode as a single MessagePack `bin` value: one
length header, then the raw bytes. It works with **both** `to_vec` and
`to_vec_named` and with the free functions — no custom `Serializer` needed — which
is exactly why it is the preferred fix.

The alternative, `Serializer::with_bytes(BytesMode::ForceIterables)` or
`ForceAll`, flips *every* `u8` container in the value to `bin` without
annotations. It is blunt: it changes the wire format globally, can break
`Deserialize` impls that expected a sequence, and only round-trips if the reader
uses the matching mode. Reach for it only when you cannot annotate the types (for
example serializing a foreign type you don't own). Default to per-field
`serde_bytes`. `BytesMode::Normal` (the default) emits `bin` exactly when
`serde_bytes` asks — keep it.

## The save/load path with zstd

The format is MessagePack then zstd. Stream rather than building a giant
intermediate `Vec`: wrap the zstd encoder as the writer and let rmp-serde's
`write_named` push straight through it, so the full uncompressed buffer never
exists at once. Mirror it on read. For the zstd side specifically — levels,
window size, `finish()` semantics, the `zstd` vs `zstd-safe` layers — see the
`pixhaus-zstd` skill; here we only show how rmp-serde plugs into it.

```rust
use std::io::{Read, Write};

// Write: document -> MessagePack -> zstd -> w.  Streamed, no full intermediate buffer.
pub fn write_project<W: Write>(w: W, project: &ProjectFile) -> Result<(), SaveError> {
    let mut zstd = zstd::stream::write::Encoder::new(w, ZSTD_LEVEL)?;
    rmp_serde::encode::write_named(&mut zstd, project)?; // named: schema can evolve
    zstd.finish()?;                                      // flush the zstd frame
    Ok(())
}

// Read: r -> un-zstd -> MessagePack -> document. from_read because we own the data.
pub fn read_project<R: Read>(r: R) -> Result<ProjectFile, LoadError> {
    let zstd = zstd::stream::read::Decoder::new(r)?;
    let project = rmp_serde::from_read(zstd)?;
    Ok(project)
}
```

`from_slice` vs `from_read`: use `from_slice` when the whole encoded blob is
already a `&[u8]` in memory and the target can borrow from it (zero-copy `&str` /
`&[u8]` fields). Use `from_read` for a streaming source like the zstd decoder
above; it requires `DeserializeOwned`, so the result owns all its data and borrows
nothing from the stream.

## Error handling

Encode and decode have **separate** `Error` types (`rmp_serde::encode::Error` and
`rmp_serde::decode::Error`) — don't try to unify them under one alias. In the `io`
library crate, wrap them in a `thiserror` enum and propagate with `?`; never
`unwrap`. `anyhow` is for the binary only (CLAUDE.md, `pixhaus-rust-conventions`).

```rust
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("decompressing project")]
    Zstd(#[from] std::io::Error),
    #[error("decoding MessagePack")]
    Decode(#[from] rmp_serde::decode::Error),
}
```

The decode variant you will actually hit in tests is `TypeMismatch` (the bytes
don't match the target type — usually a schema mismatch or a corrupt file) and
`InvalidDataRead` wrapping an `UnexpectedEof` (truncated file). See the reference
for the full variant list.

## Gotchas worth internalizing

- **`is_human_readable()` is `false` for MessagePack.** Some types in the
  ecosystem (and your own `#[serde(...)]` impls) branch on this — they emit a
  string for humans and a compact form for binary. With rmp-serde you get the
  binary branch by default. Only call `with_human_readable()` if you have a
  specific reason, and then it must match on both ends.
- **Enums are externally tagged: `{ variant_name => payload }`.** Renaming a
  variant breaks old files the same way reordering struct fields does. Treat
  variant names in persisted enums as part of the format; use
  `#[serde(rename = "...")]` to pin them if you rename the Rust identifier.
- **The free functions decode exactly one message and ignore trailing bytes.** If
  you concatenate messages or need to reject junk after the value, drive a
  `Deserializer` directly rather than calling `from_slice` once.
- **Round-trip in tests, and test schema evolution explicitly.** A unit test that
  serializes then deserializes catches the obvious breaks. Add a test that decodes
  a stored fixture from an older schema (a checked-in byte blob) so a careless
  field reorder fails loudly. See `pixhaus-testing-conventions`; `insta` is handy
  for asserting on a hex/debug view of the encoded bytes.

## Decision shortcut

```
Serializing something with rmp-serde?
├─ Does it persist to disk or cross versions (the .pixhaus file, any saved blob)?
│    └─ yes → to_vec_named / write_named. Add #[serde(default)] to new fields.
│            Compact arrays (to_vec) only for in-run, schema-frozen, size-critical data.
├─ Does the type hold a Vec<u8> / &[u8] (pixels, masks, thumbnails, any blob)?
│    └─ yes → #[serde(with = "serde_bytes")] on that field. Always. Non-negotiable for pixels.
├─ Reading from an in-memory &[u8] and want zero-copy borrows?
│    └─ from_slice.   Reading from a stream (zstd decoder, file)? → from_read.
└─ Need BytesMode::Force* ? → only for foreign types you can't annotate. Otherwise serde_bytes.
```
