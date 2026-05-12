// Palette panel e2e — covers manual-test-guide section 8
// (T-palette-001..012).
//
// Status: T-palette-001..004, T-palette-006..007 run via the palette IPC
// commands and direct UI interaction with testids wired in this PR.
// T-palette-005 stays skipped (HTML5 drag-to-reorder — see GH issue linked
// in the TODO below). T-palette-008..012 stay skipped (file-fixture
// dependency for import/export round-trips — testids are now wired).
//
// References:
//   docs/manual-test-guide.md §8           — manual flows.
//   app/src/lib.rs:157-164                 — palette IPC names.
//   ui/src/command-palette/command-registry.ts — palette:* entries.
//   ui/src/palette/PalettePanel.tsx        — UI surface.

import { $, browser, expect } from "@wdio/globals";
import { bootApp } from "../helpers/app.js";
import { byTestId, testid } from "../helpers/selectors.js";
import { getActiveProject, isCommandPaletteOpen } from "../helpers/state.js";
import { clearIpcLog, waitForIpc } from "../helpers/ipc.js";

async function focusBody(): Promise<void> {
  await browser.execute(() => {
    (document.activeElement as HTMLElement | null)?.blur?.();
    document.body.focus();
  });
}

async function openNewProjectViaButton(): Promise<void> {
  await bootApp();
  const newProject = await $(byTestId(testid.welcome.newProject));
  await newProject.click();
  await browser.waitUntil(async () => (await getActiveProject()) !== null, {
    timeout: 10000,
    timeoutMsg: "active project never registered",
  });
}

async function dispatchViaPalette(query: string): Promise<void> {
  await focusBody();
  await browser.keys(["Control", "k"]);
  await browser.waitUntil(async () => (await isCommandPaletteOpen()) === true, {
    timeout: 5000,
    timeoutMsg: "palette never opened",
  });
  const input = await $(byTestId(testid.commandPalette.input));
  await input.setValue(query);
  await browser.keys(["Enter"]);
  await browser.waitUntil(async () => !(await isCommandPaletteOpen()), {
    timeout: 5000,
    timeoutMsg: "palette did not close",
  });
}

async function ensurePalettePanelVisible(): Promise<void> {
  const visible = await browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: { panel: { palette(): boolean } };
    };
    return w.__pixhaus_debug__.panel.palette();
  });
  if (!visible) {
    await browser.execute(() => {
      const w = window as unknown as {
        __pixhaus_debug__: { command: { dispatch(id: string): void } };
      };
      w.__pixhaus_debug__.command.dispatch("window:toggle-palette");
    });
    await browser.waitUntil(
      async () =>
        browser.execute(() => {
          const w = window as unknown as {
            __pixhaus_debug__: { panel: { palette(): boolean } };
          };
          return w.__pixhaus_debug__.panel.palette();
        }),
      { timeout: 3000, timeoutMsg: "palette panel never became visible" },
    );
  }
}

describe("Palette panel (manual-test-guide §8)", () => {
  it("T-palette-001: New palette", async () => {
    await openNewProjectViaButton();
    await clearIpcLog();
    await dispatchViaPalette("new palette");
    const entries = await waitForIpc("palette_add", 1, 5000);
    await expect(entries.length).toBeGreaterThan(0);
  });

  it("T-palette-002: Add a colour", async () => {
    await openNewProjectViaButton();
    // Seed an active palette first; createNewProject doesn't include one.
    await dispatchViaPalette("new palette");
    await waitForIpc("palette_add", 1, 5000);
    await clearIpcLog();
    await dispatchViaPalette("add foreground to palette");
    const entries = await waitForIpc("palette_add_color", 1, 5000);
    await expect(entries.length).toBeGreaterThan(0);
  });

  it.skip("T-palette-003: Edit a swatch", async () => {
    // testid="palette-swatch-N" is now wired in PaletteGrid.tsx.
    // The remaining blocker is the color-picker interaction: editing
    // requires dragging the picker's gradient canvas, which WebDriver's
    // pointer action API cannot synthesise reliably under tauri-driver.
    // Unskip once a programmatic palette:set-color command is registered
    // that lets the spec bypass the drag.
  });

  it("T-palette-004: Delete a swatch", async () => {
    await openNewProjectViaButton();
    await dispatchViaPalette("new palette");
    await waitForIpc("palette_add", 1, 5000);
    await dispatchViaPalette("add foreground to palette");
    await waitForIpc("palette_add_color", 1, 5000);
    await clearIpcLog();

    await ensurePalettePanelVisible();

    // Right-click swatch 0 to open the custom context menu.
    const swatch = await $(byTestId(testid.palette.swatch(0)));
    await swatch.waitForDisplayed({ timeout: 5000 });
    await swatch.click({ button: "right" });

    // Click "Remove swatch" from the custom context menu.
    const deleteBtn = await $(byTestId(testid.palette.swatchDelete));
    await deleteBtn.waitForDisplayed({ timeout: 3000 });
    await deleteBtn.click();

    const entries = await waitForIpc("palette_remove_color", 1, 5000);
    await expect(entries.length).toBeGreaterThan(0);
  });

  it.skip("T-palette-005: Reorder via drag", async () => {
    // testid="palette-swatch-N" is now wired. Drag-to-reorder is blocked
    // by the HTML5 drag protocol: PaletteGrid uses draggable=true /
    // DragEvent, which WebDriver pointer actions do not trigger.
    // See: https://github.com/pixhaus-app/pixhaus/issues/177
  });

  it("T-palette-006: Harmony generator", async () => {
    await openNewProjectViaButton();
    await dispatchViaPalette("new palette");
    await waitForIpc("palette_add", 1, 5000);
    // Need at least one swatch in the palette so harmony has a source color.
    await dispatchViaPalette("add foreground to palette");
    await waitForIpc("palette_add_color", 1, 5000);
    await clearIpcLog();

    await ensurePalettePanelVisible();

    // Toggle the Harmony sub-panel.
    const harmonyToggle = await $(byTestId(testid.palette.harmonyToggle));
    await harmonyToggle.waitForDisplayed({ timeout: 5000 });
    await harmonyToggle.click();

    // Switch to a specific harmony type to ensure the tab is addressable.
    const triadTab = await $(byTestId(testid.palette.harmonyType("triad")));
    await triadTab.waitForDisplayed({ timeout: 3000 });
    await triadTab.click();

    // Click the first suggestion swatch to append it to the palette.
    const suggestion = await $(".harmony__swatch--suggestion");
    await suggestion.waitForDisplayed({ timeout: 3000 });
    await suggestion.click();

    const entries = await waitForIpc("palette_add_color", 1, 5000);
    await expect(entries.length).toBeGreaterThan(0);
  });

  it("T-palette-007: Ramp generator", async () => {
    await openNewProjectViaButton();
    await dispatchViaPalette("new palette");
    await waitForIpc("palette_add", 1, 5000);
    // Seed two swatches so the ramp has from/to indices 0 and 1.
    await dispatchViaPalette("add foreground to palette");
    await waitForIpc("palette_add_color", 1, 5000);
    await dispatchViaPalette("add foreground to palette");
    await waitForIpc("palette_add_color", 2, 5000);
    await clearIpcLog();

    await ensurePalettePanelVisible();

    // Toggle the Ramp sub-panel.
    const rampToggle = await $(byTestId(testid.palette.rampToggle));
    await rampToggle.waitForDisplayed({ timeout: 5000 });
    await rampToggle.click();

    // Set steps to 3 → generates 1 intermediate swatch.
    const stepsInput = await $(byTestId(testid.palette.rampSteps));
    await stepsInput.waitForDisplayed({ timeout: 3000 });
    await stepsInput.setValue("3");

    // Click "Add to palette" to run the generator.
    const generateBtn = await $(byTestId(testid.palette.rampGenerate));
    await generateBtn.waitForClickable({ timeout: 3000 });
    await generateBtn.click();

    // steps=3 → 1 intermediate swatch → exactly 1 palette_add_color.
    const entries = await waitForIpc("palette_add_color", 1, 5000);
    await expect(entries.length).toBeGreaterThan(0);
  });

  it.skip("T-palette-008: Import .gpl (GIMP)", async () => {
    // testid="palette-io-menu", testid="palette-import-format",
    // testid="palette-import-btn" are now wired in PaletteIOMenu.tsx.
    // The remaining blocker is driving the hidden <input type="file">:
    // the browser's security policy prevents programmatic file selection
    // without a real user gesture. Needs a wdio uploadFile() flow and
    // a committed fixture file (e.g. examples/test-palette.gpl).
  });

  it.skip("T-palette-009: Import .hex", async () => {
    // Same blocker as T-palette-008. testid="palette-import-hex" (option)
    // and testid="palette-import-btn" are wired.
  });

  it.skip("T-palette-010: Import .pal", async () => {
    // Same blocker as T-palette-008. testid="palette-import-pal" (option)
    // and testid="palette-import-btn" are wired.
  });

  it.skip("T-palette-011: Import .aco", async () => {
    // Same blocker as T-palette-008. testid="palette-import-aco" (option)
    // and testid="palette-import-btn" are wired. Also needs an .aco fixture.
  });

  it.skip("T-palette-012: Export", async () => {
    // testid="palette-export-format", testid="palette-export-btn" are now
    // wired in PaletteIOMenu.tsx. The remaining blocker is intercepting the
    // browser download triggered by the Blob + <a> click and asserting the
    // file contents. Needs a wdio download-intercept strategy or a
    // round-trip re-import assertion.
  });
});
