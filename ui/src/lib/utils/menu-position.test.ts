import { describe, expect, it } from "vitest";
import { clampPositionToBounds } from "./menu-position";

describe("clampPositionToBounds", () => {
  const W = 200;
  const H = 100;
  const VW = 1000;
  const VH = 800;

  it("returns the input position unchanged when the menu fits", () => {
    expect(clampPositionToBounds(100, 100, W, H, VW, VH)).toEqual({ x: 100, y: 100 });
  });

  it("shifts the menu left when the right edge would overflow", () => {
    // x=900 + 200 width = 1100 > 1000 viewport. Expect x = 1000 - 200 - 4 = 796.
    expect(clampPositionToBounds(900, 100, W, H, VW, VH)).toEqual({ x: 796, y: 100 });
  });

  it("shifts the menu up when the bottom edge would overflow", () => {
    // y=750 + 100 height = 850 > 800 viewport. Expect y = 800 - 100 - 4 = 696.
    expect(clampPositionToBounds(100, 750, W, H, VW, VH)).toEqual({ x: 100, y: 696 });
  });

  it("clamps both axes when bottom-right corner overflows", () => {
    expect(clampPositionToBounds(950, 780, W, H, VW, VH)).toEqual({ x: 796, y: 696 });
  });

  it("clamps to the margin when the input position is negative", () => {
    expect(clampPositionToBounds(-50, -10, W, H, VW, VH)).toEqual({ x: 4, y: 4 });
  });

  it("falls back to the margin when the menu is wider than the viewport", () => {
    // Menu wider than viewport: maxX would be negative; clamp keeps it at the
    // left margin so the menu starts on-screen even if it extends past.
    expect(clampPositionToBounds(500, 100, 1500, H, VW, VH)).toEqual({ x: 4, y: 100 });
  });
});
