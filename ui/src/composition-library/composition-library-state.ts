import { createSignal } from "solid-js";

const [isCompositionLibraryOpen, setCompositionLibraryOpen] = createSignal(false);

export { isCompositionLibraryOpen };

export function openCompositionLibrary(): void {
  setCompositionLibraryOpen(true);
}

export function closeCompositionLibrary(): void {
  setCompositionLibraryOpen(false);
}
