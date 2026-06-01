# Compressing many small thumbnail tiles with zstd

## Recommendation

Use a **dictionary trained once over a sample of your tiles**, then compress and
decompress each tile through the dictionary-aware bulk APIs:
`zstd::dict::from_samples` (or `zstd::dict::from_continuous`) to build the
dictionary, and `zstd::bulk::Compressor` / `zstd::bulk::Decompressor` to run each
tile through it.

`encode_all` / `decode_all` (the convenience functions, equivalent to
`bulk::compress` / `bulk::decompress`) still work tile-by-tile, but they leave
two wins on the table for this exact workload:

1. **A dictionary fixes the small-input problem.** zstd's ratio comes from
   back-references into already-seen data. A 4 KB tile gives the compressor
   almost no history to reference, so per-tile `encode_all` compresses each tile
   essentially cold and the ratio is poor. ~800 thumbnail tiles from the same
   timeline are highly self-similar (same palette, similar gradients, lots of
   shared RGBA runs). A dictionary seeds every tile's window with that shared
   content, so each tiny tile compresses as if it had already seen hundreds of
   its neighbors. This is the canonical "many small, similar blobs" case the
   zstd dictionary feature exists for.

2. **Reuse the context, skip per-call setup.** `bulk::Compressor` and
   `bulk::Decompressor` own a reusable `CCtx`/`DCtx`. Creating one per tile (which
   is what `encode_all` does internally on every call) re-allocates and re-inits
   the working state 800 times per scrub pass. Building one `Compressor`/
   `Decompressor` and calling `.compress(...)` / `.decompress(...)` in a loop
   reuses that allocation across all tiles — meaningfully cheaper when you're
   doing this repeatedly while the user drags the playhead.

Each tile still produces an **independent, self-contained compressed frame** —
you can read/write/evict any single tile without touching the others. The only
shared, immutable thing is the dictionary, which you persist alongside the cache
(or rebuild on startup from current tiles). The decompressor must use the same
dictionary the tile was compressed with.

Note the constraint: a dictionary-compressed frame can only be decompressed with
that dictionary. Version the dictionary (or store a hash of it) so a stale
dictionary can't silently corrupt reads — if the dictionary changes, the old
tiles are unreadable. Simplest policy: build the dictionary once per cache
generation and treat a dictionary change as a cache invalidation.

If for some reason you don't want to manage a dictionary at all, the smaller
upgrade is still to swap the bare `encode_all`/`decode_all` calls for reused
`bulk::Compressor`/`bulk::Decompressor` instances to recover win #2.

## Helpers

```rust
use std::io;

use zstd::bulk::{Compressor, Decompressor};
use zstd::dict::{DecoderDictionary, EncoderDictionary};

/// Compression level for cache tiles. Cache data is transient and we rewrite it
/// constantly while scrubbing, so favor speed over the last few percent of ratio.
const TILE_LEVEL: i32 = 3;

/// Build a zstd dictionary from a representative sample of tiles.
///
/// Train this once over a batch of existing tiles (a few hundred is plenty),
/// persist the bytes next to the cache, and reuse it for every compress and
/// decompress. The dictionary is the *only* shared state between tiles; each
/// compressed tile remains an independent frame.
///
/// `max_size` caps the dictionary in bytes — ~16 KiB to 64 KiB is a sane range
/// for KB-sized tiles.
pub fn train_tile_dictionary(
    samples: &[Vec<u8>],
    max_size: usize,
) -> io::Result<Vec<u8>> {
    // `from_samples` takes any slice of byte slices.
    let refs: Vec<&[u8]> = samples.iter().map(Vec::as_slice).collect();
    zstd::dict::from_samples(&refs, max_size)
}

/// A reusable compressor bound to the tile dictionary.
///
/// Hold one of these on the cache-writer side and call `compress` per tile. The
/// underlying `CCtx` and prepared dictionary are reused across every call, so we
/// don't re-init compressor state 800 times per scrub.
pub struct TileCompressor {
    inner: Compressor<'static>,
}

impl TileCompressor {
    pub fn new(dictionary: &[u8]) -> io::Result<Self> {
        // Prepare (digest) the dictionary once; the compressor keeps it.
        let prepared = EncoderDictionary::copy(dictionary, TILE_LEVEL);
        let inner = Compressor::with_prepared_dictionary(&prepared)?;
        Ok(Self { inner })
    }

    /// Compress one tile into a fresh, self-contained frame.
    pub fn compress(&mut self, tile: &[u8]) -> io::Result<Vec<u8>> {
        self.inner.compress(tile)
    }
}

/// A reusable decompressor bound to the same tile dictionary.
///
/// Must use the dictionary the tiles were compressed with — a mismatch fails
/// the decode rather than returning garbage.
pub struct TileDecompressor {
    inner: Decompressor<'static>,
}

impl TileDecompressor {
    pub fn new(dictionary: &[u8]) -> io::Result<Self> {
        let prepared = DecoderDictionary::copy(dictionary);
        let inner = Decompressor::with_prepared_dictionary(&prepared)?;
        Ok(Self { inner })
    }

    /// Decompress one tile.
    ///
    /// `capacity` is an upper bound on the decompressed size used to size the
    /// output buffer. For fixed-geometry RGBA tiles this is exactly
    /// `width * height * 4`, so pass that — no guessing, no realloc.
    pub fn decompress(&mut self, frame: &[u8], capacity: usize) -> io::Result<Vec<u8>> {
        self.inner.decompress(frame, capacity)
    }
}
```

### Usage sketch

```rust
// Once, when the cache generation is established:
let dict = train_tile_dictionary(&sample_tiles, 32 * 1024)?;
// ...persist `dict` alongside the cache (with a version/hash).

// On the write side (reused across all tiles):
let mut enc = TileCompressor::new(&dict)?;
for tile in tiles_to_persist {
    let frame = enc.compress(&tile.rgba)?;
    write_tile_to_disk(tile.id, &frame)?;
}

// On the read side while scrubbing (reused across all tiles):
let mut dec = TileDecompressor::new(&dict)?;
let rgba = dec.decompress(&frame, tile_w * tile_h * 4)?;
```

### Why not the streaming `Encoder`/`Decoder` (`zstd::stream`)?

The streaming API shines for one large, unbounded byte stream. Here every tile
is a small, fully-in-memory `Vec<u8>` with a known size, and you want one
independent frame per tile. The one-shot `bulk` path is the right fit — no
`Write`/`Read` plumbing, exact-capacity output buffers, and the same reusable
context benefit. Reach for `stream` only if you later decide to pack the whole
tile cache into one concatenated stream, which would trade per-tile random access
for a better overall ratio.
```
