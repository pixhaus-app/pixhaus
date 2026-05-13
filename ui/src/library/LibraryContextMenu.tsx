// Context menu for library tree rows (entities and groups).
//
// Mounts at viewport coordinates; dismisses on click-outside or Escape.

import { type Component, Show, createEffect, onCleanup } from "solid-js";
import { confirm } from "../lib/dialog";
import { libraryAutoTagEntity } from "../lib/commands/library";
import { pushToast } from "../lib/toast/toast-state";
import { reportCommandFailure } from "../lib/utils/errors";
import type { Entity, EntityGroup, EntityId, GroupId } from "../lib/types";
import {
  addPendingSuggestions,
  beginEntityRename,
  beginGroupRename,
  deleteEntity,
  deleteGroup,
  entities,
  groups,
  refreshLibrary,
} from "./library-state";

// ── target types ─────────────────────────────────────────────────────────────

export type EntityContextTarget = {
  kind: "entity";
  x: number;
  y: number;
  entityId: EntityId;
};

export type GroupContextTarget = {
  kind: "group";
  x: number;
  y: number;
  groupId: GroupId;
};

export type LibraryContextTarget = EntityContextTarget | GroupContextTarget;

type Props = {
  target: LibraryContextTarget | null;
  onClose: () => void;
  onAddState: (entityId: EntityId) => void;
  onMoveToGroup: (entityId: EntityId) => void;
  onCreateGroup: () => void;
};

const LibraryContextMenu: Component<Props> = (props) => {
  let menuRef!: HTMLDivElement;

  createEffect(() => {
    if (props.target === null) return;

    function handlePointerDown(e: PointerEvent): void {
      if (!menuRef.contains(e.target as Node)) {
        props.onClose();
      }
    }

    function handleKeyDown(e: KeyboardEvent): void {
      if (e.key === "Escape") props.onClose();
    }

    document.addEventListener("pointerdown", handlePointerDown, { capture: true });
    document.addEventListener("keydown", handleKeyDown);
    onCleanup(() => {
      document.removeEventListener("pointerdown", handlePointerDown, { capture: true });
      document.removeEventListener("keydown", handleKeyDown);
    });
  });

  const targetEntity = (): Entity | undefined => {
    const t = props.target;
    if (t?.kind !== "entity") return undefined;
    return entities().find((e) => e.id === t.entityId);
  };

  const targetGroup = (): EntityGroup | undefined => {
    if (props.target?.kind !== "group") return undefined;
    return groups().find((g) => g.id === (props.target as GroupContextTarget).groupId);
  };

  const isCustomEntity = () => targetEntity()?.kind.kind === "Custom";

  // The auto-tag verb is grounded against entity name + existing tags only —
  // Tilesets and Tilemaps don't carry the kind of semantic content the VLM
  // can usefully tag, so we hide the action for them.
  const isAutoTaggable = () => {
    const k = targetEntity()?.kind.kind;
    return k === "Custom" || k === "Reference";
  };

  function handleSuggestTags(entityId: EntityId): void {
    pushToast({ kind: "info", title: "Auto-tagging…" });
    libraryAutoTagEntity(entityId)
      .then((suggestions) => {
        const ids = suggestions.map((t) => t.id);
        if (ids.length > 0) {
          addPendingSuggestions(entityId, ids);
        }
        // The Rust command persists the suggestions onto the entity and
        // adds any newly-named tags to the project tag list — refresh so
        // the row's chip strip can resolve names from the cached `tags()`.
        refreshLibrary();
        pushToast({
          kind: "success",
          title:
            ids.length === 0
              ? "No new tag suggestions."
              : `Suggested ${ids.length} tag${ids.length === 1 ? "" : "s"}.`,
        });
      })
      .catch((err: unknown) => reportCommandFailure("library_auto_tag_entity", err));
  }

  function handleEntityDelete(): void {
    const entity = targetEntity();
    if (!entity) return;
    props.onClose();
    void confirm(`Delete "${entity.name}"? This cannot be undone.`, {
      title: "Delete Entity",
      kind: "warning",
    }).then((ok) => {
      if (!ok) return;
      deleteEntity(entity.id);
    });
  }

  function handleGroupDelete(): void {
    const group = targetGroup();
    if (!group) return;
    props.onClose();
    void confirm(
      `Delete group "${group.name}"?\nChoose "OK" to keep entities ungrouped, or cancel to abort.`,
      {
        title: "Delete Group",
        kind: "warning",
      },
    ).then((ok) => {
      if (!ok) return;
      deleteGroup(group.id, true);
    });
  }

  return (
    <Show when={props.target !== null}>
      <div
        ref={menuRef}
        class="ctx-menu"
        style={{
          left: `${props.target!.x}px`,
          top: `${props.target!.y}px`,
        }}
        onContextMenu={(e) => e.preventDefault()}
      >
        {/* ── Entity menu ── */}
        <Show when={props.target?.kind === "entity" && targetEntity() !== undefined}>
          <button
            class="ctx-menu__item"
            onClick={() => {
              const t = props.target as EntityContextTarget;
              beginEntityRename(t.entityId);
              props.onClose();
            }}
          >
            Rename
          </button>

          <Show when={isCustomEntity()}>
            <button
              class="ctx-menu__item"
              onClick={() => {
                const t = props.target as EntityContextTarget;
                props.onAddState(t.entityId);
                props.onClose();
              }}
            >
              Add state
            </button>
          </Show>

          <Show when={isAutoTaggable()}>
            <button
              class="ctx-menu__item"
              data-testid="ctx-menu-suggest-tags"
              onClick={() => {
                const t = props.target as EntityContextTarget;
                handleSuggestTags(t.entityId);
                props.onClose();
              }}
            >
              Suggest tags
            </button>
          </Show>

          <div class="ctx-menu__separator" />

          <button
            class="ctx-menu__item"
            onClick={() => {
              const t = props.target as EntityContextTarget;
              props.onMoveToGroup(t.entityId);
              props.onClose();
            }}
          >
            Move to group...
          </button>

          <div class="ctx-menu__separator" />

          <button class="ctx-menu__item ctx-menu__item--danger" onClick={handleEntityDelete}>
            Delete
          </button>
        </Show>

        {/* ── Group menu ── */}
        <Show when={props.target?.kind === "group" && targetGroup() !== undefined}>
          <button
            class="ctx-menu__item"
            onClick={() => {
              beginGroupRename((props.target as GroupContextTarget).groupId);
              props.onClose();
            }}
          >
            Rename
          </button>

          <button
            class="ctx-menu__item"
            onClick={() => {
              props.onCreateGroup();
              props.onClose();
            }}
          >
            New subgroup
          </button>

          <div class="ctx-menu__separator" />

          <button class="ctx-menu__item ctx-menu__item--danger" onClick={handleGroupDelete}>
            Delete group
          </button>
        </Show>
      </div>
    </Show>
  );
};

export default LibraryContextMenu;
