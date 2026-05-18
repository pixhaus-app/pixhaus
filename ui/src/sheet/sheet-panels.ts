// Geometry helpers shared between the sheet editor and the sheet view.

import type { SheetComposition } from "../lib/types/SheetComposition";
import type { SheetPanel } from "../lib/types/SheetPanel";

export type FlatPanel = {
  label: string;
  x: number;
  y: number;
  w: number;
  h: number;
};

export const DEFAULT_SHEET_DIMENSION = 1024;

export function compositionPanels(composition: SheetComposition | null | undefined): SheetPanel[] {
  if (composition == null) return [];
  return [
    ...(composition.views ?? []),
    ...(composition.expressions ?? []),
    ...(composition.callouts ?? []),
    ...(composition.outfits ?? []),
  ];
}

export function flatPanels(composition: SheetComposition | null | undefined): FlatPanel[] {
  return compositionPanels(composition).map((panel) => ({
    label: panel.label,
    x: panel.region.origin.x,
    y: panel.region.origin.y,
    w: panel.region.size.width,
    h: panel.region.size.height,
  }));
}

/**
 * Computed sheet width. When `panels` is empty, returns the fallback (default
 * 1024). When panels exist, returns the rightmost panel edge — pass
 * `floor: DEFAULT_SHEET_DIMENSION` to enforce a minimum even when panels are
 * small.
 */
export function sheetWidth(
  composition: SheetComposition | null | undefined,
  options: { floor?: number; fallback?: number } = {},
): number {
  const { floor = 0, fallback = DEFAULT_SHEET_DIMENSION } = options;
  const panels = compositionPanels(composition);
  if (panels.length === 0) return fallback;
  return Math.max(floor, ...panels.map((p) => p.region.origin.x + p.region.size.width));
}

export function sheetHeight(
  composition: SheetComposition | null | undefined,
  options: { floor?: number; fallback?: number } = {},
): number {
  const { floor = 0, fallback = DEFAULT_SHEET_DIMENSION } = options;
  const panels = compositionPanels(composition);
  if (panels.length === 0) return fallback;
  return Math.max(floor, ...panels.map((p) => p.region.origin.y + p.region.size.height));
}
