# The truncated-save bug

## Root cause: the zstd encoder is never finished

```rust
let mut encoder = zstd::stream::write::Encoder::new(file, 3)?;
rmp_serde::encode::write(&mut encoder, project)?;
Ok(()) // <-- encoder dropped here, finish() never called
```

`zstd::stream::write::Encoder` is a *streaming* encoder. As bytes are written
through it, it emits compressed blocks, but it holds back a final epilogue — the
frame's end marker (and content checksum). That epilogue is written only when you
call `encoder.finish()` (or `try_finish()`).

This function never calls `finish()`. When it returns `Ok(())`, the `encoder` is
dropped. Dropping a bare `Encoder` does **not** finish the frame — it just frees
it. So the file on disk is a zstd frame with no end marker: a truncated frame.
The loader's `Decoder` reads to what should be the end, finds no terminator, and
errors exactly as if the file were cut short. This is the single most common zstd
mistake, and the symptom — "the data looks truncated" — is its signature.

## Why small projects survive and large ones fail

The bug is always present, but whether the file happens to decode depends on how
much got flushed before the drop:

- A streaming `Encoder` (and the `BufWriter` underneath it) buffers internally.
  For a **small** project, all the compressed output can still be sitting in
  those buffers when `write` returns. Dropping the encoder flushes some of that
  through `Write`, and a small enough single-block frame can sometimes still be
  read back — the loader gets lucky and never needs the missing epilogue.
- For a **large** project, the encoder has already flushed multiple complete
  blocks to the file during serialization, and the very last block plus the
  end-of-frame epilogue are the parts that the un-called `finish()` would have
  written. Those never land, so the frame is genuinely incomplete and the loader
  reliably rejects it as truncated.

That size dependence is the tell: it's not data corruption, it's a missing
finalize step that only shows once the output crosses zstd's internal buffer
boundaries.

A second, related hole: even if you finish the encoder, the inner `BufWriter`'s
last flush can fail (for example, a full disk on the final write). `finish()`
returns the inner writer; you should flush it explicitly so that late I/O error
surfaces instead of being swallowed by a silent drop.

## The fix

Call `finish()` and propagate its result, then flush the returned writer. Do not
reach for `auto_finish()` here — its `Drop`-based finish can't return a `Result`,
so a final write failure on the save path would be swallowed, which is the last
thing you want when persisting a user's project.

```rust
use std::fs::File;
use std::io::{BufWriter, Write};

pub fn save_project(project: &Project, path: &Path) -> Result<(), SaveError> {
    let file = BufWriter::new(File::create(path)?);
    let mut encoder = zstd::stream::write::Encoder::new(file, 3)?;
    rmp_serde::encode::write(&mut encoder, project)?;

    // Write the zstd frame epilogue (end marker + checksum). Without this the
    // frame is truncated and the loader rejects it. finish() hands back the
    // inner BufWriter so we can flush it and surface any late write error.
    let mut writer = encoder.finish()?;
    writer.flush()?;
    Ok(())
}
```

### Notes for the real `io` crate

- `SaveError` needs `From<std::io::Error>` (the `?` on `File::create`,
  `Encoder::new`, `finish`, and `flush`) and `From<rmp_serde::encode::Error>`.
  Wrap both with `#[from]` on a `thiserror` enum; don't leak `io::Error` through
  the public API, and keep `anyhow` in the binary only.
- The literal `3` should be a named constant in the `io` crate (the format's one
  source of truth for compression level), not sprinkled across call sites.
- This is CPU-bound. Don't call it inline on the egui update loop — run it on
  `tokio::task::spawn_blocking` and deliver the result back over a channel the
  loop drains each frame.
