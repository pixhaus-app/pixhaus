//! Undo and redo IPC commands.
//!
//! Each command checks the pixel history first (drawing ops) and only falls
//! through to the structural `History` (layer/frame/palette ops) when the
//! pixel stack is exhausted.  This mirrors the single linear undo model that
//! drawing apps present to users: the last action — pixel or structural —
//! is the first to undo, regardless of which subsystem owns it.

use tauri::{AppHandle, State};

use crate::commands::canvas::emit_buffer_tiles;
use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

/// Data extracted from a `PixelOpBatch` for buffer restoration and tile emission.
struct PixelUndoData {
    /// `(buffer_id, pixel_bytes)` pairs to write back.
    restores: Vec<(u32, Vec<u8>)>,
    /// `(sprite_id, frame_index, w, h, stride, buffer_id)` for tile emission.
    emit_info: Vec<(u32, u32, u32, u32, u32, u32)>,
}

/// Undo the most recently applied command.
///
/// Pixel drawing ops are undone first (pixel history); structural ops
/// (add layer, rename, etc.) are undone when the pixel stack is empty.
/// Returns [`AppCommandError::NothingToUndo`] when both stacks are at
/// their beginning.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn undo(app: AppHandle, state: State<'_, AppState>) -> CommandResult<()> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;

    if doc.pixel_history.can_undo() {
        // Clone the data we need while holding the batch borrow, then drop it
        // before touching pixel_buffers (split-borrow across different fields
        // requires explicit field paths; going through the method return value
        // ties the lifetime to the whole `doc` reference).
        let data = {
            let batch = doc
                .pixel_history
                .undo()
                .ok_or(AppCommandError::NothingToUndo)?;
            PixelUndoData {
                restores: batch
                    .ops
                    .iter()
                    .map(|op| (op.buffer_id, op.before.clone()))
                    .collect(),
                emit_info: batch
                    .ops
                    .iter()
                    .map(|op| {
                        (
                            op.sprite_id,
                            op.frame_index,
                            op.buf_width,
                            op.buf_height,
                            op.buf_stride,
                            op.buffer_id,
                        )
                    })
                    .collect(),
            }
        }; // batch borrow ends here

        for (buf_id, before_pixels) in &data.restores {
            if let Some(entry) = doc.pixel_buffers.iter_mut().find(|e| e.id == *buf_id) {
                entry.pixels.clone_from(before_pixels);
            }
        }

        for (sprite_id, frame_index, w, h, stride, buf_id) in data.emit_info {
            if let Some(entry) = doc.pixel_buffers.iter().find(|e| e.id == buf_id) {
                let px = entry.pixels.clone();
                emit_buffer_tiles(&app, sprite_id, frame_index, w, h, stride, &px);
            }
        }

        doc.dirty = true;
        return Ok(());
    }

    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    doc.history.undo(project)?;
    doc.dirty = true;
    Ok(())
}

/// Redo the most-recently-undone command.
///
/// Pixel drawing ops are redone first; structural ops follow when the pixel
/// redo branch is exhausted.
/// Returns [`AppCommandError::NothingToRedo`] if no redo branch exists.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn redo(app: AppHandle, state: State<'_, AppState>) -> CommandResult<()> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;

    if doc.pixel_history.can_redo() {
        let data = {
            let batch = doc
                .pixel_history
                .redo()
                .ok_or(AppCommandError::NothingToRedo)?;
            PixelUndoData {
                restores: batch
                    .ops
                    .iter()
                    .map(|op| (op.buffer_id, op.after.clone()))
                    .collect(),
                emit_info: batch
                    .ops
                    .iter()
                    .map(|op| {
                        (
                            op.sprite_id,
                            op.frame_index,
                            op.buf_width,
                            op.buf_height,
                            op.buf_stride,
                            op.buffer_id,
                        )
                    })
                    .collect(),
            }
        };

        for (buf_id, after_pixels) in &data.restores {
            if let Some(entry) = doc.pixel_buffers.iter_mut().find(|e| e.id == *buf_id) {
                entry.pixels.clone_from(after_pixels);
            }
        }

        for (sprite_id, frame_index, w, h, stride, buf_id) in data.emit_info {
            if let Some(entry) = doc.pixel_buffers.iter().find(|e| e.id == buf_id) {
                let px = entry.pixels.clone();
                emit_buffer_tiles(&app, sprite_id, frame_index, w, h, stride, &px);
            }
        }

        doc.dirty = true;
        return Ok(());
    }

    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    doc.history.redo(project)?;
    doc.dirty = true;
    Ok(())
}
