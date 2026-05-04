import { describe, expect, it } from "vitest";
import {
  canvasToScreen,
  screenToCanvas,
  zoomAt,
  snapZoom,
  clampZoom,
  fitZoom,
  scrollToCentre,
  SNAP_ZOOMS,
  MIN_ZOOM,
  MAX_ZOOM,
  PIXEL_GRID_ZOOM_THRESHOLD,
} from "./viewport";

// Helpers
const VP = { w: 800, h: 600 }; // typical viewport

describe("canvasToScreen / screenToCanvas round-trip", () => {
  it("maps origin to centre when scroll == half-sprite", () => {
    const scrollX = 16;
    const scrollY = 16;
    const zoom = 1;
    // Canvas (16, 16) should appear at screen (400, 300) — the centre.
    const [sx, sy] = canvasToScreen(16, 16, scrollX, scrollY, zoom, VP.w, VP.h);
    expect(sx).toBeCloseTo(VP.w / 2);
    expect(sy).toBeCloseTo(VP.h / 2);
  });

  it("round-trips correctly at zoom 1", () => {
    const scrollX = 0;
    const scrollY = 0;
    const zoom = 1;
    const cases: Array<[number, number]> = [
      [0, 0],
      [10, 20],
      [100, 200],
    ];
    for (const [cx, cy] of cases) {
      const [sx, sy] = canvasToScreen(cx, cy, scrollX, scrollY, zoom, VP.w, VP.h);
      const [cx2, cy2] = screenToCanvas(sx, sy, scrollX, scrollY, zoom, VP.w, VP.h);
      expect(cx2).toBeCloseTo(cx);
      expect(cy2).toBeCloseTo(cy);
    }
  });

  it("round-trips at 8× zoom", () => {
    const scrollX = 50;
    const scrollY = 50;
    const zoom = 8;
    const [sx, sy] = canvasToScreen(60, 70, scrollX, scrollY, zoom, VP.w, VP.h);
    const [cx, cy] = screenToCanvas(sx, sy, scrollX, scrollY, zoom, VP.w, VP.h);
    expect(cx).toBeCloseTo(60);
    expect(cy).toBeCloseTo(70);
  });

  it("left edge of sprite is left of centre when centred", () => {
    // scroll is sprite centre → (0, 0) is to the upper-left of screen centre.
    const sw = 64;
    const sh = 64;
    const scrollX = sw / 2;
    const scrollY = sh / 2;
    const [sx] = canvasToScreen(0, 0, scrollX, scrollY, 1, VP.w, VP.h);
    expect(sx).toBeLessThan(VP.w / 2);
  });
});

describe("zoomAt", () => {
  it("keeps the pointed-at canvas position under the cursor after zoom", () => {
    const scrollX = 32;
    const scrollY = 32;
    const oldZoom = 1;
    const cursorSx = 300;
    const cursorSy = 200;

    // Canvas coordinate under cursor before zoom.
    const [cx0, cy0] = screenToCanvas(cursorSx, cursorSy, scrollX, scrollY, oldZoom, VP.w, VP.h);

    const newZoom = 4;
    const { scrollX: nx, scrollY: ny } = zoomAt(
      scrollX,
      scrollY,
      oldZoom,
      VP.w,
      VP.h,
      cursorSx,
      cursorSy,
      newZoom,
    );

    // Canvas coordinate under cursor after zoom should be the same.
    const [cx1, cy1] = screenToCanvas(cursorSx, cursorSy, nx, ny, newZoom, VP.w, VP.h);
    expect(cx1).toBeCloseTo(cx0, 5);
    expect(cy1).toBeCloseTo(cy0, 5);
  });

  it("keeps the centre fixed when zooming at centre", () => {
    const scrollX = 16;
    const scrollY = 16;
    const [centreSx, centreSy] = [VP.w / 2, VP.h / 2];
    const [cx0, cy0] = screenToCanvas(centreSx, centreSy, scrollX, scrollY, 1, VP.w, VP.h);

    const { scrollX: nx, scrollY: ny } = zoomAt(
      scrollX,
      scrollY,
      1,
      VP.w,
      VP.h,
      centreSx,
      centreSy,
      2,
    );
    const [cx1, cy1] = screenToCanvas(centreSx, centreSy, nx, ny, 2, VP.w, VP.h);
    expect(cx1).toBeCloseTo(cx0, 5);
    expect(cy1).toBeCloseTo(cy0, 5);
  });
});

describe("snapZoom", () => {
  it("zooms in to the next snap level", () => {
    expect(snapZoom(1, 1)).toBe(2);
    expect(snapZoom(0.5, 1)).toBe(1);
    expect(snapZoom(0.25, 1)).toBe(0.5);
  });

  it("zooms out to the previous snap level", () => {
    expect(snapZoom(1, -1)).toBe(0.5);
    expect(snapZoom(2, -1)).toBe(1);
  });

  it("clamps to MIN_ZOOM when already at minimum", () => {
    expect(snapZoom(MIN_ZOOM, -1)).toBe(MIN_ZOOM);
  });

  it("clamps to MAX_ZOOM when already at maximum", () => {
    expect(snapZoom(MAX_ZOOM, 1)).toBe(MAX_ZOOM);
  });

  it("covers all snap levels in sequence", () => {
    // Iterate from the minimum level upward, verifying each step lands on the next snap.
    let z: number = MIN_ZOOM;
    for (let i = 1; i < SNAP_ZOOMS.length; i++) {
      z = snapZoom(z, 1);
      expect(z).toBe(SNAP_ZOOMS[i] as number);
    }
  });
});

describe("clampZoom", () => {
  it("returns value as-is within range", () => {
    expect(clampZoom(1)).toBe(1);
    expect(clampZoom(4)).toBe(4);
  });

  it("clamps below min", () => {
    expect(clampZoom(0)).toBe(MIN_ZOOM);
    expect(clampZoom(-1)).toBe(MIN_ZOOM);
  });

  it("clamps above max", () => {
    expect(clampZoom(1000)).toBe(MAX_ZOOM);
  });
});

describe("fitZoom", () => {
  it("returns 1 for a sprite that fits at 1×", () => {
    // 400×400 sprite, 800×600 viewport, 16px padding → max content area 768×568
    const z = fitZoom(400, 400, 800, 600);
    expect(z).toBeGreaterThan(0);
    expect(z).toBeLessThanOrEqual(MAX_ZOOM);
  });

  it("returns a zoom ≤ 1 for a large sprite", () => {
    const z = fitZoom(4096, 4096, 800, 600);
    expect(z).toBeLessThan(1);
    expect(z).toBeGreaterThanOrEqual(MIN_ZOOM);
  });

  it("returns 1 for zero-size sprite", () => {
    expect(fitZoom(0, 0, 800, 600)).toBe(1);
  });
});

describe("scrollToCentre", () => {
  it("returns half sprite dimensions", () => {
    const { scrollX, scrollY } = scrollToCentre(64, 32);
    expect(scrollX).toBe(32);
    expect(scrollY).toBe(16);
  });
});

describe("PIXEL_GRID_ZOOM_THRESHOLD", () => {
  it("is 4 (400 %)", () => {
    expect(PIXEL_GRID_ZOOM_THRESHOLD).toBe(4);
  });
});
