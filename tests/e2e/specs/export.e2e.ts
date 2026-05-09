// Export e2e — covers manual-test-guide section 3 (T-export-001..004).
//
// Each test boots the app, opens a project (new for PNG/GIF/WebP, the
// level-forest sample for TMX since it has tilemap layers), pre-queues
// a save-dialog mock with an output path under target/test-output/,
// dispatches the export command via the command palette, and asserts
// the corresponding IPC fires with the chosen path.
//
// Visual-diff assertions on the output files are deferred to a Phase 4
// follow-up — for now the contract under test is "command dispatches,
// IPC fires with right path".

import { $, browser, expect } from "@wdio/globals";
import { bootApp } from "../helpers/app.js";
import { byTestId, testid } from "../helpers/selectors.js";
import { getActiveProject } from "../helpers/state.js";
import { clearIpcLog, findIpcByCmd, waitForIpc } from "../helpers/ipc.js";
import {
  clearDialogQueue,
  mockOpenDialog,
  mockSaveDialog,
} from "../helpers/dialog.js";
import { isCommandPaletteOpen } from "../helpers/state.js";

// Output paths under target/test-output/ to keep export artifacts out of
// the way of cargo's incremental build cache.
const PNG_OUT =
  "C:/Users/luism/Documents/GitHub/pixhaus/target/test-output/sheet.png";
const GIF_OUT =
  "C:/Users/luism/Documents/GitHub/pixhaus/target/test-output/anim.gif";
const WEBP_OUT =
  "C:/Users/luism/Documents/GitHub/pixhaus/target/test-output/anim.webp";
const TMX_OUT =
  "C:/Users/luism/Documents/GitHub/pixhaus/target/test-output/level.tmx";

const SAMPLE_LEVEL_FOREST =
  "C:/Users/luism/Documents/GitHub/pixhaus/examples/samples/level-forest.pixhaus";

async function focusBody(): Promise<void> {
  await browser.execute(() => {
    (document.activeElement as HTMLElement | null)?.blur?.();
    document.body.focus();
  });
}

async function openNewProjectViaButton(): Promise<void> {
  await bootApp();
  const newProject = await $(byTestId(testid.welcome.newProject));
  await newProject.waitForDisplayed({ timeout: 5000 });
  await newProject.click();
  await browser.waitUntil(async () => (await getActiveProject()) !== null, {
    timeout: 10000,
    timeoutMsg: "active project never registered",
  });
}

async function openSampleViaDialog(absPath: string): Promise<void> {
  await bootApp();
  await clearDialogQueue();
  await mockOpenDialog(absPath);
  const openBtn = await $(byTestId(testid.welcome.openProject));
  await openBtn.waitForDisplayed({ timeout: 5000 });
  await openBtn.click();
  await browser.waitUntil(async () => (await getActiveProject()) !== null, {
    timeout: 15000,
    timeoutMsg: "sample project never opened",
  });
}

async function toggleCommandPalette(expectOpen: boolean): Promise<void> {
  await browser.execute(() => {
    const el = document.activeElement as HTMLElement | null;
    if (el && el.tagName !== "INPUT" && el.tagName !== "TEXTAREA") {
      el.blur?.();
      document.body.focus();
    }
  });
  await browser.keys(["Control", "k"]);
  await browser.waitUntil(
    async () => (await isCommandPaletteOpen()) === expectOpen,
    {
      timeout: 5000,
      timeoutMsg: `palette never reached open=${String(expectOpen)}`,
    },
  );
}

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

describe("Export (manual-test-guide §3)", () => {
  it("T-export-001: PNG sprite sheet export", async () => {
    await openNewProjectViaButton();
    await focusBody();
    await clearDialogQueue();
    await mockSaveDialog(PNG_OUT);
    await clearIpcLog();

    await dispatchViaPalette("export png sprite sheet");

    const entries = await waitForIpc("export_png_sprite_sheet", 1, 10000);
    await expect(entries.length).toBeGreaterThan(0);
    // Wrapped invoke logs args as `{ args: { sprite_id, output_path } }`.
    const outer = entries[0]?.args as
      | { args?: { output_path?: string } }
      | undefined;
    await expect(outer?.args?.output_path).toBe(PNG_OUT);
  });

  it("T-export-002: Animated GIF export", async () => {
    await openSampleViaDialog(SAMPLE_LEVEL_FOREST);
    await focusBody();
    await mockSaveDialog(GIF_OUT);
    await clearIpcLog();

    await dispatchViaPalette("export animated gif");

    const entries = await waitForIpc("export_animated_gif", 1, 10000);
    await expect(entries.length).toBeGreaterThan(0);
    const outer = entries[0]?.args as
      | { args?: { output_path?: string } }
      | undefined;
    await expect(outer?.args?.output_path).toBe(GIF_OUT);
  });

  it("T-export-003: Animated WebP export", async () => {
    await openSampleViaDialog(SAMPLE_LEVEL_FOREST);
    await focusBody();
    await mockSaveDialog(WEBP_OUT);
    await clearIpcLog();

    await dispatchViaPalette("export animated webp");

    const entries = await waitForIpc("export_animated_webp", 1, 10000);
    await expect(entries.length).toBeGreaterThan(0);
    const outer = entries[0]?.args as
      | { args?: { output_path?: string } }
      | undefined;
    await expect(outer?.args?.output_path).toBe(WEBP_OUT);
  });

  it("T-export-004: Tilemap TMX export", async () => {
    // level-forest is the canonical tilemap sample.
    await openSampleViaDialog(SAMPLE_LEVEL_FOREST);
    await focusBody();
    await mockSaveDialog(TMX_OUT);
    await clearIpcLog();

    await dispatchViaPalette("export tilemap");

    const entries = await waitForIpc("export_tmx", 1, 10000);
    await expect(entries.length).toBeGreaterThan(0);
    const outer = entries[0]?.args as
      | { args?: { output_path?: string } }
      | undefined;
    await expect(outer?.args?.output_path).toBe(TMX_OUT);
  });
});

// Mark `mockOpenDialog` as used so prettier doesn't flag the import — it
// is actually called inside `openSampleViaDialog`.
void mockOpenDialog;
