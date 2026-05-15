// Unit tests for the canvas-size dialog's pure helpers and signal
// surface. The component itself is mostly UI glue; the logic worth
// testing is parse/validate, persistence, and open/close semantics.

import { beforeEach, describe, expect, it, vi } from "vitest";

// Polyfill localStorage before module imports settle, mirroring the
// pattern used by command-registry.test.ts (vitest's node env doesn't
// ship one).
vi.hoisted(() => {
  const g = globalThis as unknown as { localStorage?: Storage };
  const store = new Map<string, string>();
  g.localStorage = {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => {
      store.set(k, v);
    },
    removeItem: (k: string) => {
      store.delete(k);
    },
    clear: () => store.clear(),
    key: (i: number) => Array.from(store.keys())[i] ?? null,
    get length() {
      return store.size;
    },
  };
});

import {
  DEFAULT_CANVAS_SIZE,
  MAX_CANVAS_DIM,
  MIN_CANVAS_DIM,
  canvasSizeRequest,
  closeCanvasSizeDialog,
  loadLastCanvasSize,
  openCanvasSizeDialog,
  parseCanvasDim,
  saveLastCanvasSize,
  validateCanvasDim,
} from "./canvas-size-dialog-state";

beforeEach(() => {
  localStorage.clear();
  closeCanvasSizeDialog();
});

describe("parseCanvasDim", () => {
  it("returns integers for digit-only input", () => {
    expect(parseCanvasDim("32")).toBe(32);
    expect(parseCanvasDim("  128  ")).toBe(128);
    expect(parseCanvasDim("8192")).toBe(8192);
  });

  it("rejects empty, fractional, signed, or non-numeric input", () => {
    expect(parseCanvasDim("")).toBeNull();
    expect(parseCanvasDim("   ")).toBeNull();
    expect(parseCanvasDim("32.5")).toBeNull();
    expect(parseCanvasDim("-32")).toBeNull();
    expect(parseCanvasDim("32abc")).toBeNull();
    expect(parseCanvasDim("abc")).toBeNull();
  });
});

describe("validateCanvasDim", () => {
  it("accepts values inside the inclusive bounds", () => {
    expect(validateCanvasDim(MIN_CANVAS_DIM)).toBeNull();
    expect(validateCanvasDim(32)).toBeNull();
    expect(validateCanvasDim(MAX_CANVAS_DIM)).toBeNull();
  });

  it("rejects null, below-min, and above-max", () => {
    expect(validateCanvasDim(null)).toMatch(/whole number/);
    expect(validateCanvasDim(0)).toMatch(/at least/);
    expect(validateCanvasDim(MAX_CANVAS_DIM + 1)).toMatch(/at most/);
  });
});

describe("loadLastCanvasSize / saveLastCanvasSize", () => {
  it("returns the default when nothing is stored", () => {
    expect(loadLastCanvasSize()).toEqual(DEFAULT_CANVAS_SIZE);
  });

  it("round-trips a saved value", () => {
    saveLastCanvasSize({ width: 128, height: 96 });
    expect(loadLastCanvasSize()).toEqual({ width: 128, height: 96 });
  });

  it("falls back to the default when the stored value is malformed", () => {
    localStorage.setItem("pixhaus:last-canvas-size", "not-json");
    expect(loadLastCanvasSize()).toEqual(DEFAULT_CANVAS_SIZE);
  });

  it("falls back to the default when the stored value is out of range", () => {
    localStorage.setItem("pixhaus:last-canvas-size", JSON.stringify({ width: 99999, height: 32 }));
    expect(loadLastCanvasSize()).toEqual(DEFAULT_CANVAS_SIZE);
  });

  it("falls back to the default when the stored value is fractional", () => {
    localStorage.setItem("pixhaus:last-canvas-size", JSON.stringify({ width: 32.5, height: 32 }));
    expect(loadLastCanvasSize()).toEqual(DEFAULT_CANVAS_SIZE);
  });
});

describe("openCanvasSizeDialog / closeCanvasSizeDialog", () => {
  it("starts closed and reflects open state via the signal", () => {
    expect(canvasSizeRequest()).toBeNull();
    const onConfirm = vi.fn();
    openCanvasSizeDialog({ mode: "project", onConfirm });
    expect(canvasSizeRequest()).not.toBeNull();
    expect(canvasSizeRequest()?.mode).toBe("project");
  });

  it("closes back to null", () => {
    openCanvasSizeDialog({ mode: "sprite", onConfirm: () => {} });
    closeCanvasSizeDialog();
    expect(canvasSizeRequest()).toBeNull();
  });

  it("forwards the supplied onConfirm callback", () => {
    const onConfirm = vi.fn();
    openCanvasSizeDialog({ mode: "sprite", onConfirm });
    canvasSizeRequest()?.onConfirm({ width: 64, height: 48 });
    expect(onConfirm).toHaveBeenCalledWith({ width: 64, height: 48 });
  });
});
