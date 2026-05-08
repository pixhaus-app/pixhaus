// Transform handle pointer-event handling (stream S16).
//
// startTransformDrag() is called by TransformHandles when the user presses
// down on a handle element. It wires window-level mousemove and mouseup
// handlers for the drag lifecycle, updates transformBounds live (preview),
// and commits the new selection rect via IPC on release.
//
// Body drags translate both the marquee and the pixels — the IPC translate
// runs first (consuming the original selection as a mask) and the moved
// selection is committed second. Resize handles only update the marquee;
// committing them as a scale op is out of scope for this task.

import {
  transformDrag,
  setTransformDrag,
  applyHandleDelta,
  syncNumericFromBounds,
  type TransformHandle,
} from "./transform-state";
import {
  selectionRect,
  setSelectionRect,
  setTransformBounds,
  zoom,
  activeSpriteId,
  activeLayerId,
  activeFrameIndex,
} from "../canvas-state";
import { canvasSetSelection, canvasTransform, type TransformOp } from "../../lib/commands/canvas";
import { isActiveLayerWritable } from "../../layers/layer-state";
import { pushToast } from "../../lib/toast/toast-state";
import { isUnimplementedError, toastUnimplemented } from "../../lib/utils/errors";
import { toIpcRect } from "../../lib/utils/geometry";

function toastLayerLocked(): void {
  pushToast({
    kind: "info",
    title: "Layer is locked",
    body: "Unlock the layer to transform pixels.",
    durationMs: 3000,
  });
}

// Persists the new selection bounds to Rust and updates local signals.
async function commitBounds(bounds: {
  x: number;
  y: number;
  width: number;
  height: number;
}): Promise<void> {
  const anchorLayer = activeLayerId();
  try {
    await canvasSetSelection({ kind: "rect", bounds: toIpcRect(bounds) }, anchorLayer);
    setSelectionRect(bounds);
    setTransformBounds(bounds);
    syncNumericFromBounds(bounds);
  } catch (err: unknown) {
    console.error("[pixhaus] canvas_set_selection (transform commit) failed:", err);
  }
}

// Translates the active layer's pixels by the given canvas-pixel delta and
// then moves the selection to follow them. The order matters: the IPC
// translate consumes the *current* selection as a mask, so the selection
// must still cover the original pixels when canvas_transform runs. The
// selection is updated second so the marching ants land on the moved pixels.
async function commitBodyTranslate(
  originalBounds: { x: number; y: number; width: number; height: number },
  newBounds: { x: number; y: number; width: number; height: number },
): Promise<void> {
  const dx = Math.round(newBounds.x - originalBounds.x);
  const dy = Math.round(newBounds.y - originalBounds.y);

  // Snap the live preview to integer pixels so the marquee lines up with
  // the painted pixels.
  const snappedBounds = {
    ...newBounds,
    x: originalBounds.x + dx,
    y: originalBounds.y + dy,
  };

  if (dx === 0 && dy === 0) {
    setSelectionRect(snappedBounds);
    setTransformBounds(snappedBounds);
    syncNumericFromBounds(snappedBounds);
    return;
  }

  const spriteId = activeSpriteId();
  const layerId = activeLayerId();
  if (spriteId === null || layerId === null) {
    // No active document — fall back to a marquee-only commit.
    void commitBounds(snappedBounds);
    return;
  }
  if (!isActiveLayerWritable()) {
    toastLayerLocked();
    // Snap the marquee back so it doesn't drift away from the pixels.
    setSelectionRect(originalBounds);
    setTransformBounds(originalBounds);
    syncNumericFromBounds(originalBounds);
    return;
  }

  try {
    await canvasTransform({
      sprite_id: spriteId,
      layer_id: layerId,
      frame_index: activeFrameIndex(),
      ops: [{ kind: "translate", dx, dy }],
    });
  } catch (err: unknown) {
    if (isUnimplementedError(err)) {
      toastUnimplemented("Move", err, "S04");
    } else {
      console.error("[pixhaus] canvas_transform (body translate) failed:", err);
    }
    // Revert the marquee to the original position so it doesn't drift away
    // from the still-stationary pixels.
    setSelectionRect(originalBounds);
    setTransformBounds(originalBounds);
    syncNumericFromBounds(originalBounds);
    return;
  }

  void commitBounds(snappedBounds);
}

// Reverts local signals to the originalBounds captured at drag start.
function revertToDragOrigin(): void {
  const drag = transformDrag();
  if (!drag) return;
  const { originalBounds: ob } = drag;
  setSelectionRect(ob);
  setTransformBounds(ob);
  syncNumericFromBounds(ob);
}

// ── Drag lifecycle ────────────────────────────────────────────────────────────

let cleanupDrag: (() => void) | null = null;

/**
 * Starts a transform drag for the given handle.
 *
 * Call from the `onPointerDown` handler attached to a transform handle SVG
 * element. The pointer events are captured on `window` so the drag continues
 * even when the pointer moves outside the handle rect.
 */
export function startTransformDrag(handle: TransformHandle, e: PointerEvent): void {
  const bounds = selectionRect();
  if (!bounds) return;

  // Rotation stubs until S04.
  if (handle === "rotate") {
    pushToast({
      title: "Rotation requires S04 (transform operations) — not yet available.",
      kind: "info",
    });
    return;
  }

  setTransformDrag({
    handle,
    startScreenX: e.clientX,
    startScreenY: e.clientY,
    originalBounds: { ...bounds },
  });

  const onMove = (ev: PointerEvent): void => {
    const drag = transformDrag();
    if (!drag) return;

    const dxScreen = ev.clientX - drag.startScreenX;
    const dyScreen = ev.clientY - drag.startScreenY;
    const z = zoom();
    // Convert screen-px delta to canvas-px delta.
    const dx = dxScreen / z;
    const dy = dyScreen / z;

    const newBounds = applyHandleDelta(drag.handle, drag.originalBounds, dx, dy);
    // Update both transform handles and marching ants live during the drag.
    setTransformBounds(newBounds);
    setSelectionRect(newBounds);
    syncNumericFromBounds(newBounds);
  };

  const onUp = (ev: PointerEvent): void => {
    const drag = transformDrag();
    if (!drag) {
      cleanup();
      return;
    }

    const dxScreen = ev.clientX - drag.startScreenX;
    const dyScreen = ev.clientY - drag.startScreenY;
    const z = zoom();
    const dx = dxScreen / z;
    const dy = dyScreen / z;

    const newBounds = applyHandleDelta(drag.handle, drag.originalBounds, dx, dy);
    const handle = drag.handle;
    const original = drag.originalBounds;
    setTransformDrag(null);
    if (handle === "body") {
      void commitBodyTranslate(original, newBounds);
    } else {
      void commitBounds(newBounds);
    }
    cleanup();
  };

  const onKeyDown = (ev: KeyboardEvent): void => {
    if (ev.code === "Escape") {
      setTransformDrag(null);
      revertToDragOrigin();
      cleanup();
    }
  };

  function cleanup(): void {
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("keydown", onKeyDown);
    cleanupDrag = null;
  }

  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp);
  window.addEventListener("keydown", onKeyDown);
  cleanupDrag = cleanup;
}

// ── Keyboard commit / cancel ──────────────────────────────────────────────────

/**
 * Attaches Enter / Escape handling for the transform tool.
 *
 * Enter commits the current transformBounds as the new selection.
 * Escape cancels any pending drag and reverts.
 *
 * Returns a cleanup function.
 */
export function attachTransformKeyboard(): () => void {
  function onKeyDown(e: KeyboardEvent): void {
    if (e.code === "Enter") {
      const drag = transformDrag();
      if (drag) {
        // A drag is still active — cancel it (commit may already be queued
        // in the pointerup handler, so just let it run).
        return;
      }
      // No drag, but user pressed Enter: treat as explicit commit of the
      // current selectionRect as-is.
      const bounds = selectionRect();
      if (bounds) {
        void commitBounds(bounds);
        e.preventDefault();
      }
    }

    if (e.code === "Escape") {
      if (cleanupDrag) cleanupDrag();
    }
  }

  window.addEventListener("keydown", onKeyDown);
  return () => window.removeEventListener("keydown", onKeyDown);
}

// ── Pixel-level transform helpers ─────────────────────────────────────────────

// Shared dispatcher: runs one op against the active sprite/layer/frame and
// surfaces an Unimplemented error as a toast instead of a console log.
function dispatchTransformOp(op: TransformOp, label: string): void {
  const spriteId = activeSpriteId();
  const layerId = activeLayerId();
  if (spriteId === null || layerId === null) return;
  canvasTransform({
    sprite_id: spriteId,
    layer_id: layerId,
    frame_index: activeFrameIndex(),
    ops: [op],
  }).catch((err: unknown) => {
    if (isUnimplementedError(err)) {
      toastUnimplemented(label, err, "S04");
    } else {
      console.error(`[pixhaus] canvas_transform (${label}) failed:`, err);
    }
  });
}

/** Dispatches a flip-horizontal transform via IPC. */
export function dispatchFlipX(): void {
  dispatchTransformOp({ kind: "flip_horizontal" }, "Flip horizontal");
}

/** Dispatches a flip-vertical transform via IPC. */
export function dispatchFlipY(): void {
  dispatchTransformOp({ kind: "flip_vertical" }, "Flip vertical");
}

/** Dispatches a 90° clockwise rotation via IPC. */
export function dispatchRotateCw(): void {
  dispatchTransformOp({ kind: "rotate90_cw" }, "Rotate clockwise");
}

/** Dispatches a 90° counter-clockwise rotation via IPC. */
export function dispatchRotateCcw(): void {
  dispatchTransformOp({ kind: "rotate90_ccw" }, "Rotate counter-clockwise");
}
