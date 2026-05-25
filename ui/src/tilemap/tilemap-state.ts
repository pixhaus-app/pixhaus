// Tilemap editor state.
//
// This module owns all signals that drive the tilemap sub-UI: which tileset is
// active, which tile the brush is carrying, the active tool, and whether
// autotile resolution is enabled. The state is module-level (not component-
// scoped) because the tileset panel, the rule editor, and the canvas input
// handler all read and write the same signals.

import { createEffect, untrack } from "solid-js";
import { createStore } from "solid-js/store";
import type {
  AutotileKind,
  AutotileRule,
  LayerId,
  SpriteId,
  TileIndex,
  Tileset,
  TilesetId,
} from "../lib/types";
import { activeTarget } from "../canvas/canvas-state";
import { layers } from "../layers/layer-state";
import { tilesetList } from "../lib/commands/tilesets";
import { createBackendQuery } from "../lib/sync/query";

// ── TileFlags bit constants ────────────────────────────────────────────────
// Mirrors core/src/project/tilemap.rs — TileFlags is a transparent u8.

export const TILE_FLIP_X = 1 << 0;
export const TILE_FLIP_Y = 1 << 1;
export const TILE_FLIP_DIAGONAL = 1 << 2;

// ── Active tilemap context ─────────────────────────────────────────────────

/**
 * Set when a tilemap layer is foregrounded; null otherwise.
 * `installTilemapCtxSync` (in tilemap-ctx-sync.ts) keeps this in sync
 * with the active layer; direct `setActiveTilemapCtx` calls are reserved
 * for in-panel actions that mutate the same context (rename, switch).
 */
export type ActiveTilemapCtx = {
  layerId: LayerId;
  tilesetId: TilesetId;
  tileset: Tileset;
};

// UI-only tilemap state in one store. Reads are tilemapUi.activeTilemapCtx,
// tilemapUi.selectedTileIndex, etc.; writes go through the setters below.
// (The autotile rule list stays its own array store — see autotileRules.)
interface TilemapUiState {
  /** Set when a tilemap layer is foregrounded; null otherwise.
   * installTilemapCtxSync keeps this in sync with the active layer; direct
   * setActiveTilemapCtx calls are for in-panel edits (rename, switch). */
  activeTilemapCtx: ActiveTilemapCtx | null;
  /** Selected tile index; 0 is the empty-tile sentinel, default 1 (first
   * paintable) so the pencil starts in paint mode. */
  selectedTileIndex: number;
  /** Flags to apply when placing a tile (flip X / Y / diagonal). */
  selectedTileFlags: number;
  tilemapTool: TilemapTool;
  /** When true the brush resolves the autotile rule set after each stroke. */
  autotileMode: boolean;
  /** Autotile kind for the active tileset, mirrored from Tileset.autotile so
   * the editor re-renders without a round-trip. Survives tab switches. */
  localAutotileKind: AutotileKind | null;
  autotileDefaultTile: TileIndex;
}

export const [tilemapUi, setTilemapUi] = createStore<TilemapUiState>({
  activeTilemapCtx: null,
  selectedTileIndex: 1,
  selectedTileFlags: 0,
  tilemapTool: "pencil",
  autotileMode: false,
  localAutotileKind: null,
  autotileDefaultTile: 0 as TileIndex,
});

export const setActiveTilemapCtx = (v: ActiveTilemapCtx | null): void =>
  setTilemapUi("activeTilemapCtx", v);
export const setSelectedTileIndex = (v: number): void => setTilemapUi("selectedTileIndex", v);
export const setSelectedTileFlags = (v: number): void => setTilemapUi("selectedTileFlags", v);
export const setTilemapTool = (v: TilemapTool): void => setTilemapUi("tilemapTool", v);
export const setAutotileMode = (v: boolean): void => setTilemapUi("autotileMode", v);
export const setLocalAutotileKind = (v: AutotileKind | null): void =>
  setTilemapUi("localAutotileKind", v);
export const setAutotileDefaultTile = (v: TileIndex): void =>
  setTilemapUi("autotileDefaultTile", v);

// Tilesets for the active sprite. The source returns the sprite id only when
// a tilemap layer is foregrounded, so this stays as lazy as the old ad-hoc
// fetch (no tileset IPC for plain raster sprites) while createResource drops
// the stale-response handling the bridge used to do with a token counter.
// Tileset mutations invalidate "tilesets" so the next layer switch sees fresh
// data; in-place edits also push the updated tileset onto activeTilemapCtx.
const tilesetsQuery = createBackendQuery<SpriteId, Tileset[]>({
  key: "tilesets",
  source: () => {
    const sid = activeTarget.spriteId;
    const lid = activeTarget.layerId;
    if (sid === null || lid === null) return null;
    const layer = layers().find((l) => l.id === lid);
    return layer !== undefined && layer.kind.kind === "tilemap" ? sid : null;
  },
  fetch: (spriteId) => tilesetList(spriteId),
  initial: [],
  errorTitle: "Failed to load tilesets",
});

export const tilesets = tilesetsQuery.data;

/**
 * One-way derivation: active layer -> activeTilemapCtx. Must be called from a
 * reactive root (Shell's setup). Reads the cached tilesets query, so there is
 * no async fetch or stale-token bookkeeping here anymore — when the query is
 * still loading, ctx stays null and this re-runs once the tilesets arrive.
 *
 * The tilemap panel and tile-paint path are gated behind activeTilemapCtx;
 * without this it stays null, leaving the panel hidden and the pencil a no-op
 * on tilemap layers.
 */
export function installTilemapCtxSync(): void {
  createEffect(() => {
    const sid = activeTarget.spriteId;
    const lid = activeTarget.layerId;
    const all = layers();
    const tsList = tilesets();

    if (sid === null || lid === null) {
      setActiveTilemapCtx(null);
      return;
    }

    const layer = all.find((l) => l.id === lid);
    if (!layer || layer.kind.kind !== "tilemap") {
      setActiveTilemapCtx(null);
      return;
    }

    const tilesetId = layer.kind.tileset;
    const cur = untrack(() => tilemapUi.activeTilemapCtx);

    // Same tileset already bound: keep the current ctx (which in-panel edits
    // update in place) and only refresh the layer id when switching between
    // two tilemap layers backed by the same tileset.
    if (cur !== null && cur.tilesetId === tilesetId) {
      if (cur.layerId !== lid) setActiveTilemapCtx({ ...cur, layerId: lid });
      return;
    }

    const ts = tsList.find((t) => t.id === tilesetId);
    if (ts === undefined) {
      // Tilesets still loading, or the id is gone. Re-runs when the query lands.
      setActiveTilemapCtx(null);
      return;
    }
    setActiveTilemapCtx({ layerId: lid, tilesetId, tileset: ts });
  });
}

// ── Tool ──────────────────────────────────────────────────────────────────

export type TilemapTool = "pencil" | "erase";

// ── Local autotile rule state ──────────────────────────────────────────────
// The autotile rule list mirrors `Tileset.autotile` so the editor re-renders
// without round-tripping every keystroke; `tilesetSetAutotile` debounces the
// persist back to the backend. It lives at module scope (not inside
// AutotileRuleEditor) so it survives the editor unmounting on tab switch.
// Kept as its own array store for fine-grained per-rule updates.
export const [autotileRules, setAutotileRules] = createStore<AutotileRule[]>([]);

// ── Helpers ────────────────────────────────────────────────────────────────

/** Returns true when the canvas is in tilemap-paint mode. */
export function isTilemapActive(): boolean {
  return tilemapUi.activeTilemapCtx !== null;
}

/**
 * Resets tile selection and tool to defaults.
 * Call when switching to a new tileset so stale selection state doesn't carry over.
 */
export function resetTileSelection(): void {
  // Match the module-level default (1 = first paintable tile, not the
  // empty sentinel).
  setSelectedTileIndex(1);
  setSelectedTileFlags(0);
  setTilemapTool("pencil");
}
