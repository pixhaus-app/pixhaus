// Common app-lifecycle helpers used by every spec.
//
// Phase 0 keeps this small: dismiss the first-launch crash-reporting
// dialog so subsequent assertions don't trip on its backdrop, and wait
// for the Solid app to mount enough to expose __pixhaus_debug__.

import { $, browser } from "@wdio/globals";
import { byTestId, testid } from "./selectors.js";
import { waitForDebugSurface } from "./state.js";

/**
 * Waits for the debug surface and dismisses the first-launch crash
 * dialog if it's showing. Returns when the welcome screen is interactable.
 *
 * Idempotent: safe to call from multiple `before` hooks; the dialog
 * presence check is fast.
 */
export async function bootApp(): Promise<void> {
  await waitForDebugSurface();

  const dialog = await $(byTestId(testid.firstLaunch.dialog));
  const visible = await dialog.isExisting();
  if (visible) {
    const decline = await $(byTestId(testid.firstLaunch.decline));
    await decline.click();
    await dialog.waitForExist({ reverse: true, timeout: 5000 });
  }

  // Welcome screen mounts as soon as activeProject() === null. Verify it
  // landed before any spec drives the welcome buttons.
  const welcome = await $(byTestId(testid.welcome.root));
  await welcome.waitForExist({ timeout: 10000 });
}
