//! Parsed-chunk structures.
//!
//! Each variant of [`Chunk`] mirrors one of the Aseprite chunk types
//! Pixhaus understands. The reader produces a `Vec<Chunk>` per frame
//! after dispatching on the `0x2NNN` chunk-type code; the writer
//! consumes the same structures.
//!
//! Fields kept here are the ones the writer needs to round-trip the
//! file. Fields in the spec that Pixhaus drops without warning (cel
//! z-index, color-profile ICC blob, deprecated mask, properties map)
//! never enter this module.

use pixhaus_core::project::{BlendMode, Rgba};

/// One Aseprite chunk.
///
/// `Unknown` is preserved verbatim so a reader that doesn't recognise a
/// chunk type can still write it back. The variant carries the wire
/// `code` plus the entire chunk payload (everything after the 6-byte
/// chunk header).
#[derive(Clone, Debug, PartialEq)]
pub enum Chunk {
    /// `0x2004` — layer.
    Layer(LayerChunk),
    /// `0x2005` — cel (raster, linked, raw, or tilemap).
    Cel(CelChunk),
    /// `0x2007` — color profile. Always parsed shallowly: Pixhaus
    /// records only the kind so the writer can emit a "no-profile"
    /// chunk on save and surface a warning.
    ColorProfile(ColorProfileChunk),
    /// `0x2008` — external file table. Read-only on the data-model side.
    ExternalFiles(ExternalFilesChunk),
    /// `0x2018` — frame tags.
    Tags(TagsChunk),
    /// `0x2019` — full or partial palette update.
    Palette(PaletteChunk),
    /// `0x0004` — legacy 0–255 palette (read-only).
    OldPalette255(OldPaletteChunk),
    /// `0x0011` — legacy 0–63 palette (read-only).
    OldPalette63(OldPaletteChunk),
    /// `0x2020` — user data attached to the previous entity.
    UserData(UserDataChunk),
    /// `0x2022` — slice.
    Slice(SliceChunk),
    /// `0x2023` — tileset.
    Tileset(TilesetChunk),
    /// Any chunk type the reader skipped (`0x2006` cel extra, `0x2016`
    /// mask, `0x2017` path) or did not recognise.
    Unknown {
        /// Wire-level chunk-type code from the chunk header.
        code: u16,
        /// Raw payload bytes (everything after the 6-byte chunk header).
        payload: Vec<u8>,
    },
}

/// `0x2004` layer chunk.
#[derive(Clone, Debug, PartialEq)]
pub struct LayerChunk {
    /// 16-bit flags field (`LAYER_FLAG_*`).
    pub flags: u16,
    /// Layer kind — normal, group, or tilemap.
    pub kind: LayerKindCode,
    /// Hierarchy depth: `0` for top-level, `parent_depth + 1` for a
    /// child of the previous layer at `parent_depth`.
    pub child_level: u16,
    /// Composition blend mode.
    pub blend: BlendMode,
    /// Layer opacity, valid only when the file header advertises
    /// per-layer opacity.
    pub opacity: u8,
    /// Display name.
    pub name: String,
    /// Tileset index for tilemap layers (ignored for other kinds).
    pub tileset_index: u32,
    /// Optional 16-byte UUID present in 1.3+ files when the layer-UUID
    /// header flag is set. Pixhaus parses but does not act on it.
    pub uuid: Option<[u8; 16]>,
}

/// Layer-type code. Mirrors the `LAYER_TYPE_*` constants in
/// [`super::spec`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum LayerKindCode {
    /// Normal raster layer.
    Normal,
    /// Group layer (no pixel content of its own).
    Group,
    /// Tilemap layer (pixel content lives in tiles, not free pixels).
    Tilemap,
}

/// `0x2005` cel chunk. The cel type is encoded in [`CelChunkData`].
#[derive(Clone, Debug, PartialEq)]
pub struct CelChunk {
    /// Owning layer index (matches the order layer chunks appear).
    pub layer_index: u16,
    /// X position relative to the canvas.
    pub x: i16,
    /// Y position relative to the canvas.
    pub y: i16,
    /// Per-cel opacity, multiplied with the layer's opacity at composite.
    pub opacity: u8,
    /// Z-index. Aseprite uses this for cel re-ordering tricks; Pixhaus
    /// preserves it on read and emits zero on write.
    pub z_index: i16,
    /// Variant-specific payload.
    pub data: CelChunkData,
}

/// Variant payload for a [`CelChunk`].
#[derive(Clone, Debug, PartialEq)]
pub enum CelChunkData {
    /// Type 0 — raw pixel data. Pixhaus reads but never writes; the
    /// writer always emits compressed (type 2).
    Raw {
        /// Pixel width.
        width: u16,
        /// Pixel height.
        height: u16,
        /// Raw, uncompressed pixel bytes in the document's color depth.
        pixels: Vec<u8>,
    },
    /// Type 1 — link to another cel on the same layer.
    Linked {
        /// Frame number of the source cel.
        frame: u16,
    },
    /// Type 2 — ZLIB-compressed pixel data.
    Compressed {
        /// Pixel width.
        width: u16,
        /// Pixel height.
        height: u16,
        /// Decompressed pixel bytes in the document's color depth.
        pixels: Vec<u8>,
    },
    /// Type 3 — ZLIB-compressed tilemap data.
    Tilemap {
        /// Tile-grid width.
        width: u16,
        /// Tile-grid height.
        height: u16,
        /// Bits per tile entry. Always 32 for Aseprite 1.3+.
        bits_per_tile: u16,
        /// Bitmask covering the tile-id portion of each entry.
        tile_id_mask: u32,
        /// Bitmask for the X-flip flag.
        x_flip_mask: u32,
        /// Bitmask for the Y-flip flag.
        y_flip_mask: u32,
        /// Bitmask for the diagonal-flip flag.
        diagonal_flip_mask: u32,
        /// Decoded tile entries (one per cell, length `width * height`).
        tiles: Vec<u32>,
    },
}

/// `0x2018` frame-tags chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagsChunk {
    /// Tags in declaration order.
    pub tags: Vec<TagEntry>,
}

/// One tag inside a [`TagsChunk`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagEntry {
    /// First frame in the tag's range (inclusive).
    pub from_frame: u16,
    /// Last frame in the tag's range (inclusive).
    pub to_frame: u16,
    /// Loop direction code (0 fwd, 1 rev, 2 ping-pong, 3 ping-pong-rev).
    pub loop_direction: u8,
    /// Repeat count. `0` means unspecified / play forever.
    pub repeat: u16,
    /// Display name.
    pub name: String,
    /// Deprecated 3-byte tag color. Pixhaus reads, then writes as zero.
    pub deprecated_color: [u8; 3],
}

/// `0x2019` palette chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteChunk {
    /// Total entry count of the resolved palette.
    pub palette_size: u32,
    /// First index this chunk updates.
    pub first_index: u32,
    /// Last index this chunk updates (inclusive).
    pub last_index: u32,
    /// Entries, each carrying its color and optional name.
    pub entries: Vec<PaletteEntryWire>,
}

/// One palette entry on the wire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteEntryWire {
    /// Color value.
    pub color: Rgba,
    /// Optional name. `None` if the entry's `flags` byte does not set
    /// the name-present bit.
    pub name: Option<String>,
}

/// `0x0004` and `0x0011` legacy palette chunks.
///
/// These are read-only; Pixhaus never emits them. The body is one or
/// more `(start_index, run_length, [r, g, b]*)` packets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OldPaletteChunk {
    /// Decoded color sequence indexed from `0`. Each entry is the RGB
    /// tuple from the packet, expanded to 0–255 (the `0x0011` chunk
    /// natively stores 0–63, which the reader scales).
    pub colors: Vec<Rgba>,
}

/// `0x2020` user-data chunk.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UserDataChunk {
    /// Optional text body.
    pub text: Option<String>,
    /// Optional accent color.
    pub color: Option<Rgba>,
    /// `true` when the chunk had a properties-map flag set on the wire.
    /// Pixhaus discards the map's contents but records its presence so
    /// callers can surface a warning.
    pub had_properties: bool,
}

/// `0x2022` slice chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SliceChunk {
    /// Display name.
    pub name: String,
    /// Per-frame keys.
    pub keys: Vec<SliceKeyEntry>,
    /// `true` when the slice declares a nine-slice center on at least
    /// one of its keys (controls the chunk-level flag bit).
    pub has_nine_slice: bool,
    /// `true` when the slice declares a pivot on at least one key.
    pub has_pivot: bool,
}

/// One per-frame slice key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SliceKeyEntry {
    /// First frame from which this key takes effect.
    pub frame: u32,
    /// Slice rectangle origin X.
    pub x: i32,
    /// Slice rectangle origin Y.
    pub y: i32,
    /// Slice rectangle width.
    pub width: u32,
    /// Slice rectangle height.
    pub height: u32,
    /// Optional nine-slice center, relative to `(x, y)`.
    pub nine_slice: Option<NineSliceWire>,
    /// Optional pivot, relative to `(x, y)`.
    pub pivot: Option<PivotWire>,
}

/// Nine-slice center on the wire.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NineSliceWire {
    /// Center origin X (relative to slice origin).
    pub x: i32,
    /// Center origin Y (relative to slice origin).
    pub y: i32,
    /// Center width.
    pub width: u32,
    /// Center height.
    pub height: u32,
}

/// Pivot point on the wire.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PivotWire {
    /// Pivot X (relative to slice origin).
    pub x: i32,
    /// Pivot Y (relative to slice origin).
    pub y: i32,
}

/// `0x2023` tileset chunk.
#[derive(Clone, Debug, PartialEq)]
pub struct TilesetChunk {
    /// Tileset ID. Layer chunks reference this number through
    /// [`LayerChunk::tileset_index`].
    pub tileset_id: u32,
    /// Raw 32-bit flag field — `TILESET_FLAG_*` bits OR'd together.
    pub flags: u32,
    /// Tile count (includes the empty tile at index 0 by convention).
    pub tile_count: u32,
    /// Tile width in pixels.
    pub tile_width: u16,
    /// Tile height in pixels.
    pub tile_height: u16,
    /// Base index. Aseprite stores 1 by default; tile id 0 is the
    /// "empty" tile.
    pub base_index: i16,
    /// Display name.
    pub name: String,
    /// Variant payload (external link or inline tile pixels).
    pub source: TilesetSourceWire,
}

/// Variant payload for a [`TilesetChunk`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TilesetSourceWire {
    /// External tileset reference, resolved through an
    /// [`ExternalFilesChunk`].
    External {
        /// External-files index from the chunk header.
        external_file_id: u32,
        /// Tileset ID inside the external file.
        external_tileset_id: u32,
    },
    /// Inline tile pixels (compressed at the tail of the chunk).
    Inline {
        /// Decompressed tile pixel bytes. Layout: tiles packed
        /// vertically — `tile_count` consecutive `tile_width × tile_height`
        /// tiles at the document's color depth.
        pixels: Vec<u8>,
    },
}

/// `0x2008` external-files chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalFilesChunk {
    /// Entries indexed by their numeric ID.
    pub entries: Vec<ExternalFileEntry>,
}

/// One external-file entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalFileEntry {
    /// External-file ID. Tileset chunks reference this when their
    /// `TILESET_FLAG_EXTERNAL` bit is set.
    pub id: u32,
    /// Type code (per spec: 0 = palette, 1 = tileset, 2 = extension props,
    /// 3 = tileset extension props).
    pub kind: u8,
    /// File name relative to the `.aseprite` file.
    pub name: String,
}

/// `0x2007` color-profile chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColorProfileChunk {
    /// Profile type field. `0` = no profile, `1` = sRGB, `2` = ICC.
    pub kind: u16,
    /// `true` if the file uses a fixed gamma (when kind = `0` or `1`).
    pub fixed_gamma: bool,
    /// Gamma value (8.16 fixed point on the wire; stored here as raw bits).
    pub gamma_fixed_16_16: u32,
    /// Length of the trailing ICC blob; `0` when kind != `2`.
    pub icc_len: u32,
}
