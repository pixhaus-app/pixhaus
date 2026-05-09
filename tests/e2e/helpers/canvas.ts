// Canvas interaction helpers.
//
// Specs need to click / drag at SPRITE-PIXEL coordinates, but the
// browser action API takes VIEWPORT-PIXEL coordinates. These helpers
// translate via __pixhaus_debug__ (zoom + scroll + canvas size) so
// specs can write tests in the same coordinate system the manual guide
// uses.

import { $, browser } from "@wdio/globals";
import { byTestId, testid } from "./selectors.js";

interface ViewportPoint {
  x: number;
  y: number;
}

/**
 * Returns the (left, top) of the canvas element in WebDriver page
 * coordinates so we can offset clicks within it.
 */
async function getCanvasOrigin(): Promise<ViewportPoint> {
  const el = await $(byTestId(testid.canvas.viewport));
  const rect = await el.getLocation();
  return { x: rect.x, y: rect.y };
}

/**
 * Translates a sprite-pixel coordinate to a viewport-pixel coordinate
 * (relative to the page). Reads the current scroll/zoom from the debug
 * surface so the math stays consistent with whatever the user has set.
 */
async function spriteToViewport(
  spriteX: number,
  spriteY: number,
): Promise<ViewportPoint> {
  const debug = await browser.execute(
    (sx: number, sy: number) => {
      const w = window as unknown as {
        __pixhaus_debug__: {
          getZoom(): number;
          getScroll(): { x: number; y: number };
        };
      };
      const zoom = w.__pixhaus_debug__.getZoom();
      const scroll = w.__pixhaus_debug__.getScroll();
      // The renderer centres the sprite using scrollX/scrollY as the sprite-
      // pixel offset of the viewport origin's centre. Use the canvas element's
      // bounding rect to project sprite -> CSS-pixel.
      const canvas = document.querySelector(
        'canvas[data-testid="canvas-viewport"]',
      );
      const rect = canvas?.getBoundingClientRect();
      if (!rect) return { px: 0, py: 0 };
      const cx = rect.width / 2;
      const cy = rect.height / 2;
      const px = cx + (sx - scroll.x) * zoom;
      const py = cy + (sy - scroll.y) * zoom;
      return { px, py };
    },
    spriteX,
    spriteY,
  );

  const origin = await getCanvasOrigin();
  return {
    x: origin.x + Math.round(debug.px),
    y: origin.y + Math.round(debug.py),
  };
}

/**
 * Clicks at a sprite-pixel coordinate inside the canvas. Resolves once
 * the click is dispatched.
 */
export async function clickCanvasAt(
  spriteX: number,
  spriteY: number,
): Promise<void> {
  const pt = await spriteToViewport(spriteX, spriteY);
  await browser
    .action("pointer", { parameters: { pointerType: "mouse" } })
    .move({ x: pt.x, y: pt.y })
    .down({ button: 0 })
    .up({ button: 0 })
    .perform();
}

/**
 * Drags from one sprite-pixel coordinate to another. Generates
 * intermediate points so the canvas input handler sees a continuous
 * stroke (real-time pencil tests depend on the move events between
 * down and up).
 */
export async function dragCanvas(
  fromSpriteX: number,
  fromSpriteY: number,
  toSpriteX: number,
  toSpriteY: number,
  steps = 8,
): Promise<void> {
  const start = await spriteToViewport(fromSpriteX, fromSpriteY);
  const end = await spriteToViewport(toSpriteX, toSpriteY);

  const action = browser
    .action("pointer", { parameters: { pointerType: "mouse" } })
    .move({ x: start.x, y: start.y })
    .down({ button: 0 });

  for (let i = 1; i <= steps; i++) {
    const t = i / steps;
    const x = Math.round(start.x + (end.x - start.x) * t);
    const y = Math.round(start.y + (end.y - start.y) * t);
    action.move({ x, y, duration: 16 });
  }

  await action.up({ button: 0 }).perform();
}
