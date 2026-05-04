# .pixhaus file format

Version 1.0 — written by `pixhaus-io` crate, implemented in `io/src/pixhaus/`.

A `.pixhaus` file is a MessagePack-serialized Pixhaus project with its pixel buffers, compressed with zstd and prefixed by a fixed binary header. The format is self-describing: a reader can determine version and required features from the header alone before touching the body.

## Overview

```
[header: 28 bytes]
[body: N bytes — zstd-compressed MessagePack of PixhausArchive]
```

The header is always uncompressed and always exactly 28 bytes. The body follows immediately at offset 28.

## Header layout

```
Offset  Size  Type    Field
     0     8  bytes   magic
     8     2  u16 BE  format_major
    10     2  u16 BE  format_minor
    12     4  u32 BE  feature_flags
    16     4  u32 BE  required_flags
    20     8  u64 BE  body_len
    28     N  bytes   body (zstd-compressed)
```

### magic

Eight bytes: the ASCII text `PIXHAUS` followed by a null byte (`0x50 0x49 0x58 0x48 0x41 0x55 0x53 0x00`). A reader must reject any file whose first eight bytes differ.

### format_major / format_minor

The format version. `format_major` is 1; `format_minor` is 0.

**Compatibility rules:**

- A reader must reject files whose `format_major` exceeds its own. Major bumps are breaking changes.
- A reader may load files whose `format_minor` exceeds its own. Minor bumps are additive; the reader ignores fields it doesn't recognise.
- A writer always writes `FORMAT_MAJOR = 1` and `FORMAT_MINOR = 0` until the spec is revised.

### feature_flags

Bitmask (`u32 BE`) of optional features present in the file body. Bits map to features in the Pixhaus data model:

| Bit | Constant           | Meaning                                               |
|-----|--------------------|-------------------------------------------------------|
|   0 | `TILEMAPS`         | File contains tilemap layers and tilesets             |
|   1 | `REFERENCES`       | File contains reference layers (non-exported)         |
|   2 | `ANIMATIONS`       | File contains animation entries beyond raw frame tags |
|   3 | `SLICES`           | File contains slices with nine-slice or pivot data    |
|   4 | `VERB_HISTORY`     | File contains AI verb history                         |
| 5–31 | —               | Reserved; writers must leave these bits zero          |

This field is advisory — the authoritative flags are inside `PixhausArchive.project.feature_flags` in the body. Readers use the body copy.

### required_flags

Bitmask (`u32 BE`) of features a reader must understand to open the file correctly. A bit set in `required_flags` but absent in `feature_flags` is invalid. Bits use the same mapping as `feature_flags`.

**Reading rule:** compute `unknown = required_flags & ~KNOWN_FLAGS`. If `unknown != 0`, reject the file with an `UnknownRequiredFeatures` error before decompressing the body.

`KNOWN_FLAGS` for format version 1.0 is `0x0000001F` (all five currently defined bits).

In version 1.0, the writer sets `required_flags = feature_flags`. Future minor versions may introduce optional features where `feature_flags` is set but `required_flags` is not, so older readers can still open those files gracefully.

### body_len

Length of the compressed body in bytes (`u64 BE`). The compressed body runs from offset 28 to offset `28 + body_len`. Bytes beyond that position are undefined (and in practice absent).

A reader must reject the file if the actual byte slice is shorter than `28 + body_len`.

## Body

The body is the `zstd`-compressed MessagePack serialization of a `PixhausArchive`:

```
PixhausArchive {
    project: Project,          // full data model (B2)
    buffers: Vec<PixelBufferEntry>,
}

PixelBufferEntry {
    id:     u32,   // matches PixelBufferId values in the project
    width:  u32,
    height: u32,
    stride: u32,   // bytes per row; may be wider than width * bpc for alignment
    pixels: Vec<u8>,
}
```

### Compression

zstd level 3 (the library default) is used when writing. Readers use whatever level was stored — zstd frames are self-describing and include the original size. The writer does not additionally compress individual pixel buffers; the zstd pass over the whole body is sufficient.

### Pixel buffer bytes

`pixels` is raw channel bytes. Interpretation is implied by the color mode of the sprite that owns the referencing cel:

- `ColorMode::Rgba` — 4 bytes per pixel (R, G, B, A)
- `ColorMode::Grayscale` — 1 byte per pixel (luminance)
- `ColorMode::Indexed` — 1 byte per pixel (palette index)

Stride may exceed `width * bytes_per_channel` for alignment. Readers must step by `stride` bytes per row, not `width * bpc`.

A `PixelBufferEntry` whose `pixels` vec is empty is valid and means "no content" (e.g. an empty reference layer). Zero-sized buffers are still indexed so the project's IDs remain stable.

### MessagePack encoding

The body uses named MessagePack maps (`to_vec_named` / `from_slice`). Fields serialise with their Rust field names. Serde attributes on the core types (`skip_serializing_if`, `default`, enum `tag`) apply as declared in `pixhaus-core`.

Enum variants use the `tag = "kind"` + `rename_all = "snake_case"` convention established by B2. A reader encountering an unknown `kind` value in a tagged enum should return a deserialisation error rather than silently skipping the cel or layer.

## Schema evolution

| Change type | `format_major` | `format_minor` | Notes |
|---|---|---|---|
| Add optional field with `#[serde(default)]` | unchanged | bump | Old readers silently skip the field |
| Add new enum variant | unchanged | bump | Old readers will error if they encounter it |
| Add a new required feature flag | unchanged | bump | Set the bit in `required_flags` so old readers reject the file |
| Add a new optional feature flag | unchanged | bump | Set the bit in `feature_flags` only; old readers open the file safely |
| Remove or rename a field | bump | reset to 0 | Requires a migration or fallback in the reader |
| Change the encoding of an existing type | bump | reset to 0 | No silent data corruption across versions |

Breaking changes (major bump) require a documented migration strategy before shipping. The migration lives in `io/src/pixhaus/migrate.rs` (created when the first migration is needed).

## File size

Typical size targets (zstd level 3 vs. equivalent `.aseprite` files):

- Projects with no pixel data: < 1 kB
- 32×32 sprite, 8 frames, RGBA: within 1.2× of `.aseprite`
- 256×256 sprite, 16 frames, indexed: within 1.1× of `.aseprite`

The file size goal is within 1.5× of equivalent `.aseprite` files for typical pixel art. zstd at level 3 outperforms zlib (used by `.aseprite`) at similar CPU cost, so this target is easily met.

## Forward compatibility example

Suppose format 1.1 adds an optional `thumbnail` field to `PixhausArchive`:

1. The writer sets bit 5 in `feature_flags` (a new `THUMBNAIL` constant).
2. The writer does NOT set bit 5 in `required_flags` — thumbnails are optional.
3. A 1.0 reader: `required_flags & !KNOWN_FLAGS == 0` (bit 5 is not required), so it proceeds. Serde silently skips unknown map keys by default, so the 1.0 reader ignores `thumbnail`.
4. A 1.1 reader: reads the thumbnail normally. The mirror direction — a 1.1 reader loading a 1.0 file — relies on `#[serde(default)]` on the new `thumbnail` field so the missing-key case yields `None` instead of a deserialization error.

## Quick reference

```
# Write
cargo doc -p pixhaus-io --open
# encodes: pixhaus_io::pixhaus::encode(&archive)
# to file: pixhaus_io::pixhaus::encode_to_file(&archive, "project.pixhaus")

# Read
# decodes: pixhaus_io::pixhaus::decode(&bytes)
# from file: pixhaus_io::pixhaus::decode_from_file("project.pixhaus")
```

Both functions are blocking. Wrap in `tokio::task::spawn_blocking` when calling from an async context (B4 IPC commands do this).
