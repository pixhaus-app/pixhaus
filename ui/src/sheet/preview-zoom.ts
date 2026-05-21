// Pan/zoom math for the reference-sheet preview.
//
// The preview applies a CSS transform `translate(panX, panY) scale(scale)`
// with `transform-origin: 0 0` to the image wrap. This is a different model
// from the centred-scroll viewport in `../canvas/viewport`, so the helper
// lives here, but it reuses that module's zoom clamp and limits.
import { clampZoom } from "../canvas/viewport";

export type PreviewTransform = { scale: number; panX: number; panY: number };

// Zooms by `factor` while keeping the content point under (cursorX, cursorY)
// fixed. Cursor coordinates are CSS pixels relative to the transform
// container's top-left (its untransformed origin).
export function zoomToCursor(
  t: PreviewTransform,
  cursorX: number,
  cursorY: number,
  factor: number,
): PreviewTransform {
  const scale = clampZoom(t.scale * factor);
  const ratio = scale / t.scale;
  return {
    scale,
    panX: cursorX - (cursorX - t.panX) * ratio,
    panY: cursorY - (cursorY - t.panY) * ratio,
  };
}
