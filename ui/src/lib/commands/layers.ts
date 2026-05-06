// Layer CRUD and property commands.

import { invoke } from "@tauri-apps/api/core";
import type { BlendMode, Layer, LayerId, LayerKind, SpriteId, TilesetId } from "../types";

// ── argument types ────────────────────────────────────────────────────────────

export type LayerAddArgs = {
  sprite_id: SpriteId;
  name: string;
  kind: LayerKind;
};

// ── response types ────────────────────────────────────────────────────────────

export type LayerRenamed = {
  layer_id: LayerId;
  name: string;
};

// ── commands ──────────────────────────────────────────────────────────────────

/** Adds a new layer to a sprite. The layer is appended above all existing layers. */
export function layerAdd(args: LayerAddArgs): Promise<Layer> {
  return invoke<Layer>("layer_add", { args });
}

/** Removes a layer from a sprite by ID. Also removes all cels on that layer. */
export function layerDelete(sprite_id: SpriteId, layer_id: LayerId): Promise<void> {
  return invoke<void>("layer_delete", { sprite_id, layer_id });
}

/** Moves a layer to a new position in the layer stack. */
export function layerReorder(
  sprite_id: SpriteId,
  layer_id: LayerId,
  new_index: number,
): Promise<void> {
  return invoke<void>("layer_reorder", { sprite_id, layer_id, new_index });
}

/** Sets the blend mode for a layer. */
export function layerSetBlendMode(
  sprite_id: SpriteId,
  layer_id: LayerId,
  blend_mode: BlendMode,
): Promise<void> {
  return invoke<void>("layer_set_blend_mode", { sprite_id, layer_id, blend_mode });
}

/** Sets the opacity for a layer (0–255). */
export function layerSetOpacity(
  sprite_id: SpriteId,
  layer_id: LayerId,
  opacity: number,
): Promise<void> {
  return invoke<void>("layer_set_opacity", { sprite_id, layer_id, opacity });
}

/** Sets the visibility of a layer. */
export function layerSetVisibility(
  sprite_id: SpriteId,
  layer_id: LayerId,
  visible: boolean,
): Promise<void> {
  return invoke<void>("layer_set_visibility", { sprite_id, layer_id, visible });
}

/** Sets the locked state of a layer. */
export function layerSetLocked(
  sprite_id: SpriteId,
  layer_id: LayerId,
  locked: boolean,
): Promise<void> {
  return invoke<void>("layer_set_locked", { sprite_id, layer_id, locked });
}

/** Renames a layer. */
export function layerRename(
  sprite_id: SpriteId,
  layer_id: LayerId,
  name: string,
): Promise<LayerRenamed> {
  return invoke<LayerRenamed>("layer_rename", { sprite_id, layer_id, name });
}

/** Returns all layers in a sprite, bottom to top (index 0 is the bottom layer). */
export function layerList(sprite_id: SpriteId): Promise<Layer[]> {
  return invoke<Layer[]>("layer_list", { sprite_id });
}

/** Sets the parent group of a layer. Pass `parent_id = null` to make it top-level. */
export function layerSetParent(
  sprite_id: SpriteId,
  layer_id: LayerId,
  parent_id: LayerId | null,
): Promise<void> {
  return invoke<void>("layer_set_parent", { sprite_id, layer_id, parent_id });
}

/** Converts a layer to a group. Removes any cels on the layer. */
export function layerConvertToGroup(sprite_id: SpriteId, layer_id: LayerId): Promise<void> {
  return invoke<void>("layer_convert_to_group", { sprite_id, layer_id });
}

/** Converts a layer to a tilemap layer backed by the given tileset. */
export function layerConvertToTilemap(
  sprite_id: SpriteId,
  layer_id: LayerId,
  tileset_id: TilesetId,
): Promise<void> {
  return invoke<void>("layer_convert_to_tilemap", { sprite_id, layer_id, tileset_id });
}

/** Merges a layer with the layer immediately below it. Requires S01. */
export function layerMergeDown(sprite_id: SpriteId, layer_id: LayerId): Promise<void> {
  return invoke<void>("layer_merge_down", { sprite_id, layer_id });
}

/** Merges the given layers into a single raster layer. Requires S01. */
export function layerMergeSelected(sprite_id: SpriteId, layer_ids: LayerId[]): Promise<void> {
  return invoke<void>("layer_merge_selected", { sprite_id, layer_ids });
}

/** Flattens all visible layers into a single raster layer. Requires S01. */
export function layerFlattenVisible(sprite_id: SpriteId): Promise<void> {
  return invoke<void>("layer_flatten_visible", { sprite_id });
}
