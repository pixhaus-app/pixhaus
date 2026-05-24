// Layer panel — the right-side panel showing the layer tree for the active sprite.
//
// Tree view: layers rendered top-to-bottom (reversed from Rust's bottom-to-top order).
// Group children are indented and toggleable. Virtual scroll keeps 500+ layers at 60fps.
// Drag-to-reorder is handled per-row in LayerRow. The context menu is portal-mounted
// so it isn't clipped by panel overflow.

import { type Component, For, Show, createMemo, createSignal, onCleanup } from "solid-js";
import { activeSpriteId, activeLayerId } from "../canvas/canvas-state";
import {
  addLayer,
  dragOverIndex,
  flattenLayers,
  isGroupExpanded,
  layers,
  nextAutoName,
  selectedLayerIds,
  selectLayer,
} from "./layer-state";
import { Plus } from "lucide-solid";
import LayerRow from "./LayerRow";
import LayerContextMenu, { type ContextMenuTarget } from "./LayerContextMenu";
import TilesetPickerDialog from "./TilesetPickerDialog";
import type { LayerId } from "../lib/types";

// Default row height in px for inactive rows.
const ROW_HEIGHT = 32;
// Row height for the active (foreground) layer — shows blend mode + opacity strip.
const ACTIVE_ROW_HEIGHT = 56;

// ── Visible-range search helpers ─────────────────────────────────────────────
//
// rowOffsets is a strictly-increasing prefix sum (length n+1, where n is the
// number of rows). offsets[i] is row i's top edge, offsets[i+1] its bottom.
// Both helpers return an index in [0, n].

/**
 * Returns the smallest row index `i` in `[0, n)` whose bottom edge
 * (`offsets[i + 1]`) is strictly greater than `target`. Used to find the
 * first row whose bottom is below the overscan-top threshold (i.e. the
 * first row that's at least partially visible).
 */
function lowerBoundOffsetGt(offsets: readonly number[], n: number, target: number): number {
  let lo = 0;
  let hi = n;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if ((offsets[mid + 1] ?? 0) > target) hi = mid;
    else lo = mid + 1;
  }
  return lo;
}

/**
 * Returns the smallest row index `i` in `[0, n]` whose top edge
 * (`offsets[i]`) is at or past `target`. Used to find the first row
 * whose top is below the overscan-bottom threshold (i.e. the first row
 * that's no longer visible).
 */
function lowerBoundOffsetGte(offsets: readonly number[], n: number, target: number): number {
  let lo = 0;
  let hi = n;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if ((offsets[mid] ?? 0) >= target) hi = mid;
    else lo = mid + 1;
  }
  return lo;
}

// ── Panel component ──────────────────────────────────────────────────────────

type Props = {
  /** When true, render without the outer `.layer-panel` wrapper or its title bar —
   *  the right-rail accordion provides its own chrome. */
  inRail?: boolean;
};

const LayerPanel: Component<Props> = (props) => {
  const spriteId = activeSpriteId;

  // The layer list is backed by a createBackendQuery keyed on the active
  // sprite (see layer-state), so it loads and reloads on sprite change with
  // no panel-side effect. The old onMount/effect refetch lived here only
  // because the cache had no source of its own.
  const flatEntries = createMemo(() => flattenLayers(layers(), isGroupExpanded));

  // ── Virtual scroll ─────────────────────────────────────────────────────────

  const [scrollTop, setScrollTop] = createSignal(0);
  const [containerHeight, setContainerHeight] = createSignal(400);

  // The scroll container lives inside two <Show> conditionals (sprite
  // present + non-empty layer list), so it isn't in the DOM at the
  // outer onMount tick — the previous onMount-then-observe(scrollContainer)
  // pattern threw "parameter 1 is not of type 'Element'" the first time
  // a project loaded. Use a callback ref so the observer attaches when
  // the element actually mounts and detaches on unmount, regardless of
  // how many times the inner Show toggles.
  const resizeObserver = new ResizeObserver((entries) => {
    const entry = entries[0];
    if (entry) setContainerHeight(entry.contentRect.height);
  });
  const scrollContainerRef = (el: HTMLDivElement): void => {
    resizeObserver.observe(el);
  };
  onCleanup(() => resizeObserver.disconnect());

  // Per-row heights: the active row is 56px; all others are 32px.
  // Recomputes whenever the active layer or flat entry list changes.
  const rowHeights = createMemo(() => {
    const activeId = activeLayerId();
    return flatEntries().map((e) => (e.layer.id === activeId ? ACTIVE_ROW_HEIGHT : ROW_HEIGHT));
  });

  // Prefix-sum of row heights: rowOffsets[i] is the top of row i.
  // Length is flatEntries().length + 1; the last element is totalHeight.
  const rowOffsets = createMemo(() => {
    const heights = rowHeights();
    const offsets = new Array<number>(heights.length + 1);
    offsets[0] = 0;
    for (let i = 0; i < heights.length; i++) {
      offsets[i + 1] = (offsets[i] ?? 0) + (heights[i] ?? ROW_HEIGHT);
    }
    return offsets;
  });

  const totalHeight = createMemo(() => {
    const offsets = rowOffsets();
    return offsets[offsets.length - 1] ?? 0;
  });

  const visibleRange = createMemo(() => {
    const st = scrollTop();
    const ch = containerHeight();
    const offsets = rowOffsets();
    const n = flatEntries().length;
    // rowOffsets is a strictly-increasing prefix sum (length n+1), so
    // the first/last visible rows can be located in O(log n) with a
    // binary search instead of the prior linear scan — relevant once
    // sprites accumulate hundreds of layers.
    const overscanTop = Math.max(0, st - ROW_HEIGHT * 2);
    const overscanBottom = st + ch + ROW_HEIGHT * 2;
    // Find the first row whose bottom edge is past overscanTop.
    // offsets[i+1] is row i's bottom, so we want the smallest i with
    // offsets[i+1] > overscanTop.
    const start = lowerBoundOffsetGt(offsets, n, overscanTop);
    // Find the first row whose top edge is at/past overscanBottom.
    // offsets[i] is row i's top, so we want the smallest i with
    // offsets[i] >= overscanBottom.
    const end = lowerBoundOffsetGte(offsets, n, overscanBottom);
    return { start, end: Math.min(n, end) };
  });

  // Pairs each visible entry with its visual index in the full list so
  // that the absolute top position can be looked up in rowOffsets[].
  const visibleItems = createMemo(() => {
    const { start, end } = visibleRange();
    return flatEntries()
      .slice(start, end)
      .map((entry, i) => ({ entry, visualIndex: start + i }));
  });

  // ── Context menu ───────────────────────────────────────────────────────────

  const [contextTarget, setContextTarget] = createSignal<ContextMenuTarget | null>(null);

  function handleContextMenu(e: MouseEvent, layerId: LayerId): void {
    // Right-click on a row outside the current selection switches the
    // selection to just that row — matches Photoshop / Aseprite. Right-
    // click on a row inside the selection leaves the selection alone, so
    // multi-select operations from the menu hit every selected row.
    if (!selectedLayerIds().has(layerId)) {
      selectLayer(layerId, false);
    }
    setContextTarget({ x: e.clientX, y: e.clientY, layerId });
  }

  // ── Add layer ──────────────────────────────────────────────────────────────

  function handleAddLayer(): void {
    const id = spriteId();
    if (id !== null) addLayer(id, nextAutoName(layers(), "Layer"));
  }

  return (
    <div
      class="layer-panel"
      classList={{ "layer-panel--in-rail": !!props.inRail }}
      data-testid="layer-panel"
    >
      <Show when={!props.inRail}>
        <div class="layer-panel__header">
          <span class="layer-panel__title">Layers</span>
          <div class="layer-panel__header-actions">
            <button
              class="layer-panel__icon-btn"
              data-testid="layer-add"
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
          </div>
        </div>
      </Show>
      <Show when={props.inRail}>
        <div class="layer-panel__rail-actions">
          <button
            class="layer-panel__add-btn"
            data-testid="layer-add"
            onClick={handleAddLayer}
            disabled={spriteId() === null}
            title="New raster layer"
          >
            <Plus size={13} aria-hidden={true} />
            New layer
          </button>
        </div>
      </Show>

      <Show
        when={spriteId() !== null}
        fallback={<div class="layer-panel__empty">Open a project to see layers.</div>}
      >
        <Show
          when={flatEntries().length > 0}
          fallback={<div class="layer-panel__empty">No layers yet.</div>}
        >
          {/* Virtual scroll container.
              The dragover/drop handlers preventDefault unconditionally so
              the cursor stays in "move" mode anywhere in the panel — without
              this, motion through the active row's overflowing blend/opacity
              strip (which contains a <select> and <input type=range>) trips
              the browser's default reject and shows the prohibited icon.
              Per-row handleDragOver still runs via bubbling and updates the
              drop indicator. */}
          <div
            ref={scrollContainerRef}
            class="layer-panel__scroll"
            onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
            onDragOver={(e) => e.preventDefault()}
            onDrop={(e) => e.preventDefault()}
          >
            {/* Spacer sets the total scrollable height */}
            <div style={{ height: `${totalHeight()}px`, position: "relative" }}>
              {/* Render only the visible slice */}
              <For each={visibleItems()}>
                {({ entry, visualIndex }) => (
                  <div
                    style={{
                      position: "absolute",
                      top: `${rowOffsets()[visualIndex] ?? 0}px`,
                      width: "100%",
                    }}
                    onDragOver={(e) => e.preventDefault()}
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

      {/* Tileset picker for Convert to Tilemap Layer. The dialog renders
          itself only when its target signal is set. */}
      <TilesetPickerDialog />
    </div>
  );
};

export default LayerPanel;
