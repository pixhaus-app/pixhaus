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

interface BootOptions {
  /**
   * Clear localStorage before navigating, so persisted state from prior
   * tests (recent projects list, theme, crash-reporting opt-in) doesn't
   * affect this spec. Default false because the navigate-and-reload
   * path is enough for the JS-side signals; only enable when a spec
   * needs a "fresh profile" baseline (e.g. T-launch tests, Recent
   * Projects assertions).
   */
  clearStorage?: boolean;
}

/**
 * Waits for the debug surface and dismisses the first-launch crash
 * dialog if it's showing. Returns when the welcome screen is interactable.
 *
 * Idempotent: safe to call from multiple `before` hooks; the dialog
 * presence check is fast.
 */
export async function bootApp(opts: BootOptions = {}): Promise<void> {
  // Always navigate, even if we're already on APP_URL: this triggers a
  // full page reload which resets every JS-side signal (activeProject,
  // panel visibility, command palette state). Without the reload, a
  // previous test that opened a project leaves activeProject() non-null
  // and the welcome screen never re-mounts. Page reload is cheap (~50ms)
  // and the only reliable cross-spec reset.
  await browser.url(APP_URL);

  if (opts.clearStorage === true) {
    // Wipe persisted state (recent projects, crash-reporting opt-in,
    // theme, keybind preset) and reload again so the app reads the
    // empty state from module load.
    await browser.execute(() => {
      try {
        localStorage.clear();
      } catch {
        // localStorage may be unavailable in some test environments;
        // best-effort. The reload below is what matters.
      }
    });
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
