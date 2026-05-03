//! Tilesets: the source of tiles a tilemap layer paints.
//!
//! A tileset declares the tile size and where its tile pixels live.
//! For an inline tileset, an opaque [`PixelBufferId`] points at a
//! buffer that holds all tiles packed in row-major order. For an
//! external tileset, a relative `path` lets the loader resolve to a
//! sibling image file.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::geometry::Size;
use super::id::{PixelBufferId, TilesetId};
use super::user_data::UserData;

/// A tileset.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Tileset {
    /// Stable identifier.
    pub id: TilesetId,
    /// Display name in the tileset panel.
    pub name: String,
    /// Width and height of one tile in pixels.
    pub tile_size: Size,
    /// Number of tiles the tileset declares. Index `0` is the empty
    /// tile by convention; `tile_count` includes that index.
    pub tile_count: u32,
    /// Where the tile pixels live.
    pub source: TilesetSource,
    /// Free-form user metadata.
    #[serde(skip_serializing_if = "UserData::is_empty", default)]
    pub user_data: UserData,
}

/// Where a tileset's pixel data is stored.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum TilesetSource {
    /// Tiles packed into an in-document pixel buffer. The buffer
    /// layout is row-major with `tile_count` tiles laid out left to
    /// right; the pixel-buffer subsystem owns the exact stride.
    Inline {
        /// Handle to the packed tile buffer.
        buffer: PixelBufferId,
    },
    /// Tiles loaded from an external image file relative to the
    /// project root. Used to share a tileset across projects.
    External {
        /// Project-root-relative path to the tileset image.
        path: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_tileset_round_trip() {
        let t = Tileset {
            id: TilesetId::new(1),
            name: "dungeon".into(),
            tile_size: Size::new(16, 16),
            tile_count: 64,
            source: TilesetSource::Inline {
                buffer: PixelBufferId::new(42),
            },
            user_data: UserData::default(),
        };
        let bytes = rmp_serde::to_vec_named(&t).unwrap();
        let back: Tileset = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn external_tileset_round_trip() {
        let t = Tileset {
            id: TilesetId::new(2),
            name: "shared".into(),
            tile_size: Size::new(8, 8),
            tile_count: 32,
            source: TilesetSource::External {
                path: "tilesets/shared.png".into(),
            },
            user_data: UserData::default(),
        };
        let bytes = rmp_serde::to_vec_named(&t).unwrap();
        let back: Tileset = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn tileset_source_uses_kind_tag() {
        let s = TilesetSource::External {
            path: "x.png".into(),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "{\"kind\":\"external\",\"path\":\"x.png\"}");
    }
}
