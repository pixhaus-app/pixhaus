// Library entity, group, tag, search, and sheet-variant commands.
//
// Covers the B9.2 entity/group/tag catalog (CRUD, states, active target,
// search) plus the B10.4 reference-sheet variant operations (approve,
// update asset info, delete history entry). Commands that touch pixel
// data live in canvas.ts; palette commands live in palette.ts.

import { invoke } from "../ipc";
import type {
  ActiveTarget,
  AssetInfo,
  ColorMode,
  Entity,
  EntityGroup,
  EntityId,
  EntityKind,
  GroupId,
  NamedSprite,
  Rgba,
  SheetVariantId,
  TagDefinition,
  TagId,
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
  // Reference-kind
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
 * Promotes a history variant to canonical for a `Reference`-kind entity.
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
 * `Reference`-kind entity.
 */
export function libraryUpdateAssetInfo(entity_id: EntityId, info: AssetInfo): Promise<void> {
  return invoke<void>("library_update_asset_info", { args: { entity_id, info } });
}

/**
 * Deletes a history variant from a `Reference`-kind entity.
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
 * Suitable only for `Custom` and `Reference` entities. The verb may need a
 * configured VLM backend; failure propagates as a rejected promise.
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
 * Trains a per-entity `LoRA` from a Reference entity's canonical sheet
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
