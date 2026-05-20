//! AI-integration commands: VLM auto-tagging, suggested-tag accept/reject,
//! style-corpus management, and per-entity `LoRA` training.
//!
//! Split out of the parent `library` module; shared helpers and external
//! imports are inherited via `use super::*`.

use super::*;

// ── AI hooks (B9.4) ───────────────────────────────────────────────────────────

/// Resolves an existing [`TagDefinition`] by name (case-insensitive) or mints
/// a new auto-generated one. Returns the `TagId`.
fn find_or_create_tag(
    library: &mut pixhaus_core::project::Library,
    next_id: &mut u32,
    name: &str,
) -> TagId {
    let normalized = name.to_lowercase();
    if let Some(existing) = library
        .tags
        .iter()
        .find(|t| t.name.to_lowercase() == normalized)
    {
        return existing.id;
    }
    let id = TagId::new(*next_id);
    *next_id += 1;
    library.tags.push(TagDefinition {
        id,
        name: normalized,
        color: None,
        auto_generated: true,
    });
    id
}

/// Applies a [`VerbEffect::SuggestEntityTags`] to the project: resolves or
/// creates [`TagDefinition`]s and adds their IDs to
/// `entity.ai.suggested_tags`. Returns the updated suggested-tag list so the
/// caller can surface it to the UI without a second read.
pub(crate) fn apply_suggest_entity_tags(
    project: &mut pixhaus_core::project::Project,
    next_id: &mut u32,
    entity_id: EntityId,
    tag_names: Vec<String>,
    ts: i64,
) -> Result<Vec<TagDefinition>, AppCommandError> {
    let entity_idx = project
        .library
        .entities
        .iter()
        .position(|e| e.id == entity_id)
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "entity".into(),
            id: u64::from(entity_id.get()),
        })?;

    let mut new_ids = Vec::with_capacity(tag_names.len());
    for name in tag_names {
        let trimmed = name.trim().to_owned();
        if trimmed.is_empty() {
            continue;
        }
        let id = find_or_create_tag(&mut project.library, next_id, &trimmed);
        new_ids.push(id);
    }

    // Scope the mutable entity borrow so the immutable tags borrow below compiles.
    let suggested_ids = {
        let entity = &mut project.library.entities[entity_idx];
        for id in new_ids {
            if !entity.ai.suggested_tags.contains(&id) {
                entity.ai.suggested_tags.push(id);
            }
        }
        entity.updated_at = ts;
        entity.ai.suggested_tags.clone()
    };

    let definitions = project
        .library
        .tags
        .iter()
        .filter(|t| suggested_ids.contains(&t.id))
        .cloned()
        .collect();
    Ok(definitions)
}

/// Applies a [`VerbEffect::UpdateProjectAi`]: deduplicates and appends entity
/// IDs to `ProjectAi.style_corpus`, and optionally overwrites the `LoRA` path.
///
/// IDs that don't currently exist in the project library are silently
/// dropped. Stale references in `style_corpus` would force every later
/// corpus consumer to defend against them; filtering at the write boundary
/// keeps the invariant local.
pub(crate) fn apply_update_project_ai(
    project: &mut pixhaus_core::project::Project,
    add_entity_ids: Vec<EntityId>,
    lora_path: Option<String>,
) {
    // Collect known entity ids once so the inner loop is O(n + m).
    let known: HashSet<EntityId> = project.library.entities.iter().map(|e| e.id).collect();

    for id in add_entity_ids {
        if !known.contains(&id) {
            continue; // skip unknown
        }
        if !project.library.ai.style_corpus.contains(&id) {
            project.library.ai.style_corpus.push(id);
        }
    }
    if let Some(path) = lora_path {
        project.library.ai.project_lora_path = Some(path);
    }
}

/// Applies a [`VerbEffect::UpdateEntityAi`]: overwrites
/// `Entity.ai.lora_path` for the named entity and bumps `updated_at`.
///
/// Returns `true` when the entity was found and updated, `false` when
/// the entity id is unknown (silently dropped — same invariant as
/// [`apply_update_project_ai`], unknown ids would be a stale write).
///
/// `None` for `lora_path` is a no-op: the verb returns `None` until
/// the host downloads the safetensors and supplies the real path.
pub(crate) fn apply_update_entity_ai(
    project: &mut pixhaus_core::project::Project,
    entity_id: EntityId,
    lora_path: Option<String>,
    ts: i64,
) -> bool {
    let Some(path) = lora_path else {
        return project.library.entities.iter().any(|e| e.id == entity_id);
    };
    let Some(entity) = project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == entity_id)
    else {
        return false;
    };
    entity.ai.lora_path = Some(path);
    entity.updated_at = ts;
    true
}

/// Renders an entity's name, kind, and existing tag names into a short
/// plaintext block the auto-tag VLM can use as grounding when no sprite
/// reference is attached.
fn build_auto_tag_metadata(entity: &Entity, tags: &[TagDefinition]) -> String {
    let kind = match &entity.kind {
        EntityKind::Tileset => "Tileset".to_owned(),
        EntityKind::Tilemap => "Tilemap".to_owned(),
        EntityKind::Custom(category) => format!("Custom({category})"),
    };
    let existing: Vec<&str> = entity
        .tags
        .iter()
        .filter_map(|tid| tags.iter().find(|t| t.id == *tid).map(|t| t.name.as_str()))
        .collect();
    let existing_str = if existing.is_empty() {
        "none".to_owned()
    } else {
        existing.join(", ")
    };
    format!(
        "Entity name: {}\nKind: {kind}\nExisting tags: {existing_str}",
        entity.name
    )
}

/// Invokes the Critique verb in `LibraryAutoTag` mode for the given entity.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_auto_tag_entity(
    entity_id: EntityId,
    state: State<'_, AppState>,
) -> CommandResult<Vec<TagDefinition>> {
    // Phase 1: read lock — build context, validate entity exists, and
    // construct the metadata summary the VLM will see (since we don't
    // attach a sprite reference today, the metadata is the only
    // grounding the model has).
    let (ctx, entity_metadata) = {
        let doc = state.doc.read().await;
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let entity = find_entity(&project.library, entity_id)?;
        let metadata = build_auto_tag_metadata(entity, &project.library.tags);
        let ctx = VerbContextBuilder::new(project.metadata.clone())
            .with_library_entity(entity_id)
            .build();
        (ctx, metadata)
        // read guard drops here
    };

    // Phase 2: invoke verb — no document lock held during the async call.
    let verb_id = VerbId::new("pixhaus.builtin.critique");
    let inputs = VerbInputs::from_struct(&CritiqueInputs {
        mode: CritiqueMode::LibraryAutoTag {
            entity_id,
            entity_metadata: Some(entity_metadata),
        },
        checks: vec![],
        notes: None,
    })
    .map_err(|e| AppCommandError::VerbError {
        message: e.to_string(),
    })?;

    let invocation = state
        .verb_runtime
        .invoke(&verb_id, ctx, inputs)
        .map_err(|e| AppCommandError::VerbError {
            message: e.to_string(),
        })?;

    let preview_id = invocation.preview_id().get();
    state
        .invocations
        .insert(preview_id, invocation.cancellation());

    let result = invocation.finish().await;
    state.invocations.remove(&preview_id);

    let preview = result.map_err(|e| AppCommandError::VerbError {
        message: e.to_string(),
    })?;

    let tag_names = preview
        .output
        .effects
        .into_iter()
        .find_map(|e| {
            if let VerbEffect::SuggestEntityTags { tag_names, .. } = e {
                Some(tag_names)
            } else {
                None
            }
        })
        .unwrap_or_default();

    // Phase 3: write lock — persist suggested tags to the entity.
    let mut doc = state.doc.write().await;
    let mut next_id = doc.next_id;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let suggestions =
        apply_suggest_entity_tags(project, &mut next_id, entity_id, tag_names, now_secs())?;
    doc.next_id = next_id;
    doc.dirty = true;
    Ok(suggestions)
}

/// Moves a tag from `entity.ai.suggested_tags` to `entity.tags`, confirming
/// the VLM suggestion.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_accept_suggested_tag(
    entity_id: EntityId,
    tag_id: TagId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;

    // Validate: the tag must currently be in the entity's suggested set —
    // accepting an arbitrary tag id (one nobody suggested, or one that
    // doesn't even exist) would silently corrupt entity.tags.
    {
        let entity = find_entity_mut(&mut project.library, entity_id)?;
        if !entity.ai.suggested_tags.contains(&tag_id) {
            return Err(AppCommandError::Validation {
                detail: format!(
                    "tag {} is not in entity {}'s suggested_tags",
                    tag_id.get(),
                    entity_id.get(),
                ),
            });
        }
    }

    // Canonical attach path: validates the tag definition exists and
    // bumps `updated_at`. Returns `Ok(false)` if the tag was already on
    // the entity — fine, we want idempotence on the accept. Run this
    // *before* dropping the suggestion so a failed attach doesn't leave
    // the suggestion silently removed (partial mutation).
    tag_entity_in_project(project, entity_id, tag_id, ts)?;

    // Attach succeeded — now retire the suggestion.
    find_entity_mut(&mut project.library, entity_id)?
        .ai
        .suggested_tags
        .retain(|&id| id != tag_id);

    doc.dirty = true;
    Ok(())
}

/// Removes a tag from `entity.ai.suggested_tags`, dismissing the suggestion.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_reject_suggested_tag(
    entity_id: EntityId,
    tag_id: TagId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let entity = find_entity_mut(&mut project.library, entity_id)?;
    entity.ai.suggested_tags.retain(|&id| id != tag_id);
    entity.updated_at = ts;
    doc.dirty = true;
    Ok(())
}

/// Adds entity IDs to `ProjectAi.style_corpus`, deduplicating against what is
/// already there. Corpus management is separate from verb invocation — this
/// command does not trigger training.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_update_corpus(
    entity_ids: Vec<EntityId>,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    apply_update_project_ai(project, entity_ids, None);
    doc.dirty = true;
    Ok(())
}

// ── B10.5: per-entity LoRA training ───────────────────────────────────────────

/// Inputs surfaced to the UI for [`library_train_entity_lora`].
///
/// Mirrors the verb's parameters minus the training images — the IPC
/// command extracts those from the entity's canonical sheet so the UI
/// never has to ship the bytes back to Rust.
#[derive(Debug, Deserialize)]
pub struct TrainEntityLoraOptions {
    /// `LoRA` rank (4-32). `None` falls back to the verb default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lora_rank: Option<u32>,
    /// Training step count (200-2000). `None` falls back to the verb
    /// default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<u32>,
    /// Trigger word / label. `None` lets the verb derive one from the
    /// entity name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Override Replicate model. `None` uses the verb default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Result returned by [`library_train_entity_lora`] on a successful train.
#[derive(Debug, serde::Serialize)]
pub struct LibraryTrainEntityLoraResult {
    /// Entity the weights are bound to.
    pub entity_id: EntityId,
    /// The `LoRA` path now stored on `Entity.ai.lora_path`. Currently
    /// the Replicate weights URL — downloading the safetensors into
    /// the project directory is a follow-up shared with the
    /// project-wide style training flow.
    pub lora_path: String,
    /// Trigger word the trainer used.
    pub label: String,
    /// Replicate training job ID, retained so the UI can surface it
    /// in audit traces.
    pub training_id: String,
    /// Per-invocation handle. Stringified `PreviewId` so the JS side
    /// doesn't lose precision on values above 2^53.
    pub invocation_id: String,
}

/// Decodes a PNG byte buffer into a tightly-packed RGBA8
/// [`pixhaus_ai::plugin::context::PixelData`].
///
/// The reference sheet ships as `image/png` (B10.1); this helper sits in
/// front of the verb's `PixelData`-typed inputs so the host never moves
/// PNG bytes through the protocol unnecessarily.
fn decode_reference_sheet_png(
    bytes: &[u8],
) -> Result<pixhaus_ai::plugin::context::PixelData, AppCommandError> {
    use image::ImageReader;
    use std::io::Cursor;

    let dyn_image = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| AppCommandError::Validation {
            detail: format!("failed to read reference sheet image header: {e}"),
        })?
        .decode()
        .map_err(|e| AppCommandError::Validation {
            detail: format!("failed to decode reference sheet image: {e}"),
        })?;
    let rgba = dyn_image.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok(pixhaus_ai::plugin::context::PixelData::rgba8(
        w,
        h,
        rgba.into_raw(),
    ))
}

/// Prepares the verb context and the decoded canonical sheet image for
/// [`library_train_entity_lora`]. Holds the read lock only for the brief
/// window needed to clone what the verb invocation will consume.
async fn build_train_entity_lora_context(
    entity_id: EntityId,
    state: &AppState,
) -> CommandResult<(
    pixhaus_ai::plugin::context::VerbContext,
    pixhaus_ai::plugin::context::PixelData,
    String,
)> {
    // Hold the read lock only long enough to clone what the verb consumes.
    // The CPU-bound PNG decode runs afterwards, off the lock and off the
    // async runtime threads, per the repo's spawn_blocking convention.
    let (image_bytes, metadata, entity_name) = {
        let doc = state.doc.read().await;
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let entity = find_entity(&project.library, entity_id)?;
        let sheet = match &entity.content {
            EntityContent::Sprites {
                reference_sheet: Some(sheet),
                ..
            } => sheet.as_ref(),
            _ => {
                return Err(AppCommandError::Validation {
                    detail: format!(
                        "entity {} has no sprite reference sheet; train_entity_lora requires one",
                        entity_id.get(),
                    ),
                });
            }
        };
        let Some(canonical) = sheet.canonical.as_ref() else {
            return Err(AppCommandError::Validation {
                detail: format!(
                    "entity {} has no approved canonical reference sheet; train_entity_lora requires one",
                    entity_id.get(),
                ),
            });
        };
        (
            canonical.image.bytes.clone(),
            project.metadata.clone(),
            entity.name.clone(),
        )
    };

    let decoded = tokio::task::spawn_blocking(move || decode_reference_sheet_png(&image_bytes))
        .await
        .map_err(|e| AppCommandError::Validation {
            detail: format!("reference sheet decode task failed: {e}"),
        })??;
    let ctx = VerbContextBuilder::new(metadata)
        .with_library_entity(entity_id)
        .build();
    Ok((ctx, decoded, entity_name))
}

/// Trains a per-entity `LoRA` from a sprite entity's canonical reference sheet
/// and persists the weights URL on `Entity.ai.lora_path`.
///
/// Three-phase flow:
///
/// 1. Read lock: extract the canonical sheet bytes and decode to
///    [`pixhaus_ai::plugin::context::PixelData`].
/// 2. No lock held: dispatch the
///    `pixhaus.builtin.train_entity_lora` verb via the runtime. The
///    cancel token is registered with the in-flight invocation table on
///    `AppState` so `verb_cancel` can interrupt the 15-30 minute
///    training run.
/// 3. Write lock: apply the
///    [`pixhaus_ai::plugin::output::VerbEffect::UpdateEntityAi`] effect
///    using the verb's `weights_url`, invalidate the anchor cache for
///    the entity, and bump `entity.updated_at`.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_train_entity_lora(
    entity_id: EntityId,
    options: Option<TrainEntityLoraOptions>,
    state: State<'_, AppState>,
) -> CommandResult<LibraryTrainEntityLoraResult> {
    use pixhaus_ai::verbs::train_entity_lora::{
        EntityLoraResult, TRAIN_ENTITY_LORA_EFFECT_NAME, TRAIN_ENTITY_LORA_VERB_ID,
        TrainEntityLoraInputs,
    };

    let opts = options.unwrap_or(TrainEntityLoraOptions {
        lora_rank: None,
        steps: None,
        label: None,
        model: None,
    });

    let (ctx, decoded, entity_label) = build_train_entity_lora_context(entity_id, &state).await?;

    let label = opts.label.clone().unwrap_or(entity_label);
    let inputs = VerbInputs::from_struct(&TrainEntityLoraInputs {
        entity_id,
        training_images: vec![decoded],
        lora_rank: opts.lora_rank,
        steps: opts.steps,
        label: Some(label.clone()),
        model: opts.model,
    })
    .map_err(|e| AppCommandError::VerbError {
        message: format!("failed to build train_entity_lora inputs: {e}"),
    })?;

    let verb_id = VerbId::new(TRAIN_ENTITY_LORA_VERB_ID);
    let invocation = state
        .verb_runtime
        .invoke(&verb_id, ctx, inputs)
        .map_err(|e| AppCommandError::VerbError {
            message: e.to_string(),
        })?;

    let preview_id = invocation.preview_id().get();
    state
        .invocations
        .insert(preview_id, invocation.cancellation());
    let result = invocation.finish().await;
    state.invocations.remove(&preview_id);

    let preview = result.map_err(|e| AppCommandError::VerbError {
        message: e.to_string(),
    })?;
    let lora_result = preview
        .output
        .effects
        .iter()
        .find_map(|e| match e {
            VerbEffect::Custom { name, payload } if name == TRAIN_ENTITY_LORA_EFFECT_NAME => {
                serde_json::from_value::<EntityLoraResult>(payload.clone()).ok()
            }
            _ => None,
        })
        .ok_or_else(|| AppCommandError::VerbError {
            message: format!(
                "train_entity_lora verb did not return a {TRAIN_ENTITY_LORA_EFFECT_NAME} effect"
            ),
        })?;

    {
        let mut doc = state.doc.write().await;
        let project = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?;
        let applied = apply_update_entity_ai(
            project,
            entity_id,
            Some(lora_result.weights_url.clone()),
            now_secs(),
        );
        if !applied {
            return Err(AppCommandError::NotFound {
                entity: "entity".into(),
                id: u64::from(entity_id.get()),
            });
        }
        doc.dirty = true;
    }
    state.anchor_cache.remove(&entity_id.get());

    Ok(LibraryTrainEntityLoraResult {
        entity_id,
        lora_path: lora_result.weights_url,
        label: lora_result.label,
        training_id: lora_result.training_id,
        invocation_id: preview_id.to_string(),
    })
}
