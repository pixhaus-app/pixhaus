// Tilemap side panel.
//
// Shown when a tilemap layer is the active layer.  Contains two tabs:
//   Tileset — tile grid browser and flip/rotation controls (TilesetPanel)
//   Rules   — autotile rule editor (AutotileRuleEditor)
//
// The panel also hosts the tile-tool toggle (pencil / erase) and the
// autotile-mode checkbox.

import { Show, createSignal, createEffect, createMemo, type Component } from "solid-js";
import type { TileProperties } from "../lib/types";
import {
  activeTilemapCtx,
  tilemapTool,
  setTilemapTool,
  autotileMode,
  setAutotileMode,
  resetTileSelection,
} from "./tilemap-state";
import TilesetPanel from "./TilesetPanel";
import AutotileRuleEditor from "./AutotileRuleEditor";

// ── Tab type ──────────────────────────────────────────────────────────────

type Tab = "tileset" | "rules";

// ── Component ─────────────────────────────────────────────────────────────

const TilemapPanel: Component = () => {
  const [activeTab, setActiveTab] = createSignal<Tab>("tileset");

  const ctx = activeTilemapCtx;
  const tileset = createMemo(() => ctx()?.tileset ?? null);

  // Reset selection when the context switches to a different tileset.
  createEffect(() => {
    const c = ctx();
    if (c) resetTileSelection();
  });

  // Placeholder handler — tile property edits are persisted once S06 lands.
  function onTilePropertiesChange(_tileIndex: number, _props: TileProperties) {
    // TODO(S06): persist tile property changes to project via IPC
  }

  return (
    <Show when={tileset() !== null}>
      <div class="tilemap-panel">
        {/* Header */}
        <div class="tilemap-panel__header">
          <span class="tilemap-panel__title">{tileset()!.name}</span>
          <span class="tilemap-panel__sub">tileset</span>
        </div>

        {/* Tool row */}
        <div class="tilemap-panel__tool-row">
          <button
            class="tilemap-panel__tool-btn"
            classList={{ "tilemap-panel__tool-btn--active": tilemapTool() === "pencil" }}
            title="Pencil — place tile (P)"
            onClick={() => setTilemapTool("pencil")}
          >
            Pencil
          </button>
          <button
            class="tilemap-panel__tool-btn"
            classList={{ "tilemap-panel__tool-btn--active": tilemapTool() === "erase" }}
            title="Eraser — clear cell (E)"
            onClick={() => setTilemapTool("erase")}
          >
            Erase
          </button>
          <label class="tilemap-panel__autotile-toggle">
            <input
              type="checkbox"
              checked={autotileMode()}
              onChange={(e) => setAutotileMode(e.currentTarget.checked)}
            />
            Autotile
          </label>
        </div>

        {/* Tabs */}
        <div class="tilemap-panel__tabs" role="tablist">
          <button
            class="tilemap-panel__tab"
            classList={{ "tilemap-panel__tab--active": activeTab() === "tileset" }}
            role="tab"
            aria-selected={activeTab() === "tileset"}
            onClick={() => setActiveTab("tileset")}
          >
            Tileset
          </button>
          <button
            class="tilemap-panel__tab"
            classList={{ "tilemap-panel__tab--active": activeTab() === "rules" }}
            role="tab"
            aria-selected={activeTab() === "rules"}
            onClick={() => setActiveTab("rules")}
          >
            Rules
          </button>
        </div>

        {/* Tab body */}
        <div class="tilemap-panel__body">
          <Show when={activeTab() === "tileset"}>
            <TilesetPanel tileset={tileset()!} onTilePropertiesChange={onTilePropertiesChange} />
          </Show>
          <Show when={activeTab() === "rules"}>
            <AutotileRuleEditor />
          </Show>
        </div>
      </div>
    </Show>
  );
};

export default TilemapPanel;
