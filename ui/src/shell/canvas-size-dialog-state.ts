// Open/close state and last-used-size persistence for CanvasSizeDialog.
//
// The dialog is shared by Welcome's "New Project" button and the command
// palette's "sprite:new" entry. The caller supplies an `onConfirm`
// callback so the dialog stays decoupled from the create paths.

import { createSignal } from "solid-js";
import { loadStorageJSON } from "../lib/utils/storage";

export type CanvasSizeMode = "project" | "sprite";

export type CanvasSizeResult = {
  /** Project name. Only meaningful when mode === "project". */
  name?: string;
  width: number;
  height: number;
  /** Optional embedded reference sheet for sprite creation. */
  reference_bytes?: number[];
  reference_mime?: string;
};

export type CanvasSizeRequest = {
  mode: CanvasSizeMode;
  onConfirm: (result: CanvasSizeResult) => void;
};

const LAST_SIZE_KEY = "pixhaus:last-canvas-size";

export const MIN_CANVAS_DIM = 1;
export const MAX_CANVAS_DIM = 8192;

export type CanvasSize = { width: number; height: number };

export const DEFAULT_CANVAS_SIZE: CanvasSize = { width: 32, height: 32 };

function isCanvasSize(v: unknown): v is CanvasSize {
  if (v === null || typeof v !== "object") return false;
  const o = v as { width?: unknown; height?: unknown };
  return (
    typeof o.width === "number" &&
    Number.isInteger(o.width) &&
    o.width >= MIN_CANVAS_DIM &&
    o.width <= MAX_CANVAS_DIM &&
    typeof o.height === "number" &&
    Number.isInteger(o.height) &&
    o.height >= MIN_CANVAS_DIM &&
    o.height <= MAX_CANVAS_DIM
  );
}

/** Reads the last-used canvas size from localStorage, or the default. */
export function loadLastCanvasSize(): CanvasSize {
  return loadStorageJSON<CanvasSize>(LAST_SIZE_KEY, DEFAULT_CANVAS_SIZE, isCanvasSize);
}

/**
 * Parses a raw text-input value into a non-negative integer, or null if
 * the input is empty, fractional, signed, or contains non-digit
 * characters. parseInt is too lenient ("32abc" -> 32), so we whitelist
 * digits-only strings.
 */
export function parseCanvasDim(raw: string): number | null {
  const trimmed = raw.trim();
  if (trimmed === "") return null;
  if (!/^\d+$/.test(trimmed)) return null;
  const n = parseInt(trimmed, 10);
  if (!Number.isInteger(n)) return null;
  return n;
}

/**
 * Validates a parsed dimension against the canvas bounds. Returns an
 * error string for display, or null when the value is acceptable.
 */
export function validateCanvasDim(value: number | null): string | null {
  if (value === null) return "Enter a whole number.";
  if (value < MIN_CANVAS_DIM) return `Must be at least ${MIN_CANVAS_DIM}.`;
  if (value > MAX_CANVAS_DIM) return `Must be at most ${MAX_CANVAS_DIM}.`;
  return null;
}

/** Persists the last-used canvas size so the dialog can prefill next time. */
export function saveLastCanvasSize(size: CanvasSize): void {
  try {
    localStorage.setItem(LAST_SIZE_KEY, JSON.stringify(size));
  } catch {
    // Storage write failures are non-fatal — the dialog still functioned;
    // we just lose the prefill next time. Swallow rather than crash.
  }
}

const [canvasSizeRequest, setCanvasSizeRequest] = createSignal<CanvasSizeRequest | null>(null);

export { canvasSizeRequest };

export function openCanvasSizeDialog(request: CanvasSizeRequest): void {
  setCanvasSizeRequest(request);
}

export function closeCanvasSizeDialog(): void {
  setCanvasSizeRequest(null);
}
