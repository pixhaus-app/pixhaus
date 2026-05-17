// Library panel — left-side panel showing the entity tree for the active project.
//
// Tree view: groups (optionally nested) followed by ungrouped entities.
// Drag-to-reorder entities within the flat list. Context menus (right-click)
// for both entity rows and group rows. Search bar filters by name, Custom
// kind string, and tag names. Kind filter chips appear dynamically based on
// which Custom categories exist in the project.

import {
  type Component,
  For,
  Show,
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
} from "solid-js";
import type { Entity, EntityId, GroupId, StateId, TagDefinition, TagId } from "../lib/types";
import {
  libraryAcceptSuggestedTag,
  libraryGetAnchorPayload,
  libraryRejectSuggestedTag,
  type AnchorPayload,
} from "../lib/commands/library";
import { reportCommandFailure } from "../lib/utils/errors";
import { Button } from "../lib/ui/Button";
import { Dialog } from "../lib/ui/Dialog";
import {
  addStateToEntity,
  beginEntityRename,
  beginGroupRename,
  beginStateRename,
  cancelRename,
  commitEntityRename,
  commitGroupRename,
  commitStateRename,
  createGroup,
  deleteState,
  dragOverIndex,
  entities,
  filteredEntities,
  flattenLibrary,
  groups,
  isGroupExpanded,
  kindFilter,
  moveEntityToGroup,
  pendingTagSuggestions,
  refreshLibrary,
  removePendingSuggestion,
  renamingTarget,
  reorderEntity,
  searchQuery,
  selectedEntityId,
  setActiveTarget,
  setDragOverIndex,
  setKindFilter,
  setLibraryPanelVisible,
  setSearchQuery,
  setSelectedEntityId,
  tags,
  tagsById,
  toggleGroupExpanded,
} from "./library-state";
import LibraryContextMenu, { type LibraryContextTarget } from "./LibraryContextMenu";
import { openEntityCreate } from "./entity-create-state";
import type { LibraryTreeEntry } from "./library-state";
import { openSheetEditor } from "../sheet/sheet-state";

// ── Panel component ──────────────────────────────────────────────────────────

const LibraryPanel: Component = () => {
  createEffect(() => refreshLibrary());

  const displayed = createMemo(() => filteredEntities(entities(), tags()));

  const treeEntries = createMemo(() => flattenLibrary(displayed(), groups(), isGroupExpanded));

  // ── Context menu ─────────────────────────────────────────────────────────

  const [contextTarget, setContextTarget] = createSignal<LibraryContextTarget | null>(null);

  function handleEntityContextMenu(e: MouseEvent, entityId: EntityId): void {
    e.preventDefault();
    if (selectedEntityId() !== entityId) setSelectedEntityId(entityId);
    setContextTarget({ kind: "entity", x: e.clientX, y: e.clientY, entityId });
  }

  function handleGroupContextMenu(e: MouseEvent, groupId: GroupId): void {
    e.preventDefault();
    setContextTarget({ kind: "group", x: e.clientX, y: e.clientY, groupId });
  }

  // ── Add state modal ───────────────────────────────────────────────────────

  const [addStateTarget, setAddStateTarget] = createSignal<EntityId | null>(null);
  const [addStateName, setAddStateName] = createSignal("idle");

  function handleAddStateConfirm(): void {
    const id = addStateTarget();
    const name = addStateName().trim();
    if (!id || !name) return;
    addStateToEntity({ entity_id: id, state_name: name });
    setAddStateTarget(null);
    setAddStateName("idle");
  }

  // ── Move to group modal ───────────────────────────────────────────────────

  const [moveGroupTarget, setMoveGroupTarget] = createSignal<EntityId | null>(null);
  const [moveGroupDest, setMoveGroupDest] = createSignal<GroupId | null>(null);

  function handleMoveGroupConfirm(): void {
    const id = moveGroupTarget();
    if (id === null) return;
    moveEntityToGroup(id, moveGroupDest());
    setMoveGroupTarget(null);
    setMoveGroupDest(null);
  }

  // ── Drag-to-reorder ───────────────────────────────────────────────────────

  // Dragging is only wired to entity rows; groups are not draggable in B9.

  // ── Kind filter chips ─────────────────────────────────────────────────────

  // Collect the distinct Custom category strings that actually exist.
  const customCategories = createMemo(() => {
    const cats = new Set<string>();
    for (const e of entities()) {
      if (e.kind.kind === "Custom") cats.add(e.kind.value);
    }
    return [...cats].sort();
  });

  return (
    <div class="library-panel" data-testid="library-panel">
      {/* ── Header ── */}
      <div class="library-panel__header">
        <span class="library-panel__title">Library</span>
        <div class="library-panel__header-actions">
          <button
            class="library-panel__icon-btn"
            data-testid="library-add-entity"
            onClick={() => openEntityCreate()}
            title="New entity"
          >
            <svg
              width="12"
              height="12"
              viewBox="0 0 12 12"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
            >
              <path d="M6 1 V11 M1 6 H11" />
            </svg>
          </button>
          <button
            class="library-panel__icon-btn"
            data-testid="library-add-group"
            onClick={() => {
              const name = `Group ${groups().length + 1}`;
              createGroup(name);
            }}
            title="New group"
          >
            <svg
              width="12"
              height="12"
              viewBox="0 0 12 12"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <rect x="1" y="4" width="10" height="7" rx="1" />
              <path d="M1 4 V2.5 L4.5 2.5 L5.5 4" />
            </svg>
          </button>
          <button
            class="library-panel__icon-btn"
            onClick={() => setLibraryPanelVisible(false)}
            title="Close library panel"
          >
            <svg
              width="10"
              height="10"
              viewBox="0 0 10 10"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
            >
              <path d="M1 1 L9 9 M9 1 L1 9" />
            </svg>
          </button>
        </div>
      </div>

      {/* ── Search bar ── */}
      <div class="library-panel__search-row">
        <div class="library-panel__search-wrap">
          <svg
            class="library-panel__search-icon"
            width="12"
            height="12"
            viewBox="0 0 12 12"
            fill="none"
            stroke="currentColor"
            stroke-width="1.3"
            stroke-linecap="round"
          >
            <circle cx="5" cy="5" r="3.5" />
            <path d="M8 8 L11 11" />
          </svg>
          <input
            class="library-panel__search"
            type="text"
            placeholder="Search..."
            value={searchQuery()}
            onInput={(e) => setSearchQuery(e.currentTarget.value)}
            data-testid="library-search"
          />
          <Show when={searchQuery().length > 0}>
            <button
              class="library-panel__search-clear"
              onClick={() => setSearchQuery("")}
              title="Clear search"
            >
              ×
            </button>
          </Show>
        </div>
      </div>

      {/* ── Kind filter chips ── */}
      <Show
        when={customCategories().length > 0 || entities().some((e) => e.kind.kind !== "Custom")}
      >
        <div class="library-panel__chips">
          <button
            class="library-panel__chip"
            classList={{ "library-panel__chip--active": kindFilter() === null }}
            onClick={() => setKindFilter(null)}
          >
            All
          </button>
          <Show when={entities().some((e) => e.kind.kind === "Tileset")}>
            <button
              class="library-panel__chip"
              classList={{
                "library-panel__chip--active": kindFilter()?.kind === "Tileset",
              }}
              onClick={() => setKindFilter({ kind: "Tileset" })}
            >
              Tilesets
            </button>
          </Show>
          <Show when={entities().some((e) => e.kind.kind === "Tilemap")}>
            <button
              class="library-panel__chip"
              classList={{
                "library-panel__chip--active": kindFilter()?.kind === "Tilemap",
              }}
              onClick={() => setKindFilter({ kind: "Tilemap" })}
            >
              Tilemaps
            </button>
          </Show>
          <For each={customCategories()}>
            {(cat) => (
              <button
                class="library-panel__chip"
                classList={{
                  "library-panel__chip--active":
                    kindFilter()?.kind === "Custom" &&
                    (kindFilter() as { kind: "Custom"; value: string }).value === cat,
                }}
                onClick={() => setKindFilter({ kind: "Custom", value: cat })}
              >
                {cat}
              </button>
            )}
          </For>
        </div>
      </Show>

      {/* ── Tree ── */}
      <Show
        when={entities().length > 0 || groups().length > 0}
        fallback={
          <div class="library-panel__empty">
            <span>No entities yet.</span>
            <button class="library-panel__empty-cta" onClick={() => openEntityCreate()}>
              Create your first entity
            </button>
          </div>
        }
      >
        <Show
          when={treeEntries().length > 0}
          fallback={
            <div class="library-panel__empty">
              <span>No results for "{searchQuery()}"</span>
            </div>
          }
        >
          <div
            class="library-panel__scroll"
            onDragOver={(e) => e.preventDefault()}
            onDrop={(e) => e.preventDefault()}
            data-testid="library-tree"
          >
            <For each={treeEntries()}>
              {(entry) => (
                <Show
                  when={entry.kind === "group"}
                  fallback={
                    <EntityRow
                      entry={entry as Extract<LibraryTreeEntry, { kind: "entity" }>}
                      onContextMenu={handleEntityContextMenu}
                    />
                  }
                >
                  <GroupRow
                    entry={entry as Extract<LibraryTreeEntry, { kind: "group" }>}
                    onContextMenu={handleGroupContextMenu}
                  />
                </Show>
              )}
            </For>
            <div
              class="library-panel__drop-zone-end"
              onDragOver={(e) => {
                e.preventDefault();
                if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
                setDragOverIndex(treeEntries().length);
              }}
              onDragLeave={() => setDragOverIndex(null)}
              onDrop={(e) => {
                e.preventDefault();
                setDragOverIndex(null);
                const raw = e.dataTransfer?.getData("text/plain") ?? "";
                let parsed: { id: number; index: number } | null = null;
                try {
                  const v = JSON.parse(raw) as unknown;
                  if (
                    v !== null &&
                    typeof v === "object" &&
                    typeof (v as { id: unknown }).id === "number" &&
                    typeof (v as { index: unknown }).index === "number"
                  ) {
                    parsed = v as { id: number; index: number };
                  }
                } catch {
                  // Bad payload — ignore.
                }
                const lastIndex = treeEntries().length - 1;
                if (parsed === null || parsed.index === lastIndex) return;
                reorderEntity(parsed.id as EntityId, lastIndex);
              }}
            />
            <Show when={dragOverIndex() === treeEntries().length}>
              <div class="library-panel__drop-indicator" />
            </Show>
          </div>
        </Show>
      </Show>

      {/* ── Context menu ── */}
      <LibraryContextMenu
        target={contextTarget()}
        onClose={() => setContextTarget(null)}
        onAddState={(id) => {
          setAddStateTarget(id);
          setAddStateName("idle");
        }}
        onMoveToGroup={(id) => {
          setMoveGroupTarget(id);
          setMoveGroupDest(null);
        }}
        onCreateGroup={() => {
          if (contextTarget()?.kind === "group") {
            createGroup("New group", (contextTarget() as { groupId: GroupId }).groupId);
          } else {
            createGroup(`Group ${groups().length + 1}`);
          }
        }}
      />

      {/* ── Add state dialog ── */}
      <Dialog
        open={addStateTarget() !== null}
        title="Add state"
        onClose={() => setAddStateTarget(null)}
        size="sm"
      >
        <Dialog.Body>
          <div class="modal__field">
            <label class="modal__label">State name</label>
            <input
              class="modal__input"
              list="state-suggestions-add"
              value={addStateName()}
              onInput={(e) => setAddStateName(e.currentTarget.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleAddStateConfirm();
              }}
            />
            <datalist id="state-suggestions-add">
              <option value="idle" />
              <option value="walk" />
              <option value="run" />
              <option value="jump" />
              <option value="attack" />
              <option value="hurt" />
              <option value="death" />
              <option value="victory" />
            </datalist>
          </div>
        </Dialog.Body>
        <Dialog.Footer>
          <Button variant="ghost" onClick={() => setAddStateTarget(null)}>
            Cancel
          </Button>
          <Button onClick={handleAddStateConfirm}>Add</Button>
        </Dialog.Footer>
      </Dialog>

      {/* ── Move to group dialog ── */}
      <Dialog
        open={moveGroupTarget() !== null}
        title="Move to group"
        onClose={() => setMoveGroupTarget(null)}
        size="sm"
      >
        <Dialog.Body>
          <div class="modal__field">
            <label class="modal__label">Group</label>
            <select
              class="modal__select"
              value={moveGroupDest() ?? ""}
              onChange={(e) => {
                const v = e.currentTarget.value;
                setMoveGroupDest(v === "" ? null : (parseInt(v, 10) as GroupId));
              }}
            >
              <option value="">None (ungrouped)</option>
              <For each={groups()}>{(g) => <option value={g.id}>{g.name}</option>}</For>
            </select>
          </div>
        </Dialog.Body>
        <Dialog.Footer>
          <Button variant="ghost" onClick={() => setMoveGroupTarget(null)}>
            Cancel
          </Button>
          <Button onClick={handleMoveGroupConfirm}>Move</Button>
        </Dialog.Footer>
      </Dialog>
    </div>
  );
};

// ── Group row ────────────────────────────────────────────────────────────────

type GroupRowProps = {
  entry: Extract<LibraryTreeEntry, { kind: "group" }>;
  onContextMenu: (e: MouseEvent, groupId: GroupId) => void;
};

const GroupRow: Component<GroupRowProps> = (props) => {
  const expanded = () => isGroupExpanded(props.entry.group.id);
  const isRenaming = () => {
    const t = renamingTarget();
    return t?.kind === "group" && t.id === props.entry.group.id;
  };

  return (
    <div
      class="library-row library-row--group"
      style={{ "padding-left": `${6 + props.entry.depth * 16}px` }}
      onContextMenu={(e) => {
        e.preventDefault();
        props.onContextMenu(e, props.entry.group.id);
      }}
      data-testid={`group-row-${props.entry.group.id}`}
    >
      <button
        class="library-row__chevron"
        onClick={() => toggleGroupExpanded(props.entry.group.id)}
        title={expanded() ? "Collapse group" : "Expand group"}
      >
        <svg width="8" height="8" viewBox="0 0 8 8" fill="currentColor">
          <Show
            when={expanded()}
            fallback={
              <path
                d="M2 1 L6 4 L2 7"
                stroke="currentColor"
                stroke-width="1.5"
                fill="none"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            }
          >
            <path
              d="M1 2 L4 6 L7 2"
              stroke="currentColor"
              stroke-width="1.5"
              fill="none"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </Show>
        </svg>
      </button>

      <svg
        class="library-row__kind-icon"
        width="16"
        height="16"
        viewBox="0 0 16 16"
        fill="none"
        stroke="var(--accent)"
        stroke-width="1.2"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <rect x="1" y="5" width="14" height="9" rx="1" />
        <path d="M1 5V3.5L5.5 3.5L7 5" />
      </svg>

      <Show
        when={!isRenaming()}
        fallback={
          <InlineRenameInput
            initialName={props.entry.group.name}
            onCommit={(name) => commitGroupRename(props.entry.group.id, name)}
            onCancel={cancelRename}
          />
        }
      >
        <span class="library-row__name" onDblClick={() => beginGroupRename(props.entry.group.id)}>
          {props.entry.group.name}
        </span>
      </Show>
    </div>
  );
};

// ── Entity row ───────────────────────────────────────────────────────────────

type EntityRowProps = {
  entry: Extract<LibraryTreeEntry, { kind: "entity" }>;
  onContextMenu: (e: MouseEvent, entityId: EntityId) => void;
};

const EntityRow: Component<EntityRowProps> = (props) => {
  const isSelected = () => selectedEntityId() === props.entry.entity.id;
  const isRenaming = () => {
    const t = renamingTarget();
    return t?.kind === "entity" && t.id === props.entry.entity.id;
  };
  const isDragTarget = () => dragOverIndex() === props.entry.index;

  // State expand/collapse for Custom entities with multiple states
  const isCustom = () => props.entry.entity.kind.kind === "Custom";
  const states = () => {
    const content = props.entry.entity.content;
    if (content.type !== "Sprites") return [];
    return content.value.states;
  };
  const hasMultipleStates = () => isCustom() && states().length > 1;

  const [statesExpanded, setStatesExpanded] = createSignal(false);

  function handleClick(): void {
    setSelectedEntityId(props.entry.entity.id);
    const e = props.entry.entity;
    if (e.content.type === "Sprites") {
      const first = e.content.value.states[0];
      if (first) {
        setActiveTarget({ type: "state", value: { entity_id: e.id, state_id: first.id } });
      }
    } else if (e.content.type === "Tileset") {
      setActiveTarget({ type: "tileset", value: { entity_id: e.id } });
    } else if (e.content.type === "Tilemap") {
      setActiveTarget({ type: "tilemap", value: { entity_id: e.id } });
    }
  }

  function handleDragStart(e: DragEvent): void {
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData(
        "text/plain",
        JSON.stringify({ id: props.entry.entity.id, index: props.entry.index }),
      );
    }
  }

  function handleDragOver(e: DragEvent): void {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    setDragOverIndex(props.entry.index);
  }

  function handleDragLeave(): void {
    setDragOverIndex(null);
  }

  function handleDrop(e: DragEvent): void {
    e.preventDefault();
    setDragOverIndex(null);
    const raw = e.dataTransfer?.getData("text/plain") ?? "";
    let parsed: { id: number; index: number } | null = null;
    try {
      const v = JSON.parse(raw) as unknown;
      if (
        v !== null &&
        typeof v === "object" &&
        typeof (v as { id: unknown }).id === "number" &&
        typeof (v as { index: unknown }).index === "number"
      ) {
        parsed = v as { id: number; index: number };
      }
    } catch {
      // Bad payload — ignore.
    }
    if (parsed === null || parsed.index === props.entry.index) return;
    const newIndex = Math.max(
      0,
      parsed.index > props.entry.index ? props.entry.index : props.entry.index - 1,
    );
    reorderEntity(parsed.id as EntityId, newIndex);
  }

  return (
    <>
      <Show when={isDragTarget()}>
        <div class="library-panel__drop-indicator" />
      </Show>
      <div
        class="library-row"
        classList={{ "library-row--selected": isSelected() }}
        style={{ "padding-left": `${6 + props.entry.depth * 16}px` }}
        onClick={handleClick}
        onDblClick={() => beginEntityRename(props.entry.entity.id)}
        onContextMenu={(e) => {
          e.preventDefault();
          props.onContextMenu(e, props.entry.entity.id);
        }}
        draggable={true}
        onDragStart={handleDragStart}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        data-testid={`entity-row-${props.entry.entity.id}`}
      >
        {/* Chevron for Custom entities with multiple states */}
        <Show when={hasMultipleStates()} fallback={<div class="library-row__chevron-spacer" />}>
          <button
            class="library-row__chevron"
            onClick={(e) => {
              e.stopPropagation();
              setStatesExpanded((v) => !v);
            }}
            title={statesExpanded() ? "Collapse states" : "Expand states"}
          >
            <svg width="8" height="8" viewBox="0 0 8 8" fill="currentColor">
              <Show
                when={statesExpanded()}
                fallback={
                  <path
                    d="M2 1 L6 4 L2 7"
                    stroke="currentColor"
                    stroke-width="1.5"
                    fill="none"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                }
              >
                <path
                  d="M1 2 L4 6 L7 2"
                  stroke="currentColor"
                  stroke-width="1.5"
                  fill="none"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              </Show>
            </svg>
          </button>
        </Show>

        {/* Reference-sheet badge before the kind icon. */}
        <SheetEditorButton entity={props.entry.entity} />
        <AnchorBadge entity={props.entry.entity} />

        {/* Kind icon */}
        <EntityKindIcon kind={props.entry.entity.kind} />

        {/* Name or rename input */}
        <Show
          when={!isRenaming()}
          fallback={
            <InlineRenameInput
              initialName={props.entry.entity.name}
              onCommit={(name) => commitEntityRename(props.entry.entity.id, name)}
              onCancel={cancelRename}
            />
          }
        >
          <span class="library-row__name">{props.entry.entity.name}</span>
          <Show when={isCustom()}>
            <span class="library-row__kind-badge">
              {(props.entry.entity.kind as { kind: "Custom"; value: string }).value}
            </span>
          </Show>
          <TagChips entity={props.entry.entity} />
        </Show>
      </div>

      {/* State sub-rows for Custom entities */}
      <Show when={hasMultipleStates() && statesExpanded()}>
        <For each={states()}>
          {(state) => (
            <StateRow
              entityId={props.entry.entity.id}
              stateId={state.id}
              stateName={state.state_name}
              depth={props.entry.depth + 1}
              entity={props.entry.entity}
            />
          )}
        </For>
      </Show>
    </>
  );
};

// ── Anchor badge ─────────────────────────────────────────────────────────────

type SheetEditorButtonProps = {
  entity: Entity;
};

const SheetEditorButton: Component<SheetEditorButtonProps> = (props) => {
  const isSpriteEntity = (): boolean => props.entity.content.type === "Sprites";

  return (
    <Show when={isSpriteEntity()}>
      <button
        class="library-row__anchor-badge"
        data-testid={`library-row-sheet-editor-${props.entity.id}`}
        title="Open reference sheet editor"
        aria-label="Open reference sheet editor"
        onClick={(event) => {
          event.stopPropagation();
          openSheetEditor(props.entity.id);
        }}
      >
        ✦
      </button>
    </Show>
  );
};

type AnchorBadgeProps = {
  entity: Entity;
};

// Shown on sprite entities with an embedded reference sheet. The popover
// lazy-loads the resolved payload via `library_get_anchor_payload` on hover
// with a 250ms debounce,
// so passive scrolling past a long entity list doesn't fire dozens of
// resolves. Backend payload caching keeps repeat hovers cheap.
const AnchorBadge: Component<AnchorBadgeProps> = (props) => {
  const isVisible = (): boolean => {
    const content = props.entity.content;
    return (
      content.type === "Sprites" &&
      content.value.reference_sheet !== null &&
      content.value.reference_sheet !== undefined
    );
  };

  const [showPopover, setShowPopover] = createSignal(false);
  const [payload, setPayload] = createSignal<AnchorPayload | null>(null);
  let hoverTimer: ReturnType<typeof setTimeout> | null = null;
  // Stale-resolve guard: bumped on every hover transition and on cleanup.
  // The mouse-leave handler clears the debounce timer but can't cancel an
  // already-in-flight IPC, so a late `.then` would otherwise show a ghost
  // tooltip with no cursor nearby. Matches the refreshToken pattern at
  // library-state.ts:121-133.
  let hoverToken = 0;

  function invalidateHover(): void {
    hoverToken += 1;
  }

  function clearTimer(): void {
    if (hoverTimer !== null) {
      clearTimeout(hoverTimer);
      hoverTimer = null;
    }
  }

  function handleMouseEnter(): void {
    clearTimer();
    invalidateHover();
    const myToken = hoverToken;
    // Snapshot the entity id before the debounce; the row could be
    // re-keyed under us if the list reorders during the 250ms window
    // and we don't want a late resolve to clobber the wrong tooltip.
    const entityId = props.entity.id;
    hoverTimer = setTimeout(() => {
      libraryGetAnchorPayload(entityId)
        .then((p) => {
          if (myToken !== hoverToken) return;
          setPayload(p);
          setShowPopover(true);
        })
        .catch((err: unknown) => reportCommandFailure("library_get_anchor_payload", err));
    }, 250);
  }

  function handleMouseLeave(): void {
    clearTimer();
    invalidateHover();
    setShowPopover(false);
  }

  onCleanup(() => {
    clearTimer();
    invalidateHover();
  });

  return (
    <Show when={isVisible()}>
      <span
        class="library-row__anchor-badge"
        data-testid={`library-row-anchor-${props.entity.id}`}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
        title="Reference sheet"
      >
        <svg
          width="12"
          height="12"
          viewBox="0 0 12 12"
          fill="none"
          stroke="currentColor"
          stroke-width="1.3"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <circle cx="6" cy="3" r="1.5" />
          <path d="M6 4.5 V10" />
          <path d="M3 8 Q6 11 9 8" />
          <path d="M2.5 7 H4 M8 7 H9.5" />
        </svg>
        <Show when={showPopover() && payload() !== null}>
          <div
            class="library-row__anchor-tooltip"
            role="tooltip"
            data-testid={`library-row-anchor-tooltip-${props.entity.id}`}
          >
            <img
              class="library-row__anchor-tooltip-img"
              src={`data:${payload()!.mime};base64,${payload()!.image_b64}`}
              alt="Reference sheet preview"
            />
          </div>
        </Show>
      </span>
    </Show>
  );
};

// ── Tag chip strip ───────────────────────────────────────────────────────────

type TagChipsProps = {
  entity: Entity;
};

// Renders confirmed tags inline with the entity name and any pending VLM
// suggestions with ✓ / ✗ buttons. Confirmed chips are read-only at this
// layer — full tag management (rename, color, untag) is a future Category B
// PR; we deliberately avoid wiring untag here so the surface stays focused
// on the auto-tag accept/reject flow.
const TagChips: Component<TagChipsProps> = (props) => {
  // Lookup goes through the shared `tagsById` map (rebuilt once per
  // library refresh) so each chip-strip render is O(entity.tags) instead
  // of O(entity.tags * tags()). createMemo also re-runs only when its
  // tracked sources change, not on every parent re-render.
  const confirmedTagDefs = createMemo<TagDefinition[]>(() => {
    const ids = props.entity.tags ?? [];
    if (ids.length === 0) return [];
    const lookup = tagsById();
    const out: TagDefinition[] = [];
    for (const id of ids) {
      const def = lookup.get(id);
      if (def !== undefined) out.push(def);
    }
    return out;
  });

  const pendingTagDefs = createMemo<TagDefinition[]>(() => {
    const pending = pendingTagSuggestions().get(props.entity.id) ?? [];
    if (pending.length === 0) return [];
    const lookup = tagsById();
    const confirmed = new Set(props.entity.tags ?? []);
    const out: TagDefinition[] = [];
    for (const id of pending) {
      // Drop suggestions that have since been confirmed via accept; the
      // entity refresh promotes them into `entity.tags` and the ✓ click
      // already removed the pending entry.
      if (confirmed.has(id)) continue;
      const def = lookup.get(id);
      if (def !== undefined) out.push(def);
    }
    return out;
  });

  function handleAccept(e: MouseEvent, tagId: TagId): void {
    e.stopPropagation();
    const entityId = props.entity.id;
    libraryAcceptSuggestedTag(entityId, tagId)
      .then(() => {
        removePendingSuggestion(entityId, tagId);
        refreshLibrary();
      })
      .catch((err: unknown) => reportCommandFailure("library_accept_suggested_tag", err));
  }

  function handleReject(e: MouseEvent, tagId: TagId): void {
    e.stopPropagation();
    const entityId = props.entity.id;
    libraryRejectSuggestedTag(entityId, tagId)
      .then(() => {
        removePendingSuggestion(entityId, tagId);
        refreshLibrary();
      })
      .catch((err: unknown) => reportCommandFailure("library_reject_suggested_tag", err));
  }

  return (
    <Show when={confirmedTagDefs().length > 0 || pendingTagDefs().length > 0}>
      <div
        class="library-row__tag-chips"
        data-testid={`library-row-tag-chips-${props.entity.id}`}
        onClick={(e) => e.stopPropagation()}
      >
        <For each={confirmedTagDefs()}>
          {(tag) => (
            <span class="library-row__tag-chip" title={tag.name}>
              {tag.name}
            </span>
          )}
        </For>
        <For each={pendingTagDefs()}>
          {(tag) => (
            <span
              class="library-row__tag-chip library-row__tag-chip--pending"
              title={`Suggested: ${tag.name}`}
            >
              <span class="library-row__tag-chip-label">{tag.name}</span>
              <button
                class="library-row__tag-chip-action library-row__tag-chip-action--accept"
                onClick={(e) => handleAccept(e, tag.id)}
                title="Accept suggestion"
                aria-label={`Accept tag ${tag.name}`}
              >
                ✓
              </button>
              <button
                class="library-row__tag-chip-action library-row__tag-chip-action--reject"
                onClick={(e) => handleReject(e, tag.id)}
                title="Reject suggestion"
                aria-label={`Reject tag ${tag.name}`}
              >
                ✗
              </button>
            </span>
          )}
        </For>
      </div>
    </Show>
  );
};

// ── State sub-row ────────────────────────────────────────────────────────────

type StateRowProps = {
  entityId: EntityId;
  stateId: StateId;
  stateName: string;
  depth: number;
  entity: Entity;
};

const StateRow: Component<StateRowProps> = (props) => {
  const isRenaming = () => {
    const t = renamingTarget();
    return t?.kind === "state" && t.entityId === props.entityId && t.stateId === props.stateId;
  };

  const stateCount = () => {
    const c = props.entity.content;
    if (c.type !== "Sprites") return 0;
    return c.value.states.length;
  };

  function handleClick(): void {
    setActiveTarget({
      type: "state",
      value: { entity_id: props.entityId, state_id: props.stateId },
    });
  }

  return (
    <div
      class="library-row library-row--state"
      style={{ "padding-left": `${6 + props.depth * 16}px` }}
      onClick={handleClick}
      onDblClick={() => beginStateRename(props.entityId, props.stateId)}
      onContextMenu={(e) => {
        e.preventDefault();
        // Simple inline context: rename / delete (if not the last state)
        const canDelete = stateCount() > 1;
        const menu = document.createElement("div");
        menu.className = "ctx-menu";
        menu.style.left = `${e.clientX}px`;
        menu.style.top = `${e.clientY}px`;

        function tearDown(): void {
          if (document.body.contains(menu)) document.body.removeChild(menu);
          document.removeEventListener("pointerdown", dismiss, { capture: true });
        }

        function dismiss(ev: PointerEvent): void {
          if (!menu.contains(ev.target as Node)) tearDown();
        }

        const renameBtn = document.createElement("button");
        renameBtn.className = "ctx-menu__item";
        renameBtn.textContent = "Rename";
        renameBtn.onclick = () => {
          beginStateRename(props.entityId, props.stateId);
          tearDown();
        };
        menu.appendChild(renameBtn);

        if (canDelete) {
          const sep = document.createElement("div");
          sep.className = "ctx-menu__separator";
          menu.appendChild(sep);

          const deleteBtn = document.createElement("button");
          deleteBtn.className = "ctx-menu__item ctx-menu__item--danger";
          deleteBtn.textContent = "Delete state";
          deleteBtn.onclick = () => {
            deleteState(props.entityId, props.stateId);
            tearDown();
          };
          menu.appendChild(deleteBtn);
        }

        document.addEventListener("pointerdown", dismiss, { capture: true });
        document.body.appendChild(menu);
      }}
      data-testid={`state-row-${props.stateId}`}
    >
      <div class="library-row__chevron-spacer" />
      <svg
        class="library-row__kind-icon library-row__kind-icon--state"
        width="12"
        height="12"
        viewBox="0 0 12 12"
        fill="none"
        stroke="var(--text-disabled)"
        stroke-width="1.2"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <path d="M2 6 L5 9 L10 3" />
      </svg>

      <Show
        when={!isRenaming()}
        fallback={
          <InlineRenameInput
            initialName={props.stateName}
            onCommit={(name) => commitStateRename(props.entityId, props.stateId, name)}
            onCancel={cancelRename}
          />
        }
      >
        <span class="library-row__name library-row__name--state">{props.stateName}</span>
      </Show>
    </div>
  );
};

// ── Inline rename input ──────────────────────────────────────────────────────

type RenameInputProps = {
  initialName: string;
  onCommit: (name: string) => void;
  onCancel: () => void;
};

const InlineRenameInput: Component<RenameInputProps> = (props) => {
  let inputRef!: HTMLInputElement;
  let finalized = false;
  const [value, setValue] = createSignal(props.initialName);

  createEffect(() => {
    inputRef?.focus();
    inputRef?.select();
  });

  function finalizeCommit(v: string): void {
    if (finalized) return;
    finalized = true;
    props.onCommit(v);
  }

  function finalizeCancel(): void {
    if (finalized) return;
    finalized = true;
    props.onCancel();
  }

  function handleKeyDown(e: KeyboardEvent): void {
    if (e.key === "Enter") {
      e.preventDefault();
      finalizeCommit(value());
    } else if (e.key === "Escape") {
      e.preventDefault();
      finalizeCancel();
    }
  }

  onCleanup(() => {
    if (finalized) return;
    const v = value().trim();
    if (v.length > 0) {
      finalizeCommit(v);
    } else {
      finalizeCancel();
    }
  });

  return (
    <input
      ref={inputRef}
      class="library-row__rename-input"
      value={value()}
      onInput={(e) => setValue(e.currentTarget.value)}
      onKeyDown={handleKeyDown}
      onBlur={() => finalizeCommit(value())}
      onClick={(e) => e.stopPropagation()}
    />
  );
};

// ── Entity kind icon ─────────────────────────────────────────────────────────

import type { EntityKind } from "../lib/types";

type KindIconProps = { kind: EntityKind };

const EntityKindIcon: Component<KindIconProps> = (props) => {
  const color = (): string => {
    switch (props.kind.kind) {
      case "Tileset":
        return "var(--success)";
      case "Tilemap":
        return "var(--info)";
      default:
        return "var(--accent)";
    }
  };

  return (
    <svg
      class="library-row__kind-icon"
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      stroke={color()}
      stroke-width="1.2"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <Show when={props.kind.kind === "Tileset"}>
        <rect x="2" y="2" width="5" height="5" />
        <rect x="9" y="2" width="5" height="5" />
        <rect x="2" y="9" width="5" height="5" />
        <rect x="9" y="9" width="5" height="5" />
      </Show>
      <Show when={props.kind.kind === "Tilemap"}>
        <rect x="2" y="4" width="12" height="9" rx="1" />
        <path d="M2 7 H14 M2 10 H14 M6 4 V13 M10 4 V13" />
      </Show>
      <Show when={props.kind.kind === "Custom"}>
        <rect x="2" y="2" width="12" height="12" rx="2" fill={color()} fill-opacity="0.15" />
        <path d="M5 8 H11 M8 5 V11" />
      </Show>
    </svg>
  );
};

export default LibraryPanel;
