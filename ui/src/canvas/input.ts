// Canvas input handling: pan, zoom, cursor feedback, and tile-paint mode.
//
// Returns a cleanup function to remove all listeners.  Mount by calling
// attachCanvasInput(canvasEl) inside a Solid onMount, then call the returned
// function in onCleanup.
//
// Tile-paint mode activates automatically when `activeTilemapCtx` is non-null.
// Left-click places the selected tile; right-click erases the cell. With
// autotile mode on, paints are routed through `tile_autotile_apply` using
// the kind chosen in the Rules tab (or the kind persisted on the tileset).

import {
  scrollX,
  scrollY,
  zoom,
  setScrollX,
  setScrollY,
  setZoom,
  scheduleViewportSync,
  setCursorCanvas,
  activeSpriteId,
  activeFrameIndex,
  activeLayerId,
  isSelectMode,
  setShapePreview,
  type ShapeKind,
} from "./canvas-state";
import {
  activeTool,
  fillTolerance,
  foregroundColor,
  pixelPerfect,
  toolShape,
  toolSize,
} from "./tools/tool-state";
import { attachSelectInput } from "./select/select-input";
import { transformDrag } from "./transform/transform-state";
import { snapZoom, clampZoom, zoomAt, screenToCanvas } from "./viewport";
import { isEditableTarget } from "../keybinds/keybind-manager";
import {
  activeTilemapCtx,
  localAutotileKind,
  selectedTileIndex,
  selectedTileFlags,
  tilemapTool,
  autotileMode,
} from "../tilemap/tilemap-state";
import { tilePlace, tileErase, tileAutotileApply } from "../lib/commands/tiles";
import {
  canvasBeginStroke,
  canvasDrawStroke,
  canvasEndStroke,
  canvasExtendStroke,
  canvasFill,
} from "../lib/commands/canvas";
import { reportCommandFailure } from "../lib/utils/errors";
import { isActiveLayerWritable } from "../layers/layer-state";
import { pushToast } from "../lib/toast/toast-state";
import { ellipsePerimeterPoints, linePoints, rectPerimeterPoints } from "./tools/shape-points";

// Toast is the canonical UX for "you can't paint here right now" — we use
// a single helper so the wording stays consistent across the brush, shape
// and fill paths.
function toastLayerLocked(): void {
  pushToast({
    kind: "info",
    title: "Layer is locked",
    body: "Unlock the layer to paint on it.",
    durationMs: 3000,
  });
}

// How much the continuous zoom changes per wheel tick (scroll-wheel smooth mode).
const WHEEL_ZOOM_FACTOR = 1.1;

// ── Tile-paint helpers ─────────────────────────────────────────────────────

/**
 * Converts a canvas-coordinate point to a tile cell address, given a tile
 * size.  Returns null when the coordinate is outside the valid grid range
 * (negative canvas position).
 */
function canvasToTileCell(
  cx: number,
  cy: number,
  tileW: number,
  tileH: number,
): { cellX: number; cellY: number } | null {
  if (cx < 0 || cy < 0) return null;
  // Guard against zero-sized tilesets — division would yield Infinity
  // and downstream IPC payloads would carry garbage cell coordinates.
  if (tileW <= 0 || tileH <= 0) return null;
  return { cellX: Math.floor(cx / tileW), cellY: Math.floor(cy / tileH) };
}

// Last cell painted in the current drag stroke. Reset to null when a
// new stroke starts so the first dispatch of every stroke fires; while
// the stroke is active, dispatchTilePaint suppresses repeats over the
// same cell to avoid spamming IPC on every mousemove.
let lastPaintedCell: { x: number; y: number } | null = null;

/**
 * Dispatches a tile place or erase IPC call for the given screen position.
 *
 * Errors surface via the toast host so the user sees the validation
 * detail (e.g. "tile index N out of range" when the tileset is empty)
 * instead of a silent no-op.
 */
function dispatchTilePaint(
  screenX: number,
  screenY: number,
  el: HTMLElement,
  erase: boolean,
): void {
  const ctx = activeTilemapCtx();
  if (!ctx) return;

  const spriteId = activeSpriteId();
  const layerId = activeLayerId();
  if (spriteId === null || layerId === null) return;

  const rect = el.getBoundingClientRect();
  const sx = screenX - rect.left;
  const sy = screenY - rect.top;
  const [cx, cy] = screenToCanvas(sx, sy, scrollX(), scrollY(), zoom(), rect.width, rect.height);

  const { tile_size } = ctx.tileset;
  const cell = canvasToTileCell(cx, cy, tile_size.width, tile_size.height);
  if (!cell) return;

  // Skip the IPC if the stroke has already painted this cell. The
  // cursor still moves smoothly because cell tracking happens upstream
  // of this function.
  if (
    lastPaintedCell !== null &&
    lastPaintedCell.x === cell.cellX &&
    lastPaintedCell.y === cell.cellY
  ) {
    return;
  }
  lastPaintedCell = { x: cell.cellX, y: cell.cellY };

  const frameIndex = activeFrameIndex();
  const onErr = (err: unknown): void => {
    reportCommandFailure("tile paint", err);
  };

  if (erase) {
    tileErase({
      sprite_id: spriteId,
      layer_id: layerId,
      frame_index: frameIndex,
      cell_x: cell.cellX,
      cell_y: cell.cellY,
    }).catch(onErr);
    return;
  }

  if (autotileMode()) {
    const kind = localAutotileKind() ?? ctx.tileset.autotile;
    if (kind === null || kind === undefined) {
      reportCommandFailure(
        "tile autotile",
        "no autotile rule set selected (open the Rules tab to pick one)",
      );
      return;
    }
    tileAutotileApply({
      sprite_id: spriteId,
      layer_id: layerId,
      frame_index: frameIndex,
      kind,
      source_tile: selectedTileIndex(),
    }).catch(onErr);
    return;
  }

  tilePlace({
    sprite_id: spriteId,
    layer_id: layerId,
    frame_index: frameIndex,
    cell_x: cell.cellX,
    cell_y: cell.cellY,
    cell: { index: selectedTileIndex(), flags: selectedTileFlags() },
  }).catch(onErr);
}

/** Resets the per-stroke deduplication so the next stroke starts fresh. */
function resetTilePaintStroke(): void {
  lastPaintedCell = null;
}

// ── Freehand stroke state ──────────────────────────────────────────────────
//
// Pencil/eraser/line strokes use the begin/extend/end stroke session API
// so the user sees pixels under the cursor in real time and Ctrl+Z
// reverts one whole drag (not partial mousemoves).
//
// Per-attach state lives in `attachCanvasInput` below — nothing here
// because there is only one active drawing tool at a time and the
// session id alone is enough to identify the in-flight stroke.

// Returns the canvas-pixel under the cursor, snapped to integer
// coordinates so it lines up with the cell the user sees under the
// brush-cursor preview. Without the floor here, freehand strokes
// would round differently from the preview at fractional positions —
// the preview would show cell (3,3) and the paint would land in (4,4).
function canvasPointFromEvent(e: MouseEvent, el: HTMLElement): [number, number] {
  const rect = el.getBoundingClientRect();
  const sx = e.clientX - rect.left;
  const sy = e.clientY - rect.top;
  const [cx, cy] = screenToCanvas(sx, sy, scrollX(), scrollY(), zoom(), rect.width, rect.height);
  return [Math.floor(cx), Math.floor(cy)];
}

function isDrawingTool(): boolean {
  const t = activeTool();
  return t === "pencil" || t === "eraser";
}

function isShapeTool(): boolean {
  const t = activeTool();
  return t === "rect" || t === "ellipse" || t === "line";
}

function isFillTool(): boolean {
  return activeTool() === "fill";
}

// Anchor point of an in-progress shape (line/rect/ellipse) drag, in
// canvas pixels.
let shapeAnchor: [number, number] | null = null;

function dispatchShape(end: [number, number]): void {
  if (shapeAnchor === null) return;
  const start = shapeAnchor;
  shapeAnchor = null;

  const spriteId = activeSpriteId();
  const layerId = activeLayerId();
  if (spriteId === null || layerId === null) return;
  if (!isActiveLayerWritable()) {
    toastLayerLocked();
    return;
  }

  const tool = activeTool();
  let points: Array<[number, number]>;
  if (tool === "rect") {
    points = rectPerimeterPoints(start[0], start[1], end[0], end[1]);
  } else if (tool === "ellipse") {
    points = ellipsePerimeterPoints(start[0], start[1], end[0], end[1]);
  } else if (tool === "line") {
    points = linePoints(start[0], start[1], end[0], end[1]);
  } else {
    return;
  }
  if (points.length === 0) return;

  const color = foregroundColor();
  canvasDrawStroke({
    sprite_id: spriteId,
    layer_id: layerId,
    frame_index: activeFrameIndex(),
    points,
    color: { r: color.r, g: color.g, b: color.b, a: color.a },
    pressure: points.map(() => 1.0),
    brush_shape: toolShape(),
    brush_size: toolSize(),
    pixel_perfect: pixelPerfect(),
    erase: false,
  }).catch((err: unknown) => {
    reportCommandFailure("canvas_draw_stroke (shape)", err);
  });
}

function dispatchFill(canvasX: number, canvasY: number): void {
  const spriteId = activeSpriteId();
  const layerId = activeLayerId();
  if (spriteId === null || layerId === null) return;
  if (!isActiveLayerWritable()) {
    toastLayerLocked();
    return;
  }

  const color = foregroundColor();
  canvasFill({
    sprite_id: spriteId,
    layer_id: layerId,
    frame_index: activeFrameIndex(),
    x: Math.floor(canvasX),
    y: Math.floor(canvasY),
    color: { r: color.r, g: color.g, b: color.b, a: color.a },
    tolerance: fillTolerance(),
  }).catch((err: unknown) => {
    reportCommandFailure("canvas_fill", err);
  });
}

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
 *
 * Routing priority (highest to lowest):
 *   1. Transform drag in progress (transform-state)
 *   2. Space + drag / middle-mouse → pan
 *   3. Tilemap paint mode (activeTilemapCtx non-null)
 *   4. Selection mode (isSelectMode true) → selection input
 */
export function attachCanvasInput(el: HTMLElement): () => void {
  const pan: PanState = { active: false, lastX: 0, lastY: 0 };
  let spaceHeld = false;
  // Tracks whether a tile-paint drag stroke is in progress.
  let tilePaintActive = false;
  let tilePaintErase = false;
  // Tracks whether a rect/ellipse anchor + drag is in progress.
  let shapeActive = false;

  // ── Freehand stroke session state ───────────────────────────────────────
  //
  // `drawStrokeActive` is set synchronously on mousedown so mousemove
  // starts collecting points before the begin IPC resolves.
  // `strokeSessionId` arrives a moment later when the backend hands us
  // an id; until then `pendingStrokePoints` buffers and a flush is
  // attempted on resolve. RAF batches mousemove (60Hz) into one IPC
  // per frame.
  //
  // `strokeIpcChain` serialises every extend / end call for an
  // in-flight stroke. Without it, two extends could race on the
  // backend doc lock and the second one's points might land before
  // the first; or worse, an `end` could commit before a still-pending
  // `extend`, dropping those points and triggering a session-not-found
  // toast on the late extend. Chaining means we never have more than
  // one stroke IPC in flight at a time.
  let drawStrokeActive = false;
  let strokeSessionId: number | null = null;
  let pendingStrokePoints: Array<[number, number]> = [];
  let strokeRafHandle: number | null = null;
  let strokeIpcChain: Promise<void> = Promise.resolve();

  function scheduleStrokeFlush(): void {
    if (strokeRafHandle !== null) return;
    strokeRafHandle = requestAnimationFrame(() => {
      strokeRafHandle = null;
      if (strokeSessionId === null) return;
      if (pendingStrokePoints.length === 0) return;
      const points = pendingStrokePoints;
      pendingStrokePoints = [];
      const sid = strokeSessionId;
      strokeIpcChain = strokeIpcChain.then(() =>
        canvasExtendStroke({ session_id: sid, new_points: points }).catch((err: unknown) => {
          reportCommandFailure("canvas_extend_stroke", err);
        }),
      );
    });
  }

  function startStroke(firstPoint: [number, number], erase: boolean): void {
    const spriteId = activeSpriteId();
    const layerId = activeLayerId();
    if (spriteId === null || layerId === null) return;
    if (!isActiveLayerWritable()) {
      toastLayerLocked();
      return;
    }

    drawStrokeActive = true;
    pendingStrokePoints = [];
    strokeSessionId = null;
    // Reset the chain — a fresh stroke starts a fresh ordering.
    strokeIpcChain = Promise.resolve();

    const color = foregroundColor();
    canvasBeginStroke({
      sprite_id: spriteId,
      layer_id: layerId,
      frame_index: activeFrameIndex(),
      color: { r: color.r, g: color.g, b: color.b, a: color.a },
      brush_shape: toolShape(),
      brush_size: toolSize(),
      pixel_perfect: pixelPerfect(),
      erase,
      first_point: firstPoint,
    })
      .then((id: number) => {
        // The session may have been ended before begin resolved (fast
        // click + release). In that case `drawStrokeActive` is already
        // false and `endStroke()` set up nothing to flush — close the
        // orphan session right away with whatever points we collected
        // so we still record an undo entry.
        if (!drawStrokeActive) {
          const finalPoints = pendingStrokePoints;
          pendingStrokePoints = [];
          strokeIpcChain = strokeIpcChain.then(() =>
            canvasEndStroke({ session_id: id, new_points: finalPoints }).catch((err: unknown) => {
              reportCommandFailure("canvas_end_stroke", err);
            }),
          );
          return;
        }
        strokeSessionId = id;
        if (pendingStrokePoints.length > 0) scheduleStrokeFlush();
      })
      .catch((err: unknown) => {
        drawStrokeActive = false;
        pendingStrokePoints = [];
        reportCommandFailure("canvas_begin_stroke", err);
      });
  }

  function endStroke(): void {
    if (!drawStrokeActive) return;
    drawStrokeActive = false;
    if (strokeRafHandle !== null) {
      cancelAnimationFrame(strokeRafHandle);
      strokeRafHandle = null;
    }
    if (strokeSessionId === null) {
      // begin_stroke hasn't resolved yet. Keep `pendingStrokePoints`
      // intact — the begin .then handler detects `!drawStrokeActive`
      // and forwards the points straight to `canvas_end_stroke` so the
      // stroke still commits as one undo entry.
      return;
    }
    const session = strokeSessionId;
    strokeSessionId = null;
    const finalPoints = pendingStrokePoints;
    pendingStrokePoints = [];
    // Chain the end after every queued extend so the backend never
    // sees an extend land after end (which would drop points + toast
    // a NotFound).
    strokeIpcChain = strokeIpcChain.then(() =>
      canvasEndStroke({ session_id: session, new_points: finalPoints }).catch((err: unknown) => {
        reportCommandFailure("canvas_end_stroke", err);
      }),
    );
  }

  // Attach the selection input handler. It manages its own listener
  // lifecycle internally; we just hold the cleanup reference.
  const detachSelectInput = attachSelectInput(el);

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
    // Middle-mouse always pans regardless of active tool.
    if (e.button === 1) {
      e.preventDefault();
      startPan(e);
      return;
    }

    // A transform drag owns the pointer. pointerdown on a gizmo handle fires
    // before this mousedown and sets transformDrag, so bailing here stops the
    // press from also starting a brush stroke (the cause of the draw-while-
    // transforming + flicker bug). Space-pan still wins so the user can pan.
    if (transformDrag() !== null && !spaceHeld) return;

    // Selection mode is delegated to the selection input handler; never draw.
    if (isSelectMode() && !spaceHeld) return;

    // Tile-paint mode: left-click places, right-click erases.
    if (activeTilemapCtx() !== null && !spaceHeld) {
      if (e.button === 0) {
        e.preventDefault();
        tilePaintActive = true;
        tilePaintErase = tilemapTool() === "erase";
        resetTilePaintStroke();
        dispatchTilePaint(e.clientX, e.clientY, el, tilePaintErase);
        return;
      }
      if (e.button === 2) {
        e.preventDefault();
        tilePaintActive = true;
        tilePaintErase = true;
        resetTilePaintStroke();
        dispatchTilePaint(e.clientX, e.clientY, el, true);
        return;
      }
    }

    // Drawing / fill tools: left-click only, no spacebar (spacebar pans).
    if (e.button === 0 && !spaceHeld && activeTilemapCtx() === null) {
      if (isFillTool()) {
        e.preventDefault();
        const [cx, cy] = canvasPointFromEvent(e, el);
        dispatchFill(cx, cy);
        return;
      }
      if (isDrawingTool()) {
        e.preventDefault();
        const point = canvasPointFromEvent(e, el);
        const erase = activeTool() === "eraser";
        startStroke(point, erase);
        return;
      }
      if (isShapeTool()) {
        e.preventDefault();
        shapeActive = true;
        const anchor = canvasPointFromEvent(e, el);
        shapeAnchor = anchor;
        // Show the preview at the anchor immediately so the user gets
        // feedback before the first mousemove.
        const tool = activeTool();
        const kind: ShapeKind | null =
          tool === "line" || tool === "rect" || tool === "ellipse" ? tool : null;
        if (kind !== null) {
          setShapePreview({ kind, start: anchor, end: anchor });
        }
        return;
      }
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

    // Continue tile-paint drag.
    if (tilePaintActive) {
      dispatchTilePaint(e.clientX, e.clientY, el, tilePaintErase);
    }

    // Accumulate stroke points and schedule a RAF flush. The flush
    // batches mousemove (60Hz) into one extend IPC per frame so the
    // backend never re-rasterizes more than once per repaint.
    if (drawStrokeActive) {
      pendingStrokePoints.push(canvasPointFromEvent(e, el));
      scheduleStrokeFlush();
    }

    // Update the shape preview so the outline tracks the cursor
    // during a line/rect/ellipse drag. The actual paint still
    // dispatches on mouseup via dispatchShape.
    if (shapeActive && shapeAnchor !== null) {
      const end = canvasPointFromEvent(e, el);
      const tool = activeTool();
      const kind: ShapeKind | null =
        tool === "line" || tool === "rect" || tool === "ellipse" ? tool : null;
      if (kind !== null) {
        setShapePreview({ kind, start: shapeAnchor, end });
      }
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

  function onMouseUp(e: MouseEvent): void {
    tilePaintActive = false;
    resetTilePaintStroke();

    if (drawStrokeActive) {
      endStroke();
    }

    if (shapeActive) {
      shapeActive = false;
      const end = canvasPointFromEvent(e, el);
      // Clear the preview before dispatch so the user sees only the
      // committed pixels (not preview + paint stacked).
      setShapePreview(null);
      dispatchShape(end);
    }

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
    if (strokeRafHandle !== null) {
      cancelAnimationFrame(strokeRafHandle);
      strokeRafHandle = null;
    }
    if (drawStrokeActive) {
      endStroke();
    }
    detachSelectInput();
  };
}
