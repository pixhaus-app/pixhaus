// Solid reactive state for the canvas viewport.
//
// This module owns the viewport's mutable state as Solid signals.  The Canvas
// component reads them; input handlers write them.  Changes are periodically
// synced to the Rust side via canvas_set_viewport so the project's persisted
// canvas state stays current.

import { createSignal } from "solid-js";
import type { CanvasState, LayerId, SpriteId } from "../lib/types";
import { canvasSetViewport } from "../lib/commands/canvas";
import { fitZoom } from "./viewport";

// ── Viewport state ─────────────────────────────────────────────────────────

export const [scrollX, setScrollX] = createSignal(0);
export const [scrollY, setScrollY] = createSignal(0);
export const [zoom, setZoom] = createSignal(1);
export const [onionSkin, setOnionSkin] = createSignal(false);
export const [showPixelGrid, setShowPixelGrid] = createSignal(true);
export const [showTileGrid, setShowTileGrid] = createSignal(false);

// Number of canvas pixels between major-grid lines when showTileGrid is on.
export const [gridSpacing, setGridSpacing] = createSignal(8);

// Onion skin: how many neighbour frames to overlay and at what opacity.
// onionSkin (above) is the on/off toggle. These three control the overlay's
// shape; the renderer reads them when onionSkin() is true.
export const [onionSkinPrev, setOnionSkinPrev] = createSignal(1);
export const [onionSkinNext, setOnionSkinNext] = createSignal(1);
export const [onionSkinOpacity, setOnionSkinOpacity] = createSignal(0.4);

// Currently foregrounded sprite, frame, and layer.
//
// activeLayerId is owned by this module (not the layer panel) so the
// canvas input handler can read it without a circular import. The
// layer panel writes to it via setActiveLayerId on selection.
export const [activeSpriteId, setActiveSpriteId] = createSignal<SpriteId | null>(null);
export const [activeFrameIndex, setActiveFrameIndex] = createSignal<number>(0);
export const [activeLayerId, setActiveLayerId] = createSignal<LayerId | null>(null);

// Active selection rect in canvas coordinates, null when no selection.
export const [selectionRect, setSelectionRect] = createSignal<{
  x: number;
  y: number;
  width: number;
  height: number;
} | null>(null);

// Shape of the active selection. "rect" is an exact rectangle (drag marquee);
// "mask" is an arbitrary region (wand, lasso, ellipse, color range) for which
// `selectionRect` only holds the bounding box. The transform gizmo is shown
// for "rect" selections only — resize handles on a mask's bounding box would
// imply a rectangular transform the mask doesn't actually support.
export const [selectionKind, setSelectionKind] = createSignal<"rect" | "mask" | null>(null);

// Per-pixel mask of the active "mask"-kind selection, cropped to its bounding
// box, with `data` as one alpha byte per pixel (0 = out, 255 = in). null for
// rect selections and when nothing is selected. The renderer uploads this as a
// texture to trace the selection's true outline; without it a mask could only
// be drawn as its bounding box.
export const [selectionMask, setSelectionMask] = createSignal<{
  x: number;
  y: number;
  width: number;
  height: number;
  data: Uint8Array;
} | null>(null);

// Layer the active selection was anchored on. Mirrors the `anchor_layer`
// the backend stored when `canvas_set_selection` ran (see select-input).
// Lets non-paint consumers (e.g. tile capture) know which layer's pixels
// the selection refers to without re-querying the backend.
export const [selectionLayerId, setSelectionLayerId] = createSignal<LayerId | null>(null);

// Whether the canvas is in selection-tool mode. Input routing checks this
// before dispatching pointer events to the selection handlers.
// S16 sets it to true when the user activates any select tool.
export const [isSelectMode, setIsSelectMode] = createSignal(false);

// ── Brush preview state ───────────────────────────────────────────────────────
//
// The canonical tool signals live in tools/tool-state.ts (owned by S15).
// These shims re-export them so Canvas.tsx and overlays.tsx keep reading from
// a single stable import path without touching those files.

export type { BrushShape } from "./tools/tool-state";
export { toolSize as brushSize, toolShape as brushShape } from "./tools/tool-state";

// Cursor position in canvas coordinates, or null when the pointer is off-canvas.
export const [cursorCanvas, setCursorCanvas] = createSignal<{ x: number; y: number } | null>(null);

// ── Shape preview ──────────────────────────────────────────────────────────
//
// Set by the canvas input handler while a line/rect/ellipse drag is in
// progress so the overlay can render the outline the user is about to
// commit. `null` means no shape drag is active. The kind is captured at
// drag-start so a tool change mid-drag (which the input handler doesn't
// allow today, but defense-in-depth) can't morph the preview.
export type ShapeKind = "line" | "rect" | "ellipse";
export type ShapePreviewState = {
  kind: ShapeKind;
  start: [number, number];
  end: [number, number];
};
export const [shapePreview, setShapePreview] = createSignal<ShapePreviewState | null>(null);

// ── Transform target (S16 will own; default null) ──────────────────────────

/**
 * Bounding box drawn with eight resize handles + one rotation handle.
 * S16 will set this when a selection is being transformed; for now the
 * signal exists so renderer/Canvas wiring is in place.
 */
export const [transformBounds, setTransformBounds] = createSignal<{
  x: number;
  y: number;
  width: number;
  height: number;
} | null>(null);

// ── Derived helpers ─────────────────────────────────────────────────────────

/** Snapshot of the viewport state for passing to the renderer. */
export function viewportSnapshot(width: number, height: number) {
  return {
    scrollX: scrollX(),
    scrollY: scrollY(),
    zoom: zoom(),
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
  // Skip sync when no sprite is active — the project may not be open
  // yet. Use an explicit null check: SpriteId is a numeric newtype and
  // `0` is a valid id, which `if (!sprite)` would silently swallow.
  if (sprite === null) return;

  // The Rust CanvasState no longer carries `active_sprite` after the
  // B9.1 cleanup; library focus now lives on `Project.active`. The IPC
  // command still needs to know which sprite the viewport belongs to,
  // but the on-the-wire CanvasState only carries layer/frame/zoom.
  const state: CanvasState = {
    active_layer: activeLayerId(),
    active_frame: activeFrameIndex(),
    scroll_x: scrollX(),
    scroll_y: scrollY(),
    zoom: zoom(),
    onion_skin: onionSkin(),
    show_tile_grid: showTileGrid(),
  };

  canvasSetViewport(state).catch((err: unknown) => {
    console.warn("[pixhaus] canvas_set_viewport failed:", err);
  });
}

// ── Reset ───────────────────────────────────────────────────────────────────

/**
 * Clears all per-document canvas state — the active sprite/frame/layer
 * triple and any selection. Called when a project closes or the active
 * project changes identity so the previous document's selections don't
 * leak into the next one (e.g. preventing the auto-select-first-sprite
 * effect from short-circuiting on a stale `activeSpriteId`).
 */
export function resetCanvasState(): void {
  setActiveSpriteId(null);
  setActiveFrameIndex(0);
  setActiveLayerId(null);
  setSelectionRect(null);
  setSelectionKind(null);
  setTransformBounds(null);
  setIsSelectMode(false);
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
  // Centre on the sprite.
  setScrollX(spriteW * 0.5);
  setScrollY(spriteH * 0.5);

  // Fit zoom delegates to viewport.ts so the snap/padding rules stay
  // in one place — both the keyboard zoom shortcuts and this reset path
  // pick the same value for a given sprite + viewport pair.
  setZoom(fitZoom(spriteW, spriteH, vpW, vpH));

  setActiveSpriteId(spriteId);
  setActiveFrameIndex(0);
  setActiveLayerId(null);
  scheduleViewportSync();
}
