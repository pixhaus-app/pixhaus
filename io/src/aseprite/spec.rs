//! Constants for the Aseprite binary file format.
//!
//! Mirrors `docs/ase-file-specs.md` from the Aseprite repository. Every
//! magic word, chunk code, blend-mode index, and bit position the
//! reader and writer touch lives here so a future spec drift hits one
//! file rather than three.

use pixhaus_core::project::BlendMode;

/// Magic word at offset 4 of an Aseprite file header.
pub const FILE_MAGIC: u16 = 0xA5E0;

/// Magic word at offset 4 of every Aseprite frame header.
pub const FRAME_MAGIC: u16 = 0xF1FA;

/// Length of the Aseprite file header in bytes (fixed by the spec).
pub const HEADER_LEN: usize = 128;

/// Length of the per-frame header in bytes (fixed by the spec).
pub const FRAME_HEADER_LEN: usize = 16;

/// Length of the per-chunk header (size + type) in bytes.
pub const CHUNK_HEADER_LEN: usize = 6;

/// Per-chunk decompressed-data safety cap, in bytes. A `2005` cel chunk
/// or `2023` tileset chunk that decompresses past this limit is
/// rejected — defends against zip-bomb style crafted inputs without
/// holding the cel pixel-buffer to a smaller bound than the engine
/// genuinely supports.
pub const MAX_CHUNK_DECOMPRESSED: u64 = 256 * 1024 * 1024;

/// Default zlib compression level used when writing chunks.
///
/// Level 6 matches Aseprite's own writer and the zlib library default.
/// Higher levels save a few percent at multiple-times the compression
/// cost; round-trip tests pass at any level so this is purely a
/// throughput choice.
pub const ZLIB_LEVEL: u32 = 6;

/// Header flag bit: per-layer opacity is meaningful (always set in 1.3+).
pub const HEADER_FLAG_LAYER_OPACITY: u32 = 1 << 0;

/// Header flag bit: layer chunks may carry per-layer 16-byte UUIDs.
/// Pixhaus never sets this on write; on read, UUIDs are parsed and
/// discarded.
pub const HEADER_FLAG_LAYER_UUID: u32 = 1 << 3;

/// Layer flag bit: layer is visible in the canvas.
pub const LAYER_FLAG_VISIBLE: u16 = 1 << 0;
/// Layer flag bit: layer is editable (`!locked` in Pixhaus).
pub const LAYER_FLAG_EDITABLE: u16 = 1 << 1;
/// Layer flag bit: layer is locked against movement (Pixhaus has no
/// equivalent; preserved as a flag and re-emitted on write).
pub const LAYER_FLAG_LOCK_MOVEMENT: u16 = 1 << 2;
/// Layer flag bit: layer is the document's background layer.
pub const LAYER_FLAG_BACKGROUND: u16 = 1 << 3;
/// Layer flag bit: editor prefers linked-cel placement (hint, not state).
pub const LAYER_FLAG_PREFER_LINKED_CELS: u16 = 1 << 4;
/// Layer flag bit: group is rendered collapsed in the layer panel.
pub const LAYER_FLAG_GROUP_COLLAPSED: u16 = 1 << 5;
/// Layer flag bit: layer is a reference layer.
pub const LAYER_FLAG_REFERENCE: u16 = 1 << 6;

/// Layer type code for a normal raster layer.
pub const LAYER_TYPE_NORMAL: u16 = 0;
/// Layer type code for a group layer.
pub const LAYER_TYPE_GROUP: u16 = 1;
/// Layer type code for a tilemap layer.
pub const LAYER_TYPE_TILEMAP: u16 = 2;

/// Cel type code for raw uncompressed image data (Aseprite legacy).
pub const CEL_TYPE_RAW: u16 = 0;
/// Cel type code for a linked cel.
pub const CEL_TYPE_LINKED: u16 = 1;
/// Cel type code for ZLIB-compressed image data.
pub const CEL_TYPE_COMPRESSED: u16 = 2;
/// Cel type code for ZLIB-compressed tilemap data.
pub const CEL_TYPE_COMPRESSED_TILEMAP: u16 = 3;

/// Color depth code for 32-bit RGBA.
pub const COLOR_DEPTH_RGBA: u16 = 32;
/// Color depth code for 16-bit grayscale-with-alpha.
pub const COLOR_DEPTH_GRAYSCALE: u16 = 16;
/// Color depth code for 8-bit indexed.
pub const COLOR_DEPTH_INDEXED: u16 = 8;

/// Tileset flag: the tile data lives in an external file.
pub const TILESET_FLAG_EXTERNAL: u32 = 1 << 0;
/// Tileset flag: the tile data is inline (ZLIB-compressed at the chunk tail).
pub const TILESET_FLAG_INLINE: u32 = 1 << 1;
/// Tileset flag: tile id zero is the empty tile.
pub const TILESET_FLAG_EMPTY_TILE: u32 = 1 << 2;

/// User-data flag: text field is present.
pub const USER_DATA_FLAG_TEXT: u32 = 1 << 0;
/// User-data flag: color field is present.
pub const USER_DATA_FLAG_COLOR: u32 = 1 << 1;
/// User-data flag: properties map is present (read-only — discarded on read).
pub const USER_DATA_FLAG_PROPERTIES: u32 = 1 << 2;

/// Slice chunk flag: nine-slice center rect is present.
pub const SLICE_FLAG_NINE_SLICE: u32 = 1 << 0;
/// Slice chunk flag: pivot point is present.
pub const SLICE_FLAG_PIVOT: u32 = 1 << 1;

/// Palette entry flag: color carries a name string.
pub const PALETTE_ENTRY_FLAG_NAME: u16 = 1 << 0;

/// Numeric chunk-type codes per the Aseprite spec.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(u16)]
pub enum ChunkType {
    /// `0x0004`: legacy 0–255 palette. Read-only; never written.
    OldPalette255 = 0x0004,
    /// `0x0011`: legacy 0–63 palette. Read-only; never written.
    OldPalette63 = 0x0011,
    /// `0x2004`: layer.
    Layer = 0x2004,
    /// `0x2005`: cel.
    Cel = 0x2005,
    /// `0x2006`: cel extra (precise float bounds — ignored).
    CelExtra = 0x2006,
    /// `0x2007`: color profile (ignored; sRGB assumed).
    ColorProfile = 0x2007,
    /// `0x2008`: external file table (read-only; needed to resolve tilesets).
    ExternalFiles = 0x2008,
    /// `0x2016`: deprecated mask.
    Mask = 0x2016,
    /// `0x2017`: never-written path chunk.
    Path = 0x2017,
    /// `0x2018`: frame tags.
    Tags = 0x2018,
    /// `0x2019`: palette.
    Palette = 0x2019,
    /// `0x2020`: user data.
    UserData = 0x2020,
    /// `0x2022`: slice.
    Slice = 0x2022,
    /// `0x2023`: tileset.
    Tileset = 0x2023,
}

impl ChunkType {
    /// Returns the [`ChunkType`] matching `code`, or `None` if unknown.
    #[must_use]
    pub const fn from_code(code: u16) -> Option<Self> {
        match code {
            0x0004 => Some(Self::OldPalette255),
            0x0011 => Some(Self::OldPalette63),
            0x2004 => Some(Self::Layer),
            0x2005 => Some(Self::Cel),
            0x2006 => Some(Self::CelExtra),
            0x2007 => Some(Self::ColorProfile),
            0x2008 => Some(Self::ExternalFiles),
            0x2016 => Some(Self::Mask),
            0x2017 => Some(Self::Path),
            0x2018 => Some(Self::Tags),
            0x2019 => Some(Self::Palette),
            0x2020 => Some(Self::UserData),
            0x2022 => Some(Self::Slice),
            0x2023 => Some(Self::Tileset),
            _ => None,
        }
    }

    /// Returns the wire-level code for the chunk type.
    #[must_use]
    pub const fn code(self) -> u16 {
        self as u16
    }
}

/// Maps an Aseprite blend-mode integer (0–18) to the matching Pixhaus
/// [`BlendMode`].
///
/// Returns `None` for codes the spec does not define. The reader uses
/// this to flag unknown blend modes; the writer goes the other way via
/// [`blend_mode_to_aseprite`].
#[must_use]
pub const fn blend_mode_from_aseprite(code: u16) -> Option<BlendMode> {
    let mode = match code {
        0 => BlendMode::Normal,
        1 => BlendMode::Multiply,
        2 => BlendMode::Screen,
        3 => BlendMode::Overlay,
        4 => BlendMode::Darken,
        5 => BlendMode::Lighten,
        6 => BlendMode::ColorDodge,
        7 => BlendMode::ColorBurn,
        8 => BlendMode::HardLight,
        9 => BlendMode::SoftLight,
        10 => BlendMode::Difference,
        11 => BlendMode::Exclusion,
        12 => BlendMode::Hue,
        13 => BlendMode::Saturation,
        14 => BlendMode::Color,
        15 => BlendMode::Luminosity,
        16 => BlendMode::Addition,
        17 => BlendMode::Subtract,
        18 => BlendMode::Divide,
        _ => return None,
    };
    Some(mode)
}

/// Maps a Pixhaus [`BlendMode`] to its Aseprite integer code (0–18).
///
/// The eight S55 OpenToonz-derived modes (`LinearBurn`, `DarkerColor`,
/// `LinearDodge`, `LighterColor`, `VividLight`, `LinearLight`,
/// `PinLight`, `HardMix`) have no Aseprite equivalent and downgrade to
/// `Normal` (code `0`). Callers wanting to warn on the downgrade should
/// gate the call with [`blend_mode_supported_by_aseprite`] first.
//
// The downgrade arm intentionally shares its result (`0`) with the
// `Normal` arm — the two are kept distinct so the file diff stays
// readable when future formats grow new slots.
#[allow(clippy::match_same_arms)]
#[must_use]
pub const fn blend_mode_to_aseprite(mode: BlendMode) -> u16 {
    match mode {
        BlendMode::Normal => 0,
        BlendMode::Multiply => 1,
        BlendMode::Screen => 2,
        BlendMode::Overlay => 3,
        BlendMode::Darken => 4,
        BlendMode::Lighten => 5,
        BlendMode::ColorDodge => 6,
        BlendMode::ColorBurn => 7,
        BlendMode::HardLight => 8,
        BlendMode::SoftLight => 9,
        BlendMode::Difference => 10,
        BlendMode::Exclusion => 11,
        BlendMode::Hue => 12,
        BlendMode::Saturation => 13,
        BlendMode::Color => 14,
        BlendMode::Luminosity => 15,
        BlendMode::Addition => 16,
        BlendMode::Subtract => 17,
        BlendMode::Divide => 18,
        // S55 modes — no slot in Aseprite; downgrade to Normal.
        BlendMode::LinearBurn
        | BlendMode::DarkerColor
        | BlendMode::LinearDodge
        | BlendMode::LighterColor
        | BlendMode::VividLight
        | BlendMode::LinearLight
        | BlendMode::PinLight
        | BlendMode::HardMix => 0,
    }
}

/// Whether `mode` has a native Aseprite blend-mode code.
///
/// Returns `false` for the eight S55 OpenToonz-derived modes — they
/// downgrade to `Normal` on Aseprite export. The writer uses this to
/// decide when to emit a `tracing::warn!` per affected layer.
#[must_use]
pub const fn blend_mode_supported_by_aseprite(mode: BlendMode) -> bool {
    matches!(
        mode,
        BlendMode::Normal
            | BlendMode::Multiply
            | BlendMode::Screen
            | BlendMode::Overlay
            | BlendMode::Darken
            | BlendMode::Lighten
            | BlendMode::ColorDodge
            | BlendMode::ColorBurn
            | BlendMode::HardLight
            | BlendMode::SoftLight
            | BlendMode::Difference
            | BlendMode::Exclusion
            | BlendMode::Hue
            | BlendMode::Saturation
            | BlendMode::Color
            | BlendMode::Luminosity
            | BlendMode::Addition
            | BlendMode::Subtract
            | BlendMode::Divide
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_type_round_trip() {
        for code in [
            0x0004u16, 0x0011, 0x2004, 0x2005, 0x2006, 0x2007, 0x2008, 0x2016, 0x2017, 0x2018,
            0x2019, 0x2020, 0x2022, 0x2023,
        ] {
            let ty = ChunkType::from_code(code).unwrap();
            assert_eq!(ty.code(), code);
        }
    }

    #[test]
    fn unknown_chunk_type_is_none() {
        assert!(ChunkType::from_code(0xFFFF).is_none());
        assert!(ChunkType::from_code(0x2000).is_none());
    }

    #[test]
    fn blend_mode_round_trips_through_aseprite_codes() {
        let modes = [
            BlendMode::Normal,
            BlendMode::Multiply,
            BlendMode::Screen,
            BlendMode::Overlay,
            BlendMode::Darken,
            BlendMode::Lighten,
            BlendMode::ColorDodge,
            BlendMode::ColorBurn,
            BlendMode::HardLight,
            BlendMode::SoftLight,
            BlendMode::Difference,
            BlendMode::Exclusion,
            BlendMode::Hue,
            BlendMode::Saturation,
            BlendMode::Color,
            BlendMode::Luminosity,
            BlendMode::Addition,
            BlendMode::Subtract,
            BlendMode::Divide,
        ];
        for mode in modes {
            let code = blend_mode_to_aseprite(mode);
            assert_eq!(blend_mode_from_aseprite(code), Some(mode));
        }
    }

    #[test]
    fn unknown_blend_mode_code_is_none() {
        assert!(blend_mode_from_aseprite(99).is_none());
    }

    #[test]
    fn s55_modes_downgrade_to_normal_code() {
        for mode in [
            BlendMode::LinearBurn,
            BlendMode::DarkerColor,
            BlendMode::LinearDodge,
            BlendMode::LighterColor,
            BlendMode::VividLight,
            BlendMode::LinearLight,
            BlendMode::PinLight,
            BlendMode::HardMix,
        ] {
            assert_eq!(blend_mode_to_aseprite(mode), 0, "{mode:?}");
            assert!(!blend_mode_supported_by_aseprite(mode), "{mode:?}");
        }
    }

    #[test]
    fn native_modes_are_marked_supported() {
        for mode in [
            BlendMode::Normal,
            BlendMode::Multiply,
            BlendMode::Screen,
            BlendMode::Overlay,
            BlendMode::Darken,
            BlendMode::Lighten,
            BlendMode::ColorDodge,
            BlendMode::ColorBurn,
            BlendMode::HardLight,
            BlendMode::SoftLight,
            BlendMode::Difference,
            BlendMode::Exclusion,
            BlendMode::Hue,
            BlendMode::Saturation,
            BlendMode::Color,
            BlendMode::Luminosity,
            BlendMode::Addition,
            BlendMode::Subtract,
            BlendMode::Divide,
        ] {
            assert!(blend_mode_supported_by_aseprite(mode), "{mode:?}");
        }
    }
}
