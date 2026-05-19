// Pure grid math for row-major frame extraction from a packed sprite
// sheet.
//
// Adapted from FalSprite (https://github.com/lovisdotio/falsprite)
// `public/app.js:469-527`. MIT-licensed, Copyright (c) 2026 lovisdotio.
// See `LICENSES/falsprite-MIT.txt` for the full upstream license text
// and `LICENSES/NOTICE.txt` for the project-wide attribution registry.
//
// The cell-size invariants — floor-divide for cell dimensions, row-major
// frame ordering, imageSmoothingEnabled = false at the canvas — are
// non-negotiable for pixel-art-correct playback. Pixhaus uses this
// module from the timeline preview overlay and from the new
// animated-sprite-sheet verb's preview panel.

/** Coordinate of one cell in a square grid. */
export type GridCoord = {
  /** Column index, `0..gridSize`. */
  col: number;
  /** Row index, `0..gridSize`. */
  row: number;
};

/**
 * Returns the row/column for `frameId` in a square grid of side
 * `gridSize`. Row-major order — cells read left-to-right then
 * top-to-bottom.
 *
 * Negative or zero grid sizes are clamped to `1` so the result remains
 * defined; callers are expected to validate the grid size before
 * playback.
 */
export function frameCoord(frameId: number, gridSize: number): GridCoord {
  const g = Math.max(1, gridSize | 0);
  const id = Math.max(0, frameId | 0);
  return { col: id % g, row: Math.floor(id / g) };
}

/**
 * Cell size for a sheet of dimensions `width × height` packed in a
 * square grid of side `gridSize`. Floor-divides — non-divisible
 * dimensions drop the remainder so every cell has the same size.
 */
export function cellSize(
  width: number,
  height: number,
  gridSize: number,
): { width: number; height: number } {
  const g = Math.max(1, gridSize | 0);
  return {
    width: Math.floor(width / g),
    height: Math.floor(height / g),
  };
}

/** Frame count for a square grid of side `gridSize`. */
export function gridFrameCount(gridSize: number): number {
  const g = Math.max(1, gridSize | 0);
  return g * g;
}

/**
 * Draws cell `frameId` from a packed sheet onto `ctx` at the supplied
 * destination rectangle. Sets `imageSmoothingEnabled = false` so pixel
 * art upscales without blurring.
 *
 * Returns `false` when the frame is out of range (the caller didn't
 * validate against [`gridFrameCount`]); returns `true` after a
 * successful draw.
 */
export function drawCell(
  ctx: CanvasRenderingContext2D,
  image: CanvasImageSource & { width: number; height: number },
  frameId: number,
  gridSize: number,
  dest: { x: number; y: number; width: number; height: number },
): boolean {
  if (frameId < 0 || frameId >= gridFrameCount(gridSize)) return false;
  const { col, row } = frameCoord(frameId, gridSize);
  const { width: cellW, height: cellH } = cellSize(image.width, image.height, gridSize);
  if (cellW <= 0 || cellH <= 0) return false;
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(
    image,
    col * cellW,
    row * cellH,
    cellW,
    cellH,
    dest.x,
    dest.y,
    dest.width,
    dest.height,
  );
  return true;
}
