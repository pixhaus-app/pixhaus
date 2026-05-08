// Tag bar row rendered above the frame column headers.
//
// Existing tags appear as coloured ranges. Pointer-drag on empty space
// creates a new tag; right-click on a tag opens a context menu (Rename,
// Delete); left-click jumps to the tag's first frame. An empty-state hint
// renders inside the bar so the drag-to-create affordance is discoverable
// before the first tag exists.

import { type Component, For, Show, createMemo, createSignal } from "solid-js";
import { Portal } from "solid-js/web";
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
  renameTag,
  setTagDragState,
  tagDragState,
  uniqueTagName,
} from "./timeline-state";
import ModalInput from "../components/ModalInput";
import { clampMenuToViewport } from "../lib/utils/menu-position";

// Fixed visual constant — must match the `--tl-frame-w` CSS variable in
// index.css. Virtual-scroll math, scrub-head positioning, tag positioning
// and frameIndex-from-clientX all derive from this.
export const FRAME_WIDTH = 36;

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

type ContextMenuState = {
  readonly x: number;
  readonly y: number;
  readonly tagName: string;
};

const FrameTagBar: Component<Props> = (props) => {
  const drag = tagDragState;

  const [ctxMenu, setCtxMenu] = createSignal<ContextMenuState | null>(null);
  const [renameTarget, setRenameTarget] = createSignal<string | null>(null);

  const visibleTags = createMemo(() => {
    const count = props.frameCount;
    return frameTags().filter((t) => t.range.end >= 0 && t.range.start < count);
  });

  const isEmpty = createMemo(
    () => visibleTags().length === 0 && drag().kind === "none" && props.frameCount > 0,
  );

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
    setCtxMenu({ x: e.clientX, y: e.clientY, tagName: tag.name });
  }

  function closeCtxMenu(): void {
    setCtxMenu(null);
  }

  function startRename(): void {
    const m = ctxMenu();
    if (m === null) return;
    setRenameTarget(m.tagName);
    closeCtxMenu();
  }

  function commitRename(newName: string): void {
    const id = props.spriteId;
    const oldName = renameTarget();
    if (id === null || oldName === null) return;
    const trimmed = newName.trim();
    if (trimmed === "" || trimmed === oldName) {
      setRenameTarget(null);
      return;
    }
    renameTag(id, oldName, trimmed);
    setRenameTarget(null);
  }

  function deleteFromMenu(): void {
    const m = ctxMenu();
    const id = props.spriteId;
    if (m !== null && id !== null) deleteTag(id, m.tagName);
    closeCtxMenu();
  }

  function validateRename(value: string): string | null {
    const trimmed = value.trim();
    if (trimmed.length === 0) return "Name is required";
    const oldName = renameTarget();
    if (trimmed === oldName) return null;
    if (frameTags().some((t) => t.name === trimmed)) return "A tag with this name already exists";
    return null;
  }

  const dragPreview = createMemo(() => {
    const d = drag();
    if (d.kind !== "dragging") return null;
    return { lo: Math.min(d.startFrame, d.endFrame), hi: Math.max(d.startFrame, d.endFrame) };
  });

  return (
    <>
      <div
        class="tl-tag-bar"
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
      >
        <Show when={isEmpty()}>
          <div class="tl-tag-bar__hint">Drag to create a tag</div>
        </Show>

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

      <Show when={ctxMenu()}>
        {(m) => (
          <Portal>
            <div class="tl-ctx-backdrop" onClick={closeCtxMenu} />
            <div
              class="tl-ctx-menu"
              ref={clampMenuToViewport(m().x, m().y)}
              style={{ left: `${m().x}px`, top: `${m().y}px` }}
            >
              <button class="tl-ctx-menu__item" onClick={startRename}>
                Rename tag
              </button>
              <button class="tl-ctx-menu__item" onClick={deleteFromMenu}>
                Delete tag
              </button>
            </div>
          </Portal>
        )}
      </Show>

      <ModalInput
        open={renameTarget() !== null}
        title="Rename tag"
        label="Tag name"
        initialValue={renameTarget() ?? ""}
        submitLabel="Rename"
        validate={validateRename}
        onSubmit={commitRename}
        onClose={() => setRenameTarget(null)}
      />
    </>
  );
};

export default FrameTagBar;
