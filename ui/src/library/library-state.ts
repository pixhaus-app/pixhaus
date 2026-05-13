// Reactive state for the library panel.
//
// Entity and group data lives in Rust; this module caches the last-fetched
// lists and tracks UI-only state (selection, rename, drag target, expand/
// collapse, search query, panel visibility). Mutations go through the command
// wrappers, which call Rust and then refresh via refreshLibrary().

import { createSignal } from "solid-js";
import type {
  ActiveTarget,
  Entity,
  EntityGroup,
  EntityId,
  EntityKind,
  GroupId,
  SheetVariantId,
  StateId,
  TagDefinition,
  TagId,
} from "../lib/types";
import {
  libraryAddState,
  libraryApproveSheetVariant,
  libraryCreateEntity,
  libraryCreateGroup,
  libraryDeleteEntity,
  libraryDeleteGroup,
  libraryDeleteState,
  libraryListEntities,
  libraryListGroups,
  libraryListTags,
  libraryMoveEntityToGroup,
  libraryRenameEntity,
  libraryRenameGroup,
  libraryRenameState,
  libraryReorderEntities,
  librarySetActiveTarget,
  libraryUpdateCorpus,
  type LibraryCreateEntityArgs,
  type LibraryAddStateArgs,
} from "../lib/commands/library";
import { reportCommandFailure } from "../lib/utils/errors";

export type { LibraryCreateEntityArgs, LibraryAddStateArgs };

// ── Tree flattening ─────────────────────────────────────────────────────────

export type LibraryTreeEntry =
  | { kind: "group"; group: EntityGroup; depth: number; index: number }
  | { kind: "entity"; entity: Entity; depth: number; index: number };

/**
 * Flattens entities and groups into a top-down ordered display list.
 *
 * Groups appear before their children and before ungrouped entities at the
 * same depth. Collapsed groups exclude their children. Ungrouped entities
 * appear at the bottom of the flat list.
 */
export function flattenLibrary(
  entities: Entity[],
  groups: EntityGroup[],
  isGroupExpanded: (id: GroupId) => boolean,
): LibraryTreeEntry[] {
  const result: LibraryTreeEntry[] = [];
  let index = 0;

  function visitGroup(groupId: GroupId, depth: number): void {
    const group = groups.find((g) => g.id === groupId);
    if (!group) return;

    result.push({ kind: "group", group, depth, index: index++ });

    if (!isGroupExpanded(groupId)) return;

    // Direct child groups first (nested groups)
    for (const childGroup of groups) {
      const pid = childGroup.parent_id;
      if (pid !== null && pid !== undefined && pid === groupId) {
        visitGroup(childGroup.id, depth + 1);
      }
    }

    // Entities belonging to this group
    for (const entity of entities) {
      const gid = entity.group_id;
      if (gid !== null && gid !== undefined && gid === groupId) {
        result.push({ kind: "entity", entity, depth: depth + 1, index: index++ });
      }
    }
  }

  // Walk top-level groups (no parent)
  for (const group of groups) {
    const pid = group.parent_id;
    if (pid === null || pid === undefined) {
      visitGroup(group.id, 0);
    }
  }

  // Ungrouped entities at the end
  for (const entity of entities) {
    const gid = entity.group_id;
    if (gid === null || gid === undefined) {
      result.push({ kind: "entity", entity, depth: 0, index: index++ });
    }
  }

  return result;
}

// ── Panel visibility ────────────────────────────────────────────────────────

export const [isLibraryPanelVisible, setLibraryPanelVisible] = createSignal(true);

// ── Data cache ──────────────────────────────────────────────────────────────

export const [entities, setEntities] = createSignal<Entity[]>([]);
export const [groups, setGroups] = createSignal<EntityGroup[]>([]);
export const [tags, setTags] = createSignal<TagDefinition[]>([]);

// Lookup from TagId to its definition. Recomputed once per refreshLibrary()
// so chip rendering can hit the map by id in O(1) instead of running a
// linear `tags().find(...)` per chip per row.
export const [tagsById, setTagsById] = createSignal<ReadonlyMap<TagId, TagDefinition>>(new Map());

// Stale-request guard: incremented on every refreshLibrary() call. Async
// IPC responses from a superseded refresh are dropped on arrival.
let refreshToken = 0;

export function refreshLibrary(): void {
  refreshToken += 1;
  const myToken = refreshToken;

  Promise.all([libraryListEntities(), libraryListGroups(), libraryListTags()])
    .then(([newEntities, newGroups, newTags]) => {
      if (myToken !== refreshToken) return;
      setEntities(newEntities);
      setGroups(newGroups);
      setTags(newTags);

      // Rebuild the id lookup so chip rendering doesn't pay linear search
      // costs per row.
      const byId = new Map<TagId, TagDefinition>();
      for (const tag of newTags) byId.set(tag.id, tag);
      setTagsById(byId);

      // Drop pending suggestions for entities that no longer exist. Without
      // this, switching projects (or deleting an entity then creating a new
      // one that reuses its id) would surface stale chips against the new
      // entity. Refreshing on the same entity list is a no-op.
      const liveIds = new Set(newEntities.map((e) => e.id));
      setPendingTagSuggestions((prev) => {
        let removed = false;
        const next = new Map<EntityId, ReadonlyArray<TagId>>();
        for (const [id, sugg] of prev) {
          if (liveIds.has(id)) {
            next.set(id, sugg);
          } else {
            removed = true;
          }
        }
        return removed ? next : prev;
      });
    })
    .catch((err: unknown) => console.error("[pixhaus] refreshLibrary:", err));
}

// ── AI tag suggestions (B9.4) ───────────────────────────────────────────────

// Pending VLM suggestions per entity, keyed by EntityId. Tags only land in
// `entity.tags` once the user accepts them; until then they live here so the
// row can render a chip strip with accept/reject buttons. The Map is replaced
// (not mutated) on every update so the Solid signal fires.
export const [pendingTagSuggestions, setPendingTagSuggestions] = createSignal<
  ReadonlyMap<EntityId, ReadonlyArray<TagId>>
>(new Map());

export function addPendingSuggestions(entityId: EntityId, tagIds: TagId[]): void {
  setPendingTagSuggestions((prev) => {
    const next = new Map(prev);
    const existing = next.get(entityId) ?? [];
    // Dedupe: a re-run of auto-tag against the same entity shouldn't double
    // up the pending list.
    const merged = [...existing];
    for (const id of tagIds) {
      if (!merged.includes(id)) merged.push(id);
    }
    next.set(entityId, merged);
    return next;
  });
}

export function removePendingSuggestion(entityId: EntityId, tagId: TagId): void {
  setPendingTagSuggestions((prev) => {
    const existing = prev.get(entityId);
    if (existing === undefined) return prev;
    const filtered = existing.filter((id) => id !== tagId);
    const next = new Map(prev);
    if (filtered.length === 0) {
      next.delete(entityId);
    } else {
      next.set(entityId, filtered);
    }
    return next;
  });
}

export function clearPendingSuggestions(entityId: EntityId): void {
  setPendingTagSuggestions((prev) => {
    if (!prev.has(entityId)) return prev;
    const next = new Map(prev);
    next.delete(entityId);
    return next;
  });
}

/**
 * Background-refreshes the project AI corpus to include the given entity.
 *
 * Fire-and-forget — the caller proceeds even if the corpus update fails;
 * the toast surface notifies the user and the next mutation gets a fresh
 * chance to push the corpus state forward.
 */
export function refreshCorpusFor(entityId: EntityId): void {
  libraryUpdateCorpus([entityId]).catch((err: unknown) =>
    reportCommandFailure("library_update_corpus", err),
  );
}

// ── Selection ───────────────────────────────────────────────────────────────

export const [selectedEntityId, setSelectedEntityId] = createSignal<EntityId | null>(null);

// ── Inline rename ───────────────────────────────────────────────────────────

export type RenameTarget =
  | { kind: "entity"; id: EntityId }
  | { kind: "group"; id: GroupId }
  | { kind: "state"; entityId: EntityId; stateId: StateId };

export const [renamingTarget, setRenamingTarget] = createSignal<RenameTarget | null>(null);

export function beginEntityRename(id: EntityId): void {
  setRenamingTarget({ kind: "entity", id });
}

export function beginGroupRename(id: GroupId): void {
  setRenamingTarget({ kind: "group", id });
}

export function beginStateRename(entityId: EntityId, stateId: StateId): void {
  setRenamingTarget({ kind: "state", entityId, stateId });
}

export function cancelRename(): void {
  setRenamingTarget(null);
}

export function commitEntityRename(id: EntityId, name: string): void {
  setRenamingTarget(null);
  const trimmed = name.trim();
  if (!trimmed) return;
  libraryRenameEntity(id, trimmed)
    .then(() => refreshLibrary())
    .catch((err: unknown) => console.error("[pixhaus] library_rename_entity:", err));
}

export function commitGroupRename(id: GroupId, name: string): void {
  setRenamingTarget(null);
  const trimmed = name.trim();
  if (!trimmed) return;
  libraryRenameGroup(id, trimmed)
    .then(() => refreshLibrary())
    .catch((err: unknown) => console.error("[pixhaus] library_rename_group:", err));
}

export function commitStateRename(entityId: EntityId, stateId: StateId, name: string): void {
  setRenamingTarget(null);
  const trimmed = name.trim();
  if (!trimmed) return;
  libraryRenameState(entityId, stateId, trimmed)
    .then(() => refreshLibrary())
    .catch((err: unknown) => console.error("[pixhaus] library_rename_state:", err));
}

// ── Expanded groups ─────────────────────────────────────────────────────────

// Groups default to expanded; track collapsed ones explicitly.
const [collapsedGroups, setCollapsedGroups] = createSignal<ReadonlySet<GroupId>>(new Set());

export function isGroupExpanded(id: GroupId): boolean {
  return !collapsedGroups().has(id);
}

export function toggleGroupExpanded(id: GroupId): void {
  setCollapsedGroups((prev) => {
    const next = new Set(prev);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    return next;
  });
}

export function ensureGroupExpanded(id: GroupId): void {
  setCollapsedGroups((prev) => {
    if (!prev.has(id)) return prev;
    const next = new Set(prev);
    next.delete(id);
    return next;
  });
}

// ── Drag-to-reorder ─────────────────────────────────────────────────────────

// Flat-list index where the drop indicator renders.
export const [dragOverIndex, setDragOverIndex] = createSignal<number | null>(null);

// ── Search ──────────────────────────────────────────────────────────────────

export const [searchQuery, setSearchQuery] = createSignal("");

// Active kind filter. null = show all.
export const [kindFilter, setKindFilter] = createSignal<EntityKind | null>(null);

// Active tag filter. null = show all.
export const [tagFilter, setTagFilter] = createSignal<TagId | null>(null);

/**
 * Filters entities by the current search query, kind filter, and tag filter.
 * Matches entity names, Custom kind strings, and tag names case-insensitively.
 */
export function filteredEntities(allEntities: Entity[], allTags: TagDefinition[]): Entity[] {
  const q = searchQuery().toLowerCase().trim();
  const kf = kindFilter();
  const tf = tagFilter();

  return allEntities.filter((entity) => {
    if (kf !== null) {
      if (kf.kind !== entity.kind.kind) return false;
      if (kf.kind === "Custom" && entity.kind.kind === "Custom" && kf.value !== entity.kind.value) {
        return false;
      }
    }

    if (tf !== null) {
      const entityTags = entity.tags ?? [];
      if (!entityTags.includes(tf)) return false;
    }

    if (!q) return true;

    if (entity.name.toLowerCase().includes(q)) return true;

    if (entity.kind.kind === "Custom" && entity.kind.value.toLowerCase().includes(q)) return true;

    const entityTags = entity.tags ?? [];
    for (const tagId of entityTags) {
      const def = allTags.find((t) => t.id === tagId);
      if (def?.name.toLowerCase().includes(q)) return true;
    }

    return false;
  });
}

// ── Mutation helpers ────────────────────────────────────────────────────────

export function createEntity(args: LibraryCreateEntityArgs): void {
  libraryCreateEntity(args)
    .then((entity) => {
      refreshLibrary();
      setSelectedEntityId(entity.id);
    })
    .catch((err: unknown) => console.error("[pixhaus] library_create_entity:", err));
}

export function deleteEntity(entityId: EntityId): void {
  libraryDeleteEntity(entityId)
    .then(() => {
      refreshLibrary();
      if (selectedEntityId() === entityId) setSelectedEntityId(null);
    })
    .catch((err: unknown) => console.error("[pixhaus] library_delete_entity:", err));
}

export function moveEntityToGroup(entityId: EntityId, groupId: GroupId | null): void {
  libraryMoveEntityToGroup(entityId, groupId)
    .then(() => refreshLibrary())
    .catch((err: unknown) => console.error("[pixhaus] library_move_entity_to_group:", err));
}

export function reorderEntity(entityId: EntityId, newIndex: number): void {
  libraryReorderEntities(entityId, newIndex)
    .then(() => refreshLibrary())
    .catch((err: unknown) => console.error("[pixhaus] library_reorder_entities:", err));
}

export function createGroup(name: string, parentId?: GroupId): void {
  libraryCreateGroup({ name, parent_id: parentId ?? null })
    .then((group) => {
      refreshLibrary();
      ensureGroupExpanded(group.id);
    })
    .catch((err: unknown) => console.error("[pixhaus] library_create_group:", err));
}

export function deleteGroup(groupId: GroupId, keepEntities: boolean): void {
  libraryDeleteGroup({ group_id: groupId, keep_entities: keepEntities })
    .then(() => refreshLibrary())
    .catch((err: unknown) => console.error("[pixhaus] library_delete_group:", err));
}

export function addStateToEntity(args: LibraryAddStateArgs): void {
  libraryAddState(args)
    .then(() => {
      refreshLibrary();
      // The new state is meaningful library content — refresh the AI
      // style corpus so subsequent verb runs see it.
      refreshCorpusFor(args.entity_id);
    })
    .catch((err: unknown) => console.error("[pixhaus] library_add_state:", err));
}

/**
 * Promotes a sheet variant to canonical, returns the updated entity, and
 * fires a background corpus refresh so the new canonical sheet is part of
 * the AI style context for subsequent verb runs.
 *
 * Sheet panels call this instead of `libraryApproveSheetVariant` directly so
 * the corpus update lives next to the mutation that triggers it.
 */
export function approveSheetVariantAndRefreshCorpus(
  entityId: EntityId,
  variantId: SheetVariantId,
): Promise<Entity> {
  return libraryApproveSheetVariant(entityId, variantId).then((updated) => {
    refreshCorpusFor(entityId);
    return updated;
  });
}

export function deleteState(entityId: EntityId, stateId: StateId): void {
  libraryDeleteState(entityId, stateId)
    .then(() => refreshLibrary())
    .catch((err: unknown) => console.error("[pixhaus] library_delete_state:", err));
}

export function setActiveTarget(target: ActiveTarget): void {
  librarySetActiveTarget(target).catch((err: unknown) =>
    console.error("[pixhaus] library_set_active_target:", err),
  );
}
