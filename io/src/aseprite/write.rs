//! Encode an [`AsepriteDocument`] back to `.aseprite` bytes.

use std::io::Write;
use std::path::Path;

use flate2::Compression;
use flate2::write::ZlibEncoder;

use crate::error::{Error, Result};

use super::byteio::write::{
    put_aseprite_string, put_i16_le, put_i32_le, put_u16_le, put_u32_le, put_zeros,
};
use super::chunk::{
    CelChunk, CelChunkData, Chunk, ColorProfileChunk, ExternalFilesChunk, LayerChunk,
    LayerKindCode, NineSliceWire, OldPaletteChunk, PaletteChunk, PivotWire, SliceChunk, TagsChunk,
    TilesetChunk, TilesetSourceWire, UserDataChunk,
};
use super::document::{AsepriteDocument, DocumentFrame, DocumentHeader};
use super::spec::{
    CEL_TYPE_COMPRESSED, CEL_TYPE_COMPRESSED_TILEMAP, CEL_TYPE_LINKED, CEL_TYPE_RAW,
    CHUNK_HEADER_LEN, ChunkType, FILE_MAGIC, FRAME_HEADER_LEN, FRAME_MAGIC, HEADER_LEN,
    LAYER_TYPE_GROUP, LAYER_TYPE_NORMAL, LAYER_TYPE_TILEMAP, PALETTE_ENTRY_FLAG_NAME,
    SLICE_FLAG_NINE_SLICE, SLICE_FLAG_PIVOT, TILESET_FLAG_EMPTY_TILE, TILESET_FLAG_EXTERNAL,
    TILESET_FLAG_INLINE, USER_DATA_FLAG_COLOR, USER_DATA_FLAG_PROPERTIES, USER_DATA_FLAG_TEXT,
    ZLIB_LEVEL, blend_mode_to_aseprite,
};

/// Encodes `document` to a fresh `.aseprite` byte vector.
///
/// # Errors
///
/// - [`Error::Io`] when zlib compression of a chunk payload fails.
/// - [`Error::Truncated`] when an inline-tileset payload's pixel byte
///   count does not match `tile_count * tile_width * tile_height *
///   bytes_per_pixel`.
pub fn encode(document: &AsepriteDocument) -> Result<Vec<u8>> {
    // Frame and chunk byte sizes are recorded in their respective
    // headers, so the writer first builds frame bodies into temporary
    // buffers, then prepends the file header with the resulting total.
    let mut frame_blobs: Vec<Vec<u8>> = Vec::with_capacity(document.frames.len());
    for frame in &document.frames {
        frame_blobs.push(write_frame(frame, &document.header)?);
    }
    let frames_total: usize = frame_blobs.iter().map(Vec::len).sum();
    let header_u64 = u64::try_from(HEADER_LEN).unwrap_or(u64::MAX);
    let frames_total_u64 = u64::try_from(frames_total).unwrap_or(u64::MAX);
    let file_size: u64 = header_u64.saturating_add(frames_total_u64);
    let capacity = usize::try_from(file_size).unwrap_or(usize::MAX);
    let mut out = Vec::with_capacity(capacity);
    write_header(&mut out, &document.header, document.frames.len(), file_size);
    for blob in frame_blobs {
        out.extend_from_slice(&blob);
    }
    Ok(out)
}

/// Encodes `document` and writes it to `path` atomically.
///
/// Writes to a sibling tempfile first, fsyncs, then renames into place.
/// A crash mid-write leaves the previous file (if any) intact.
///
/// # Errors
///
/// - [`Error::Io`] for any filesystem failure.
pub fn encode_to_file(document: &AsepriteDocument, path: impl AsRef<Path>) -> Result<()> {
    use std::fs::{File, rename};
    let bytes = encode(document)?;
    let path = path.as_ref();
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    {
        let mut f = File::create(&tmp).map_err(Error::Io)?;
        f.write_all(&bytes).map_err(Error::Io)?;
        f.sync_all().map_err(Error::Io)?;
    }
    if let Err(e) = rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(Error::Io(e));
    }
    Ok(())
}

fn write_header(out: &mut Vec<u8>, header: &DocumentHeader, frame_count: usize, file_size: u64) {
    let frame_count_u16 = u16::try_from(frame_count).unwrap_or(u16::MAX);
    let file_size_u32 = u32::try_from(file_size).unwrap_or(u32::MAX);
    put_u32_le(out, file_size_u32);
    put_u16_le(out, FILE_MAGIC);
    put_u16_le(out, frame_count_u16);
    put_u16_le(out, header.width);
    put_u16_le(out, header.height);
    put_u16_le(out, header.color_depth.bits());
    put_u32_le(out, header.flags);
    put_u16_le(out, header.deprecated_speed_ms);
    put_u32_le(out, 0);
    put_u32_le(out, 0);
    out.push(header.transparent_index);
    put_zeros(out, 3);
    put_u16_le(out, header.color_count);
    out.push(header.pixel_width);
    out.push(header.pixel_height);
    put_i16_le(out, header.grid_x);
    put_i16_le(out, header.grid_y);
    put_u16_le(out, header.grid_width);
    put_u16_le(out, header.grid_height);
    let consumed = out.len();
    debug_assert!(consumed <= HEADER_LEN);
    put_zeros(out, HEADER_LEN.saturating_sub(consumed));
}

fn write_frame(frame: &DocumentFrame, header: &DocumentHeader) -> Result<Vec<u8>> {
    let mut chunk_blobs: Vec<(u16, Vec<u8>)> = Vec::with_capacity(frame.chunks.len());
    for chunk in &frame.chunks {
        chunk_blobs.push(write_chunk(chunk, header)?);
    }
    let chunks_byte_len: usize = chunk_blobs
        .iter()
        .map(|(_, payload)| payload.len() + CHUNK_HEADER_LEN)
        .sum();
    let frame_bytes = FRAME_HEADER_LEN + chunks_byte_len;
    let mut out = Vec::with_capacity(frame_bytes);
    let frame_bytes_u32 = u32::try_from(frame_bytes).unwrap_or(u32::MAX);
    put_u32_le(&mut out, frame_bytes_u32);
    put_u16_le(&mut out, FRAME_MAGIC);
    let chunk_count = chunk_blobs.len();
    let old_count = u16::try_from(chunk_count.min(usize::from(u16::MAX))).unwrap_or(u16::MAX);
    put_u16_le(&mut out, old_count);
    put_u16_le(&mut out, frame.duration_ms);
    put_zeros(&mut out, 2);
    let new_count = u32::try_from(chunk_count).unwrap_or(u32::MAX);
    put_u32_le(&mut out, new_count);
    for (code, payload) in chunk_blobs {
        let chunk_size = u32::try_from(payload.len() + CHUNK_HEADER_LEN).unwrap_or(u32::MAX);
        put_u32_le(&mut out, chunk_size);
        put_u16_le(&mut out, code);
        out.extend_from_slice(&payload);
    }
    Ok(out)
}

fn write_chunk(chunk: &Chunk, header: &DocumentHeader) -> Result<(u16, Vec<u8>)> {
    Ok(match chunk {
        Chunk::Layer(c) => (ChunkType::Layer.code(), write_layer(c)),
        Chunk::Cel(c) => (ChunkType::Cel.code(), write_cel(c)?),
        Chunk::ColorProfile(c) => (ChunkType::ColorProfile.code(), write_color_profile(c)),
        Chunk::ExternalFiles(c) => (ChunkType::ExternalFiles.code(), write_external_files(c)),
        Chunk::Tags(c) => (ChunkType::Tags.code(), write_tags(c)),
        Chunk::Palette(c) => (ChunkType::Palette.code(), write_palette(c)),
        Chunk::OldPalette255(c) => (ChunkType::OldPalette255.code(), write_old_palette(c, false)),
        Chunk::OldPalette63(c) => (ChunkType::OldPalette63.code(), write_old_palette(c, true)),
        Chunk::UserData(c) => (ChunkType::UserData.code(), write_user_data(c)),
        Chunk::Slice(c) => (ChunkType::Slice.code(), write_slice(c)),
        Chunk::Tileset(c) => (ChunkType::Tileset.code(), write_tileset(c, header)?),
        Chunk::Unknown { code, payload } => (*code, payload.clone()),
    })
}

fn write_layer(c: &LayerChunk) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + c.name.len());
    put_u16_le(&mut out, c.flags);
    let kind_code = match c.kind {
        LayerKindCode::Normal => LAYER_TYPE_NORMAL,
        LayerKindCode::Group => LAYER_TYPE_GROUP,
        LayerKindCode::Tilemap => LAYER_TYPE_TILEMAP,
    };
    put_u16_le(&mut out, kind_code);
    put_u16_le(&mut out, c.child_level);
    put_u16_le(&mut out, 0);
    put_u16_le(&mut out, 0);
    put_u16_le(&mut out, blend_mode_to_aseprite(c.blend));
    out.push(c.opacity);
    put_zeros(&mut out, 3);
    put_aseprite_string(&mut out, &c.name);
    if matches!(c.kind, LayerKindCode::Tilemap) {
        put_u32_le(&mut out, c.tileset_index);
    }
    if let Some(uuid) = c.uuid {
        out.extend_from_slice(&uuid);
    }
    out
}

fn write_cel(c: &CelChunk) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(32);
    put_u16_le(&mut out, c.layer_index);
    put_i16_le(&mut out, c.x);
    put_i16_le(&mut out, c.y);
    out.push(c.opacity);
    let cel_type = match &c.data {
        CelChunkData::Raw { .. } => CEL_TYPE_RAW,
        CelChunkData::Linked { .. } => CEL_TYPE_LINKED,
        CelChunkData::Compressed { .. } => CEL_TYPE_COMPRESSED,
        CelChunkData::Tilemap { .. } => CEL_TYPE_COMPRESSED_TILEMAP,
    };
    put_u16_le(&mut out, cel_type);
    put_i16_le(&mut out, c.z_index);
    put_zeros(&mut out, 5);
    match &c.data {
        CelChunkData::Raw {
            width,
            height,
            pixels,
        } => {
            put_u16_le(&mut out, *width);
            put_u16_le(&mut out, *height);
            out.extend_from_slice(pixels);
        }
        CelChunkData::Linked { frame } => {
            put_u16_le(&mut out, *frame);
        }
        CelChunkData::Compressed {
            width,
            height,
            pixels,
        } => {
            put_u16_le(&mut out, *width);
            put_u16_le(&mut out, *height);
            let compressed = deflate(pixels)?;
            out.extend_from_slice(&compressed);
        }
        CelChunkData::Tilemap {
            width,
            height,
            bits_per_tile,
            tile_id_mask,
            x_flip_mask,
            y_flip_mask,
            diagonal_flip_mask,
            tiles,
        } => {
            put_u16_le(&mut out, *width);
            put_u16_le(&mut out, *height);
            put_u16_le(&mut out, *bits_per_tile);
            put_u32_le(&mut out, *tile_id_mask);
            put_u32_le(&mut out, *x_flip_mask);
            put_u32_le(&mut out, *y_flip_mask);
            put_u32_le(&mut out, *diagonal_flip_mask);
            put_zeros(&mut out, 10);
            let mut raw = Vec::with_capacity(tiles.len() * 4);
            for tile in tiles {
                raw.extend_from_slice(&tile.to_le_bytes());
            }
            let compressed = deflate(&raw)?;
            out.extend_from_slice(&compressed);
        }
    }
    Ok(out)
}

fn write_color_profile(c: &ColorProfileChunk) -> Vec<u8> {
    let mut out = Vec::with_capacity(16);
    put_u16_le(&mut out, c.kind);
    put_u16_le(&mut out, u16::from(c.fixed_gamma));
    put_u32_le(&mut out, c.gamma_fixed_16_16);
    put_zeros(&mut out, 8);
    out
}

fn write_external_files(c: &ExternalFilesChunk) -> Vec<u8> {
    let mut out = Vec::new();
    let count = u32::try_from(c.entries.len()).unwrap_or(u32::MAX);
    put_u32_le(&mut out, count);
    put_zeros(&mut out, 8);
    for entry in &c.entries {
        put_u32_le(&mut out, entry.id);
        out.push(entry.kind);
        put_zeros(&mut out, 7);
        put_aseprite_string(&mut out, &entry.name);
    }
    out
}

fn write_tags(c: &TagsChunk) -> Vec<u8> {
    let mut out = Vec::new();
    let count = u16::try_from(c.tags.len()).unwrap_or(u16::MAX);
    put_u16_le(&mut out, count);
    put_zeros(&mut out, 8);
    for tag in &c.tags {
        put_u16_le(&mut out, tag.from_frame);
        put_u16_le(&mut out, tag.to_frame);
        out.push(tag.loop_direction);
        put_u16_le(&mut out, tag.repeat);
        put_zeros(&mut out, 6);
        out.push(tag.deprecated_color[0]);
        out.push(tag.deprecated_color[1]);
        out.push(tag.deprecated_color[2]);
        put_zeros(&mut out, 1);
        put_aseprite_string(&mut out, &tag.name);
    }
    out
}

fn write_palette(c: &PaletteChunk) -> Vec<u8> {
    let mut out = Vec::new();
    put_u32_le(&mut out, c.palette_size);
    put_u32_le(&mut out, c.first_index);
    put_u32_le(&mut out, c.last_index);
    put_zeros(&mut out, 8);
    for entry in &c.entries {
        let flags = if entry.name.is_some() {
            PALETTE_ENTRY_FLAG_NAME
        } else {
            0
        };
        put_u16_le(&mut out, flags);
        out.push(entry.color.r);
        out.push(entry.color.g);
        out.push(entry.color.b);
        out.push(entry.color.a);
        if let Some(name) = &entry.name {
            put_aseprite_string(&mut out, name);
        }
    }
    out
}

fn write_old_palette(c: &OldPaletteChunk, scale_to_63: bool) -> Vec<u8> {
    // Single packet, run-length 0 means 256. The legacy format only
    // addresses 256 colors total; truncate any tail rather than emit a
    // multi-packet encoding the spec doesn't really exercise.
    let mut out = Vec::new();
    let trimmed = if c.colors.len() > 256 {
        &c.colors[..256]
    } else {
        c.colors.as_slice()
    };
    if trimmed.is_empty() {
        put_u16_le(&mut out, 0);
        return out;
    }
    put_u16_le(&mut out, 1);
    out.push(0); // skip
    let len_byte = if trimmed.len() == 256 {
        0u8
    } else {
        u8::try_from(trimmed.len()).unwrap_or(255)
    };
    out.push(len_byte);
    for color in trimmed {
        let (r, g, b) = if scale_to_63 {
            (clamp_63(color.r), clamp_63(color.g), clamp_63(color.b))
        } else {
            (color.r, color.g, color.b)
        };
        out.push(r);
        out.push(g);
        out.push(b);
    }
    out
}

fn clamp_63(v: u8) -> u8 {
    let scaled = (u32::from(v) * 63 + 127) / 255;
    u8::try_from(scaled).unwrap_or(63)
}

fn write_user_data(c: &UserDataChunk) -> Vec<u8> {
    let mut out = Vec::new();
    let mut flags: u32 = 0;
    if c.text.is_some() {
        flags |= USER_DATA_FLAG_TEXT;
    }
    if c.color.is_some() {
        flags |= USER_DATA_FLAG_COLOR;
    }
    // Drop the properties-map flag on write — Pixhaus does not preserve
    // the map's contents. Aseprite would reject a chunk that asserts
    // the bit but does not include a map body.
    let _ = USER_DATA_FLAG_PROPERTIES;
    put_u32_le(&mut out, flags);
    if let Some(text) = &c.text {
        put_aseprite_string(&mut out, text);
    }
    if let Some(color) = c.color {
        out.push(color.r);
        out.push(color.g);
        out.push(color.b);
        out.push(color.a);
    }
    out
}

fn write_slice(c: &SliceChunk) -> Vec<u8> {
    let mut out = Vec::new();
    let key_count = u32::try_from(c.keys.len()).unwrap_or(u32::MAX);
    put_u32_le(&mut out, key_count);
    let mut flags: u32 = 0;
    if c.has_nine_slice {
        flags |= SLICE_FLAG_NINE_SLICE;
    }
    if c.has_pivot {
        flags |= SLICE_FLAG_PIVOT;
    }
    put_u32_le(&mut out, flags);
    put_zeros(&mut out, 4);
    put_aseprite_string(&mut out, &c.name);
    for key in &c.keys {
        put_u32_le(&mut out, key.frame);
        put_i32_le(&mut out, key.x);
        put_i32_le(&mut out, key.y);
        put_u32_le(&mut out, key.width);
        put_u32_le(&mut out, key.height);
        if c.has_nine_slice {
            let nine = key.nine_slice.unwrap_or(NineSliceWire {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            });
            put_i32_le(&mut out, nine.x);
            put_i32_le(&mut out, nine.y);
            put_u32_le(&mut out, nine.width);
            put_u32_le(&mut out, nine.height);
        }
        if c.has_pivot {
            let pivot = key.pivot.unwrap_or(PivotWire { x: 0, y: 0 });
            put_i32_le(&mut out, pivot.x);
            put_i32_le(&mut out, pivot.y);
        }
    }
    out
}

fn write_tileset(c: &TilesetChunk, header: &DocumentHeader) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    put_u32_le(&mut out, c.tileset_id);
    let mut flags = c.flags;
    flags &= !(TILESET_FLAG_INLINE | TILESET_FLAG_EXTERNAL);
    match &c.source {
        TilesetSourceWire::External { .. } => flags |= TILESET_FLAG_EXTERNAL,
        TilesetSourceWire::Inline { .. } => flags |= TILESET_FLAG_INLINE | TILESET_FLAG_EMPTY_TILE,
    }
    put_u32_le(&mut out, flags);
    put_u32_le(&mut out, c.tile_count);
    put_u16_le(&mut out, c.tile_width);
    put_u16_le(&mut out, c.tile_height);
    put_i16_le(&mut out, c.base_index);
    put_zeros(&mut out, 14);
    put_aseprite_string(&mut out, &c.name);
    match &c.source {
        TilesetSourceWire::External {
            external_file_id,
            external_tileset_id,
        } => {
            put_u32_le(&mut out, *external_file_id);
            put_u32_le(&mut out, *external_tileset_id);
        }
        TilesetSourceWire::Inline { pixels } => {
            let bpp = u64::from(header.color_depth.bytes_per_pixel());
            let expected =
                u64::from(c.tile_count) * u64::from(c.tile_width) * u64::from(c.tile_height) * bpp;
            if pixels.len() as u64 != expected {
                return Err(Error::Truncated);
            }
            let compressed = deflate(pixels)?;
            put_u32_le(
                &mut out,
                u32::try_from(compressed.len()).unwrap_or(u32::MAX),
            );
            out.extend_from_slice(&compressed);
        }
    }
    Ok(out)
}

fn deflate(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(ZLIB_LEVEL));
    encoder.write_all(data).map_err(Error::Io)?;
    encoder.finish().map_err(Error::Io)
}

#[cfg(test)]
mod tests {
    use pixhaus_core::project::Rgba;

    use super::super::read::decode;
    use super::*;

    #[test]
    fn empty_document_round_trip() {
        let doc = AsepriteDocument::empty(8, 8);
        let bytes = encode(&doc).unwrap();
        let back = decode(&bytes).unwrap();
        assert_eq!(back.header.width, 8);
        assert_eq!(back.header.height, 8);
        assert!(back.frames.is_empty());
    }

    #[test]
    fn deflate_round_trips_a_buffer() {
        let bytes = (0u8..=255).cycle().take(1024).collect::<Vec<_>>();
        let compressed = deflate(&bytes).unwrap();
        let mut decoder = flate2::read::ZlibDecoder::new(compressed.as_slice());
        let mut back = Vec::new();
        std::io::Read::read_to_end(&mut decoder, &mut back).unwrap();
        assert_eq!(back, bytes);
    }

    #[test]
    fn old_palette_emits_single_packet() {
        let chunk = super::super::chunk::OldPaletteChunk {
            colors: vec![Rgba::opaque(10, 20, 30), Rgba::opaque(255, 255, 255)],
        };
        let payload = write_old_palette(&chunk, false);
        // packet count + skip + len + 2 entries × 3 bytes.
        assert_eq!(payload.len(), 2 + 1 + 1 + 6);
    }

    #[test]
    fn clamp_63_endpoints() {
        assert_eq!(clamp_63(0), 0);
        assert_eq!(clamp_63(255), 63);
        // mid-range maps to roughly half of 63.
        assert_eq!(clamp_63(128), 32);
    }
}
