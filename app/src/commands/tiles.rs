//! Tilemap editing commands.

use pixhaus_core::project::{
    FrameIndex, LayerId, PixelBufferId, Size, SpriteId, TileCell, TileIndex, Tileset, TilesetId,
    TilesetSource, UserData,
};
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

// ── Tileset management ────────────────────────────────────────────────────────

/// Arguments for adding a new tileset to a sprite.
#[derive(Debug, Deserialize)]
pub struct TilesetAddArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Display name for the new tileset.
    pub name: String,
    /// Width of each tile in pixels.
    pub tile_width: u32,
    /// Height of each tile in pixels.
    pub tile_height: u32,
}

/// Lists all tilesets in a sprite.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn tileset_list(
    sprite_id: SpriteId,
    state: State<'_, AppState>,
) -> CommandResult<Vec<Tileset>> {
    let doc = state.doc.read().await;
    let sprite = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?
        .sprites
        .iter()
        .find(|s| s.id == sprite_id)
        .ok_or(AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?;
    Ok(sprite.tilesets.clone())
}

/// Adds a new empty tileset to a sprite.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn tileset_add(
    args: TilesetAddArgs,
    state: State<'_, AppState>,
) -> CommandResult<Tileset> {
    let mut doc = state.doc.write().await;
    let id = TilesetId::new(doc.next_id);
    doc.next_id += 1;
    let tileset = {
        let sprite = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?
            .sprites
            .iter_mut()
            .find(|s| s.id == args.sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(args.sprite_id.get()),
            })?;
        let tileset = Tileset {
            id,
            name: args.name,
            tile_size: Size::new(args.tile_width, args.tile_height),
            // tile_count = 1 (just the implicit empty tile at index 0).
            // The pixel buffer subsystem (S01) grows this as tiles are added.
            tile_count: 1,
            base_index: 1,
            source: TilesetSource::Inline {
                buffer: PixelBufferId::new(0),
            },
            properties: Vec::new(),
            user_data: UserData::default(),
        };
        sprite.tilesets.push(tileset.clone());
        tileset
    };
    doc.dirty = true;
    Ok(tileset)
}

/// Renames a tileset.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn tileset_rename(
    sprite_id: SpriteId,
    tileset_id: TilesetId,
    name: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let sprite = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?
        .sprites
        .iter_mut()
        .find(|s| s.id == sprite_id)
        .ok_or(AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?;
    let tileset = sprite
        .tilesets
        .iter_mut()
        .find(|t| t.id == tileset_id)
        .ok_or(AppCommandError::NotFound {
            entity: "tileset".into(),
            id: u64::from(tileset_id.get()),
        })?;
    tileset.name = name;
    doc.dirty = true;
    Ok(())
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

    #[test]
    fn tileset_add_args_fields() {
        let args = TilesetAddArgs {
            sprite_id: SpriteId::new(1),
            name: "dungeon".into(),
            tile_width: 16,
            tile_height: 16,
        };
        assert_eq!(args.tile_width, 16);
        assert_eq!(args.tile_height, 16);
    }

    #[test]
    fn tileset_add_uses_default_user_data() {
        let ud = UserData::default();
        assert!(ud.is_empty());
    }
}
