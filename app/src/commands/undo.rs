//! Undo and redo IPC commands.

use pixhaus_core::undo::Error as UndoError;
use tauri::State;

use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

/// Maps `core::undo::Error` to the typed `AppCommandError` variants the
/// IPC contract exposes.
///
/// `NothingToUndo` / `NothingToRedo` get their own variants so the UI
/// can disable menu items by switching on `kind`. `CommandFailed` and
/// `Poisoned` collapse to `HistoryCorrupted` — both indicate the
/// project state may no longer be trustworthy and the user should
/// reload. `HistoryFull` is a catastrophic but well-defined failure
/// surfaced as `Validation` (it should never fire in practice).
fn map_undo_error(err: UndoError) -> AppCommandError {
    match err {
        UndoError::NothingToUndo => AppCommandError::NothingToUndo,
        UndoError::NothingToRedo => AppCommandError::NothingToRedo,
        UndoError::CommandFailed { .. } | UndoError::Poisoned { .. } => {
            AppCommandError::HistoryCorrupted {
                detail: err.to_string(),
            }
        }
        other => AppCommandError::Validation {
            detail: other.to_string(),
        },
    }
}

/// Undo the most recently applied command.
///
/// Returns [`AppCommandError::NoActiveProject`] if no project is open,
/// [`AppCommandError::NothingToUndo`] at the history root, or
/// [`AppCommandError::HistoryCorrupted`] if a prior command failed and
/// the project state is no longer trustworthy.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn undo(state: State<'_, AppState>) -> CommandResult<()> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    doc.history.undo(project).map_err(map_undo_error)?;
    doc.dirty = true;
    Ok(())
}

/// Redo the most-recently-undone command.
///
/// Returns [`AppCommandError::NoActiveProject`] if no project is open,
/// [`AppCommandError::NothingToRedo`] if no redo branch exists, or
/// [`AppCommandError::HistoryCorrupted`] if a prior command failed.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn redo(state: State<'_, AppState>) -> CommandResult<()> {
    let mut lock = state.doc.write().await;
    let doc = &mut *lock;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    doc.history.redo(project).map_err(map_undo_error)?;
    doc.dirty = true;
    Ok(())
}
