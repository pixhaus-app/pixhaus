// Bridge: palette index -> RGBA tool color.
//
// The palette panel owns `foregroundIndex` / `backgroundIndex` / `activePalette`.
// The drawing tools (`tool-state.ts`) own `foregroundColor` / `backgroundColor`
// as RGBA values that the brush input handler ships across IPC. Without
// this bridge, clicking a swatch updates the index but the tool color stays
// at its initial black/white — strokes draw black no matter what.

import { createEffect } from "solid-js";
import { activePalette, foregroundIndex, backgroundIndex } from "../../palette/palette-panel-state";
import { setForegroundColor, setBackgroundColor } from "./tool-state";

const BLACK = { r: 0, g: 0, b: 0, a: 255 } as const;
const WHITE = { r: 255, g: 255, b: 255, a: 255 } as const;

// One-way reactive bridge. Must be called from a reactive root (e.g. inside
// a component setup); the effects live for the owning scope's lifetime and
// fire whenever the active palette or the FG/BG index changes.
export function setupPaletteColorSync(): void {
  createEffect(() => {
    const entry = activePalette()?.colors[foregroundIndex()];
    setForegroundColor(entry ? { ...entry.color } : { ...BLACK });
  });
  createEffect(() => {
    const entry = activePalette()?.colors[backgroundIndex()];
    setBackgroundColor(entry ? { ...entry.color } : { ...WHITE });
  });
}
