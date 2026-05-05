// Custom pixelmatch comparison helper.
//
// Use this when you need direct control over the comparison — e.g. to compare
// a cropped region, or to diff two arbitrary PNG buffers rather than relying
// on Playwright's expect(page).toHaveScreenshot().
//
// For most tests, prefer expect(page).toHaveScreenshot() in specs; it uses
// the same underlying pixelmatch algorithm and handles baseline management
// automatically.

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { PNG } from "pngjs";
import pixelmatch from "pixelmatch";

export interface CompareResult {
  /** Fraction of differing pixels in [0.0, 1.0]. */
  diffRatio: number;
  totalPixels: number;
  diffPixels: number;
}

export interface CompareOptions {
  /**
   * Per-channel colour threshold for pixelmatch. 0.0 = exact; 1.0 = anything
   * matches. Default 0.1 absorbs minor anti-aliasing noise.
   */
  threshold?: number;
}

/**
 * Compares actualPng (a PNG-encoded Buffer) against the committed baseline at
 * baselinePath. Writes a diff PNG to diffPath when pixels diverge.
 *
 * First-run behaviour: if no baseline exists at baselinePath, the actual image
 * is written there and the function returns diffRatio 0 so the test passes.
 * Commit the new baseline file to lock the reference.
 *
 * Run `pnpm visual:update` (or `playwright test --update-snapshots`) to
 * regenerate all baselines intentionally.
 */
export function compareScreenshot(
  actualPng: Buffer,
  baselinePath: string,
  diffPath: string,
  options: CompareOptions = {},
): CompareResult {
  const { threshold = 0.1 } = options;
  const actual = PNG.sync.read(actualPng);
  const totalPixels = actual.width * actual.height;

  if (!pathExists(baselinePath)) {
    mkdirSync(dirname(baselinePath), { recursive: true });
    writeFileSync(baselinePath, actualPng);
    return { diffRatio: 0, totalPixels, diffPixels: 0 };
  }

  const baseline = PNG.sync.read(readFileSync(baselinePath));

  if (actual.width !== baseline.width || actual.height !== baseline.height) {
    throw new Error(
      `screenshot size mismatch: actual ${actual.width}x${actual.height} vs baseline ${baseline.width}x${baseline.height}. ` +
        `Delete the baseline at ${baselinePath} and re-run to regenerate.`,
    );
  }

  const diff = new PNG({ width: actual.width, height: actual.height });
  const diffPixels = pixelmatch(
    actual.data,
    baseline.data,
    diff.data,
    actual.width,
    actual.height,
    {
      threshold,
    },
  );

  if (diffPixels > 0) {
    mkdirSync(dirname(diffPath), { recursive: true });
    writeFileSync(diffPath, PNG.sync.write(diff));
  }

  return {
    diffRatio: diffPixels / totalPixels,
    totalPixels,
    diffPixels,
  };
}

function pathExists(p: string): boolean {
  return existsSync(p);
}
