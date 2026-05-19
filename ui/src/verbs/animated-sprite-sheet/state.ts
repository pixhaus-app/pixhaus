// Solid-side open/close state for the animated-sprite-sheet form.
//
// The form mounts under the shell (see `AnimatedSpriteSheetHost.tsx`)
// and observes this signal. The command-palette / panel entry calls
// `openAnimatedSpriteSheetForm()`.

import { createSignal } from "solid-js";

const [open, setOpenInternal] = createSignal(false);

export function isAnimatedSpriteSheetOpen(): boolean {
  return open();
}

export function openAnimatedSpriteSheetForm(): void {
  setOpenInternal(true);
}

export function closeAnimatedSpriteSheetForm(): void {
  setOpenInternal(false);
}
