//! Tilemap operational code: autotile engine and animated tile stepping.
//!
//! The data model types (`TilemapData`, `TileCell`, `TileFlags`, `Tileset`,
//! etc.) live in [`crate::project`]. This module contains the algorithms
//! that operate on those types:
//!
//! - [`autotile`] — resolve which tile index to paint based on neighbor state.
//! - [`animate`] — resolve which source tile to display for animated tiles.

pub mod animate;
pub mod autotile;
pub mod geometry;

pub use animate::step_animation;
pub use autotile::{
    AutotileKind, AutotileRule, AutotileRuleSet, BLOB47_MASKS, NeighborCondition,
    blob47_tile_index, corner16_tile_index, minimal4_tile_index, resolve_autotile,
    trim_blob47_mask,
};
pub use geometry::cell_to_pixel;
