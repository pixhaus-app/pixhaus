// Layer panel — the right-side panel showing the layer tree for the active sprite.
//
// Tree view: layers rendered top-to-bottom (reversed from Rust's bottom-to-top order).
// Group children are indented and toggleable. Virtual scroll keeps 500+ layers at 60fps.
// Drag-to-reorder is handled per-row in LayerRow. The context menu is portal-mounted
// so it isn't clipped by panel overflow.

import {
  type Component,
  For,
  Show,
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
} from "solid-js";
import { activeSpriteId } from "../canvas/canvas-state";
import {
  addLayer,
  dragOverIndex,
  flattenLayers,
  isGroupExpanded,
  layers,
  refreshLayers,
  setLayerPanelVisible,
} from "./layer-state";
import LayerRow from "./LayerRow";
import LayerContextMenu, { type ContextMenuTarget } from "./LayerContextMenu";
import type { LayerId } from "../lib/types";

// Row height in px — fixed for the virtual list calculation.
const ROW_HEIGHT = 32;

// ── Panel component ──────────────────────────────────────────────────────────

const LayerPanel: Component = () => {
  const spriteId = activeSpriteId;

  // Single effect handles both the initial load and every sprite change.
  // The previous version had onMount + a tracking effect, which fired
  // refreshLayers() twice on mount and could race when the sprite-id
  // signal changed before the first IPC settled.
  createEffect(() => {
    spriteId(); // track
    refreshLayers();
  });

  const flatEntries = createMemo(() => flattenLayers(layers(), isGroupExpanded));

  // ── Virtual scroll ─────────────────────────────────────────────────────────

  let scrollContainer!: HTMLDivElement;
  const [scrollTop, setScrollTop] = createSignal(0);
  const [containerHeight, setContainerHeight] = createSignal(400);

  let resizeObserver: ResizeObserver;
  onMount(() => {
    resizeObserver = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (entry) setContainerHeight(entry.contentRect.height);
    });
    resizeObserver.observe(scrollContainer);
  });
  onCleanup(() => resizeObserver?.disconnect());

  const totalHeight = createMemo(() => flatEntries().length * ROW_HEIGHT);

  const visibleRange = createMemo(() => {
    const st = scrollTop();
    const ch = containerHeight();
    const start = Math.max(0, Math.floor(st / ROW_HEIGHT) - 2);
    const end = Math.min(flatEntries().length, Math.ceil((st + ch) / ROW_HEIGHT) + 2);
    return { start, end };
  });

  // ── Context menu ───────────────────────────────────────────────────────────

  const [contextTarget, setContextTarget] = createSignal<ContextMenuTarget | null>(null);

  function handleContextMenu(e: MouseEvent, layerId: LayerId): void {
    setContextTarget({ x: e.clientX, y: e.clientY, layerId });
  }

  // ── Add layer ──────────────────────────────────────────────────────────────

  function handleAddLayer(): void {
    const id = spriteId();
    if (id !== null) addLayer(id, "Layer");
  }

  return (
    <div class="layer-panel">
      <div class="layer-panel__header">
        <span class="layer-panel__title">Layers</span>
        <div class="layer-panel__header-actions">
          <button
            class="layer-panel__icon-btn"
            onClick={handleAddLayer}
            disabled={spriteId() === null}
            title="New raster layer"
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
            class="layer-panel__icon-btn"
            onClick={() => setLayerPanelVisible(false)}
            title="Close layer panel"
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

      <Show
        when={spriteId() !== null}
        fallback={<div class="layer-panel__empty">Open a project to see layers.</div>}
      >
        <Show
          when={flatEntries().length > 0}
          fallback={<div class="layer-panel__empty">No layers yet.</div>}
        >
          {/* Virtual scroll container */}
          <div
            ref={scrollContainer}
            class="layer-panel__scroll"
            onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
          >
            {/* Spacer sets the total scrollable height */}
            <div style={{ height: `${totalHeight()}px`, position: "relative" }}>
              {/* Render only the visible slice */}
              <For each={flatEntries().slice(visibleRange().start, visibleRange().end)}>
                {(entry) => (
                  <div
                    style={{
                      position: "absolute",
                      top: `${(visibleRange().start + flatEntries().slice(visibleRange().start, visibleRange().end).indexOf(entry)) * ROW_HEIGHT}px`,
                      width: "100%",
                    }}
                  >
                    <Show when={dragOverIndex() === entry.index}>
                      <div class="layer-panel__drop-indicator" />
                    </Show>
                    <LayerRow
                      layer={entry.layer}
                      layerIndex={entry.index}
                      depth={entry.depth}
                      spriteId={spriteId()!}
                      onContextMenu={handleContextMenu}
                    />
                  </div>
                )}
              </For>
            </div>
          </div>
        </Show>
      </Show>

      {/* Context menu — rendered outside the scroll container to avoid clipping */}
      <LayerContextMenu
        target={contextTarget()}
        spriteId={spriteId() ?? 0}
        onClose={() => setContextTarget(null)}
      />
    </div>
  );
};

export default LayerPanel;
