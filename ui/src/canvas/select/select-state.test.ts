import { describe, it, expect, beforeEach } from "vitest";
import {
  setSelectTool,
  selectTool,
  setMarqueeDrag,
  marqueeDrag,
  dragToBounds,
  dragIsNonEmpty,
  snapToPixel,
  resetSelectState,
  setLassoPoints,
  lassoPoints,
} from "./select-state";

describe("dragToBounds", () => {
  it("normalises a top-left → bottom-right drag", () => {
    const b = dragToBounds({ startX: 10, startY: 20, currentX: 50, currentY: 80 });
    expect(b).toEqual({ x: 10, y: 20, width: 40, height: 60 });
  });

  it("normalises a bottom-right → top-left drag", () => {
    const b = dragToBounds({ startX: 50, startY: 80, currentX: 10, currentY: 20 });
    expect(b).toEqual({ x: 10, y: 20, width: 40, height: 60 });
  });

  it("normalises a mixed-direction drag", () => {
    const b = dragToBounds({ startX: 30, startY: 10, currentX: 10, currentY: 50 });
    expect(b).toEqual({ x: 10, y: 10, width: 20, height: 40 });
  });

  it("returns zero size when start === current", () => {
    const b = dragToBounds({ startX: 5, startY: 5, currentX: 5, currentY: 5 });
    expect(b.width).toBe(0);
    expect(b.height).toBe(0);
  });
});

describe("dragIsNonEmpty", () => {
  it("returns true when extent >= 1 in both axes", () => {
    expect(dragIsNonEmpty({ startX: 0, startY: 0, currentX: 1, currentY: 1 })).toBe(true);
  });

  it("returns false when x extent < 1", () => {
    expect(dragIsNonEmpty({ startX: 0, startY: 0, currentX: 0, currentY: 5 })).toBe(false);
  });

  it("returns false when y extent < 1", () => {
    expect(dragIsNonEmpty({ startX: 0, startY: 0, currentX: 5, currentY: 0 })).toBe(false);
  });
});

describe("snapToPixel", () => {
  it("floors positive fractional values", () => {
    expect(snapToPixel(3.7)).toBe(3);
    expect(snapToPixel(3.0)).toBe(3);
  });

  it("floors negative values consistently with Math.floor", () => {
    expect(snapToPixel(-1.2)).toBe(-2);
  });
});

describe("selectTool signal", () => {
  it("defaults to 'rect'", () => {
    expect(selectTool()).toBe("rect");
  });

  it("updates when set", () => {
    setSelectTool("wand");
    expect(selectTool()).toBe("wand");
    setSelectTool("rect"); // restore
  });
});

describe("resetSelectState", () => {
  beforeEach(() => {
    setMarqueeDrag({ startX: 0, startY: 0, currentX: 10, currentY: 10 });
    setLassoPoints([
      [1, 2],
      [3, 4],
    ]);
  });

  it("clears marquee drag", () => {
    resetSelectState();
    expect(marqueeDrag()).toBeNull();
  });

  it("clears lasso points", () => {
    resetSelectState();
    expect(lassoPoints()).toEqual([]);
  });
});
