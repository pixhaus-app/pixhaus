// Tilemap editor state.
//
// This module owns all signals that drive the tilemap sub-UI: which tileset is
// active, which tile the brush is carrying, the active tool, and whether
// autotile resolution is enabled. The state is module-level (not component-
// scoped) because the tileset panel, the rule editor, and the canvas input
// handler all read and write the same signals.

import { createSignal } from "solid-js";
import { createStore } from "solid-js/store";
import type {
  AutotileKind,
  AutotileRule,
  LayerId,
  TileIndex,
  Tileset,
  TilesetId,
} from "../lib/types";

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
// Autotile kind / rule set for the active tileset.  Persisted to the project
// once the full tilemap IPC (S06) lands; held in UI state for now.
//
// These signals live at module scope (not inside AutotileRuleEditor) so the
// rules and default_tile survive the editor unmounting — e.g. when the user
// switches preferences tabs and back. Solid's <Tabs/> implementation here
// destroys the inactive panel's components, so any state inside the
// component is lost on every tab switch.

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
