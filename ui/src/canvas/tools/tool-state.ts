// Reactive tool state for the brush engine.
//
// This module owns every signal the toolbar and options panel write.
// The canvas input handler reads them to decide what IPC call to dispatch.

import { createSignal } from "solid-js";

// ── Tool type ─────────────────────────────────────────────────────────────────

export type ToolType = "pencil" | "eraser" | "fill" | "rect" | "ellipse" | "line";

/** Tools that use the brush options panel (size, shape, pixel-perfect). */
export const BRUSH_TOOLS: readonly ToolType[] = ["pencil", "eraser", "line", "rect", "ellipse"];

// ── Active tool ───────────────────────────────────────────────────────────────

export const [activeTool, setActiveTool] = createSignal<ToolType>("pencil");

// ── Brush options ─────────────────────────────────────────────────────────────

export type BrushShape = "pixel" | "circle" | "square";

/** Brush diameter in canvas pixels (1–64). */
export const [toolSize, setToolSize] = createSignal(1);

export const [toolShape, setToolShape] = createSignal<BrushShape>("pixel");

/** Stroke opacity in 0–1 range. */
export const [toolOpacity, setToolOpacity] = createSignal(1.0);

/** When true, post-process strokes to remove diagonal corner artifacts. */
export const [pixelPerfect, setPixelPerfect] = createSignal(false);

/** Fill tolerance 0–255 (0 = exact match). */
export const [fillTolerance, setFillTolerance] = createSignal(0);

// ── Foreground / background colours ──────────────────────────────────────────

export type Rgba = { r: number; g: number; b: number; a: number };

export const [foregroundColor, setForegroundColor] = createSignal<Rgba>({
  r: 0,
  g: 0,
  b: 0,
  a: 255,
});

export const [backgroundColor, setBackgroundColor] = createSignal<Rgba>({
  r: 255,
  g: 255,
  b: 255,
  a: 255,
});
