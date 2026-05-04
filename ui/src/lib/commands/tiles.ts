// Tilemap editing commands.
// All commands are stubbed until S06 (tilemap data structures) lands.

import { invoke } from "@tauri-apps/api/core";
import type { FrameIndex, LayerId, SpriteId, TileCell, TileIndex } from "../types";

// ── argument types ────────────────────────────────────────────────────────────

export type TilePlaceArgs = {
  sprite_id: SpriteId;
  layer_id: LayerId;
  frame_index: FrameIndex;
  cell_x: number;
  cell_y: number;
  cell: TileCell;
};

export type TileEraseArgs = {
  sprite_id: SpriteId;
  layer_id: LayerId;
  frame_index: FrameIndex;
  cell_x: number;
  cell_y: number;
};

export type AutotileArgs = {
  sprite_id: SpriteId;
  layer_id: LayerId;
  frame_index: FrameIndex;
  rule_set: string;
  source_tile: TileIndex;
};

// ── commands ──────────────────────────────────────────────────────────────────

/**
 * Places a tile cell at a position on a tilemap layer.
 * Requires S06 (tilemap) — returns an error until S06 lands.
 */
export function tilePlace(args: TilePlaceArgs): Promise<void> {
  return invoke<void>("tile_place", { args });
}

/**
 * Erases a tile cell at a position on a tilemap layer.
 * Requires S06 (tilemap) — returns an error until S06 lands.
 */
export function tileErase(args: TileEraseArgs): Promise<void> {
  return invoke<void>("tile_erase", { args });
}

/**
 * Applies autotile rules to a region of a tilemap layer.
 * Requires S06 (tilemap) — returns an error until S06 lands.
 */
export function tileAutotileApply(args: AutotileArgs): Promise<void> {
  return invoke<void>("tile_autotile_apply", { args });
}
