//! Project lifecycle commands: new, open, save, close, sprite CRUD.

use std::path::PathBuf;

use pixhaus_core::project::{ColorMode, Project, ProjectMetadata, Size, Sprite, SpriteId};
use pixhaus_core::undo::History;
use pixhaus_io::pixhaus::{PixelBufferEntry, PixhausArchive};
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::task::JoinError;

use crate::error::{AppCommandError, CommandResult};
use crate::state::{AppState, DocumentStore};

/// Maps a `tokio::task::JoinError` from a `spawn_blocking` task into a
/// human-readable validation error. Distinguishes panic from
/// runtime-initiated cancellation so the UI / log surface reflects what
/// actually happened (Copilot review of PR #50).
fn describe_join_error(op: &str, err: &JoinError) -> AppCommandError {
    let detail = if err.is_panic() {
        format!("{op} task panicked")
    } else if err.is_cancelled() {
        format!("{op} task cancelled (runtime shutdown)")
    } else {
        format!("{op} task did not complete: {err}")
    };
    AppCommandError::Validation { detail }
}

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
    Ok(install_new_project(&mut doc, Project::new(name)))
}

/// Resets the document store to host a brand-new project. Lifted out of
/// `project_new` so the history-reset invariant is testable without a
/// tauri `State`.
fn install_new_project(doc: &mut DocumentStore, project: Project) -> ProjectStatus {
    let status = ProjectStatus {
        metadata: project.metadata.clone(),
        path: None,
        dirty: true,
        sprite_count: project.sprites.len(),
    };
    doc.project = Some(project);
    doc.path = None;
    doc.dirty = true;
    doc.next_id = 1;
    // Reset undo so a fresh project doesn't inherit the previous
    // session's history (which would walk into commands written
    // against a different sprite set).
    doc.history = History::new();
    doc.pixel_buffers = Vec::new();
    status
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
/// minting can't collide with ids that came in from disk. Pixel buffers
/// from the archive are retained in `DocumentStore::pixel_buffers` so
/// `project_save` can write them back without losing content.
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
    .map_err(|join_err| describe_join_error("pixhaus decode", &join_err))??;

    let mut doc = state.doc.write().await;
    Ok(install_loaded_project(
        &mut doc,
        archive.project,
        path_buf,
        archive.buffers,
    ))
}

/// Swaps a freshly-decoded project and its pixel buffers into the document
/// store and resets per-document state (path, dirty flag, id counter, undo
/// history). Lifted out of `project_open` so the history-reset and
/// id-recompute invariants are testable without a tauri `State`.
fn install_loaded_project(
    doc: &mut DocumentStore,
    project: Project,
    path: PathBuf,
    buffers: Vec<PixelBufferEntry>,
) -> ProjectStatus {
    let next_id = compute_next_id(&project);
    let status = ProjectStatus {
        metadata: project.metadata.clone(),
        path: Some(path.to_string_lossy().into_owned()),
        dirty: false,
        sprite_count: project.sprites.len(),
    };
    doc.project = Some(project);
    doc.path = Some(path);
    doc.dirty = false;
    doc.next_id = next_id;
    // Reset undo so opening a different project doesn't inherit the
    // previous session's command history.
    doc.history = History::new();
    doc.pixel_buffers = buffers;
    status
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

/// Imports a `.psd` file and makes it the active project.
///
/// Decodes via [`pixhaus_io::psd::decode_from_file`] on a blocking-thread
/// pool, then replaces the active project in the same way `project_open`
/// does. The imported project has no associated filesystem path (it has not
/// been saved as `.pixhaus` yet), so `dirty` is `true` on return.
///
/// Non-fatal conversion warnings are logged at the `warn` level; callers do
/// not receive them through this command because the `ProjectStatus` type
/// has no warnings field. A future command revision can add that field when
/// the UI has a surface to display warnings.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_import_psd(
    path: String,
    state: State<'_, AppState>,
) -> CommandResult<ProjectStatus> {
    let path_buf = PathBuf::from(&path);
    let converted = tokio::task::spawn_blocking({
        let path_buf = path_buf.clone();
        move || pixhaus_io::psd::decode_from_file(&path_buf)
    })
    .await
    .map_err(|join_err| describe_join_error("PSD decode", &join_err))??;

    for w in &converted.warnings {
        tracing::warn!(path = %path_buf.display(), warning = ?w, "PSD import warning");
    }

    let PixhausArchive { project, buffers } = converted.archive;
    let next_id = compute_next_id(&project);
    let status = ProjectStatus {
        metadata: project.metadata.clone(),
        path: None,
        dirty: true,
        sprite_count: project.sprites.len(),
    };

    let mut doc = state.doc.write().await;
    doc.project = Some(project);
    doc.path = None;
    doc.dirty = true;
    doc.next_id = next_id;
    doc.pixel_buffers = buffers;
    Ok(status)
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
/// Pixel buffers from `DocumentStore::pixel_buffers` are included in the
/// encoded archive so cels round-trip with their pixel data intact.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_save(path: Option<String>, state: State<'_, AppState>) -> CommandResult<()> {
    // Hold the write guard for the entire save so a concurrent
    // project_open / project_new can't mutate doc.project between our
    // archive snapshot and the dirty/path bookkeeping at the end. The
    // editor is single-user so the contention is theoretical, but the
    // write-guard-for-duration shape makes the invariant local: the
    // bytes on disk match the dirty=false state we record.
    let mut doc = state.doc.write().await;

    let target = match path {
        Some(p) => PathBuf::from(p),
        None => doc
            .path
            .clone()
            .ok_or_else(|| AppCommandError::Validation {
                detail: "save requires a path on first call (use save-as)".into(),
            })?,
    };

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    // Stamp metadata and snapshot both the project and the pixel buffers
    // before crossing the spawn_blocking boundary. The mutable borrow of
    // `project` ends after the clone so `doc.pixel_buffers` can be
    // borrowed immutably on the next line.
    let project_snap = {
        let project = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?;
        stamp_save_metadata(project, now_secs);
        project.clone()
    };
    let archive = PixhausArchive {
        project: project_snap,
        buffers: doc.pixel_buffers.clone(),
    };
    let target_for_blocking = target.clone();
    let encode_result = tokio::task::spawn_blocking(move || {
        pixhaus_io::pixhaus::encode_to_file(&archive, &target_for_blocking)
    })
    .await
    .map_err(|join_err| {
        let detail = if join_err.is_panic() {
            "encode task panicked".into()
        } else if join_err.is_cancelled() {
            "encode task cancelled (runtime shutdown)".into()
        } else {
            format!("encode task did not complete: {join_err}")
        };
        AppCommandError::Validation { detail }
    })?;
    encode_result?;

    doc.path = Some(target);
    doc.dirty = false;
    Ok(())
}

/// Stamps `updated_at` + `editor_version` on every save and lazily
/// initialises `created_at` if the project hasn't been saved before.
/// `now_secs` is taken as a parameter so tests can pin a deterministic
/// timestamp instead of reaching for the wall clock.
fn stamp_save_metadata(project: &mut Project, now_secs: i64) {
    if project.metadata.created_at == 0 {
        project.metadata.created_at = now_secs;
    }
    project.metadata.updated_at = now_secs;
    project.metadata.editor_version = env!("CARGO_PKG_VERSION").into();
}

/// Closes the active project, discarding all in-memory state.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_close(state: State<'_, AppState>) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    doc.project = None;
    doc.path = None;
    doc.dirty = false;
    doc.pixel_buffers = Vec::new();
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
    use pixhaus_core::undo::{Command, CommandResult as UndoCommandResult};
    use pixhaus_io::pixhaus::PixelBufferEntry;

    /// No-op `Command` used by the history-reset regression tests. It
    /// pushes nothing onto the project; the history just needs *a* node
    /// so we can observe that `install_*_project` clears it.
    struct NoOpCommand;
    impl Command for NoOpCommand {
        fn label(&self) -> &'static str {
            "noop"
        }
        fn apply(&mut self, _project: &mut Project) -> UndoCommandResult {
            Ok(())
        }
        fn undo(&mut self, _project: &mut Project) -> UndoCommandResult {
            Ok(())
        }
    }

    fn doc_with_history_entry() -> DocumentStore {
        let mut doc = DocumentStore::default();
        let mut project = Project::new("seed");
        doc.history
            .push(Box::new(NoOpCommand), &mut project)
            .expect("seed history push");
        doc.project = Some(project);
        assert!(
            doc.history.node_count() > 0,
            "test setup: history should be non-empty before the reset"
        );
        doc
    }

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

    // ── history-reset regression tests ────────────────────────────────────
    //
    // Both project_new and project_open must clear the undo history so
    // commands recorded against a previous project can't replay against
    // an unrelated set of sprites. These cover the helpers the tauri
    // commands now delegate to.

    #[test]
    fn install_new_project_clears_history() {
        let mut doc = doc_with_history_entry();
        let _ = install_new_project(&mut doc, Project::new("fresh"));
        assert_eq!(
            doc.history.node_count(),
            0,
            "project_new must drop the previous session's undo history"
        );
        assert_eq!(doc.next_id, 1);
        assert!(doc.path.is_none());
        assert!(doc.dirty);
    }

    #[test]
    fn install_loaded_project_clears_history() {
        let mut doc = doc_with_history_entry();
        let _ = install_loaded_project(
            &mut doc,
            Project::new("loaded"),
            PathBuf::from("/tmp/x.pixhaus"),
            Vec::new(),
        );
        assert_eq!(
            doc.history.node_count(),
            0,
            "project_open must drop the previous session's undo history"
        );
        assert!(!doc.dirty, "loaded project starts clean");
        assert_eq!(
            doc.path.as_deref(),
            Some(std::path::Path::new("/tmp/x.pixhaus"))
        );
    }

    #[test]
    fn install_loaded_project_stores_buffers() {
        let entry = PixelBufferEntry {
            id: 1,
            width: 4,
            height: 4,
            stride: 16,
            pixels: vec![0u8; 64],
        };
        let mut doc = DocumentStore::default();
        let _ = install_loaded_project(
            &mut doc,
            Project::new("buf-test"),
            PathBuf::from("/tmp/buf.pixhaus"),
            vec![entry.clone()],
        );
        assert_eq!(doc.pixel_buffers.len(), 1);
        assert_eq!(doc.pixel_buffers[0].id, entry.id);
        assert_eq!(doc.pixel_buffers[0].pixels.len(), entry.pixels.len());
    }

    #[test]
    fn install_new_project_clears_buffers() {
        let mut doc = DocumentStore {
            pixel_buffers: vec![PixelBufferEntry {
                id: 5,
                width: 2,
                height: 2,
                stride: 8,
                pixels: vec![0u8; 16],
            }],
            ..DocumentStore::default()
        };
        let _ = install_new_project(&mut doc, Project::new("fresh"));
        assert!(
            doc.pixel_buffers.is_empty(),
            "project_new must discard the previous session's pixel buffers"
        );
    }

    // ── stamp_save_metadata ───────────────────────────────────────────────

    #[test]
    fn stamp_save_metadata_initialises_created_at_when_zero() {
        let mut project = Project::new("fresh");
        project.metadata.created_at = 0;
        stamp_save_metadata(&mut project, 1_700_000_000);
        assert_eq!(project.metadata.created_at, 1_700_000_000);
        assert_eq!(project.metadata.updated_at, 1_700_000_000);
        assert_eq!(project.metadata.editor_version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn stamp_save_metadata_preserves_existing_created_at() {
        let mut project = Project::new("existing");
        project.metadata.created_at = 1_000_000_000;
        stamp_save_metadata(&mut project, 1_700_000_000);
        assert_eq!(
            project.metadata.created_at, 1_000_000_000,
            "created_at must not be overwritten on subsequent saves"
        );
        assert_eq!(project.metadata.updated_at, 1_700_000_000);
    }

    // ── filesystem round-trip ─────────────────────────────────────────────
    //
    // Goes through the io crate (encode_to_file / decode_from_file) but
    // bypasses the tauri command itself — `State<'_, AppState>` is not
    // constructible from a unit test. The point of this test is to lock
    // the metadata-stamping contract end-to-end: stamp + encode + decode
    // returns a project whose updated_at and editor_version survived the
    // archive round-trip.

    #[test]
    fn metadata_round_trips_through_pixhaus_archive() {
        use pixhaus_io::pixhaus::{decode_from_file, encode_to_file};

        let mut project = Project::new("round-trip");
        project.metadata.created_at = 0;
        stamp_save_metadata(&mut project, 1_700_000_000);

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "pixhaus-app-save-roundtrip-{}-{nanos}.pixhaus",
            std::process::id(),
        ));

        let archive = PixhausArchive::new(project);
        encode_to_file(&archive, &path).expect("encode");
        let loaded = decode_from_file(&path).expect("decode");
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.project.metadata.created_at, 1_700_000_000);
        assert_eq!(loaded.project.metadata.updated_at, 1_700_000_000);
        assert_eq!(
            loaded.project.metadata.editor_version,
            env!("CARGO_PKG_VERSION")
        );
    }

    /// Pixel buffers stored in `DocumentStore::pixel_buffers` survive a
    /// save/load cycle. Encodes an archive that includes a synthetic buffer
    /// entry, then decodes it and checks the bytes match.
    #[test]
    fn pixel_buffers_round_trip_through_pixhaus_archive() {
        use pixhaus_io::pixhaus::{decode_from_file, encode_to_file};

        let project = Project::new("buf-round-trip");
        let pixels: Vec<u8> = (0u8..=255).cycle().take(64).collect();
        let buffers = vec![PixelBufferEntry {
            id: 3,
            width: 4,
            height: 4,
            stride: 16,
            pixels: pixels.clone(),
        }];

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "pixhaus-app-buf-roundtrip-{}-{nanos}.pixhaus",
            std::process::id(),
        ));

        let archive = PixhausArchive { project, buffers };
        encode_to_file(&archive, &path).expect("encode");
        let loaded = decode_from_file(&path).expect("decode");
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            loaded.buffers.len(),
            1,
            "buffer count must survive the round-trip"
        );
        assert_eq!(loaded.buffers[0].id, 3);
        assert_eq!(loaded.buffers[0].width, 4);
        assert_eq!(loaded.buffers[0].height, 4);
        assert_eq!(loaded.buffers[0].pixels, pixels);
    }
}
