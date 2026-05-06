// Tag bar row rendered above the frame column headers.
//
// Existing tags appear as coloured ranges. Pointer-drag on empty space
// creates a new tag; right-click on a tag deletes it; left-click jumps
// to the tag's first frame.

import { type Component, For, Show, createMemo } from "solid-js";
import type { FrameIndex, FrameTag, SpriteId } from "../lib/types";
import {
  activeFrameIndex,
  setActiveFrameIndex,
  scheduleViewportSync,
} from "../canvas/canvas-state";
import {
  createTag,
  deleteTag,
  frameTags,
  setTagDragState,
  tagDragState,
  uniqueTagName,
} from "./timeline-state";

// Fixed visual constant — must match TimelinePanel.FRAME_WIDTH.
export const FRAME_WIDTH = 24;

// Deterministic colour palette (8 choices, cycling on hash of name).
const TAG_COLORS = [
  "#4e8ef7",
  "#e85f5c",
  "#4ec97e",
  "#e8b84e",
  "#b14ef7",
  "#e87d4e",
  "#4ecfe8",
  "#e8e04e",
] as const;

function tagColor(name: string): string {
  let h = 0;
  for (let i = 0; i < name.length; i++) h = (h * 31 + name.charCodeAt(i)) >>> 0;
  return TAG_COLORS[h % TAG_COLORS.length] ?? TAG_COLORS[0]!;
}

function frameIndexFromX(clientX: number, containerLeft: number, scrollLeft: number): FrameIndex {
  return Math.max(0, Math.floor((clientX - containerLeft + scrollLeft) / FRAME_WIDTH));
}

type Props = {
  readonly spriteId: SpriteId | null;
  readonly frameCount: number;
  readonly scrollLeft: number;
  readonly containerLeft: number;
};

const FrameTagBar: Component<Props> = (props) => {
  const drag = tagDragState;

  const visibleTags = createMemo(() => {
    const count = props.frameCount;
    return frameTags().filter((t) => t.range.end >= 0 && t.range.start < count);
  });

  // ── Pointer events for drag-to-create ───────────────────────────────────────

  function handlePointerDown(e: PointerEvent): void {
    if (e.button !== 0) return;
    const idx = frameIndexFromX(e.clientX, props.containerLeft, props.scrollLeft);
    const clamped = Math.min(idx, Math.max(0, props.frameCount - 1));
    setTagDragState({ kind: "dragging", startFrame: clamped, endFrame: clamped });
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    e.preventDefault();
  }

  function handlePointerMove(e: PointerEvent): void {
    const d = drag();
    if (d.kind !== "dragging") return;
    const idx = frameIndexFromX(e.clientX, props.containerLeft, props.scrollLeft);
    const clamped = Math.min(idx, Math.max(0, props.frameCount - 1));
    setTagDragState({ kind: "dragging", startFrame: d.startFrame, endFrame: clamped });
  }

  function handlePointerUp(e: PointerEvent): void {
    const d = drag();
    if (d.kind !== "dragging") return;
    const id = props.spriteId;
    if (id !== null) {
      const lo = Math.min(d.startFrame, d.endFrame);
      const hi = Math.max(d.startFrame, d.endFrame);
      createTag(id, uniqueTagName(), { start: lo, end: hi });
    }
    setTagDragState({ kind: "none" });
    (e.currentTarget as Element).releasePointerCapture(e.pointerId);
  }

  // Solid tuple event handlers: (data, event). Tag is first, event is second.
  function handleTagClick(tag: FrameTag, e: MouseEvent): void {
    e.stopPropagation();
    if (tag.range.start !== activeFrameIndex()) {
      setActiveFrameIndex(tag.range.start);
      scheduleViewportSync();
    }
  }

  function handleTagContextMenu(tag: FrameTag, e: MouseEvent): void {
    e.preventDefault();
    e.stopPropagation();
    const id = props.spriteId;
    if (id !== null) deleteTag(id, tag.name);
  }

  const dragPreview = createMemo(() => {
    const d = drag();
    if (d.kind !== "dragging") return null;
    return { lo: Math.min(d.startFrame, d.endFrame), hi: Math.max(d.startFrame, d.endFrame) };
  });

  return (
    <div
      class="tl-tag-bar"
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
    >
      <For each={visibleTags()}>
        {(tag) => (
          <div
            class="tl-tag"
            style={{
              left: `${tag.range.start * FRAME_WIDTH}px`,
              width: `${(tag.range.end - tag.range.start + 1) * FRAME_WIDTH}px`,
              "background-color": tagColor(tag.name),
            }}
            title={`${tag.name} (${tag.loop_direction})`}
            onClick={[handleTagClick, tag]}
            onContextMenu={[handleTagContextMenu, tag]}
          >
            <span class="tl-tag__label">{tag.name}</span>
          </div>
        )}
      </For>

      <Show when={dragPreview()}>
        {(preview) => (
          <div
            class="tl-tag tl-tag--preview"
            style={{
              left: `${preview().lo * FRAME_WIDTH}px`,
              width: `${(preview().hi - preview().lo + 1) * FRAME_WIDTH}px`,
            }}
          />
        )}
      </Show>
    </div>
  );
};

export default FrameTagBar;
