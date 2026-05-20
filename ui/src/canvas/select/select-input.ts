// Canvas pointer-event handling for selection tools (stream S16).
//
// attachSelectInput() is called by the canvas input handler when the active
// tool is a selection tool (rect, ellipse, lasso, wand, color-range). It
// handles the full pointer lifecycle — start, drag, commit, cancel — and
// returns a cleanup function that removes all listeners.
//
// Selection tools that require pixel data (ellipse, lasso, wand, color-range)
// are routed to stub IPC commands that return Unimplemented until S01 lands.

import { screenToCanvas } from "../viewport";
import {
  scrollX,
  scrollY,
  zoom,
  selectionRect,
  setSelectionRect,
  setSelectionKind,
  setSelectionLayerId,
  activeSpriteId,
  activeLayerId,
  isSelectMode,
} from "../canvas-state";
import {
  selectTool,
  marqueeDrag,
  setMarqueeDrag,
  lassoPoints,
  setLassoPoints,
  selectionAddMode,
  selectionSubtractMode,
  dragToBounds,
  dragIsNonEmpty,
  snapToPixel,
  wandTolerance,
  wandConnectivity,
  wandGapClose,
  wandGapDistance,
  colorRangeTolerance,
  colorRangeTarget,
  setColorRangeTarget,
  resetSelectState,
} from "./select-state";
import {
  canvasSetSelection,
  canvasSelectMagicWand,
  canvasSelectColorRange,
  canvasSelectLasso,
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
  return screenToCanvas(sx, sy, scrollX(), scrollY(), zoom(), rect.width, rect.height);
}

// Applies shift-add / alt-subtract modifiers to the pending selection.
// Currently only "replace" is implemented for rect (add/subtract require mask
// buffers from S01). Logs a stub warning when the mode cannot be honoured.
function warnIfModifierNotSupported(): void {
  if (selectionAddMode() || selectionSubtractMode()) {
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
    setSelectionLayerId(null);
  } catch (err: unknown) {
    console.error("[pixhaus] canvas_set_selection (deselect) failed:", err);
  }
}

// Reflects a backend selection region into the local rect signal.
//
// Both `rect` and `mask` regions carry a `bounds` rect, so the marching-ants
// overlay and transform gizmo can track either. Wand / lasso / color-range
// all produce mask regions; without honouring `mask` here they appeared to do
// nothing because only `rect` was handled.
function applyRegionSelection(
  region: SelectionRegion | null | undefined,
  anchorLayer: LayerId | null,
): void {
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
}

// ── Marquee drag (rect + ellipse) ────────────────────────────────────────────

// Ellipse drag reuses the same geometry as rect drag on the client side.
// The distinction matters only at commit time (rect → rect region, ellipse →
// mask region via a Rust algorithm). Until S01 lands ellipse is stubbed.

function onMarqueeDown(e: MouseEvent, el: HTMLElement): void {
  e.preventDefault();
  const [cx, cy] = eventToCanvas(e, el);
  setMarqueeDrag({
    startX: snapToPixel(cx),
    startY: snapToPixel(cy),
    currentX: snapToPixel(cx),
    currentY: snapToPixel(cy),
  });
}

function onMarqueeMove(e: MouseEvent, el: HTMLElement): void {
  const drag = marqueeDrag();
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
  const drag = marqueeDrag();
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
  const tool = selectTool();

  if (tool === "rect") {
    warnIfModifierNotSupported();
    await commitRectSelection(bounds);
  } else {
    // Ellipse marquee isn't wired to its IPC command yet. The core algorithm
    // (select_ellipse) exists, but the editable canvas can't draw a non-rect
    // mask outline yet, so an ellipse selection would render as a misleading
    // bounding box. Surface that honestly instead of committing.
    pushToast({
      title: "Ellipse selection isn't available yet.",
      kind: "info",
    });
    // Revert the live preview back to whatever was committed before.
    const prev = selectionRect();
    setSelectionRect(prev);
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
      tolerance: wandTolerance(),
      connectivity: wandConnectivity(),
      gap_close: wandGapClose() ? { closing_distance: wandGapDistance() } : null,
    });
    applyRegionSelection(state.region, anchorLayer);
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
  const target = colorRangeTarget();
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
      tolerance: colorRangeTolerance(),
    });
    applyRegionSelection(state.region, anchorLayer);
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
  const pts = lassoPoints();
  setLassoPoints([...pts, [snapToPixel(cx), snapToPixel(cy)]]);
}

async function commitLasso(): Promise<void> {
  const pts = lassoPoints();
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
    applyRegionSelection(state.region, anchorLayer);
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
    if (!isSelectMode()) return;
    if (e.button !== 0) return;
    // A gizmo-handle press (pointerdown) fires before this mousedown and sets
    // transformDrag; bail so transforming an existing selection doesn't also
    // start a brand-new marquee underneath it.
    if (transformDrag() !== null) return;
    const tool = selectTool();
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
    if (!isSelectMode()) return;
    const tool = selectTool();
    if (tool === "rect" || tool === "ellipse") {
      onMarqueeMove(e, el);
    }
  }

  function onMouseUp(e: MouseEvent): void {
    if (!isSelectMode()) return;
    const tool = selectTool();
    if (tool === "rect" || tool === "ellipse") {
      void onMarqueeUp(e, el);
    }
  }

  function onKeyDown(e: KeyboardEvent): void {
    if (!isSelectMode()) return;
    if (e.code === "Enter" && selectTool() === "lasso") {
      void commitLasso();
      e.preventDefault();
    }
    if (e.code === "Escape") {
      // Cancel any in-progress drag.
      const drag = marqueeDrag();
      if (drag) {
        setMarqueeDrag(null);
        setSelectionRect(null);
      }
      if (lassoPoints().length > 0) {
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
