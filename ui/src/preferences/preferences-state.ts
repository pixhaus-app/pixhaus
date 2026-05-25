import { createStore } from "solid-js/store";

// Preferences-modal open/close. A store for consistency with the rest of the
// UI state layer; read as preferencesModal.open.
export const [preferencesModal, setPreferencesModal] = createStore({ open: false });

export function openPreferences(): void {
  setPreferencesModal("open", true);
}

export function closePreferences(): void {
  setPreferencesModal("open", false);
}
