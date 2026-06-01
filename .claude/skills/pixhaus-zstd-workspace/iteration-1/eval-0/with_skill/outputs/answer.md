# `.pixhaus` save/load

The `.pixhaus` format is the document model serialized to MessagePack with
`rmp-serde`, then run through zstd. Because we support 8K canvases the
uncompressed document is large, so we stream rather than buffer: serialize
MessagePack *directly into* a zstd encoder that writes *directly to* the file.
Neither the full uncompressed nor the full compressed buffer is ever
materialized.

```rust
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;

/// zstd level for project saves. Level 3 is zstd's default and the
/// ratio/speed sweet spot the format was built around — saves should feel
/// instant. One named constant so the format has a single source of truth;
/// don't sprinkle a magic `3` across call sites.
const PROJECT_COMPRESSION_LEVEL: i32 = 3;

/// Errors from reading or writing a `.pixhaus` file.
#[derive(Debug, thiserror::Error)]
pub enum ProjectIoError {
    /// Filesystem or zstd failure. The `zstd` crate surfaces its own errors as
    /// `std::io::Error`, so this arm also covers a corrupt or truncated frame.
    #[error("project file I/O failed: {0}")]
    Io(#[from] std::io::Error),

    /// MessagePack serialization failed while writing the document.
    #[error("failed to encode project: {0}")]
    Encode(#[from] rmp_serde::encode::Error),

    /// MessagePack deserialization failed while reading the document.
    #[error("failed to decode project: {0}")]
    Decode(#[from] rmp_serde::decode::Error),
}

/// Serialize `project` to MessagePack, compress with zstd, and write it to
/// `path` as a `.pixhaus` file. Fully streamed: nothing is buffered whole.
pub fn save_project<P: Serialize>(project: &P, path: &Path) -> Result<(), ProjectIoError> {
    let file = BufWriter::new(File::create(path)?);
    let mut encoder = zstd::stream::write::Encoder::new(file, PROJECT_COMPRESSION_LEVEL)?;

    // Writes MessagePack bytes straight through the encoder into the file.
    rmp_serde::encode::write(&mut encoder, project)?;

    // REQUIRED: flush the zstd epilogue (content checksum + end marker). If the
    // encoder is merely dropped — including via an early `?` above — the frame
    // is truncated and the file won't decompress. `finish()` is also where late
    // write errors (a full disk on the final flush) surface, so propagate it.
    encoder.finish()?;
    Ok(())
}

/// Read a `.pixhaus` file from `path`: decompress with zstd and deserialize the
/// MessagePack document. Fully streamed.
pub fn load_project<P: DeserializeOwned>(path: &Path) -> Result<P, ProjectIoError> {
    let file = BufReader::new(File::open(path)?);
    let decoder = zstd::stream::read::Decoder::new(file)?;
    let project = rmp_serde::decode::from_read(decoder)?;
    Ok(project)
}
```

## Key choices

- **Streaming, not `encode_all`/`decode_all`.** The skill's decision shortcut
  points the project-file path at `stream::write::Encoder` +
  `rmp_serde::encode::write` + `finish()` (read side: `stream::read::Decoder`).
  At 8K the uncompressed MessagePack buffer is large; streaming keeps both it and
  the compressed buffer out of memory. `encode_all` is for small/medium buffers
  you already hold in a `Vec<u8>` — not this.

- **Explicit `finish()`, never `auto_finish()`.** A streaming `Encoder` buffers
  a final epilogue written only on `finish()`. Drop it without finishing — an
  early `return`, a `?` that bails, a panic — and the frame is truncated and
  won't decompress. This is the single most common zstd bug. I call `finish()`
  explicitly so late I/O errors (full disk on the final flush) surface;
  `auto_finish()` swallows them in `Drop`, which is wrong on a save path where
  reporting a write failure matters.

- **Level 3 as a named constant.** Level 3 is zstd's default and the
  ratio/speed sweet spot; saves should feel instant. Cranking to 19 for a few
  percent smaller files stalls the save. One `const` rather than a scattered
  magic `3`.

- **`thiserror` with `#[from]`, no leaked `io::Error`.** Per the workspace rule
  (`thiserror` in library crates, `anyhow` only in the binary, no `unwrap()`),
  the `io` crate owns a `ProjectIoError`. zstd reports failures — including a
  corrupt/truncated frame — as `std::io::Error`, so the `Io` arm covers both
  filesystem and decompression errors. rmp-serde splits encode and decode into
  separate error types, so there are two arms; all three use `#[from]` for clean
  `?` propagation. No `Box<dyn Error>` in the public signature.

- **Generic bounds.** `save_project<P: Serialize>` and
  `load_project<P: DeserializeOwned>` match the task's stated `Project:
  Serialize + DeserializeOwned`, avoiding a premature `Box<dyn Trait>`.

## Notes for the call site (not part of these functions)

- **Run off the UI thread.** zstd compression is CPU-bound and a large save
  takes real time. Per the workspace async rules, hand `save_project`/
  `load_project` to `tokio::task::spawn_blocking` and deliver the result back
  over a channel the egui update loop drains each frame. Never call these inline
  on the update loop — it freezes the frame.

- **Untrusted input.** Users open `.pixhaus` files from other people and from
  plugins. When loading bytes you didn't write, guard against decompression
  bombs with `decoder.window_log_max(n)` before deserializing, which caps peak
  decode memory and rejects frames demanding more. I left it off the core
  function to keep the signature focused; add it (with a project-size-derived
  cap) if/when load accepts foreign files directly.
