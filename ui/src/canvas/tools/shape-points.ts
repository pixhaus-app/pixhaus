// Point sequences for the rectangle and ellipse tools.
//
// The brush engine renders strokes as a list of canvas-pixel coordinates;
// shapes share that pipeline by emitting one point per pixel along the
// shape's perimeter. Two clicks (anchor + terminus) define an axis-aligned
// bounding box; these helpers normalise the corner order so callers can
// pass them in any sequence.
//
// Coordinates are integer canvas pixels. The brush engine snaps floats
// to the nearest pixel anyway, but emitting integers up front keeps the
// payload predictable and lets the unit tests assert exact membership.
//
// Algorithm choices:
//   - Rect: walk the four edges in order; the corners are emitted by the
//     adjacent edges so we drop the duplicates explicitly.
//   - Ellipse: midpoint algorithm with four-way symmetry, mirroring the
//     core `pixhaus_core::canvas::tools::shapes::draw_ellipse` output so
//     the rendered pixels match what the Rust helper would produce if it
//     was called directly.

/** Normalises two corners into `(x0, y0)` top-left, `(x1, y1)` bottom-right. */
function normaliseBox(
  x0: number,
  y0: number,
  x1: number,
  y1: number,
): { x0: number; y0: number; x1: number; y1: number } {
  const minX = Math.min(x0, x1);
  const maxX = Math.max(x0, x1);
  const minY = Math.min(y0, y1);
  const maxY = Math.max(y0, y1);
  return { x0: minX, y0: minY, x1: maxX, y1: maxY };
}

/**
 * Emits the pixels along a Bresenham line from `(ax, ay)` to `(bx, by)`.
 *
 * The line tool dispatches the result through the same point-list path
 * the brush uses, so the engine doesn't need a separate "draw line"
 * code path. Pixels are integer canvas coordinates; degenerate input
 * (start == end) returns a single pixel.
 */
export function linePoints(
  ax: number,
  ay: number,
  bx: number,
  by: number,
): Array<[number, number]> {
  let x0 = Math.round(ax);
  let y0 = Math.round(ay);
  const x1 = Math.round(bx);
  const y1 = Math.round(by);

  const dx = Math.abs(x1 - x0);
  const dy = -Math.abs(y1 - y0);
  const sx = x0 < x1 ? 1 : -1;
  const sy = y0 < y1 ? 1 : -1;
  let err = dx + dy;

  const out: Array<[number, number]> = [];
  // Bound the loop so a pathological input can't run away.
  const cap = (dx - dy) * 2 + 1;
  for (let i = 0; i < cap; i += 1) {
    out.push([x0, y0]);
    if (x0 === x1 && y0 === y1) break;
    const e2 = 2 * err;
    if (e2 >= dy) {
      err += dy;
      x0 += sx;
    }
    if (e2 <= dx) {
      err += dx;
      y0 += sy;
    }
  }
  return out;
}

/**
 * Emits the perimeter pixels of an axis-aligned rectangle.
 *
 * The corners are passed once; the edges are walked top → right → bottom → left
 * with the start of each edge skipped (it was emitted by the previous edge).
 * For a 1x1 box this collapses to a single pixel.
 */
export function rectPerimeterPoints(
  ax: number,
  ay: number,
  bx: number,
  by: number,
): Array<[number, number]> {
  const x0 = Math.round(ax);
  const y0 = Math.round(ay);
  const x1 = Math.round(bx);
  const y1 = Math.round(by);
  const box = normaliseBox(x0, y0, x1, y1);

  // Degenerate cases — single pixel or single line.
  if (box.x0 === box.x1 && box.y0 === box.y1) {
    return [[box.x0, box.y0]];
  }
  if (box.x0 === box.x1) {
    const out: Array<[number, number]> = [];
    for (let y = box.y0; y <= box.y1; y += 1) out.push([box.x0, y]);
    return out;
  }
  if (box.y0 === box.y1) {
    const out: Array<[number, number]> = [];
    for (let x = box.x0; x <= box.x1; x += 1) out.push([x, box.y0]);
    return out;
  }

  const out: Array<[number, number]> = [];
  // Top edge, including both corners.
  for (let x = box.x0; x <= box.x1; x += 1) out.push([x, box.y0]);
  // Right edge, skipping the top-right corner.
  for (let y = box.y0 + 1; y <= box.y1; y += 1) out.push([box.x1, y]);
  // Bottom edge, skipping the bottom-right corner, walking right-to-left.
  for (let x = box.x1 - 1; x >= box.x0; x -= 1) out.push([x, box.y1]);
  // Left edge, skipping both corners.
  for (let y = box.y1 - 1; y >= box.y0 + 1; y -= 1) out.push([box.x0, y]);
  return out;
}

/**
 * Emits the perimeter pixels of an ellipse inscribed in the bounding box.
 *
 * Mirrors the four-way-symmetric midpoint algorithm in
 * `core/src/canvas/tools/shapes.rs::midpoint_ellipse_outline` so the
 * rasterisation is consistent with what the core helper produces.
 *
 * Pixels are deduplicated (the symmetric plotter visits the four cardinal
 * points twice when `x` or `y` is zero).
 */
export function ellipsePerimeterPoints(
  ax: number,
  ay: number,
  bx: number,
  by: number,
): Array<[number, number]> {
  const x0 = Math.round(ax);
  const y0 = Math.round(ay);
  const x1 = Math.round(bx);
  const y1 = Math.round(by);
  const box = normaliseBox(x0, y0, x1, y1);

  const cx = Math.trunc((box.x0 + box.x1) / 2);
  const cy = Math.trunc((box.y0 + box.y1) / 2);
  const rx = Math.trunc((box.x1 - box.x0) / 2);
  const ry = Math.trunc((box.y1 - box.y0) / 2);

  if (rx <= 0 || ry <= 0) {
    return [[cx, cy]];
  }

  const seen = new Set<string>();
  const points: Array<[number, number]> = [];
  const plot = (x: number, y: number): void => {
    for (const [px, py] of [
      [cx + x, cy + y],
      [cx - x, cy + y],
      [cx + x, cy - y],
      [cx - x, cy - y],
    ] as Array<[number, number]>) {
      const key = `${px},${py}`;
      if (seen.has(key)) continue;
      seen.add(key);
      points.push([px, py]);
    }
  };

  // Region 1 — slope > -1.
  let x = 0;
  let y = ry;
  const rx2 = rx * rx;
  const ry2 = ry * ry;
  let d1 = ry2 - rx2 * ry + Math.trunc(rx2 / 4);
  let dxTerm = 2 * ry2 * x;
  let dyTerm = 2 * rx2 * y;

  while (dxTerm < dyTerm) {
    plot(x, y);
    x += 1;
    dxTerm += 2 * ry2;
    if (d1 < 0) {
      d1 += dxTerm + ry2;
    } else {
      y -= 1;
      dyTerm -= 2 * rx2;
      d1 += dxTerm - dyTerm + ry2;
    }
  }

  // Region 2 — slope <= -1.
  let d2 = ry2 * (x * x + x) + rx2 * (y - 1) * (y - 1) - rx2 * ry2;
  while (y >= 0) {
    plot(x, y);
    y -= 1;
    dyTerm -= 2 * rx2;
    if (d2 > 0) {
      d2 += rx2 - dyTerm;
    } else {
      x += 1;
      dxTerm += 2 * ry2;
      d2 += dxTerm - dyTerm + rx2;
    }
  }

  return points;
}
