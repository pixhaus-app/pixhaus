// Help specs — About modal + Documentation link.
//
// Maps to docs/manual-test-guide.md section 13 ("Help"):
//   - T-help-001: About modal shows version
//   - T-help-002: Docs link opens browser
//
// Both flows go through the command palette (Ctrl+K), since the menu
// bar is owned by the OS shell and isn't reliably drivable via
// tauri-driver. The palette dispatches the same command IDs the menu
// items would, so coverage is equivalent.

import { $, browser, expect } from "@wdio/globals";
import { bootApp } from "../helpers/app.js";
import { byTestId, testid } from "../helpers/selectors.js";
import { isCommandPaletteOpen } from "../helpers/state.js";
import { clearIpcLog, waitForIpc } from "../helpers/ipc.js";

/** Opens the command palette via Ctrl+K and waits for the input to mount. */
async function openCommandPalette(): Promise<void> {
  // Ensure focus is on the body so the keybind manager sees the chord.
  await browser.execute(() => {
    (document.activeElement as HTMLElement | null)?.blur?.();
    document.body.focus();
  });
  await browser.keys(["Control", "k"]);
  await browser.waitUntil(async () => isCommandPaletteOpen(), {
    timeout: 5000,
    timeoutMsg: "command palette never opened after Ctrl+K",
  });
  const input = await $(byTestId(testid.commandPalette.input));
  await input.waitForDisplayed({ timeout: 5000 });
}

describe("Help (manual-test-guide §13)", () => {
  it("T-help-001: About modal shows version", async () => {
    await bootApp();
    await clearIpcLog();
    await openCommandPalette();

    const input = await $(byTestId(testid.commandPalette.input));
    await input.setValue("about");

    // The "help:about" command should be the top match for "about".
    const aboutItem = await $(
      byTestId(testid.commandPalette.item("help:about")),
    );
    await aboutItem.waitForDisplayed({ timeout: 3000 });

    await browser.keys(["Enter"]);

    // The handler invokes app_about and renders a message dialog with
    // the version. The IPC firing is the deterministic signal — the
    // dialog itself is rendered by the OS, not the WebView.
    const entries = await waitForIpc("app_about", 1, 5000);
    await expect(entries.length).toBeGreaterThan(0);

    const result = entries[0]?.result as
      | { name?: string; version?: string }
      | undefined;
    await expect(result).toBeDefined();
    await expect(typeof result?.version).toBe("string");
    // Versions look like "0.1.0" or "1.2.3-rc.1"; just assert non-empty
    // and digit-led so we don't pin to a specific release.
    await expect(result?.version ?? "").toMatch(/^\d+\.\d+\.\d+/);
  });

  it.skip("T-help-002: Docs link opens browser", async () => {
    // Per docs/manual-test-guide.md §13 T-help-002, the "Documentation"
    // command opens https://pixhaus.app/docs in an external browser.
    // Under tauri-driver there is no concrete observable behavior we can
    // assert against — no IPC is fired (the handler calls window.open
    // when the shell plugin is unavailable, and the OS browser launch
    // is opaque to the WebDriver session). Skipping until either
    //   (a) the handler emits a Tauri event we can subscribe to, or
    //   (b) we wire a tauri-plugin-shell mock into the harness.
    // TODO(testid): no IPC/event hook to assert docs open under tauri-driver
  });
});
