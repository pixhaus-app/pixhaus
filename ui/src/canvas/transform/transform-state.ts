// Transform handle state (stream S16).
//
// Owns the live drag state for the eight resize handles and one rotation
// handle. The canvas input handler sets these signals; the TransformHandles
// overlay component reads them to show the preview position.

import { createStore } from "solid-js/store";

// ── Handle types ─────────────────────────────────────────────────────────────

/**
 * Which transform handle the user is dragging.
 *
 * The four corners and four edge midpoints control scale.
 * "rotate" controls rotation (stub until S04).
 * "body" drags the entire selection box to translate it.
 */
export type TransformHandle = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w" | "rotate" | "body";

// ── Drag state ───────────────────────────────────────────────────────────────

/**
 * Active drag. null = no drag in progress.
 *
 * `originalBounds` captures the selection bounds at the moment the drag
 * started so that we can revert cleanly on Escape.
 */
export type TransformDrag = {
  handle: TransformHandle;
  /** Start pointer position in screen coordinates (CSS px). */
  startScreenX: number;
  startScreenY: number;
  /** Selection bounding box at drag start, in canvas coordinates. */
  originalBounds: { x: number; y: number; width: number; height: number };
};

// One store for the whole transform sub-UI. Reads are transform.transformDrag,
// transform.numericX, etc.; writes go through the named setters. The numeric
// fields back SelectionNumericPanel and are kept in sync with selectionRect by
// Canvas.tsx via a createEffect.
export interface TransformState {
  transformDrag: TransformDrag | null;
  numericX: number;
  numericY: number;
  numericW: number;
  numericH: number;
}

export const [transform, setTransform] = createStore<TransformState>({
  transformDrag: null,
  numericX: 0,
  numericY: 0,
  numericW: 0,
  numericH: 0,
});

export const setTransformDrag = (v: TransformDrag | null): void => setTransform("transformDrag", v);
export const setNumericX = (v: number): void => setTransform("numericX", v);
export const setNumericY = (v: number): void => setTransform("numericY", v);
export const setNumericW = (v: number): void => setTransform("numericW", v);
export const setNumericH = (v: number): void => setTransform("numericH", v);

/** Syncs the numeric panel signals from a selection rect. */
export function syncNumericFromBounds(bounds: {
  x: number;
  y: number;
  width: number;
  height: number;
}): void {
  setNumericX(Math.round(bounds.x));
  setNumericY(Math.round(bounds.y));
  setNumericW(Math.round(bounds.width));
  setNumericH(Math.round(bounds.height));
}

// ── Reset ────────────────────────────────────────────────────────────────────

export function resetTransformState(): void {
  setTransformDrag(null);
  setNumericX(0);
  setNumericY(0);
  setNumericW(0);
  setNumericH(0);
}

// ── Handle drag math ──────────────────────────────────────────────────────────

/**
 * Computes the new bounding box for the given handle drag delta.
 *
 * `dx` and `dy` are the pointer displacement in canvas pixels (screen
 * delta divided by zoom). The minimum dimension is clamped to 1 pixel so
 * the selection can never have zero area.
 *
 * "body" translates the whole box. Corner/edge handles resize by moving
 * only the anchored edges. Rotation is not handled here — it stubs until S04.
 */
export function applyHandleDelta(
  handle: TransformHandle,
  originalBounds: { x: number; y: number; width: number; height: number },
  dx: number,
  dy: number,
): { x: number; y: number; width: number; height: number } {
  const { x, y, width, height } = originalBounds;
  const MIN = 1;

  switch (handle) {
    case "body":
      return { x: x + dx, y: y + dy, width, height };

    case "nw":
      return {
        x: x + dx,
        y: y + dy,
        width: Math.max(MIN, width - dx),
        height: Math.max(MIN, height - dy),
      };
    case "n":
      return { x, y: y + dy, width, height: Math.max(MIN, height - dy) };
    case "ne":
      return {
        x,
        y: y + dy,
        width: Math.max(MIN, width + dx),
        height: Math.max(MIN, height - dy),
      };
    case "e":
      return { x, y, width: Math.max(MIN, width + dx), height };
    case "se":
      return {
        x,
        y,
        width: Math.max(MIN, width + dx),
        height: Math.max(MIN, height + dy),
      };
    case "s":
      return { x, y, width, height: Math.max(MIN, height + dy) };
    case "sw":
      return {
        x: x + dx,
        y,
        width: Math.max(MIN, width - dx),
        height: Math.max(MIN, height + dy),
      };
    case "w":
      return { x: x + dx, y, width: Math.max(MIN, width - dx), height };

    case "rotate":
      // Rotation requires S04; return original bounds unchanged.
      return { x, y, width, height };

    default:
      ((_x: never) => {})(handle);
      return { x, y, width, height };
  }
}
