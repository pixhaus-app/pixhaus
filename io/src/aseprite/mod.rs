//! Read and write Aseprite `.aseprite` / `.ase` files.
//!
//! The compatibility contract — which chunk types are supported, what
//! survives a round-trip, what is dropped — lives in
//! `docs/aseprite-compat.md` (B7). The reader and writer here mirror
//! that contract; if the doc and the code diverge, the doc wins until
//! a planning revision is recorded.
//!
//! # Module shape
//!
//! - [`spec`] — magic numbers, chunk codes, blend-mode mapping.
//! - [`byteio`] — little-endian readers and writers over a borrowed slice.
//! - [`chunk`] — parsed-chunk variants ([`Chunk`]) and per-chunk types.
//! - [`document`] — [`AsepriteDocument`], the in-memory shape one step
//!   above raw bytes.
//! - [`read`] — [`decode`] / [`decode_from_file`].
//! - [`mod@write`] — [`encode`] / [`encode_to_file`].
//! - [`archive`] — [`document_to_archive`] / [`archive_to_document`]
//!   for translating to and from [`crate::pixhaus::PixhausArchive`].
//!
//! # Quick start
//!
//! ```no_run
//! use pixhaus_io::aseprite::{
//!     archive_to_document, decode, document_to_archive, encode,
//! };
//! use pixhaus_io::pixhaus::PixhausArchive;
//! use pixhaus_core::project::Project;
//!
//! // Read an .aseprite file into a Pixhaus archive.
//! let bytes = std::fs::read("hero.aseprite")?;
//! let document = decode(&bytes)?;
//! let converted = document_to_archive(&document, "hero")?;
//! // converted.warnings carries any non-fatal compatibility notes.
//!
//! // Write it back out.
//! let document_again = archive_to_document(&converted.archive);
//! let bytes_again = encode(&document_again)?;
//! std::fs::write("hero-roundtrip.aseprite", bytes_again)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod archive;
pub mod byteio;
pub mod chunk;
pub mod document;
pub mod read;
pub mod spec;
pub mod write;

pub use archive::{ConversionWarning, ConvertedArchive, archive_to_document, document_to_archive};
pub use chunk::{
    CelChunk, CelChunkData, Chunk, ColorProfileChunk, ExternalFileEntry, ExternalFilesChunk,
    LayerChunk, LayerKindCode, NineSliceWire, OldPaletteChunk, PaletteChunk, PaletteEntryWire,
    PivotWire, SliceChunk, SliceKeyEntry, TagEntry, TagsChunk, TilesetChunk, TilesetSourceWire,
    UserDataChunk,
};
pub use document::{AsepriteDocument, ColorDepth, DocumentFrame, DocumentHeader};
pub use read::{decode, decode_from_file};
pub use spec::{ChunkType, blend_mode_from_aseprite, blend_mode_to_aseprite};
pub use write::{encode, encode_to_file};
