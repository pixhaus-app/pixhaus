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
  /** Brush shape: "pixel", "circle", or "square". Defaults to "pixel". */
  brush_shape: "pixel" | "circle" | "square";
  /** Brush diameter in canvas pixels. Defaults to 1. */
  brush_size: number;
  /** Remove diagonal corner artifacts from pixel-art strokes. */
  pixel_perfect: boolean;
  /** Draw with fully-transparent pixels (eraser mode). */
  erase: boolean;
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

/**
 * One transform operation in a [`TransformArgs`] batch.
 *
 * The discriminator field is `kind` and matches the Rust enum
 * `app::commands::canvas::TransformOpArg` with `serde(tag = "kind",
 * rename_all = "snake_case")`. Operations are applied in order; the
 * output of each becomes the input of the next.
 */
export type TransformOp =
  | { kind: "translate"; dx: number; dy: number }
  | { kind: "scale_nearest"; new_width: number; new_height: number }
  | { kind: "scale_integer_multiple"; factor: number }
  | { kind: "scale_integer_divisor"; divisor: number }
  | { kind: "rotate90_cw" }
  | { kind: "rotate90_ccw" }
  | { kind: "rotate180" }
  | { kind: "rotate_rot_sprite"; degrees: number }
  | { kind: "rotate_bilinear"; degrees: number }
  | { kind: "flip_horizontal" }
  | { kind: "flip_vertical" }
  | { kind: "skew_x"; factor: number }
  | { kind: "skew_y"; factor: number }
  | {
      kind: "perspective";
      corners: [[number, number], [number, number], [number, number], [number, number]];
    };

export type TransformArgs = {
  sprite_id: SpriteId;
  layer_id: LayerId;
  frame_index: FrameIndex;
  /** Ordered list of operations to apply. */
  ops: TransformOp[];
};

// ── result types ─────────────────────────────────────────────────────────────

export type CanvasComposite = {
  /** Sprite canvas width in pixels. */
  sprite_width: number;
  /** Sprite canvas height in pixels. */
  sprite_height: number;
  /** Tile side length in canvas pixels. */
  tile_size: number;
  /** Number of tile columns. */
  tiles_x: number;
  /** Number of tile rows. */
  tiles_y: number;
};

// ── commands ──────────────────────────────────────────────────────────────────

/**
 * Returns tile-grid metadata for the given sprite.
 * The renderer calls this when a sprite becomes active to learn canvas size
 * and tile layout.  Actual pixel data arrives via canvas:tile-dirty events.
 */
export function canvasComposite(spriteId: SpriteId): Promise<CanvasComposite> {
  return invoke<CanvasComposite>("canvas_composite", { sprite_id: spriteId });
}

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
 * Selects the entire canvas of the given sprite as a rectangular region.
 */
export function canvasSelectAll(spriteId: SpriteId): Promise<SelectionState> {
  return invoke<SelectionState>("canvas_select_all", { sprite_id: spriteId });
}

/**
 * Inverts the current selection.
 * Requires S01 (pixel buffers) — returns an error until S01 lands.
 */
export function canvasInvertSelection(
  spriteId: SpriteId,
  anchorLayer: LayerId | null,
): Promise<SelectionState> {
  return invoke<SelectionState>("canvas_invert_selection", {
    sprite_id: spriteId,
    anchor_layer: anchorLayer,
  });
}

export type MagicWandArgs = {
  sprite_id: SpriteId;
  anchor_layer: LayerId | null;
  seed_x: number;
  seed_y: number;
  tolerance: number;
  connectivity: "four" | "eight";
};

/**
 * Selects a contiguous region via flood-fill from a seed pixel.
 * Requires S01 (pixel buffers) — returns an error until S01 lands.
 */
export function canvasSelectMagicWand(args: MagicWandArgs): Promise<SelectionState> {
  return invoke<SelectionState>("canvas_select_magic_wand", { ...args });
}

export type ColorRangeArgs = {
  sprite_id: SpriteId;
  anchor_layer: LayerId | null;
  x: number;
  y: number;
  target_color: Rgba;
  tolerance: number;
};

/**
 * Selects pixels within a color tolerance of the target color.
 * Requires S01 (pixel buffers) — returns an error until S01 lands.
 */
export function canvasSelectColorRange(args: ColorRangeArgs): Promise<SelectionState> {
  return invoke<SelectionState>("canvas_select_color_range", { ...args });
}

export type LassoArgs = {
  sprite_id: SpriteId;
  anchor_layer: LayerId | null;
  points: Array<{ x: number; y: number }>;
};

/**
 * Selects the polygon defined by the given points.
 * Requires S01 (pixel buffers) — returns an error until S01 lands.
 */
export function canvasSelectLasso(args: LassoArgs): Promise<SelectionState> {
  return invoke<SelectionState>("canvas_select_lasso", { ...args });
}

/**
 * Replaces the entire canvas viewport state (scroll, zoom, active ids, toggles).
 * The UI calls this on every meaningful viewport change so save/load can restore it.
 */
export function canvasSetViewport(canvas: CanvasState): Promise<CanvasState> {
  return invoke<CanvasState>("canvas_set_viewport", { canvas });
}
