//! Project lifecycle commands: new, open, save, close, sprite CRUD.

use std::path::PathBuf;

use pixhaus_core::project::{ColorMode, Project, ProjectMetadata, Size, Sprite, SpriteId};
use pixhaus_core::undo::History;
use pixhaus_io::pixhaus::PixhausArchive;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{AppCommandError, CommandResult};
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
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_new(name: String, state: State<'_, AppState>) -> CommandResult<ProjectStatus> {
    let mut doc = state.doc.write().await;
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
    // Reset undo so a fresh project doesn't inherit the previous
    // session's history (which would walk into commands written
    // against a different sprite set).
    doc.history = History::new();
    Ok(status)
}

/// Opens a `.pixhaus` project from disk.
///
/// Decodes the file via [`pixhaus_io::pixhaus::decode_from_file`] on a
/// blocking-thread pool (the decoder reads + zstd-decompresses + msgpack-
/// deserialises synchronously and we must not stall the tokio runtime),
/// then atomically swaps the loaded project into the app's document
/// store.
///
/// `next_id` is recomputed from the loaded project so subsequent CRUD
/// minting can't collide with ids that came in from disk. The pixel
/// buffers in the archive are dropped for now; `DocumentStore` doesn't
/// have a buffer cache yet, and the canvas-side composite path will
/// pick them up when that's wired.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_open(
    path: String,
    state: State<'_, AppState>,
) -> CommandResult<ProjectStatus> {
    let path_buf = PathBuf::from(&path);
    let archive = tokio::task::spawn_blocking({
        let path_buf = path_buf.clone();
        move || pixhaus_io::pixhaus::decode_from_file(&path_buf)
    })
    .await
    .map_err(|join_err| AppCommandError::Validation {
        detail: format!("decode task panicked: {join_err}"),
    })??;

    let project = archive.project;
    let next_id = compute_next_id(&project);
    let status = ProjectStatus {
        metadata: project.metadata.clone(),
        path: Some(path),
        dirty: false,
        sprite_count: project.sprites.len(),
    };

    let mut doc = state.doc.write().await;
    doc.project = Some(project);
    doc.path = Some(path_buf);
    doc.dirty = false;
    doc.next_id = next_id;
    // Reset undo so opening a different project doesn't inherit the
    // previous session's command history.
    doc.history = History::new();
    Ok(status)
}

/// Returns one greater than the maximum id seen across every entity in
/// `project`, or `1` if the project is empty. Calling sites that mint
/// fresh ids monotonically (every `*_add` command) need this to avoid
/// reusing an id that came in from disk.
fn compute_next_id(project: &Project) -> u32 {
    let mut max = 0u32;
    for sprite in &project.sprites {
        max = max.max(sprite.id.get());
        for layer in &sprite.layers {
            max = max.max(layer.id.get());
        }
        for palette in &sprite.palettes {
            max = max.max(palette.id.get());
        }
        for tileset in &sprite.tilesets {
            max = max.max(tileset.id.get());
        }
        for slice in &sprite.slices {
            max = max.max(slice.id.get());
        }
        for animation in &sprite.animations {
            max = max.max(animation.id.get());
        }
        for cel in &sprite.cels {
            if let pixhaus_core::project::CelData::Raster { buffer, .. } = &cel.data {
                max = max.max(buffer.get());
            }
        }
    }
    max.saturating_add(1)
}

/// Saves the active project to disk.
///
/// Mirrors [`project_open`]: encodes the in-memory project via
/// [`pixhaus_io::pixhaus::encode_to_file`] on the blocking-thread pool
/// (the encoder zstd-compresses + msgpack-serialises synchronously and
/// must not stall the tokio runtime).
///
/// Path resolution:
/// - `Some(path)` overrides any previous save target (Save As).
/// - `None` uses the document store's last-known path. If both are
///   absent the caller hasn't picked a file yet — return
///   [`AppCommandError::Validation`] so the UI can route to a save-as
///   flow.
///
/// `DocumentStore` has no pixel-buffer cache today, so the encoded
/// archive ships with `buffers: Vec::new()`. Cels carry their
/// `PixelBufferId` references but the bytes round-trip lossily — the
/// pixel-buffer-storage stream (`S15-prep`) picks that up.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_save(path: Option<String>, state: State<'_, AppState>) -> CommandResult<()> {
    let target = {
        let doc = state.doc.read().await;
        if doc.project.is_none() {
            return Err(AppCommandError::NoActiveProject);
        }
        match path {
            Some(p) => PathBuf::from(p),
            None => doc
                .path
                .clone()
                .ok_or_else(|| AppCommandError::Validation {
                    detail: "save requires a path on first call (use save-as)".into(),
                })?,
        }
    };

    // Snapshot the project so we can release the read lock before doing
    // blocking I/O. Cloning the Project is fine — it's the same
    // round-trip the round-trip tests already hit.
    let archive = {
        let doc = state.doc.read().await;
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?
            .clone();
        PixhausArchive::new(project)
    };

    let target_for_blocking = target.clone();
    tokio::task::spawn_blocking(move || {
        pixhaus_io::pixhaus::encode_to_file(&archive, &target_for_blocking)
    })
    .await
    .map_err(|join_err| AppCommandError::Validation {
        detail: format!("encode task panicked: {join_err}"),
    })??;

    let mut doc = state.doc.write().await;
    doc.path = Some(target);
    doc.dirty = false;
    Ok(())
}

/// Closes the active project, discarding all in-memory state.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_close(state: State<'_, AppState>) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    doc.project = None;
    doc.path = None;
    doc.dirty = false;
    Ok(())
}

/// Returns the active project's status, or `None` if no project is open.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_get(state: State<'_, AppState>) -> CommandResult<Option<ProjectStatus>> {
    let doc = state.doc.read().await;
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
#[tauri::command(async, rename_all = "snake_case")]
pub async fn sprite_add(args: SpriteAddArgs, state: State<'_, AppState>) -> CommandResult<Sprite> {
    let mut doc = state.doc.write().await;
    let id = SpriteId::new(doc.next_id);
    doc.next_id += 1;
    let sprite = {
        let project = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?;
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
#[tauri::command(async, rename_all = "snake_case")]
pub async fn sprite_delete(sprite_id: SpriteId, state: State<'_, AppState>) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    {
        let project = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?;
        let before = project.sprites.len();
        project.sprites.retain(|s| s.id != sprite_id);
        if project.sprites.len() == before {
            return Err(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            });
        }
    }
    doc.dirty = true;
    Ok(())
}

/// Returns all sprites in the active project.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn sprite_list(state: State<'_, AppState>) -> CommandResult<Vec<Sprite>> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
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

    #[test]
    fn compute_next_id_empty_project_returns_one() {
        assert_eq!(compute_next_id(&Project::new("empty")), 1);
    }

    /// Round-trip: build a project in memory, encode it to a temp file,
    /// then load it back through `decode_from_file` and verify
    /// `compute_next_id` is past every loaded entity.
    ///
    /// We don't depend on `examples/samples/*.pixhaus` here — that
    /// fixture is owned by the io crate's tests and importing it from
    /// app/ would couple two crates' test setups. Building a sprite
    /// in-line is enough to lock the contract.
    #[test]
    fn compute_next_id_passes_max_loaded_id() {
        use pixhaus_core::project::{
            BlendMode, Layer, LayerId, LayerKind, Palette, PaletteId, Size, Sprite, SpriteId,
            UserData,
        };
        let mut project = Project::new("nextid-fixture");
        let mut sprite = Sprite::empty(SpriteId::new(7), "main", Size::new(8, 8));
        sprite.layers.push(Layer {
            id: LayerId::new(11),
            name: "bg".into(),
            kind: LayerKind::Raster,
            blend_mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            locked: false,
            parent: None,
            user_data: UserData::default(),
        });
        sprite.palettes.push(Palette {
            id: PaletteId::new(42),
            name: "main".into(),
            colors: Vec::new(),
            user_data: UserData::default(),
        });
        project.sprites.push(sprite);

        let next = compute_next_id(&project);
        assert!(
            next > 42,
            "next_id must exceed every loaded id; got {next} for max id 42"
        );
    }
}
