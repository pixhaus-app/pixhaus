// Solid reactive state for the canvas viewport.
//
// This module owns the viewport's mutable state as a single Solid store
// (`viewport`). The Canvas component and overlays read it as `viewport.zoom`,
// `viewport.scrollX`, etc.; input handlers write it through the named setter
// functions, which keep the store's write surface encapsulated. Changes are
// periodically synced to the Rust side via canvas_set_viewport so the
// project's persisted canvas state stays current.

import { createStore } from "solid-js/store";
import type { CanvasState, LayerId, SpriteId } from "../lib/types";
import { canvasSetViewport } from "../lib/commands/canvas";
import { fitZoom } from "./viewport";
import {
  activeSpriteId,
  setActiveSpriteId,
  activeFrameIndex,
  setActiveFrameIndex,
  activeLayerId,
  setActiveLayerId,
} from "../stores/active-store";

// Currently foregrounded sprite, frame, and layer.
//
// The triple lives in stores/active-store.ts (a leaf module) so canvas and the
// layer panel can both read it without importing each other. Re-export here so
// existing `from "../canvas/canvas-state"` imports keep resolving.
export {
  activeSpriteId,
  setActiveSpriteId,
  activeFrameIndex,
  setActiveFrameIndex,
  activeLayerId,
  setActiveLayerId,
} from "../stores/active-store";

// ── Types ──────────────────────────────────────────────────────────────────

export interface RectBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Shape of the active selection. "rect" is an exact rectangle (drag marquee);
 * "mask" is an arbitrary region (wand, lasso, ellipse, color range) for which
 * selectionRect holds only the bounding box. The transform gizmo shows for
 * "rect" selections only. */
export type SelectionKind = "rect" | "mask";

export interface SelectionMaskData extends RectBounds {
  /** One alpha byte per pixel (0 = out, 255 = in), cropped to the bounding box. */
  data: Uint8Array;
}

export type ShapeKind = "line" | "rect" | "ellipse";
export interface ShapePreviewState {
  kind: ShapeKind;
  start: [number, number];
  end: [number, number];
}

export interface ViewportState {
  scrollX: number;
  scrollY: number;
  zoom: number;
  onionSkin: boolean;
  showPixelGrid: boolean;
  showTileGrid: boolean;
  /** Canvas pixels between major-grid lines when showTileGrid is on. */
  gridSpacing: number;
  /** Onion skin: neighbour frames to overlay and the overlay opacity. The
   * renderer reads these when onionSkin is true. */
  onionSkinPrev: number;
  onionSkinNext: number;
  onionSkinOpacity: number;
  /** Active selection rect in canvas coordinates, null when no selection. */
  selectionRect: RectBounds | null;
  selectionKind: SelectionKind | null;
  /** Per-pixel mask of the active "mask"-kind selection; null for rect
   * selections. The renderer uploads it as a texture to trace the true
   * outline. */
  selectionMask: SelectionMaskData | null;
  /** Layer the active selection was anchored on (mirrors the backend's
   * anchor_layer), so non-paint consumers know which layer it refers to. */
  selectionLayerId: LayerId | null;
  /** Whether the canvas is in selection-tool mode; input routing checks it
   * before dispatching pointer events to the selection handlers. */
  isSelectMode: boolean;
  /** Cursor position in canvas coordinates, null when off-canvas. */
  cursorCanvas: { x: number; y: number } | null;
  /** Outline preview while a line/rect/ellipse drag is in progress. */
  shapePreview: ShapePreviewState | null;
  /** Bounding box drawn with resize + rotation handles while transforming. */
  transformBounds: RectBounds | null;
}

// ── Store ──────────────────────────────────────────────────────────────────

export const [viewport, setViewport] = createStore<ViewportState>({
  scrollX: 0,
  scrollY: 0,
  zoom: 1,
  onionSkin: false,
  showPixelGrid: true,
  showTileGrid: false,
  gridSpacing: 8,
  onionSkinPrev: 1,
  onionSkinNext: 1,
  onionSkinOpacity: 0.4,
  selectionRect: null,
  selectionKind: null,
  selectionMask: null,
  selectionLayerId: null,
  isSelectMode: false,
  cursorCanvas: null,
  shapePreview: null,
  transformBounds: null,
});

// ── Setters (encapsulate the store's write surface) ──────────────────────────

export const setScrollX = (v: number): void => setViewport("scrollX", v);
export const setScrollY = (v: number): void => setViewport("scrollY", v);
export const setZoom = (v: number): void => setViewport("zoom", v);
export const setOnionSkin = (v: boolean): void => setViewport("onionSkin", v);
export const setShowPixelGrid = (v: boolean): void => setViewport("showPixelGrid", v);
export const setShowTileGrid = (v: boolean): void => setViewport("showTileGrid", v);
export const setGridSpacing = (v: number): void => setViewport("gridSpacing", v);
export const setOnionSkinPrev = (v: number): void => setViewport("onionSkinPrev", v);
export const setOnionSkinNext = (v: number): void => setViewport("onionSkinNext", v);
export const setOnionSkinOpacity = (v: number): void => setViewport("onionSkinOpacity", v);
export const setSelectionRect = (v: RectBounds | null): void => setViewport("selectionRect", v);
export const setSelectionKind = (v: SelectionKind | null): void => setViewport("selectionKind", v);
export const setSelectionMask = (v: SelectionMaskData | null): void =>
  setViewport("selectionMask", v);
export const setSelectionLayerId = (v: LayerId | null): void => setViewport("selectionLayerId", v);
export const setIsSelectMode = (v: boolean): void => setViewport("isSelectMode", v);
export const setCursorCanvas = (v: { x: number; y: number } | null): void =>
  setViewport("cursorCanvas", v);
export const setShapePreview = (v: ShapePreviewState | null): void =>
  setViewport("shapePreview", v);
export const setTransformBounds = (v: RectBounds | null): void => setViewport("transformBounds", v);

// ── Brush preview shims ──────────────────────────────────────────────────────
//
// The canonical tool state lives in tools/tool-state.ts (owned by S15). These
// accessor shims let Canvas.tsx and overlays.tsx read brush size/shape as
// functions from a single stable import path.

import { tool, type BrushShape } from "./tools/tool-state";

export type { BrushShape } from "./tools/tool-state";
export const brushSize = (): number => tool.size;
export const brushShape = (): BrushShape => tool.shape;

// ── Derived helpers ─────────────────────────────────────────────────────────

/** Snapshot of the viewport state for passing to the renderer. */
export function viewportSnapshot(width: number, height: number) {
  return {
    scrollX: viewport.scrollX,
    scrollY: viewport.scrollY,
    zoom: viewport.zoom,
    width,
    height,
  };
}

// ── Sync to Rust ────────────────────────────────────────────────────────────

// Debounce: only push to Rust 200 ms after the last viewport change.
let syncTimer: ReturnType<typeof setTimeout> | null = null;

/** Schedules a deferred sync of the current viewport state to the Rust side. */
export function scheduleViewportSync(): void {
  if (syncTimer !== null) clearTimeout(syncTimer);
  syncTimer = setTimeout(() => {
    syncTimer = null;
    pushViewportToRust();
  }, 200);
}

function pushViewportToRust(): void {
  const sprite = activeSpriteId();
  // Skip sync when no sprite is active — the project may not be open yet. Use
  // an explicit null check: SpriteId is a numeric newtype and `0` is a valid
  // id, which `if (!sprite)` would silently swallow.
  if (sprite === null) return;

  // The Rust CanvasState no longer carries `active_sprite` after the B9.1
  // cleanup; library focus now lives on `Project.active`. The IPC command
  // still needs to know which sprite the viewport belongs to, but the
  // on-the-wire CanvasState only carries layer/frame/zoom.
  const state: CanvasState = {
    active_layer: activeLayerId(),
    active_frame: activeFrameIndex(),
    scroll_x: viewport.scrollX,
    scroll_y: viewport.scrollY,
    zoom: viewport.zoom,
    onion_skin: viewport.onionSkin,
    show_tile_grid: viewport.showTileGrid,
  };

  canvasSetViewport(state).catch((err: unknown) => {
    console.warn("[pixhaus] canvas_set_viewport failed:", err);
  });
}

// ── Reset ───────────────────────────────────────────────────────────────────

/**
 * Clears all per-document canvas state — the active sprite/frame/layer triple
 * and any selection. Called when a project closes or the active project
 * changes identity so the previous document's selections don't leak into the
 * next one.
 */
export function resetCanvasState(): void {
  setActiveSpriteId(null);
  setActiveFrameIndex(0);
  setActiveLayerId(null);
  setViewport({
    selectionRect: null,
    selectionKind: null,
    selectionMask: null,
    transformBounds: null,
    isSelectMode: false,
  });
}

/**
 * Resets the viewport to centre the sprite and pick a fit-to-window zoom.
 * Called when a new sprite is loaded.
 */
export function resetViewport(
  spriteW: number,
  spriteH: number,
  vpW: number,
  vpH: number,
  spriteId: SpriteId,
): void {
  // Centre on the sprite. Fit zoom delegates to viewport.ts so the snap/padding
  // rules stay in one place across the keyboard shortcuts and this reset path.
  setViewport({
    scrollX: spriteW * 0.5,
    scrollY: spriteH * 0.5,
    zoom: fitZoom(spriteW, spriteH, vpW, vpH),
  });

  setActiveSpriteId(spriteId);
  setActiveFrameIndex(0);
  setActiveLayerId(null);
  scheduleViewportSync();
}
