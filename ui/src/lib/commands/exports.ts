// Export commands: PNG sheet, animated GIF / WebP, Tiled .tmx.
//
// Each wrapper is a thin shell over the matching Tauri IPC. The dialog
// flow lives in the menu / command-palette dispatcher, not here — these
// stay focused on the cross-process call.

import { invoke } from "@tauri-apps/api/core";
import type { SpriteId } from "../types";

export type ExportPngArgs = {
  sprite_id: SpriteId;
  output_path: string;
  /** Optional grid column count. Falsy / missing means "single row". */
  grid_cols?: number | null;
};

export type ExportGifArgs = {
  sprite_id: SpriteId;
  output_path: string;
};

export type ExportWebpArgs = {
  sprite_id: SpriteId;
  output_path: string;
};

export type ExportTmxArgs = {
  sprite_id: SpriteId;
  output_path: string;
};

/**
 * Exports the active sprite as a PNG sprite sheet plus an Aseprite-format
 * JSON sidecar. The sidecar is written next to the PNG with the same stem.
 */
export function exportPngSpriteSheet(args: ExportPngArgs): Promise<void> {
  return invoke<void>("export_png_sprite_sheet", { args });
}

/** Exports the active sprite as an animated `.gif` (global NeuQuant palette). */
export function exportAnimatedGif(args: ExportGifArgs): Promise<void> {
  return invoke<void>("export_animated_gif", { args });
}

/** Exports the active sprite as an animated `.webp` (lossless VP8L). */
export function exportAnimatedWebp(args: ExportWebpArgs): Promise<void> {
  return invoke<void>("export_animated_webp", { args });
}

/** Exports tilemap layers as a Tiled `.tmx` plus one `.tsx` per tileset. */
export function exportTmx(args: ExportTmxArgs): Promise<void> {
  return invoke<void>("export_tmx", { args });
}
