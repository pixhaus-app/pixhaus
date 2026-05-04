//! Read and write the `.pixhaus` project file format.
//!
//! The format uses a fixed 28-byte header followed by a zstd-compressed
//! `MessagePack` body. See `docs/file-format.md` for the full specification.
//!
//! # Quick start
//!
//! ```no_run
//! use pixhaus_io::pixhaus::{PixhausArchive, encode, decode_from_file};
//! use pixhaus_core::project::Project;
//!
//! // Write
//! let archive = PixhausArchive::new(Project::new("my_sprite"));
//! let bytes = encode(&archive).expect("encode failed");
//!
//! // Read
//! let loaded = decode_from_file("my_sprite.pixhaus").expect("decode failed");
//! let _project = loaded.project;
//! ```

mod read;
mod schema;
mod write;

pub use read::{decode, decode_from_file};
pub use schema::{PixelBufferEntry, PixhausArchive};
pub use write::{encode, encode_to_file};
