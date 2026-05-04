import { createSignal } from "solid-js";

const [isPreferencesOpen, setPreferencesOpen] = createSignal(false);

export { isPreferencesOpen };

export function openPreferences(): void {
  setPreferencesOpen(true);
}

export function closePreferences(): void {
  setPreferencesOpen(false);
}
