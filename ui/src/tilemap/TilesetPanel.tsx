// Tileset browser panel.
//
// Shows the active tileset as a grid of numbered tile cells.  Click a tile to
// select it for painting; right-click to edit its collision and animation
// properties.  Tile pixel previews are not yet rendered — the pixel-buffer
// pipeline (S01) must land first.  Until then, each cell shows its tile index.

import { For, Show, createSignal, createMemo, onCleanup, type Component } from "solid-js";
import type { CollisionShape, TileProperties, Tileset } from "../lib/types";
import {
  selectedTileIndex,
  setSelectedTileIndex,
  selectedTileFlags,
  setSelectedTileFlags,
  TILE_FLIP_X,
  TILE_FLIP_Y,
  TILE_FLIP_DIAGONAL,
} from "./tilemap-state";

// ── Context-menu ──────────────────────────────────────────────────────────

interface CtxMenu {
  tileIndex: number;
  props: TileProperties;
  x: number;
  y: number;
}

// ── Helpers ───────────────────────────────────────────────────────────────

function collisionLabel(shape: CollisionShape | undefined): string {
  switch (shape) {
    case "full":
      return "Full";
    case "none":
    default:
      return "None";
  }
}

// Cycles to the next CollisionShape value.
function nextCollision(shape: CollisionShape | undefined): CollisionShape {
  return shape === "full" ? "none" : "full";
}

// ── Component ─────────────────────────────────────────────────────────────

interface TilesetPanelProps {
  tileset: Tileset;
  // Called when the user edits a tile's properties via the context menu.
  onTilePropertiesChange?: (tileIndex: number, props: TileProperties) => void;
}

const TilesetPanel: Component<TilesetPanelProps> = (props) => {
  const [ctxMenu, setCtxMenu] = createSignal<CtxMenu | null>(null);

  // Close the context menu on any global click.
  function onWindowClick() {
    setCtxMenu(null);
  }
  window.addEventListener("click", onWindowClick);
  onCleanup(() => window.removeEventListener("click", onWindowClick));

  // Tile count includes the empty tile at index 0.
  const tileCount = createMemo(() => props.tileset.tile_count);

  // Indices to render: 0 is the empty tile (shown distinctly), rest are paintable.
  const tileIndices = createMemo(() => Array.from({ length: tileCount() }, (_, i) => i));

  const tileW = createMemo(() => props.tileset.tile_size.width);
  const tileH = createMemo(() => props.tileset.tile_size.height);
  const displayLabel = createMemo(() => props.tileset.base_index);

  function tileLabel(index: number): string {
    if (index === 0) return "∅"; // empty-set symbol for the empty tile
    return String(index - 1 + displayLabel());
  }

  function onTileClick(index: number, e: MouseEvent) {
    e.stopPropagation();
    if (e.button !== 0) return;
    setSelectedTileIndex(index);
  }

  function onTileContextMenu(index: number, e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    const propsForTile: TileProperties = props.tileset.properties?.[index] ?? {};
    setCtxMenu({ tileIndex: index, props: propsForTile, x: e.clientX, y: e.clientY });
  }

  // ── Flags toolbar ────────────────────────────────────────────────────

  function toggleFlag(flag: number) {
    setSelectedTileFlags((f) => f ^ flag);
  }

  function hasFlag(flag: number): boolean {
    return (selectedTileFlags() & flag) !== 0;
  }

  // ── Context-menu actions ──────────────────────────────────────────────

  function onToggleCollision() {
    const menu = ctxMenu();
    if (!menu) return;
    const newCollision = nextCollision(menu.props.collision);
    const updated: TileProperties = { ...menu.props, collision: newCollision };
    setCtxMenu({ ...menu, props: updated });
    props.onTilePropertiesChange?.(menu.tileIndex, updated);
  }

  return (
    <div class="ts-panel">
      {/* Flip / rotation toolbar */}
      <div class="ts-panel__toolbar">
        <button
          class="ts-panel__flag-btn"
          classList={{ "ts-panel__flag-btn--active": hasFlag(TILE_FLIP_X) }}
          title="Flip horizontal"
          onClick={() => toggleFlag(TILE_FLIP_X)}
        >
          H
        </button>
        <button
          class="ts-panel__flag-btn"
          classList={{ "ts-panel__flag-btn--active": hasFlag(TILE_FLIP_Y) }}
          title="Flip vertical"
          onClick={() => toggleFlag(TILE_FLIP_Y)}
        >
          V
        </button>
        <button
          class="ts-panel__flag-btn"
          classList={{ "ts-panel__flag-btn--active": hasFlag(TILE_FLIP_DIAGONAL) }}
          title="Diagonal flip (90° rotation)"
          onClick={() => toggleFlag(TILE_FLIP_DIAGONAL)}
        >
          D
        </button>
        <span class="ts-panel__tile-size">
          {tileW()}&#xd7;{tileH()}px
        </span>
      </div>

      {/* Tile grid */}
      <div class="ts-panel__grid" role="listbox" aria-label="Tile picker">
        <For each={tileIndices()}>
          {(index) => (
            <button
              class="ts-panel__tile"
              classList={{
                "ts-panel__tile--empty": index === 0,
                "ts-panel__tile--selected": selectedTileIndex() === index,
              }}
              role="option"
              aria-selected={selectedTileIndex() === index}
              title={index === 0 ? "Empty tile" : `Tile ${tileLabel(index)}`}
              onClick={(e) => onTileClick(index, e)}
              onContextMenu={(e) => onTileContextMenu(index, e)}
            >
              <span class="ts-panel__tile-label">{tileLabel(index)}</span>
            </button>
          )}
        </For>
      </div>

      {/* Context menu */}
      <Show when={ctxMenu()}>
        {(menu) => (
          <div
            class="ts-panel__ctx-menu"
            style={{ left: `${menu().x}px`, top: `${menu().y}px` }}
            onClick={(e) => e.stopPropagation()}
          >
            <div class="ts-panel__ctx-header">Tile {tileLabel(menu().tileIndex)}</div>
            <button class="ts-panel__ctx-item" onClick={onToggleCollision}>
              Collision: {collisionLabel(menu().props.collision)}
            </button>
            <button
              class="ts-panel__ctx-item ts-panel__ctx-item--disabled"
              disabled
              title="Animation editing arrives with the pixel-buffer pipeline (S01)"
            >
              Animation — pending S01
            </button>
          </div>
        )}
      </Show>
    </div>
  );
};

export default TilesetPanel;
