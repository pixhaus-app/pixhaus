// Library entity, group, tag, and search commands.

import { invoke } from "../ipc";
import type {
  ActiveTarget,
  ColorMode,
  Entity,
  EntityGroup,
  EntityId,
  EntityKind,
  GroupId,
  NamedSprite,
  Rgba,
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
