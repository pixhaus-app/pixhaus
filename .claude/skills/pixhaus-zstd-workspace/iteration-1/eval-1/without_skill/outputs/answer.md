# zstd save bug: the encoder is never finished

## Root cause

The save function builds a `zstd::stream::write::Encoder`, writes the
MessagePack-serialized project into it, and then returns — letting the encoder
drop at the end of scope. It never calls `encoder.finish()`.

That is the bug. A zstd `Encoder` is a buffering writer. It does two things you
cannot skip:

1. It holds compressed output in an internal buffer and only emits a complete
   zstd block once it has enough input (or is told to flush).
2. It writes the zstd **frame epilogue** — the final-block marker plus the
   frame footer (and, depending on params, a content checksum) — only when the
   stream is closed.

Both of those happen in `finish()`. They do **not** happen in `Drop`. The zstd
`Encoder`'s `Drop` impl cannot flush or finalize, because closing the frame
performs I/O that can fail, and `drop` cannot return an error. So when the
encoder is dropped without `finish()`:

- any data still sitting in zstd's internal buffer is discarded, and
- the frame footer is never written.

On top of that, the encoder owns the `BufWriter<File>`. Dropping the encoder
does not guarantee the `BufWriter`'s own 8 KiB buffer is flushed to the file
either, and `BufWriter::drop` swallows any flush error.

## Why small projects look fine and large ones don't

It's not really about size crossing a magic threshold — it's that with more
data, more bytes are guaranteed to be stranded. zstd works in blocks. Once the
serialized project exceeds roughly an internal block's worth of data, a full
block plus the unwritten frame epilogue never reach the file. The loader's
`Decoder` then reads the partial frame, hits EOF before the frame is complete,
and reports the stream as truncated/corrupted — exactly the symptom reported.

A tiny project can happen to fit entirely in the buffers and, in some cases,
still decode, which is why small saves "work." That is luck, not correctness:
even small frames are missing their proper epilogue and should not be relied on.

## The fix

Call `finish()` to flush zstd and write the frame footer. `finish()` returns
the inner `BufWriter`, so flush that too (and `sync_all` the file so the bytes
are actually on disk before `save_project` reports success).

```rust
use std::fs::File;
use std::io::{BufWriter, Write};

pub fn save_project(project: &Project, path: &Path) -> Result<(), SaveError> {
    let file = BufWriter::new(File::create(path)?);

    let mut encoder = zstd::stream::write::Encoder::new(file, 3)?;
    rmp_serde::encode::write(&mut encoder, project)?;

    // Flush zstd's buffer and write the frame epilogue. This is the step the
    // original code skipped. `finish()` hands back the inner BufWriter.
    let mut writer = encoder.finish()?;

    // Flush the BufWriter and fsync so the save is durable before we return Ok.
    writer.flush()?;
    writer.into_inner().map_err(|e| e.into_error())?.sync_all()?;

    Ok(())
}
```

Notes:

- `SaveError` needs a variant that carries `std::io::Error` (via `#[from]` on a
  `thiserror` enum) so the `?` on `finish()`, `flush()`, `sync_all()`, and the
  `BufWriter` `into_inner` error all convert cleanly. `rmp_serde`'s encode error
  needs its own variant too.
- `BufWriter::into_inner` returns `Result<File, IntoInnerError<BufWriter<File>>>`;
  call `.into_error()` on the error to get an `io::Error` for `?`.

### Alternative: `auto_finish()`

`Encoder::auto_finish()` returns an `AutoFinishEncoder` that calls `finish()` in
its `Drop`. It avoids the explicit `finish()` call:

```rust
let file = BufWriter::new(File::create(path)?);
let mut encoder = zstd::stream::write::Encoder::new(file, 3)?.auto_finish();
rmp_serde::encode::write(&mut encoder, project)?;
// finish() runs on drop here
```

Avoid this on a save path. Finalizing in `Drop` means any error from writing the
frame footer or flushing the file is **swallowed** — the save can fail silently
and the user still sees "saved." For a function whose whole job is to produce a
file that reopens, you want the finalize error to surface through `?`. Use
explicit `finish()`.
