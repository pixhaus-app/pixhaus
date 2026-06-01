# Compressing cached thumbnail tiles

## Recommendation

Use `zstd::bulk::{Compressor, Decompressor}`, built once and reused across all
~800 tiles — not `encode_all`/`decode_all` per tile.

`encode_all` is the right tool when you have a single buffer in hand and want a
one-shot frame. But it allocates and tears down a fresh zstd context on every
call. Run it 800 times during a scrub and you pay that setup/teardown 800 times
on the hot path. `bulk::Compressor` holds one context alive across calls, so each
`compress` reuses the allocation instead of rebuilding it. Same on the read side
with `Decompressor`. Each `compress` call still emits a standalone, self-contained
zstd frame — exactly what you want for independent tiles you can read back in any
order — so you get the per-tile independence of `encode_all` without the
per-tile context churn.

Two more reasons this fits the timeline-scrub workload:

- **A capacity bound on decode.** `Decompressor::decompress(data, capacity)` caps
  the output size and errors rather than growing past it. A thumbnail tile has a
  known maximum (width × height × 4 for RGBA), so you have a tight, honest ceiling
  to pass — it doubles as a guard against a corrupt cache entry. `decode_all` has
  no such bound.
- **Room to add a dictionary later.** 800 small, similar payloads is the textbook
  case for a trained zstd dictionary. The bulk API takes a prepared dictionary via
  `with_prepared_dictionary` with no change to call sites; `encode_all` does not.
  You don't need one today — but starting on the bulk path keeps that door open.

What to keep in mind on each side:

- **The `Compressor` is `&mut self`** — `compress` mutates the reused context — so
  it isn't `Sync`. Keep one per worker thread, or guard it behind the owning
  thread. Don't share one `Compressor` across threads.
- **Pick the capacity from your own size ceiling, not the frame.**
  `Decompressor::upper_bound(data)` reads the size the frame *claims*; treat it as
  a sizing hint, not trusted truth. The real bound is your known tile dimensions.
- **There's no `finish()` footgun here.** That trap belongs to the streaming
  `Encoder` (drop it unfinished and the frame truncates). The bulk `compress` call
  returns a complete frame every time — nothing to finish, nothing to truncate.
- **Level 3 (the default) is the right starting point** for cache tiles you write
  and read constantly during a scrub; the goal is to feel instant, not to squeeze
  the last byte. Define it as a named constant rather than sprinkling `3`.

## Helpers

```rust
use std::io;

use zstd::bulk::{Compressor, Decompressor};

/// Compression level for cached thumbnail tiles. 3 is zstd's default — the
/// ratio/speed sweet spot. These tiles are written and read constantly while
/// scrubbing, so favor speed; don't crank this without measuring.
const TILE_COMPRESSION_LEVEL: i32 = 3;

/// A reusable codec for the thumbnail-tile cache. Build one and keep it for the
/// lifetime of the cache (one per worker thread — `Compressor` is `&mut self`,
/// so it is not `Sync`). Reusing the contexts avoids re-allocating the zstd
/// state on every one of the ~800 tiles touched during a scrub.
pub struct TileCodec {
    compressor: Compressor<'static>,
    decompressor: Decompressor<'static>,
}

impl TileCodec {
    /// Build a codec at the default tile compression level.
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            compressor: Compressor::new(TILE_COMPRESSION_LEVEL)?,
            decompressor: Decompressor::new()?,
        })
    }

    /// Compress one tile's RGBA bytes into a standalone zstd frame.
    ///
    /// Each call produces an independent frame, so tiles can be written and
    /// read back in any order. Reuses the held compression context.
    pub fn compress_tile(&mut self, rgba: &[u8]) -> io::Result<Vec<u8>> {
        self.compressor.compress(rgba)
    }

    /// Decompress one tile frame back to RGBA bytes.
    ///
    /// `max_decompressed_len` is the largest output you'll accept — pass the
    /// tile's known size (width * height * 4). `decompress` errors rather than
    /// growing past it, which bounds memory against a corrupt cache entry.
    pub fn decompress_tile(
        &mut self,
        frame: &[u8],
        max_decompressed_len: usize,
    ) -> io::Result<Vec<u8>> {
        self.decompressor.decompress(frame, max_decompressed_len)
    }
}
```

### Notes for the `io` crate

- The `zstd` crate returns `io::Result`. In a library crate, map these into the
  crate's `thiserror` type with `#[from]` rather than leaking `io::Error` through
  public APIs (`anyhow` stays in the binary). The signatures above show
  `io::Result` for clarity; wrap accordingly at the crate boundary.
- Never `unwrap()` a compress/decompress result outside tests — a corrupt cache
  entry is an error to handle (drop and regenerate the thumbnail), not a panic.
- If profiling later shows per-frame overhead dominates across the 800 similar
  tiles, train a dictionary (`zstd::dict::from_samples`) once and build the codec
  with `Compressor::with_prepared_dictionary` / `Decompressor::with_prepared_dictionary`.
  The encode and decode sides must use the *same* dictionary bytes, so version it
  as part of the cache format. No call-site changes beyond construction.
```
