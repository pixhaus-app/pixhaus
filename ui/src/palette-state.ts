import { createStore } from "solid-js/store";

// Command-palette open/close. A store for consistency with the rest of the UI
// state layer; read as commandPalette.open.
export const [commandPalette, setCommandPalette] = createStore({ open: false });

export function openCommandPalette(): void {
  setCommandPalette("open", true);
}

export function closeCommandPalette(): void {
  setCommandPalette("open", false);
}
