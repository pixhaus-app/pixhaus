import { createStore } from "solid-js/store";

// Composition-library modal open/close. A store for consistency with the rest
// of the UI state layer; read as compositionLibrary.open.
export const [compositionLibrary, setCompositionLibrary] = createStore({ open: false });

export function openCompositionLibrary(): void {
  setCompositionLibrary("open", true);
}

export function closeCompositionLibrary(): void {
  setCompositionLibrary("open", false);
}
