//! Wire-format types and constants for the `.pixhaus` binary format.
//!
//! See `docs/file-format.md` for the full byte-level specification.

use pixhaus_core::project::{PixelBufferId, Project};
use serde::{Deserialize, Serialize};

/// Magic bytes that identify a `.pixhaus` file.
/// ASCII "PIXHAUS" followed by a null byte.
pub const MAGIC: &[u8; 8] = b"PIXHAUS\0";

/// Format version major written by this build.
pub const FORMAT_MAJOR: u16 = 1;

/// Format version minor written by this build.
pub const FORMAT_MINOR: u16 = 0;

/// Fixed header length in bytes.
///
/// Layout:
/// - `magic`          8 bytes
/// - `format_major`   2 bytes (u16 BE)
/// - `format_minor`   2 bytes (u16 BE)
/// - `feature_flags`  4 bytes (u32 BE)
/// - `required_flags` 4 bytes (u32 BE)
/// - `body_len`       8 bytes (u64 BE)
///
/// Total: 28 bytes. The compressed body starts immediately after.
pub const HEADER_LEN: usize = 28;

/// zstd compression level used when writing the body.
/// Level 3 matches the zstd default: good compression with fast decode.
pub const ZSTD_LEVEL: i32 = 3;

/// Bitfield of feature flags this reader understands.
///
/// Any bit set in a file's `required_flags` that is absent here causes
/// the reader to reject the file with [`UnknownRequiredFeatures`].
///
/// [`UnknownRequiredFeatures`]: crate::error::Error::UnknownRequiredFeatures
pub const KNOWN_FLAGS: u32 = 0x1F; // TILEMAPS | REFERENCES | ANIMATIONS | SLICES | VERB_HISTORY

/// Top-level structure encoded into the `.pixhaus` file body.
///
/// The body is `zstd::encode(msgpack(PixhausArchive))`. The `project`
/// field contains the structural data model; `buffers` provides the
/// pixel bytes that [`PixelBufferId`] handles point into.
///
/// An archive with an empty `buffers` vec is valid: it encodes projects
/// whose cels reference buffers that haven't been populated yet
/// (e.g. a freshly-created sprite before any drawing).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PixhausArchive {
    /// The Pixhaus project: metadata, sprites, layers, cels, palettes, etc.
    pub project: Project,
    /// Pixel buffer payloads, keyed by the raw value of each
    /// [`PixelBufferId`] that appears in the project.
    pub buffers: Vec<PixelBufferEntry>,
}

impl PixhausArchive {
    /// Constructs an archive with no pixel buffers.
    #[must_use]
    pub fn new(project: Project) -> Self {
        Self {
            project,
            buffers: Vec::new(),
        }
    }

    /// Returns the buffer entry for `id`, if present.
    #[must_use]
    pub fn buffer(&self, id: PixelBufferId) -> Option<&PixelBufferEntry> {
        self.buffers.iter().find(|b| b.id == id.get())
    }
}

/// One pixel buffer stored in a [`PixhausArchive`].
///
/// The `pixels` field holds raw channel bytes. The interpretation
/// (RGBA, grayscale, palette index) is implied by the color mode of
/// the referencing sprite.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PixelBufferEntry {
    /// Raw value of the [`PixelBufferId`] that references this buffer.
    pub id: u32,
    /// Width of the buffer in pixels.
    pub width: u32,
    /// Height of the buffer in pixels.
    pub height: u32,
    /// Bytes per row. May be wider than `width * bytes_per_pixel` to
    /// satisfy alignment requirements.
    pub stride: u32,
    /// Raw pixel bytes. Not additionally compressed; zstd handles the
    /// file body as a whole.
    pub pixels: Vec<u8>,
}

impl PixelBufferEntry {
    /// Returns the typed pixel buffer ID for this entry.
    #[must_use]
    pub fn pixel_buffer_id(&self) -> PixelBufferId {
        PixelBufferId::new(self.id)
    }

    /// Returns the expected byte length for a packed buffer (`width * bytes_per_channel`
    /// per row). Actual `pixels.len()` may be larger when `stride > width * bpc`.
    #[must_use]
    pub fn packed_len(&self, bytes_per_channel: u32) -> usize {
        (self.width * self.height * bytes_per_channel) as usize
    }
}
