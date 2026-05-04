// Viewport coordinate math for the canvas surface.
//
// All transforms are pure functions so they are testable without a DOM.
// "Canvas coordinates" are sprite pixel coordinates (0,0 = top-left of sprite).
// "Screen coordinates" are CSS pixels relative to the canvas element (0,0 = top-left).
//
// The viewport is centred: scroll(X|Y) is the canvas coordinate that maps to
// the viewport's centre point.

/** Zoom levels that the +/- keys and scroll wheel snap to. */
export const SNAP_ZOOMS = [
  1 / 16, // 6.25 %
  1 / 8, // 12.5 %
  1 / 4, // 25 %
  1 / 2, // 50 %
  1, // 100 %
  2, // 200 %
  4, // 400 %
  8, // 800 %
  16, // 1600 %
] as const satisfies readonly number[];

export const MIN_ZOOM = 1 / 16;
export const MAX_ZOOM = 16;

/**
 * Pixel grid becomes visible at this zoom level.
 * One canvas pixel is 4 CSS pixels at 400 %, large enough to draw gutters.
 */
export const PIXEL_GRID_ZOOM_THRESHOLD = 4;

// ── Transforms ────────────────────────────────────────────────────────────────

/** Canvas pixel (cx, cy) → screen pixel (sx, sy) relative to viewport element. */
export function canvasToScreen(
  cx: number,
  cy: number,
  scrollX: number,
  scrollY: number,
  zoom: number,
  vpW: number,
  vpH: number,
): [number, number] {
  return [(cx - scrollX) * zoom + vpW * 0.5, (cy - scrollY) * zoom + vpH * 0.5];
}

/** Screen pixel (sx, sy) → canvas pixel (cx, cy). Inverse of canvasToScreen. */
export function screenToCanvas(
  sx: number,
  sy: number,
  scrollX: number,
  scrollY: number,
  zoom: number,
  vpW: number,
  vpH: number,
): [number, number] {
  return [(sx - vpW * 0.5) / zoom + scrollX, (sy - vpH * 0.5) / zoom + scrollY];
}

/**
 * Recomputes scroll so that the canvas coordinate currently under
 * (screenX, screenY) stays fixed while the zoom changes to newZoom.
 */
export function zoomAt(
  scrollX: number,
  scrollY: number,
  zoom: number,
  vpW: number,
  vpH: number,
  screenX: number,
  screenY: number,
  newZoom: number,
): { scrollX: number; scrollY: number } {
  const [cx, cy] = screenToCanvas(screenX, screenY, scrollX, scrollY, zoom, vpW, vpH);
  return {
    scrollX: cx - (screenX - vpW * 0.5) / newZoom,
    scrollY: cy - (screenY - vpH * 0.5) / newZoom,
  };
}

/**
 * Returns the next snap zoom level in the given direction.
 * dir > 0 = zoom in, dir < 0 = zoom out.
 * Returns the nearest snap that is strictly beyond current in that direction.
 */
export function snapZoom(current: number, dir: 1 | -1): number {
  if (dir > 0) {
    const found = SNAP_ZOOMS.find((z) => z > current);
    return found !== undefined ? found : MAX_ZOOM;
  }
  const found = [...SNAP_ZOOMS].reverse().find((z) => z < current);
  return found !== undefined ? found : MIN_ZOOM;
}

/**
 * Clamps zoom to [MIN_ZOOM, MAX_ZOOM].
 * Use this when computing smooth (non-snap) zoom from scroll delta.
 */
export function clampZoom(z: number): number {
  return Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, z));
}

/**
 * Centres the viewport on the given sprite canvas rect (origin + size).
 * Returns the scroll values that frame the sprite with padding if the
 * current zoom makes it fit, or show it at 1:1 otherwise.
 */
export function scrollToCentre(
  spriteW: number,
  spriteH: number,
): { scrollX: number; scrollY: number } {
  return { scrollX: spriteW * 0.5, scrollY: spriteH * 0.5 };
}

/**
 * Fits the sprite into the viewport at the largest snap zoom where it
 * still fits, with at least `padding` CSS pixels on each side.
 */
export function fitZoom(
  spriteW: number,
  spriteH: number,
  vpW: number,
  vpH: number,
  padding = 16,
): number {
  const maxW = vpW - padding * 2;
  const maxH = vpH - padding * 2;
  if (maxW <= 0 || maxH <= 0 || spriteW === 0 || spriteH === 0) return 1;
  const raw = Math.min(maxW / spriteW, maxH / spriteH);
  return clampZoom(raw);
}
