// Window / panels e2e — covers manual-test-guide section 12
// (T-window-001..005).
//
// Each panel toggle test boots the app, opens a project (so the panels
// have something to attach to), and drives the corresponding palette
// entry twice — once to flip the visibility, once to flip it back — to
// guarantee the test cleans up its own state.

import { $, browser, expect } from "@wdio/globals";
import { bootApp } from "../helpers/app.js";
import { byTestId, testid } from "../helpers/selectors.js";
import {
  getActiveProject,
  getPanelVisibility,
  isCommandPaletteOpen,
} from "../helpers/state.js";

/**
 * Sends Ctrl+K and waits until the palette open/closed state matches
 * `expectOpen`. Each call must release the chord cleanly so subsequent
 * keystrokes don't carry a stuck modifier.
 */
async function toggleCommandPalette(expectOpen: boolean): Promise<void> {
  await browser.keys(["Control", "k"]);
  await browser.waitUntil(
    async () => (await isCommandPaletteOpen()) === expectOpen,
    {
      timeout: 5000,
      timeoutMsg: `palette never reached open=${String(expectOpen)}`,
    },
  );
}

/**
 * Opens the palette, types `query`, presses Enter on the top match, and
 * waits for the palette to close after dispatch.
 */
async function dispatchViaPalette(query: string): Promise<void> {
  await toggleCommandPalette(true);
  const input = await $(byTestId(testid.commandPalette.input));
  await input.waitForDisplayed({ timeout: 5000 });
  await input.setValue(query);
  await browser.keys(["Enter"]);
  await browser.waitUntil(async () => !(await isCommandPaletteOpen()), {
    timeout: 5000,
    timeoutMsg: "palette did not close after Enter",
  });
}

/**
 * Idempotent: opens a fresh project via the welcome screen if one
 * isn't already active.
 */
async function ensureProjectOpen(): Promise<void> {
  if ((await getActiveProject()) !== null) return;
  const newProjectBtn = await $(byTestId(testid.welcome.newProject));
  await newProjectBtn.waitForClickable({ timeout: 5000 });
  await newProjectBtn.click();
  await browser.waitUntil(async () => (await getActiveProject()) !== null, {
    timeout: 10000,
    timeoutMsg: "active project never populated after New Project click",
  });
}

type PanelKey = "layers" | "timeline" | "palette" | "tilemap";

/**
 * Drives the toggle-N-panel command twice and asserts the named flag
 * flips each time. Sharing the body keeps the four T-window-00N tests
 * narrow without consolidating them — each `it()` still names its own
 * test ID.
 */
async function exerciseToggle(panel: PanelKey, query: string): Promise<void> {
  await bootApp();
  await ensureProjectOpen();

  const initial = (await getPanelVisibility())[panel];

  await dispatchViaPalette(query);
  await browser.waitUntil(
    async () => (await getPanelVisibility())[panel] !== initial,
    {
      timeout: 5000,
      timeoutMsg: `${panel} panel did not flip on first toggle`,
    },
  );

  await dispatchViaPalette(query);
  await browser.waitUntil(
    async () => (await getPanelVisibility())[panel] === initial,
    {
      timeout: 5000,
      timeoutMsg: `${panel} panel did not flip back on second toggle`,
    },
  );
}

describe("Window / panels (T-window)", () => {
  it("T-window-001: Toggle Layer panel flips isLayerPanelVisible", async () => {
    await exerciseToggle("layers", "toggle layer panel");
  });

  it("T-window-002: Toggle Timeline panel flips isTimelinePanelVisible", async () => {
    await exerciseToggle("timeline", "toggle timeline");
  });

  it("T-window-003: Toggle Color Palette panel flips isPalettePanelVisible", async () => {
    await exerciseToggle("palette", "toggle color palette");
  });

  it("T-window-004: Toggle Tilemap panel flips isTilemapPanelVisible", async () => {
    await exerciseToggle("tilemap", "toggle tilemap panel");
  });

  it("T-window-005: Preferences modal opens via Ctrl+, and closes via close button", async () => {
    await bootApp();

    // TODO(testid): no debug-surface accessor exists for preferences
    // open-state, and no data-testid is wired on the preferences modal.
    // Fall back to an aria-label dialog selector. The Escape-to-close
    // path requires keyboard focus inside the backdrop; the close-button
    // path is more reliable under WebDriver, where focus may not have
    // landed inside the modal yet.
    await browser.keys(["Control", ","]);

    const dialog = await $('[role="dialog"][aria-label*="Preferences" i]');
    await dialog.waitForDisplayed({ timeout: 5000 });
    await expect(dialog).toBeDisplayed();

    const closeBtn = await $('button[aria-label="Close Preferences"]');
    await closeBtn.click();
    await dialog.waitForDisplayed({ reverse: true, timeout: 5000 });
    await expect(await dialog.isDisplayed().catch(() => false)).toBe(false);
  });
});
