// Canvas operation commands.
// Pixel-drawing ops (draw_stroke, fill, transform) are stubbed until S01 lands.

import { invoke } from "@tauri-apps/api/core";
import type {
  CanvasState,
  FrameIndex,
  LayerId,
  Rgba,
  SelectionRegion,
  SelectionState,
  SpriteId,
} from "../types";

// ── argument types ────────────────────────────────────────────────────────────

export type DrawStrokeArgs = {
  sprite_id: SpriteId;
  layer_id: LayerId;
  frame_index: FrameIndex;
  /** Stroke path as [x, y] pairs in canvas coordinates. */
  points: Array<[number, number]>;
  color: Rgba;
  /** Per-point pressure values, same length as `points`. 1.0 = full pressure. */
  pressure: number[];
};

export type FillArgs = {
  sprite_id: SpriteId;
  layer_id: LayerId;
  frame_index: FrameIndex;
  x: number;
  y: number;
  color: Rgba;
  /** Tolerance for color matching: 0 = exact, 255 = match all. */
  tolerance: number;
};

export type TransformArgs = {
  sprite_id: SpriteId;
  layer_id: LayerId;
  frame_index: FrameIndex;
  translate_x: number;
  translate_y: number;
  flip_x: boolean;
  flip_y: boolean;
  /** Clockwise rotation in 90-degree steps (0–3). */
  rotate_cw90: number;
};

// ── commands ──────────────────────────────────────────────────────────────────

/**
 * Paints a freehand stroke on a layer cel.
 * Requires S01 (pixel buffers) — returns an error until S01 lands.
 */
export function canvasDrawStroke(args: DrawStrokeArgs): Promise<void> {
  return invoke<void>("canvas_draw_stroke", { args });
}

/**
 * Flood-fills a contiguous region on a layer cel.
 * Requires S01 (pixel buffers) — returns an error until S01 lands.
 */
export function canvasFill(args: FillArgs): Promise<void> {
  return invoke<void>("canvas_fill", { args });
}

/**
 * Applies a geometric transform to a layer cel.
 * Requires S01 (pixel buffers) — returns an error until S01 lands.
 */
export function canvasTransform(args: TransformArgs): Promise<void> {
  return invoke<void>("canvas_transform", { args });
}

/**
 * Sets the canvas selection. Pass `null` for `region` to clear the selection.
 */
export function canvasSetSelection(
  region: SelectionRegion | null,
  anchor_layer: LayerId | null,
): Promise<SelectionState> {
  return invoke<SelectionState>("canvas_set_selection", { region, anchor_layer });
}

/**
 * Replaces the entire canvas viewport state (scroll, zoom, active ids, toggles).
 * The UI calls this on every meaningful viewport change so save/load can restore it.
 */
export function canvasSetViewport(canvas: CanvasState): Promise<CanvasState> {
  return invoke<CanvasState>("canvas_set_viewport", { canvas });
}
