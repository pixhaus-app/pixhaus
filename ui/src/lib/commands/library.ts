// Library IPC command wrappers — entity and sheet operations.
//
// These are the commands added in B9.2 (entity CRUD) and B10.4 (sheet
// variant approval, asset info, history management). Commands that touch
// pixel data live in canvas.ts; palette commands live in palette.ts.

import { invoke } from "../ipc";
import type {
  AssetInfo,
  Entity,
  EntityId,
  EntityKind,
  GroupId,
  SheetVariantId,
  TagId,
} from "../types";

// ── entity queries ────────────────────────────────────────────────────────────

/** Returns a single entity by stable id. */
export function libraryGetEntity(entity_id: EntityId): Promise<Entity> {
  return invoke<Entity>("library_get_entity", { entity_id });
}

/** Lists all entities, optionally filtered by kind, group, or tag. */
export function libraryListEntities(opts?: {
  kind?: EntityKind;
  group_id?: GroupId;
  tag_id?: TagId;
}): Promise<Entity[]> {
  return invoke<Entity[]>("library_list_entities", {
    kind: opts?.kind ?? null,
    group_id: opts?.group_id ?? null,
    tag_id: opts?.tag_id ?? null,
  });
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
