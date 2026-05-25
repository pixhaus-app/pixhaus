// Module-level state for the Update Available modal.
//
// Lives in the shell layer (not preferences) because the modal appears
// from the Help menu and must work regardless of which panel has focus.

import { createStore } from "solid-js/store";
import type { UpdateInfo } from "../lib/types";

interface UpdateModalState {
  /** True when the Update Available modal should render. */
  showUpdateModal: boolean;
  /** Metadata for the available update, or null when nothing is staged. */
  updateInfo: UpdateInfo | null;
}

export const [updateModal, setUpdateModal] = createStore<UpdateModalState>({
  showUpdateModal: false,
  updateInfo: null,
});

/** Opens the modal and seeds it with the update metadata to display. */
export function openUpdateModal(info: UpdateInfo): void {
  setUpdateModal({ updateInfo: info, showUpdateModal: true });
}

/** Closes the modal. The cached `updateInfo` is left in place so a brief
 * reopen during the same session reuses it; the next `openUpdateModal`
 * call overwrites it anyway. */
export function closeUpdateModal(): void {
  setUpdateModal("showUpdateModal", false);
}
