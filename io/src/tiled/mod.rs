//! Tiled-compatible `.tmx` + `.tsx` export (stream S12 + S12-followup).
//!
//! Exports tilemap layers from a Pixhaus sprite as a set of Tiled XML files
//! ready for Unity's `SuperTiled2Unity` importer.
//!
//! Multiple tilesets are supported. Each tileset is assigned a `firstgid`
//! offset so GIDs across tilesets are globally unique within the map. Each
//! layer declares which tileset it draws from; the exporter resolves the
//! correct `firstgid` when encoding GIDs.
//!
//! Output targets Tiled 1.10 format with CSV-encoded layer data and external
//! TSX tileset references.
//!
//! # Usage
//!
//! ```no_run
//! use pixhaus_io::tiled::{
//!     export_tilemap, TiledExportOptions, TiledLayerInput, TilesetExportInput,
//! };
//! use pixhaus_core::project::tilemap::TilemapData;
//! use pixhaus_core::project::tileset::Tileset;
//!
//! # fn tileset() -> Tileset { todo!() }
//! # fn data() -> TilemapData { todo!() }
//!
//! let ts = tileset();
//! let output = export_tilemap(
//!     &[TilesetExportInput { tileset: &ts, image_path: "dungeon.png".into() }],
//!     &[TiledLayerInput { name: "ground", data: &data(), tileset_index: 0 }],
//!     &TiledExportOptions { name: "dungeon".into() },
//! )?;
//! std::fs::write("dungeon.tmx", &output.tmx)?;
//! std::fs::write(&output.tilesets[0].filename, &output.tilesets[0].tsx)?;
//! # Ok::<(), pixhaus_io::Error>(())
//! ```

pub mod schema;
mod tileset_xml;

use std::fmt::Write;

use pixhaus_core::project::tilemap::{TileCell, TileFlags, TilemapData};
use pixhaus_core::project::tileset::Tileset;

use crate::error::{Error, Result};

// Tiled flip flag bits OR-ed into the high three bits of an encoded GID.
const GID_FLIP_X: u32 = 0x8000_0000;
const GID_FLIP_Y: u32 = 0x4000_0000;
const GID_FLIP_DIAGONAL: u32 = 0x2000_0000;

pub(crate) const TILED_FORMAT_VERSION: &str = "1.10";
pub(crate) const TILED_APP_VERSION: &str = "1.10.0";

// ── Public types ──────────────────────────────────────────────────────────────

/// One tileset contributed to an [`export_tilemap`] call.
pub struct TilesetExportInput<'a> {
    /// The tileset data.
    pub tileset: &'a Tileset,
    /// Path to the tileset PNG relative to the `.tmx` file on disk.
    /// Written verbatim into the TSX `<image source="…">` attribute.
    pub image_path: String,
}

/// One tilemap layer contributed to an [`export_tilemap`] call.
pub struct TiledLayerInput<'a> {
    /// Display name for the Tiled `<layer>` element.
    pub name: &'a str,
    /// Tile grid data for this layer.
    pub data: &'a TilemapData,
    /// Zero-based index into the `tilesets` slice this layer draws from.
    ///
    /// Every non-empty cell in `data` must reference a tile that exists in
    /// `tilesets[tileset_index]`. The exporter validates this before encoding.
    pub tileset_index: usize,
}

/// Options controlling an [`export_tilemap`] call.
pub struct TiledExportOptions {
    /// Name written into the `<map>` element.
    pub name: String,
}

/// Output of [`export_tilemap`]: raw UTF-8 XML for all files.
#[derive(Debug)]
pub struct TiledExportOutput {
    /// Contents of the `.tmx` map file.
    pub tmx: String,
    /// One TSX output per tileset, in the same order as the `tilesets` input.
    pub tilesets: Vec<TilesetOutput>,
}

/// A single tileset's `.tsx` output.
#[derive(Debug)]
pub struct TilesetOutput {
    /// Filename to write to disk (e.g. `"dungeon.tsx"`).
    pub filename: String,
    /// Contents of the `.tsx` tileset file.
    pub tsx: String,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Export tilemap layers as a Tiled-compatible `.tmx` plus one `.tsx` per tileset.
///
/// All layers in `layers` must share the same `width` and `height`. An empty
/// `layers` slice is valid and produces a `.tmx` with no `<layer>` elements.
///
/// `firstgid` is computed automatically: the first tileset starts at 1, each
/// subsequent tileset's `firstgid` equals the previous tileset's `firstgid`
/// plus its real tile count (excluding the Pixhaus empty-tile sentinel at
/// index 0).
///
/// # Errors
///
/// - [`Error::TiledLayerSizeMismatch`] when layers have different dimensions.
/// - [`Error::TiledLayerTilesetIndexOutOfRange`] when a layer's `tileset_index`
///   is out of bounds for the `tilesets` slice.
/// - [`Error::TiledTileIndexOutOfRange`] when a cell references a tile that
///   does not exist in the layer's tileset.
pub fn export_tilemap(
    tilesets: &[TilesetExportInput<'_>],
    layers: &[TiledLayerInput<'_>],
    options: &TiledExportOptions,
) -> Result<TiledExportOutput> {
    validate_layers(layers)?;
    validate_tileset_indices(tilesets.len(), layers)?;
    let firstgids = compute_firstgids(tilesets);
    validate_tile_indices(tilesets, layers)?;
    // Compute filenames once, deduplicating collisions by suffixing the
    // index. Two tilesets sharing a display name would otherwise both
    // resolve to `{name}.tsx` and the second write would clobber the
    // first. Use the same filenames in both the TSX outputs and the
    // <tileset source="..."> references inside the TMX.
    let filenames = tsx_filenames(tilesets);
    let tsx_outputs = tilesets
        .iter()
        .zip(filenames.iter())
        .map(|(ts_input, filename)| {
            let tsx = tileset_xml::build_tsx(ts_input.tileset, &ts_input.image_path);
            TilesetOutput {
                filename: filename.clone(),
                tsx,
            }
        })
        .collect();
    let tmx = build_tmx(tilesets, &firstgids, &filenames, layers, options)?;
    schema::validate_tmx(&tmx)?;
    Ok(TiledExportOutput {
        tmx,
        tilesets: tsx_outputs,
    })
}

/// Computes the per-tileset TSX filenames, suffixing duplicates with their
/// input index. `["forest", "forest", "stone"]` becomes `["forest.tsx",
/// "forest_1.tsx", "stone.tsx"]`. The first occurrence keeps the bare name
/// so the common case (unique names) stays readable.
fn tsx_filenames(tilesets: &[TilesetExportInput<'_>]) -> Vec<String> {
    let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    tilesets
        .iter()
        .enumerate()
        .map(|(i, ts_input)| {
            let name: &str = ts_input.tileset.name.as_str();
            let count = seen.entry(name).or_insert(0);
            let filename = if *count == 0 {
                format!("{name}.tsx")
            } else {
                format!("{name}_{i}.tsx")
            };
            *count += 1;
            filename
        })
        .collect()
}

// ── Validation ────────────────────────────────────────────────────────────────

fn validate_layers(layers: &[TiledLayerInput<'_>]) -> Result<()> {
    let Some(first) = layers.first() else {
        return Ok(());
    };
    let (w, h) = (first.data.width, first.data.height);
    for (index, layer) in layers.iter().enumerate().skip(1) {
        if layer.data.width != w || layer.data.height != h {
            return Err(Error::TiledLayerSizeMismatch {
                expected_w: w,
                expected_h: h,
                got_w: layer.data.width,
                got_h: layer.data.height,
                layer_index: index,
            });
        }
    }
    Ok(())
}

fn validate_tileset_indices(tileset_count: usize, layers: &[TiledLayerInput<'_>]) -> Result<()> {
    for (layer_index, layer) in layers.iter().enumerate() {
        if layer.tileset_index >= tileset_count {
            return Err(Error::TiledLayerTilesetIndexOutOfRange {
                layer_index,
                tileset_index: layer.tileset_index,
                tileset_count,
            });
        }
    }
    Ok(())
}

/// Pre-flight check: every non-empty cell references a tile that exists in the
/// layer's declared tileset. Stale references would produce GIDs that point at
/// the wrong atlas entry once Tiled resolves them via `firstgid + local_id`.
fn validate_tile_indices(
    tilesets: &[TilesetExportInput<'_>],
    layers: &[TiledLayerInput<'_>],
) -> Result<()> {
    for (layer_index, layer) in layers.iter().enumerate() {
        let tile_count = tilesets[layer.tileset_index].tileset.tile_count;
        for row in 0..layer.data.height {
            for col in 0..layer.data.width {
                let cell = layer.data.cell(col, row).ok_or(Error::TiledMissingCell {
                    layer_index,
                    col,
                    row,
                    width: layer.data.width,
                    height: layer.data.height,
                })?;
                if cell.is_empty() {
                    continue;
                }
                let idx = cell.index.get();
                if idx >= tile_count {
                    return Err(Error::TiledTileIndexOutOfRange {
                        layer_index,
                        col,
                        row,
                        tile_index: idx,
                        tile_count,
                    });
                }
            }
        }
    }
    Ok(())
}

// ── firstgid math ─────────────────────────────────────────────────────────────

/// Returns one `firstgid` per tileset, starting at 1.
///
/// Each tileset's `firstgid` equals the previous one's `firstgid` plus the
/// number of real tiles in that tileset. "Real tiles" excludes the Pixhaus
/// empty-tile sentinel at index 0, so `real_count = tile_count - 1`.
fn compute_firstgids(tilesets: &[TilesetExportInput<'_>]) -> Vec<u32> {
    let mut firstgids = Vec::with_capacity(tilesets.len());
    let mut next: u32 = 1;
    for ts_input in tilesets {
        firstgids.push(next);
        let real_count = ts_input.tileset.tile_count.saturating_sub(1);
        next = next.saturating_add(real_count);
    }
    firstgids
}

// ── TMX building ──────────────────────────────────────────────────────────────

fn build_tmx(
    tilesets: &[TilesetExportInput<'_>],
    firstgids: &[u32],
    tsx_filenames: &[String],
    layers: &[TiledLayerInput<'_>],
    _options: &TiledExportOptions,
) -> Result<String> {
    // Map grid cell size comes from the first tileset. Tiled uses this as the
    // visual grid; individual tilesets carry their own tile dimensions in TSX.
    let (tile_w, tile_h) = tilesets.first().map_or((0, 0), |ts| {
        let s = ts.tileset.tile_size;
        (s.width, s.height)
    });
    let (map_w, map_h) = layers
        .first()
        .map_or((0, 0), |l| (l.data.width, l.data.height));
    // No real tilemap has 2^32 layers; saturating keeps nextlayerid valid XML.
    let next_layer_id = u32::try_from(layers.len())
        .unwrap_or(u32::MAX)
        .saturating_add(1);

    let mut out = String::with_capacity(512);
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    writeln!(
        out,
        "<map version=\"{TILED_FORMAT_VERSION}\" tiledversion=\"{TILED_APP_VERSION}\"\n\
         \x20    orientation=\"orthogonal\" renderorder=\"right-down\"\n\
         \x20    width=\"{map_w}\" height=\"{map_h}\"\n\
         \x20    tilewidth=\"{tile_w}\" tileheight=\"{tile_h}\"\n\
         \x20    infinite=\"0\" nextlayerid=\"{next_layer_id}\" nextobjectid=\"1\">"
    )
    .ok();
    out.push('\n');

    for (i, _ts_input) in tilesets.iter().enumerate() {
        writeln!(
            out,
            "  <tileset firstgid=\"{}\" source=\"{}\"/>",
            firstgids[i],
            xml_escape_attr(&tsx_filenames[i])
        )
        .ok();
    }
    out.push('\n');

    for (i, layer) in layers.iter().enumerate() {
        let id = u32::try_from(i).unwrap_or(u32::MAX).saturating_add(1);
        let firstgid = firstgids[layer.tileset_index];
        append_layer_xml(&mut out, id, i, layer, firstgid)?;
        out.push('\n');
    }
    out.push_str("</map>\n");
    Ok(out)
}

fn append_layer_xml(
    out: &mut String,
    id: u32,
    layer_index: usize,
    layer: &TiledLayerInput<'_>,
    firstgid: u32,
) -> Result<()> {
    let w = layer.data.width;
    let h = layer.data.height;
    writeln!(
        out,
        "  <layer id=\"{id}\" name=\"{}\" width=\"{w}\" height=\"{h}\">",
        xml_escape_attr(layer.name)
    )
    .ok();
    out.push_str("    <data encoding=\"csv\">\n");
    encode_csv_into(out, layer_index, layer.data, firstgid)?;
    out.push_str("    </data>\n");
    out.push_str("  </layer>\n");
    Ok(())
}

// ── CSV encoding ──────────────────────────────────────────────────────────────

/// Encodes `data` as a CSV block matching Tiled's format and appends it to
/// `out`. Streams directly into the buffer to avoid per-row allocations.
///
/// Rows are separated by a comma-then-newline: all rows except the last end
/// with a trailing comma before the newline, matching Tiled's own export.
/// Empty cells encode as `0`; occupied cells encode as a `u32` GID with
/// Tiled flip bits OR-ed into the high three bits.
fn encode_csv_into(
    out: &mut String,
    layer_index: usize,
    data: &TilemapData,
    firstgid: u32,
) -> Result<()> {
    for row in 0..data.height {
        if row > 0 {
            out.push_str(",\n");
        }
        for col in 0..data.width {
            let cell = data.cell(col, row).ok_or(Error::TiledMissingCell {
                layer_index,
                col,
                row,
                width: data.width,
                height: data.height,
            })?;
            if col > 0 {
                out.push(',');
            }
            write!(out, "{}", encode_gid(cell, firstgid)).ok();
        }
    }
    out.push('\n');
    Ok(())
}

// ── GID encoding ─────────────────────────────────────────────────────────────

/// Encodes a single [`TileCell`] as a Tiled GID using the given `firstgid`.
///
/// Mapping:
///
/// - `TileIndex(0)` (empty sentinel) → `0`
/// - `TileIndex(n)` → `firstgid + (n - 1)` with Tiled flip bits OR-ed into bits 31–29
///
/// Derivation: Tiled local id = `tile_index - 1`; GID = `firstgid + local_id`.
/// With a single tileset at `firstgid = 1` this simplifies to `GID = n`.
fn encode_gid(cell: TileCell, firstgid: u32) -> u32 {
    if cell.is_empty() {
        return 0;
    }
    let local_id = cell.index.get() - 1;
    let gid = firstgid + local_id;
    let mut flip = 0u32;
    if cell.flags.contains(TileFlags::FLIP_X) {
        flip |= GID_FLIP_X;
    }
    if cell.flags.contains(TileFlags::FLIP_Y) {
        flip |= GID_FLIP_Y;
    }
    if cell.flags.contains(TileFlags::FLIP_DIAGONAL) {
        flip |= GID_FLIP_DIAGONAL;
    }
    gid | flip
}

// ── XML helpers ───────────────────────────────────────────────────────────────

/// Escapes `s` for use as an XML attribute value in a double-quoted context.
pub(crate) fn xml_escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::UserData;
    use pixhaus_core::project::geometry::Size;
    use pixhaus_core::project::id::{PixelBufferId, TileIndex, TilesetId};
    use pixhaus_core::project::tileset::{
        AnimLoopMode, CollisionShape, TileAnimation, TileAnimationFrame, TileProperties, TileShape,
        Tileset, TilesetSource,
    };

    fn sample_tileset() -> Tileset {
        Tileset {
            id: TilesetId::new(1),
            name: "dungeon".into(),
            tile_size: Size::new(16, 16),
            shape: TileShape::Square,
            hex_offset: None,
            // tile_count includes the empty-tile sentinel at index 0.
            tile_count: 6,
            base_index: 1,
            source: TilesetSource::External {
                path: "dungeon.png".into(),
            },
            properties: Vec::new(),
            autotile: None,
            user_data: UserData::default(),
        }
    }

    fn sample_ts_input(ts: &Tileset) -> TilesetExportInput<'_> {
        TilesetExportInput {
            tileset: ts,
            image_path: "dungeon.png".into(),
        }
    }

    fn options() -> TiledExportOptions {
        TiledExportOptions {
            name: "dungeon".into(),
        }
    }

    // ── GID encoding ─────────────────────────────────────────────────────────

    #[test]
    fn empty_cell_encodes_as_zero() {
        assert_eq!(encode_gid(TileCell::empty(), 1), 0);
    }

    #[test]
    fn no_flip_encodes_as_firstgid_plus_local_id() {
        // TileIndex(3) with firstgid=1: local_id=2, GID=3.
        let cell = TileCell {
            index: TileIndex::new(3),
            flags: TileFlags::empty(),
        };
        assert_eq!(encode_gid(cell, 1), 3);
    }

    #[test]
    fn second_tileset_firstgid_offsets_gid() {
        // TileIndex(1) with firstgid=6: local_id=0, GID=6.
        let cell = TileCell {
            index: TileIndex::new(1),
            flags: TileFlags::empty(),
        };
        assert_eq!(encode_gid(cell, 6), 6);
    }

    #[test]
    fn flip_x_sets_bit_31() {
        let cell = TileCell {
            index: TileIndex::new(4),
            flags: TileFlags::FLIP_X,
        };
        assert_eq!(encode_gid(cell, 1), 4 | 0x8000_0000);
    }

    #[test]
    fn flip_y_sets_bit_30() {
        let cell = TileCell {
            index: TileIndex::new(4),
            flags: TileFlags::FLIP_Y,
        };
        assert_eq!(encode_gid(cell, 1), 4 | 0x4000_0000);
    }

    #[test]
    fn flip_diagonal_sets_bit_29() {
        let cell = TileCell {
            index: TileIndex::new(4),
            flags: TileFlags::FLIP_DIAGONAL,
        };
        assert_eq!(encode_gid(cell, 1), 4 | 0x2000_0000);
    }

    #[test]
    fn flip_x_and_y_matches_reference_dungeon_value() {
        // dungeon.tmx encodes tile 4 | flip-X | flip-Y as 3221225476.
        let cell = TileCell {
            index: TileIndex::new(4),
            flags: TileFlags::FLIP_X.union(TileFlags::FLIP_Y),
        };
        assert_eq!(encode_gid(cell, 1), 3_221_225_476);
    }

    #[test]
    fn flip_x_only_matches_reference_dungeon_value() {
        // dungeon.tmx: tile 4 | flip-X = 2147483652
        let cell = TileCell {
            index: TileIndex::new(4),
            flags: TileFlags::FLIP_X,
        };
        assert_eq!(encode_gid(cell, 1), 2_147_483_652);
    }

    #[test]
    fn flip_y_only_matches_reference_dungeon_value() {
        // dungeon.tmx: tile 4 | flip-Y = 1073741828
        let cell = TileCell {
            index: TileIndex::new(4),
            flags: TileFlags::FLIP_Y,
        };
        assert_eq!(encode_gid(cell, 1), 1_073_741_828);
    }

    // ── firstgid math ─────────────────────────────────────────────────────────

    #[test]
    fn single_tileset_firstgid_is_one() {
        let ts = sample_tileset();
        let inputs = [TilesetExportInput {
            tileset: &ts,
            image_path: "a.png".into(),
        }];
        let firstgids = compute_firstgids(&inputs);
        assert_eq!(firstgids, [1]);
    }

    #[test]
    fn second_tileset_firstgid_follows_first_real_tile_count() {
        // First tileset: tile_count=6 → 5 real tiles → second starts at 6.
        let ts1 = sample_tileset(); // tile_count=6
        let mut ts2 = sample_tileset();
        ts2.tile_count = 4; // 3 real tiles
        let inputs = [
            TilesetExportInput {
                tileset: &ts1,
                image_path: "a.png".into(),
            },
            TilesetExportInput {
                tileset: &ts2,
                image_path: "b.png".into(),
            },
        ];
        let firstgids = compute_firstgids(&inputs);
        assert_eq!(firstgids, [1, 6]);
    }

    #[test]
    fn three_tilesets_firstgids_accumulate() {
        // ts1: tile_count=3 (2 real) → next=3
        // ts2: tile_count=5 (4 real) → next=7
        // ts3: tile_count=2 (1 real) → next=8
        let mk = |tile_count| Tileset {
            id: TilesetId::new(1),
            name: "x".into(),
            tile_size: Size::new(16, 16),
            shape: TileShape::Square,
            hex_offset: None,
            tile_count,
            base_index: 1,
            source: TilesetSource::External {
                path: "x.png".into(),
            },
            properties: Vec::new(),
            autotile: None,
            user_data: UserData::default(),
        };
        let ts1 = mk(3);
        let ts2 = mk(5);
        let ts3 = mk(2);
        let inputs = [
            TilesetExportInput {
                tileset: &ts1,
                image_path: "a.png".into(),
            },
            TilesetExportInput {
                tileset: &ts2,
                image_path: "b.png".into(),
            },
            TilesetExportInput {
                tileset: &ts3,
                image_path: "c.png".into(),
            },
        ];
        let firstgids = compute_firstgids(&inputs);
        assert_eq!(firstgids, [1, 3, 7]);
    }

    // ── CSV encoding ──────────────────────────────────────────────────────────

    fn csv_for(data: &TilemapData, firstgid: u32) -> String {
        let mut out = String::new();
        encode_csv_into(&mut out, 0, data, firstgid).expect("test-only call: data is well-formed");
        out
    }

    #[test]
    fn all_empty_2x2_encodes_to_zeros() {
        let csv = csv_for(&TilemapData::empty(2, 2), 1);
        assert_eq!(csv, "0,0,\n0,0\n");
    }

    #[test]
    fn single_cell_encodes_tile_index() {
        let mut data = TilemapData::empty(1, 1);
        data.cells[0] = TileCell {
            index: TileIndex::new(2),
            flags: TileFlags::empty(),
        };
        assert_eq!(csv_for(&data, 1), "2\n");
    }

    #[test]
    fn single_cell_with_offset_firstgid() {
        // TileIndex(1) from a second tileset with firstgid=6 → GID 6.
        let mut data = TilemapData::empty(1, 1);
        data.cells[0] = TileCell {
            index: TileIndex::new(1),
            flags: TileFlags::empty(),
        };
        assert_eq!(csv_for(&data, 6), "6\n");
    }

    #[test]
    fn last_row_has_no_trailing_comma() {
        let mut data = TilemapData::empty(3, 2);
        data.cells[3] = TileCell {
            index: TileIndex::new(1),
            flags: TileFlags::empty(),
        };
        let csv = csv_for(&data, 1);
        let last = csv.trim_end_matches('\n').lines().last().unwrap_or("");
        assert!(!last.ends_with(','), "last row must not end with a comma");
    }

    #[test]
    fn flip_encoded_in_csv_gid() {
        let mut data = TilemapData::empty(1, 1);
        data.cells[0] = TileCell {
            index: TileIndex::new(4),
            flags: TileFlags::FLIP_X,
        };
        let csv = csv_for(&data, 1);
        assert_eq!(csv, "2147483652\n");
    }

    // ── Tile-index validation ─────────────────────────────────────────────────

    #[test]
    fn out_of_range_tile_index_is_rejected() {
        // sample_tileset has tile_count = 6; index 7 is past the end.
        let mut data = TilemapData::empty(2, 1);
        data.cells[0] = TileCell {
            index: TileIndex::new(7),
            flags: TileFlags::empty(),
        };
        let ts = sample_tileset();
        let layers = [TiledLayerInput {
            name: "stale",
            data: &data,
            tileset_index: 0,
        }];
        let err = export_tilemap(&[sample_ts_input(&ts)], &layers, &options())
            .expect_err("must reject stale tile index");
        match err {
            Error::TiledTileIndexOutOfRange {
                layer_index,
                col,
                row,
                tile_index,
                tile_count,
            } => {
                assert_eq!(layer_index, 0);
                assert_eq!(col, 0);
                assert_eq!(row, 0);
                assert_eq!(tile_index, 7);
                assert_eq!(tile_count, 6);
            }
            other => panic!("wrong variant: {other:?}"),
        }
    }

    #[test]
    fn empty_cell_with_out_of_range_index_field_is_allowed() {
        // TileIndex(0) is the empty sentinel — it must not trip the
        // index-out-of-range check even when tile_count is 0.
        let data = TilemapData::empty(1, 1);
        let mut tileset = sample_tileset();
        tileset.tile_count = 0;
        let layers = [TiledLayerInput {
            name: "blank",
            data: &data,
            tileset_index: 0,
        }];
        export_tilemap(
            &[TilesetExportInput {
                tileset: &tileset,
                image_path: "d.png".into(),
            }],
            &layers,
            &options(),
        )
        .unwrap();
    }

    #[test]
    fn invalid_tileset_index_is_rejected() {
        let data = TilemapData::empty(1, 1);
        let ts = sample_tileset();
        // tileset_index=1 but only one tileset provided.
        let layers = [TiledLayerInput {
            name: "bad",
            data: &data,
            tileset_index: 1,
        }];
        let err = export_tilemap(&[sample_ts_input(&ts)], &layers, &options())
            .expect_err("must reject out-of-range tileset_index");
        assert!(matches!(
            err,
            Error::TiledLayerTilesetIndexOutOfRange {
                layer_index: 0,
                tileset_index: 1,
                tileset_count: 1
            }
        ));
    }

    // ── Export: single tileset ────────────────────────────────────────────────

    #[test]
    fn empty_layers_produces_valid_tmx() {
        let ts = sample_tileset();
        let out = export_tilemap(&[sample_ts_input(&ts)], &[], &options()).unwrap();
        assert!(out.tmx.contains("<map"));
        assert!(out.tmx.contains("</map>"));
        assert!(!out.tmx.contains("<layer"));
    }

    #[test]
    fn single_layer_appears_in_tmx() {
        let ts = sample_tileset();
        let data = TilemapData::empty(4, 3);
        let layers = [TiledLayerInput {
            name: "ground",
            data: &data,
            tileset_index: 0,
        }];
        let out = export_tilemap(&[sample_ts_input(&ts)], &layers, &options()).unwrap();
        assert!(out.tmx.contains("name=\"ground\""));
        assert!(out.tmx.contains("width=\"4\""));
        assert!(out.tmx.contains("height=\"3\""));
        assert!(out.tmx.contains("encoding=\"csv\""));
    }

    #[test]
    fn map_dimensions_match_first_layer() {
        let ts = sample_tileset();
        let data = TilemapData::empty(8, 5);
        let layers = [TiledLayerInput {
            name: "base",
            data: &data,
            tileset_index: 0,
        }];
        let out = export_tilemap(&[sample_ts_input(&ts)], &layers, &options()).unwrap();
        assert!(out.tmx.contains("width=\"8\""));
        assert!(out.tmx.contains("height=\"5\""));
    }

    #[test]
    fn tsx_filename_referenced_in_tmx() {
        let ts = sample_tileset();
        let out = export_tilemap(&[sample_ts_input(&ts)], &[], &options()).unwrap();
        assert!(out.tmx.contains("source=\"dungeon.tsx\""));
    }

    #[test]
    fn layer_ids_are_one_based_and_sequential() {
        let ts = sample_tileset();
        let d = TilemapData::empty(2, 2);
        let layers = [
            TiledLayerInput {
                name: "a",
                data: &d,
                tileset_index: 0,
            },
            TiledLayerInput {
                name: "b",
                data: &d,
                tileset_index: 0,
            },
        ];
        let out = export_tilemap(&[sample_ts_input(&ts)], &layers, &options()).unwrap();
        assert!(out.tmx.contains("id=\"1\""));
        assert!(out.tmx.contains("id=\"2\""));
        assert!(out.tmx.contains("nextlayerid=\"3\""));
    }

    #[test]
    fn tile_dimensions_from_tileset_in_map_header() {
        let ts = sample_tileset();
        let out = export_tilemap(&[sample_ts_input(&ts)], &[], &options()).unwrap();
        assert!(out.tmx.contains("tilewidth=\"16\""));
        assert!(out.tmx.contains("tileheight=\"16\""));
    }

    #[test]
    fn mismatched_layer_sizes_return_error() {
        let ts = sample_tileset();
        let d1 = TilemapData::empty(4, 4);
        let d2 = TilemapData::empty(8, 4);
        let layers = [
            TiledLayerInput {
                name: "a",
                data: &d1,
                tileset_index: 0,
            },
            TiledLayerInput {
                name: "b",
                data: &d2,
                tileset_index: 0,
            },
        ];
        let result = export_tilemap(&[sample_ts_input(&ts)], &layers, &options());
        assert!(matches!(
            result,
            Err(Error::TiledLayerSizeMismatch { layer_index: 1, .. })
        ));
    }

    #[test]
    fn output_has_one_tileset_entry() {
        let ts = sample_tileset();
        let out = export_tilemap(&[sample_ts_input(&ts)], &[], &options()).unwrap();
        assert_eq!(out.tilesets.len(), 1);
        assert_eq!(out.tilesets[0].filename, "dungeon.tsx");
    }

    #[test]
    fn duplicate_tileset_names_produce_unique_tsx_filenames() {
        // Two tilesets sharing the display name "forest" used to both
        // get "forest.tsx", clobbering each other on disk and breaking
        // the TMX <tileset source="..."> resolution.
        let a = Tileset {
            id: TilesetId::new(1),
            name: "forest".into(),
            tile_size: Size::new(16, 16),
            shape: TileShape::Square,
            hex_offset: None,
            tile_count: 4,
            base_index: 1,
            source: TilesetSource::External {
                path: "a.png".into(),
            },
            properties: Vec::new(),
            autotile: None,
            user_data: UserData::default(),
        };
        let b = Tileset {
            id: TilesetId::new(2),
            name: "forest".into(), // intentional collision
            tile_size: Size::new(16, 16),
            shape: TileShape::Square,
            hex_offset: None,
            tile_count: 4,
            base_index: 1,
            source: TilesetSource::External {
                path: "b.png".into(),
            },
            properties: Vec::new(),
            autotile: None,
            user_data: UserData::default(),
        };
        let inputs = [
            TilesetExportInput {
                tileset: &a,
                image_path: "a.png".into(),
            },
            TilesetExportInput {
                tileset: &b,
                image_path: "b.png".into(),
            },
        ];
        let out = export_tilemap(&inputs, &[], &options()).unwrap();
        assert_eq!(out.tilesets.len(), 2);
        assert_eq!(out.tilesets[0].filename, "forest.tsx");
        assert_eq!(out.tilesets[1].filename, "forest_1.tsx");
        // The TMX must reference the deduplicated filenames so Tiled
        // resolves both tilesets correctly.
        assert!(out.tmx.contains(r#"source="forest.tsx""#));
        assert!(out.tmx.contains(r#"source="forest_1.tsx""#));
    }

    #[test]
    fn schema_validation_runs_on_export() {
        // Valid export passes; this is a smoke test that schema::validate_tmx
        // is actually invoked from the public path. (Negative cases are
        // covered by the schema module's own unit tests.)
        let ts = sample_tileset();
        let out = export_tilemap(&[sample_ts_input(&ts)], &[], &options()).unwrap();
        assert!(super::schema::validate_tmx(&out.tmx).is_ok());
    }

    #[test]
    fn tsx_contains_tileset_name_and_tile_dimensions() {
        let ts = sample_tileset();
        let out = export_tilemap(&[sample_ts_input(&ts)], &[], &options()).unwrap();
        assert!(out.tilesets[0].tsx.contains("name=\"dungeon\""));
        assert!(out.tilesets[0].tsx.contains("tilewidth=\"16\""));
        assert!(out.tilesets[0].tsx.contains("tileheight=\"16\""));
    }

    #[test]
    fn tsx_tilecount_excludes_empty_sentinel() {
        // tile_count=6 → 5 real tiles in the atlas.
        let ts = sample_tileset();
        let out = export_tilemap(&[sample_ts_input(&ts)], &[], &options()).unwrap();
        assert!(out.tilesets[0].tsx.contains("tilecount=\"5\""));
    }

    #[test]
    fn tsx_image_source_uses_provided_path() {
        let ts = sample_tileset();
        let out = export_tilemap(&[sample_ts_input(&ts)], &[], &options()).unwrap();
        assert!(out.tilesets[0].tsx.contains("source=\"dungeon.png\""));
    }

    #[test]
    fn tileset_with_collision_emits_tile_element_with_property() {
        let mut ts = sample_tileset();
        ts.properties = vec![
            TileProperties::default(),
            TileProperties {
                collision: CollisionShape::Full,
                animation: None,
            },
        ];
        let out = export_tilemap(&[sample_ts_input(&ts)], &[], &options()).unwrap();
        // TileIndex(1) → Tiled local id 0.
        assert!(out.tilesets[0].tsx.contains("<tile id=\"0\">"));
        assert!(out.tilesets[0].tsx.contains("name=\"collision\""));
        assert!(out.tilesets[0].tsx.contains("value=\"true\""));
    }

    #[test]
    fn tileset_with_animation_emits_animation_element() {
        let mut ts = sample_tileset();
        ts.properties = vec![
            TileProperties::default(),
            TileProperties {
                collision: CollisionShape::None,
                animation: Some(TileAnimation {
                    frames: vec![
                        TileAnimationFrame {
                            tile_index: TileIndex::new(1),
                            duration_ms: 100,
                        },
                        TileAnimationFrame {
                            tile_index: TileIndex::new(2),
                            duration_ms: 200,
                        },
                    ],
                    loop_mode: AnimLoopMode::Loop,
                }),
            },
        ];
        let out = export_tilemap(&[sample_ts_input(&ts)], &[], &options()).unwrap();
        assert!(out.tilesets[0].tsx.contains("<animation>"));
        assert!(out.tilesets[0].tsx.contains("duration=\"100\""));
        assert!(out.tilesets[0].tsx.contains("duration=\"200\""));
    }

    #[test]
    fn default_tile_properties_emit_no_tile_element() {
        let ts = sample_tileset();
        let out = export_tilemap(&[sample_ts_input(&ts)], &[], &options()).unwrap();
        // sample_tileset has no properties set → no <tile> elements.
        assert!(!out.tilesets[0].tsx.contains("<tile id="));
    }

    // ── Export: multiple tilesets ─────────────────────────────────────────────

    fn second_tileset() -> Tileset {
        Tileset {
            id: TilesetId::new(2),
            name: "forest".into(),
            tile_size: Size::new(16, 16),
            shape: TileShape::Square,
            hex_offset: None,
            tile_count: 4, // 3 real tiles
            base_index: 1,
            source: TilesetSource::External {
                path: "forest.png".into(),
            },
            properties: Vec::new(),
            autotile: None,
            user_data: UserData::default(),
        }
    }

    #[test]
    fn two_tilesets_emit_correct_firstgids_in_tmx() {
        let ts1 = sample_tileset(); // tile_count=6 → firstgid=1
        let ts2 = second_tileset(); // tile_count=4 → firstgid=6
        let inputs = [
            TilesetExportInput {
                tileset: &ts1,
                image_path: "dungeon.png".into(),
            },
            TilesetExportInput {
                tileset: &ts2,
                image_path: "forest.png".into(),
            },
        ];
        let out = export_tilemap(&inputs, &[], &options()).unwrap();
        assert!(out.tmx.contains("firstgid=\"1\" source=\"dungeon.tsx\""));
        assert!(out.tmx.contains("firstgid=\"6\" source=\"forest.tsx\""));
        assert_eq!(out.tilesets.len(), 2);
        assert_eq!(out.tilesets[0].filename, "dungeon.tsx");
        assert_eq!(out.tilesets[1].filename, "forest.tsx");
    }

    #[test]
    fn layer_from_second_tileset_uses_correct_firstgid() {
        // ts1: tile_count=6 → firstgid=1; ts2: tile_count=4 → firstgid=6.
        // A layer from ts2 with TileIndex(1) should encode as GID 6.
        let ts1 = sample_tileset();
        let ts2 = second_tileset();
        let mut data = TilemapData::empty(1, 1);
        data.cells[0] = TileCell {
            index: TileIndex::new(1),
            flags: TileFlags::empty(),
        };
        let inputs = [
            TilesetExportInput {
                tileset: &ts1,
                image_path: "dungeon.png".into(),
            },
            TilesetExportInput {
                tileset: &ts2,
                image_path: "forest.png".into(),
            },
        ];
        let layers = [TiledLayerInput {
            name: "trees",
            data: &data,
            tileset_index: 1,
        }];
        let out = export_tilemap(&inputs, &layers, &options()).unwrap();
        // The CSV data for this layer should contain GID 6.
        assert!(
            out.tmx.contains("6\n"),
            "expected GID 6 in CSV, got:\n{}",
            out.tmx
        );
    }

    #[test]
    fn layers_from_different_tilesets_encoded_independently() {
        let ts1 = sample_tileset(); // tile_count=6 → firstgid=1
        let ts2 = second_tileset(); // tile_count=4 → firstgid=6
        let mut ground = TilemapData::empty(1, 1);
        ground.cells[0] = TileCell {
            index: TileIndex::new(2),
            flags: TileFlags::empty(),
        };
        let mut trees = TilemapData::empty(1, 1);
        trees.cells[0] = TileCell {
            index: TileIndex::new(3),
            flags: TileFlags::empty(),
        };
        let inputs = [
            TilesetExportInput {
                tileset: &ts1,
                image_path: "dungeon.png".into(),
            },
            TilesetExportInput {
                tileset: &ts2,
                image_path: "forest.png".into(),
            },
        ];
        let layers = [
            TiledLayerInput {
                name: "ground",
                data: &ground,
                tileset_index: 0,
            },
            TiledLayerInput {
                name: "trees",
                data: &trees,
                tileset_index: 1,
            },
        ];
        let out = export_tilemap(&inputs, &layers, &options()).unwrap();
        // ground: TileIndex(2) at firstgid=1 → GID 2
        // trees:  TileIndex(3) at firstgid=6 → GID 8
        assert!(out.tmx.contains("name=\"ground\""));
        assert!(out.tmx.contains("name=\"trees\""));
        // GID 2 and GID 8 both appear in the TMX.
        assert!(out.tmx.contains('2'), "expected GID 2 for ground layer");
        assert!(out.tmx.contains('8'), "expected GID 8 for trees layer");
    }

    // ── XML escaping ──────────────────────────────────────────────────────────

    #[test]
    fn xml_escape_attr_handles_ampersand() {
        assert_eq!(xml_escape_attr("a&b"), "a&amp;b");
    }

    #[test]
    fn xml_escape_attr_handles_double_quote() {
        assert_eq!(xml_escape_attr("say \"hi\""), "say &quot;hi&quot;");
    }

    #[test]
    fn xml_escape_attr_handles_angle_brackets() {
        assert_eq!(xml_escape_attr("a<b>c"), "a&lt;b&gt;c");
    }

    #[test]
    fn xml_escape_attr_leaves_plain_strings_unchanged() {
        assert_eq!(xml_escape_attr("dungeon.tsx"), "dungeon.tsx");
    }

    // ── Inline tileset source ─────────────────────────────────────────────────

    #[test]
    fn inline_tileset_source_uses_provided_image_path() {
        let ts = Tileset {
            id: TilesetId::new(2),
            name: "forest".into(),
            tile_size: Size::new(8, 8),
            shape: TileShape::Square,
            hex_offset: None,
            tile_count: 5,
            base_index: 1,
            source: TilesetSource::Inline {
                buffer: PixelBufferId::new(1),
            },
            properties: Vec::new(),
            autotile: None,
            user_data: UserData::default(),
        };
        let inputs = [TilesetExportInput {
            tileset: &ts,
            image_path: "tilesets/forest.png".into(),
        }];
        let out = export_tilemap(&inputs, &[], &options()).unwrap();
        assert!(
            out.tilesets[0]
                .tsx
                .contains("source=\"tilesets/forest.png\"")
        );
    }

    // ── Schema validation ─────────────────────────────────────────────────────

    #[test]
    fn exported_tmx_passes_schema_validation() {
        let ts = sample_tileset();
        let data = TilemapData::empty(4, 3);
        let layers = [TiledLayerInput {
            name: "ground",
            data: &data,
            tileset_index: 0,
        }];
        let out = export_tilemap(&[sample_ts_input(&ts)], &layers, &options()).unwrap();
        schema::validate_tmx(&out.tmx).expect("exported TMX must pass schema validation");
    }

    #[test]
    fn multi_tileset_tmx_passes_schema_validation() {
        let ts1 = sample_tileset();
        let ts2 = second_tileset();
        let data = TilemapData::empty(2, 2);
        let inputs = [
            TilesetExportInput {
                tileset: &ts1,
                image_path: "dungeon.png".into(),
            },
            TilesetExportInput {
                tileset: &ts2,
                image_path: "forest.png".into(),
            },
        ];
        let layers = [
            TiledLayerInput {
                name: "ground",
                data: &data,
                tileset_index: 0,
            },
            TiledLayerInput {
                name: "trees",
                data: &data,
                tileset_index: 1,
            },
        ];
        let out = export_tilemap(&inputs, &layers, &options()).unwrap();
        schema::validate_tmx(&out.tmx).expect("multi-tileset TMX must pass schema validation");
    }

    // ── Round-trip (S39 importer interface) ───────────────────────────────────
    //
    // These tests simulate what the S39 Unity importer reads: CSV-encoded GIDs
    // plus the tileset firstgid table. They verify that a full export → decode
    // cycle produces the original tile indices.

    /// Decodes a CSV GID block back to raw GID values (no flip bit stripping).
    fn parse_csv_gids(csv: &str) -> Vec<u32> {
        csv.lines()
            .flat_map(|line| line.split(','))
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().parse::<u32>().expect("valid u32"))
            .collect()
    }

    /// Extracts `(firstgid, source)` pairs from a TMX string.
    fn parse_tileset_firstgids(tmx: &str) -> Vec<(u32, String)> {
        tmx.lines()
            .filter(|l| l.contains("<tileset "))
            .map(|l| {
                let fg_start = l.find("firstgid=\"").unwrap() + 10;
                let fg_end = l[fg_start..].find('"').unwrap() + fg_start;
                let fg: u32 = l[fg_start..fg_end].parse().unwrap();
                let src_start = l.find("source=\"").unwrap() + 8;
                let src_end = l[src_start..].find('"').unwrap() + src_start;
                (fg, l[src_start..src_end].to_string())
            })
            .collect()
    }

    /// Strips Tiled flip bits (high 3 bits) and returns the bare GID.
    fn gid_base(gid: u32) -> u32 {
        gid & !(GID_FLIP_X | GID_FLIP_Y | GID_FLIP_DIAGONAL)
    }

    /// Given a bare GID and firstgids table, returns `(tileset_index, local_id)`.
    fn resolve_gid(gid: u32, firstgids: &[(u32, String)]) -> Option<(usize, u32)> {
        if gid == 0 {
            return None; // empty cell
        }
        // Walk in reverse: the rightmost tileset whose firstgid <= gid owns it.
        for (i, &(fg, _)) in firstgids.iter().enumerate().rev() {
            if gid >= fg {
                return Some((i, gid - fg));
            }
        }
        None
    }

    #[test]
    fn round_trip_single_tileset_tile_indices() {
        // Export a 2×1 layer with TileIndex(1) and TileIndex(3), then parse
        // back and verify local IDs map to the original indices.
        let ts = sample_tileset();
        let mut data = TilemapData::empty(2, 1);
        data.cells[0] = TileCell {
            index: TileIndex::new(1),
            flags: TileFlags::empty(),
        };
        data.cells[1] = TileCell {
            index: TileIndex::new(3),
            flags: TileFlags::empty(),
        };
        let layers = [TiledLayerInput {
            name: "ground",
            data: &data,
            tileset_index: 0,
        }];
        let out = export_tilemap(&[sample_ts_input(&ts)], &layers, &options()).unwrap();

        let firstgids = parse_tileset_firstgids(&out.tmx);
        assert_eq!(firstgids.len(), 1);
        assert_eq!(firstgids[0].0, 1);

        // Extract CSV block for the single layer.
        let csv_start =
            out.tmx.find("<data encoding=\"csv\">").unwrap() + "<data encoding=\"csv\">".len();
        let csv_end = out.tmx.find("</data>").unwrap();
        let gids = parse_csv_gids(&out.tmx[csv_start..csv_end]);
        assert_eq!(gids.len(), 2);

        // GID 1: local_id=0 → TileIndex(1) in tileset 0.
        let (ts_idx0, local0) = resolve_gid(gid_base(gids[0]), &firstgids).unwrap();
        assert_eq!(ts_idx0, 0);
        assert_eq!(local0 + 1, 1, "local_id+1 should equal original TileIndex");

        // GID 3: local_id=2 → TileIndex(3) in tileset 0.
        let (ts_idx1, local1) = resolve_gid(gid_base(gids[1]), &firstgids).unwrap();
        assert_eq!(ts_idx1, 0);
        assert_eq!(local1 + 1, 3, "local_id+1 should equal original TileIndex");
    }

    #[test]
    fn round_trip_multi_tileset_gid_resolution() {
        // Two tilesets: dungeon (tile_count=6, firstgid=1), forest (tile_count=4, firstgid=6).
        // Layer 0 from dungeon has TileIndex(2); layer 1 from forest has TileIndex(3).
        let ts1 = sample_tileset();
        let ts2 = second_tileset();
        let mut ground = TilemapData::empty(1, 1);
        ground.cells[0] = TileCell {
            index: TileIndex::new(2),
            flags: TileFlags::empty(),
        };
        let mut trees = TilemapData::empty(1, 1);
        trees.cells[0] = TileCell {
            index: TileIndex::new(3),
            flags: TileFlags::empty(),
        };
        let inputs = [
            TilesetExportInput {
                tileset: &ts1,
                image_path: "dungeon.png".into(),
            },
            TilesetExportInput {
                tileset: &ts2,
                image_path: "forest.png".into(),
            },
        ];
        let layers = [
            TiledLayerInput {
                name: "ground",
                data: &ground,
                tileset_index: 0,
            },
            TiledLayerInput {
                name: "trees",
                data: &trees,
                tileset_index: 1,
            },
        ];
        let out = export_tilemap(&inputs, &layers, &options()).unwrap();

        let firstgids = parse_tileset_firstgids(&out.tmx);
        assert_eq!(firstgids.len(), 2);
        assert_eq!(firstgids[0].0, 1); // dungeon
        assert_eq!(firstgids[1].0, 6); // forest

        // Parse both CSV blocks.
        let mut csv_iter = out.tmx.match_indices("<data encoding=\"csv\">");
        let ground_csv_pos = csv_iter.next().unwrap().0 + "<data encoding=\"csv\">".len();
        let trees_csv_pos = csv_iter.next().unwrap().0 + "<data encoding=\"csv\">".len();
        let data_ends: Vec<_> = out.tmx.match_indices("</data>").collect();
        let ground_gids = parse_csv_gids(&out.tmx[ground_csv_pos..data_ends[0].0]);
        let trees_gids = parse_csv_gids(&out.tmx[trees_csv_pos..data_ends[1].0]);

        // Ground: GID 2 → tileset 0, local_id 1 → TileIndex(2).
        let (ts_g, local_g) = resolve_gid(gid_base(ground_gids[0]), &firstgids).unwrap();
        assert_eq!(ts_g, 0, "ground layer belongs to tileset 0");
        assert_eq!(local_g + 1, 2, "ground TileIndex round-trip");

        // Trees: GID 8 → tileset 1, local_id 2 → TileIndex(3).
        let (ts_t, local_t) = resolve_gid(gid_base(trees_gids[0]), &firstgids).unwrap();
        assert_eq!(ts_t, 1, "trees layer belongs to tileset 1");
        assert_eq!(local_t + 1, 3, "trees TileIndex round-trip");
    }

    #[test]
    fn round_trip_flip_flags_survive_encoding() {
        // Flip bits must not corrupt the GID base value during round-trip.
        let ts = sample_tileset();
        let mut data = TilemapData::empty(1, 1);
        data.cells[0] = TileCell {
            index: TileIndex::new(4),
            flags: TileFlags::FLIP_X.union(TileFlags::FLIP_Y),
        };
        let layers = [TiledLayerInput {
            name: "flipped",
            data: &data,
            tileset_index: 0,
        }];
        let out = export_tilemap(&[sample_ts_input(&ts)], &layers, &options()).unwrap();

        let firstgids = parse_tileset_firstgids(&out.tmx);
        let csv_start =
            out.tmx.find("<data encoding=\"csv\">").unwrap() + "<data encoding=\"csv\">".len();
        let csv_end = out.tmx.find("</data>").unwrap();
        let gids = parse_csv_gids(&out.tmx[csv_start..csv_end]);

        let raw = gids[0];
        // High bits set for FLIP_X and FLIP_Y.
        assert_ne!(raw & GID_FLIP_X, 0);
        assert_ne!(raw & GID_FLIP_Y, 0);
        // Base GID strips flip bits; should resolve to TileIndex(4).
        let (_, local) = resolve_gid(gid_base(raw), &firstgids).unwrap();
        assert_eq!(local + 1, 4, "TileIndex round-trip with flip flags");
    }
}
