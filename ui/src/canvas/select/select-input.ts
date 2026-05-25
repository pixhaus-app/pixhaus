// Canvas pointer-event handling for selection tools (stream S16).
//
// attachSelectInput() is called by the canvas input handler when the active
// tool is a selection tool (rect, ellipse, lasso, wand, color-range). It
// handles the full pointer lifecycle — start, drag, commit, cancel — and
// returns a cleanup function that removes all listeners.
//
// Rect commits a rect region; ellipse / lasso / wand / color-range commit
// mask regions, and their per-pixel mask is pulled back so the renderer can
// trace the selection's true outline (see applyRegionSelection).

import { screenToCanvas } from "../viewport";
import {
  viewport,
  setSelectionRect,
  setSelectionKind,
  setSelectionMask,
  setSelectionLayerId,
  activeSpriteId,
  activeLayerId,
} from "../canvas-state";
import {
  select,
  setMarqueeDrag,
  setLassoPoints,
  dragToBounds,
  dragIsNonEmpty,
  snapToPixel,
  setColorRangeTarget,
  resetSelectState,
} from "./select-state";
import {
  canvasSetSelection,
  canvasSelectMagicWand,
  canvasSelectColorRange,
  canvasSelectLasso,
  canvasSelectEllipse,
  canvasGetSelectionMask,
  canvasInvertSelection,
} from "../../lib/commands/canvas";
import { transformDrag } from "../transform/transform-state";
import { pushToast } from "../../lib/toast/toast-state";
import { isUnimplementedError, toastUnimplemented } from "../../lib/utils/errors";
import { toIpcRect } from "../../lib/utils/geometry";
import type { Rgba, LayerId } from "../../lib/types";
import type { SelectionRegion } from "../../lib/types/SelectionRegion";

// Gets the canvas coordinate from a raw screen event, accounting for the
// viewport's current scroll and zoom. el is the container element.
function eventToCanvas(e: MouseEvent, el: HTMLElement): [number, number] {
  const rect = el.getBoundingClientRect();
  const sx = e.clientX - rect.left;
  const sy = e.clientY - rect.top;
  return screenToCanvas(
    sx,
    sy,
    viewport.scrollX,
    viewport.scrollY,
    viewport.zoom,
    rect.width,
    rect.height,
  );
}

// Applies shift-add / alt-subtract modifiers to the pending selection.
// Currently only "replace" is implemented for rect (add/subtract require mask
// buffers from S01). Logs a stub warning when the mode cannot be honoured.
function warnIfModifierNotSupported(): void {
  if (select.selectionAddMode || select.selectionSubtractMode) {
    pushToast({
      title: "Add / subtract mode requires S01 (pixel buffers) — using replace instead.",
      kind: "info",
    });
  }
}

// Commits a rect selection via IPC and updates the local state signals.
async function commitRectSelection(bounds: {
  x: number;
  y: number;
  width: number;
  height: number;
}): Promise<void> {
  const anchorLayer = activeLayerId();
  try {
    const region = { kind: "rect" as const, bounds: toIpcRect(bounds) };
    await canvasSetSelection(region, anchorLayer);
    setSelectionRect({ x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height });
    setSelectionKind("rect");
    setSelectionMask(null);
    setSelectionLayerId(anchorLayer);
  } catch (err: unknown) {
    console.error("[pixhaus] canvas_set_selection failed:", err);
  }
}

// Clears the selection — sets region to null, clears the local rect signal.
async function commitDeselect(): Promise<void> {
  try {
    await canvasSetSelection(null, null);
    setSelectionRect(null);
    setSelectionKind(null);
    setSelectionMask(null);
    setSelectionLayerId(null);
  } catch (err: unknown) {
    console.error("[pixhaus] canvas_set_selection (deselect) failed:", err);
  }
}

// Reflects a backend selection region into the local signals.
//
// Both `rect` and `mask` regions carry a `bounds` rect, which drives the
// transform gizmo and the bounding-box fallback. For a `mask` region we also
// pull the per-pixel mask so the renderer can trace the selection's true
// outline rather than its bounding box. Wand / lasso / ellipse / color-range
// all produce mask regions.
async function applyRegionSelection(
  region: SelectionRegion | null | undefined,
  anchorLayer: LayerId | null,
): Promise<void> {
  if (!region) return;
  const b = region.bounds;
  setSelectionRect({
    x: b.origin.x,
    y: b.origin.y,
    width: b.size.width,
    height: b.size.height,
  });
  setSelectionKind(region.kind);
  setSelectionLayerId(anchorLayer);

  if (region.kind !== "mask") {
    setSelectionMask(null);
    return;
  }
  try {
    const mask = await canvasGetSelectionMask();
    setSelectionMask(
      mask
        ? {
            x: mask.x,
            y: mask.y,
            width: mask.width,
            height: mask.height,
            data: new Uint8Array(mask.data),
          }
        : null,
    );
  } catch (err: unknown) {
    console.error("[pixhaus] canvas_get_selection_mask failed:", err);
    setSelectionMask(null);
  }
}

// ── Marquee drag (rect + ellipse) ────────────────────────────────────────────

// Ellipse drag reuses the same geometry as rect drag on the client side.
// The distinction matters only at commit time: rect commits a rect region;
// ellipse commits a mask region via the core select_ellipse algorithm and
// renders its true outline.

function onMarqueeDown(e: MouseEvent, el: HTMLElement): void {
  e.preventDefault();
  const [cx, cy] = eventToCanvas(e, el);
  // Starting a fresh marquee drops any prior mask outline so the live rect
  // preview isn't drawn underneath a stale arbitrary-shape selection.
  setSelectionMask(null);
  setSelectionKind(null);
  setMarqueeDrag({
    startX: snapToPixel(cx),
    startY: snapToPixel(cy),
    currentX: snapToPixel(cx),
    currentY: snapToPixel(cy),
  });
}

function onMarqueeMove(e: MouseEvent, el: HTMLElement): void {
  const drag = select.marqueeDrag;
  if (!drag) return;
  const [cx, cy] = eventToCanvas(e, el);
  const updated = {
    ...drag,
    currentX: snapToPixel(cx),
    currentY: snapToPixel(cy),
  };
  setMarqueeDrag(updated);

  // Preview: update selection rect live so the WebGL marching ants track
  // the drag. We only update the signal — IPC fires on mouseup. Mirror
  // the in-progress anchor so consumers reading selectionLayerId during
  // the drag see a consistent (rect, layer) pair.
  if (dragIsNonEmpty(updated)) {
    const b = dragToBounds(updated);
    setSelectionRect(b);
    setSelectionLayerId(activeLayerId());
  }
}

async function onMarqueeUp(e: MouseEvent, el: HTMLElement): Promise<void> {
  const drag = select.marqueeDrag;
  setMarqueeDrag(null);
  if (!drag) return;

  const [cx, cy] = eventToCanvas(e, el);
  const finalDrag = { ...drag, currentX: snapToPixel(cx), currentY: snapToPixel(cy) };

  // A click without drag = deselect.
  if (!dragIsNonEmpty(finalDrag)) {
    await commitDeselect();
    return;
  }

  const bounds = dragToBounds(finalDrag);
  const tool = select.selectTool;

  if (tool === "rect") {
    warnIfModifierNotSupported();
    await commitRectSelection(bounds);
  } else {
    warnIfModifierNotSupported();
    await commitEllipseSelection(bounds);
  }
}

// Commits an ellipse selection via IPC. The core inscribes the ellipse in
// `bounds` and returns a mask region; applyRegionSelection then pulls the mask
// so the renderer can trace its true outline.
async function commitEllipseSelection(bounds: {
  x: number;
  y: number;
  width: number;
  height: number;
}): Promise<void> {
  const spriteId = activeSpriteId();
  const anchorLayer = activeLayerId();
  if (spriteId === null) return;
  try {
    const state = await canvasSelectEllipse({
      sprite_id: spriteId,
      anchor_layer: anchorLayer,
      bounds: toIpcRect(bounds),
    });
    await applyRegionSelection(state.region, anchorLayer);
  } catch (err: unknown) {
    if (isUnimplementedError(err)) {
      toastUnimplemented("Ellipse selection", err, "S01");
    } else {
      console.error("[pixhaus] canvas_select_ellipse failed:", err);
    }
  }
}

// ── Magic wand ───────────────────────────────────────────────────────────────

async function onWandClick(e: MouseEvent, el: HTMLElement): Promise<void> {
  e.preventDefault();
  const [cx, cy] = eventToCanvas(e, el);
  const spriteId = activeSpriteId();
  const anchorLayer = activeLayerId();
  if (spriteId === null) return;

  try {
    const state = await canvasSelectMagicWand({
      sprite_id: spriteId,
      anchor_layer: anchorLayer,
      seed_x: Math.round(cx),
      seed_y: Math.round(cy),
      tolerance: select.wandTolerance,
      connectivity: select.wandConnectivity,
      gap_close: select.wandGapClose ? { closing_distance: select.wandGapDistance } : null,
    });
    await applyRegionSelection(state.region, anchorLayer);
  } catch (err: unknown) {
    if (isUnimplementedError(err)) {
      toastUnimplemented("Magic wand", err, "S01");
    } else {
      console.error("[pixhaus] canvas_select_magic_wand failed:", err);
    }
  }
}

// ── Color range ──────────────────────────────────────────────────────────────

async function onColorRangeClick(e: MouseEvent, el: HTMLElement): Promise<void> {
  e.preventDefault();
  const [cx, cy] = eventToCanvas(e, el);
  const spriteId = activeSpriteId();
  const anchorLayer = activeLayerId();
  if (spriteId === null) return;

  // The target color is picked from the click position; if the IPC stub is
  // not yet wired, we still store the click for live preview purposes.
  const target = select.colorRangeTarget;
  if (!target) {
    pushToast({
      title: "Click to pick a target color first, then click again to select.",
      kind: "info",
    });
    // Store a placeholder so the next click can attempt the selection.
    const placeholderColor: Rgba = { r: 0, g: 0, b: 0, a: 255 };
    setColorRangeTarget(placeholderColor);
    return;
  }

  try {
    const state = await canvasSelectColorRange({
      sprite_id: spriteId,
      anchor_layer: anchorLayer,
      x: Math.round(cx),
      y: Math.round(cy),
      target_color: target,
      tolerance: select.colorRangeTolerance,
    });
    await applyRegionSelection(state.region, anchorLayer);
    setColorRangeTarget(null);
  } catch (err: unknown) {
    if (isUnimplementedError(err)) {
      toastUnimplemented("Color range", err, "S01");
    } else {
      console.error("[pixhaus] canvas_select_color_range failed:", err);
    }
  }
}

// ── Lasso ────────────────────────────────────────────────────────────────────

// Lasso mode: each click adds a point. Double-click or Enter closes the path.

function onLassoDown(e: MouseEvent, el: HTMLElement): void {
  e.preventDefault();
  const [cx, cy] = eventToCanvas(e, el);
  const pts = select.lassoPoints;
  setLassoPoints([...pts, [snapToPixel(cx), snapToPixel(cy)]]);
}

async function commitLasso(): Promise<void> {
  const pts = select.lassoPoints;
  if (pts.length < 3) {
    setLassoPoints([]);
    return;
  }
  const spriteId = activeSpriteId();
  const anchorLayer = activeLayerId();
  if (spriteId === null) {
    setLassoPoints([]);
    return;
  }

  try {
    const state = await canvasSelectLasso({
      sprite_id: spriteId,
      anchor_layer: anchorLayer,
      points: pts.map(([x, y]) => ({ x, y })),
    });
    await applyRegionSelection(state.region, anchorLayer);
  } catch (err: unknown) {
    if (isUnimplementedError(err)) {
      toastUnimplemented("Lasso selection", err, "S01");
    } else {
      console.error("[pixhaus] canvas_select_lasso failed:", err);
    }
  } finally {
    setLassoPoints([]);
  }
}

// ── Main attach ──────────────────────────────────────────────────────────────

/**
 * Attaches selection tool pointer listeners to `el`.
 *
 * Only call when the active tool is a selection tool.  Returns a cleanup
 * function — call it from Solid's onCleanup or when the tool changes.
 */
export function attachSelectInput(el: HTMLElement): () => void {
  function onMouseDown(e: MouseEvent): void {
    if (!viewport.isSelectMode) return;
    if (e.button !== 0) return;
    // A gizmo-handle press (pointerdown) fires before this mousedown and sets
    // transformDrag; bail so transforming an existing selection doesn't also
    // start a brand-new marquee underneath it.
    if (transformDrag() !== null) return;
    const tool = select.selectTool;
    if (tool === "rect" || tool === "ellipse") {
      onMarqueeDown(e, el);
    } else if (tool === "wand") {
      void onWandClick(e, el);
    } else if (tool === "color-range") {
      void onColorRangeClick(e, el);
    } else if (tool === "lasso") {
      if (e.detail === 2) {
        // Double-click closes the lasso.
        void commitLasso();
      } else {
        onLassoDown(e, el);
      }
    }
  }

  function onMouseMove(e: MouseEvent): void {
    if (!viewport.isSelectMode) return;
    const tool = select.selectTool;
    if (tool === "rect" || tool === "ellipse") {
      onMarqueeMove(e, el);
    }
  }

  function onMouseUp(e: MouseEvent): void {
    if (!viewport.isSelectMode) return;
    const tool = select.selectTool;
    if (tool === "rect" || tool === "ellipse") {
      void onMarqueeUp(e, el);
    }
  }

  function onKeyDown(e: KeyboardEvent): void {
    if (!viewport.isSelectMode) return;
    if (e.code === "Enter" && select.selectTool === "lasso") {
      void commitLasso();
      e.preventDefault();
    }
    if (e.code === "Escape") {
      // Cancel any in-progress drag.
      const drag = select.marqueeDrag;
      if (drag) {
        setMarqueeDrag(null);
        setSelectionRect(null);
      }
      if (select.lassoPoints.length > 0) {
        setLassoPoints([]);
      }
    }
  }

  el.addEventListener("mousedown", onMouseDown);
  window.addEventListener("mousemove", onMouseMove);
  window.addEventListener("mouseup", onMouseUp);
  window.addEventListener("keydown", onKeyDown);

  return () => {
    el.removeEventListener("mousedown", onMouseDown);
    window.removeEventListener("mousemove", onMouseMove);
    window.removeEventListener("mouseup", onMouseUp);
    window.removeEventListener("keydown", onKeyDown);
    resetSelectState();
  };
}

// Re-export for use in command handlers.
export { commitDeselect };
export { canvasInvertSelection };
