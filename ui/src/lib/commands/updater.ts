// Auto-update commands.
//
// Wraps the two updater IPCs registered in app/src/lib.rs. The backend's
// startup check (`check_on_startup`) emits `updater:available` directly;
// these wrappers cover the user-triggered path (Help > Check for updates)
// and the install action.

import { invoke } from "../ipc";
import type { UpdateInfo } from "../types";

/**
 * Asks the backend whether a newer release is available.
 *
 * Resolves to the update metadata when one is found, or `null` when the
 * running build is already the latest. Rejects with `AppCommandError` on
 * network or signature-verification failures.
 */
export function updaterCheck(): Promise<UpdateInfo | null> {
  return invoke<UpdateInfo | null>("updater_check");
}

/**
 * Downloads and installs the latest available update.
 *
 * Emits `updater:progress` events during download and `updater:ready`
 * once the installer is staged. On most platforms the app restarts
 * automatically after this resolves. Rejects with `AppCommandError`
 * (Validation variant) when no update is available or the install fails.
 */
export function updaterInstall(): Promise<void> {
  return invoke<void>("updater_install");
}
