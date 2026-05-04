//! Read and write the `.pixhaus` project file format.
//!
//! The format uses a fixed 28-byte header followed by a zstd-compressed
//! `MessagePack` body. See `docs/file-format.md` for the full specification.
//!
//! # Quick start
//!
//! ```no_run
//! use pixhaus_io::pixhaus::{PixhausArchive, encode, decode};
//! use pixhaus_core::project::Project;
//!
//! // Round-trip in memory.
//! let archive = PixhausArchive::new(Project::new("my_sprite"));
//! let bytes = encode(&archive)?;
//! let loaded = decode(&bytes)?;
//! let _project = loaded.project;
//! # Ok::<(), pixhaus_io::error::Error>(())
//! ```
//!
//! For file-backed I/O, see [`encode_to_file`] and [`decode_from_file`].

mod migrate;
mod read;
mod schema;
mod write;

pub use read::{decode, decode_from_file};
pub use schema::{PixelBufferEntry, PixhausArchive};
pub use write::{encode, encode_to_file};
