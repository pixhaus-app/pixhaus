//! Canvas operation commands.
//!
//! Pixel-drawing operations (`draw_stroke`, `fill`, `transform`) are stubbed
//! until stream S01 (pixel buffer and blend modes) lands. Viewport and
//! selection commands are fully implemented.

use pixhaus_core::project::{
    CanvasState, LayerId, Rgba, SelectionRegion, SelectionState, SpriteId,
};
use serde::Deserialize;
use tauri::State;

use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

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
}
