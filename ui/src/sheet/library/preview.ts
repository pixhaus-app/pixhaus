// Scaled layout preview helpers for paneled structures.
//
// Pure functions — no Solid reactivity, no DOM. Used by StructureEditor to
// render the live SVG preview and by tests to verify scaling math.

/**
 * A scaled bounding box for one panel, ready to render into a preview area.
 */
export interface PreviewBox {
  x: number;
  y: number;
  w: number;
  h: number;
  label: string;
}

/**
 * Scales a paneled structure's panel rects to fit within `maxW` x `maxH`,
 * preserving aspect ratio. The scale factor is `min(maxW/canvasW, maxH/canvasH)`.
 *
 * Each element in `panels` describes one panel with a label and a pixel rect
 * relative to the structure canvas.
 */
export function previewBoxes(
  canvasW: number,
  canvasH: number,
  panels: { label: string; rect: { x: number; y: number; w: number; h: number } }[],
  maxW: number,
  maxH: number,
): PreviewBox[] {
  if (canvasW <= 0 || canvasH <= 0) return [];
  const scale = Math.min(maxW / canvasW, maxH / canvasH);
  return panels.map(({ label, rect }) => ({
    x: rect.x * scale,
    y: rect.y * scale,
    w: rect.w * scale,
    h: rect.h * scale,
    label,
  }));
}
