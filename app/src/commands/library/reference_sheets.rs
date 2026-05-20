//! Reference-sheet IPC commands: generation, refinement, variants,
//! chat turns, promotion, cross-model grids, vector export, the project
//! asset library, project-level AI preferences, and `LoRA` training.
//!
//! Split out of the parent `library` module; shared helpers, arg types,
//! and external imports are inherited via `use super::*`.

use super::*;

// ── embedded reference sheet commands ────────────────────────────────────────

/// Arguments for approving a history variant as canonical.
#[derive(Debug, Deserialize)]
pub struct LibraryApproveSheetVariantArgs {
    /// Target sprite entity. Must own an embedded reference sheet.
    pub entity_id: EntityId,
    /// The variant to approve. Must be present in the entity's `history`.
    pub variant_id: SheetVariantId,
}

/// Arguments for updating a sprite entity's embedded reference-sheet info.
#[derive(Debug, Deserialize)]
pub struct LibraryUpdateAssetInfoArgs {
    /// Target sprite entity. Must own an embedded reference sheet.
    pub entity_id: EntityId,
    /// Replacement asset info. Overwrites the existing value.
    pub info: AssetInfo,
}

/// Arguments for generating reference-sheet draft candidates.
#[derive(Debug, Deserialize)]
pub struct LibraryGenerateReferenceSheetArgs {
    /// Target sprite entity that will receive generated draft variants.
    pub entity_id: EntityId,
    /// User description of the subject.
    pub prompt: String,
    /// Sheet template.
    pub template: ReferenceSheetTemplateId,
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
    /// Requested chroma-key background color.
    pub chroma_color: Rgba,
    /// Backend quality hint.
    #[serde(default)]
    pub quality: Quality,
    /// Number of candidates to request. Clamped by the verb.
    pub candidate_count: u8,
    /// Optional explicit model override. `None`/`Auto` uses routing.
    #[serde(default)]
    pub model: Option<ModelId>,
    /// Ordered generation references.
    #[serde(default)]
    pub references: Vec<ReferenceSlot>,
    /// Search/grounding hint for Google image models.
    #[serde(default)]
    pub real_world_grounding: bool,
    /// Optional Flux `LoRA` asset to apply.
    #[serde(default)]
    pub applied_lora: Option<AssetId>,
    /// `LoRA` strength when a Flux `LoRA` is applied.
    #[serde(default = "default_request_lora_weight")]
    pub lora_weight: f32,
}

fn default_request_lora_weight() -> f32 {
    1.0
}

/// Arguments for importing a reference-sheet image as an unapproved draft.
#[derive(Debug, Deserialize)]
pub struct LibraryImportReferenceSheetArgs {
    /// Target sprite entity that will receive the draft.
    pub entity_id: EntityId,
    /// Image bytes to store.
    pub bytes: Vec<u8>,
    /// MIME type for `bytes`. Defaults to `image/png` when empty.
    pub mime: Option<String>,
}

/// Arguments for removing a non-canonical reference-sheet variant.
#[derive(Debug, Deserialize)]
pub struct LibraryRemoveReferenceSheetVariantArgs {
    /// Target sprite entity.
    pub entity_id: EntityId,
    /// Draft/history variant to remove.
    pub variant_id: SheetVariantId,
}

/// Generates draft reference-sheet variants for a sprite entity.
///
/// The generated candidates are persisted in `ReferenceSheet::history`
/// and never become canonical until the user approves one.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_generate_reference_sheet(
    args: LibraryGenerateReferenceSheetArgs,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<RequestId> {
    if args.prompt.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "reference sheet prompt must not be empty".into(),
        });
    }

    let (style_notes, model, lora_asset) = {
        let doc = state.doc.read().await;
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let entity = find_entity(&project.library, args.entity_id)?;
        if !matches!(entity.content, EntityContent::Sprites { .. }) {
            return Err(AppCommandError::Validation {
                detail: format!(
                    "entity {} is not a sprite entity; reference sheets belong to sprites",
                    args.entity_id.get()
                ),
            });
        }
        let preferred = selected_model(args.model, OperationKind::FreshGeneration, project);
        let model = configured_execution_model(preferred, &state);
        let lora_asset = find_lora_asset(project, args.applied_lora);
        (project.library.ai.style_notes.clone(), model, lora_asset)
    };

    let (request_id, token, lease) =
        start_sheet_request_with_lease(&state, args.entity_id, true, None)?;
    let spec = VariantCommitSpec {
        provider: SheetProviderRequest {
            operation: OperationKind::FreshGeneration,
            model,
            quality: args.quality,
            prompt: args.prompt,
            template: args.template,
            width: args.width,
            height: args.height,
            chroma_color: args.chroma_color,
            references: args.references,
            source_image: None,
            mask: None,
            candidate_count: args.candidate_count,
            applied_lora: lora_asset,
            lora_weight: args.lora_weight,
            real_world_grounding: args.real_world_grounding,
        },
        build: VariantBuildSpec {
            origin: VariantOrigin::FreshGeneration,
            parent_variant_id: None,
            refinement: None,
            promotion: false,
            chat_transcript: None,
            applied_lora: args.applied_lora,
            lora_weight: args.lora_weight,
            real_world_grounding: args.real_world_grounding,
        },
        stream_index: 0,
    };
    spawn_sheet_provider_job(
        app,
        request_id,
        args.entity_id,
        token,
        lease,
        style_notes,
        spec,
    );
    Ok(request_id)
}

/// Imports a reference image as a draft candidate on a sprite entity.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_import_reference_sheet(
    args: LibraryImportReferenceSheetArgs,
    state: State<'_, AppState>,
) -> CommandResult<Entity> {
    if args.bytes.is_empty() {
        return Err(AppCommandError::Validation {
            detail: "reference sheet image bytes must not be empty".into(),
        });
    }

    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let mut next_id = doc.next_id;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let updated = import_reference_sheet_draft(project, &mut next_id, args, ts)?;
    doc.next_id = next_id;
    doc.dirty = true;
    Ok(updated)
}

/// Approves a [`SheetVariant`] as the canonical embedded reference sheet of
/// a sprite entity.
///
/// Moves the variant from `history` into `canonical`, demotes the previous
/// canonical to `history[0]` when one exists, runs eyedropper palette
/// extraction over the new canonical's image bytes (skipped when the
/// variant already carries an extracted palette), bumps `updated_at`, and
/// invalidates any cached [`AnchorPayload`] for the entity. Returns the
/// updated entity so the UI can refresh local state without a separate
/// `library_get_entity`.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_approve_sheet_variant(
    args: LibraryApproveSheetVariantArgs,
    state: State<'_, AppState>,
) -> CommandResult<Entity> {
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    // Run the full B10.3 approval flow (variant swap + palette extraction).
    approve_sheet_variant(
        project,
        args.entity_id,
        args.variant_id,
        ExtractionOptions::default(),
    )?;
    // Bump `updated_at` so the UI refresh observes the entity as changed.
    let entity = find_entity_mut(&mut project.library, args.entity_id)?;
    entity.updated_at = ts;
    let updated = entity.clone();
    doc.dirty = true;
    drop(doc);

    state.anchor_cache.remove(&args.entity_id.get());

    Ok(updated)
}

/// Returns the current [`AnchorPayload`] for an entity, building it lazily
/// and caching the result.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_get_anchor_payload(
    entity_id: EntityId,
    state: State<'_, AppState>,
) -> CommandResult<Option<AnchorPayload>> {
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
        _ => return Ok(None),
    };
    let Some(canonical) = sheet.canonical.as_ref() else {
        return Ok(None);
    };
    let live_hash = pixhaus_ai::plugin::anchor::stable_hash(&canonical.image.bytes);
    let lora_path = crate::commands::verbs::resolve_lora_path(
        entity,
        project.library.ai.project_lora_path.as_deref(),
    );

    if let Some(cached) = state.anchor_cache.get(&entity.id.get()) {
        if cached.canonical_hash == live_hash && cached.lora_path == lora_path {
            return Ok(Some(cached.clone()));
        }
    }

    let payload = AnchorPayload::from_sprite_entity(entity, DEFAULT_ANCHOR_STRENGTH, lora_path);

    if let Some(p) = &payload {
        state
            .anchor_cache
            .insert(p.reference_entity_id.get(), p.clone());
    }

    Ok(payload)
}

/// Updates the asset info (name, age, species, personality notes) for a
/// sprite entity's embedded reference sheet.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_update_asset_info(
    args: LibraryUpdateAssetInfoArgs,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    update_asset_info_in_project(project, args.entity_id, args.info, ts)?;
    doc.dirty = true;
    Ok(())
}

/// Deletes a history variant from a sprite entity's embedded reference sheet.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_delete_sheet_variant(
    entity_id: EntityId,
    variant_id: SheetVariantId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    delete_sheet_variant_in_project(project, entity_id, variant_id, ts)?;
    doc.dirty = true;
    Ok(())
}

/// Removes a non-canonical reference-sheet draft/history variant.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_remove_reference_sheet_variant(
    args: LibraryRemoveReferenceSheetVariantArgs,
    state: State<'_, AppState>,
) -> CommandResult<Entity> {
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    delete_sheet_variant_in_project(project, args.entity_id, args.variant_id, ts)?;
    let updated = find_entity(&project.library, args.entity_id)?.clone();
    doc.dirty = true;
    Ok(updated)
}

/// Request id returned by v1 reference-sheet operations.
pub type RequestId = u64;

const SHEET_REQUEST_PROGRESS_EVENT: &str = "SheetRequestProgress";
const SHEET_REQUEST_CANDIDATE_COMPLETE_EVENT: &str = "SheetRequestCandidateComplete";
const SHEET_REQUEST_COMPLETE_EVENT: &str = "SheetRequestComplete";
const SHEET_REQUEST_CANCELLED_EVENT: &str = "SheetRequestCancelled";
const SHEET_REQUEST_ERROR_EVENT: &str = "SheetRequestError";
const TRAINING_JOB_PROGRESS_EVENT: &str = "TrainingJobProgress";
const TRAINING_JOB_COMPLETE_EVENT: &str = "TrainingJobComplete";
const TRAINING_JOB_FAILED_EVENT: &str = "TrainingJobFailed";

/// Progress event emitted for reference-sheet jobs.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize)]
pub struct SheetRequestProgressPayload {
    pub request_id: RequestId,
    pub stream_index: u8,
    pub candidate_index: u8,
    pub partial_index: u8,
    pub partial_image: Option<ReferenceImage>,
    pub elapsed_ms: u32,
}

/// Candidate-complete event emitted after a draft variant is committed.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize)]
pub struct SheetRequestCandidateCompletePayload {
    pub request_id: RequestId,
    pub stream_index: u8,
    pub candidate_index: u8,
    pub variant: SheetVariant,
}

/// Completion event emitted after all candidates for a request are committed.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize)]
pub struct SheetRequestCompletePayload {
    pub request_id: RequestId,
    pub entity_id: EntityId,
    pub sprite: Entity,
    pub total_cost_usd: f64,
}

/// Cancellation event emitted when a job exits through its cancellation token.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize)]
pub struct SheetRequestCancelledPayload {
    pub request_id: RequestId,
}

/// Error event emitted for provider/app failures after a request id is minted.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize)]
pub struct SheetRequestErrorPayload {
    pub request_id: RequestId,
    pub error: AppCommandError,
}

/// `LoRA` training progress event.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize)]
pub struct TrainingJobProgressPayload {
    pub job_id: TrainingJobId,
    pub status: TrainingStatus,
    pub message: String,
}

/// `LoRA` training completion event.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize)]
pub struct TrainingJobCompletePayload {
    pub job_id: TrainingJobId,
    pub lora_asset: LoraAsset,
}

/// `LoRA` training failure event.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize)]
pub struct TrainingJobFailedPayload {
    pub job_id: TrainingJobId,
    pub error: AppCommandError,
}

/// Model/quality pair used by cross-model comparison.
#[allow(missing_docs)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelQualityPair {
    pub model: ModelId,
    pub quality: Quality,
}

/// Arguments for refining a sheet variant.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct LibraryRefineReferenceSheetVariantArgs {
    pub entity_id: EntityId,
    pub parent_variant_id: SheetVariantId,
    pub refinement: RefinementKind,
    pub prompt: String,
    pub quality: Quality,
    pub candidate_count: u8,
    pub model: Option<ModelId>,
    #[serde(default)]
    pub additional_references: Vec<ReferenceSlot>,
}

/// Arguments for a conversational sheet edit.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct LibrarySubmitChatTurnArgs {
    pub entity_id: EntityId,
    pub variant_id: SheetVariantId,
    pub user_message: String,
    #[serde(default)]
    pub mask: Option<ReferenceImage>,
    pub model: Option<ModelId>,
}

/// Arguments for "promote to final".
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct LibraryPromoteVariantToFinalArgs {
    pub entity_id: EntityId,
    pub source_variant_id: SheetVariantId,
    pub target_quality: Quality,
    pub target_model: Option<ModelId>,
}

/// Arguments for cross-model comparison.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct LibraryStartCrossModelGridArgs {
    pub entity_id: EntityId,
    pub prompt: String,
    pub template: ReferenceSheetTemplateId,
    pub width: u32,
    pub height: u32,
    pub chroma_color: Rgba,
    #[serde(default)]
    pub references: Vec<ReferenceSlot>,
    pub combinations: Vec<ModelQualityPair>,
    pub candidate_count_per_combo: u8,
}

/// Arguments for vector export.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct LibraryExportVariantAsVectorArgs {
    pub entity_id: EntityId,
    pub variant_id: SheetVariantId,
}

/// Arguments for saving a raw reference asset.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct LibrarySaveReferenceToLibraryArgs {
    pub image: ReferenceImage,
    #[serde(default)]
    pub role: ReferenceRole,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Arguments for saving a variant as a card/swatch.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct LibrarySaveVariantCardArgs {
    pub entity_id: EntityId,
    pub variant_id: SheetVariantId,
    pub name: String,
    #[serde(default)]
    pub style_notes: String,
}

/// Arguments for updating reference asset tags.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct LibraryUpdateAssetTagsArgs {
    pub asset_id: AssetId,
    pub tags: Vec<String>,
}

/// Arguments for `LoRA` training.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct LibraryTrainLoraArgs {
    pub name: String,
    pub kind: LoraKind,
    pub target_model: ModelId,
    pub trigger_word: String,
    pub training_data: Vec<AssetId>,
}

/// Arguments for project operation model preferences.
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub struct ProjectSetOperationModelPrefArgs {
    pub operation: OperationKind,
    pub model: ModelId,
}

struct SheetRequestLease {
    manager: Arc<crate::state::ReferenceSheetRequestManager>,
    request_id: RequestId,
    entity_id: EntityId,
    counts_toward_sprite_cap: bool,
    chat_variant_id: Option<SheetVariantId>,
}

impl Drop for SheetRequestLease {
    fn drop(&mut self) {
        self.manager.finish(
            self.request_id,
            self.entity_id.get(),
            self.counts_toward_sprite_cap,
            self.chat_variant_id.map(SheetVariantId::get),
        );
    }
}

#[derive(Clone)]
struct SheetProviderRequest {
    operation: OperationKind,
    model: ModelId,
    quality: Quality,
    prompt: String,
    template: ReferenceSheetTemplateId,
    width: u32,
    height: u32,
    chroma_color: Rgba,
    references: Vec<ReferenceSlot>,
    source_image: Option<ReferenceImage>,
    mask: Option<ReferenceImage>,
    candidate_count: u8,
    applied_lora: Option<LoraAsset>,
    lora_weight: f32,
    real_world_grounding: bool,
}

#[derive(Clone)]
struct VariantBuildSpec {
    origin: VariantOrigin,
    parent_variant_id: Option<SheetVariantId>,
    refinement: Option<RefinementKind>,
    promotion: bool,
    chat_transcript: Option<ChatTranscript>,
    applied_lora: Option<AssetId>,
    lora_weight: f32,
    real_world_grounding: bool,
}

#[derive(Clone)]
struct VariantCommitSpec {
    provider: SheetProviderRequest,
    build: VariantBuildSpec,
    stream_index: u8,
}

#[derive(Clone)]
struct GeneratedSheetImage {
    image: ReferenceImage,
    model: ModelId,
    cost_usd: Option<f64>,
}

fn start_sheet_request_with_lease(
    state: &AppState,
    entity_id: EntityId,
    counts_toward_sprite_cap: bool,
    chat_variant_id: Option<SheetVariantId>,
) -> CommandResult<(RequestId, CancellationToken, SheetRequestLease)> {
    let (request_id, token) = state
        .reference_sheet_requests
        .start(
            entity_id.get(),
            counts_toward_sprite_cap,
            chat_variant_id.map(SheetVariantId::get),
        )
        .map_err(|detail| AppCommandError::Validation {
            detail: detail.into(),
        })?;
    let lease = SheetRequestLease {
        manager: state.reference_sheet_requests.clone(),
        request_id,
        entity_id,
        counts_toward_sprite_cap,
        chat_variant_id,
    };
    Ok((request_id, token, lease))
}

fn find_variant(sheet: &ReferenceSheet, variant_id: SheetVariantId) -> Option<&SheetVariant> {
    sheet
        .canonical
        .as_ref()
        .filter(|variant| variant.id == variant_id)
        .or_else(|| {
            sheet
                .variants
                .iter()
                .find(|variant| variant.id == variant_id)
        })
}

fn emit_sheet_event<T: Serialize>(app: &AppHandle, event: &str, payload: &T) {
    if let Err(err) = app.emit(event, payload) {
        tracing::warn!(event, error = %err, "failed to emit reference-sheet event");
    }
}

fn emit_sheet_error(app: &AppHandle, request_id: RequestId, error: AppCommandError) {
    emit_sheet_event(
        app,
        SHEET_REQUEST_ERROR_EVENT,
        &SheetRequestErrorPayload { request_id, error },
    );
}

fn emit_sheet_cancelled(app: &AppHandle, request_id: RequestId) {
    emit_sheet_event(
        app,
        SHEET_REQUEST_CANCELLED_EVENT,
        &SheetRequestCancelledPayload { request_id },
    );
}

fn elapsed_ms_since(started: SystemTime) -> u32 {
    started.elapsed().map_or(0, |elapsed| {
        u32::try_from(elapsed.as_millis()).unwrap_or(u32::MAX)
    })
}

fn pixel_data_to_reference_image(pixels: &PixelData) -> Option<ReferenceImage> {
    if pixels.bytes_per_pixel != 4 || pixels.width == 0 || pixels.height == 0 {
        return None;
    }
    let row_len = usize::try_from(pixels.width).ok()?.checked_mul(4)?;
    let stride = usize::try_from(pixels.stride).ok()?;
    if stride < row_len {
        return None;
    }
    let mut packed = Vec::with_capacity(row_len.checked_mul(usize::try_from(pixels.height).ok()?)?);
    for y in 0..usize::try_from(pixels.height).ok()? {
        let start = y.checked_mul(stride)?;
        let end = start.checked_add(row_len)?;
        packed.extend_from_slice(pixels.bytes.get(start..end)?);
    }
    let image =
        ImageBuffer::<ImageRgba<u8>, Vec<u8>>::from_raw(pixels.width, pixels.height, packed)?;
    let bytes = encode_rgba_png(&image).ok()?;
    Some(ReferenceImage {
        bytes,
        mime: "image/png".into(),
    })
}

fn spawn_provider_progress_bridge(
    app: AppHandle,
    request_id: RequestId,
    stream_index: u8,
    started: SystemTime,
    mut progress_rx: tokio::sync::mpsc::Receiver<VerbProgressEvent>,
) {
    tauri::async_runtime::spawn(async move {
        let mut partial_index: u8 = 0;
        while let Some(event) = progress_rx.recv().await {
            match event {
                VerbProgressEvent::PartialPixels {
                    effect_index,
                    pixels,
                } => {
                    partial_index = partial_index.saturating_add(1);
                    emit_sheet_event(
                        &app,
                        SHEET_REQUEST_PROGRESS_EVENT,
                        &SheetRequestProgressPayload {
                            request_id,
                            stream_index,
                            candidate_index: u8::try_from(effect_index).unwrap_or(u8::MAX),
                            partial_index,
                            partial_image: pixel_data_to_reference_image(&pixels),
                            elapsed_ms: elapsed_ms_since(started),
                        },
                    );
                }
                VerbProgressEvent::Started { .. }
                | VerbProgressEvent::Step { .. }
                | VerbProgressEvent::Cost(_)
                | VerbProgressEvent::Log { .. }
                | VerbProgressEvent::Eta { .. } => {
                    emit_sheet_event(
                        &app,
                        SHEET_REQUEST_PROGRESS_EVENT,
                        &SheetRequestProgressPayload {
                            request_id,
                            stream_index,
                            candidate_index: 0,
                            partial_index,
                            partial_image: None,
                            elapsed_ms: elapsed_ms_since(started),
                        },
                    );
                }
            }
        }
    });
}

fn emit_training_event<T: Serialize>(app: &AppHandle, event: &str, payload: &T) {
    if let Err(err) = app.emit(event, payload) {
        tracing::warn!(event, error = %err, "failed to emit training event");
    }
}

fn model_provider(model: ModelId) -> &'static str {
    match model {
        ModelId::Auto | ModelId::OpenAiGptImage2 => "openai",
        ModelId::GoogleNanoBananaPro | ModelId::GoogleGeminiFlashImage => "google_ai",
        ModelId::FalFluxKontext
        | ModelId::FalFluxDev
        | ModelId::FalRecraftVectorize
        | ModelId::FalRealEsrgan => "fal",
    }
}

fn is_openai_model(model: ModelId) -> bool {
    matches!(model, ModelId::Auto | ModelId::OpenAiGptImage2)
}

fn model_label(model: ModelId) -> &'static str {
    match model {
        ModelId::Auto => "auto",
        ModelId::OpenAiGptImage2 => "gpt-image-2",
        ModelId::GoogleNanoBananaPro => "gemini-3-pro-image-preview",
        ModelId::GoogleGeminiFlashImage => "gemini-3.1-flash-image-preview",
        ModelId::FalFluxKontext => "fal-ai/flux-pro/kontext",
        ModelId::FalFluxDev => "fal-ai/flux-lora",
        ModelId::FalRecraftVectorize => "fal-ai/recraft/vectorize",
        ModelId::FalRealEsrgan => "fal-ai/real-esrgan",
    }
}

fn image_quality(quality: Quality) -> ImageQuality {
    match quality {
        Quality::Auto => ImageQuality::Auto,
        Quality::Low => ImageQuality::Low,
        Quality::Medium => ImageQuality::Medium,
        Quality::High => ImageQuality::High,
    }
}

fn chroma_hex(color: Rgba) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}

fn reference_role_name(role: ReferenceRole) -> &'static str {
    match role {
        ReferenceRole::Subject => "subject",
        ReferenceRole::Style => "style",
        ReferenceRole::Pose => "pose",
        ReferenceRole::Outfit => "outfit",
        ReferenceRole::Context => "context",
        ReferenceRole::Generic => "generic",
    }
}

fn reference_data_uri(image: &ReferenceImage) -> String {
    format!(
        "data:{};base64,{}",
        image.mime,
        base64::engine::general_purpose::STANDARD.encode(&image.bytes)
    )
}

fn compose_sheet_prompt(request: &SheetProviderRequest, style_notes: &str) -> String {
    let mut parts = Vec::new();
    if !style_notes.trim().is_empty() {
        parts.push(format!("Project style notes:\n{}", style_notes.trim()));
    }
    parts.push(format!(
        "Create a sprite reference sheet using template {:?} at {}x{}.",
        request.template, request.width, request.height
    ));
    parts.push(format!(
        "Background must be a flat solid chroma key color {} with no shadows, gradients, or background props.",
        chroma_hex(request.chroma_color)
    ));
    if !request.references.is_empty() {
        let hints = request
            .references
            .iter()
            .enumerate()
            .map(|(i, slot)| {
                format!(
                    "Reference {} is {} guidance with weight {:.2}.",
                    i + 1,
                    reference_role_name(slot.role),
                    slot.weight
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        parts.push(hints);
    }
    if request.real_world_grounding
        && matches!(
            request.model,
            ModelId::GoogleNanoBananaPro | ModelId::GoogleGeminiFlashImage
        )
    {
        parts.push("Use accurate real-world references for named places, objects, and scenes when composing this image.".into());
    }
    if let Some(lora) = &request.applied_lora {
        parts.push(format!(
            "Apply the Flux LoRA trigger word `{}` at weight {:.2}.",
            lora.trigger_word, request.lora_weight
        ));
    }
    match request.operation {
        OperationKind::MaskedRefinement | OperationKind::RegionalRefinement => {
            parts.push("Preserve everything outside the edited region.".into());
        }
        OperationKind::PromptOnlyRefinement | OperationKind::ChatTurn => {
            parts.push("Preserve the character identity, proportions, palette, and sheet layout unless the user specifically asks to change them.".into());
        }
        OperationKind::Promotion => {
            parts.push(
                "Re-render this approved direction as a polished final reference sheet.".into(),
            );
        }
        _ => {}
    }
    parts.push(request.prompt.trim().to_owned());
    parts.join("\n\n")
}

fn selected_model(
    explicit: Option<ModelId>,
    operation: OperationKind,
    project: &pixhaus_core::project::Project,
) -> ModelId {
    explicit
        .filter(|model| *model != ModelId::Auto)
        .or({
            project
                .library
                .ai
                .per_operation_model_prefs
                .get(&operation)
                .copied()
                .filter(|model| *model != ModelId::Auto)
        })
        .unwrap_or(match operation {
            OperationKind::FreshGeneration => ModelId::GoogleNanoBananaPro,
            OperationKind::MaskedRefinement
            | OperationKind::RegionalRefinement
            | OperationKind::Promotion
            | OperationKind::CrossModelGrid => ModelId::OpenAiGptImage2,
            OperationKind::PromptOnlyRefinement | OperationKind::ChatTurn => {
                ModelId::GoogleNanoBananaPro
            }
            OperationKind::VectorExport => ModelId::FalRecraftVectorize,
            OperationKind::Upscale => ModelId::FalRealEsrgan,
            OperationKind::LoraTraining => ModelId::FalFluxDev,
        })
}

fn find_lora_asset(
    project: &pixhaus_core::project::Project,
    asset_id: Option<AssetId>,
) -> Option<LoraAsset> {
    asset_id.and_then(|asset_id| {
        project
            .library
            .ai
            .asset_library
            .loras
            .iter()
            .find(|asset| asset.id == asset_id)
            .cloned()
    })
}

fn configured_execution_model(preferred: ModelId, state: &AppState) -> ModelId {
    if is_openai_model(preferred) {
        return preferred;
    }
    let backends = state.verb_runtime.list_backends();
    let preferred_ready = backends
        .iter()
        .any(|backend| backend.id == model_provider(preferred) && backend.available);
    if preferred_ready {
        return preferred;
    }
    let openai_ready = backends
        .iter()
        .any(|backend| backend.id == "openai" && backend.available);
    if openai_ready {
        ModelId::OpenAiGptImage2
    } else {
        preferred
    }
}

fn mint_sheet_variant_id(next_id: &mut u32) -> SheetVariantId {
    let id = SheetVariantId::new(*next_id);
    *next_id += 1;
    id
}

fn mint_asset_id(next_id: &mut u32) -> AssetId {
    let id = AssetId::new(*next_id);
    *next_id += 1;
    id
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
async fn invoke_provider_images(
    app: AppHandle,
    request_id: RequestId,
    stream_index: u8,
    started: SystemTime,
    runtime: Arc<VerbRuntime>,
    request: &SheetProviderRequest,
    style_notes: &str,
    cancel: CancellationToken,
) -> CommandResult<Vec<GeneratedSheetImage>> {
    if cancel.is_cancelled() {
        return Err(AppCommandError::VerbError {
            message: "reference-sheet request cancelled".into(),
        });
    }
    let capability = if request.source_image.is_some() {
        if request.mask.is_some() {
            BackendCapabilities::IMAGE_INPAINT
        } else {
            BackendCapabilities::IMAGE_EDIT
        }
    } else {
        BackendCapabilities::IMAGE_GENERATION
    };
    let backend_id = model_provider(request.model);
    let backend = runtime
        .select_backend_by_id(
            backend_id,
            capability,
            &VerbId::new("pixhaus.reference_sheet.dispatch"),
        )
        .map_err(|err| AppCommandError::VerbError {
            message: err.to_string(),
        })?;
    let proxy = backend
        .as_any()
        .downcast_ref::<BackendProxy>()
        .ok_or_else(|| AppCommandError::VerbError {
            message: "selected backend does not expose the image execution surface".into(),
        })?;

    let prompt = compose_sheet_prompt(request, style_notes);
    let model = if request.model == ModelId::Auto {
        None
    } else {
        Some(model_label(request.model).to_owned())
    };
    let (progress, progress_rx) = VerbProgress::channel();
    spawn_provider_progress_bridge(app, request_id, stream_index, started, progress_rx);
    let reference_images = request
        .references
        .iter()
        .map(|slot| slot.image.bytes.clone())
        .collect::<Vec<_>>();
    let response = if backend_id == "fal" {
        proxy
            .fat()
            .invoke(
                InferenceRequest::Replicate(pixhaus_ai::backends::ReplicateRequest {
                    model: model_label(request.model).into(),
                    version: None,
                    input: build_fal_sheet_input(request, &prompt),
                }),
                progress,
                cancel,
            )
            .await
            .map_err(|err| AppCommandError::VerbError {
                message: err.to_string(),
            })?
    } else if let Some(source) = &request.source_image {
        let edit = ImageEditRequest {
            model,
            image: source.bytes.clone(),
            mask: request.mask.as_ref().map(|mask| mask.bytes.clone()),
            prompt,
            negative_prompt: None,
            num_images: u32::from(request.candidate_count.clamp(1, 4)),
            style_image: request
                .references
                .first()
                .map(|slot| slot.image.bytes.clone()),
            reference_images,
        };
        let req = if request.mask.is_some() {
            InferenceRequest::ImageInpaint(edit)
        } else {
            InferenceRequest::ImageEdit(edit)
        };
        proxy
            .fat()
            .invoke(req, progress, cancel)
            .await
            .map_err(|err| AppCommandError::VerbError {
                message: err.to_string(),
            })?
    } else {
        let image_gen = ImageGenRequest {
            model,
            prompt,
            negative_prompt: None,
            width: request.width,
            height: request.height,
            steps: None,
            seed: None,
            num_images: u32::from(request.candidate_count.clamp(1, 4)),
            quality: Some(image_quality(request.quality)),
            style_image: request
                .references
                .first()
                .map(|slot| slot.image.bytes.clone()),
            reference_images,
        };
        proxy
            .fat()
            .invoke(
                InferenceRequest::ImageGeneration(image_gen),
                progress,
                cancel,
            )
            .await
            .map_err(|err| AppCommandError::VerbError {
                message: err.to_string(),
            })?
    };

    let images = match response {
        InferenceResponse::Image(output) => output.images,
        _ => {
            return Err(AppCommandError::VerbError {
                message: "provider returned a non-image response for reference-sheet operation"
                    .into(),
            });
        }
    };
    if images.is_empty() {
        return Err(AppCommandError::VerbError {
            message: "provider returned zero reference-sheet images".into(),
        });
    }
    Ok(images
        .into_iter()
        .map(|bytes| GeneratedSheetImage {
            image: ReferenceImage {
                bytes,
                mime: "image/png".into(),
            },
            model: request.model,
            cost_usd: None,
        })
        .collect())
}

fn region_bounds(region: &pixhaus_core::project::RegionDefinition) -> Option<(u32, u32, u32, u32)> {
    let min_x = region.polygon.iter().map(|p| p.x).min()?;
    let min_y = region.polygon.iter().map(|p| p.y).min()?;
    let max_x = region.polygon.iter().map(|p| p.x).max()?;
    let max_y = region.polygon.iter().map(|p| p.y).max()?;
    if max_x <= min_x || max_y <= min_y {
        return None;
    }
    Some((
        u32::try_from(min_x.max(0)).unwrap_or(0),
        u32::try_from(min_y.max(0)).unwrap_or(0),
        u32::try_from(max_x.max(0)).unwrap_or(0),
        u32::try_from(max_y.max(0)).unwrap_or(0),
    ))
}

fn validate_non_overlapping_regions(
    regions: &[pixhaus_core::project::RegionDefinition],
) -> CommandResult<Vec<(u32, u32, u32, u32)>> {
    let mut bounds = Vec::with_capacity(regions.len());
    for region in regions {
        let Some(next) = region_bounds(region) else {
            return Err(AppCommandError::Validation {
                detail: "regional refinement region must have a non-empty polygon".into(),
            });
        };
        if bounds.iter().any(|existing| rects_overlap(*existing, next)) {
            return Err(AppCommandError::Validation {
                detail: "regional refinement regions must not overlap".into(),
            });
        }
        bounds.push(next);
    }
    Ok(bounds)
}

fn rects_overlap(a: (u32, u32, u32, u32), b: (u32, u32, u32, u32)) -> bool {
    let (ax0, ay0, ax1, ay1) = a;
    let (bx0, by0, bx1, by1) = b;
    ax0 < bx1 && ax1 > bx0 && ay0 < by1 && ay1 > by0
}

fn region_mask_png(
    width: u32,
    height: u32,
    bounds: (u32, u32, u32, u32),
) -> CommandResult<ReferenceImage> {
    let (x0, y0, x1, y1) = bounds;
    let mut mask: ImageBuffer<ImageRgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(width, height, ImageRgba([0, 0, 0, 255]));
    let max_x = x1.min(width);
    let max_y = y1.min(height);
    for y in y0.min(height)..max_y {
        for x in x0.min(width)..max_x {
            mask.put_pixel(x, y, ImageRgba([255, 255, 255, 255]));
        }
    }
    let mut bytes = Vec::new();
    mask.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|err| AppCommandError::Validation {
            detail: format!("failed to encode regional mask: {err}"),
        })?;
    Ok(ReferenceImage {
        bytes,
        mime: "image/png".into(),
    })
}

fn composite_region(
    base: &mut ImageBuffer<ImageRgba<u8>, Vec<u8>>,
    edited: &[u8],
    bounds: (u32, u32, u32, u32),
) -> CommandResult<()> {
    let edited = image::load_from_memory(edited)
        .map_err(|err| AppCommandError::Validation {
            detail: format!("failed to decode regional refinement output: {err}"),
        })?
        .into_rgba8();
    let (x0, y0, x1, y1) = bounds;
    let width = base.width().min(edited.width());
    let height = base.height().min(edited.height());
    for y in y0.min(height)..y1.min(height) {
        for x in x0.min(width)..x1.min(width) {
            let pixel = *edited.get_pixel(x, y);
            base.put_pixel(x, y, pixel);
        }
    }
    Ok(())
}

fn encode_rgba_png(image: &ImageBuffer<ImageRgba<u8>, Vec<u8>>) -> CommandResult<Vec<u8>> {
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|err| AppCommandError::Validation {
            detail: format!("failed to encode regional composite: {err}"),
        })?;
    Ok(bytes)
}

fn encode_reference_assets_zip(assets: &[ReferenceAsset]) -> CommandResult<Vec<u8>> {
    let buf = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (i, asset) in assets.iter().enumerate() {
        let ext = if asset.image.mime.contains("jpeg") || asset.image.mime.contains("jpg") {
            "jpg"
        } else if asset.image.mime.contains("webp") {
            "webp"
        } else {
            "png"
        };
        zip.start_file(format!("{i:04}.{ext}"), options)
            .map_err(|err| AppCommandError::Validation {
                detail: format!("failed to create LoRA training archive: {err}"),
            })?;
        zip.write_all(&asset.image.bytes)
            .map_err(|err| AppCommandError::Validation {
                detail: format!("failed to write LoRA training archive: {err}"),
            })?;
    }
    zip.finish()
        .map(std::io::Cursor::into_inner)
        .map_err(|err| AppCommandError::Validation {
            detail: format!("failed to finish LoRA training archive: {err}"),
        })
}

#[allow(clippy::disallowed_methods)]
fn build_fal_sheet_input(request: &SheetProviderRequest, prompt: &str) -> serde_json::Value {
    let mut input = serde_json::json!({
        "prompt": prompt,
        "num_images": request.candidate_count.clamp(1, 4),
        "sync_mode": true,
    });
    if request.source_image.is_none() {
        input["image_size"] = serde_json::json!({
            "width": request.width,
            "height": request.height,
        });
    }
    if let Some(source) = &request.source_image {
        input["image_url"] = serde_json::json!(reference_data_uri(source));
    }
    if let Some(mask) = &request.mask {
        input["mask_url"] = serde_json::json!(reference_data_uri(mask));
    }
    let reference_urls = request
        .references
        .iter()
        .map(|slot| reference_data_uri(&slot.image))
        .collect::<Vec<_>>();
    if let Some(first) = reference_urls.first() {
        input["reference_image_url"] = serde_json::json!(first);
        input["image_url"] = input
            .get("image_url")
            .cloned()
            .unwrap_or_else(|| serde_json::json!(first));
    }
    if !reference_urls.is_empty() {
        input["reference_image_urls"] = serde_json::json!(reference_urls);
    }
    if let Some(lora) = &request.applied_lora {
        input["loras"] = serde_json::json!([
            {
                "path": lora.fal_lora_url,
                "scale": request.lora_weight,
            }
        ]);
        input["lora_url"] = serde_json::json!(lora.fal_lora_url);
        input["lora_scale"] = serde_json::json!(request.lora_weight);
    }
    input
}

#[allow(clippy::too_many_arguments)]
async fn invoke_regional_refinement(
    app: AppHandle,
    request_id: RequestId,
    stream_index: u8,
    started: SystemTime,
    runtime: Arc<VerbRuntime>,
    source: &SheetVariant,
    request: &SheetProviderRequest,
    regions: &[pixhaus_core::project::RegionDefinition],
    style_notes: &str,
    cancel: CancellationToken,
) -> CommandResult<Vec<GeneratedSheetImage>> {
    let bounds = validate_non_overlapping_regions(regions)?;
    let mut outputs = Vec::new();
    for _ in 0..request.candidate_count.clamp(1, 4) {
        let mut composite = image::load_from_memory(&source.image.bytes)
            .map_err(|err| AppCommandError::Validation {
                detail: format!("failed to decode source sheet for regional refinement: {err}"),
            })?
            .into_rgba8();
        for (region, bounds) in regions.iter().zip(bounds.iter().copied()) {
            let mask = region_mask_png(source.width, source.height, bounds)?;
            let mut refs = request.references.clone();
            refs.extend(region.region_references.clone());
            let mut region_request = request.clone();
            region_request.prompt = format!(
                "{}\n\nRegion instruction: {}",
                request.prompt, region.prompt
            );
            region_request.references = refs;
            region_request.source_image = Some(ReferenceImage {
                bytes: encode_rgba_png(&composite)?,
                mime: "image/png".into(),
            });
            region_request.mask = Some(mask);
            region_request.candidate_count = 1;
            let images = invoke_provider_images(
                app.clone(),
                request_id,
                stream_index,
                started,
                runtime.clone(),
                &region_request,
                style_notes,
                cancel.clone(),
            )
            .await?;
            if cancel.is_cancelled() {
                return Err(AppCommandError::VerbError {
                    message: "reference-sheet request cancelled".into(),
                });
            }
            let Some(image) = images.into_iter().next() else {
                return Err(AppCommandError::VerbError {
                    message: "regional refinement provider returned no image".into(),
                });
            };
            composite_region(&mut composite, &image.image.bytes, bounds)?;
        }
        outputs.push(GeneratedSheetImage {
            image: ReferenceImage {
                bytes: encode_rgba_png(&composite)?,
                mime: "image/png".into(),
            },
            model: request.model,
            cost_usd: None,
        });
    }
    Ok(outputs)
}

#[allow(clippy::disallowed_methods)]
async fn invoke_vector_export(
    runtime: Arc<VerbRuntime>,
    image: &ReferenceImage,
    cancel: CancellationToken,
) -> CommandResult<ReferenceImage> {
    let backend = runtime
        .select_backend_by_id(
            "fal",
            BackendCapabilities::IMAGE_GENERATION,
            &VerbId::new("pixhaus.reference_sheet.vector_export"),
        )
        .map_err(|err| AppCommandError::VerbError {
            message: err.to_string(),
        })?;
    let proxy = backend
        .as_any()
        .downcast_ref::<BackendProxy>()
        .ok_or_else(|| AppCommandError::VerbError {
            message: "selected fal backend does not expose the image execution surface".into(),
        })?;
    let data_uri = format!(
        "data:{};base64,{}",
        image.mime,
        base64::engine::general_purpose::STANDARD.encode(&image.bytes)
    );
    let response = proxy
        .fat()
        .invoke(
            InferenceRequest::Replicate(pixhaus_ai::backends::ReplicateRequest {
                model: model_label(ModelId::FalRecraftVectorize).into(),
                version: None,
                input: serde_json::json!({ "image_url": data_uri }),
            }),
            VerbProgress::discard(),
            cancel,
        )
        .await
        .map_err(|err| AppCommandError::VerbError {
            message: err.to_string(),
        })?;
    let InferenceResponse::Image(output) = response else {
        return Err(AppCommandError::VerbError {
            message: "fal vector export returned a non-image response".into(),
        });
    };
    let Some(bytes) = output.images.into_iter().next() else {
        return Err(AppCommandError::VerbError {
            message: "fal vector export returned no SVG".into(),
        });
    };
    Ok(ReferenceImage {
        bytes,
        mime: "image/svg+xml".into(),
    })
}

fn build_variant_from_provider_image(
    id: SheetVariantId,
    ts: i64,
    image: GeneratedSheetImage,
    spec: &VariantCommitSpec,
) -> SheetVariant {
    let mut variant = SheetVariant::from_image(id, ts, image.image);
    variant.template = spec.provider.template;
    variant.width = spec.provider.width;
    variant.height = spec.provider.height;
    variant.chroma_color = spec.provider.chroma_color;
    variant.user_prompt.clone_from(&spec.provider.prompt);
    variant.composed_prompt = compose_sheet_prompt(&spec.provider, "");
    variant.references.clone_from(&spec.provider.references);
    variant.model = image.model;
    variant.quality = spec.provider.quality;
    variant.parent_variant_id = spec.build.parent_variant_id;
    variant.origin = spec.build.origin;
    variant.refinement.clone_from(&spec.build.refinement);
    variant.chat_transcript = spec.build.chat_transcript.clone().map(|mut transcript| {
        if let Some(last) = transcript.turns.last_mut() {
            last.resulting_variant_id = id;
        }
        transcript
    });
    variant.promotion = spec.build.promotion;
    variant.applied_lora = spec.build.applied_lora;
    variant.lora_weight = spec.build.lora_weight;
    variant.real_world_grounding = spec.build.real_world_grounding;
    variant.cost_usd = image.cost_usd;
    variant.extracted_palette =
        extract_palette_from_image_bytes(&variant.image.bytes, ExtractionOptions::default())
            .unwrap_or_default();
    variant
}

fn embedded_reference_sheet_or_create_mut(
    entity: &mut Entity,
) -> Result<&mut ReferenceSheet, AppCommandError> {
    match &mut entity.content {
        EntityContent::Sprites {
            reference_sheet, ..
        } => Ok(reference_sheet
            .get_or_insert_with(|| {
                Box::new(ReferenceSheet {
                    canonical: None,
                    variants: Vec::new(),
                    prompts: Vec::new(),
                    info: AssetInfo::default(),
                })
            })
            .as_mut()),
        _ => Err(AppCommandError::Validation {
            detail: format!(
                "entity {} is not a sprite entity; reference sheets belong to sprites",
                entity.id.get()
            ),
        }),
    }
}

async fn commit_provider_variants(
    app: &AppHandle,
    request_id: RequestId,
    entity_id: EntityId,
    specs: Vec<(VariantCommitSpec, Vec<GeneratedSheetImage>)>,
) -> CommandResult<Entity> {
    let ts = now_secs();
    let state = app.state::<AppState>();
    let mut doc = state.doc.write().await;
    let mut next_id = doc.next_id;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let entity = find_entity_mut(&mut project.library, entity_id)?;
    let sheet = embedded_reference_sheet_or_create_mut(entity)?;

    let mut new_variants = Vec::new();
    for (spec, images) in specs {
        for (candidate_index, image) in images.into_iter().enumerate() {
            let variant = build_variant_from_provider_image(
                mint_sheet_variant_id(&mut next_id),
                ts,
                image,
                &spec,
            );
            emit_sheet_event(
                app,
                SHEET_REQUEST_CANDIDATE_COMPLETE_EVENT,
                &SheetRequestCandidateCompletePayload {
                    request_id,
                    stream_index: spec.stream_index,
                    candidate_index: u8::try_from(candidate_index).unwrap_or(u8::MAX),
                    variant: variant.clone(),
                },
            );
            new_variants.push(variant);
        }
    }
    new_variants.append(&mut sheet.variants);
    sheet.variants = new_variants;
    entity.updated_at = ts;
    let updated = entity.clone();
    doc.next_id = next_id;
    doc.dirty = true;
    drop(doc);

    state.anchor_cache.remove(&entity_id.get());
    emit_sheet_event(
        app,
        SHEET_REQUEST_COMPLETE_EVENT,
        &SheetRequestCompletePayload {
            request_id,
            entity_id,
            sprite: updated.clone(),
            total_cost_usd: 0.0,
        },
    );
    Ok(updated)
}

fn spawn_sheet_provider_job(
    app: AppHandle,
    request_id: RequestId,
    entity_id: EntityId,
    token: CancellationToken,
    lease: SheetRequestLease,
    style_notes: String,
    spec: VariantCommitSpec,
) {
    tauri::async_runtime::spawn(async move {
        let _lease = lease;
        let started = SystemTime::now();
        if token.is_cancelled() {
            emit_sheet_cancelled(&app, request_id);
            return;
        }
        let _permit = app
            .state::<AppState>()
            .reference_sheet_requests
            .acquire_sprite_permit(entity_id.get(), true, &token)
            .await;
        if token.is_cancelled() {
            emit_sheet_cancelled(&app, request_id);
            return;
        }
        emit_sheet_event(
            &app,
            SHEET_REQUEST_PROGRESS_EVENT,
            &SheetRequestProgressPayload {
                request_id,
                stream_index: spec.stream_index,
                candidate_index: 0,
                partial_index: 0,
                partial_image: None,
                elapsed_ms: 0,
            },
        );
        let runtime = app.state::<AppState>().verb_runtime.clone();
        let result = invoke_provider_images(
            app.clone(),
            request_id,
            spec.stream_index,
            started,
            runtime,
            &spec.provider,
            &style_notes,
            token.clone(),
        )
        .await;
        if token.is_cancelled() {
            emit_sheet_cancelled(&app, request_id);
            return;
        }
        match result {
            Ok(images) => {
                let elapsed_ms = started.elapsed().map_or(0, |elapsed| {
                    u32::try_from(elapsed.as_millis()).unwrap_or(u32::MAX)
                });
                emit_sheet_event(
                    &app,
                    SHEET_REQUEST_PROGRESS_EVENT,
                    &SheetRequestProgressPayload {
                        request_id,
                        stream_index: spec.stream_index,
                        candidate_index: 0,
                        partial_index: 1,
                        partial_image: None,
                        elapsed_ms,
                    },
                );
                if let Err(err) =
                    commit_provider_variants(&app, request_id, entity_id, vec![(spec, images)])
                        .await
                {
                    emit_sheet_error(&app, request_id, err);
                }
            }
            Err(err) => emit_sheet_error(&app, request_id, err),
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn spawn_regional_sheet_job(
    app: AppHandle,
    request_id: RequestId,
    entity_id: EntityId,
    token: CancellationToken,
    lease: SheetRequestLease,
    style_notes: String,
    source: SheetVariant,
    spec: VariantCommitSpec,
    regions: Vec<pixhaus_core::project::RegionDefinition>,
) {
    tauri::async_runtime::spawn(async move {
        let _lease = lease;
        let started = SystemTime::now();
        if token.is_cancelled() {
            emit_sheet_cancelled(&app, request_id);
            return;
        }
        let _permit = app
            .state::<AppState>()
            .reference_sheet_requests
            .acquire_sprite_permit(entity_id.get(), true, &token)
            .await;
        if token.is_cancelled() {
            emit_sheet_cancelled(&app, request_id);
            return;
        }
        let runtime = app.state::<AppState>().verb_runtime.clone();
        let result = invoke_regional_refinement(
            app.clone(),
            request_id,
            spec.stream_index,
            started,
            runtime,
            &source,
            &spec.provider,
            &regions,
            &style_notes,
            token.clone(),
        )
        .await;
        if token.is_cancelled() {
            emit_sheet_cancelled(&app, request_id);
            return;
        }
        match result {
            Ok(images) => {
                if let Err(err) =
                    commit_provider_variants(&app, request_id, entity_id, vec![(spec, images)])
                        .await
                {
                    emit_sheet_error(&app, request_id, err);
                }
            }
            Err(err) => emit_sheet_error(&app, request_id, err),
        }
    });
}

fn spawn_cross_model_job(
    app: AppHandle,
    request_id: RequestId,
    entity_id: EntityId,
    token: CancellationToken,
    lease: SheetRequestLease,
    style_notes: String,
    specs: Vec<VariantCommitSpec>,
) {
    tauri::async_runtime::spawn(async move {
        let _lease = lease;
        let started = SystemTime::now();
        let runtime = app.state::<AppState>().verb_runtime.clone();
        let mut tasks = tokio::task::JoinSet::new();
        for spec in specs {
            emit_sheet_event(
                &app,
                SHEET_REQUEST_PROGRESS_EVENT,
                &SheetRequestProgressPayload {
                    request_id,
                    stream_index: spec.stream_index,
                    candidate_index: 0,
                    partial_index: 0,
                    partial_image: None,
                    elapsed_ms: 0,
                },
            );
            let runtime = runtime.clone();
            let style_notes = style_notes.clone();
            let token = token.clone();
            let app_for_task = app.clone();
            tasks.spawn(async move {
                let result = invoke_provider_images(
                    app_for_task,
                    request_id,
                    spec.stream_index,
                    started,
                    runtime,
                    &spec.provider,
                    &style_notes,
                    token,
                )
                .await;
                (spec, result)
            });
        }
        let mut completed = Vec::new();
        while let Some(joined) = tasks.join_next().await {
            if token.is_cancelled() {
                tasks.abort_all();
                emit_sheet_cancelled(&app, request_id);
                return;
            }
            let (spec, result) = match joined {
                Ok(result) => result,
                Err(err) => {
                    emit_sheet_error(
                        &app,
                        request_id,
                        AppCommandError::VerbError {
                            message: format!("cross-model stream task failed: {err}"),
                        },
                    );
                    continue;
                }
            };
            match result {
                Ok(images) => completed.push((spec, images)),
                Err(err) => {
                    emit_sheet_error(&app, request_id, err);
                }
            }
        }
        if completed.is_empty() {
            emit_sheet_error(
                &app,
                request_id,
                AppCommandError::VerbError {
                    message: "cross-model grid produced no successful candidates".into(),
                },
            );
            return;
        }
        if let Err(err) = commit_provider_variants(&app, request_id, entity_id, completed).await {
            emit_sheet_error(&app, request_id, err);
        }
    });
}

/// Built-in reference-sheet templates for the UI.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_list_reference_sheet_templates()
-> CommandResult<Vec<ReferenceSheetTemplateDefinition>> {
    Ok(built_in_reference_sheet_templates())
}

/// Cancels a reference-sheet request if it is still in flight.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_cancel_reference_sheet_request(
    request_id: RequestId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    if state.reference_sheet_requests.cancel(request_id) {
        Ok(())
    } else {
        Err(AppCommandError::NotFound {
            entity: "reference sheet request".into(),
            id: request_id,
        })
    }
}

/// Refines a variant by creating one or more derived draft variants.
#[tauri::command(async, rename_all = "snake_case")]
#[allow(clippy::too_many_lines)]
pub async fn library_refine_reference_sheet_variant(
    args: LibraryRefineReferenceSheetVariantArgs,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<RequestId> {
    if args.prompt.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "refinement prompt must not be empty".into(),
        });
    }
    let (source, style_notes, model, lora_asset) = {
        let doc = state.doc.read().await;
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let entity = find_entity(&project.library, args.entity_id)?;
        let sheet = match &entity.content {
            EntityContent::Sprites {
                reference_sheet: Some(sheet),
                ..
            } => sheet.as_ref(),
            _ => {
                return Err(AppCommandError::Validation {
                    detail: "entity has no sprite reference sheet".into(),
                });
            }
        };
        let source = find_variant(sheet, args.parent_variant_id)
            .cloned()
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "sheet variant".into(),
                id: u64::from(args.parent_variant_id.get()),
            })?;
        let operation = match &args.refinement {
            RefinementKind::Masked { .. } => OperationKind::MaskedRefinement,
            RefinementKind::PromptOnly => OperationKind::PromptOnlyRefinement,
            RefinementKind::Regional { .. } => OperationKind::RegionalRefinement,
        };
        let lora_asset = find_lora_asset(project, source.applied_lora);
        (
            source,
            project.library.ai.style_notes.clone(),
            configured_execution_model(selected_model(args.model, operation, project), &state),
            lora_asset,
        )
    };
    let operation = match &args.refinement {
        RefinementKind::Masked { .. } => OperationKind::MaskedRefinement,
        RefinementKind::PromptOnly => OperationKind::PromptOnlyRefinement,
        RefinementKind::Regional { .. } => OperationKind::RegionalRefinement,
    };
    let mask = match &args.refinement {
        RefinementKind::Masked { mask_png } => Some(mask_png.clone()),
        RefinementKind::PromptOnly | RefinementKind::Regional { .. } => None,
    };
    let regional_regions = match &args.refinement {
        RefinementKind::Regional { regions } => Some(regions.clone()),
        RefinementKind::Masked { .. } | RefinementKind::PromptOnly => None,
    };
    let (request_id, token, lease) =
        start_sheet_request_with_lease(&state, args.entity_id, true, None)?;
    let mut references = source.references.clone();
    references.extend(args.additional_references.clone());
    let spec = VariantCommitSpec {
        provider: SheetProviderRequest {
            operation,
            model,
            quality: args.quality,
            prompt: args.prompt,
            template: source.template,
            width: source.width,
            height: source.height,
            chroma_color: source.chroma_color,
            references,
            source_image: Some(source.image.clone()),
            mask,
            candidate_count: args.candidate_count.clamp(1, 4),
            applied_lora: lora_asset,
            lora_weight: source.lora_weight,
            real_world_grounding: source.real_world_grounding,
        },
        build: VariantBuildSpec {
            origin: VariantOrigin::Refinement,
            parent_variant_id: Some(source.id),
            refinement: Some(args.refinement),
            promotion: false,
            chat_transcript: None,
            applied_lora: source.applied_lora,
            lora_weight: source.lora_weight,
            real_world_grounding: source.real_world_grounding,
        },
        stream_index: 0,
    };
    if let Some(regions) = regional_regions {
        spawn_regional_sheet_job(
            app,
            request_id,
            args.entity_id,
            token,
            lease,
            style_notes,
            source,
            spec,
            regions,
        );
    } else {
        spawn_sheet_provider_job(
            app,
            request_id,
            args.entity_id,
            token,
            lease,
            style_notes,
            spec,
        );
    }
    Ok(request_id)
}

/// Adds a conversational edit turn and persists the resulting draft variant.
#[tauri::command(async, rename_all = "snake_case")]
#[allow(clippy::too_many_lines)]
pub async fn library_submit_chat_turn(
    args: LibrarySubmitChatTurnArgs,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<RequestId> {
    if args.user_message.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "chat message must not be empty".into(),
        });
    }
    let (source, style_notes, model, lora_asset) = {
        let doc = state.doc.read().await;
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let entity = find_entity(&project.library, args.entity_id)?;
        let sheet = match &entity.content {
            EntityContent::Sprites {
                reference_sheet: Some(sheet),
                ..
            } => sheet.as_ref(),
            _ => {
                return Err(AppCommandError::Validation {
                    detail: "entity has no sprite reference sheet".into(),
                });
            }
        };
        let source = find_variant(sheet, args.variant_id)
            .cloned()
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "sheet variant".into(),
                id: u64::from(args.variant_id.get()),
            })?;
        let lora_asset = find_lora_asset(project, source.applied_lora);
        (
            source,
            project.library.ai.style_notes.clone(),
            configured_execution_model(
                selected_model(args.model, OperationKind::ChatTurn, project),
                &state,
            ),
            lora_asset,
        )
    };
    let (request_id, token, lease) =
        start_sheet_request_with_lease(&state, args.entity_id, true, Some(args.variant_id))?;
    let mut transcript = source.chat_transcript.clone().unwrap_or_default();
    transcript.turns.push(ChatTurn {
        timestamp: now_secs(),
        user_message: args.user_message.clone(),
        mask: args.mask.clone(),
        // The committed provider variant receives its durable id after
        // completion. The final transcript is patched with that id during
        // the commit helper.
        resulting_variant_id: SheetVariantId::new(0),
    });
    let refinement = args
        .mask
        .clone()
        .map(|mask_png| RefinementKind::Masked { mask_png });
    let spec = VariantCommitSpec {
        provider: SheetProviderRequest {
            operation: OperationKind::ChatTurn,
            model,
            quality: Quality::Medium,
            prompt: args.user_message,
            template: source.template,
            width: source.width,
            height: source.height,
            chroma_color: source.chroma_color,
            references: source.references.clone(),
            source_image: Some(source.image.clone()),
            mask: args.mask,
            candidate_count: 1,
            applied_lora: lora_asset,
            lora_weight: source.lora_weight,
            real_world_grounding: source.real_world_grounding,
        },
        build: VariantBuildSpec {
            origin: VariantOrigin::ChatTurn,
            parent_variant_id: Some(source.id),
            refinement,
            promotion: false,
            chat_transcript: Some(transcript),
            applied_lora: source.applied_lora,
            lora_weight: source.lora_weight,
            real_world_grounding: source.real_world_grounding,
        },
        stream_index: 0,
    };
    spawn_sheet_provider_job(
        app,
        request_id,
        args.entity_id,
        token,
        lease,
        style_notes,
        spec,
    );
    Ok(request_id)
}

/// Creates a high-quality promoted draft derived from a source variant.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_promote_variant_to_final(
    args: LibraryPromoteVariantToFinalArgs,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<RequestId> {
    let (source, style_notes, model, lora_asset) = {
        let doc = state.doc.read().await;
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let entity = find_entity(&project.library, args.entity_id)?;
        let sheet = match &entity.content {
            EntityContent::Sprites {
                reference_sheet: Some(sheet),
                ..
            } => sheet.as_ref(),
            _ => {
                return Err(AppCommandError::Validation {
                    detail: "entity has no sprite reference sheet".into(),
                });
            }
        };
        let source = find_variant(sheet, args.source_variant_id)
            .cloned()
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "sheet variant".into(),
                id: u64::from(args.source_variant_id.get()),
            })?;
        let lora_asset = find_lora_asset(project, source.applied_lora);
        (
            source,
            project.library.ai.style_notes.clone(),
            configured_execution_model(
                selected_model(args.target_model, OperationKind::Promotion, project),
                &state,
            ),
            lora_asset,
        )
    };
    let (request_id, token, lease) =
        start_sheet_request_with_lease(&state, args.entity_id, true, None)?;
    let spec = VariantCommitSpec {
        provider: SheetProviderRequest {
            operation: OperationKind::Promotion,
            model,
            quality: args.target_quality,
            prompt: source.user_prompt.clone(),
            template: source.template,
            width: source.width,
            height: source.height,
            chroma_color: source.chroma_color,
            references: source.references.clone(),
            source_image: Some(source.image.clone()),
            mask: None,
            candidate_count: 1,
            applied_lora: lora_asset,
            lora_weight: source.lora_weight,
            real_world_grounding: source.real_world_grounding,
        },
        build: VariantBuildSpec {
            origin: VariantOrigin::Promotion,
            parent_variant_id: Some(source.id),
            refinement: None,
            promotion: true,
            chat_transcript: source.chat_transcript.clone(),
            applied_lora: source.applied_lora,
            lora_weight: source.lora_weight,
            real_world_grounding: source.real_world_grounding,
        },
        stream_index: 0,
    };
    spawn_sheet_provider_job(
        app,
        request_id,
        args.entity_id,
        token,
        lease,
        style_notes,
        spec,
    );
    Ok(request_id)
}

/// Creates comparison-grid draft placeholders for each model/quality pair.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_start_cross_model_grid(
    args: LibraryStartCrossModelGridArgs,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<RequestId> {
    if args.prompt.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "cross-model prompt must not be empty".into(),
        });
    }
    if !(2..=4).contains(&args.combinations.len()) {
        return Err(AppCommandError::Validation {
            detail: "cross-model grid requires 2 to 4 model combinations".into(),
        });
    }
    let style_notes = {
        let doc = state.doc.read().await;
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let entity = find_entity(&project.library, args.entity_id)?;
        if !matches!(entity.content, EntityContent::Sprites { .. }) {
            return Err(AppCommandError::Validation {
                detail: format!(
                    "entity {} is not a sprite entity; reference sheets belong to sprites",
                    args.entity_id.get()
                ),
            });
        }
        project.library.ai.style_notes.clone()
    };
    let (request_id, token, lease) =
        start_sheet_request_with_lease(&state, args.entity_id, false, None)?;
    let per_combo = args.candidate_count_per_combo.clamp(1, 2);
    let specs = args
        .combinations
        .iter()
        .enumerate()
        .map(|(stream_index, combo)| VariantCommitSpec {
            provider: SheetProviderRequest {
                operation: OperationKind::CrossModelGrid,
                model: combo.model,
                quality: combo.quality,
                prompt: args.prompt.clone(),
                template: args.template,
                width: args.width,
                height: args.height,
                chroma_color: args.chroma_color,
                references: args.references.clone(),
                source_image: None,
                mask: None,
                candidate_count: per_combo,
                applied_lora: None,
                lora_weight: 1.0,
                real_world_grounding: false,
            },
            build: VariantBuildSpec {
                origin: VariantOrigin::CrossModelGrid,
                parent_variant_id: None,
                refinement: None,
                promotion: false,
                chat_transcript: None,
                applied_lora: None,
                lora_weight: 1.0,
                real_world_grounding: false,
            },
            stream_index: u8::try_from(stream_index).unwrap_or(u8::MAX),
        })
        .collect();
    spawn_cross_model_job(
        app,
        request_id,
        args.entity_id,
        token,
        lease,
        style_notes,
        specs,
    );
    Ok(request_id)
}

/// Stores an SVG vector export on a variant.
#[tauri::command(async, rename_all = "snake_case")]
#[allow(clippy::too_many_lines)]
pub async fn library_export_variant_as_vector(
    args: LibraryExportVariantAsVectorArgs,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<RequestId> {
    let source_image = {
        let doc = state.doc.read().await;
        let project = doc
            .project
            .as_ref()
            .ok_or(AppCommandError::NoActiveProject)?;
        let entity = find_entity(&project.library, args.entity_id)?;
        let sheet = match &entity.content {
            EntityContent::Sprites {
                reference_sheet: Some(sheet),
                ..
            } => sheet.as_ref(),
            _ => {
                return Err(AppCommandError::Validation {
                    detail: "entity has no sprite reference sheet".into(),
                });
            }
        };
        find_variant(sheet, args.variant_id)
            .map(|variant| variant.image.clone())
            .ok_or_else(|| AppCommandError::NotFound {
                entity: "sheet variant".into(),
                id: u64::from(args.variant_id.get()),
            })?
    };
    let (request_id, token, lease) =
        start_sheet_request_with_lease(&state, args.entity_id, true, None)?;
    tauri::async_runtime::spawn(async move {
        let _lease = lease;
        if token.is_cancelled() {
            emit_sheet_cancelled(&app, request_id);
            return;
        }
        let _permit = app
            .state::<AppState>()
            .reference_sheet_requests
            .acquire_sprite_permit(args.entity_id.get(), true, &token)
            .await;
        if token.is_cancelled() {
            emit_sheet_cancelled(&app, request_id);
            return;
        }
        let runtime = app.state::<AppState>().verb_runtime.clone();
        let result = invoke_vector_export(runtime, &source_image, token.clone()).await;
        if token.is_cancelled() {
            emit_sheet_cancelled(&app, request_id);
            return;
        }
        let svg = match result {
            Ok(svg) => svg,
            Err(err) => {
                emit_sheet_error(&app, request_id, err);
                return;
            }
        };
        let state = app.state::<AppState>();
        let updated = {
            let ts = now_secs();
            let mut doc = state.doc.write().await;
            let Some(project) = doc.project.as_mut() else {
                emit_sheet_error(&app, request_id, AppCommandError::NoActiveProject);
                return;
            };
            let Some(entity) = project
                .library
                .entities
                .iter_mut()
                .find(|e| e.id == args.entity_id)
            else {
                emit_sheet_error(
                    &app,
                    request_id,
                    AppCommandError::NotFound {
                        entity: "entity".into(),
                        id: u64::from(args.entity_id.get()),
                    },
                );
                return;
            };
            let sheet = match embedded_reference_sheet_mut(entity) {
                Ok(sheet) => sheet,
                Err(err) => {
                    emit_sheet_error(&app, request_id, err);
                    return;
                }
            };
            let Some(variant) = sheet
                .canonical
                .as_mut()
                .filter(|variant| variant.id == args.variant_id)
                .or_else(|| {
                    sheet
                        .variants
                        .iter_mut()
                        .find(|variant| variant.id == args.variant_id)
                })
            else {
                emit_sheet_error(
                    &app,
                    request_id,
                    AppCommandError::NotFound {
                        entity: "sheet variant".into(),
                        id: u64::from(args.variant_id.get()),
                    },
                );
                return;
            };
            variant.vector_image = Some(svg);
            entity.updated_at = ts;
            let updated = entity.clone();
            doc.dirty = true;
            updated
        };
        emit_sheet_event(
            &app,
            SHEET_REQUEST_COMPLETE_EVENT,
            &SheetRequestCompletePayload {
                request_id,
                entity_id: args.entity_id,
                sprite: updated,
                total_cost_usd: 0.0,
            },
        );
    });
    Ok(request_id)
}

/// Saves a reference image into the project asset library.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_save_reference_to_library(
    args: LibrarySaveReferenceToLibraryArgs,
    state: State<'_, AppState>,
) -> CommandResult<ReferenceAsset> {
    if args.image.bytes.is_empty() {
        return Err(AppCommandError::Validation {
            detail: "reference image bytes must not be empty".into(),
        });
    }
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let mut next_id = doc.next_id;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let asset = ReferenceAsset {
        id: mint_asset_id(&mut next_id),
        image: args.image,
        default_role: args.role,
        tags: args.tags,
        source_variant_id: None,
        created_at: ts,
    };
    project
        .library
        .ai
        .asset_library
        .references
        .push(asset.clone());
    doc.next_id = next_id;
    doc.dirty = true;
    Ok(asset)
}

/// Saves a variant image as a character card plus a reference asset.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_save_variant_as_character_card(
    args: LibrarySaveVariantCardArgs,
    state: State<'_, AppState>,
) -> CommandResult<CharacterCard> {
    if args.name.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "character card name must not be empty".into(),
        });
    }
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let mut next_id = doc.next_id;
    let ref_id = mint_asset_id(&mut next_id);
    let card_id = mint_asset_id(&mut next_id);
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let entity = find_entity_mut(&mut project.library, args.entity_id)?;
    let sheet = embedded_reference_sheet_mut(entity)?;
    let variant = find_variant(sheet, args.variant_id)
        .cloned()
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "sheet variant".into(),
            id: u64::from(args.variant_id.get()),
        })?;
    project
        .library
        .ai
        .asset_library
        .references
        .push(ReferenceAsset {
            id: ref_id,
            image: variant.image,
            default_role: ReferenceRole::Subject,
            tags: Vec::new(),
            source_variant_id: Some(args.variant_id),
            created_at: ts,
        });
    let card = CharacterCard {
        id: card_id,
        name: args.name,
        references: vec![ref_id],
        style_notes: args.style_notes,
        associated_lora: None,
        created_at: ts,
    };
    project
        .library
        .ai
        .asset_library
        .character_cards
        .push(card.clone());
    doc.next_id = next_id;
    doc.dirty = true;
    Ok(card)
}

/// Saves a variant image as a style swatch plus a reference asset.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_save_variant_as_style_swatch(
    args: LibrarySaveVariantCardArgs,
    state: State<'_, AppState>,
) -> CommandResult<StyleSwatch> {
    if args.name.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "style swatch name must not be empty".into(),
        });
    }
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let mut next_id = doc.next_id;
    let ref_id = mint_asset_id(&mut next_id);
    let swatch_id = mint_asset_id(&mut next_id);
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let entity = find_entity_mut(&mut project.library, args.entity_id)?;
    let sheet = embedded_reference_sheet_mut(entity)?;
    let variant = find_variant(sheet, args.variant_id)
        .cloned()
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "sheet variant".into(),
            id: u64::from(args.variant_id.get()),
        })?;
    project
        .library
        .ai
        .asset_library
        .references
        .push(ReferenceAsset {
            id: ref_id,
            image: variant.image,
            default_role: ReferenceRole::Style,
            tags: Vec::new(),
            source_variant_id: Some(args.variant_id),
            created_at: ts,
        });
    let swatch = StyleSwatch {
        id: swatch_id,
        name: args.name,
        references: vec![ref_id],
        style_notes: args.style_notes,
        associated_lora: None,
        created_at: ts,
    };
    project
        .library
        .ai
        .asset_library
        .style_swatches
        .push(swatch.clone());
    doc.next_id = next_id;
    doc.dirty = true;
    Ok(swatch)
}

/// Returns the project-scoped asset library.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_browse_assets(
    _filters: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> CommandResult<AssetLibrary> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    Ok(project.library.ai.asset_library.clone())
}

/// Gets a single asset by id.
#[tauri::command(async, rename_all = "snake_case")]
#[allow(clippy::disallowed_methods)]
pub async fn library_get_asset(
    asset_id: AssetId,
    state: State<'_, AppState>,
) -> CommandResult<serde_json::Value> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    let library = &project.library.ai.asset_library;
    if let Some(asset) = library.references.iter().find(|asset| asset.id == asset_id) {
        return Ok(serde_json::json!({ "kind": "reference", "value": asset }));
    }
    if let Some(asset) = library
        .character_cards
        .iter()
        .find(|asset| asset.id == asset_id)
    {
        return Ok(serde_json::json!({ "kind": "character_card", "value": asset }));
    }
    if let Some(asset) = library
        .style_swatches
        .iter()
        .find(|asset| asset.id == asset_id)
    {
        return Ok(serde_json::json!({ "kind": "style_swatch", "value": asset }));
    }
    if let Some(asset) = library.loras.iter().find(|asset| asset.id == asset_id) {
        return Ok(serde_json::json!({ "kind": "lora", "value": asset }));
    }
    Err(AppCommandError::NotFound {
        entity: "asset".into(),
        id: u64::from(asset_id.get()),
    })
}

/// Removes an asset from the project asset library.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_remove_asset(
    asset_id: AssetId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let library = &mut project.library.ai.asset_library;
    let before = (
        library.references.len(),
        library.character_cards.len(),
        library.style_swatches.len(),
        library.loras.len(),
    );
    library.references.retain(|asset| asset.id != asset_id);
    library.character_cards.retain(|asset| asset.id != asset_id);
    library.style_swatches.retain(|asset| asset.id != asset_id);
    library.loras.retain(|asset| asset.id != asset_id);
    let after = (
        library.references.len(),
        library.character_cards.len(),
        library.style_swatches.len(),
        library.loras.len(),
    );
    if before == after {
        return Err(AppCommandError::NotFound {
            entity: "asset".into(),
            id: u64::from(asset_id.get()),
        });
    }
    doc.dirty = true;
    Ok(())
}

/// Updates tags on a reference asset.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_update_asset_tags(
    args: LibraryUpdateAssetTagsArgs,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let Some(asset) = project
        .library
        .ai
        .asset_library
        .references
        .iter_mut()
        .find(|asset| asset.id == args.asset_id)
    else {
        return Err(AppCommandError::NotFound {
            entity: "reference asset".into(),
            id: u64::from(args.asset_id.get()),
        });
    };
    asset.tags = args.tags;
    doc.dirty = true;
    Ok(())
}

/// Starts a fal `LoRA` training job record.
#[tauri::command(async, rename_all = "snake_case")]
#[allow(clippy::too_many_lines)]
pub async fn library_train_lora(
    args: LibraryTrainLoraArgs,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<TrainingJobId> {
    if args.name.trim().is_empty() || args.trigger_word.trim().is_empty() {
        return Err(AppCommandError::Validation {
            detail: "LoRA name and trigger word must not be empty".into(),
        });
    }
    if args.training_data.len() < 10 {
        return Err(AppCommandError::Validation {
            detail: "LoRA training requires at least 10 saved reference assets".into(),
        });
    }
    let ts = now_secs();
    let mut doc = state.doc.write().await;
    let mut next_id = doc.next_id;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let available_refs: HashMap<AssetId, ReferenceAsset> = project
        .library
        .ai
        .asset_library
        .references
        .iter()
        .map(|asset| (asset.id, asset.clone()))
        .collect();
    if let Some(missing) = args
        .training_data
        .iter()
        .find(|asset_id| !available_refs.contains_key(asset_id))
    {
        return Err(AppCommandError::NotFound {
            entity: "reference asset".into(),
            id: u64::from(missing.get()),
        });
    }
    let training_assets = args
        .training_data
        .iter()
        .filter_map(|asset_id| available_refs.get(asset_id).cloned())
        .collect::<Vec<_>>();
    let id = TrainingJobId::new(next_id);
    next_id += 1;
    project.library.ai.training_jobs.push(TrainingJob {
        id,
        asset_name: args.name.clone(),
        kind: args.kind,
        target_model: args.target_model,
        trigger_word: args.trigger_word.clone(),
        training_data: args.training_data.clone(),
        fal_job_id: format!("pending-fal-{}", id.get()),
        status: TrainingStatus::Running,
        created_at: ts,
        completed_at: None,
        result_lora_id: None,
        error: None,
    });
    doc.next_id = next_id;
    doc.dirty = true;
    drop(doc);

    emit_training_event(
        &app,
        TRAINING_JOB_PROGRESS_EVENT,
        &TrainingJobProgressPayload {
            job_id: id,
            status: TrainingStatus::Running,
            message: "submitting fal LoRA training job".into(),
        },
    );
    let asset_name = args.name;
    let kind = args.kind;
    let target_model = args.target_model;
    let trigger_word = args.trigger_word;
    tauri::async_runtime::spawn(async move {
        let result = async {
            let archive = encode_reference_assets_zip(&training_assets)?;
            let backend =
                FalBackend::from_keychain().map_err(|err| AppCommandError::VerbError {
                    message: err.to_string(),
                })?;
            backend
                .train_lora_archive(
                    archive,
                    &trigger_word,
                    matches!(kind, LoraKind::Style),
                    CancellationToken::new(),
                )
                .await
                .map_err(|err| AppCommandError::VerbError {
                    message: err.to_string(),
                })
        }
        .await;
        let state = app.state::<AppState>();
        match result {
            Ok(training) => {
                let maybe_asset = {
                    let mut doc = state.doc.write().await;
                    let mut next_id = doc.next_id;
                    let Some(project) = doc.project.as_mut() else {
                        emit_training_event(
                            &app,
                            TRAINING_JOB_FAILED_EVENT,
                            &TrainingJobFailedPayload {
                                job_id: id,
                                error: AppCommandError::NoActiveProject,
                            },
                        );
                        return;
                    };
                    let Some(job) = project
                        .library
                        .ai
                        .training_jobs
                        .iter_mut()
                        .find(|job| job.id == id)
                    else {
                        emit_training_event(
                            &app,
                            TRAINING_JOB_FAILED_EVENT,
                            &TrainingJobFailedPayload {
                                job_id: id,
                                error: AppCommandError::NotFound {
                                    entity: "training job".into(),
                                    id: u64::from(id.get()),
                                },
                            },
                        );
                        return;
                    };
                    if job.status == TrainingStatus::Cancelled {
                        return;
                    }
                    let lora_id = mint_asset_id(&mut next_id);
                    let asset = LoraAsset {
                        id: lora_id,
                        name: asset_name.clone(),
                        kind,
                        trigger_word: trigger_word.clone(),
                        target_model,
                        fal_lora_url: training.lora_url,
                        training_data_thumbnails: training_assets
                            .iter()
                            .take(12)
                            .map(|asset| asset.image.clone())
                            .collect(),
                        created_at: now_secs(),
                    };
                    project.library.ai.asset_library.loras.push(asset.clone());
                    job.status = TrainingStatus::Completed;
                    job.completed_at = Some(now_secs());
                    job.result_lora_id = Some(lora_id);
                    doc.next_id = next_id;
                    doc.dirty = true;
                    Some(asset)
                };
                if let Some(asset) = maybe_asset {
                    emit_training_event(
                        &app,
                        TRAINING_JOB_COMPLETE_EVENT,
                        &TrainingJobCompletePayload {
                            job_id: id,
                            lora_asset: asset,
                        },
                    );
                }
            }
            Err(err) => {
                {
                    let mut doc = state.doc.write().await;
                    if let Some(project) = doc.project.as_mut()
                        && let Some(job) = project
                            .library
                            .ai
                            .training_jobs
                            .iter_mut()
                            .find(|job| job.id == id)
                    {
                        job.status = TrainingStatus::Failed;
                        job.completed_at = Some(now_secs());
                        job.error = Some(err.to_string());
                        doc.dirty = true;
                    }
                }
                emit_training_event(
                    &app,
                    TRAINING_JOB_FAILED_EVENT,
                    &TrainingJobFailedPayload {
                        job_id: id,
                        error: err,
                    },
                );
            }
        }
    });
    Ok(id)
}

/// Returns one `LoRA` training job.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_get_training_job_status(
    job_id: TrainingJobId,
    state: State<'_, AppState>,
) -> CommandResult<TrainingJob> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    project
        .library
        .ai
        .training_jobs
        .iter()
        .find(|job| job.id == job_id)
        .cloned()
        .ok_or_else(|| AppCommandError::NotFound {
            entity: "training job".into(),
            id: u64::from(job_id.get()),
        })
}

/// Lists all `LoRA` training jobs.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_list_training_jobs(
    state: State<'_, AppState>,
) -> CommandResult<Vec<TrainingJob>> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    Ok(project.library.ai.training_jobs.clone())
}

/// Cancels a queued/running `LoRA` training job record.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn library_cancel_training_job(
    job_id: TrainingJobId,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    let Some(job) = project
        .library
        .ai
        .training_jobs
        .iter_mut()
        .find(|job| job.id == job_id)
    else {
        return Err(AppCommandError::NotFound {
            entity: "training job".into(),
            id: u64::from(job_id.get()),
        });
    };
    job.status = TrainingStatus::Cancelled;
    job.completed_at = Some(now_secs());
    doc.dirty = true;
    Ok(())
}

/// Project style notes getter.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_get_style_notes(state: State<'_, AppState>) -> CommandResult<String> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    Ok(project.library.ai.style_notes.clone())
}

/// Project style notes setter.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_set_style_notes(
    notes: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.library.ai.style_notes = notes;
    doc.dirty = true;
    Ok(())
}

/// Stores a per-operation model preference.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_set_operation_model_pref(
    args: ProjectSetOperationModelPrefArgs,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project
        .library
        .ai
        .per_operation_model_prefs
        .insert(args.operation, args.model);
    doc.dirty = true;
    Ok(())
}

/// Clears all per-operation model preferences.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_clear_operation_model_prefs(state: State<'_, AppState>) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.library.ai.per_operation_model_prefs.clear();
    doc.dirty = true;
    Ok(())
}

/// Sets the default chroma color.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_set_default_chroma(
    color: Rgba,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.library.ai.default_chroma = color;
    doc.dirty = true;
    Ok(())
}

/// Gets the default quality.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_get_default_quality(state: State<'_, AppState>) -> CommandResult<Quality> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    Ok(project.library.ai.default_quality)
}

/// Sets the default quality.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_set_default_quality(
    quality: Quality,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.library.ai.default_quality = quality;
    doc.dirty = true;
    Ok(())
}

/// Gets the default candidate count.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_get_default_candidate_count(state: State<'_, AppState>) -> CommandResult<u8> {
    let doc = state.doc.read().await;
    let project = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?;
    Ok(project.library.ai.default_candidate_count)
}

/// Sets the default candidate count.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn project_set_default_candidate_count(
    n: u8,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    if !(1..=4).contains(&n) {
        return Err(AppCommandError::Validation {
            detail: "default candidate count must be between 1 and 4".into(),
        });
    }
    let mut doc = state.doc.write().await;
    let project = doc
        .project
        .as_mut()
        .ok_or(AppCommandError::NoActiveProject)?;
    project.library.ai.default_candidate_count = n;
    doc.dirty = true;
    Ok(())
}
