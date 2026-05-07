// Tilemap editing commands.

import { invoke } from "@tauri-apps/api/core";
import type {
  FrameIndex,
  LayerId,
  SpriteId,
  TileCell,
  TileIndex,
  TileProperties,
  Tileset,
  TilesetId,
} from "../types";

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
  /** Rule set tag: `"blob47"`, `"corner16"`, or `"minimal4"`. */
  rule_set: string;
  source_tile: TileIndex;
};

export type TilesetSetTileMetadataArgs = {
  sprite_id: SpriteId;
  tileset_id: TilesetId;
  tile_index: TileIndex;
  metadata: TileProperties;
};

// ── event payloads ────────────────────────────────────────────────────────────

/** Payload for the `tilemap:cell-changed` event. */
export type TilemapCellChangedPayload = {
  sprite_id: number;
  layer_id: number;
  frame_index: number;
  cell_x: number;
  cell_y: number;
};

/** Payload for the `tilemap:bulk-changed` event (autotile apply). */
export type TilemapBulkChangedPayload = {
  sprite_id: number;
  layer_id: number;
  frame_index: number;
};

// ── commands ──────────────────────────────────────────────────────────────────

/** Places a tile cell at a position on a tilemap layer. */
export function tilePlace(args: TilePlaceArgs): Promise<void> {
  return invoke<void>("tile_place", { args });
}

/** Erases a tile cell at a position on a tilemap layer. */
export function tileErase(args: TileEraseArgs): Promise<void> {
  return invoke<void>("tile_erase", { args });
}

/** Applies autotile rules to a region of a tilemap layer. */
export function tileAutotileApply(args: AutotileArgs): Promise<void> {
  return invoke<void>("tile_autotile_apply", { args });
}

/**
 * Sets per-tile metadata (collision, animation) on a tileset entry.
 * Returns the updated tileset so the panel can refresh local state
 * without a follow-up `tileset_list` round-trip.
 */
export function tilesetSetTileMetadata(args: TilesetSetTileMetadataArgs): Promise<Tileset> {
  return invoke<Tileset>("tileset_set_tile_metadata", { args });
}
