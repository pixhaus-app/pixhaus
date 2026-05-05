// Right-click context menu for layer rows.
//
// Appears at the cursor position when a layer is right-clicked. Dismisses on
// click-outside, Escape, or after any action. Menu items that operate on the
// active sprite are wired through layer-state helpers.

import { type Component, Show, createEffect, onCleanup } from "solid-js";
import type { Layer, LayerId, SpriteId } from "../lib/types";
import {
  addLayer,
  beginRename,
  convertLayerToGroup,
  convertLayerToTilemap,
  deleteLayer,
  flattenVisibleLayers,
  layers,
  mergeLayerDown,
  mergeSelectedLayers,
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

  function action(fn: () => void): () => void {
    return () => {
      fn();
      props.onClose();
    };
  }

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
        <button class="ctx-menu__item" onClick={action(() => beginRename(props.target!.layerId))}>
          Rename
        </button>
        <button
          class="ctx-menu__item"
          onClick={action(() => addLayer(props.spriteId, `${layer()!.name} copy`))}
        >
          Duplicate
        </button>

        <div class="ctx-menu__separator" />

        <button
          class="ctx-menu__item"
          onClick={action(() => mergeLayerDown(props.spriteId, props.target!.layerId))}
          disabled={!canMergeDown()}
          title={canMergeDown() ? undefined : "No layer below to merge into"}
        >
          Merge Down
        </button>
        <button
          class="ctx-menu__item"
          onClick={action(() => mergeSelectedLayers(props.spriteId, selectedLayerIds()))}
          disabled={!multiSelected()}
          title={multiSelected() ? undefined : "Select two or more layers to merge"}
        >
          Merge Selected
        </button>
        <button class="ctx-menu__item" onClick={action(() => flattenVisibleLayers(props.spriteId))}>
          Flatten Visible
        </button>

        <div class="ctx-menu__separator" />

        <button
          class="ctx-menu__item"
          onClick={action(() => convertLayerToGroup(props.spriteId, props.target!.layerId))}
          disabled={isGroup()}
          title={isGroup() ? "Layer is already a group" : undefined}
        >
          Convert to Group
        </button>
        <button
          class="ctx-menu__item"
          onClick={action(() => convertLayerToTilemap(props.spriteId, props.target!.layerId))}
          disabled={isTilemap()}
          title={isTilemap() ? "Layer is already a tilemap layer" : undefined}
        >
          Convert to Tilemap Layer
        </button>

        <div class="ctx-menu__separator" />

        <button
          class="ctx-menu__item ctx-menu__item--danger"
          onClick={action(() => deleteLayer(props.spriteId, props.target!.layerId))}
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
