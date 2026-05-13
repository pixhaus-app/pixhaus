// Module-level state for the Update Available modal.
//
// Lives in the shell layer (not preferences) because the modal appears
// from the Help menu and must work regardless of which panel has focus.
// Mirrors the open/close + payload pattern used by preferences-state.

import { createSignal } from "solid-js";
import type { UpdateInfo } from "../lib/types";

const [showUpdateModalSignal, setShowUpdateModal] = createSignal(false);
const [updateInfoSignal, setUpdateInfo] = createSignal<UpdateInfo | null>(null);

/** True when the Update Available modal should render. */
export const showUpdateModal = showUpdateModalSignal;

/** Metadata for the available update, or `null` when nothing is staged. */
export const updateInfo = updateInfoSignal;

/** Opens the modal and seeds it with the update metadata to display. */
export function openUpdateModal(info: UpdateInfo): void {
  setUpdateInfo(info);
  setShowUpdateModal(true);
}

/** Closes the modal. The cached `updateInfo` is left in place so a brief
 * reopen during the same session reuses it; the next `openUpdateModal`
 * call overwrites it anyway. */
export function closeUpdateModal(): void {
  setShowUpdateModal(false);
}
