import { describe, expect, it, vi } from "vitest";

import { cellSize, drawCell, frameCoord, gridFrameCount } from "./frame-grid";

describe("frameCoord — row-major order", () => {
  it("reads left-to-right then top-to-bottom for a 4x4 grid", () => {
    expect(frameCoord(0, 4)).toEqual({ col: 0, row: 0 });
    expect(frameCoord(1, 4)).toEqual({ col: 1, row: 0 });
    expect(frameCoord(3, 4)).toEqual({ col: 3, row: 0 });
    expect(frameCoord(4, 4)).toEqual({ col: 0, row: 1 });
    expect(frameCoord(15, 4)).toEqual({ col: 3, row: 3 });
  });

  it("clamps grid size 0 to 1", () => {
    expect(frameCoord(0, 0)).toEqual({ col: 0, row: 0 });
  });

  it("floors fractional inputs (defensive against double inputs)", () => {
    expect(frameCoord(1.7, 3)).toEqual({ col: 1, row: 0 });
  });

  it("treats negative frame ids as zero", () => {
    expect(frameCoord(-3, 4)).toEqual({ col: 0, row: 0 });
  });
});

describe("cellSize — floor-divide invariants", () => {
  it("floor-divides non-divisible widths", () => {
    expect(cellSize(17, 17, 4)).toEqual({ width: 4, height: 4 });
    expect(cellSize(100, 100, 6)).toEqual({ width: 16, height: 16 });
  });

  it("returns the exact dimensions on divisible inputs", () => {
    expect(cellSize(128, 128, 4)).toEqual({ width: 32, height: 32 });
  });

  it("clamps zero grid", () => {
    expect(cellSize(64, 64, 0)).toEqual({ width: 64, height: 64 });
  });
});

describe("gridFrameCount", () => {
  it("returns g * g", () => {
    expect(gridFrameCount(2)).toBe(4);
    expect(gridFrameCount(3)).toBe(9);
    expect(gridFrameCount(6)).toBe(36);
  });

  it("clamps below 1", () => {
    expect(gridFrameCount(0)).toBe(1);
    expect(gridFrameCount(-3)).toBe(1);
  });
});

describe("drawCell", () => {
  function makeCtx() {
    return {
      imageSmoothingEnabled: true,
      drawImage: vi.fn(),
    } as unknown as CanvasRenderingContext2D;
  }

  function makeImage() {
    return {
      width: 64,
      height: 64,
      // jsdom doesn't provide CanvasImageSource — we only care about the
      // call shape, not the actual draw.
    } as unknown as CanvasImageSource & { width: number; height: number };
  }

  it("returns false for out-of-range frame ids", () => {
    const ctx = makeCtx();
    expect(drawCell(ctx, makeImage(), -1, 4, { x: 0, y: 0, width: 16, height: 16 })).toBe(false);
    expect(drawCell(ctx, makeImage(), 16, 4, { x: 0, y: 0, width: 16, height: 16 })).toBe(false);
    expect((ctx.drawImage as ReturnType<typeof vi.fn>).mock.calls.length).toBe(0);
  });

  it("draws cell 0 from the top-left source quadrant", () => {
    const ctx = makeCtx();
    const img = makeImage();
    const ok = drawCell(ctx, img, 0, 4, { x: 10, y: 20, width: 32, height: 32 });
    expect(ok).toBe(true);
    expect(ctx.imageSmoothingEnabled).toBe(false);
    expect(ctx.drawImage).toHaveBeenCalledWith(img, 0, 0, 16, 16, 10, 20, 32, 32);
  });

  it("draws cell 5 (row 1, col 1 in a 4×4 grid) from src (16,16)", () => {
    const ctx = makeCtx();
    const img = makeImage();
    drawCell(ctx, img, 5, 4, { x: 0, y: 0, width: 16, height: 16 });
    expect(ctx.drawImage).toHaveBeenCalledWith(img, 16, 16, 16, 16, 0, 0, 16, 16);
  });

  it("disables image smoothing on every call", () => {
    const ctx = makeCtx();
    ctx.imageSmoothingEnabled = true;
    drawCell(ctx, makeImage(), 0, 2, { x: 0, y: 0, width: 8, height: 8 });
    expect(ctx.imageSmoothingEnabled).toBe(false);
  });
});
