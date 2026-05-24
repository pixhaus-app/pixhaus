// Reactive tool state for the brush engine.
//
// This module owns every field the toolbar and options panel write. The
// canvas input handler reads them to decide what IPC call to dispatch. The
// state is a single Solid store: consumers read `tool.activeTool`,
// `tool.size`, etc., and mutate through the named setter functions, which keep
// write call sites stable and the store's write surface encapsulated.

import { createStore } from "solid-js/store";

// ── Types ──────────────────────────────────────────────────────────────────

export type ToolType = "pencil" | "eraser" | "fill" | "rect" | "ellipse" | "line";

/** Tools that use the brush options panel (size, shape, pixel-perfect). */
export const BRUSH_TOOLS: readonly ToolType[] = ["pencil", "eraser", "line", "rect", "ellipse"];

export type BrushShape = "pixel" | "circle" | "square";

export type Rgba = { r: number; g: number; b: number; a: number };

export interface ToolState {
  activeTool: ToolType;
  /** Brush diameter in canvas pixels (1–64). */
  size: number;
  shape: BrushShape;
  /** Stroke opacity in 0–1 range. */
  opacity: number;
  /** When true, post-process strokes to remove diagonal corner artifacts. */
  pixelPerfect: boolean;
  /** Fill tolerance 0–255 (0 = exact match). */
  fillTolerance: number;
  foregroundColor: Rgba;
  backgroundColor: Rgba;
}

// ── Store ──────────────────────────────────────────────────────────────────

export const [tool, setTool] = createStore<ToolState>({
  activeTool: "pencil",
  size: 1,
  shape: "pixel",
  opacity: 1.0,
  pixelPerfect: false,
  fillTolerance: 0,
  foregroundColor: { r: 0, g: 0, b: 0, a: 255 },
  backgroundColor: { r: 255, g: 255, b: 255, a: 255 },
});

// ── Setters (encapsulate the store's write surface) ──────────────────────────

export function setActiveTool(value: ToolType): void {
  setTool("activeTool", value);
}

export function setToolSize(value: number): void {
  setTool("size", value);
}

export function setToolShape(value: BrushShape): void {
  setTool("shape", value);
}

export function setToolOpacity(value: number): void {
  setTool("opacity", value);
}

export function setPixelPerfect(value: boolean): void {
  setTool("pixelPerfect", value);
}

export function setFillTolerance(value: number): void {
  setTool("fillTolerance", value);
}

export function setForegroundColor(value: Rgba): void {
  setTool("foregroundColor", value);
}

export function setBackgroundColor(value: Rgba): void {
  setTool("backgroundColor", value);
}
