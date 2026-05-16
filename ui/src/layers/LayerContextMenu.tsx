// Right-click context menu for layer rows.
//
// Appears at the cursor position when a layer is right-clicked. Dismisses on
// click-outside, Escape, or after any action. Menu items that operate on the
// active sprite are wired through layer-state helpers.

import { type Component, Show, createEffect, createSignal, onCleanup } from "solid-js";
import { ConfirmDialog } from "../lib/ui/ConfirmDialog";
import type { Layer, LayerId, SpriteId } from "../lib/types";
import {
  addLayer,
  beginRename,
  deleteLayers,
  flattenVisibleLayers,
  layers,
  mergeLayerDown,
  mergeSelectedLayers,
  openTilesetPicker,
  selectedLayerIds,
  wrapLayersInGroup,
} from "./layer-state";

export type ContextMenuTarget = {
  x: number;
  y: number;
  layerId: LayerId;
};

type Props = {
  target: ContextMenuTarget | null;
  spriteId: SpriteId;
  onClose: () => void;
};

type PendingDelete = {
  spriteId: SpriteId;
  ids: LayerId[];
  title: string;
  message: string;
};

const LayerContextMenu: Component<Props> = (props) => {
  let menuRef!: HTMLDivElement;
  const [pendingDelete, setPendingDelete] = createSignal<PendingDelete | null>(null);

  const targetLayer = (): Layer | undefined => layers().find((l) => l.id === props.target?.layerId);

  createEffect(() => {
    if (props.target === null) return;

    function handlePointerDown(e: PointerEvent): void {
      if (menuRef && !menuRef.contains(e.target as Node)) {
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

  const layer = () => targetLayer();
  const isGroup = () => layer()?.kind.kind === "group";
  const isTilemap = () => layer()?.kind.kind === "tilemap";
  // Merge Down requires a layer below this one in the flat list.
  const canMergeDown = () => {
    const l = layer();
    if (!l) return false;
    const all = layers();
    const idx = all.findIndex((x) => x.id === l.id);
    return idx > 0;
  };
  const multiSelected = () => selectedLayerIds().size >= 2;
  // Convert to Group accepts non-group layers; reject if the target or
  // any other selected layer is already a group (the backend would
  // reject anyway, but disabling here saves a round-trip).
  const canConvertToGroup = () => {
    const sel = selectedLayerIds();
    const ids = sel.size > 0 ? [...sel] : layer() ? [layer()!.id] : [];
    if (ids.length === 0) return false;
    const all = layers();
    return ids.every((id) => {
      const l = all.find((x) => x.id === id);
      return l !== undefined && l.kind.kind !== "group";
    });
  };
  // Delete must leave at least one layer on the sprite. With multi-select
  // this means the selection cannot cover every layer.
  const canDelete = () => {
    const total = layers().length;
    const sel = selectedLayerIds();
    const count = sel.size > 0 ? sel.size : 1;
    return total - count >= 1;
  };

  /**
   * Cancel any pending layer delete confirmation.
   *
   * Clears the internal pending-delete state so the confirmation dialog is closed.
   */
  function cancelDelete(): void {
    setPendingDelete(null);
  }

  /**
   * Confirm and execute a pending layer deletion.
   *
   * If a pending delete exists, clears the pending delete state and deletes the specified layers from the associated sprite.
   */
  function confirmDelete(): void {
    const pending = pendingDelete();
    if (pending === null) return;
    setPendingDelete(null);
    deleteLayers(pending.spriteId, pending.ids);
  }

  return (
    <>
      <Show when={props.target !== null && layer() !== undefined}>
        <div
          ref={menuRef}
          class="ctx-menu"
          style={{
            left: `${props.target!.x}px`,
            top: `${props.target!.y}px`,
          }}
          onContextMenu={(e) => e.preventDefault()}
        >
          <button
            class="ctx-menu__item"
            data-testid="layer-ctx-rename"
            onClick={() => {
              beginRename(props.target!.layerId);
              props.onClose();
            }}
          >
            Rename
          </button>
          <button
            class="ctx-menu__item"
            data-testid="layer-ctx-duplicate"
            onClick={() => {
              addLayer(props.spriteId, `${layer()!.name} copy`);
              props.onClose();
            }}
          >
            Duplicate
          </button>

          <div class="ctx-menu__separator" />

          <button
            class="ctx-menu__item"
            data-testid="layer-ctx-merge-down"
            onClick={() => {
              mergeLayerDown(props.spriteId, props.target!.layerId);
              props.onClose();
            }}
            disabled={!canMergeDown()}
            title={canMergeDown() ? undefined : "No layer below to merge into"}
          >
            Merge Down
          </button>
          <button
            class="ctx-menu__item"
            data-testid="layer-ctx-merge-selected"
            onClick={() => {
              mergeSelectedLayers(props.spriteId, selectedLayerIds());
              props.onClose();
            }}
            disabled={!multiSelected()}
            title={multiSelected() ? undefined : "Select two or more layers to merge"}
          >
            Merge Selected
          </button>
          <button
            class="ctx-menu__item"
            data-testid="layer-ctx-flatten-visible"
            onClick={() => {
              flattenVisibleLayers(props.spriteId);
              props.onClose();
            }}
          >
            Flatten Visible
          </button>

          <div class="ctx-menu__separator" />

          <button
            class="ctx-menu__item"
            data-testid="layer-ctx-convert-group"
            onClick={() => {
              // After R2-B1 the right-clicked target is always in
              // selectedLayerIds — operate on the whole selection so
              // multi-select wraps every selected layer into one group.
              const sel = selectedLayerIds();
              const ids = sel.size > 0 ? [...sel] : [props.target!.layerId];
              wrapLayersInGroup(props.spriteId, ids);
              props.onClose();
            }}
            disabled={!canConvertToGroup()}
            title={
              isGroup()
                ? "Layer is already a group"
                : !canConvertToGroup()
                  ? "One or more selected layers are already groups"
                  : undefined
            }
          >
            Convert to Group
          </button>
          <button
            class="ctx-menu__item"
            data-testid="layer-ctx-convert-tilemap"
            onClick={() => {
              // Tileset selection is asynchronous (the user may need to
              // create one), so open the picker dialog and let it call
              // convertLayerToTilemap once a tileset id is chosen.
              openTilesetPicker(props.spriteId, props.target!.layerId);
              props.onClose();
            }}
            disabled={isTilemap()}
            title={isTilemap() ? "Layer is already a tilemap layer" : undefined}
          >
            Convert to Tilemap Layer
          </button>

          <div class="ctx-menu__separator" />

          <button
            class="ctx-menu__item ctx-menu__item--danger"
            data-testid="layer-ctx-delete"
            onClick={() => {
              // After R2-B1 the right-clicked layer is in selectedLayerIds.
              // Multi-select deletes every selected layer; the single-layer
              // case keeps the named-layer prompt for clarity.
              const spriteId = props.spriteId;
              const sel = selectedLayerIds();
              const ids = sel.size > 0 ? [...sel] : [props.target!.layerId];
              const message =
                ids.length > 1
                  ? `Delete ${ids.length} layers? This can be undone.`
                  : `Delete "${layer()?.name ?? "this layer"}"? This can be undone.`;
              // Close the menu before the dialog opens so the menu doesn't
              // sit on top of the modal while the user reads it.
              props.onClose();
              setPendingDelete({
                spriteId,
                ids,
                title: ids.length > 1 ? "Delete layers" : "Delete layer",
                message,
              });
            }}
            disabled={!canDelete()}
            title={!canDelete() ? "Cannot delete every layer on the sprite" : undefined}
          >
            Delete
          </button>
        </div>
      </Show>

      <ConfirmDialog
        open={pendingDelete() !== null}
        title={pendingDelete()?.title ?? "Delete layer"}
        message={pendingDelete()?.message ?? ""}
        confirmLabel={pendingDelete()?.ids.length === 1 ? "Delete" : "Delete layers"}
        onConfirm={confirmDelete}
        onCancel={cancelDelete}
        testId="layer-delete-confirm"
      />
    </>
  );
};

export default LayerContextMenu;
