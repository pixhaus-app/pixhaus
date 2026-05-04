//! Tilemap editing commands.
//!
//! All commands here are stubbed until stream S06 (tilemap data structures and
//! autotile rules) lands.

use pixhaus_core::project::{FrameIndex, LayerId, SpriteId, TileCell, TileIndex};
use serde::Deserialize;
use tauri::State;

use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

/// Arguments for placing a tile on a tilemap cel.
#[derive(Debug, Deserialize)]
pub struct TilePlaceArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Target tilemap layer.
    pub layer_id: LayerId,
    /// Target frame.
    pub frame_index: FrameIndex,
    /// Column (x) of the cell in the tile grid.
    pub cell_x: u32,
    /// Row (y) of the cell in the tile grid.
    pub cell_y: u32,
    /// Tile to place.
    pub cell: TileCell,
}

/// Arguments for erasing a tile on a tilemap cel.
#[derive(Debug, Deserialize)]
pub struct TileEraseArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Target tilemap layer.
    pub layer_id: LayerId,
    /// Target frame.
    pub frame_index: FrameIndex,
    /// Column (x) of the cell to erase.
    pub cell_x: u32,
    /// Row (y) of the cell to erase.
    pub cell_y: u32,
}

/// Arguments for applying autotile rules to a region of a tilemap cel.
#[derive(Debug, Deserialize)]
pub struct AutotileArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Target tilemap layer.
    pub layer_id: LayerId,
    /// Target frame.
    pub frame_index: FrameIndex,
    /// Autotile rule set to apply (by name, resolved in S06).
    pub rule_set: String,
    /// Tile to use as the "base" for the rule set.
    pub source_tile: TileIndex,
}

/// Places a tile cell at a position on a tilemap layer.
///
/// Requires stream S06 (tilemap data structures). Returns an error until S06 lands.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn tile_place(_args: TilePlaceArgs, _state: State<'_, AppState>) -> CommandResult<()> {
    Err(AppCommandError::Unimplemented {
        stream: "S06 (tilemap)".into(),
    })
}

/// Erases a tile cell at a position on a tilemap layer.
///
/// Requires stream S06 (tilemap data structures). Returns an error until S06 lands.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn tile_erase(_args: TileEraseArgs, _state: State<'_, AppState>) -> CommandResult<()> {
    Err(AppCommandError::Unimplemented {
        stream: "S06 (tilemap)".into(),
    })
}

/// Applies autotile rules to a region of a tilemap layer.
///
/// Requires stream S06 (tilemap data structures). Returns an error until S06 lands.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn tile_autotile_apply(
    _args: AutotileArgs,
    _state: State<'_, AppState>,
) -> CommandResult<()> {
    Err(AppCommandError::Unimplemented {
        stream: "S06 (tilemap)".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::TileFlags;

    #[test]
    fn tile_place_args_constructs() {
        let args = TilePlaceArgs {
            sprite_id: SpriteId::new(1),
            layer_id: LayerId::new(2),
            frame_index: FrameIndex::new(0),
            cell_x: 3,
            cell_y: 4,
            cell: TileCell {
                index: TileIndex::new(5),
                flags: TileFlags::empty(),
            },
        };
        assert_eq!(args.cell.index.get(), 5);
    }
}
