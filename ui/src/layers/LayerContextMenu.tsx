// Right-click context menu for layer rows.
//
// Appears at the cursor position when a layer is right-clicked. Dismisses on
// click-outside, Escape, or after any action. Menu items that operate on the
// active sprite are wired through layer-state helpers.

import { type Component, Show, createEffect, onCleanup } from "solid-js";
import type { Layer, LayerId, SpriteId } from "../lib/types";
import { addLayer, beginRename, deleteLayer, layers } from "./layer-state";

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
