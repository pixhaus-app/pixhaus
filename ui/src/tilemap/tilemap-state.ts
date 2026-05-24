// Tilemap editor state.
//
// This module owns all signals that drive the tilemap sub-UI: which tileset is
// active, which tile the brush is carrying, the active tool, and whether
// autotile resolution is enabled. The state is module-level (not component-
// scoped) because the tileset panel, the rule editor, and the canvas input
// handler all read and write the same signals.

import { createEffect, createSignal, untrack } from "solid-js";
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
import { activeSpriteId, activeLayerId } from "../canvas/canvas-state";
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

export const [activeTilemapCtx, setActiveTilemapCtx] = createSignal<ActiveTilemapCtx | null>(null);

// Tilesets for the active sprite. The source returns the sprite id only when
// a tilemap layer is foregrounded, so this stays as lazy as the old ad-hoc
// fetch (no tileset IPC for plain raster sprites) while createResource drops
// the stale-response handling the bridge used to do with a token counter.
// Tileset mutations invalidate "tilesets" so the next layer switch sees fresh
// data; in-place edits also push the updated tileset onto activeTilemapCtx.
const tilesetsQuery = createBackendQuery<SpriteId, Tileset[]>({
  key: "tilesets",
  source: () => {
    const sid = activeSpriteId();
    const lid = activeLayerId();
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
    const sid = activeSpriteId();
    const lid = activeLayerId();
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
    const cur = untrack(() => activeTilemapCtx());

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

// ── Tile selection ─────────────────────────────────────────────────────────

// Selected tile index in the active tileset.
// Index 0 is the empty-tile sentinel; default to 1 (the first paintable
// tile) so the pencil tool starts in paint mode rather than erase mode.
// Users can still select index 0 explicitly to paint "empty" if they
// want the autotile editor's empty default.
export const [selectedTileIndex, setSelectedTileIndex] = createSignal<number>(1);

// Bitfield of flags to apply when placing a tile (flip X, flip Y, diagonal).
export const [selectedTileFlags, setSelectedTileFlags] = createSignal<number>(0);

// ── Tool ──────────────────────────────────────────────────────────────────

export type TilemapTool = "pencil" | "erase";

export const [tilemapTool, setTilemapTool] = createSignal<TilemapTool>("pencil");

// ── Autotile mode ──────────────────────────────────────────────────────────

// When true, the brush resolves the autotile rule set after each stroke rather
// than placing the selected tile index literally.
export const [autotileMode, setAutotileMode] = createSignal(false);

// ── Local autotile rule state ──────────────────────────────────────────────
// Autotile kind / rule set for the active tileset. The editor mirrors
// `Tileset.autotile` here so the UI can re-render without round-tripping
// every keystroke; `tilesetSetAutotile` debounces the persist back to
// the backend.
//
// These signals live at module scope (not inside AutotileRuleEditor) so the
// rules and default_tile survive the editor unmounting — e.g. when the user
// switches tabs. Solid's <Tabs/> implementation here destroys the inactive
// panel's components, so any state inside the component is lost on every
// tab switch.

export const [localAutotileKind, setLocalAutotileKind] = createSignal<AutotileKind | null>(null);

export const [autotileRules, setAutotileRules] = createStore<AutotileRule[]>([]);

export const [autotileDefaultTile, setAutotileDefaultTile] = createSignal<TileIndex>(
  0 as TileIndex,
);

// ── Helpers ────────────────────────────────────────────────────────────────

/** Returns true when the canvas is in tilemap-paint mode. */
export function isTilemapActive(): boolean {
  return activeTilemapCtx() !== null;
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
