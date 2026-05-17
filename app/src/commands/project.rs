//! Project lifecycle commands: new, open, save, close, sprite CRUD.

use std::path::PathBuf;

use pixhaus_core::project::{
    ActiveTarget, AiMetadata, ColorMode, Entity, EntityContent, EntityDefaults, EntityId,
    EntityKind, NamedSprite, Project, ProjectMetadata, ReferenceSheet, SheetVariantId, Size,
    Sprite, SpriteId, StateId, UserData,
};
use pixhaus_core::undo::History;
use pixhaus_io::pixhaus::{PixelBufferEntry, PixhausArchive};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{AppCommandError, CommandResult, join_error_to_command_error};
use crate::state::{AppState, DocumentStore};

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
    /// Optional image bytes for the sprite's canonical reference sheet.
    pub reference_bytes: Option<Vec<u8>>,
    /// MIME type for `reference_bytes`. Defaults to `image/png` when
    /// bytes are provided and this is absent.
    pub reference_mime: Option<String>,
}

/// Creates a new empty project, replacing any currently open document.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_new(name: String, state: State<'_, AppState>) -> CommandResult<ProjectStatus> {
    let mut doc = state.doc.write().await;
    let status = install_new_project(&mut doc, Project::new(name));
    drop(doc);
    state.anchor_cache.clear();
    Ok(status)
}

/// Resets the document store to host a brand-new project. Lifted out of
/// `project_new` so the history-reset invariant is testable without a
/// tauri `State`.
fn install_new_project(doc: &mut DocumentStore, project: Project) -> ProjectStatus {
    let status = ProjectStatus {
        metadata: project.metadata.clone(),
        path: None,
        dirty: true,
        sprite_count: project.sprites_iter().count(),
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
    .map_err(|join_err| join_error_to_command_error("pixhaus decode", &join_err))??;

    let mut doc = state.doc.write().await;
    let status = install_loaded_project(&mut doc, archive.project, path_buf, archive.buffers);
    drop(doc);
    state.anchor_cache.clear();
    Ok(status)
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
        sprite_count: project.sprites_iter().count(),
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
///
/// All ID types (`EntityId`, `StateId`, `SpriteId`, `LayerId`, `GroupId`,
/// `TagId`, `PaletteId`, `TilesetId`, `SliceId`, `AnimationId`,
/// `PixelBufferId`, `SheetVariantId`) draw from the same monotonic counter.
/// This function must scan every type so a save/load cycle never hands out
/// a raw value that is already in use by any ID type, regardless of which
/// type holds it.
fn compute_next_id(project: &Project) -> u32 {
    use pixhaus_core::project::{
        CelData, EntityContent, LayerKind, SelectionRegion, TilesetSource,
    };
    let mut max = 0u32;

    // Walk every library entity and its content.
    for entity in &project.library.entities {
        max = max.max(entity.id.get()); // EntityId
        match &entity.content {
            EntityContent::Sprites {
                states,
                reference_sheet,
            } => {
                for state in states {
                    max = max.max(state.id.get()); // StateId
                    let sprite = &state.sprite;
                    max = max.max(sprite.id.get()); // SpriteId
                    for layer in &sprite.layers {
                        max = max.max(layer.id.get()); // LayerId
                        if let LayerKind::Reference { image, .. } = &layer.kind {
                            max = max.max(image.get()); // PixelBufferId (reference layer)
                        }
                    }
                    for palette in &sprite.palettes {
                        max = max.max(palette.id.get());
                    }
                    for tileset in &sprite.tilesets {
                        max = max.max(tileset.id.get());
                        if let TilesetSource::Inline { buffer } = &tileset.source {
                            max = max.max(buffer.get());
                        }
                    }
                    for slice in &sprite.slices {
                        max = max.max(slice.id.get());
                    }
                    for animation in &sprite.animations {
                        max = max.max(animation.id.get());
                    }
                    for cel in &sprite.cels {
                        if let CelData::Raster { buffer, .. } = &cel.data {
                            max = max.max(buffer.get());
                        }
                    }
                }
                if let Some(sheet) = reference_sheet {
                    if let Some(canonical) = &sheet.canonical {
                        max = max.max(canonical.id.get()); // SheetVariantId
                    }
                    for variant in &sheet.variants {
                        max = max.max(variant.id.get()); // SheetVariantId
                    }
                }
            }
            EntityContent::Tileset { tileset } => {
                max = max.max(tileset.id.get()); // TilesetId
                if let TilesetSource::Inline { buffer } = &tileset.source {
                    max = max.max(buffer.get()); // PixelBufferId
                }
            }
            EntityContent::Tilemap { scene } => {
                for layer in &scene.layers {
                    max = max.max(layer.id.get()); // LayerId
                }
                // scene.tilesets[].tileset_entity_id is EntityId already covered
                // by the outer entity loop — no rescan needed.
            }
        }
    }

    // Project-wide groups and tags also draw from the same counter.
    for group in &project.library.groups {
        max = max.max(group.id.get()); // GroupId
    }
    for tag in &project.library.tags {
        max = max.max(tag.id.get()); // TagId
    }
    // Project-wide shared palettes.
    for palette in &project.library.palettes {
        max = max.max(palette.id.get());
    }

    // Selection masks live in `pixel_buffers` too. Without this branch,
    // `next_id` after a save/load could collide with a live mask buffer
    // and the next allocation would overwrite the selection.
    if let Some(SelectionRegion::Mask { mask, .. }) = &project.selection.region {
        max = max.max(mask.get());
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
    .map_err(|join_err| join_error_to_command_error("PSD decode", &join_err))??;

    for w in &converted.warnings {
        tracing::warn!(path = %path_buf.display(), warning = ?w, "PSD import warning");
    }

    let mut doc = state.doc.write().await;
    Ok(install_imported_project(&mut doc, converted.archive))
}

/// Imports an `.aseprite` / `.ase` file and makes it the active project.
///
/// Decodes the file via [`pixhaus_io::aseprite::decode_from_file`] on the
/// blocking-thread pool, then translates the document into a
/// [`PixhausArchive`] via [`pixhaus_io::aseprite::document_to_archive`].
/// The imported project has no associated filesystem path (the user must
/// follow up with Save As to write a `.pixhaus` file), so `dirty` is `true`
/// on return.
///
/// The sprite name passed to the converter is derived from the file stem;
/// callers that want a different display name can rename the sprite after
/// the import returns.
///
/// Non-fatal conversion warnings are logged at the `warn` level. The
/// `ProjectStatus` shape has no warnings field today; a follow-up cut can
/// surface them once the UI has a place to display them.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_import_aseprite(
    path: String,
    state: State<'_, AppState>,
) -> CommandResult<ProjectStatus> {
    let path_buf = PathBuf::from(&path);
    let sprite_name = path_buf
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imported")
        .to_owned();

    let converted = tokio::task::spawn_blocking({
        let path_buf = path_buf.clone();
        move || -> Result<pixhaus_io::aseprite::ConvertedArchive, pixhaus_io::Error> {
            let document = pixhaus_io::aseprite::decode_from_file(&path_buf)?;
            pixhaus_io::aseprite::document_to_archive(&document, sprite_name)
        }
    })
    .await
    .map_err(|join_err| join_error_to_command_error("aseprite decode", &join_err))??;

    for w in &converted.warnings {
        tracing::warn!(path = %path_buf.display(), warning = ?w, "Aseprite import warning");
    }

    let mut doc = state.doc.write().await;
    Ok(install_imported_project(&mut doc, converted.archive))
}

/// Swaps an imported project into the document store. Like
/// `install_loaded_project` but without a path (imports require a
/// follow-up save-as) and with `dirty = true` so the user is prompted to
/// save before quitting. Resets the undo history for the same reason
/// `install_new_project` and `install_loaded_project` do — commands
/// recorded against a previous document can't replay against an
/// imported one.
fn install_imported_project(doc: &mut DocumentStore, archive: PixhausArchive) -> ProjectStatus {
    let PixhausArchive { project, buffers } = archive;
    let next_id = compute_next_id(&project);
    let status = ProjectStatus {
        metadata: project.metadata.clone(),
        path: None,
        dirty: true,
        sprite_count: project.sprites_iter().count(),
    };
    doc.project = Some(project);
    doc.path = None;
    doc.dirty = true;
    doc.next_id = next_id;
    doc.history = History::new();
    doc.pixel_buffers = buffers;
    status
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
    // Stamp metadata and snapshot the project before crossing the
    // spawn_blocking boundary. Project::clone is unavoidable here — the
    // encoder needs an owned value on the blocking thread and we can't
    // hand it the lock guard.
    let project_snap = {
        let project = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?;
        stamp_save_metadata(project, now_secs);
        project.clone()
    };
    // Move the pixel buffers out of the document store rather than
    // cloning. They're potentially large (one entry per cel × the cel's
    // pixel byte count), so a clone roughly doubles peak memory during
    // save. The closure hands them back via the return tuple regardless
    // of encode success so we can restore them on doc.pixel_buffers
    // before propagating any encode error. Caveat: if the blocking task
    // panics or is cancelled, the buffers are dropped with the closure;
    // doc.pixel_buffers stays empty in that degenerate case (panics
    // here are not a normal control flow path).
    let taken_buffers = std::mem::take(&mut doc.pixel_buffers);
    let archive = PixhausArchive {
        project: project_snap,
        buffers: taken_buffers,
    };
    let target_for_blocking = target.clone();
    let (encode_result, returned_buffers) = tokio::task::spawn_blocking(move || {
        let result = pixhaus_io::pixhaus::encode_to_file(&archive, &target_for_blocking);
        (result, archive.buffers)
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
    // Restore the buffers before propagating any encode error so a
    // failed save leaves the document store in the same shape it had
    // pre-save (just with the metadata stamp). Without this, a disk-
    // full or permission-denied error would silently drop the user's
    // pixel data on the way back to the UI.
    doc.pixel_buffers = returned_buffers;
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
    drop(doc);
    state.anchor_cache.clear();
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
        sprite_count: project.sprites_iter().count(),
    }))
}

/// Adds a new empty sprite to the active project.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn sprite_add(
    mut args: SpriteAddArgs,
    state: State<'_, AppState>,
) -> CommandResult<Sprite> {
    let mut doc = state.doc.write().await;
    if args
        .reference_bytes
        .as_ref()
        .is_some_and(std::vec::Vec::is_empty)
    {
        return Err(AppCommandError::Validation {
            detail: "reference_bytes must be non-empty when provided".into(),
        });
    }
    let id = SpriteId::new(doc.next_id);
    doc.next_id += 1;
    let entity_id_raw = doc.next_id;
    doc.next_id += 1;
    let state_id_raw = doc.next_id;
    doc.next_id += 1;
    let reference_sheet = args.reference_bytes.take().map(|bytes| {
        let mime = args
            .reference_mime
            .take()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "image/png".into());
        let variant_id = SheetVariantId::new(doc.next_id);
        doc.next_id += 1;
        Box::new(crate::commands::library::reference_sheet_from_image(
            bytes, mime, variant_id, 0,
        ))
    });
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
        install_sprite_as_new_entity(
            project,
            sprite.clone(),
            entity_id_raw,
            state_id_raw,
            reference_sheet,
        );
        sprite
    };
    doc.dirty = true;
    Ok(sprite)
}

/// Wraps a sprite into the project library as a fresh `Custom`-kind
/// entity with one primary state. Marks the new state active so
/// `project.active` points at it after the call.
fn install_sprite_as_new_entity(
    project: &mut Project,
    sprite: Sprite,
    entity_id_raw: u32,
    state_id_raw: u32,
    reference_sheet: Option<Box<ReferenceSheet>>,
) -> EntityId {
    let entity_id = EntityId::new(entity_id_raw);
    let state_id = StateId::new(state_id_raw);
    let name = sprite.name.clone();
    project.library.entities.push(Entity {
        id: entity_id,
        kind: EntityKind::Custom("Sprite".into()),
        name,
        group_id: None,
        tags: Vec::new(),
        defaults: EntityDefaults::default(),
        content: EntityContent::Sprites {
            states: vec![NamedSprite {
                id: state_id,
                state_name: "primary".into(),
                sprite,
                engine_tags: Vec::new(),
            }],
            reference_sheet,
        },
        ai: AiMetadata::default(),
        user_data: UserData::default(),
        created_at: 0,
        updated_at: 0,
    });
    project.active = ActiveTarget::State {
        entity_id,
        state_id,
    };
    entity_id
}

/// Removes a sprite from the active project by ID.
///
/// The legacy data model held one sprite per entity, so removing a
/// sprite from the library means removing the entity that owns the
/// matching state.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn sprite_delete(sprite_id: SpriteId, state: State<'_, AppState>) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    {
        let project = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?;
        let before = project.library.entities.len();
        project.library.entities.retain(|e| match &e.content {
            EntityContent::Sprites { states, .. } => {
                !states.iter().any(|s| s.sprite.id == sprite_id)
            }
            _ => true,
        });
        if project.library.entities.len() == before {
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
    Ok(project
        .sprites_iter()
        .map(|(named, _)| named.sprite.clone())
        .collect())
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
            sprite_count: project.sprites_iter().count(),
        };
        assert_eq!(status.sprite_count, 0);
        assert!(status.dirty);
    }

    #[test]
    fn compute_next_id_empty_project_returns_one() {
        assert_eq!(compute_next_id(&Project::new("empty")), 1);
    }

    #[test]
    fn install_sprite_as_new_entity_leaves_reference_sheet_empty_when_absent() {
        let mut project = Project::new("sprite-without-sheet");
        let sprite = Sprite::empty(SpriteId::new(1), "main", Size::new(16, 16));

        install_sprite_as_new_entity(&mut project, sprite, 10, 11, None);

        let EntityContent::Sprites {
            reference_sheet, ..
        } = &project.library.entities[0].content
        else {
            panic!("expected sprite entity content");
        };
        assert!(reference_sheet.is_none());
    }

    #[test]
    fn install_sprite_as_new_entity_embeds_reference_sheet_when_provided() {
        let mut project = Project::new("sprite-with-sheet");
        let sprite = Sprite::empty(SpriteId::new(1), "main", Size::new(16, 16));
        let sheet = crate::commands::library::reference_sheet_from_image(
            vec![1, 2, 3],
            "image/png".into(),
            SheetVariantId::new(12),
            123,
        );

        install_sprite_as_new_entity(&mut project, sprite, 10, 11, Some(Box::new(sheet)));

        let EntityContent::Sprites {
            reference_sheet, ..
        } = &project.library.entities[0].content
        else {
            panic!("expected sprite entity content");
        };
        let sheet = reference_sheet.as_ref().expect("reference sheet");
        let canonical = sheet.canonical.as_ref().expect("canonical");
        assert_eq!(canonical.id, SheetVariantId::new(12));
        assert_eq!(canonical.image.bytes, vec![1, 2, 3]);
        assert_eq!(canonical.created_at, 123);
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
        install_sprite_as_new_entity(&mut project, sprite, 1_000, 1_001, None);

        let next = compute_next_id(&project);
        assert!(
            next > 42,
            "next_id must exceed every loaded id; got {next} for max id 42"
        );
    }

    /// Selection mask buffers also live in `pixel_buffers` — if
    /// `compute_next_id` skips them, reopening a saved project leaks the
    /// next allocation onto the live mask.
    #[test]
    fn compute_next_id_passes_selection_mask_id() {
        use pixhaus_core::project::{PixelBufferId, Rect, SelectionRegion, Size, Sprite, SpriteId};
        let mut project = Project::new("selection-fixture");
        let sprite = Sprite::empty(SpriteId::new(2), "main", Size::new(8, 8));
        install_sprite_as_new_entity(&mut project, sprite, 1_000, 1_001, None);
        project.selection.region = Some(SelectionRegion::Mask {
            bounds: Rect::from_xywh(0, 0, 8, 8),
            mask: PixelBufferId::new(99),
        });

        let next = compute_next_id(&project);
        assert!(
            next > 99,
            "next_id must exceed selection mask id; got {next} for mask id 99"
        );
    }

    /// Pins the B9.2 fix: entity and state IDs are minted from the same
    /// counter as sprite IDs, so `compute_next_id` must scan them too.
    /// Without the fix, a project whose `entity_id` or `state_id` exceeded the
    /// max sprite/layer/palette id would hand out a colliding id on the
    /// next save+reload cycle.
    #[test]
    fn compute_next_id_includes_entity_and_state_ids() {
        let mut project = Project::new("entity-id-fixture");
        // entity_id_raw=100 and state_id_raw=200 both exceed the sprite id
        // and all sub-entity ids, so the old code (which ignored entity/state
        // IDs) would have returned 3 (max(sprite_id=2) + 1), not 201.
        let sprite = Sprite::empty(
            pixhaus_core::project::SpriteId::new(2),
            "main",
            Size::new(8, 8),
        );
        install_sprite_as_new_entity(&mut project, sprite, 100, 200, None);
        let next = compute_next_id(&project);
        assert!(
            next > 200,
            "next_id must exceed EntityId and StateId; got {next} for max state_id 200"
        );
    }

    /// Pins the A2 fix: `LayerKind::Reference` layers carry a `PixelBufferId`
    /// for the reference image. Without the fix a project with such a layer
    /// whose buffer id exceeded every other id would hand out a colliding id
    /// after the next save+reload cycle.
    #[test]
    fn compute_next_id_includes_reference_layer_buffer_id() {
        use pixhaus_core::project::{
            BlendMode, IVec2, Layer, LayerId, LayerKind, PixelBufferId, Size, Sprite, SpriteId,
            UserData,
        };
        let mut project = Project::new("ref-layer-fixture");
        let mut sprite = Sprite::empty(SpriteId::new(2), "main", Size::new(8, 8));
        sprite.layers.push(Layer {
            id: LayerId::new(5),
            name: "bg".into(),
            kind: LayerKind::Raster,
            blend_mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            locked: false,
            parent: None,
            user_data: UserData::default(),
        });
        sprite.layers.push(Layer {
            id: LayerId::new(6),
            name: "ref".into(),
            kind: LayerKind::Reference {
                image: PixelBufferId::new(99),
                origin: IVec2 { x: 0, y: 0 },
            },
            blend_mode: BlendMode::Normal,
            opacity: 128,
            visible: true,
            locked: true,
            parent: None,
            user_data: UserData::default(),
        });
        install_sprite_as_new_entity(&mut project, sprite, 10, 11, None);
        let next = compute_next_id(&project);
        assert!(
            next > 99,
            "next_id must exceed reference-layer PixelBufferId; got {next}"
        );
    }

    /// Pins the A1 fix: `Tilemap`-kind entities carry `LayerId`s on their
    /// scene layers and sprite entities may carry `SheetVariantId`s in
    /// their embedded reference sheets.
    /// Both were silently skipped before this fix.
    #[test]
    fn compute_next_id_covers_tilemap_and_sprite_reference_content() {
        use pixhaus_core::project::{
            AiMetadata, AssetInfo, Entity, EntityContent, EntityDefaults, EntityId, EntityKind,
            LayerId, ReferenceImage, ReferenceSheet, SheetVariant, SheetVariantId, Size,
            TilemapData, TilemapLayer, TilemapScene, UserData,
        };

        let mut project = Project::new("tilemap-ref-fixture");

        // Tilemap entity with a layer carrying LayerId=50.
        project.library.entities.push(Entity {
            id: EntityId::new(1),
            kind: EntityKind::Tilemap,
            name: "Level1".into(),
            group_id: None,
            tags: Vec::new(),
            defaults: EntityDefaults::default(),
            content: EntityContent::Tilemap {
                scene: TilemapScene {
                    size: Size::new(10, 10),
                    tilesets: Vec::new(),
                    layers: vec![TilemapLayer {
                        id: LayerId::new(50),
                        name: "ground".into(),
                        data: TilemapData::empty(10, 10),
                        opacity: 255,
                        visible: true,
                    }],
                    properties: std::collections::BTreeMap::default(),
                },
            },
            ai: AiMetadata::default(),
            user_data: UserData::default(),
            created_at: 0,
            updated_at: 0,
        });

        // Sprite entity with canonical id=60 and a history variant id=70.
        project.library.entities.push(Entity {
            id: EntityId::new(2),
            kind: EntityKind::Custom("Character".into()),
            name: "Sheet".into(),
            group_id: None,
            tags: Vec::new(),
            defaults: EntityDefaults::default(),
            content: EntityContent::Sprites {
                states: Vec::new(),
                reference_sheet: Some(Box::new(ReferenceSheet {
                    canonical: Some(SheetVariant::from_image(
                        SheetVariantId::new(60),
                        0,
                        ReferenceImage {
                            bytes: vec![1],
                            mime: "image/png".into(),
                        },
                    )),
                    variants: vec![SheetVariant::from_image(
                        SheetVariantId::new(70),
                        0,
                        ReferenceImage {
                            bytes: vec![2],
                            mime: "image/png".into(),
                        },
                    )],
                    prompts: Vec::new(),
                    info: AssetInfo::default(),
                })),
            },
            ai: AiMetadata::default(),
            user_data: UserData::default(),
            created_at: 0,
            updated_at: 0,
        });

        let next = compute_next_id(&project);
        assert!(
            next > 70,
            "next_id must exceed all tilemap LayerIds and sprite reference SheetVariantIds; got {next}"
        );
    }

    /// `GroupId` and `TagId` also draw from `next_id`. If they exceed every
    /// sprite-side id, `compute_next_id` must still track past them.
    #[test]
    fn compute_next_id_includes_group_and_tag_ids() {
        use pixhaus_core::project::{EntityGroup, GroupId, Rgba, TagDefinition, TagId, UserData};
        let mut project = Project::new("group-tag-ids-fixture");
        project.library.groups.push(EntityGroup {
            id: GroupId::new(50),
            name: "Characters".into(),
            parent_id: None,
            user_data: UserData::default(),
        });
        project.library.tags.push(TagDefinition {
            id: TagId::new(75),
            name: "hero".into(),
            color: Some(Rgba {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            }),
            auto_generated: false,
        });
        let next = compute_next_id(&project);
        assert!(
            next > 75,
            "next_id must exceed GroupId and TagId; got {next}"
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
    fn install_imported_project_clears_history_and_stores_buffers() {
        // Both invariants in one test — they share a fixture and the
        // bug Copilot caught (history not cleared on PSD import) maps
        // directly to a missing assertion here.
        let mut doc = doc_with_history_entry();
        let entry = PixelBufferEntry {
            id: 7,
            width: 4,
            height: 4,
            stride: 16,
            pixels: vec![1u8; 64],
        };
        let archive = PixhausArchive {
            project: Project::new("imported"),
            buffers: vec![entry.clone()],
        };
        let status = install_imported_project(&mut doc, archive);
        assert_eq!(
            doc.history.node_count(),
            0,
            "project_import_psd must drop the previous session's undo history"
        );
        assert!(doc.dirty, "imported project is dirty until first save");
        assert!(doc.path.is_none(), "imported project has no on-disk path");
        assert!(status.path.is_none());
        assert_eq!(doc.pixel_buffers.len(), 1);
        assert_eq!(doc.pixel_buffers[0].id, entry.id);
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

        // encode_to_file does a sibling-tmp + rename. NamedTempFile
        // would hold the destination open and the rename would fail
        // on Windows; use a unique dir + non-existent filename instead.
        let tmp = tempfile::tempdir().expect("temp dir");
        let path = tmp.path().join("metadata-roundtrip.pixhaus");

        let archive = PixhausArchive::new(project);
        encode_to_file(&archive, &path).expect("encode");
        let loaded = decode_from_file(&path).expect("decode");

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

        // See `metadata_round_trips_through_pixhaus_archive` above for
        // why this uses tempdir() + join rather than NamedTempFile.
        let tmp = tempfile::tempdir().expect("temp dir");
        let path = tmp.path().join("buf-roundtrip.pixhaus");

        let archive = PixhausArchive { project, buffers };
        encode_to_file(&archive, &path).expect("encode");
        let loaded = decode_from_file(&path).expect("decode");

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

    // ── aseprite import ───────────────────────────────────────────────────────
    //
    // B9.5 restores the importer against the library data model.
    // Exercises the full decode → document_to_archive path against the
    // real fixture to catch regressions in the file-to-archive translation.

    #[test]
    fn import_aseprite_single_frame_rgba_produces_library_entity() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("app crate has parent")
            .join("examples")
            .join("aseprite-roundtrip")
            .join("single-frame-rgba.aseprite");

        let document = pixhaus_io::aseprite::decode_from_file(&path)
            .unwrap_or_else(|e| panic!("decode {}: {e:?}", path.display()));
        let result = pixhaus_io::aseprite::document_to_archive(&document, "imported")
            .unwrap_or_else(|e| panic!("import failed: {e:?}"));

        let entities = &result.archive.project.library.entities;
        assert_eq!(entities.len(), 1, "import must produce exactly one entity");

        let pixhaus_io::aseprite::archive::ConvertedArchive { archive, .. } = result;
        let entity = &archive.project.library.entities[0];
        let pixhaus_core::project::library::EntityContent::Sprites { states, .. } = &entity.content
        else {
            panic!("entity must be Sprites variant");
        };
        assert!(!states.is_empty(), "must have at least one state");
    }
}
