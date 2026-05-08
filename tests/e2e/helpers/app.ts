// Common app-lifecycle helpers used by every spec.
//
// Phase 0 keeps this small: navigate to the Tauri app URL, dismiss the
// first-launch crash-reporting dialog so subsequent assertions don't
// trip on its backdrop, and wait for the Solid app to mount enough to
// expose __pixhaus_debug__.

import { $, browser } from "@wdio/globals";
import { byTestId, testid } from "./selectors.js";
import { waitForDebugSurface } from "./state.js";

// Tauri 2 serves embedded assets from this URL on Windows and Linux when
// the binary is built with the custom-protocol feature (default). The
// scheme is intercepted inside the binary's WebView; navigating here
// from the WebDriver session triggers Tauri's asset handler.
const APP_URL = "http://tauri.localhost/";

/**
 * Waits for the debug surface and dismisses the first-launch crash
 * dialog if it's showing. Returns when the welcome screen is interactable.
 *
 * Idempotent: safe to call from multiple `before` hooks; the dialog
 * presence check is fast.
 */
export async function bootApp(): Promise<void> {
  // Tauri auto-loads the app URL on binary startup, but msedgedriver's
  // session-creation handshake resets the WebView to about:blank. Force
  // a navigate so every spec lands on a known-good page.
  const currentUrl = await browser.getUrl().catch(() => "");
  if (!currentUrl.startsWith(APP_URL)) {
    await browser.url(APP_URL);
  }

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
