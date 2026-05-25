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
import { createBackendQuery } from "../lib/sync/query";
import { runMutation } from "../lib/sync/mutation";
import { projectState } from "../project-state";

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

// ── Data cache ──────────────────────────────────────────────────────────────

// Declared before the query below because the query's onLoaded prunes this
// map on its initial (synchronous) run, which would otherwise hit the const's
// temporal dead zone. See the "AI tag suggestions" section for the mutators.
export const [pendingTagSuggestions, setPendingTagSuggestions] = createSignal<
  ReadonlyMap<EntityId, ReadonlyArray<TagId>>
>(new Map());

interface LibraryData {
  entities: Entity[];
  groups: EntityGroup[];
  tags: TagDefinition[];
  // Lookup from TagId to its definition, built once per fetch so chip
  // rendering hits the map in O(1) instead of a linear tags().find() per chip.
  tagsById: ReadonlyMap<TagId, TagDefinition>;
}

const EMPTY_LIBRARY: LibraryData = {
  entities: [],
  groups: [],
  tags: [],
  tagsById: new Map(),
};

// Entities, groups, and tags load together as one project-global query. They
// used to share a hand-rolled refresh token to drop superseded responses; the
// query gets that from createResource. The source is the active project, so
// the library loads on project open and clears on close.
const libraryQuery = createBackendQuery<true, LibraryData>({
  key: "library",
  source: () => (projectState.activeProject !== null ? true : null),
  fetch: async () => {
    const [newEntities, newGroups, newTags] = await Promise.all([
      libraryListEntities(),
      libraryListGroups(),
      libraryListTags(),
    ]);
    const byId = new Map<TagId, TagDefinition>();
    for (const tag of newTags) byId.set(tag.id, tag);
    return { entities: newEntities, groups: newGroups, tags: newTags, tagsById: byId };
  },
  initial: EMPTY_LIBRARY,
  onLoaded: (data) => pruneStaleSuggestions(data.entities),
  errorTitle: "Failed to load library",
});

export const entities = (): Entity[] => libraryQuery.data().entities;
export const groups = (): EntityGroup[] => libraryQuery.data().groups;
export const tags = (): TagDefinition[] => libraryQuery.data().tags;
export const tagsById = (): ReadonlyMap<TagId, TagDefinition> => libraryQuery.data().tagsById;

/** Refetches entities, groups, and tags for the active project. */
export function refreshLibrary(): void {
  void libraryQuery.refetch();
}

// Drop pending suggestions for entities that no longer exist. Without this,
// switching projects (or deleting an entity then creating a new one that
// reuses its id) would surface stale chips against the new entity. Refreshing
// on the same entity list is a no-op.
function pruneStaleSuggestions(liveEntities: readonly Entity[] | undefined): void {
  const liveIds = new Set((liveEntities ?? []).map((e) => e.id));
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
}

// ── AI tag suggestions (B9.4) ───────────────────────────────────────────────

// Pending VLM suggestions per entity, keyed by EntityId. Tags only land in
// `entity.tags` once the user accepts them; until then they live here so the
// row can render a chip strip with accept/reject buttons. The Map is replaced
// (not mutated) on every update so the Solid signal fires. Declared above the
// library query (temporal-dead-zone reasons); the mutators follow here.
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
  void runMutation({ run: () => libraryRenameEntity(id, trimmed), invalidate: ["library"] });
}

export function commitGroupRename(id: GroupId, name: string): void {
  setRenamingTarget(null);
  const trimmed = name.trim();
  if (!trimmed) return;
  void runMutation({ run: () => libraryRenameGroup(id, trimmed), invalidate: ["library"] });
}

export function commitStateRename(entityId: EntityId, stateId: StateId, name: string): void {
  setRenamingTarget(null);
  const trimmed = name.trim();
  if (!trimmed) return;
  void runMutation({
    run: () => libraryRenameState(entityId, stateId, trimmed),
    invalidate: ["library"],
  });
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
  void runMutation({
    run: () => libraryCreateEntity(args),
    invalidate: ["library"],
    onSuccess: (entity) => setSelectedEntityId(entity.id),
  });
}

export function deleteEntity(entityId: EntityId): void {
  void runMutation({
    run: () => libraryDeleteEntity(entityId),
    invalidate: ["library"],
    onSuccess: () => {
      if (selectedEntityId() === entityId) setSelectedEntityId(null);
    },
  });
}

export function moveEntityToGroup(entityId: EntityId, groupId: GroupId | null): void {
  void runMutation({
    run: () => libraryMoveEntityToGroup(entityId, groupId),
    invalidate: ["library"],
  });
}

export function reorderEntity(entityId: EntityId, newIndex: number): void {
  void runMutation({
    run: () => libraryReorderEntities(entityId, newIndex),
    invalidate: ["library"],
  });
}

export function createGroup(name: string, parentId?: GroupId): void {
  void runMutation({
    run: () => libraryCreateGroup({ name, parent_id: parentId ?? null }),
    invalidate: ["library"],
    onSuccess: (group) => ensureGroupExpanded(group.id),
  });
}

export function deleteGroup(groupId: GroupId, keepEntities: boolean): void {
  void runMutation({
    run: () => libraryDeleteGroup({ group_id: groupId, keep_entities: keepEntities }),
    invalidate: ["library"],
  });
}

export function addStateToEntity(args: LibraryAddStateArgs): void {
  void runMutation({
    run: () => libraryAddState(args),
    invalidate: ["library"],
    // The new state is meaningful library content — refresh the AI style
    // corpus so subsequent verb runs see it.
    onSuccess: () => refreshCorpusFor(args.entity_id),
  });
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
  void runMutation({
    run: () => libraryDeleteState(entityId, stateId),
    invalidate: ["library"],
  });
}

export function setActiveTarget(target: ActiveTarget): void {
  librarySetActiveTarget(target).catch((err: unknown) =>
    console.error("[pixhaus] library_set_active_target:", err),
  );
}
