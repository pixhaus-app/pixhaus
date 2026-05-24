// The active sprite / frame / layer triple.
//
// This used to live in canvas-state.ts purely so the canvas input handler
// could read activeLayerId without importing the layer panel (which imports
// canvas-state) — a circular-import dodge. Hoisting the triple here lets both
// canvas and layers depend on a leaf module instead of each other.
//
// These are the "which thing is focused" ids shared across the editor. They
// are UI-owned selection state; the backend mirrors them through
// canvas_set_viewport (see canvas-store).

import { createSignal } from "solid-js";
import type { LayerId, SpriteId } from "../lib/types";

export const [activeSpriteId, setActiveSpriteId] = createSignal<SpriteId | null>(null);
export const [activeFrameIndex, setActiveFrameIndex] = createSignal<number>(0);
export const [activeLayerId, setActiveLayerId] = createSignal<LayerId | null>(null);

/** Clears the active triple. Called when a project closes or changes identity. */
export function resetActiveTarget(): void {
  setActiveSpriteId(null);
  setActiveFrameIndex(0);
  setActiveLayerId(null);
}
