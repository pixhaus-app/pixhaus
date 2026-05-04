//! Decode `.aseprite` bytes into an [`AsepriteDocument`].

use std::io::Read;
use std::path::Path;

use flate2::read::ZlibDecoder;
use pixhaus_core::project::{BlendMode, Rgba};

use crate::error::{Error, Result};

use super::byteio::Cursor;
use super::chunk::{
    CelChunk, CelChunkData, Chunk, ColorProfileChunk, ExternalFileEntry, ExternalFilesChunk,
    LayerChunk, LayerKindCode, NineSliceWire, OldPaletteChunk, PaletteChunk, PaletteEntryWire,
    PivotWire, SliceChunk, SliceKeyEntry, TagEntry, TagsChunk, TilesetChunk, TilesetSourceWire,
    UserDataChunk,
};
use super::document::{AsepriteDocument, ColorDepth, DocumentFrame, DocumentHeader};
use super::spec::{
    CEL_TYPE_COMPRESSED, CEL_TYPE_COMPRESSED_TILEMAP, CEL_TYPE_LINKED, CEL_TYPE_RAW,
    CHUNK_HEADER_LEN, ChunkType, FILE_MAGIC, FRAME_MAGIC, HEADER_FLAG_LAYER_UUID, HEADER_LEN,
    LAYER_TYPE_GROUP, LAYER_TYPE_NORMAL, LAYER_TYPE_TILEMAP, MAX_CHUNK_DECOMPRESSED,
    PALETTE_ENTRY_FLAG_NAME, SLICE_FLAG_NINE_SLICE, SLICE_FLAG_PIVOT, TILESET_FLAG_EXTERNAL,
    TILESET_FLAG_INLINE, USER_DATA_FLAG_COLOR, USER_DATA_FLAG_PROPERTIES, USER_DATA_FLAG_TEXT,
    blend_mode_from_aseprite,
};

/// Decodes a `.aseprite` byte slice into an [`AsepriteDocument`].
///
/// # Errors
///
/// - [`Error::Truncated`] when the slice ends before a declared field.
/// - [`Error::BadAsepriteMagic`] when the file magic does not match.
/// - [`Error::BadAsepriteFrameMagic`] when any frame's magic is wrong.
/// - [`Error::UnsupportedColorDepth`] when the header's bits-per-pixel
///   is not one of `{32, 16, 8}`.
/// - [`Error::UnsupportedLayerType`] / [`Error::UnsupportedCelType`] /
///   [`Error::UnsupportedTileBitWidth`] for chunk fields the reader
///   does not implement.
/// - [`Error::AsepriteChunkTooLarge`] when a compressed chunk
///   decompresses past the per-chunk safety cap.
/// - [`Error::Io`] when zlib decompression fails on a chunk.
pub fn decode(data: &[u8]) -> Result<AsepriteDocument> {
    let mut cursor = Cursor::new(data);
    let header = read_header(&mut cursor)?;
    let frame_count = read_header_frame_count(data)?;
    let mut frames = Vec::with_capacity(frame_count as usize);
    for _ in 0..frame_count {
        frames.push(read_frame(&mut cursor, &header)?);
    }
    Ok(AsepriteDocument { header, frames })
}

/// Reads a `.aseprite` file from `path` and decodes it.
///
/// This is a blocking call; wrap in `tokio::task::spawn_blocking` from
/// async context.
///
/// # Errors
///
/// Returns the same errors as [`decode`], plus [`Error::Io`] if the
/// file cannot be opened or read.
pub fn decode_from_file(path: impl AsRef<Path>) -> Result<AsepriteDocument> {
    let bytes = std::fs::read(path.as_ref())?;
    decode(&bytes)
}

fn read_header_frame_count(data: &[u8]) -> Result<u16> {
    // Frame count lives at offset 6-7 of the header; we re-read from
    // the original slice rather than tracking it through `read_header`
    // because the header reader's job is to advance past 128 bytes
    // exactly, not return a struct field for every offset.
    let bytes = data.get(6..8).ok_or(Error::Truncated)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_header(cursor: &mut Cursor<'_>) -> Result<DocumentHeader> {
    let header_start = cursor.position();
    if cursor.remaining() < HEADER_LEN {
        return Err(Error::Truncated);
    }
    // File size — informational; ignored. The frame loop drives parsing.
    let _file_size = cursor.read_u32_le()?;
    let magic = cursor.read_u16_le()?;
    if magic != FILE_MAGIC {
        return Err(Error::BadAsepriteMagic { found: magic });
    }
    let _frames = cursor.read_u16_le()?;
    let width = cursor.read_u16_le()?;
    let height = cursor.read_u16_le()?;
    let bits = cursor.read_u16_le()?;
    let color_depth = ColorDepth::from_bits(bits).ok_or(Error::UnsupportedColorDepth { bits })?;
    let flags = cursor.read_u32_le()?;
    let deprecated_speed_ms = cursor.read_u16_le()?;
    let _zero1 = cursor.read_u32_le()?;
    let _zero2 = cursor.read_u32_le()?;
    let transparent_index = cursor.read_u8()?;
    cursor.skip(3)?; // ignored 3 bytes
    let color_count = cursor.read_u16_le()?;
    let pixel_width = cursor.read_u8()?;
    let pixel_height = cursor.read_u8()?;
    let grid_x = cursor.read_i16_le()?;
    let grid_y = cursor.read_i16_le()?;
    let grid_width = cursor.read_u16_le()?;
    let grid_height = cursor.read_u16_le()?;
    // Reserved 84 bytes — skip to the end of the 128-byte header.
    let consumed = cursor.position() - header_start;
    if consumed < HEADER_LEN {
        cursor.skip(HEADER_LEN - consumed)?;
    }
    Ok(DocumentHeader {
        width,
        height,
        color_depth,
        flags,
        deprecated_speed_ms,
        transparent_index,
        color_count,
        pixel_width,
        pixel_height,
        grid_x,
        grid_y,
        grid_width,
        grid_height,
    })
}

fn read_frame(cursor: &mut Cursor<'_>, header: &DocumentHeader) -> Result<DocumentFrame> {
    let frame_start = cursor.position();
    let frame_bytes = cursor.read_u32_le()? as usize;
    let magic = cursor.read_u16_le()?;
    if magic != FRAME_MAGIC {
        return Err(Error::BadAsepriteFrameMagic { found: magic });
    }
    let old_chunk_count = cursor.read_u16_le()?;
    let duration_ms = cursor.read_u16_le()?;
    cursor.skip(2)?; // reserved
    let new_chunk_count = cursor.read_u32_le()?;
    let chunk_count = if new_chunk_count == 0 {
        u32::from(old_chunk_count)
    } else {
        new_chunk_count
    };

    let mut chunks = Vec::with_capacity(chunk_count as usize);
    for _ in 0..chunk_count {
        chunks.push(read_chunk(cursor, header)?);
    }
    // Trust frame_bytes for forward-compatibility: if the writer
    // emitted chunks we don't model, jump to the declared end. This
    // shouldn't fire for files we wrote ourselves, but it keeps third-
    // party files from desyncing the cursor.
    let consumed = cursor.position() - frame_start;
    if consumed < frame_bytes {
        cursor.skip(frame_bytes - consumed)?;
    }
    Ok(DocumentFrame {
        duration_ms,
        chunks,
    })
}

fn read_chunk(cursor: &mut Cursor<'_>, header: &DocumentHeader) -> Result<Chunk> {
    let chunk_start = cursor.position();
    let chunk_size = cursor.read_u32_le()? as usize;
    let chunk_type_code = cursor.read_u16_le()?;
    let payload_len = chunk_size
        .checked_sub(CHUNK_HEADER_LEN)
        .ok_or(Error::Truncated)?;
    let payload_end = chunk_start
        .checked_add(chunk_size)
        .ok_or(Error::Truncated)?;
    let payload = cursor.read_slice(payload_len)?;

    let chunk = match ChunkType::from_code(chunk_type_code) {
        Some(ChunkType::Layer) => Chunk::Layer(parse_layer(payload, header)?),
        Some(ChunkType::Cel) => Chunk::Cel(parse_cel(payload, header)?),
        Some(ChunkType::ColorProfile) => Chunk::ColorProfile(parse_color_profile(payload)?),
        Some(ChunkType::ExternalFiles) => Chunk::ExternalFiles(parse_external_files(payload)?),
        Some(ChunkType::Tags) => Chunk::Tags(parse_tags(payload)?),
        Some(ChunkType::Palette) => Chunk::Palette(parse_palette(payload)?),
        Some(ChunkType::OldPalette255) => Chunk::OldPalette255(parse_old_palette(payload, false)?),
        Some(ChunkType::OldPalette63) => Chunk::OldPalette63(parse_old_palette(payload, true)?),
        Some(ChunkType::UserData) => Chunk::UserData(parse_user_data(payload)?),
        Some(ChunkType::Slice) => Chunk::Slice(parse_slice(payload)?),
        Some(ChunkType::Tileset) => Chunk::Tileset(parse_tileset(payload, header)?),
        // Spec-known but Pixhaus-ignored chunk types: preserve verbatim
        // so the writer can replay anything we don't model.
        Some(ChunkType::CelExtra | ChunkType::Mask | ChunkType::Path) | None => Chunk::Unknown {
            code: chunk_type_code,
            payload: payload.to_vec(),
        },
    };

    // Chunk readers can leave the inner cursor anywhere within the
    // payload (some bytes are reserved). The outer cursor is already
    // positioned at `payload_end` because `read_slice` advanced it.
    debug_assert_eq!(cursor.position(), payload_end);
    Ok(chunk)
}

fn parse_layer(payload: &[u8], header: &DocumentHeader) -> Result<LayerChunk> {
    let mut cur = Cursor::new(payload);
    let flags = cur.read_u16_le()?;
    let kind_code = cur.read_u16_le()?;
    let kind = match kind_code {
        LAYER_TYPE_NORMAL => LayerKindCode::Normal,
        LAYER_TYPE_GROUP => LayerKindCode::Group,
        LAYER_TYPE_TILEMAP => LayerKindCode::Tilemap,
        other => return Err(Error::UnsupportedLayerType { kind: other }),
    };
    let child_level = cur.read_u16_le()?;
    let _default_w = cur.read_u16_le()?;
    let _default_h = cur.read_u16_le()?;
    let blend_code = cur.read_u16_le()?;
    let (blend, unknown_blend_code) = match blend_mode_from_aseprite(blend_code) {
        Some(mode) => (mode, None),
        None => (BlendMode::default(), Some(blend_code)),
    };
    let opacity = cur.read_u8()?;
    cur.skip(3)?; // reserved
    let name = cur.read_aseprite_string()?;
    let tileset_index = if matches!(kind, LayerKindCode::Tilemap) {
        cur.read_u32_le()?
    } else {
        0
    };
    let uuid = if header.flags & HEADER_FLAG_LAYER_UUID != 0 && cur.remaining() >= 16 {
        let mut buf = [0u8; 16];
        let bytes = cur.read_slice(16)?;
        buf.copy_from_slice(bytes);
        Some(buf)
    } else {
        None
    };
    Ok(LayerChunk {
        flags,
        kind,
        child_level,
        blend,
        unknown_blend_code,
        opacity,
        name,
        tileset_index,
        uuid,
    })
}

fn parse_cel(payload: &[u8], header: &DocumentHeader) -> Result<CelChunk> {
    let mut cur = Cursor::new(payload);
    let layer_index = cur.read_u16_le()?;
    let x = cur.read_i16_le()?;
    let y = cur.read_i16_le()?;
    let opacity = cur.read_u8()?;
    let cel_type = cur.read_u16_le()?;
    let z_index = cur.read_i16_le()?;
    cur.skip(5)?; // reserved
    let data = match cel_type {
        CEL_TYPE_RAW => parse_cel_raw(&mut cur, header)?,
        CEL_TYPE_LINKED => CelChunkData::Linked {
            frame: cur.read_u16_le()?,
        },
        CEL_TYPE_COMPRESSED => parse_cel_compressed(&mut cur, header)?,
        CEL_TYPE_COMPRESSED_TILEMAP => parse_cel_tilemap(&mut cur)?,
        kind => return Err(Error::UnsupportedCelType { kind }),
    };
    Ok(CelChunk {
        layer_index,
        x,
        y,
        opacity,
        z_index,
        data,
    })
}

fn parse_cel_raw(cur: &mut Cursor<'_>, header: &DocumentHeader) -> Result<CelChunkData> {
    let width = cur.read_u16_le()?;
    let height = cur.read_u16_le()?;
    let len = cel_pixel_len(width, height, header.color_depth);
    let pixels = cur.read_slice(len)?.to_vec();
    Ok(CelChunkData::Raw {
        width,
        height,
        pixels,
    })
}

fn parse_cel_compressed(cur: &mut Cursor<'_>, header: &DocumentHeader) -> Result<CelChunkData> {
    let width = cur.read_u16_le()?;
    let height = cur.read_u16_le()?;
    let compressed = cur.read_slice(cur.remaining())?;
    let pixels = inflate_with_cap(compressed, MAX_CHUNK_DECOMPRESSED)?;
    let expected = cel_pixel_len(width, height, header.color_depth);
    if pixels.len() != expected {
        return Err(Error::Truncated);
    }
    Ok(CelChunkData::Compressed {
        width,
        height,
        pixels,
    })
}

fn parse_cel_tilemap(cur: &mut Cursor<'_>) -> Result<CelChunkData> {
    let width = cur.read_u16_le()?;
    let height = cur.read_u16_le()?;
    let bits_per_tile = cur.read_u16_le()?;
    if bits_per_tile != 32 {
        return Err(Error::UnsupportedTileBitWidth {
            bits: bits_per_tile,
        });
    }
    let tile_id_mask = cur.read_u32_le()?;
    let x_flip_mask = cur.read_u32_le()?;
    let y_flip_mask = cur.read_u32_le()?;
    let diagonal_flip_mask = cur.read_u32_le()?;
    cur.skip(10)?; // reserved
    let compressed = cur.read_slice(cur.remaining())?;
    let raw = inflate_with_cap(compressed, MAX_CHUNK_DECOMPRESSED)?;
    let cell_count = (width as usize) * (height as usize);
    let expected = cell_count * 4;
    if raw.len() != expected {
        return Err(Error::Truncated);
    }
    let mut tiles = Vec::with_capacity(cell_count);
    for chunk in raw.chunks_exact(4) {
        tiles.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(CelChunkData::Tilemap {
        width,
        height,
        bits_per_tile,
        tile_id_mask,
        x_flip_mask,
        y_flip_mask,
        diagonal_flip_mask,
        tiles,
    })
}

fn parse_color_profile(payload: &[u8]) -> Result<ColorProfileChunk> {
    let mut cur = Cursor::new(payload);
    let kind = cur.read_u16_le()?;
    let flags = cur.read_u16_le()?;
    let gamma_fixed_16_16 = cur.read_u32_le()?;
    cur.skip(8)?; // reserved
    // ICC blob length is implied by the rest of the chunk when kind=2.
    let icc_len = if kind == 2 {
        u32::try_from(cur.remaining()).unwrap_or(u32::MAX)
    } else {
        0
    };
    Ok(ColorProfileChunk {
        kind,
        fixed_gamma: flags & 1 != 0,
        gamma_fixed_16_16,
        icc_len,
    })
}

fn parse_external_files(payload: &[u8]) -> Result<ExternalFilesChunk> {
    let mut cur = Cursor::new(payload);
    let count = cur.read_u32_le()?;
    cur.skip(8)?; // reserved
    let mut entries = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let id = cur.read_u32_le()?;
        let kind = cur.read_u8()?;
        cur.skip(7)?; // reserved
        let name = cur.read_aseprite_string()?;
        entries.push(ExternalFileEntry { id, kind, name });
    }
    Ok(ExternalFilesChunk { entries })
}

fn parse_tags(payload: &[u8]) -> Result<TagsChunk> {
    let mut cur = Cursor::new(payload);
    let count = cur.read_u16_le()?;
    cur.skip(8)?; // reserved
    let mut tags = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let from_frame = cur.read_u16_le()?;
        let to_frame = cur.read_u16_le()?;
        let loop_direction = cur.read_u8()?;
        let repeat = cur.read_u16_le()?;
        cur.skip(6)?; // reserved
        let r = cur.read_u8()?;
        let g = cur.read_u8()?;
        let b = cur.read_u8()?;
        cur.skip(1)?; // extra reserved
        let name = cur.read_aseprite_string()?;
        tags.push(TagEntry {
            from_frame,
            to_frame,
            loop_direction,
            repeat,
            name,
            deprecated_color: [r, g, b],
        });
    }
    Ok(TagsChunk { tags })
}

fn parse_palette(payload: &[u8]) -> Result<PaletteChunk> {
    let mut cur = Cursor::new(payload);
    let palette_size = cur.read_u32_le()?;
    let first_index = cur.read_u32_le()?;
    let last_index = cur.read_u32_le()?;
    cur.skip(8)?; // reserved
    let count = last_index
        .checked_sub(first_index)
        .and_then(|n| n.checked_add(1))
        .unwrap_or(0);
    let mut entries = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let entry_flags = cur.read_u16_le()?;
        let r = cur.read_u8()?;
        let g = cur.read_u8()?;
        let b = cur.read_u8()?;
        let a = cur.read_u8()?;
        let name = if entry_flags & PALETTE_ENTRY_FLAG_NAME != 0 {
            Some(cur.read_aseprite_string()?)
        } else {
            None
        };
        entries.push(PaletteEntryWire {
            color: Rgba::new(r, g, b, a),
            name,
        });
    }
    Ok(PaletteChunk {
        palette_size,
        first_index,
        last_index,
        entries,
    })
}

fn parse_old_palette(payload: &[u8], scale_63_to_255: bool) -> Result<OldPaletteChunk> {
    let mut cur = Cursor::new(payload);
    let packets = cur.read_u16_le()?;
    let mut colors = Vec::new();
    for _ in 0..packets {
        let _skip = cur.read_u8()?;
        let run_byte = cur.read_u8()?;
        let run = if run_byte == 0 {
            256usize
        } else {
            usize::from(run_byte)
        };
        for _ in 0..run {
            let r = cur.read_u8()?;
            let g = cur.read_u8()?;
            let b = cur.read_u8()?;
            let (r, g, b) = if scale_63_to_255 {
                (scale_63(r), scale_63(g), scale_63(b))
            } else {
                (r, g, b)
            };
            colors.push(Rgba::opaque(r, g, b));
        }
    }
    Ok(OldPaletteChunk { colors })
}

fn scale_63(v: u8) -> u8 {
    // Saturating cast keeps the conversion total even if the file
    // breaks the spec by storing a value > 63.
    let scaled = (u32::from(v.min(63)) * 255 + 31) / 63;
    u8::try_from(scaled).unwrap_or(255)
}

fn parse_user_data(payload: &[u8]) -> Result<UserDataChunk> {
    let mut cur = Cursor::new(payload);
    let flags = cur.read_u32_le()?;
    let text = if flags & USER_DATA_FLAG_TEXT != 0 {
        Some(cur.read_aseprite_string()?)
    } else {
        None
    };
    let color = if flags & USER_DATA_FLAG_COLOR != 0 {
        let r = cur.read_u8()?;
        let g = cur.read_u8()?;
        let b = cur.read_u8()?;
        let a = cur.read_u8()?;
        Some(Rgba::new(r, g, b, a))
    } else {
        None
    };
    Ok(UserDataChunk {
        text,
        color,
        had_properties: flags & USER_DATA_FLAG_PROPERTIES != 0,
    })
}

fn parse_slice(payload: &[u8]) -> Result<SliceChunk> {
    let mut cur = Cursor::new(payload);
    let key_count = cur.read_u32_le()?;
    let flags = cur.read_u32_le()?;
    cur.skip(4)?; // reserved
    let name = cur.read_aseprite_string()?;
    let has_nine_slice = flags & SLICE_FLAG_NINE_SLICE != 0;
    let has_pivot = flags & SLICE_FLAG_PIVOT != 0;
    let mut keys = Vec::with_capacity(key_count as usize);
    for _ in 0..key_count {
        let frame = cur.read_u32_le()?;
        let x = cur.read_i32_le()?;
        let y = cur.read_i32_le()?;
        let width = cur.read_u32_le()?;
        let height = cur.read_u32_le()?;
        let nine_slice = if has_nine_slice {
            let cx = cur.read_i32_le()?;
            let cy = cur.read_i32_le()?;
            let cw = cur.read_u32_le()?;
            let ch = cur.read_u32_le()?;
            Some(NineSliceWire {
                x: cx,
                y: cy,
                width: cw,
                height: ch,
            })
        } else {
            None
        };
        let pivot = if has_pivot {
            let px = cur.read_i32_le()?;
            let py = cur.read_i32_le()?;
            Some(PivotWire { x: px, y: py })
        } else {
            None
        };
        keys.push(SliceKeyEntry {
            frame,
            x,
            y,
            width,
            height,
            nine_slice,
            pivot,
        });
    }
    Ok(SliceChunk {
        name,
        keys,
        has_nine_slice,
        has_pivot,
    })
}

fn parse_tileset(payload: &[u8], header: &DocumentHeader) -> Result<TilesetChunk> {
    let mut cur = Cursor::new(payload);
    let tileset_id = cur.read_u32_le()?;
    let flags = cur.read_u32_le()?;
    let tile_count = cur.read_u32_le()?;
    let tile_width = cur.read_u16_le()?;
    let tile_height = cur.read_u16_le()?;
    let base_index = cur.read_i16_le()?;
    cur.skip(14)?; // reserved
    let name = cur.read_aseprite_string()?;
    let source = if flags & TILESET_FLAG_EXTERNAL != 0 {
        let external_file_id = cur.read_u32_le()?;
        let external_tileset_id = cur.read_u32_le()?;
        TilesetSourceWire::External {
            external_file_id,
            external_tileset_id,
        }
    } else if flags & TILESET_FLAG_INLINE != 0 {
        let _compressed_len = cur.read_u32_le()?;
        let compressed = cur.read_slice(cur.remaining())?;
        let pixels = inflate_with_cap(compressed, MAX_CHUNK_DECOMPRESSED)?;
        let bpp = u64::from(header.color_depth.bytes_per_pixel());
        let expected = u64::from(tile_count) * u64::from(tile_width) * u64::from(tile_height) * bpp;
        if pixels.len() as u64 != expected {
            return Err(Error::Truncated);
        }
        TilesetSourceWire::Inline { pixels }
    } else {
        // Tileset with neither flag is malformed; preserve as inline-empty.
        TilesetSourceWire::Inline { pixels: Vec::new() }
    };
    Ok(TilesetChunk {
        tileset_id,
        flags,
        tile_count,
        tile_width,
        tile_height,
        base_index,
        name,
        source,
    })
}

fn cel_pixel_len(width: u16, height: u16, depth: ColorDepth) -> usize {
    (width as usize) * (height as usize) * (depth.bytes_per_pixel() as usize)
}

fn inflate_with_cap(compressed: &[u8], cap: u64) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut decoder = ZlibDecoder::new(compressed);
    let mut limited = (&mut decoder).take(cap + 1);
    let read = limited.read_to_end(&mut out).map_err(Error::Io)?;
    if read as u64 > cap {
        return Err(Error::AsepriteChunkTooLarge { limit: cap });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_buffer() {
        let bytes = [0u8; 16];
        let err = decode(&bytes).unwrap_err();
        assert!(matches!(err, Error::Truncated));
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = vec![0u8; HEADER_LEN];
        // file size first
        bytes[0..4].copy_from_slice(&128u32.to_le_bytes());
        bytes[4..6].copy_from_slice(&0xDEADu16.to_le_bytes());
        let err = decode(&bytes).unwrap_err();
        assert!(matches!(err, Error::BadAsepriteMagic { found: 0xDEAD }));
    }

    #[test]
    fn rejects_bad_color_depth() {
        let mut bytes = vec![0u8; HEADER_LEN];
        bytes[0..4].copy_from_slice(&128u32.to_le_bytes());
        bytes[4..6].copy_from_slice(&FILE_MAGIC.to_le_bytes());
        bytes[6..8].copy_from_slice(&0u16.to_le_bytes()); // 0 frames
        bytes[8..10].copy_from_slice(&8u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&8u16.to_le_bytes());
        bytes[12..14].copy_from_slice(&24u16.to_le_bytes()); // unsupported
        let err = decode(&bytes).unwrap_err();
        assert!(matches!(err, Error::UnsupportedColorDepth { bits: 24 }));
    }

    #[test]
    fn parses_zero_frame_header() {
        let mut bytes = vec![0u8; HEADER_LEN];
        let header_size = u32::try_from(HEADER_LEN).unwrap();
        bytes[0..4].copy_from_slice(&header_size.to_le_bytes());
        bytes[4..6].copy_from_slice(&FILE_MAGIC.to_le_bytes());
        bytes[6..8].copy_from_slice(&0u16.to_le_bytes());
        bytes[8..10].copy_from_slice(&16u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&16u16.to_le_bytes());
        bytes[12..14].copy_from_slice(&32u16.to_le_bytes()); // RGBA
        let doc = decode(&bytes).unwrap();
        assert_eq!(doc.header.width, 16);
        assert_eq!(doc.header.height, 16);
        assert_eq!(doc.header.color_depth, ColorDepth::Rgba);
        assert!(doc.frames.is_empty());
    }

    #[test]
    fn scale_63_endpoints() {
        assert_eq!(scale_63(0), 0);
        assert_eq!(scale_63(63), 255);
        // Out-of-spec values clamp to 255 instead of wrapping.
        assert_eq!(scale_63(255), 255);
    }
}
