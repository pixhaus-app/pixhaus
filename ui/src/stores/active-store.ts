// The active sprite / frame / layer triple.
//
// This used to live in canvas-state.ts purely so the canvas input handler
// could read activeLayerId without importing the layer panel (which imports
// canvas-state) — a circular-import dodge. Hoisting the triple here lets both
// canvas and layers depend on a leaf module instead of each other.
//
// One Solid store: reads are activeTarget.spriteId, activeTarget.frameIndex,
// activeTarget.layerId; writes go through the named setters. These are the
// "which thing is focused" ids shared across the editor — UI-owned selection
// state; the backend mirrors them through canvas_set_viewport (see canvas).

import { createStore } from "solid-js/store";
import type { LayerId, SpriteId } from "../lib/types";

export interface ActiveTarget {
  spriteId: SpriteId | null;
  frameIndex: number;
  layerId: LayerId | null;
}

export const [activeTarget, setActiveTarget] = createStore<ActiveTarget>({
  spriteId: null,
  frameIndex: 0,
  layerId: null,
});

export const setActiveSpriteId = (v: SpriteId | null): void => setActiveTarget("spriteId", v);
export const setActiveFrameIndex = (v: number): void => setActiveTarget("frameIndex", v);
export const setActiveLayerId = (v: LayerId | null): void => setActiveTarget("layerId", v);

/** Clears the active triple. Called when a project closes or changes identity. */
export function resetActiveTarget(): void {
  setActiveTarget({ spriteId: null, frameIndex: 0, layerId: null });
}
