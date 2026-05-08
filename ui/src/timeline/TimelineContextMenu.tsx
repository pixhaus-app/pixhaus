// Right-click context menu for frame operations in the timeline.
//
// Portal-mounted so it is not clipped by the panel's overflow: hidden.

import { type Component, For, Show } from "solid-js";
import { Portal } from "solid-js/web";
import type { FrameIndex, SpriteId } from "../lib/types";
import { activeFrameIndex } from "../canvas/canvas-state";
import { clampMenuToViewport } from "../lib/utils/menu-position";
import {
  addFrame,
  copyFrame,
  deleteFrames,
  duplicateFrame,
  pasteFrame,
  copiedFrameIndex,
  reverseSelectedFrames,
  selectedFrames,
} from "./timeline-state";

export type ContextMenuTarget = {
  readonly x: number;
  readonly y: number;
  readonly frameIndex: FrameIndex;
};

type MenuItem =
  | { readonly kind: "sep" }
  | { readonly kind: "item"; readonly label: string; readonly handler: () => void };

type Props = {
  readonly target: ContextMenuTarget | null;
  readonly spriteId: SpriteId;
  readonly onClose: () => void;
};

const TimelineContextMenu: Component<Props> = (props) => {
  function effectiveSelection(): ReadonlySet<FrameIndex> {
    const sel = selectedFrames();
    const t = props.target;
    if (t !== null && sel.has(t.frameIndex)) return sel;
    return new Set([t?.frameIndex ?? activeFrameIndex()]);
  }

  function buildItems(): MenuItem[] {
    const id = props.spriteId;
    const eff = effectiveSelection();
    const canPaste = copiedFrameIndex() !== null;
    const multi = eff.size > 1;
    // Operate on the right-click target by default. Menu items that
    // intentionally consume the active frame (none currently) can fall
    // back to `activeFrameIndex()` explicitly.
    const target = props.target?.frameIndex ?? activeFrameIndex();
    const close = props.onClose;

    return [
      {
        kind: "item",
        label: "Insert frame",
        handler: () => {
          addFrame(id);
          close();
        },
      },
      {
        kind: "item",
        label: multi ? "Delete frames" : "Delete frame",
        handler: () => {
          deleteFrames(id, eff);
          close();
        },
      },
      {
        kind: "item",
        label: "Duplicate frame",
        handler: () => {
          duplicateFrame(id, target);
          close();
        },
      },
      { kind: "sep" },
      {
        kind: "item",
        label: "Copy",
        handler: () => {
          copyFrame(target);
          close();
        },
      },
      {
        kind: "item",
        label: canPaste ? "Paste" : "Paste (empty clipboard)",
        handler: () => {
          if (canPaste) pasteFrame(id);
          close();
        },
      },
      { kind: "sep" },
      {
        kind: "item",
        label: "Reverse selected",
        handler: () => {
          if (multi) reverseSelectedFrames(id, eff);
          close();
        },
      },
    ];
  }

  return (
    <Show when={props.target}>
      {(t) => (
        <Portal>
          <div class="tl-ctx-backdrop" onClick={props.onClose} />
          <div
            class="tl-ctx-menu"
            ref={clampMenuToViewport(t().x, t().y)}
            style={{ left: `${t().x}px`, top: `${t().y}px` }}
          >
            <For each={buildItems()}>
              {(item) =>
                item.kind === "sep" ? (
                  <div class="tl-ctx-menu__sep" />
                ) : (
                  <button class="tl-ctx-menu__item" onClick={item.handler}>
                    {item.label}
                  </button>
                )
              }
            </For>
          </div>
        </Portal>
      )}
    </Show>
  );
};

export default TimelineContextMenu;
