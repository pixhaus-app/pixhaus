// Launch specs — first-launch dialog + welcome screen.
//
// Maps to docs/manual-test-guide.md section 1 ("Setup & cold start"):
//   - T-launch-001: First-launch crash-reporting dialog appears
//   - T-launch-002: Decline crash reporting
//   - T-launch-003: Welcome screen renders with all sections
//
// T-launch-001 and T-launch-002 need first-launch state, so they bypass
// bootApp() (which auto-dismisses the dialog) and instead clear
// localStorage + reload to force the dialog back. T-launch-003 uses the
// normal returning-user state via bootApp().

import { $, browser, expect } from "@wdio/globals";
import { bootApp } from "../helpers/app.js";
import { byTestId, testid } from "../helpers/selectors.js";
import {
  getActiveProject,
  getCrashReportingDialogShown,
  getCrashReportingEnabled,
} from "../helpers/state.js";

const APP_URL = "http://tauri.localhost/";

/**
 * Drops the app back into first-launch state by clearing localStorage,
 * reloading the WebView, and waiting for __pixhaus_debug__ to mount.
 * Used by T-launch-001 and T-launch-002 — every other spec uses bootApp().
 */
async function reloadAsFirstLaunch(): Promise<void> {
  // Make sure we're on the app URL before touching localStorage; the
  // WebView occasionally lands on about:blank between sessions.
  const currentUrl = await browser.getUrl().catch(() => "");
  if (!currentUrl.startsWith(APP_URL)) {
    await browser.url(APP_URL);
  }
  await browser.execute(() => localStorage.clear());
  await browser.url(APP_URL);
  await browser.waitUntil(
    async () => {
      const present = await browser.execute(() => {
        return (
          typeof (window as unknown as { __pixhaus_debug__?: unknown })
            .__pixhaus_debug__ !== "undefined"
        );
      });
      return present === true;
    },
    {
      timeout: 10000,
      timeoutMsg: "window.__pixhaus_debug__ never appeared after reload",
    },
  );
}

describe("Launch (manual-test-guide §1)", () => {
  it("T-launch-001: First-launch crash-reporting dialog appears", async () => {
    await reloadAsFirstLaunch();

    const dialog = await $(byTestId(testid.firstLaunch.dialog));
    await expect(dialog).toBeDisplayed();

    const accept = await $(byTestId(testid.firstLaunch.accept));
    const decline = await $(byTestId(testid.firstLaunch.decline));
    await expect(accept).toBeDisplayed();
    await expect(decline).toBeDisplayed();

    // The dialog is the active modal — flag has not been flipped yet.
    await expect(await getCrashReportingDialogShown()).toBe(false);
  });

  it("T-launch-002: Decline crash reporting", async () => {
    await reloadAsFirstLaunch();

    const dialog = await $(byTestId(testid.firstLaunch.dialog));
    await expect(dialog).toBeDisplayed();

    const decline = await $(byTestId(testid.firstLaunch.decline));
    await decline.click();

    // Dialog unmounts.
    await dialog.waitForExist({ reverse: true, timeout: 5000 });

    // Decline path flips the seen-flag true and crash reporting false,
    // so the dialog won't reappear next launch.
    await expect(await getCrashReportingDialogShown()).toBe(true);
    await expect(await getCrashReportingEnabled()).toBe(false);
  });

  it("T-launch-003: Welcome screen renders with all sections", async () => {
    await bootApp();

    // No active project — editor shell stays unmounted, welcome stays up.
    await expect(await getActiveProject()).toBeNull();

    const welcome = await $(byTestId(testid.welcome.root));
    await expect(welcome).toBeDisplayed();

    const newProject = await $(byTestId(testid.welcome.newProject));
    const openProject = await $(byTestId(testid.welcome.openProject));
    await expect(newProject).toBeDisplayed();
    await expect(openProject).toBeDisplayed();

    const samples = await $(byTestId(testid.welcome.samples));
    await expect(samples).toBeDisplayed();

    // Recent section is conditional — empty profile means it's absent.
    // Either present-but-empty or fully unmounted is acceptable per
    // the manual-test-guide T-launch-003 expectation.
    const recent = await $(byTestId(testid.welcome.recent));
    const recentExists = await recent.isExisting();
    if (recentExists) {
      const firstRecent = await $(byTestId(testid.welcome.recentItem(0)));
      await expect(await firstRecent.isExisting()).toBe(false);
    }
  });
});
