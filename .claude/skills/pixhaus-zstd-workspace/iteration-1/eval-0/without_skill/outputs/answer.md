# `.pixhaus` save / load

```rust
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;

/// zstd level. 3 is the library default and the right starting point: it
/// already gives most of the ratio, and at 8K the encode cost matters as much
/// as the bytes saved. Bump later only if profiling on real projects says so.
const ZSTD_LEVEL: i32 = 3;

#[derive(Debug, Error)]
pub enum ProjectIoError {
    #[error("failed to open project file {path}")]
    Open {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to create project file {path}")]
    Create {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// zstd compression / decompression I/O. Covered by io::Error because the
    /// zstd stream wrappers surface their errors as std::io::Error.
    #[error("zstd stream error for {path}")]
    Zstd {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to serialize project to MessagePack")]
    Encode(#[source] rmp_serde::encode::Error),

    #[error("failed to deserialize project from MessagePack")]
    Decode(#[source] rmp_serde::decode::Error),
}

/// Serialize `project` to MessagePack, zstd-compress the stream, and write it
/// to `path`.
pub fn save_project<P: Serialize>(project: &P, path: &Path) -> Result<(), ProjectIoError> {
    let file = File::create(path).map_err(|source| ProjectIoError::Create {
        path: path.to_path_buf(),
        source,
    })?;

    // Buffer the file, then layer the zstd encoder on top. rmp-serde writes
    // many small chunks; the BufWriter keeps those from each becoming a syscall.
    let writer = BufWriter::new(file);
    let mut encoder =
        zstd::stream::write::Encoder::new(writer, ZSTD_LEVEL).map_err(|source| {
            ProjectIoError::Zstd {
                path: path.to_path_buf(),
                source,
            }
        })?;

    // Stream MessagePack straight into the encoder — never materialize the
    // whole serialized blob in memory, which at 8K would be large.
    rmp_serde::encode::write(&mut encoder, project).map_err(ProjectIoError::Encode)?;

    // finish() flushes the final zstd frame and returns the inner writer; the
    // error here is the one that catches a truncated/failed write.
    encoder
        .finish()
        .map_err(|source| ProjectIoError::Zstd {
            path: path.to_path_buf(),
            source,
        })?
        .into_inner()
        .map_err(|e| ProjectIoError::Zstd {
            path: path.to_path_buf(),
            source: e.into_error(),
        })?;

    Ok(())
}

/// Read `path`, zstd-decompress, and deserialize the MessagePack into a `P`.
pub fn load_project<P: DeserializeOwned>(path: &Path) -> Result<P, ProjectIoError> {
    let file = File::open(path).map_err(|source| ProjectIoError::Open {
        path: path.to_path_buf(),
        source,
    })?;

    let reader = BufReader::new(file);
    let decoder = zstd::stream::read::Decoder::new(reader).map_err(|source| {
        ProjectIoError::Zstd {
            path: path.to_path_buf(),
            source,
        }
    })?;

    // Decode straight from the decompressing reader. from_read pulls bytes as
    // it needs them, so the only large allocation is the Project itself.
    rmp_serde::decode::from_read(decoder).map_err(ProjectIoError::Decode)
}
```

## Key choices

- **Stream, don't buffer.** Both functions wire the serializer/deserializer
  directly onto the zstd stream (`encode::write` into `Encoder`,
  `decode::from_read` from `Decoder`). I never build a `Vec<u8>` of the full
  MessagePack blob. For 8K projects that intermediate buffer would be the
  largest allocation in the path, and there's no reason to pay for it.

- **`BufWriter` / `BufReader` under zstd.** rmp-serde emits lots of small
  writes and reads. Without buffering, each becomes a syscall. zstd's own
  buffering helps on the compressed side, but the file handle still wants a
  buffer; layering order is file → BufWriter → Encoder so the buffer sits
  between the OS and the compressor.

- **`encoder.finish()` is not optional.** zstd writes a frame epilogue on
  finish. Dropping the encoder without calling `finish()` can leave a
  truncated, undecodable file and swallow the write error. I call it explicitly
  and propagate the error, then `into_inner()` on the recovered `BufWriter` to
  surface any final flush failure.

- **Level 3.** That's zstd's default and the sane starting point — most of the
  ratio for a fraction of the time of the high levels. At 8K, encode time is a
  real cost on save, so I don't reach for level 19 without profiling evidence.
  Pulled out to a `const` so it's a one-line change.

- **`thiserror`, no `unwrap`, no `anyhow`.** Library crate, so the error is a
  concrete enum. Variants carry the offending `path` and a `#[source]` so the
  caller (and the shell's error reporting) gets a full chain. Encode/decode and
  zstd/I/O failures stay distinguishable — they need different messaging
  (corrupt file vs. permissions vs. wrong format).

- **Generic over `P`, not a concrete `Project`.** Matches the
  `Serialize + DeserializeOwned` assumption, keeps `io` from depending on
  `core`'s concrete type, and makes the functions trivially testable with a
  small stand-in struct.

One thing to watch: `zstd::stream::read::Decoder` will happily decompress an
adversarial file into a huge allocation. These are local user files, so it's
fine for now, but if `.pixhaus` ever travels untrusted, add a
`Decoder::with_window_size_limit` / a `take()` cap before deserializing.
