// Canvas input handling: pan, zoom, cursor feedback.
//
// Returns a cleanup function to remove all listeners.  Mount by calling
// attachCanvasInput(canvasEl) inside a Solid onMount, then call the returned
// function in onCleanup.

import {
  scrollX,
  scrollY,
  zoom,
  setScrollX,
  setScrollY,
  setZoom,
  scheduleViewportSync,
  setCursorCanvas,
} from "./canvas-state";
import { snapZoom, clampZoom, zoomAt, screenToCanvas } from "./viewport";
import { isEditableTarget } from "../keybinds/keybind-manager";

// How much the continuous zoom changes per wheel tick (scroll-wheel smooth mode).
const WHEEL_ZOOM_FACTOR = 1.1;

// ── Pan state ──────────────────────────────────────────────────────────────

interface PanState {
  active: boolean;
  lastX: number;
  lastY: number;
}

// ── Main attach ────────────────────────────────────────────────────────────

/**
 * Attaches all canvas interaction listeners to `el`.
 * Returns a cleanup function — call it from Solid's onCleanup.
 */
export function attachCanvasInput(el: HTMLElement): () => void {
  const pan: PanState = { active: false, lastX: 0, lastY: 0 };
  let spaceHeld = false;

  // ── Wheel zoom ────────────────────────────────────────────────────────

  function onWheel(e: WheelEvent): void {
    e.preventDefault();

    const rect = el.getBoundingClientRect();
    const sx = e.clientX - rect.left;
    const sy = e.clientY - rect.top;
    const vpW = rect.width;
    const vpH = rect.height;

    const curZoom = zoom();

    if (e.ctrlKey || e.metaKey) {
      // Ctrl+scroll: snap to adjacent zoom level.
      const dir = e.deltaY < 0 ? 1 : -1;
      const newZoom = snapZoom(curZoom, dir as 1 | -1);
      const { scrollX: nx, scrollY: ny } = zoomAt(
        scrollX(),
        scrollY(),
        curZoom,
        vpW,
        vpH,
        sx,
        sy,
        newZoom,
      );
      setScrollX(nx);
      setScrollY(ny);
      setZoom(newZoom);
    } else if (e.shiftKey) {
      // Shift+scroll: pan horizontally.
      setScrollX(scrollX() + e.deltaY / curZoom);
    } else if (
      e.deltaMode === WheelEvent.DOM_DELTA_LINE ||
      e.deltaMode === WheelEvent.DOM_DELTA_PAGE
    ) {
      // Line/page mode: snap zoom.
      const dir = e.deltaY < 0 ? 1 : -1;
      const newZoom = snapZoom(curZoom, dir as 1 | -1);
      const { scrollX: nx, scrollY: ny } = zoomAt(
        scrollX(),
        scrollY(),
        curZoom,
        vpW,
        vpH,
        sx,
        sy,
        newZoom,
      );
      setScrollX(nx);
      setScrollY(ny);
      setZoom(newZoom);
    } else {
      // Plain scroll: smooth continuous zoom anchored at cursor.
      const factor = e.deltaY < 0 ? WHEEL_ZOOM_FACTOR : 1 / WHEEL_ZOOM_FACTOR;
      const newZoom = clampZoom(curZoom * factor);
      const { scrollX: nx, scrollY: ny } = zoomAt(
        scrollX(),
        scrollY(),
        curZoom,
        vpW,
        vpH,
        sx,
        sy,
        newZoom,
      );
      setScrollX(nx);
      setScrollY(ny);
      setZoom(newZoom);
    }

    scheduleViewportSync();
  }

  // ── Pan via middle-mouse or spacebar+drag ────────────────────────────

  function startPan(e: MouseEvent): void {
    pan.active = true;
    pan.lastX = e.clientX;
    pan.lastY = e.clientY;
    el.style.cursor = "grabbing";
  }

  function onMouseDown(e: MouseEvent): void {
    // Middle-mouse always pans.
    if (e.button === 1) {
      e.preventDefault();
      startPan(e);
      return;
    }
    // Left mouse + spacebar pans.
    if (e.button === 0 && spaceHeld) {
      e.preventDefault();
      startPan(e);
    }
  }

  function onMouseMove(e: MouseEvent): void {
    // Always update the cursor's canvas position so brush-cursor overlays
    // can track the pointer regardless of pan/draw state.
    const rect = el.getBoundingClientRect();
    const sx = e.clientX - rect.left;
    const sy = e.clientY - rect.top;
    if (sx >= 0 && sy >= 0 && sx <= rect.width && sy <= rect.height) {
      const [cx, cy] = screenToCanvas(
        sx,
        sy,
        scrollX(),
        scrollY(),
        zoom(),
        rect.width,
        rect.height,
      );
      setCursorCanvas({ x: cx, y: cy });
    } else {
      setCursorCanvas(null);
    }

    if (!pan.active) return;
    const dx = e.clientX - pan.lastX;
    const dy = e.clientY - pan.lastY;
    pan.lastX = e.clientX;
    pan.lastY = e.clientY;

    const z = zoom();
    setScrollX(scrollX() - dx / z);
    setScrollY(scrollY() - dy / z);
    scheduleViewportSync();
  }

  function onMouseLeave(): void {
    setCursorCanvas(null);
  }

  function onMouseUp(): void {
    if (pan.active) {
      pan.active = false;
      el.style.cursor = spaceHeld ? "grab" : "";
    }
  }

  // ── Space key ─────────────────────────────────────────────────────────

  function onKeyDown(e: KeyboardEvent): void {
    // Funnel through the shared editable-target check so spacebar pan
    // doesn't fire while the user is typing into an `<input>`,
    // `<textarea>`, `<select>`, or any contenteditable element. The
    // local check used to only handle `<input>`/`<textarea>`.
    if (isEditableTarget(e)) return;

    if (e.code === "Space" && !spaceHeld) {
      spaceHeld = true;
      if (!pan.active) el.style.cursor = "grab";
      e.preventDefault();
    }
  }

  function onKeyUp(e: KeyboardEvent): void {
    if (e.code === "Space") {
      spaceHeld = false;
      if (!pan.active) el.style.cursor = "";
    }
  }

  // ── Prevent context menu on middle-click ──────────────────────────────

  function onContextMenu(e: MouseEvent): void {
    // Suppress the context menu that Firefox shows on middle-click release.
    if (e.button === 1) e.preventDefault();
  }

  // ── Register ──────────────────────────────────────────────────────────

  el.addEventListener("wheel", onWheel, { passive: false });
  el.addEventListener("mousedown", onMouseDown);
  window.addEventListener("mousemove", onMouseMove);
  window.addEventListener("mouseup", onMouseUp);
  window.addEventListener("keydown", onKeyDown);
  window.addEventListener("keyup", onKeyUp);
  el.addEventListener("contextmenu", onContextMenu);
  el.addEventListener("mouseleave", onMouseLeave);

  return () => {
    el.removeEventListener("wheel", onWheel);
    el.removeEventListener("mousedown", onMouseDown);
    window.removeEventListener("mousemove", onMouseMove);
    window.removeEventListener("mouseup", onMouseUp);
    window.removeEventListener("keydown", onKeyDown);
    window.removeEventListener("keyup", onKeyUp);
    el.removeEventListener("contextmenu", onContextMenu);
    el.removeEventListener("mouseleave", onMouseLeave);
  };
}
