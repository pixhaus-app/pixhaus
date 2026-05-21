import { describe, expect, it } from "vitest";
import { MAX_ZOOM, MIN_ZOOM } from "../canvas/viewport";
import { zoomToCursor } from "./preview-zoom";

describe("zoomToCursor", () => {
  const identity = { scale: 1, panX: 0, panY: 0 };

  it("keeps the point under the cursor fixed when zooming in", () => {
    const next = zoomToCursor(identity, 100, 50, 2);
    expect(next.scale).toBe(2);
    // Content point at cursor before zoom: (cursor - pan) / scale = (100, 50).
    // After zoom it must still land under the cursor: pan + point * scale === cursor.
    expect(next.panX + 100 * next.scale).toBeCloseTo(100);
    expect(next.panY + 50 * next.scale).toBeCloseTo(50);
  });

  it("keeps the point under the cursor fixed when zooming out", () => {
    const start = { scale: 4, panX: -120, panY: -30 };
    const next = zoomToCursor(start, 200, 80, 0.5);
    expect(next.scale).toBe(2);
    const pointX = (200 - start.panX) / start.scale;
    const pointY = (80 - start.panY) / start.scale;
    expect(next.panX + pointX * next.scale).toBeCloseTo(200);
    expect(next.panY + pointY * next.scale).toBeCloseTo(80);
  });

  it("clamps at MAX_ZOOM", () => {
    const next = zoomToCursor({ scale: MAX_ZOOM, panX: 0, panY: 0 }, 10, 10, 4);
    expect(next.scale).toBe(MAX_ZOOM);
  });

  it("clamps at MIN_ZOOM", () => {
    const next = zoomToCursor({ scale: MIN_ZOOM, panX: 0, panY: 0 }, 10, 10, 0.25);
    expect(next.scale).toBe(MIN_ZOOM);
  });
});
