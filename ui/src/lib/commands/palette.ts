// Palette CRUD and color management commands.

import { invoke } from "../ipc";
import type { Palette, PaletteId, Rgba, SpriteId } from "../types";

// ── argument types ────────────────────────────────────────────────────────────

export type PaletteAddColorArgs = {
  sprite_id: SpriteId;
  palette_id: PaletteId;
  color: Rgba;
  name?: string | null;
};

export type PaletteSetColorArgs = {
  sprite_id: SpriteId;
  palette_id: PaletteId;
  index: number;
  color: Rgba;
  name?: string | null;
};

// ── response types ────────────────────────────────────────────────────────────

export type PaletteSwapResult = {
  from_id: PaletteId;
  to_id: PaletteId;
};

// ── commands ──────────────────────────────────────────────────────────────────

/** Adds a new empty palette to a sprite. */
export function paletteAdd(sprite_id: SpriteId, name: string): Promise<Palette> {
  return invoke<Palette>("palette_add", { sprite_id, name });
}

/** Removes a palette from a sprite by ID. */
export function paletteDelete(sprite_id: SpriteId, palette_id: PaletteId): Promise<void> {
  return invoke<void>("palette_delete", { sprite_id, palette_id });
}

/** Appends a color to a palette. Returns the index of the new swatch. */
export function paletteAddColor(args: PaletteAddColorArgs): Promise<number> {
  return invoke<number>("palette_add_color", { args });
}

/** Removes the swatch at `index` from a palette. */
export function paletteRemoveColor(
  sprite_id: SpriteId,
  palette_id: PaletteId,
  index: number,
): Promise<void> {
  return invoke<void>("palette_remove_color", { sprite_id, palette_id, index });
}

/**
 * Moves a color from `from_index` to `to_index` within a palette,
 * shifting every entry between by one. The drag-to-reorder UI in
 * PaletteGrid calls this to persist the new order.
 */
export function paletteReorderColors(
  sprite_id: SpriteId,
  palette_id: PaletteId,
  from_index: number,
  to_index: number,
): Promise<void> {
  return invoke<void>("palette_reorder_colors", {
    sprite_id,
    palette_id,
    from_index,
    to_index,
  });
}

/** Replaces the color (and optionally the name) at a specific palette index. */
export function paletteSetColor(args: PaletteSetColorArgs): Promise<void> {
  return invoke<void>("palette_set_color", { args });
}

/** Swaps the positions of two palettes in a sprite's palette list. */
export function paletteSwap(
  sprite_id: SpriteId,
  from_id: PaletteId,
  to_id: PaletteId,
): Promise<PaletteSwapResult> {
  return invoke<PaletteSwapResult>("palette_swap", { sprite_id, from_id, to_id });
}

/** Returns all palettes in a sprite. */
export function paletteList(sprite_id: SpriteId): Promise<Palette[]> {
  return invoke<Palette[]>("palette_list", { sprite_id });
}
