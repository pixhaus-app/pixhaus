//! TSX (tileset XML) generation for the Tiled exporter.
//!
//! The atlas layout assumed here is a single horizontal strip: all
//! non-empty tiles packed left-to-right in one row. This matches the
//! output of S10's sprite-sheet packer when used to produce a tileset
//! image.
//!
//! Tiled local IDs are 0-based. Pixhaus `TileIndex(n)` maps to local
//! ID `n - 1`; `TileIndex(0)` is the empty sentinel and has no atlas
//! entry.

use pixhaus_core::project::id::TileIndex;
use pixhaus_core::project::tileset::{
    AnimLoopMode, CollisionShape, TileAnimation, TileProperties, Tileset,
};

use std::fmt::Write;

use super::{TILED_APP_VERSION, TILED_FORMAT_VERSION, xml_escape_attr};

/// Generates the `.tsx` tileset XML for `tileset`.
///
/// `image_path` is written verbatim into `<image source="…">`.
///
/// Image dimensions are computed assuming a single-row atlas:
/// `image_width = (tile_count - 1) * tile_width`, `image_height = tile_height`.
pub(super) fn build_tsx(tileset: &Tileset, image_path: &str) -> String {
    // Exclude the empty-tile sentinel (index 0) from the atlas count.
    let real_tile_count = tileset.tile_count.saturating_sub(1);
    let tile_w = tileset.tile_size.width;
    let tile_h = tileset.tile_size.height;
    let image_w = real_tile_count * tile_w;
    let image_h = tile_h;

    let mut out = String::with_capacity(512);
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    writeln!(
        out,
        "<tileset version=\"{TILED_FORMAT_VERSION}\" tiledversion=\"{TILED_APP_VERSION}\"\n\
         \x20        name=\"{}\" tilewidth=\"{tile_w}\" tileheight=\"{tile_h}\"\n\
         \x20        spacing=\"0\" margin=\"0\"\n\
         \x20        tilecount=\"{real_tile_count}\" columns=\"{real_tile_count}\">",
        xml_escape_attr(&tileset.name)
    )
    .ok();
    writeln!(
        out,
        "  <image source=\"{}\" width=\"{image_w}\" height=\"{image_h}\"/>",
        xml_escape_attr(image_path)
    )
    .ok();

    append_tile_elements(&mut out, tileset);

    out.push_str("</tileset>\n");
    out
}

/// Appends one `<tile>` element for every tile with non-default properties.
///
/// Only tiles `1..tile_count` are real atlas tiles. Tile 0 is the empty
/// sentinel and never appears in the TSX.
fn append_tile_elements(out: &mut String, tileset: &Tileset) {
    for tile_index in 1..tileset.tile_count {
        let props = tileset.tile_properties(TileIndex::new(tile_index));
        if is_default_props(props) {
            continue;
        }
        // Tiled local id = Pixhaus tile_index - 1.
        let local_id = tile_index - 1;
        writeln!(out, "  <tile id=\"{local_id}\">").ok();
        append_properties_element(out, props);
        if let Some(anim) = &props.animation {
            append_animation_element(out, anim);
        }
        out.push_str("  </tile>\n");
    }
}

fn is_default_props(props: &TileProperties) -> bool {
    props.collision == CollisionShape::None && props.animation.is_none()
}

/// Appends a `<properties>` block when the tile has non-default collision.
fn append_properties_element(out: &mut String, props: &TileProperties) {
    if props.collision == CollisionShape::None {
        return;
    }
    out.push_str("    <properties>\n");
    if props.collision == CollisionShape::Full {
        out.push_str("      <property name=\"collision\" type=\"bool\" value=\"true\"/>\n");
    }
    out.push_str("    </properties>\n");
}

/// Appends an `<animation>` block for the given tile animation.
///
/// Each frame's `tile_index` is converted to a Tiled local ID by
/// subtracting 1. [`TileIndex(0)`](TileIndex) is the empty-tile
/// sentinel and never has a local-id mapping; frames referencing it
/// are an authoring error and are skipped (with a comment in the
/// output) rather than silently encoded as the first real tile.
///
/// Tiled's animation model assumes a forward loop. Pixhaus also
/// supports [`AnimLoopMode::Once`] and [`AnimLoopMode::PingPong`];
/// those modes can't be expressed in the TSX, so the export emits a
/// comment noting the downgrade for round-trip diagnostics.
fn append_animation_element(out: &mut String, anim: &TileAnimation) {
    if anim.loop_mode != AnimLoopMode::Loop {
        writeln!(
            out,
            "    <!-- pixhaus loop_mode={:?}; Tiled supports forward-loop only -->",
            anim.loop_mode
        )
        .ok();
    }
    out.push_str("    <animation>\n");
    for frame in &anim.frames {
        let raw = frame.tile_index.get();
        if raw == 0 {
            // TileIndex(0) is the empty sentinel — no atlas slot to
            // reference. Emit a comment so the omission is debuggable
            // when Tiled disagrees with Pixhaus on frame count.
            out.push_str("      <!-- skipped frame: TileIndex(0) is the empty sentinel -->\n");
            continue;
        }
        let local_id = raw - 1;
        writeln!(
            out,
            "      <frame tileid=\"{local_id}\" duration=\"{}\"/>",
            frame.duration_ms
        )
        .ok();
    }
    out.push_str("    </animation>\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::UserData;
    use pixhaus_core::project::geometry::Size;
    use pixhaus_core::project::id::{PixelBufferId, TilesetId};
    use pixhaus_core::project::tileset::{
        AnimLoopMode, TileAnimation, TileAnimationFrame, TilesetSource,
    };

    fn sample_tileset() -> Tileset {
        Tileset {
            id: TilesetId::new(1),
            name: "dungeon".into(),
            tile_size: Size::new(16, 16),
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

    #[test]
    fn build_tsx_produces_well_formed_header() {
        let tsx = build_tsx(&sample_tileset(), "dungeon.png");
        assert!(tsx.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(tsx.contains("<tileset"));
        assert!(tsx.contains("</tileset>"));
    }

    #[test]
    fn single_row_image_dimensions_computed_from_tile_count() {
        // tile_count=6 → 5 real tiles → image 80×16.
        let tsx = build_tsx(&sample_tileset(), "dungeon.png");
        assert!(tsx.contains("width=\"80\""));
        assert!(tsx.contains("height=\"16\""));
    }

    #[test]
    fn columns_equals_real_tile_count() {
        let tsx = build_tsx(&sample_tileset(), "dungeon.png");
        assert!(tsx.contains("columns=\"5\""));
    }

    #[test]
    fn tile_with_full_collision_has_property_element() {
        let mut ts = sample_tileset();
        ts.properties = vec![
            TileProperties::default(),
            TileProperties {
                collision: CollisionShape::Full,
                animation: None,
            },
        ];
        let tsx = build_tsx(&ts, "dungeon.png");
        assert!(tsx.contains("<tile id=\"0\">"));
        assert!(tsx.contains("<property name=\"collision\" type=\"bool\" value=\"true\"/>"));
    }

    #[test]
    fn tile_with_no_collision_emits_no_properties_block() {
        let mut ts = sample_tileset();
        ts.properties = vec![
            TileProperties::default(),
            TileProperties {
                collision: CollisionShape::None,
                animation: None,
            },
        ];
        let tsx = build_tsx(&ts, "dungeon.png");
        // Tile has no interesting properties → no <tile> element at all.
        assert!(!tsx.contains("<tile id="));
    }

    #[test]
    fn tile_with_animation_has_animation_element() {
        let mut ts = sample_tileset();
        ts.properties = vec![
            TileProperties::default(),
            TileProperties {
                collision: CollisionShape::None,
                animation: Some(TileAnimation {
                    frames: vec![
                        TileAnimationFrame {
                            tile_index: TileIndex::new(1),
                            duration_ms: 150,
                        },
                        TileAnimationFrame {
                            tile_index: TileIndex::new(2),
                            duration_ms: 150,
                        },
                    ],
                    loop_mode: AnimLoopMode::PingPong,
                }),
            },
        ];
        let tsx = build_tsx(&ts, "dungeon.png");
        assert!(tsx.contains("<animation>"));
        assert!(tsx.contains("<frame tileid=\"0\" duration=\"150\"/>"));
        assert!(tsx.contains("<frame tileid=\"1\" duration=\"150\"/>"));
        assert!(tsx.contains("</animation>"));
    }

    #[test]
    fn animation_frame_tile_index_converts_to_local_id() {
        // TileIndex(3) → local id 2.
        let mut ts = sample_tileset();
        ts.properties = vec![
            TileProperties::default(),
            TileProperties {
                collision: CollisionShape::None,
                animation: Some(TileAnimation {
                    frames: vec![TileAnimationFrame {
                        tile_index: TileIndex::new(3),
                        duration_ms: 100,
                    }],
                    loop_mode: AnimLoopMode::Loop,
                }),
            },
        ];
        let tsx = build_tsx(&ts, "dungeon.png");
        assert!(tsx.contains("tileid=\"2\""));
    }

    #[test]
    fn tileset_with_no_properties_emits_no_tile_elements() {
        let tsx = build_tsx(&sample_tileset(), "dungeon.png");
        assert!(!tsx.contains("<tile id="));
    }

    #[test]
    fn inline_tileset_source_uses_provided_image_path() {
        let ts = Tileset {
            id: TilesetId::new(2),
            name: "forest".into(),
            tile_size: Size::new(8, 8),
            tile_count: 4,
            base_index: 1,
            source: TilesetSource::Inline {
                buffer: PixelBufferId::new(7),
            },
            properties: Vec::new(),
            autotile: None,
            user_data: UserData::default(),
        };
        let tsx = build_tsx(&ts, "tilesets/forest.png");
        assert!(tsx.contains("source=\"tilesets/forest.png\""));
    }

    #[test]
    fn tileset_name_with_special_chars_is_escaped() {
        let mut ts = sample_tileset();
        ts.name = "dungeon & cave".into();
        let tsx = build_tsx(&ts, "dungeon.png");
        assert!(tsx.contains("name=\"dungeon &amp; cave\""));
    }

    #[test]
    fn zero_tile_count_does_not_panic() {
        let ts = Tileset {
            id: TilesetId::new(3),
            name: "empty".into(),
            tile_size: Size::new(16, 16),
            tile_count: 0,
            base_index: 1,
            source: TilesetSource::External {
                path: "empty.png".into(),
            },
            properties: Vec::new(),
            autotile: None,
            user_data: UserData::default(),
        };
        let tsx = build_tsx(&ts, "empty.png");
        assert!(tsx.contains("tilecount=\"0\""));
    }
}
