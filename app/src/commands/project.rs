//! Project lifecycle commands: new, open, save, close, sprite CRUD.

use pixhaus_core::project::{ColorMode, Project, ProjectMetadata, Size, Sprite, SpriteId};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

/// Status snapshot returned by project-level commands.
#[derive(Debug, Serialize)]
pub struct ProjectStatus {
    /// Core project metadata (name, timestamps, editor version).
    pub metadata: ProjectMetadata,
    /// Filesystem path if the project has been saved at least once.
    pub path: Option<String>,
    /// `true` when in-memory state differs from the last save.
    pub dirty: bool,
    /// Number of sprites in the project.
    pub sprite_count: usize,
}

/// Arguments for adding a new sprite to the active project.
#[derive(Debug, Deserialize)]
pub struct SpriteAddArgs {
    /// Display name for the sprite.
    pub name: String,
    /// Canvas width in pixels.
    pub canvas_width: u32,
    /// Canvas height in pixels.
    pub canvas_height: u32,
    /// Authoring color mode.
    pub color_mode: ColorMode,
}

/// Creates a new empty project, replacing any currently open document.
#[tauri::command(async)]
pub async fn project_new(
    name: String,
    state: State<'_, AppState>,
) -> Result<ProjectStatus, String> {
    let mut doc = state.doc.lock().await;
    let project = Project::new(name);
    let status = ProjectStatus {
        metadata: project.metadata.clone(),
        path: None,
        dirty: true,
        sprite_count: 0,
    };
    doc.project = Some(project);
    doc.path = None;
    doc.dirty = true;
    doc.next_id = 1;
    Ok(status)
}

/// Opens a project from disk.
///
/// Requires B3 (`.pixhaus` file format). Returns an error until B3 lands.
#[tauri::command(async)]
pub async fn project_open(
    _path: String,
    _state: State<'_, AppState>,
) -> Result<ProjectStatus, String> {
    Err("not yet implemented: project_open requires B3 (.pixhaus format)".to_string())
}

/// Saves the active project to disk.
///
/// Requires B3 (`.pixhaus` file format). Returns an error until B3 lands.
#[tauri::command(async)]
pub async fn project_save(
    _path: Option<String>,
    _state: State<'_, AppState>,
) -> Result<(), String> {
    Err("not yet implemented: project_save requires B3 (.pixhaus format)".to_string())
}

/// Closes the active project, discarding all in-memory state.
#[tauri::command(async)]
pub async fn project_close(state: State<'_, AppState>) -> Result<(), String> {
    let mut doc = state.doc.lock().await;
    doc.project = None;
    doc.path = None;
    doc.dirty = false;
    Ok(())
}

/// Returns the active project's status, or `None` if no project is open.
#[tauri::command(async)]
pub async fn project_get(state: State<'_, AppState>) -> Result<Option<ProjectStatus>, String> {
    let doc = state.doc.lock().await;
    let Some(project) = &doc.project else {
        return Ok(None);
    };
    Ok(Some(ProjectStatus {
        metadata: project.metadata.clone(),
        path: doc
            .path
            .as_ref()
            .and_then(|p| p.to_str().map(str::to_owned)),
        dirty: doc.dirty,
        sprite_count: project.sprites.len(),
    }))
}

/// Adds a new empty sprite to the active project.
#[tauri::command(async)]
pub async fn sprite_add(args: SpriteAddArgs, state: State<'_, AppState>) -> Result<Sprite, String> {
    let mut doc = state.doc.lock().await;
    let id = SpriteId::new(doc.next_id);
    doc.next_id += 1;
    let sprite = {
        let project = doc.project.as_mut().ok_or("no active project")?;
        let mut sprite = Sprite::empty(
            id,
            args.name,
            Size::new(args.canvas_width, args.canvas_height),
        );
        sprite.color_mode = args.color_mode;
        project.sprites.push(sprite.clone());
        sprite
    };
    doc.dirty = true;
    Ok(sprite)
}

/// Removes a sprite from the active project by ID.
#[tauri::command(async)]
pub async fn sprite_delete(sprite_id: SpriteId, state: State<'_, AppState>) -> Result<(), String> {
    let mut doc = state.doc.lock().await;
    {
        let project = doc.project.as_mut().ok_or("no active project")?;
        let before = project.sprites.len();
        project.sprites.retain(|s| s.id != sprite_id);
        if project.sprites.len() == before {
            return Err(format!("sprite {} not found", sprite_id.get()));
        }
    }
    doc.dirty = true;
    Ok(())
}

/// Returns all sprites in the active project.
#[tauri::command(async)]
pub async fn sprite_list(state: State<'_, AppState>) -> Result<Vec<Sprite>, String> {
    let doc = state.doc.lock().await;
    let project = doc.project.as_ref().ok_or("no active project")?;
    Ok(project.sprites.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_status_sprite_count_zero_on_new() {
        let project = Project::new("test");
        let status = ProjectStatus {
            metadata: project.metadata.clone(),
            path: None,
            dirty: true,
            sprite_count: project.sprites.len(),
        };
        assert_eq!(status.sprite_count, 0);
        assert!(status.dirty);
    }
}
