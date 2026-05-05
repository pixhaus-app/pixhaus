//! Parsed [`AsepriteDocument`].
//!
//! This is the in-memory shape the reader produces and the writer
//! consumes. It is one step removed from raw bytes (chunks are decoded,
//! ZLIB payloads decompressed, strings as `String`) and one step
//! removed from the Pixhaus data model (frame/chunk structure is
//! preserved verbatim, no [`pixhaus_core::project::Project`] yet).
//!
//! Conversions to and from [`pixhaus_core::project::Project`] live in
//! [`super::archive`]. Readers that only need the on-disk layout — for
//! diagnostics, validation, fixture authoring — can stop at this type.

use super::chunk::Chunk;

/// Color depth declared by the file header.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ColorDepth {
    /// 32-bit RGBA, four channels at 8 bits each.
    Rgba,
    /// 16-bit grayscale-with-alpha (one luma + one alpha byte).
    Grayscale,
    /// 8-bit indexed-mode pixels.
    Indexed,
}

impl ColorDepth {
    /// Bits per pixel for this color depth.
    #[must_use]
    pub const fn bits(self) -> u16 {
        match self {
            Self::Rgba => 32,
            Self::Grayscale => 16,
            Self::Indexed => 8,
        }
    }

    /// Bytes per pixel for this color depth.
    #[must_use]
    pub const fn bytes_per_pixel(self) -> u32 {
        match self {
            Self::Rgba => 4,
            Self::Grayscale => 2,
            Self::Indexed => 1,
        }
    }

    /// Maps a wire-level bits-per-pixel value to a [`ColorDepth`].
    /// Returns `None` for any value the spec does not document.
    #[must_use]
    pub const fn from_bits(bits: u16) -> Option<Self> {
        match bits {
            32 => Some(Self::Rgba),
            16 => Some(Self::Grayscale),
            8 => Some(Self::Indexed),
            _ => None,
        }
    }
}

/// Aseprite file header, parsed.
///
/// All fields the reader keeps. Fields the spec defines but Pixhaus
/// drops without warning (pixel ratio, grid, palette-size hint) are
/// preserved here so the writer can re-emit them with sensible
/// defaults.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DocumentHeader {
    /// Canvas width in pixels.
    pub width: u16,
    /// Canvas height in pixels.
    pub height: u16,
    /// Color authoring depth.
    pub color_depth: ColorDepth,
    /// 32-bit flags field (`HEADER_FLAG_*`).
    pub flags: u32,
    /// Deprecated speed-in-ms field. Aseprite still writes it; the
    /// per-frame duration takes precedence.
    pub deprecated_speed_ms: u16,
    /// Index of the transparent color in indexed-mode files.
    pub transparent_index: u8,
    /// Palette size hint from the header (informational; the actual
    /// palette length comes from `0x2019` chunks).
    pub color_count: u16,
    /// Pixel-ratio numerator (width). Defaults to `1` for square pixels.
    pub pixel_width: u8,
    /// Pixel-ratio denominator (height). Defaults to `1`.
    pub pixel_height: u8,
    /// Grid X position (editor preference; not part of the document).
    pub grid_x: i16,
    /// Grid Y position.
    pub grid_y: i16,
    /// Grid cell width.
    pub grid_width: u16,
    /// Grid cell height.
    pub grid_height: u16,
}

impl DocumentHeader {
    /// Constructs a minimal RGBA header for a sprite of `(width, height)`.
    /// Sets the per-layer-opacity flag (the spec's recommended default
    /// since 1.3) and leaves grid/pixel-ratio at the spec's defaults.
    #[must_use]
    pub const fn rgba(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            color_depth: ColorDepth::Rgba,
            flags: 1, // HEADER_FLAG_LAYER_OPACITY
            deprecated_speed_ms: 100,
            transparent_index: 0,
            color_count: 0,
            pixel_width: 1,
            pixel_height: 1,
            grid_x: 0,
            grid_y: 0,
            grid_width: 16,
            grid_height: 16,
        }
    }
}

/// One frame in the parsed document.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentFrame {
    /// Display duration in milliseconds.
    pub duration_ms: u16,
    /// Chunks that appear within this frame, in declaration order.
    pub chunks: Vec<Chunk>,
}

impl DocumentFrame {
    /// Constructs a frame with the given duration and no chunks.
    #[must_use]
    pub const fn new(duration_ms: u16) -> Self {
        Self {
            duration_ms,
            chunks: Vec::new(),
        }
    }
}

/// A parsed Aseprite document.
#[derive(Clone, Debug, PartialEq)]
pub struct AsepriteDocument {
    /// Decoded file header.
    pub header: DocumentHeader,
    /// Frames in order. Frame `0` contains layer/tag/palette/slice/
    /// tileset chunks per the spec; subsequent frames hold cels and
    /// per-frame palette overrides.
    pub frames: Vec<DocumentFrame>,
}

impl AsepriteDocument {
    /// Constructs an empty RGBA document with no frames.
    #[must_use]
    pub const fn empty(width: u16, height: u16) -> Self {
        Self {
            header: DocumentHeader::rgba(width, height),
            frames: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_depth_round_trips() {
        for depth in [ColorDepth::Rgba, ColorDepth::Grayscale, ColorDepth::Indexed] {
            assert_eq!(ColorDepth::from_bits(depth.bits()), Some(depth));
        }
    }

    #[test]
    fn unknown_color_depth_is_none() {
        assert!(ColorDepth::from_bits(24).is_none());
        assert!(ColorDepth::from_bits(0).is_none());
    }

    #[test]
    fn rgba_header_sets_layer_opacity_flag() {
        let h = DocumentHeader::rgba(64, 64);
        assert_eq!(h.flags & 1, 1);
        assert_eq!(h.color_depth, ColorDepth::Rgba);
    }
}
