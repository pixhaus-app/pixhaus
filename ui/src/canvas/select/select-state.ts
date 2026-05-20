// Selection tool state (stream S16).
//
// Module-level signals so the canvas input handler, the command registry,
// and future tool-option panels all share the same reactive state without
// circular imports. Mirrors the tilemap-state.ts pattern.

import { createSignal } from "solid-js";
import type { Rgba } from "../../lib/types";

// ── Tool type ────────────────────────────────────────────────────────────────

/**
 * Which selection tool is active.
 *
 * "rect" and "ellipse" are marquee tools driven by drag.
 * "lasso" accumulates click-points into a polygon (stub until S01).
 * "wand" and "color-range" use pixel data (stub until S01).
 */
export type SelectTool = "rect" | "ellipse" | "lasso" | "wand" | "color-range";

export const [selectTool, setSelectTool] = createSignal<SelectTool>("rect");

// ── Drag state ───────────────────────────────────────────────────────────────

/**
 * Live drag state for marquee tools (rect, ellipse).
 * null = no drag in progress.
 * All values are canvas coordinates (sprite pixels).
 */
export type MarqueeDrag = {
  startX: number;
  startY: number;
  currentX: number;
  currentY: number;
};

export const [marqueeDrag, setMarqueeDrag] = createSignal<MarqueeDrag | null>(null);

// ── Lasso ───────────────────────────────────────────────────────────────────

/** Accumulated polygon points for the lasso tool, in canvas coordinates. */
export const [lassoPoints, setLassoPoints] = createSignal<Array<[number, number]>>([]);

// ── Magic wand options ───────────────────────────────────────────────────────

/** Flood-fill tolerance (0 = exact match, 255 = match all). */
export const [wandTolerance, setWandTolerance] = createSignal(32);

/** Pixel connectivity for the flood-fill expansion. */
export const [wandConnectivity, setWandConnectivity] = createSignal<"four" | "eight">("four");

/**
 * When true, the magic wand runs a gap-closing pre-pass before flood-fill,
 * bridging near-miss outline endpoints so almost-closed shapes still
 * select cleanly. Matches OpenToonz's "auto-close" wand behaviour.
 */
export const [wandGapClose, setWandGapClose] = createSignal(false);

/** Maximum gap (in pixels) the auto-close pre-pass will try to bridge. */
export const [wandGapDistance, setWandGapDistance] = createSignal(10);

// ── Color range options ──────────────────────────────────────────────────────

/** Tolerance for the color-range selector. */
export const [colorRangeTolerance, setColorRangeTolerance] = createSignal(32);

/** Target color for the color-range selector (null = nothing picked yet). */
export const [colorRangeTarget, setColorRangeTarget] = createSignal<Rgba | null>(null);

// ── Boolean modifier flags ───────────────────────────────────────────────────

// Held-key flags updated by the canvas input handler on keydown/keyup.

/** Shift held: new selection is unioned with the current one. */
export const [selectionAddMode, setSelectionAddMode] = createSignal(false);

/** Alt held: new selection is subtracted from the current one. */
export const [selectionSubtractMode, setSelectionSubtractMode] = createSignal(false);

// ── Geometry helpers ─────────────────────────────────────────────────────────

/**
 * Converts a marquee drag into a normalised { x, y, width, height } rect.
 *
 * "Normalised" means x < x+width and y < y+height regardless of which
 * direction the user dragged. The result is in canvas coordinates and
 * always has non-negative dimensions.
 */
export function dragToBounds(drag: MarqueeDrag): {
  x: number;
  y: number;
  width: number;
  height: number;
} {
  const x = Math.min(drag.startX, drag.currentX);
  const y = Math.min(drag.startY, drag.currentY);
  const width = Math.abs(drag.currentX - drag.startX);
  const height = Math.abs(drag.currentY - drag.startY);
  return { x, y, width, height };
}

/**
 * Snaps a canvas coordinate to the nearest integer pixel boundary.
 * Selection tools operate on whole pixels, not sub-pixel positions.
 */
export function snapToPixel(v: number): number {
  return Math.floor(v);
}

/** Returns true when a drag has enough extent to constitute a real selection. */
export function dragIsNonEmpty(drag: MarqueeDrag): boolean {
  return Math.abs(drag.currentX - drag.startX) >= 1 && Math.abs(drag.currentY - drag.startY) >= 1;
}

// ── Reset ────────────────────────────────────────────────────────────────────

/**
 * Clears all in-progress drag / lasso / color-range state.
 * Called on project close and on Escape.
 */
export function resetSelectState(): void {
  setMarqueeDrag(null);
  setLassoPoints([]);
  setColorRangeTarget(null);
  setSelectionAddMode(false);
  setSelectionSubtractMode(false);
}
