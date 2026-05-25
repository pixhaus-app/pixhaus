// Selection tool state (stream S16).
//
// One Solid store shared by the canvas input handler, the command registry,
// and the tool-option panels. Reads are `select.selectTool`,
// `select.marqueeDrag`, etc.; writes go through the named setter functions.

import { createStore } from "solid-js/store";
import type { Rgba } from "../../lib/types";

/**
 * Which selection tool is active.
 *
 * "rect" and "ellipse" are marquee tools driven by drag.
 * "lasso" accumulates click-points into a polygon.
 * "wand" and "color-range" use pixel data.
 */
export type SelectTool = "rect" | "ellipse" | "lasso" | "wand" | "color-range";

/**
 * Live drag state for marquee tools (rect, ellipse). null = no drag in
 * progress. All values are canvas coordinates (sprite pixels).
 */
export type MarqueeDrag = {
  startX: number;
  startY: number;
  currentX: number;
  currentY: number;
};

export interface SelectState {
  selectTool: SelectTool;
  marqueeDrag: MarqueeDrag | null;
  /** Accumulated polygon points for the lasso tool, in canvas coordinates. */
  lassoPoints: Array<[number, number]>;
  /** Magic-wand flood-fill tolerance (0 = exact, 255 = match all). */
  wandTolerance: number;
  /** Pixel connectivity for the flood-fill expansion. */
  wandConnectivity: "four" | "eight";
  /** Run a gap-closing pre-pass before flood-fill (OpenToonz "auto-close"). */
  wandGapClose: boolean;
  /** Maximum gap (px) the auto-close pre-pass will bridge. */
  wandGapDistance: number;
  colorRangeTolerance: number;
  /** Target color for the color-range selector (null = nothing picked). */
  colorRangeTarget: Rgba | null;
  /** Shift held: new selection is unioned with the current one. */
  selectionAddMode: boolean;
  /** Alt held: new selection is subtracted from the current one. */
  selectionSubtractMode: boolean;
}

export const [select, setSelect] = createStore<SelectState>({
  selectTool: "rect",
  marqueeDrag: null,
  lassoPoints: [],
  wandTolerance: 32,
  wandConnectivity: "four",
  wandGapClose: false,
  wandGapDistance: 10,
  colorRangeTolerance: 32,
  colorRangeTarget: null,
  selectionAddMode: false,
  selectionSubtractMode: false,
});

// ── Setters ──────────────────────────────────────────────────────────────────

export const setSelectTool = (v: SelectTool): void => setSelect("selectTool", v);
export const setMarqueeDrag = (v: MarqueeDrag | null): void => setSelect("marqueeDrag", v);
export const setLassoPoints = (v: Array<[number, number]>): void => setSelect("lassoPoints", v);
export const setWandTolerance = (v: number): void => setSelect("wandTolerance", v);
export const setWandConnectivity = (v: "four" | "eight"): void => setSelect("wandConnectivity", v);
export const setWandGapClose = (v: boolean): void => setSelect("wandGapClose", v);
export const setWandGapDistance = (v: number): void => setSelect("wandGapDistance", v);
export const setColorRangeTolerance = (v: number): void => setSelect("colorRangeTolerance", v);
export const setColorRangeTarget = (v: Rgba | null): void => setSelect("colorRangeTarget", v);
export const setSelectionAddMode = (v: boolean): void => setSelect("selectionAddMode", v);
export const setSelectionSubtractMode = (v: boolean): void => setSelect("selectionSubtractMode", v);

// ── Geometry helpers ─────────────────────────────────────────────────────────

/**
 * Converts a marquee drag into a normalised { x, y, width, height } rect.
 * "Normalised" means non-negative width/height regardless of drag direction.
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

/** Snaps a canvas coordinate to the nearest integer pixel boundary. */
export function snapToPixel(v: number): number {
  return Math.floor(v);
}

/** Returns true when a drag has enough extent to constitute a real selection. */
export function dragIsNonEmpty(drag: MarqueeDrag): boolean {
  return Math.abs(drag.currentX - drag.startX) >= 1 && Math.abs(drag.currentY - drag.startY) >= 1;
}

/**
 * Clears all in-progress drag / lasso / color-range state.
 * Called on project close and on Escape.
 */
export function resetSelectState(): void {
  setSelect({
    marqueeDrag: null,
    lassoPoints: [],
    colorRangeTarget: null,
    selectionAddMode: false,
    selectionSubtractMode: false,
  });
}
