// Solid reactive state for the canvas viewport.
//
// This module owns the viewport's mutable state as Solid signals.  The Canvas
// component reads them; input handlers write them.  Changes are periodically
// synced to the Rust side via canvas_set_viewport so the project's persisted
// canvas state stays current.

import { createSignal } from "solid-js";
import type { CanvasState, SpriteId } from "../lib/types";
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

// Currently foregrounded sprite and frame.
export const [activeSpriteId, setActiveSpriteId] = createSignal<SpriteId | null>(null);
export const [activeFrameIndex, setActiveFrameIndex] = createSignal<number>(0);

// Active selection rect in canvas coordinates, null when no selection.
export const [selectionRect, setSelectionRect] = createSignal<{
  x: number;
  y: number;
  width: number;
  height: number;
} | null>(null);

// ── Brush preview state (S15 will own; defaults are stubs) ─────────────────

export type BrushShape = "round" | "square";

/** Brush diameter in canvas pixels. */
export const [brushSize, setBrushSize] = createSignal(1);
export const [brushShape, setBrushShape] = createSignal<BrushShape>("round");

// Cursor position in canvas coordinates, or null when the pointer is off-canvas.
export const [cursorCanvas, setCursorCanvas] = createSignal<{ x: number; y: number } | null>(null);

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

  const state: CanvasState = {
    active_sprite: sprite,
    active_layer: null,
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
  scheduleViewportSync();
}
