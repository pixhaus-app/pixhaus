// Right-click context menu for layer rows.
//
// Appears at the cursor position when a layer is right-clicked. Dismisses on
// click-outside, Escape, or after any action. Menu items that operate on the
// active sprite are wired through layer-state helpers.

import { type Component, Show, createEffect, onCleanup } from "solid-js";
import { confirm } from "@tauri-apps/plugin-dialog";
import type { Layer, LayerId, SpriteId } from "../lib/types";
import {
  addLayer,
  beginRename,
  convertLayerToGroup,
  deleteLayer,
  flattenVisibleLayers,
  layers,
  mergeLayerDown,
  mergeSelectedLayers,
  openTilesetPicker,
  selectedLayerIds,
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

const LayerContextMenu: Component<Props> = (props) => {
  let menuRef!: HTMLDivElement;

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

  return (
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
          onClick={() => {
            beginRename(props.target!.layerId);
            props.onClose();
          }}
        >
          Rename
        </button>
        <button
          class="ctx-menu__item"
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
          onClick={() => {
            convertLayerToGroup(props.spriteId, props.target!.layerId);
            props.onClose();
          }}
          disabled={isGroup()}
          title={isGroup() ? "Layer is already a group" : undefined}
        >
          Convert to Group
        </button>
        <button
          class="ctx-menu__item"
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
          onClick={() => {
            const target = props.target!.layerId;
            const name = layer()?.name ?? "this layer";
            const spriteId = props.spriteId;
            // Close the menu before the dialog opens so the menu doesn't
            // sit on top of the modal while the user reads it.
            props.onClose();
            void confirm(`Delete "${name}"? This can be undone.`, {
              title: "Delete Layer",
              kind: "warning",
            }).then((ok) => {
              if (ok) deleteLayer(spriteId, target);
            });
          }}
          disabled={layers().length <= 1}
          title={layers().length <= 1 ? "Cannot delete the only layer" : undefined}
        >
          Delete
        </button>
      </div>
    </Show>
  );
};

export default LayerContextMenu;
