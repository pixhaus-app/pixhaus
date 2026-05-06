import { describe, expect, it } from "vitest";
import { ellipsePerimeterPoints, rectPerimeterPoints } from "./shape-points";

function setOf(points: Array<[number, number]>): Set<string> {
  return new Set(points.map(([x, y]) => `${x},${y}`));
}

describe("rectPerimeterPoints", () => {
  it("emits a single pixel for a degenerate box", () => {
    expect(rectPerimeterPoints(3, 4, 3, 4)).toEqual([[3, 4]]);
  });

  it("emits a horizontal line when y0 === y1", () => {
    expect(rectPerimeterPoints(0, 5, 3, 5)).toEqual([
      [0, 5],
      [1, 5],
      [2, 5],
      [3, 5],
    ]);
  });

  it("emits the full border for a 4x3 rect with no duplicates", () => {
    const pts = rectPerimeterPoints(0, 0, 3, 2);
    const set = setOf(pts);
    // Top edge.
    expect(set.has("0,0")).toBe(true);
    expect(set.has("3,0")).toBe(true);
    // Right edge.
    expect(set.has("3,1")).toBe(true);
    // Bottom edge.
    expect(set.has("0,2")).toBe(true);
    expect(set.has("3,2")).toBe(true);
    // Left edge.
    expect(set.has("0,1")).toBe(true);
    // Interior must NOT be painted.
    expect(set.has("1,1")).toBe(false);
    expect(set.has("2,1")).toBe(false);
    // Total perimeter for a 4x3 rect = 2*(w-1) + 2*(h-1) = 6 + 4 = 10.
    expect(pts.length).toBe(10);
    // No duplicates.
    expect(set.size).toBe(pts.length);
  });

  it("normalises reversed corners", () => {
    const a = rectPerimeterPoints(5, 5, 1, 1);
    const b = rectPerimeterPoints(1, 1, 5, 5);
    expect(setOf(a)).toEqual(setOf(b));
  });
});

describe("ellipsePerimeterPoints", () => {
  it("emits the centre pixel for a degenerate box", () => {
    expect(ellipsePerimeterPoints(4, 4, 4, 4)).toEqual([[4, 4]]);
  });

  it("plots cardinal points of an inscribed circle", () => {
    // 11x11 box centred at (5,5) → rx = ry = 5.
    const pts = ellipsePerimeterPoints(0, 0, 10, 10);
    const set = setOf(pts);
    // Cardinal points must be present.
    expect(set.has("5,0")).toBe(true);
    expect(set.has("5,10")).toBe(true);
    expect(set.has("0,5")).toBe(true);
    expect(set.has("10,5")).toBe(true);
    // Centre must NOT be on the perimeter of a non-degenerate ellipse.
    expect(set.has("5,5")).toBe(false);
  });

  it("contains no duplicates", () => {
    const pts = ellipsePerimeterPoints(0, 0, 16, 8);
    expect(setOf(pts).size).toBe(pts.length);
  });

  it("normalises reversed corners", () => {
    const a = ellipsePerimeterPoints(10, 10, 0, 0);
    const b = ellipsePerimeterPoints(0, 0, 10, 10);
    expect(setOf(a)).toEqual(setOf(b));
  });
});
