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

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::geometry::Size;
use super::id::{PixelBufferId, TileIndex, TilesetId};
use super::user_data::UserData;

/// Grid geometry of a tilemap that draws from this tileset.
///
/// One tileset handles square, isometric, and both hex orientations via this
/// parameter; cell-to-pixel math (see `core::tilemap::cell_to_pixel`) switches
/// on it while the rest of the tilemap stack stays shape-agnostic.
///
/// Adopted from Pixelorama's `TileSetCustom` `tile_shape`
/// (`src/Classes/Cels/CelTileMap.gd`). See `THIRD_PARTY_NOTICES.md`.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
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
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
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
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CollisionShape {
    /// No collision — the tile is fully passable.
    #[default]
    None,
    /// Full-tile axis-aligned box collision.
    Full,
}

/// Loop mode for a tile animation.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
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
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
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
/// according to `loop_mode` at playback time (see `core::tilemap::animate`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
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
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
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
    /// Autotile rule set bound to this tileset, if any. Standard kinds
    /// (`Blob47`, `Corner16`, `Minimal4`) need no extra data; the
    /// `Custom` variant carries its own rules + default tile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autotile: Option<crate::tilemap::AutotileKind>,
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
            autotile: None,
            user_data: UserData::default(),
        }
    }

    #[test]
    fn inline_tileset_round_trip() {
        let t = sample_tileset();
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
            shape: TileShape::Square,
            hex_offset: None,
            tile_count: 32,
            base_index: 1,
            source: TilesetSource::External {
                path: "tilesets/shared.png".into(),
            },
            properties: Vec::new(),
            autotile: None,
            user_data: UserData::default(),
        };
        let bytes = rmp_serde::to_vec_named(&t).unwrap();
        let back: Tileset = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn base_index_round_trips_non_default_value() {
        let t = Tileset {
            id: TilesetId::new(3),
            name: "decals".into(),
            tile_size: Size::new(8, 8),
            shape: TileShape::Square,
            hex_offset: None,
            tile_count: 4,
            base_index: 5,
            source: TilesetSource::Inline {
                buffer: PixelBufferId::new(7),
            },
            properties: Vec::new(),
            autotile: None,
            user_data: UserData::default(),
        };
        let bytes = rmp_serde::to_vec_named(&t).unwrap();
        let back: Tileset = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(back.base_index, 5);
    }

    #[test]
    fn tileset_with_autotile_round_trips() {
        use crate::tilemap::{AutotileKind, AutotileRule, AutotileRuleSet, NeighborCondition};
        let t = Tileset {
            id: TilesetId::new(7),
            name: "walls".into(),
            tile_size: Size::new(16, 16),
            shape: TileShape::Square,
            hex_offset: None,
            tile_count: 8,
            base_index: 1,
            source: TilesetSource::Inline {
                buffer: PixelBufferId::new(11),
            },
            properties: Vec::new(),
            autotile: Some(AutotileKind::Custom(AutotileRuleSet {
                rules: vec![AutotileRule {
                    conditions: [NeighborCondition::Any; 8],
                    result_tile: TileIndex::new(3),
                    result_flags: crate::project::tilemap::TileFlags::empty(),
                }],
                default_tile: TileIndex::new(1),
            })),
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

    #[test]
    fn tile_properties_default_when_out_of_range() {
        let t = sample_tileset();
        let p = t.tile_properties(TileIndex::new(99));
        assert_eq!(p.collision, CollisionShape::None);
        assert!(p.animation.is_none());
    }

    #[test]
    fn tile_properties_returns_stored_data() {
        let mut t = sample_tileset();
        t.properties = vec![
            TileProperties::default(), // tile 0: empty tile
            TileProperties {
                collision: CollisionShape::Full,
                animation: None,
            },
        ];
        assert_eq!(
            t.tile_properties(TileIndex::new(1)).collision,
            CollisionShape::Full
        );
        assert_eq!(
            t.tile_properties(TileIndex::new(0)).collision,
            CollisionShape::None
        );
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
    fn tile_animation_empty_duration() {
        let anim = TileAnimation {
            frames: Vec::new(),
            loop_mode: AnimLoopMode::Once,
        };
        assert_eq!(anim.total_duration_ms(), 0);
    }

    #[test]
    fn tileset_with_properties_round_trip() {
        let mut t = sample_tileset();
        t.properties = vec![
            TileProperties::default(),
            TileProperties {
                collision: CollisionShape::Full,
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
        let bytes = rmp_serde::to_vec_named(&t).unwrap();
        let back: Tileset = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn collision_shape_default_is_none() {
        assert_eq!(CollisionShape::default(), CollisionShape::None);
    }

    #[test]
    fn anim_loop_mode_default_is_loop() {
        assert_eq!(AnimLoopMode::default(), AnimLoopMode::Loop);
    }
}
