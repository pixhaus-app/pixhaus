import { describe, expect, it } from "vitest";
import { previewBoxes } from "./preview";

describe("previewBoxes", () => {
  it("scales a 1024x1536 canvas into 256x384 at scale 0.25", () => {
    const boxes = previewBoxes(
      1024,
      1536,
      [{ label: "main", rect: { x: 0, y: 0, w: 200, h: 480 } }],
      256,
      384,
    );
    expect(boxes).toHaveLength(1);
    // scale = min(256/1024, 384/1536) = min(0.25, 0.25) = 0.25
    const box = boxes[0]!;
    expect(box.x).toBeCloseTo(0);
    expect(box.y).toBeCloseTo(0);
    expect(box.w).toBeCloseTo(50); // 200 * 0.25
    expect(box.h).toBeCloseTo(120); // 480 * 0.25
    expect(box.label).toBe("main");
  });

  it("preserves aspect ratio when width is the tighter constraint", () => {
    // canvas 400x200, max 100x200 → scale = min(0.25, 1.0) = 0.25
    const boxes = previewBoxes(
      400,
      200,
      [{ label: "a", rect: { x: 10, y: 20, w: 100, h: 50 } }],
      100,
      200,
    );
    const box = boxes[0]!;
    expect(box.x).toBeCloseTo(2.5); // 10 * 0.25
    expect(box.y).toBeCloseTo(5); // 20 * 0.25
    expect(box.w).toBeCloseTo(25); // 100 * 0.25
    expect(box.h).toBeCloseTo(12.5); // 50 * 0.25
  });

  it("maps multiple panels", () => {
    const boxes = previewBoxes(
      100,
      100,
      [
        { label: "p1", rect: { x: 0, y: 0, w: 50, h: 50 } },
        { label: "p2", rect: { x: 50, y: 50, w: 50, h: 50 } },
      ],
      50,
      50,
    );
    expect(boxes).toHaveLength(2);
    const box1 = boxes[1]!;
    expect(box1.x).toBeCloseTo(25);
    expect(box1.y).toBeCloseTo(25);
  });

  it("returns empty array for zero-dimension canvas", () => {
    expect(previewBoxes(0, 100, [], 256, 256)).toEqual([]);
    expect(previewBoxes(100, 0, [], 256, 256)).toEqual([]);
  });
});
