// Tileset management commands.

import { invoke } from "@tauri-apps/api/core";
import type {
  AutotileKind,
  FrameIndex,
  LayerId,
  SpriteId,
  Tileset,
  TilesetAddTileResult,
  TilesetId,
} from "../types";

// ── argument types ────────────────────────────────────────────────────────────

export type TilesetAddArgs = {
  sprite_id: SpriteId;
  name: string;
  tile_width: number;
  tile_height: number;
};

export type TilesetAddTileArgs = {
  sprite_id: SpriteId;
  tileset_id: TilesetId;
  source_layer_id: LayerId;
  frame_index: FrameIndex;
  source_x: number;
  source_y: number;
};

// ── commands ──────────────────────────────────────────────────────────────────

/** Returns all tilesets registered on a sprite. */
export function tilesetList(sprite_id: SpriteId): Promise<Tileset[]> {
  return invoke<Tileset[]>("tileset_list", { sprite_id });
}

/** Creates a new empty tileset on a sprite and returns it. */
export function tilesetAdd(args: TilesetAddArgs): Promise<Tileset> {
  return invoke<Tileset>("tileset_add", { args });
}

/** Renames a tileset. */
export function tilesetRename(
  sprite_id: SpriteId,
  tileset_id: TilesetId,
  name: string,
): Promise<void> {
  return invoke<void>("tileset_rename", { sprite_id, tileset_id, name });
}

/**
 * Captures a `tile_size`-sized region from a raster source layer and appends
 * it to the tileset as a new tile. Returns the updated tileset and the index
 * of the new tile.
 */
export function tilesetAddTile(args: TilesetAddTileArgs): Promise<TilesetAddTileResult> {
  return invoke<TilesetAddTileResult>("tileset_add_tile", { args });
}

/**
 * Sets (or clears) the autotile rule set bound to a tileset. Returns the
 * updated tileset so the caller can refresh local state in one round-trip.
 */
export function tilesetSetAutotile(
  sprite_id: SpriteId,
  tileset_id: TilesetId,
  autotile: AutotileKind | null,
): Promise<Tileset> {
  return invoke<Tileset>("tileset_set_autotile", { sprite_id, tileset_id, autotile });
}
