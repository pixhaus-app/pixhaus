// Transform handle pointer-event handling (stream S16).
//
// startTransformDrag() is called by TransformHandles when the user presses
// down on a handle element. It wires window-level mousemove and mouseup
// handlers for the drag lifecycle, updates transformBounds live (preview),
// and commits the new selection rect via IPC on release.
//
// Rotation and pixel-level transforms (flip, rotate) are stubbed until S04.

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
} from "../canvas-state";
import { canvasSetSelection, canvasTransform } from "../../lib/commands/canvas";
import { pushToast } from "../../lib/toast/toast-state";

// Converts a canvas-coordinate bounds object to the IPC Rect type.
function toIpcRect(bounds: { x: number; y: number; width: number; height: number }) {
  return {
    origin: { x: Math.round(bounds.x), y: Math.round(bounds.y) },
    size: {
      width: Math.max(1, Math.round(bounds.width)),
      height: Math.max(1, Math.round(bounds.height)),
    },
  };
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
    setTransformDrag(null);
    void commitBounds(newBounds);
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

// ── Pixel-level transform stubs ───────────────────────────────────────────────

/**
 * Dispatches a flip-horizontal transform via IPC.
 * Stubs until S01/S04 land.
 */
export function dispatchFlipX(): void {
  const spriteId = activeSpriteId();
  const layerId = activeLayerId();
  if (spriteId === null || layerId === null) return;
  canvasTransform({
    sprite_id: spriteId,
    layer_id: layerId,
    frame_index: 0,
    translate_x: 0,
    translate_y: 0,
    flip_x: true,
    flip_y: false,
    rotate_cw90: 0,
  }).catch((err: unknown) => {
    const e = err as { kind?: string; stream?: string };
    if (e?.kind === "unimplemented") {
      pushToast({
        title: `Flip requires ${e.stream ?? "S01"} — not yet available.`,
        kind: "info",
      });
    } else {
      console.error("[pixhaus] canvas_transform (flip X) failed:", err);
    }
  });
}

/**
 * Dispatches a flip-vertical transform via IPC.
 * Stubs until S01/S04 land.
 */
export function dispatchFlipY(): void {
  const spriteId = activeSpriteId();
  const layerId = activeLayerId();
  if (spriteId === null || layerId === null) return;
  canvasTransform({
    sprite_id: spriteId,
    layer_id: layerId,
    frame_index: 0,
    translate_x: 0,
    translate_y: 0,
    flip_x: false,
    flip_y: true,
    rotate_cw90: 0,
  }).catch((err: unknown) => {
    const e = err as { kind?: string; stream?: string };
    if (e?.kind === "unimplemented") {
      pushToast({
        title: `Flip requires ${e.stream ?? "S01"} — not yet available.`,
        kind: "info",
      });
    } else {
      console.error("[pixhaus] canvas_transform (flip Y) failed:", err);
    }
  });
}

/**
 * Dispatches a 90° clockwise rotation via IPC.
 * Stubs until S01/S04 land.
 */
export function dispatchRotateCw(): void {
  const spriteId = activeSpriteId();
  const layerId = activeLayerId();
  if (spriteId === null || layerId === null) return;
  canvasTransform({
    sprite_id: spriteId,
    layer_id: layerId,
    frame_index: 0,
    translate_x: 0,
    translate_y: 0,
    flip_x: false,
    flip_y: false,
    rotate_cw90: 1,
  }).catch((err: unknown) => {
    const e = err as { kind?: string; stream?: string };
    if (e?.kind === "unimplemented") {
      pushToast({
        title: `Rotate requires ${e.stream ?? "S01"} — not yet available.`,
        kind: "info",
      });
    } else {
      console.error("[pixhaus] canvas_transform (rotate CW) failed:", err);
    }
  });
}

/**
 * Dispatches a 90° counter-clockwise rotation via IPC.
 * Stubs until S01/S04 land.
 */
export function dispatchRotateCcw(): void {
  const spriteId = activeSpriteId();
  const layerId = activeLayerId();
  if (spriteId === null || layerId === null) return;
  canvasTransform({
    sprite_id: spriteId,
    layer_id: layerId,
    frame_index: 0,
    translate_x: 0,
    translate_y: 0,
    flip_x: false,
    flip_y: false,
    rotate_cw90: 3,
  }).catch((err: unknown) => {
    const e = err as { kind?: string; stream?: string };
    if (e?.kind === "unimplemented") {
      pushToast({
        title: `Rotate requires ${e.stream ?? "S01"} — not yet available.`,
        kind: "info",
      });
    } else {
      console.error("[pixhaus] canvas_transform (rotate CCW) failed:", err);
    }
  });
}
