// Solid reactive state for the canvas viewport.
//
// This module owns the viewport's mutable state as Solid signals.  The Canvas
// component reads them; input handlers write them.  Changes are periodically
// synced to the Rust side via canvas_set_viewport so the project's persisted
// canvas state stays current.

import { createSignal } from "solid-js";
import type { CanvasState, SpriteId } from "../lib/types";
import { canvasSetViewport } from "../lib/commands/canvas";

// ── Viewport state ─────────────────────────────────────────────────────────

export const [scrollX, setScrollX] = createSignal(0);
export const [scrollY, setScrollY] = createSignal(0);
export const [zoom, setZoom] = createSignal(1);
export const [onionSkin, setOnionSkin] = createSignal(false);
export const [showPixelGrid, setShowPixelGrid] = createSignal(true);
export const [showTileGrid, setShowTileGrid] = createSignal(false);

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
  // Skip sync when no sprite is active — the project may not be open yet.
  if (!sprite) return;

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

  // Fit zoom: largest snap that leaves 16 px padding.
  const padding = 16;
  const raw = Math.min((vpW - padding * 2) / spriteW, (vpH - padding * 2) / spriteH);
  const snaps = [1 / 16, 1 / 8, 1 / 4, 1 / 2, 1, 2, 4, 8, 16] as const;
  const candidates = snaps.filter((z) => z <= raw);
  const fit: number =
    candidates.length > 0 ? (candidates[candidates.length - 1] as number) : 1 / 16;
  setZoom(fit);

  setActiveSpriteId(spriteId);
  setActiveFrameIndex(0);
  scheduleViewportSync();
}
