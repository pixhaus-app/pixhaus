//! Error types for the `pixhaus-io` crate.

use pixhaus_core::project::SchemaError;
use thiserror::Error;

/// Errors returned by the Pixhaus I/O crate.
#[derive(Debug, Error)]
pub enum Error {
    /// Byte sequence at offset 0 does not match the expected `PIXHAUS\0` magic.
    #[error("not a .pixhaus file")]
    InvalidMagic,

    /// File was written by a pre-release Pixhaus and is no longer
    /// loadable. Wraps [`SchemaError::PreReleaseFile`] so the data-model
    /// rejection bubbles up through the io error chain.
    #[error(transparent)]
    Schema(#[from] SchemaError),

    /// An importer for a legacy format is temporarily disabled while the
    /// B9 project-library migration completes. Aseprite and PSD import
    /// land back in B9.5 with library-aware translation.
    #[error(
        "Importing {format} files is temporarily unsupported during the B9 migration. \
         Will be restored in B9.5."
    )]
    LegacyImportUnsupported {
        /// Short format name (`"aseprite"`, `"psd"`).
        format: &'static str,
    },

    /// The project library contains no Custom-kind entity with sprite
    /// states to export to Aseprite format. Either the project is empty
    /// or all entities are Tilesets, Tilemaps, or References.
    #[error("no sprite entity found in the project library for Aseprite export")]
    NoSpriteEntityForExport,

    /// The file's container format major version is higher than this
    /// reader supports. Distinct from
    /// [`Self::UnsupportedSchemaVersion`], which targets the embedded
    /// data-model version.
    #[error("unsupported format version {major}.{minor}")]
    UnsupportedVersion {
        /// Major version field from the file header.
        major: u16,
        /// Minor version field from the file header.
        minor: u16,
    },

    /// The data-model schema version embedded in the body is not
    /// compatible with this reader. Distinct from
    /// [`Self::UnsupportedVersion`] so error messages disambiguate
    /// container drift from project-format drift.
    #[error("unsupported project schema version {major}.{minor}")]
    UnsupportedSchemaVersion {
        /// Major version of `Project::schema_version`.
        major: u16,
        /// Minor version of `Project::schema_version`.
        minor: u16,
    },

    /// The file requires feature flags this reader doesn't understand.
    /// `required` contains only the unknown bits.
    #[error("file requires unknown feature flags: {required:#010x}")]
    UnknownRequiredFeatures {
        /// Bitmask of required flags not present in `KNOWN_FLAGS`.
        required: u32,
    },

    /// The header's `required_flags` includes bits absent from
    /// `feature_flags`. The spec mandates `required ⊆ feature` — this
    /// is the reader-side enforcement.
    #[error(
        "invalid feature flags: required {required:#010x} not subset of advertised {advertised:#010x}"
    )]
    InconsistentFeatureFlags {
        /// `feature_flags` field from the header (offset 12-15).
        advertised: u32,
        /// `required_flags` field from the header (offset 16-19).
        required: u32,
    },

    /// The compressed body's decompressed size exceeded the safety
    /// cap. Prevents a small `.pixhaus` file claiming a multi-GB
    /// decompressed body and OOM-ing the process before any
    /// deserialization runs.
    #[error("decompressed body exceeded safety cap of {limit} bytes")]
    DecompressedTooLarge {
        /// The cap that was breached, in bytes.
        limit: u64,
    },

    /// The byte slice ended before the expected field was reached.
    #[error("file is truncated or corrupt")]
    Truncated,

    /// A palette file is malformed or contains an unsupported variant.
    #[error("invalid palette format: {0}")]
    InvalidPalette(String),

    /// A palette has more entries than the target format's count field
    /// can represent. RIFF PAL caps at `u16::MAX`; an oversize palette
    /// would silently truncate the count and emit a malformed file, so
    /// we reject up-front.
    #[error("palette has {count} entries; {format} format max is {max}")]
    PaletteTooLarge {
        /// Number of colors the caller tried to encode.
        count: usize,
        /// Maximum supported by the target format.
        max: usize,
        /// Format name, for the error message (e.g. `"RIFF PAL"`).
        format: &'static str,
    },

    /// A RIFF PAL or ACO file contains a color space Pixhaus cannot convert.
    #[error("unsupported color space: {code}")]
    UnsupportedColorSpace {
        /// The raw color space code from the file.
        code: u16,
    },

    /// The Lospec HTTP API returned an error or an unexpected response.
    #[error("Lospec API error: {0}")]
    LospecApi(String),

    /// An HTTP request to the Lospec API failed.
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// A filesystem or stream operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The compressed body could not be decoded.
    #[error("deserialization failed: {0}")]
    Deserialize(#[from] rmp_serde::decode::Error),

    /// The archive could not be encoded to `MessagePack`.
    #[error("serialization failed: {0}")]
    Serialize(#[from] rmp_serde::encode::Error),

    // ── PNG sprite sheet (S10) ──────────────────────────────────────────────
    /// The frame list passed to the sprite sheet exporter was empty.
    #[error("no frames to export")]
    NoFrames,

    /// `Grid { cols: 0 }` is not a valid layout strategy.
    #[error("grid column count must be non-zero")]
    GridColsZero,

    /// The number of composited frame buffers does not match the number
    /// of frames in the sprite.
    #[error("frame buffer count {buffers} does not match sprite frame count {frames}")]
    FrameCountMismatch {
        /// Number of pixel buffers supplied by the caller.
        buffers: usize,
        /// Number of frames declared in the sprite.
        frames: usize,
    },

    /// A composited frame buffer has the wrong dimensions.
    #[error(
        "frame {index} has wrong size: expected {expected_w}×{expected_h}, \
         got {actual_w}×{actual_h}"
    )]
    FrameSizeMismatch {
        /// Zero-based frame index.
        index: usize,
        /// Expected width (from `Sprite.canvas`).
        expected_w: u32,
        /// Expected height (from `Sprite.canvas`).
        expected_h: u32,
        /// Actual buffer width.
        actual_w: u32,
        /// Actual buffer height.
        actual_h: u32,
    },

    /// PNG encoding failed.
    #[error("PNG encoding failed: {0}")]
    PngEncode(#[from] image::ImageError),

    /// JSON serialization of sprite sheet metadata failed.
    #[error("JSON serialization failed: {0}")]
    JsonSerialize(#[from] serde_json::Error),

    /// The packed sprite sheet would exceed the per-side dimension cap.
    ///
    /// Many GPUs reject textures wider or taller than 8 192–16 384 px.
    /// Any sheet that large is also unusable as a Unity sprite atlas.
    #[error("sprite sheet dimensions {width}×{height} exceed the per-side cap of {max} px")]
    SheetTooLarge {
        /// Computed sheet width that triggered the error.
        width: u32,
        /// Computed sheet height that triggered the error.
        height: u32,
        /// The cap that was exceeded (`MAX_SHEET_DIM`).
        max: u32,
    },

    // ── Animated export (S11) ──────────────────────────────────────────────
    /// The frame list passed to an animated exporter was empty.
    #[error("no frames to export")]
    NoAnimationFrames,

    /// The number of composited frame buffers does not match the number of
    /// frames declared in the sprite.
    #[error("frame buffer count {buffers} does not match sprite frame count {frames}")]
    AnimFrameCountMismatch {
        /// Number of pixel buffers supplied by the caller.
        buffers: usize,
        /// Number of frames declared in the sprite.
        frames: usize,
    },

    /// A composited frame buffer has the wrong dimensions for animated export.
    #[error(
        "animation frame {index} has wrong size: expected {expected_w}×{expected_h}, \
         got {actual_w}×{actual_h}"
    )]
    AnimFrameSizeMismatch {
        /// Zero-based frame index.
        index: usize,
        /// Expected width (from `Sprite.canvas`).
        expected_w: u32,
        /// Expected height (from `Sprite.canvas`).
        expected_h: u32,
        /// Actual buffer width.
        actual_w: u32,
        /// Actual buffer height.
        actual_h: u32,
    },

    /// The existing palette has more entries than GIF supports (256 max).
    #[error("existing palette has {count} entries; GIF supports at most 256")]
    PaletteExceedsGifMax {
        /// Number of palette entries in the project palette.
        count: usize,
    },

    /// The `gif` crate reported an encoding error.
    #[error("GIF encoding failed: {0}")]
    GifEncode(#[from] gif::EncodingError),

    /// The `webp` crate reported an error building the animated WebP.
    #[error("animated WebP encoding failed: {0}")]
    WebPEncode(String),

    /// The caller passed `WebPOptions` with values outside the
    /// libwebp-accepted ranges (`quality` `[0, 100]`, `method` `[0, 6]`,
    /// `loop_count` `>= 0`). Caught up front so libwebp's opaque error
    /// codes don't leak through.
    #[error("invalid WebP options: {detail}")]
    InvalidWebPOptions {
        /// Human-readable description of which field was out of range.
        detail: String,
    },

    /// No `ffmpeg` binary was found on `PATH`. MP4 export requires ffmpeg.
    #[error("ffmpeg not found on PATH; install ffmpeg to enable MP4 export")]
    FfmpegNotFound,

    /// `ffmpeg` exited with a non-zero status code.
    #[error("ffmpeg exited with status {code:?}: {stderr}")]
    FfmpegFailed {
        /// Exit code, if the process terminated normally.
        code: Option<i32>,
        /// Stderr captured from ffmpeg.
        stderr: String,
    },

    /// Video decode produced no usable frames.
    #[error("video decode produced no frames: {0}")]
    VideoDecodeFailed(String),
    // ── Tiled TMX export (S12) ─────────────────────────────────────────────────
    /// Two or more tilemap layers passed to the TMX exporter have different
    /// `width` or `height` values. Tiled requires all layers in a map to
    /// share the same dimensions.
    #[error(
        "tilemap layers must share the same dimensions: \
         expected {expected_w}×{expected_h}, \
         layer {layer_index} has {got_w}×{got_h}"
    )]
    TiledLayerSizeMismatch {
        /// Width of the first (reference) layer.
        expected_w: u32,
        /// Height of the first (reference) layer.
        expected_h: u32,
        /// Width of the offending layer.
        got_w: u32,
        /// Height of the offending layer.
        got_h: u32,
        /// Zero-based index of the offending layer in the input slice.
        layer_index: usize,
    },

    /// A tilemap layer addressed a cell index that the tileset can't
    /// resolve (`tile_index >= tileset.tile_count`). Stale tile
    /// references must be cleaned up before export — a partially-empty
    /// GID would point at the wrong atlas slot in Tiled.
    #[error(
        "tile index {tile_index} in layer {layer_index} at ({col}, {row}) \
         is past the end of the tileset (tile_count = {tile_count})"
    )]
    TiledTileIndexOutOfRange {
        /// Zero-based layer index in the input slice.
        layer_index: usize,
        /// Column of the offending cell.
        col: u32,
        /// Row of the offending cell.
        row: u32,
        /// The `TileIndex` value that was out of range.
        tile_index: u32,
        /// The tileset's `tile_count` for context.
        tile_count: u32,
    },

    /// A tilemap layer's `cell(col, row)` returned `None` for an
    /// in-bounds coordinate. Indicates a structural defect in the layer
    /// data — the exporter rejects rather than silently emitting an
    /// empty cell.
    #[error("tilemap layer {layer_index} has no cell at ({col}, {row}) within {width}×{height}")]
    TiledMissingCell {
        /// Zero-based layer index in the input slice.
        layer_index: usize,
        /// Column with no cell.
        col: u32,
        /// Row with no cell.
        row: u32,
        /// Layer's declared width.
        width: u32,
        /// Layer's declared height.
        height: u32,
    },

    /// A layer's `tileset_index` is out of bounds for the tilesets slice.
    #[error(
        "layer {layer_index} references tileset {tileset_index} \
         but only {tileset_count} tilesets were provided"
    )]
    TiledLayerTilesetIndexOutOfRange {
        /// Zero-based layer index in the input slice.
        layer_index: usize,
        /// The `tileset_index` value that was out of range.
        tileset_index: usize,
        /// Number of tilesets provided.
        tileset_count: usize,
    },

    /// Generated TMX output fails a structural constraint from the Tiled 1.10
    /// TMX schema. The message names the violated constraint.
    #[error("TMX schema violation: {0}")]
    TiledSchemaViolation(String),

    // ── PSD import (S09) ────────────────────────────────────────────────────
    /// The `psd` crate returned a parse error.
    #[error("PSD parse error: {0}")]
    PsdParse(String),
    // ── Aseprite (S08) ──────────────────────────────────────────────────────
    /// The first two bytes of the frame magic word (offset 4 of every
    /// frame header in an Aseprite file) do not match `0xF1FA`.
    #[error("bad aseprite frame magic: expected 0xF1FA, found {found:#06x}")]
    BadAsepriteFrameMagic {
        /// Magic word read from the frame header.
        found: u16,
    },

    /// The two magic bytes at offset 4 of an Aseprite header do not
    /// match `0xA5E0`.
    #[error("not an .aseprite file (expected magic 0xA5E0, found {found:#06x})")]
    BadAsepriteMagic {
        /// Magic word read from the file header.
        found: u16,
    },

    /// The aseprite header declares a color depth this reader does not
    /// support. Valid values per spec: 32 (RGBA), 16 (Grayscale), 8
    /// (Indexed).
    #[error("unsupported aseprite color depth: {bits} bits per pixel")]
    UnsupportedColorDepth {
        /// Bits-per-pixel value from the header.
        bits: u16,
    },

    /// The aseprite layer chunk uses a child level that would underflow
    /// the parent stack — i.e. claims to be a child of a layer that is
    /// not present in the chunk stream.
    #[error("aseprite layer hierarchy is malformed (child level {child} has no parent)")]
    InvalidLayerHierarchy {
        /// Child-level value from the offending layer chunk.
        child: u16,
    },

    /// The aseprite cel chunk references a layer index that exceeds the
    /// number of layer chunks parsed so far in the document.
    #[error("aseprite cel references unknown layer index {layer}")]
    UnknownCelLayer {
        /// Layer index field on the cel chunk.
        layer: u16,
    },

    /// The aseprite cel chunk declares a type the reader does not
    /// understand. Valid values per spec: 0 (raw), 1 (linked), 2
    /// (compressed image), 3 (compressed tilemap).
    #[error("unsupported aseprite cel type: {kind}")]
    UnsupportedCelType {
        /// Cel-type field read from the chunk.
        kind: u16,
    },

    /// The aseprite layer chunk declares a type the reader does not
    /// understand. Valid values per spec: 0 (normal), 1 (group), 2
    /// (tilemap).
    #[error("unsupported aseprite layer type: {kind}")]
    UnsupportedLayerType {
        /// Layer-type field read from the chunk.
        kind: u16,
    },

    /// A compressed cel or tileset chunk decompressed past the
    /// per-chunk safety cap. Prevents a maliciously crafted zlib frame
    /// from OOM-ing the reader before chunks downstream get a chance
    /// to parse.
    #[error("aseprite chunk decompressed past {limit}-byte safety cap")]
    AsepriteChunkTooLarge {
        /// Per-chunk cap that was breached, in bytes.
        limit: u64,
    },

    /// The aseprite tilemap cel declares a tile bit width Pixhaus does
    /// not support. The Aseprite spec only documents 32-bit tiles in
    /// 1.3+; encountering any other value means a future or corrupt
    /// extension.
    #[error("unsupported aseprite tilemap tile width: {bits} bits per tile")]
    UnsupportedTileBitWidth {
        /// Bits-per-tile field from the tilemap cel header.
        bits: u16,
    },

    // ── Aseprite merged export (B9.5) ──────────────────────────────────────────
    /// The combined frame count of all states exceeds the Aseprite wire
    /// limit of `u16::MAX` (65535) frames. A merged export of this entity
    /// is not possible; use per-state export instead.
    #[error(
        "merged frame count {total} exceeds the Aseprite limit of 65535; \
         use per-state export instead"
    )]
    FrameCountOverflow {
        /// Total merged frame count that exceeded `u16::MAX`.
        total: u32,
    },
}

/// Crate-local result alias.
pub type Result<T> = std::result::Result<T, Error>;
