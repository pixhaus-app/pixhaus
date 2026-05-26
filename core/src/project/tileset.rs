//! Tilesets: the source of tiles a tilemap layer paints.
//!
//! A tileset declares the tile size and where its tile pixels live.
//! For an inline tileset, an opaque [`PixelBufferId`] points at a
//! buffer that holds all tiles packed in row-major order. For an
//! external tileset, a relative `path` lets the loader resolve to a
//! sibling image file.
//!
//! Per-tile metadata (collision, animation) lives in [`TileProperties`],
//! stored at `tileset.properties[tile_index]`.
//!
//! Note: the main checkout's `autotile` field is dropped in this slice —
//! it referenced the top-level `tilemap` autotile subsystem, which the
//! vertical slice does not port.

use serde::{Deserialize, Serialize};

use super::geometry::Size;
use super::id::{PixelBufferId, TileIndex, TilesetId};
use super::user_data::UserData;

/// Grid geometry of a tilemap that draws from this tileset.
///
/// One tileset handles square, isometric, and both hex orientations via this
/// parameter; cell-to-pixel math switches on it while the rest of the tilemap
/// stack stays shape-agnostic.
///
/// Adopted from Pixelorama's `TileSetCustom` `tile_shape`
/// (`src/Classes/Cels/CelTileMap.gd`). See `THIRD_PARTY_NOTICES.md`.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileShape {
    /// Axis-aligned square grid. The default.
    #[default]
    Square,
    /// Isometric diamond grid.
    Isometric,
    /// Hexagonal grid with pointy-top tiles (offset rows).
    HexPointy,
    /// Hexagonal grid with flat-top tiles (offset columns).
    HexFlat,
}

/// Which rows or columns take the half-step offset in a hex grid.
///
/// Only meaningful when [`TileShape`] is `HexPointy` (rows) or `HexFlat`
/// (columns). Adopted from Pixelorama's `tile_offset_axis`.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HexOffsetAxis {
    /// Odd-indexed rows are pushed half a tile right (pointy-top).
    #[default]
    OddRow,
    /// Even-indexed rows are pushed half a tile right (pointy-top).
    EvenRow,
    /// Odd-indexed columns are pushed half a tile down (flat-top).
    OddCol,
    /// Even-indexed columns are pushed half a tile down (flat-top).
    EvenCol,
}

/// Collision shape for a tile.
///
/// Full-tile collision is the common case for opaque wall tiles.
/// Explicit `None` marks a tile as passable (decorations, overlays).
/// Additional shapes (slope, partial) are deferred to future streams.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollisionShape {
    /// No collision — the tile is fully passable.
    #[default]
    None,
    /// Full-tile axis-aligned box collision.
    Full,
}

/// Loop mode for a tile animation.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimLoopMode {
    /// Restart from the first frame after the last.
    #[default]
    Loop,
    /// Freeze on the last frame after playing once.
    Once,
    /// Reverse direction at each endpoint.
    PingPong,
}

/// One frame in a tile animation sequence.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TileAnimationFrame {
    /// Source tile index inside the same tileset.
    pub tile_index: TileIndex,
    /// Display duration of this frame in milliseconds.
    pub duration_ms: u32,
}

/// Animation sequence attached to a tile.
///
/// Animated tiles are the canonical representation: tile `n` in the
/// tileset carries this struct; the renderer advances through `frames`
/// according to `loop_mode` at playback time.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TileAnimation {
    /// Ordered list of frames (must be non-empty after validation).
    pub frames: Vec<TileAnimationFrame>,
    /// What happens when the last frame finishes.
    pub loop_mode: AnimLoopMode,
}

impl TileAnimation {
    /// Total duration of one forward pass through `frames`, in milliseconds.
    ///
    /// Returns `0` if `frames` is empty.
    #[must_use]
    pub fn total_duration_ms(&self) -> u64 {
        self.frames.iter().map(|f| u64::from(f.duration_ms)).sum()
    }
}

/// Per-tile metadata stored alongside the tileset.
///
/// Indexed by [`TileIndex`] — the entry at position `i` describes tile `i`.
/// The `Tileset` stores a sparse `Vec<TileProperties>` that is resized to
/// match `tile_count`; out-of-bounds indices default to
/// `TileProperties::default()` (no collision, no animation).
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TileProperties {
    /// Collision shape for this tile.
    #[serde(default, skip_serializing_if = "is_no_collision")]
    pub collision: CollisionShape,
    /// Animation sequence, if any.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub animation: Option<TileAnimation>,
}

// serde skip_serializing_if requires &T, so the Copy argument stays by reference.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_no_collision(c: &CollisionShape) -> bool {
    matches!(*c, CollisionShape::None)
}

/// A tileset.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Tileset {
    /// Stable identifier.
    pub id: TilesetId,
    /// Display name in the tileset panel.
    pub name: String,
    /// Width and height of one tile in pixels.
    pub tile_size: Size,
    /// Grid geometry tilemaps using this tileset are laid out with.
    /// Defaults to [`TileShape::Square`]; files written before tile shapes
    /// existed load as square and re-save with the field omitted.
    #[serde(default, skip_serializing_if = "is_square")]
    pub shape: TileShape,
    /// Which rows/columns take the half-step offset for hex shapes. `None`
    /// for square and isometric grids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hex_offset: Option<HexOffsetAxis>,
    /// Number of tiles the tileset declares. Index `0` is the empty
    /// tile by convention; `tile_count` includes that index.
    pub tile_count: u32,
    /// Index displayed for the first non-empty tile. Aseprite stores
    /// this so a tileset can present itself as 1-based ("tile 1") or
    /// 0-based ("tile 0") in its UI without renumbering pixel data.
    /// Tile id `0` always remains the empty tile internally.
    #[serde(default = "default_base_index")]
    pub base_index: i16,
    /// Where the tile pixels live.
    pub source: TilesetSource,
    /// Per-tile metadata. Length may be less than `tile_count`; indices
    /// beyond the vec length implicitly have `TileProperties::default()`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<TileProperties>,
    /// Free-form user metadata.
    #[serde(skip_serializing_if = "UserData::is_empty", default)]
    pub user_data: UserData,
}

impl Tileset {
    /// Returns the [`TileProperties`] for `index`.
    ///
    /// Returns a default (no-collision, no-animation) if `index` is
    /// beyond the stored `properties` vec.
    #[must_use]
    pub fn tile_properties(&self, index: TileIndex) -> &TileProperties {
        self.properties
            .get(index.get() as usize)
            .map_or(&DEFAULT_TILE_PROPS, |p| p)
    }
}

static DEFAULT_TILE_PROPS: TileProperties = TileProperties {
    collision: CollisionShape::None,
    animation: None,
};

fn default_base_index() -> i16 {
    1
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_square(shape: &TileShape) -> bool {
    matches!(*shape, TileShape::Square)
}

/// Where a tileset's pixel data is stored.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
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

    fn sample_tileset() -> Tileset {
        Tileset {
            id: TilesetId::new(1),
            name: "dungeon".into(),
            tile_size: Size::new(16, 16),
            shape: TileShape::Square,
            hex_offset: None,
            tile_count: 64,
            base_index: 1,
            source: TilesetSource::Inline {
                buffer: PixelBufferId::new(42),
            },
            properties: Vec::new(),
            user_data: UserData::default(),
        }
    }

    #[test]
    fn inline_tileset_round_trip() {
        let t = sample_tileset();
        let json = serde_json::to_string(&t).unwrap();
        let back: Tileset = serde_json::from_str(&json).unwrap();
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

    #[test]
    fn tile_properties_default_when_out_of_range() {
        let t = sample_tileset();
        let p = t.tile_properties(TileIndex::new(99));
        assert_eq!(p.collision, CollisionShape::None);
        assert!(p.animation.is_none());
    }

    #[test]
    fn tile_animation_total_duration() {
        let anim = TileAnimation {
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
        };
        assert_eq!(anim.total_duration_ms(), 300);
    }

    #[test]
    fn collision_shape_default_is_none() {
        assert_eq!(CollisionShape::default(), CollisionShape::None);
    }
}
