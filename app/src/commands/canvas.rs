//! Canvas operation commands.
//!
//! Pixel-drawing operations (`draw_stroke`, `fill`, `transform`) are stubbed
//! until stream S01 (pixel buffer and blend modes) lands. Viewport, selection,
//! and composite-info commands are fully implemented.

use pixhaus_core::project::{
    CanvasState, LayerId, Rgba, SelectionRegion, SelectionState, SpriteId,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

/// Tile size used by the canvas renderer, in canvas pixels per side.
pub const TILE_SIZE: u32 = 256;

/// Arguments for a freehand stroke. Requires S01.
#[derive(Debug, Deserialize)]
pub struct DrawStrokeArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Target layer.
    pub layer_id: LayerId,
    /// Target frame (0-indexed).
    pub frame_index: u32,
    /// Stroke path as `[x, y]` pairs in canvas coordinates.
    pub points: Vec<[f32; 2]>,
    /// Stroke color.
    pub color: Rgba,
    /// Per-point pressure values, same length as `points`. `1.0` = full pressure.
    pub pressure: Vec<f32>,
}

/// Arguments for a flood fill. Requires S01.
#[derive(Debug, Deserialize)]
pub struct FillArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Target layer.
    pub layer_id: LayerId,
    /// Target frame (0-indexed).
    pub frame_index: u32,
    /// Seed point in canvas coordinates.
    pub x: i32,
    /// Seed point in canvas coordinates.
    pub y: i32,
    /// Fill color.
    pub color: Rgba,
    /// Tolerance for color matching (`0` = exact, `255` = match all).
    pub tolerance: u8,
}

/// Arguments for a transform operation. Requires S01.
#[derive(Debug, Deserialize)]
pub struct TransformArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Target layer.
    pub layer_id: LayerId,
    /// Target frame (0-indexed).
    pub frame_index: u32,
    /// Translation delta in canvas pixels.
    pub translate_x: i32,
    /// Translation delta in canvas pixels.
    pub translate_y: i32,
    /// Horizontal flip.
    pub flip_x: bool,
    /// Vertical flip.
    pub flip_y: bool,
    /// Clockwise rotation in 90-degree steps (0–3).
    pub rotate_cw90: u8,
}

/// Metadata returned by [`canvas_composite`].
///
/// Tells the frontend how many tiles cover the sprite and their dimensions.
/// Actual pixel data arrives later via `canvas:tile-dirty` events emitted when
/// drawing operations change the pixel buffer (streams S15+).
#[derive(Debug, Serialize)]
pub struct CanvasComposite {
    /// Sprite canvas width in pixels.
    pub sprite_width: u32,
    /// Sprite canvas height in pixels.
    pub sprite_height: u32,
    /// Tile side length in canvas pixels.
    pub tile_size: u32,
    /// Number of tile columns (`ceil(sprite_width / tile_size)`).
    pub tiles_x: u32,
    /// Number of tile rows (`ceil(sprite_height / tile_size)`).
    pub tiles_y: u32,
}

/// Returns the tile grid dimensions for the given sprite.
///
/// The renderer calls this once when a sprite becomes active to learn its
/// canvas size and tile layout.  Pixel data for each tile arrives via
/// `canvas:tile-dirty` events as editing operations modify the pixel buffers.
///
/// Currently returns metadata only (no pixel bytes).  Tile data will be
/// emitted by drawing commands once stream S15 integrates S01's pixel buffers.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_composite(
    sprite_id: SpriteId,
    state: State<'_, AppState>,
) -> CommandResult<CanvasComposite> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    let sprite = project
        .sprites
        .iter()
        .find(|s| s.id == sprite_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?;

    let w = sprite.canvas.width;
    let h = sprite.canvas.height;
    Ok(CanvasComposite {
        sprite_width: w,
        sprite_height: h,
        tile_size: TILE_SIZE,
        tiles_x: w.div_ceil(TILE_SIZE),
        tiles_y: h.div_ceil(TILE_SIZE),
    })
}

/// Paints a freehand stroke on a layer cel.
///
/// Requires stream S01 (pixel buffers). Returns an error until S01 lands.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_draw_stroke(
    _args: DrawStrokeArgs,
    _state: State<'_, AppState>,
) -> CommandResult<()> {
    Err(AppCommandError::Unimplemented {
        stream: "S01 (pixel buffers)".into(),
    })
}

/// Flood-fills a contiguous region on a layer cel.
///
/// Requires stream S01 (pixel buffers). Returns an error until S01 lands.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_fill(_args: FillArgs, _state: State<'_, AppState>) -> CommandResult<()> {
    Err(AppCommandError::Unimplemented {
        stream: "S01 (pixel buffers)".into(),
    })
}

/// Applies a geometric transform (translate, flip, rotate) to a layer cel.
///
/// Requires stream S01 (pixel buffers). Returns an error until S01 lands.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_transform(
    _args: TransformArgs,
    _state: State<'_, AppState>,
) -> CommandResult<()> {
    Err(AppCommandError::Unimplemented {
        stream: "S01 (pixel buffers)".into(),
    })
}

/// Sets the canvas selection. Pass `None` for `region` to clear the selection.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_set_selection(
    region: Option<SelectionRegion>,
    anchor_layer: Option<LayerId>,
    state: State<'_, AppState>,
) -> CommandResult<SelectionState> {
    let mut doc = state.doc.write().await;
    let selection = SelectionState {
        region,
        anchor_layer,
    };
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.selection = selection.clone();
    doc.dirty = true;
    Ok(selection)
}

/// Replaces the entire canvas viewport state (scroll, zoom, active ids, toggles).
///
/// The UI owns viewport state and pushes it here on every meaningful change
/// so that save/load can restore the last viewport.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn canvas_set_viewport(
    canvas: CanvasState,
    state: State<'_, AppState>,
) -> CommandResult<CanvasState> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.canvas = canvas.clone();
    doc.dirty = true;
    Ok(canvas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::{Project, Size, Sprite};

    #[test]
    fn draw_stroke_args_pressure_and_points_match() {
        let args = DrawStrokeArgs {
            sprite_id: SpriteId::new(1),
            layer_id: LayerId::new(2),
            frame_index: 0,
            points: vec![[0.0, 0.0], [1.0, 1.0]],
            color: Rgba::opaque(255, 0, 0),
            pressure: vec![1.0, 0.8],
        };
        assert_eq!(args.points.len(), args.pressure.len());
    }

    #[test]
    fn canvas_composite_tile_counts_round_up() {
        // 256 × 256 sprite → exactly 1 tile in each dimension.
        let w = 256u32;
        let h = 256u32;
        assert_eq!(w.div_ceil(TILE_SIZE), 1);
        assert_eq!(h.div_ceil(TILE_SIZE), 1);

        // 257 × 257 → 2 tiles in each dimension.
        let w2 = 257u32;
        let h2 = 257u32;
        assert_eq!(w2.div_ceil(TILE_SIZE), 2);
        assert_eq!(h2.div_ceil(TILE_SIZE), 2);
    }

    #[test]
    fn canvas_composite_large_sprite_tile_grid() {
        // 4096 × 2048 sprite → 16 × 8 tile grid.
        assert_eq!(4096u32.div_ceil(TILE_SIZE), 16);
        assert_eq!(2048u32.div_ceil(TILE_SIZE), 8);
    }

    #[test]
    fn canvas_composite_metadata_matches_sprite() {
        let mut project = Project::new("test");
        let sprite = Sprite::empty(SpriteId::new(1), "hero", Size::new(64, 48));
        project.sprites.push(sprite);

        let sprite = project.sprites.first().unwrap();
        let composite = CanvasComposite {
            sprite_width: sprite.canvas.width,
            sprite_height: sprite.canvas.height,
            tile_size: TILE_SIZE,
            tiles_x: sprite.canvas.width.div_ceil(TILE_SIZE),
            tiles_y: sprite.canvas.height.div_ceil(TILE_SIZE),
        };
        assert_eq!(composite.sprite_width, 64);
        assert_eq!(composite.sprite_height, 48);
        assert_eq!(composite.tiles_x, 1);
        assert_eq!(composite.tiles_y, 1);
    }
}
