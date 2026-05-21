// Library entity, group, tag, search, and sheet-variant commands.
//
// Covers the B9.2 entity/group/tag catalog (CRUD, states, active target,
// search) plus reference-sheet variant operations embedded on sprite
// entities (approve, update asset info, delete history entry). Commands
// that touch pixel data live in canvas.ts; palette commands live in palette.ts.
//
// Composition library commands (spec sections 12, 15-17) are appended at the
// bottom of this file.

import { invoke } from "../ipc";
import type {
  ActiveTarget,
  AssetId,
  AssetLibrary,
  AssetInfo,
  CharacterCard,
  ColorMode,
  CompositionKind,
  CompositionLibrary,
  ConflictPolicy,
  Entity,
  EntityGroup,
  EntityId,
  EntityKind,
  GroupId,
  ImportReport,
  LoraKind,
  ModelId,
  NamedSprite,
  OperationKind,
  PaletteEntry,
  PromptId,
  PromptTemplate,
  Quality,
  ReferenceAsset,
  ReferenceImage,
  ReferenceRole,
  ReferenceSheetTemplateDefinition,
  ReferenceSheetTemplateId,
  ReferenceSlot,
  RefinementKind,
  ResolvedVariable,
  Rgba,
  SheetComposition,
  SheetVariantId,
  Structure,
  StructureId,
  Style,
  StyleId,
  StyleSwatch,
  TagDefinition,
  TagId,
  TrainingJob,
  TrainingJobId,
} from "../types";

// ── arg types ─────────────────────────────────────────────────────────────────

export type LibraryCreateEntityArgs = {
  kind: EntityKind;
  name: string;
  group_id?: GroupId | null;
  // Custom-kind
  initial_states?: string[];
  canvas_width?: number;
  canvas_height?: number;
  color_mode?: ColorMode;
  // Tileset-kind
  tile_width?: number;
  tile_height?: number;
  // Tilemap-kind
  scene_width?: number;
  scene_height?: number;
  // Custom-kind optional embedded reference sheet
  reference_bytes?: number[];
  reference_mime?: string;
};

export type LibraryAddStateArgs = {
  entity_id: EntityId;
  state_name: string;
  canvas_width?: number;
  canvas_height?: number;
  color_mode?: ColorMode;
};

export type ReferenceSheetTemplate = "character" | "item" | "tileset" | "custom";
export type ImageQuality = "auto" | "low" | "medium" | "high";

export type LibraryGenerateReferenceSheetArgs = {
  entity_id: EntityId;
  prompt: string;
  template: ReferenceSheetTemplateId;
  width: number;
  height: number;
  chroma_color: Rgba;
  quality: Quality;
  candidate_count: number;
  model?: ModelId | null;
  references?: ReferenceSlot[];
  real_world_grounding?: boolean;
  applied_lora?: AssetId | null;
  lora_weight?: number;
  structure_id?: StructureId | undefined;
  style_id?: StyleId | undefined;
  prompt_id?: PromptId | undefined;
  variable_values?: Record<string, string> | undefined;
  inline_text?: string | undefined;
  inline_negatives?: string | undefined;
};

export type LibraryImportReferenceSheetArgs = {
  entity_id: EntityId;
  bytes: number[];
  mime?: string | null;
};

export type RequestId = number;

export type ModelQualityPair = {
  model: ModelId;
  quality: Quality;
};

export type LibraryRefineReferenceSheetVariantArgs = {
  entity_id: EntityId;
  parent_variant_id: SheetVariantId;
  refinement: RefinementKind;
  prompt: string;
  quality: Quality;
  candidate_count: number;
  model?: ModelId | null;
  additional_references?: ReferenceSlot[];
};

export type LibrarySubmitChatTurnArgs = {
  entity_id: EntityId;
  variant_id: SheetVariantId;
  user_message: string;
  mask?: ReferenceImage | null;
  model?: ModelId | null;
};

export type LibraryPromoteVariantToFinalArgs = {
  entity_id: EntityId;
  source_variant_id: SheetVariantId;
  target_quality: Quality;
  target_model?: ModelId | null;
};

export type LibraryStartCrossModelGridArgs = {
  entity_id: EntityId;
  prompt: string;
  template: ReferenceSheetTemplateId;
  width: number;
  height: number;
  chroma_color: Rgba;
  references?: ReferenceSlot[];
  combinations: ModelQualityPair[];
  candidate_count_per_combo: number;
};

export type LibrarySaveReferenceToLibraryArgs = {
  image: ReferenceImage;
  role?: ReferenceRole;
  tags?: string[];
};

export type LibrarySaveVariantCardArgs = {
  entity_id: EntityId;
  variant_id: SheetVariantId;
  name: string;
  style_notes?: string;
};

export type LibraryTrainLoraArgs = {
  name: string;
  kind: LoraKind;
  target_model: ModelId;
  trigger_word: string;
  training_data: AssetId[];
};

export type ProjectSetOperationModelPrefArgs = {
  operation: OperationKind;
  model: ModelId;
};

export type LibraryCreateGroupArgs = {
  name: string;
  parent_id?: GroupId | null;
};

export type LibraryDeleteGroupArgs = {
  group_id: GroupId;
  keep_entities: boolean;
};

export type LibraryAddTagArgs = {
  name: string;
  color?: Rgba | null;
};

export type LibrarySearchArgs = {
  query: string;
  kind_filter?: EntityKind | null;
  group_filter?: GroupId | null;
  tag_filter?: TagId | null;
};

// ── entity commands ───────────────────────────────────────────────────────────

export function libraryCreateEntity(args: LibraryCreateEntityArgs): Promise<Entity> {
  return invoke<Entity>("library_create_entity", { args });
}

export function libraryDeleteEntity(entity_id: EntityId): Promise<void> {
  return invoke<void>("library_delete_entity", { entity_id });
}

export function libraryRenameEntity(entity_id: EntityId, name: string): Promise<Entity> {
  return invoke<Entity>("library_rename_entity", { entity_id, name });
}

export function libraryGetEntity(entity_id: EntityId): Promise<Entity> {
  return invoke<Entity>("library_get_entity", { entity_id });
}

export function libraryListEntities(
  kind?: EntityKind | null,
  group_id?: GroupId | null,
  tag_id?: TagId | null,
): Promise<Entity[]> {
  return invoke<Entity[]>("library_list_entities", { kind, group_id, tag_id });
}

export function libraryReorderEntities(entity_id: EntityId, new_index: number): Promise<void> {
  return invoke<void>("library_reorder_entities", { entity_id, new_index });
}

export function libraryMoveEntityToGroup(
  entity_id: EntityId,
  group_id: GroupId | null,
): Promise<void> {
  return invoke<void>("library_move_entity_to_group", { entity_id, group_id });
}

export function libraryTagEntity(entity_id: EntityId, tag_id: TagId): Promise<void> {
  return invoke<void>("library_tag_entity", { entity_id, tag_id });
}

export function libraryUntagEntity(entity_id: EntityId, tag_id: TagId): Promise<void> {
  return invoke<void>("library_untag_entity", { entity_id, tag_id });
}

// ── state commands ────────────────────────────────────────────────────────────

export function libraryAddState(args: LibraryAddStateArgs): Promise<NamedSprite> {
  return invoke<NamedSprite>("library_add_state", { args });
}

export function libraryDeleteState(entity_id: EntityId, state_id: number): Promise<void> {
  return invoke<void>("library_delete_state", { entity_id, state_id });
}

export function libraryRenameState(
  entity_id: EntityId,
  state_id: number,
  state_name: string,
): Promise<void> {
  return invoke<void>("library_rename_state", { entity_id, state_id, state_name });
}

// ── active target commands ────────────────────────────────────────────────────

export function librarySetActiveTarget(target: ActiveTarget): Promise<void> {
  return invoke<void>("library_set_active_target", { target });
}

export function libraryGetActiveTarget(): Promise<ActiveTarget> {
  return invoke<ActiveTarget>("library_get_active_target");
}

// ── group commands ────────────────────────────────────────────────────────────

export function libraryCreateGroup(args: LibraryCreateGroupArgs): Promise<EntityGroup> {
  return invoke<EntityGroup>("library_create_group", { args });
}

export function libraryDeleteGroup(args: LibraryDeleteGroupArgs): Promise<void> {
  return invoke<void>("library_delete_group", { args });
}

export function libraryRenameGroup(group_id: GroupId, name: string): Promise<void> {
  return invoke<void>("library_rename_group", { group_id, name });
}

export function librarySetGroupParent(group_id: GroupId, parent_id: GroupId | null): Promise<void> {
  return invoke<void>("library_set_group_parent", { group_id, parent_id });
}

export function libraryListGroups(): Promise<EntityGroup[]> {
  return invoke<EntityGroup[]>("library_list_groups");
}

// ── tag commands ──────────────────────────────────────────────────────────────

export function libraryAddTag(args: LibraryAddTagArgs): Promise<TagDefinition> {
  return invoke<TagDefinition>("library_add_tag", { args });
}

export function libraryDeleteTag(tag_id: TagId): Promise<void> {
  return invoke<void>("library_delete_tag", { tag_id });
}

export function libraryRenameTag(tag_id: TagId, name: string): Promise<void> {
  return invoke<void>("library_rename_tag", { tag_id, name });
}

export function libraryListTags(): Promise<TagDefinition[]> {
  return invoke<TagDefinition[]>("library_list_tags");
}

// ── search ────────────────────────────────────────────────────────────────────

export function librarySearch(args: LibrarySearchArgs): Promise<Entity[]> {
  return invoke<Entity[]>("library_search", { args });
}

// ── sheet variant commands ────────────────────────────────────────────────────

/**
 * Promotes a history variant to canonical for a sprite entity's embedded
 * reference sheet.
 *
 * The previous canonical moves to the front of `history`. Returns the
 * updated entity so callers can refresh their local state without a
 * separate `libraryGetEntity` round-trip.
 */
export function libraryApproveSheetVariant(
  entity_id: EntityId,
  variant_id: SheetVariantId,
): Promise<Entity> {
  return invoke<Entity>("library_approve_sheet_variant", {
    args: { entity_id, variant_id },
  });
}

/**
 * Overwrites the asset info (name, age, species, personality notes) for a
 * sprite entity's embedded reference sheet.
 */
export function libraryUpdateAssetInfo(entity_id: EntityId, info: AssetInfo): Promise<void> {
  return invoke<void>("library_update_asset_info", { args: { entity_id, info } });
}

/**
 * Deletes a history variant from a sprite entity's embedded reference sheet.
 *
 * Rejects with `Validation` if `variant_id` is the current canonical —
 * approve a replacement first.
 */
export function libraryDeleteSheetVariant(
  entity_id: EntityId,
  variant_id: SheetVariantId,
): Promise<void> {
  return invoke<void>("library_delete_sheet_variant", { entity_id, variant_id });
}

/**
 * Generates unapproved reference-sheet draft candidates on a sprite entity.
 */
export function libraryGenerateReferenceSheet(
  args: LibraryGenerateReferenceSheetArgs,
): Promise<RequestId> {
  return invoke<RequestId>("library_generate_reference_sheet", { args });
}

/**
 * Imports a reference-sheet image as an unapproved draft candidate.
 */
export function libraryImportReferenceSheet(
  args: LibraryImportReferenceSheetArgs,
): Promise<Entity> {
  return invoke<Entity>("library_import_reference_sheet", { args });
}

/**
 * Removes a non-canonical reference-sheet draft/history variant and returns
 * the updated entity.
 */
export function libraryRemoveReferenceSheetVariant(
  entity_id: EntityId,
  variant_id: SheetVariantId,
): Promise<Entity> {
  return invoke<Entity>("library_remove_reference_sheet_variant", {
    args: { entity_id, variant_id },
  });
}

export function libraryListReferenceSheetTemplates(): Promise<ReferenceSheetTemplateDefinition[]> {
  return invoke<ReferenceSheetTemplateDefinition[]>("library_list_reference_sheet_templates");
}

export function libraryCancelReferenceSheetRequest(request_id: RequestId): Promise<void> {
  return invoke<void>("library_cancel_reference_sheet_request", { request_id });
}

export function libraryRefineReferenceSheetVariant(
  args: LibraryRefineReferenceSheetVariantArgs,
): Promise<RequestId> {
  return invoke<RequestId>("library_refine_reference_sheet_variant", { args });
}

export function librarySubmitChatTurn(args: LibrarySubmitChatTurnArgs): Promise<RequestId> {
  return invoke<RequestId>("library_submit_chat_turn", { args });
}

export function libraryPromoteVariantToFinal(
  args: LibraryPromoteVariantToFinalArgs,
): Promise<RequestId> {
  return invoke<RequestId>("library_promote_variant_to_final", { args });
}

export function libraryStartCrossModelGrid(
  args: LibraryStartCrossModelGridArgs,
): Promise<RequestId> {
  return invoke<RequestId>("library_start_cross_model_grid", { args });
}

export function libraryExportVariantAsVector(
  entity_id: EntityId,
  variant_id: SheetVariantId,
): Promise<RequestId> {
  return invoke<RequestId>("library_export_variant_as_vector", {
    args: { entity_id, variant_id },
  });
}

export function librarySaveReferenceToLibrary(
  args: LibrarySaveReferenceToLibraryArgs,
): Promise<ReferenceAsset> {
  return invoke<ReferenceAsset>("library_save_reference_to_library", { args });
}

export function librarySaveVariantAsCharacterCard(
  args: LibrarySaveVariantCardArgs,
): Promise<CharacterCard> {
  return invoke<CharacterCard>("library_save_variant_as_character_card", { args });
}

export function librarySaveVariantAsStyleSwatch(
  args: LibrarySaveVariantCardArgs,
): Promise<StyleSwatch> {
  return invoke<StyleSwatch>("library_save_variant_as_style_swatch", { args });
}

export function libraryBrowseAssets(filters?: unknown): Promise<AssetLibrary> {
  return invoke<AssetLibrary>("library_browse_assets", { filters: filters ?? null });
}

export function libraryGetAsset(asset_id: AssetId): Promise<unknown> {
  return invoke<unknown>("library_get_asset", { asset_id });
}

export function libraryRemoveAsset(asset_id: AssetId): Promise<void> {
  return invoke<void>("library_remove_asset", { asset_id });
}

export function libraryUpdateAssetTags(asset_id: AssetId, tags: string[]): Promise<void> {
  return invoke<void>("library_update_asset_tags", { args: { asset_id, tags } });
}

export function libraryTrainLora(args: LibraryTrainLoraArgs): Promise<TrainingJobId> {
  return invoke<TrainingJobId>("library_train_lora", { args });
}

export function libraryGetTrainingJobStatus(job_id: TrainingJobId): Promise<TrainingJob> {
  return invoke<TrainingJob>("library_get_training_job_status", { job_id });
}

export function libraryListTrainingJobs(): Promise<TrainingJob[]> {
  return invoke<TrainingJob[]>("library_list_training_jobs");
}

export function libraryCancelTrainingJob(job_id: TrainingJobId): Promise<void> {
  return invoke<void>("library_cancel_training_job", { job_id });
}

export function projectGetStyleNotes(): Promise<string> {
  return invoke<string>("project_get_style_notes");
}

export function projectSetStyleNotes(notes: string): Promise<void> {
  return invoke<void>("project_set_style_notes", { notes });
}

export function projectSetOperationModelPref(
  args: ProjectSetOperationModelPrefArgs,
): Promise<void> {
  return invoke<void>("project_set_operation_model_pref", { args });
}

export function projectClearOperationModelPrefs(): Promise<void> {
  return invoke<void>("project_clear_operation_model_prefs");
}

export function projectSetDefaultChroma(color: Rgba): Promise<void> {
  return invoke<void>("project_set_default_chroma", { color });
}

export function projectSetDefaultQuality(quality: Quality): Promise<void> {
  return invoke<void>("project_set_default_quality", { quality });
}

export function projectGetDefaultQuality(): Promise<Quality> {
  return invoke<Quality>("project_get_default_quality");
}

export function projectSetDefaultCandidateCount(n: number): Promise<void> {
  return invoke<void>("project_set_default_candidate_count", { n });
}

export function projectGetDefaultCandidateCount(): Promise<number> {
  return invoke<number>("project_get_default_candidate_count");
}

// ── B9.4: library AI hooks ────────────────────────────────────────────────────

/**
 * Runs the Critique verb in `LibraryAutoTag` mode against the given entity.
 *
 * Resolves with the `TagDefinition`s the VLM suggested; the same list also
 * lives on `entity.ai.suggested_tags` server-side, so the caller can either
 * use the return value directly or refresh the entity via `libraryGetEntity`.
 * The suggestions are persisted but pending — `libraryAcceptSuggestedTag`
 * promotes one to `entity.tags`, `libraryRejectSuggestedTag` drops it.
 *
 * Suitable only for `Custom` entities. The verb may need a configured VLM
 * backend; failure propagates as a rejected promise.
 */
export function libraryAutoTagEntity(entity_id: EntityId): Promise<TagDefinition[]> {
  return invoke<TagDefinition[]>("library_auto_tag_entity", { entity_id });
}

/**
 * Promotes a pending suggested tag to a confirmed tag on the entity.
 *
 * Rejects with `Validation` if `tag_id` is not in `entity.ai.suggested_tags`;
 * the wrapper never silently corrupts entity state by adding an arbitrary tag.
 */
export function libraryAcceptSuggestedTag(entity_id: EntityId, tag_id: TagId): Promise<void> {
  return invoke<void>("library_accept_suggested_tag", { entity_id, tag_id });
}

/**
 * Drops a pending suggested tag without confirming it.
 *
 * Idempotent: rejecting a tag that was already removed is a no-op.
 */
export function libraryRejectSuggestedTag(entity_id: EntityId, tag_id: TagId): Promise<void> {
  return invoke<void>("library_reject_suggested_tag", { entity_id, tag_id });
}

/**
 * Refreshes the project's AI style corpus by adding the given entity ids.
 *
 * Deduplicates against the existing corpus — passing an already-tracked
 * entity is harmless. Does not train; corpus management is decoupled from
 * verb invocation.
 */
export function libraryUpdateCorpus(entity_ids: EntityId[]): Promise<void> {
  return invoke<void>("library_update_corpus", { entity_ids });
}

// ── B10.5: per-entity LoRA training ───────────────────────────────────────────

export type TrainEntityLoraOptions = {
  lora_rank?: number | null;
  steps?: number | null;
  label?: string | null;
  model?: string | null;
};

export type LibraryTrainEntityLoraResult = {
  entity_id: EntityId;
  /**
   * The `LoRA` path now stored on `Entity.ai.lora_path`. Currently the
   * Replicate weights URL — downloading the safetensors into the
   * project directory is a follow-up shared with the project-wide
   * style training flow.
   */
  lora_path: string;
  label: string;
  training_id: string;
  invocation_id: string;
};

/**
 * Trains a per-entity `LoRA` from a sprite entity's canonical reference sheet
 * and persists the weights URL on `Entity.ai.lora_path`.
 *
 * Takes 15-30 minutes against Replicate. The returned promise only
 * resolves once the job completes; the `invocation_id` carried on the
 * result is intended for audit traces and after-the-fact correlation.
 *
 * **Cancellation during the in-flight run is not currently exposed by
 * this IPC.** The verb itself respects cooperative cancel checkpoints,
 * but the host has not yet wired a side channel that hands the active
 * `invocation_id` to the UI mid-run. Tracking work for that (and for
 * downloading the safetensors to the project directory so
 * `lora_path` becomes a real path) is filed as a follow-up issue
 * against this PR.
 */
export function libraryTrainEntityLora(
  entity_id: EntityId,
  options?: TrainEntityLoraOptions,
): Promise<LibraryTrainEntityLoraResult> {
  return invoke<LibraryTrainEntityLoraResult>("library_train_entity_lora", {
    entity_id,
    options: options ?? null,
  });
}

// ── entity anchor payload ─────────────────────────────────────────────────────

/**
 * Resolved anchor payload returned by [`libraryGetAnchorPayload`].
 *
 * Mirrors the Rust `pixhaus_ai::plugin::anchor::AnchorPayload` shape. The
 * `image_bytes` field is a `Vec<u8>` on the Rust side and arrives as a
 * `number[]` over the Tauri IPC bridge; UI code that wants to render the
 * image generally prefers the pre-encoded `image_b64` string.
 *
 * Defined here rather than under `lib/types/` because the `pixhaus-ai`
 * crate does not currently emit ts-rs bindings; if it ever does, swap
 * this declaration for the generated barrel re-export in one step.
 */
export type AnchorPayload = {
  /**
   * Sprite entity id for the entity that owns the embedded reference sheet.
   *
   * The field name is kept for compatibility with existing AI payload
   * consumers; it no longer points to a standalone Reference entity.
   */
  reference_entity_id: EntityId;
  // `canonical_hash` is intentionally omitted from this type. It exists
  // on the Rust side as a u64 FNV-1a (ai/src/plugin/anchor.rs:46) used
  // by the host's anchor cache. u64 values past 2^53 - 1 lose precision
  // when serde routes them through a JS number, so exposing the field
  // here would mislead any consumer that relied on its exact value. If
  // UI code ever needs the hash, re-add it as a `string` and pair with
  // a matching `serialize_with` on the Rust struct.
  mime: string;
  image_bytes: number[];
  image_b64: string;
  palette: PaletteEntry[];
  composition: SheetComposition;
  lora_path: string | null;
  strength: number;
};

/**
 * Returns the resolved [`AnchorPayload`] for an entity, or `null` when
 * the sprite entity has no embedded reference sheet.
 *
 * Cache hits are decided by the canonical-sheet hash; the wrapper itself
 * does no client-side caching.
 */
export function libraryGetAnchorPayload(entity_id: EntityId): Promise<AnchorPayload | null> {
  return invoke<AnchorPayload | null>("library_get_anchor_payload", { entity_id });
}

// ── composition library commands (spec sections 12, 15-17) ───────────────────
//
// These wrappers map to the Rust `library_*` commands registered in
// `app/src/commands/composition.rs`. Param key names match the Rust
// snake_case parameters exactly because the Tauri IPC bridge deserializes
// them without renaming.

/** Returns the full composition library: project records + built-in registry. */
export function listComposition(): Promise<CompositionLibrary> {
  return invoke<CompositionLibrary>("library_list_composition");
}

/** Inserts or replaces a project-tier Structure by id. */
export function upsertStructure(structure: Structure): Promise<void> {
  return invoke<void>("library_upsert_structure", { structure });
}

/** Inserts or replaces a project-tier Style by id. */
export function upsertStyle(style: Style): Promise<void> {
  return invoke<void>("library_upsert_style", { style });
}

/** Inserts or replaces a project-tier PromptTemplate by id. */
export function upsertPrompt(prompt: PromptTemplate): Promise<void> {
  return invoke<void>("library_upsert_prompt", { prompt });
}

/** Deletes a project-tier Structure. No-op if the id is a built-in. */
export function deleteStructure(id: StructureId): Promise<void> {
  return invoke<void>("library_delete_structure", { id });
}

/** Deletes a project-tier Style. No-op if the id is a built-in. */
export function deleteStyle(id: StyleId): Promise<void> {
  return invoke<void>("library_delete_style", { id });
}

/** Deletes a project-tier PromptTemplate. No-op if the id is a built-in. */
export function deletePrompt(id: PromptId): Promise<void> {
  return invoke<void>("library_delete_prompt", { id });
}

/**
 * Forks a built-in composition record into the project tier.
 * Returns the id of the newly created record.
 */
export function forkBuiltin(
  kind: CompositionKind,
  builtin_id: string,
  as_new: boolean,
): Promise<string> {
  return invoke<string>("library_fork_builtin", { kind, builtin_id, as_new });
}

/** Exports a subset of the library to a `.pixstyle` pack at `path`. */
export function exportPack(
  structure_ids: string[],
  style_ids: string[],
  prompt_ids: string[],
  path: string,
): Promise<void> {
  return invoke<void>("library_export_pack", { structure_ids, style_ids, prompt_ids, path });
}

/** Imports a `.pixstyle` pack from `path`, resolving conflicts with `policy`. */
export function importPack(path: string, policy: ConflictPolicy): Promise<ImportReport> {
  return invoke<ImportReport>("library_import_pack", { path, policy });
}

/**
 * Copies the composition library from another project file into this one,
 * resolving conflicts with `policy`.
 */
export function copyFromProject(
  source_path: string,
  policy: ConflictPolicy,
): Promise<ImportReport> {
  return invoke<ImportReport>("library_copy_from_project", { source_path, policy });
}

/**
 * Resolves the variables of a saved PromptTemplate against the given entity's
 * metadata, returning one `ResolvedVariable` per declared or detected token.
 */
export function resolvePromptVariables(
  prompt_id: PromptId,
  entity_id: EntityId,
): Promise<ResolvedVariable[]> {
  return invoke<ResolvedVariable[]>("library_resolve_prompt_variables", { prompt_id, entity_id });
}
