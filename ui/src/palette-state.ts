import { createSignal } from "solid-js";

const [isCommandPaletteOpen, setCommandPaletteOpen] = createSignal(false);

export { isCommandPaletteOpen };

export function openCommandPalette(): void {
  setCommandPaletteOpen(true);
}

export function closeCommandPalette(): void {
  setCommandPaletteOpen(false);
}
