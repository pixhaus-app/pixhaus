// Tileset management commands.

import { invoke } from "@tauri-apps/api/core";
import type { SpriteId, Tileset, TilesetId } from "../types";

// ── argument types ────────────────────────────────────────────────────────────

export type TilesetAddArgs = {
  sprite_id: SpriteId;
  name: string;
  tile_width: number;
  tile_height: number;
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
