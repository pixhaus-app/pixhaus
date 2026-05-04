//! Undo and redo IPC commands.

use tauri::State;

use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

/// Undo the most recently applied command.
///
/// Returns `NoActiveProject` if no project is open, or `Validation` if
/// there is nothing to undo.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn undo(state: State<'_, AppState>) -> CommandResult<()> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    doc.history
        .undo(project)
        .map_err(|e| AppCommandError::Validation {
            detail: e.to_string(),
        })?;
    doc.dirty = true;
    Ok(())
}

/// Redo the most-recently-undone command.
///
/// Returns `NoActiveProject` if no project is open, or `Validation` if
/// there is nothing to redo.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn redo(state: State<'_, AppState>) -> CommandResult<()> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    doc.history
        .redo(project)
        .map_err(|e| AppCommandError::Validation {
            detail: e.to_string(),
        })?;
    doc.dirty = true;
    Ok(())
}
