//! Tiled-compatible `.tmx` + `.tsx` export (stream S12).
//!
//! Exports one frame's tilemap layers from a Pixhaus sprite as a pair of
//! Tiled XML files ready for Unity's `SuperTiled2Unity` importer.
//!
//! Output targets Tiled 1.10 format with CSV-encoded layer data and an
//! external TSX tileset reference.
//!
//! # Usage
//!
//! ```no_run
//! use pixhaus_io::tiled::{export_tilemap, TiledExportOptions, TiledLayerInput};
//! use pixhaus_core::project::tilemap::TilemapData;
//! use pixhaus_core::project::tileset::Tileset;
//!
//! # fn tileset() -> Tileset { todo!() }
//! # fn data() -> TilemapData { todo!() }
//!
//! let output = export_tilemap(
//!     &tileset(),
//!     &[TiledLayerInput { name: "ground", data: &data() }],
//!     &TiledExportOptions {
//!         name: "dungeon".into(),
//!         tileset_image_path: "dungeon.png".into(),
//!     },
//! )?;
//! std::fs::write("dungeon.tmx", &output.tmx)?;
//! std::fs::write("dungeon.tsx", &output.tsx)?;
//! # Ok::<(), pixhaus_io::Error>(())
//! ```

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

/// One tilemap layer contributed to a [`export_tilemap`] call.
pub struct TiledLayerInput<'a> {
    /// Display name for the Tiled `<layer>` element.
    pub name: &'a str,
    /// Tile grid data for this layer.
    pub data: &'a TilemapData,
}

/// Options controlling a [`export_tilemap`] call.
pub struct TiledExportOptions {
    /// Name used for both `<map>` and `<tileset>` elements. The TSX
    /// filename referenced in the TMX is derived as `"{name}.tsx"`.
    pub name: String,
    /// Path to the tileset PNG relative to the `.tmx` file on disk.
    /// Written verbatim into the TSX `<image source="…">` attribute.
    pub tileset_image_path: String,
}

/// Output of [`export_tilemap`]: raw UTF-8 XML for both files.
pub struct TiledExportOutput {
    /// Contents of the `.tmx` map file.
    pub tmx: String,
    /// Contents of the companion `.tsx` tileset file.
    pub tsx: String,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Export tilemap layers as a Tiled-compatible `.tmx` + `.tsx` pair.
///
/// All layers in `layers` must share the same `width` and `height`.
/// An empty `layers` slice is valid and produces a `.tmx` with no
/// `<layer>` elements and a `0×0` map size.
///
/// The first GID in the tileset is always `1`. Pixhaus `TileIndex(0)`
/// is the empty-tile sentinel and always encodes as GID `0`.
///
/// # Errors
///
/// - [`Error::TiledLayerSizeMismatch`] when two or more layers have
///   different `width` or `height` values.
pub fn export_tilemap(
    tileset: &Tileset,
    layers: &[TiledLayerInput<'_>],
    options: &TiledExportOptions,
) -> Result<TiledExportOutput> {
    validate_layers(layers)?;
    let tsx = tileset_xml::build_tsx(tileset, options);
    let tmx = build_tmx(tileset, layers, options);
    Ok(TiledExportOutput { tmx, tsx })
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

// ── TMX building ──────────────────────────────────────────────────────────────

fn build_tmx(
    tileset: &Tileset,
    layers: &[TiledLayerInput<'_>],
    options: &TiledExportOptions,
) -> String {
    let tile_w = tileset.tile_size.width;
    let tile_h = tileset.tile_size.height;
    let (map_w, map_h) = layers
        .first()
        .map_or((0, 0), |l| (l.data.width, l.data.height));
    // No real tilemap has 2^32 layers; saturating to u32::MAX keeps the
    // nextlayerid field valid XML for any practical input.
    let next_layer_id = u32::try_from(layers.len())
        .unwrap_or(u32::MAX)
        .saturating_add(1);
    let tsx_file = format!("{}.tsx", options.name);

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
    writeln!(
        out,
        "  <tileset firstgid=\"1\" source=\"{}\"/>",
        xml_escape_attr(&tsx_file)
    )
    .ok();
    out.push('\n');
    for (i, layer) in layers.iter().enumerate() {
        let id = u32::try_from(i).unwrap_or(u32::MAX).saturating_add(1);
        append_layer_xml(&mut out, id, layer);
        out.push('\n');
    }
    out.push_str("</map>\n");
    out
}

fn append_layer_xml(out: &mut String, id: u32, layer: &TiledLayerInput<'_>) {
    let w = layer.data.width;
    let h = layer.data.height;
    writeln!(
        out,
        "  <layer id=\"{id}\" name=\"{}\" width=\"{w}\" height=\"{h}\">",
        xml_escape_attr(layer.name)
    )
    .ok();
    out.push_str("    <data encoding=\"csv\">\n");
    out.push_str(&encode_csv(layer.data));
    out.push_str("    </data>\n");
    out.push_str("  </layer>\n");
}

// ── CSV encoding ──────────────────────────────────────────────────────────────

/// Encodes `data` as a CSV block matching Tiled's format.
///
/// Rows are on separate lines separated by a comma: all rows except the
/// last end with a trailing comma before the newline, matching Tiled's
/// own export. Empty cells encode as `0`; occupied cells encode as a
/// `u32` with Tiled flip bits OR-ed into the high three bits.
fn encode_csv(data: &TilemapData) -> String {
    let mut rows: Vec<String> = Vec::with_capacity(data.height as usize);
    for row in 0..data.height {
        let mut parts: Vec<String> = Vec::with_capacity(data.width as usize);
        for col in 0..data.width {
            // col/row are guaranteed in-range; unwrap_or_default is a
            // belt-and-suspenders guard against future structural changes.
            let cell = data.cell(col, row).unwrap_or_default();
            parts.push(encode_gid(cell).to_string());
        }
        rows.push(parts.join(","));
    }
    // join(",\n") places a comma at the end of every row except the last,
    // which is the format Tiled itself writes.
    let mut csv = rows.join(",\n");
    csv.push('\n');
    csv
}

// ── GID encoding ─────────────────────────────────────────────────────────────

/// Encodes a single [`TileCell`] as a Tiled GID.
///
/// With `firstgid = 1` the mapping is:
///
/// - `TileIndex(0)` (empty sentinel) → `0`
/// - `TileIndex(n)` → `n` with Tiled flip bits OR-ed into bits 31–29
///
/// Derivation: Tiled local id = `tile_index - 1`; GID = `firstgid +
/// local_id = 1 + (n - 1) = n`.
fn encode_gid(cell: TileCell) -> u32 {
    if cell.is_empty() {
        return 0;
    }
    let gid = cell.index.get();
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
        AnimLoopMode, CollisionShape, TileAnimation, TileAnimationFrame, TileProperties, Tileset,
        TilesetSource,
    };

    fn sample_tileset() -> Tileset {
        Tileset {
            id: TilesetId::new(1),
            name: "dungeon".into(),
            tile_size: Size::new(16, 16),
            // tile_count includes the empty-tile sentinel at index 0.
            tile_count: 6,
            base_index: 1,
            source: TilesetSource::External {
                path: "dungeon.png".into(),
            },
            properties: Vec::new(),
            user_data: UserData::default(),
        }
    }

    fn options() -> TiledExportOptions {
        TiledExportOptions {
            name: "dungeon".into(),
            tileset_image_path: "dungeon.png".into(),
        }
    }

    // ── GID encoding ─────────────────────────────────────────────────────────

    #[test]
    fn empty_cell_encodes_as_zero() {
        assert_eq!(encode_gid(TileCell::empty()), 0);
    }

    #[test]
    fn no_flip_encodes_as_tile_index() {
        let cell = TileCell {
            index: TileIndex::new(3),
            flags: TileFlags::empty(),
        };
        assert_eq!(encode_gid(cell), 3);
    }

    #[test]
    fn flip_x_sets_bit_31() {
        let cell = TileCell {
            index: TileIndex::new(4),
            flags: TileFlags::FLIP_X,
        };
        assert_eq!(encode_gid(cell), 4 | 0x8000_0000);
    }

    #[test]
    fn flip_y_sets_bit_30() {
        let cell = TileCell {
            index: TileIndex::new(4),
            flags: TileFlags::FLIP_Y,
        };
        assert_eq!(encode_gid(cell), 4 | 0x4000_0000);
    }

    #[test]
    fn flip_diagonal_sets_bit_29() {
        let cell = TileCell {
            index: TileIndex::new(4),
            flags: TileFlags::FLIP_DIAGONAL,
        };
        assert_eq!(encode_gid(cell), 4 | 0x2000_0000);
    }

    #[test]
    fn flip_x_and_y_matches_reference_dungeon_value() {
        // dungeon.tmx encodes tile 4 | flip-X | flip-Y as 3221225476.
        let cell = TileCell {
            index: TileIndex::new(4),
            flags: TileFlags::FLIP_X.union(TileFlags::FLIP_Y),
        };
        assert_eq!(encode_gid(cell), 3_221_225_476);
    }

    #[test]
    fn flip_x_only_matches_reference_dungeon_value() {
        // dungeon.tmx: tile 4 | flip-X = 2147483652
        let cell = TileCell {
            index: TileIndex::new(4),
            flags: TileFlags::FLIP_X,
        };
        assert_eq!(encode_gid(cell), 2_147_483_652);
    }

    #[test]
    fn flip_y_only_matches_reference_dungeon_value() {
        // dungeon.tmx: tile 4 | flip-Y = 1073741828
        let cell = TileCell {
            index: TileIndex::new(4),
            flags: TileFlags::FLIP_Y,
        };
        assert_eq!(encode_gid(cell), 1_073_741_828);
    }

    // ── CSV encoding ──────────────────────────────────────────────────────────

    #[test]
    fn all_empty_2x2_encodes_to_zeros() {
        let csv = encode_csv(&TilemapData::empty(2, 2));
        assert_eq!(csv, "0,0,\n0,0\n");
    }

    #[test]
    fn single_cell_encodes_tile_index() {
        let mut data = TilemapData::empty(1, 1);
        data.cells[0] = TileCell {
            index: TileIndex::new(2),
            flags: TileFlags::empty(),
        };
        assert_eq!(encode_csv(&data), "2\n");
    }

    #[test]
    fn last_row_has_no_trailing_comma() {
        let mut data = TilemapData::empty(3, 2);
        data.cells[3] = TileCell {
            index: TileIndex::new(1),
            flags: TileFlags::empty(),
        };
        let csv = encode_csv(&data);
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
        let csv = encode_csv(&data);
        assert_eq!(csv, "2147483652\n");
    }

    // ── Export ────────────────────────────────────────────────────────────────

    #[test]
    fn empty_layers_produces_valid_tmx() {
        let out = export_tilemap(&sample_tileset(), &[], &options()).unwrap();
        assert!(out.tmx.contains("<map"));
        assert!(out.tmx.contains("</map>"));
        assert!(!out.tmx.contains("<layer"));
    }

    #[test]
    fn single_layer_appears_in_tmx() {
        let data = TilemapData::empty(4, 3);
        let layers = [TiledLayerInput {
            name: "ground",
            data: &data,
        }];
        let out = export_tilemap(&sample_tileset(), &layers, &options()).unwrap();
        assert!(out.tmx.contains("name=\"ground\""));
        assert!(out.tmx.contains("width=\"4\""));
        assert!(out.tmx.contains("height=\"3\""));
        assert!(out.tmx.contains("encoding=\"csv\""));
    }

    #[test]
    fn map_dimensions_match_first_layer() {
        let data = TilemapData::empty(8, 5);
        let layers = [TiledLayerInput {
            name: "base",
            data: &data,
        }];
        let out = export_tilemap(&sample_tileset(), &layers, &options()).unwrap();
        assert!(out.tmx.contains("width=\"8\""));
        assert!(out.tmx.contains("height=\"5\""));
    }

    #[test]
    fn tsx_filename_referenced_in_tmx() {
        let out = export_tilemap(&sample_tileset(), &[], &options()).unwrap();
        assert!(out.tmx.contains("source=\"dungeon.tsx\""));
    }

    #[test]
    fn layer_ids_are_one_based_and_sequential() {
        let d = TilemapData::empty(2, 2);
        let layers = [
            TiledLayerInput {
                name: "a",
                data: &d,
            },
            TiledLayerInput {
                name: "b",
                data: &d,
            },
        ];
        let out = export_tilemap(&sample_tileset(), &layers, &options()).unwrap();
        assert!(out.tmx.contains("id=\"1\""));
        assert!(out.tmx.contains("id=\"2\""));
        assert!(out.tmx.contains("nextlayerid=\"3\""));
    }

    #[test]
    fn tile_dimensions_from_tileset_in_map_header() {
        let out = export_tilemap(&sample_tileset(), &[], &options()).unwrap();
        assert!(out.tmx.contains("tilewidth=\"16\""));
        assert!(out.tmx.contains("tileheight=\"16\""));
    }

    #[test]
    fn mismatched_layer_sizes_return_error() {
        let d1 = TilemapData::empty(4, 4);
        let d2 = TilemapData::empty(8, 4);
        let layers = [
            TiledLayerInput {
                name: "a",
                data: &d1,
            },
            TiledLayerInput {
                name: "b",
                data: &d2,
            },
        ];
        let result = export_tilemap(&sample_tileset(), &layers, &options());
        assert!(matches!(
            result,
            Err(Error::TiledLayerSizeMismatch { layer_index: 1, .. })
        ));
    }

    #[test]
    fn tsx_contains_tileset_name_and_tile_dimensions() {
        let out = export_tilemap(&sample_tileset(), &[], &options()).unwrap();
        assert!(out.tsx.contains("name=\"dungeon\""));
        assert!(out.tsx.contains("tilewidth=\"16\""));
        assert!(out.tsx.contains("tileheight=\"16\""));
    }

    #[test]
    fn tsx_tilecount_excludes_empty_sentinel() {
        // tile_count=6 → 5 real tiles in the atlas.
        let out = export_tilemap(&sample_tileset(), &[], &options()).unwrap();
        assert!(out.tsx.contains("tilecount=\"5\""));
    }

    #[test]
    fn tsx_image_source_uses_provided_path() {
        let out = export_tilemap(&sample_tileset(), &[], &options()).unwrap();
        assert!(out.tsx.contains("source=\"dungeon.png\""));
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
        let out = export_tilemap(&ts, &[], &options()).unwrap();
        // TileIndex(1) → Tiled local id 0.
        assert!(out.tsx.contains("<tile id=\"0\">"));
        assert!(out.tsx.contains("name=\"collision\""));
        assert!(out.tsx.contains("value=\"true\""));
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
        let out = export_tilemap(&ts, &[], &options()).unwrap();
        assert!(out.tsx.contains("<animation>"));
        assert!(out.tsx.contains("duration=\"100\""));
        assert!(out.tsx.contains("duration=\"200\""));
    }

    #[test]
    fn default_tile_properties_emit_no_tile_element() {
        let out = export_tilemap(&sample_tileset(), &[], &options()).unwrap();
        // sample_tileset has no properties set → no <tile> elements.
        assert!(!out.tsx.contains("<tile id="));
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
            tile_count: 5,
            base_index: 1,
            source: pixhaus_core::project::tileset::TilesetSource::Inline {
                buffer: PixelBufferId::new(1),
            },
            properties: Vec::new(),
            user_data: UserData::default(),
        };
        let opts = TiledExportOptions {
            name: "forest".into(),
            tileset_image_path: "tilesets/forest.png".into(),
        };
        let out = export_tilemap(&ts, &[], &opts).unwrap();
        assert!(out.tsx.contains("source=\"tilesets/forest.png\""));
    }
}
