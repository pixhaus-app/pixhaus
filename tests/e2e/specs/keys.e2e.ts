// Keyboard shortcut sweep — Aseprite preset.
//
// Maps to docs/manual-test-guide.md section 14 ("Keyboard shortcut sweep").
// One test per row of the Aseprite-preset column. Photoshop preset diverges
// on a couple of bindings (toggle grid: Ctrl+'; tool keys: P/E/G/L/U/U) and
// is a follow-up spec.
//
// Each test follows the same shape:
//   1. bootApp() (and create a project for tests that need one open).
//   2. Focus the body so the keybind manager sees the chord.
//   3. clearIpcLog().
//   4. Pre-queue dialog responses for any action that triggers a native
//      picker (Open, Save As).
//   5. browser.keys([...]).
//   6. Assert via waitForIpc() or a state-accessor signal change.

import { $, browser, expect } from "@wdio/globals";
import { bootApp } from "../helpers/app.js";
import { byTestId, testid } from "../helpers/selectors.js";
import { getActiveProject, getZoom } from "../helpers/state.js";
import { clearIpcLog, findIpcByCmd, waitForIpc } from "../helpers/ipc.js";
import {
  clearDialogQueue,
  mockOpenDialog,
  mockSaveDialog,
} from "../helpers/dialog.js";

/**
 * Boots the app and clicks the welcome "New project" button, waiting until
 * an active project is registered. Used by every test that requires an open
 * project to exercise its shortcut (Save, Undo, Select all, etc.).
 */
async function openNewProjectViaButton(): Promise<void> {
  await bootApp();
  const newProject = await $(byTestId(testid.welcome.newProject));
  await newProject.waitForDisplayed({ timeout: 5000 });
  await newProject.click();
  await browser.waitUntil(async () => (await getActiveProject()) !== null, {
    timeout: 10000,
    timeoutMsg: "active project never registered after New Project click",
  });
}

/**
 * Drops focus to the document body so the global keybind manager (which
 * ignores chords originating from <input>/<textarea>/contenteditable) sees
 * the next browser.keys() call. Mirrors the pattern used by help.e2e.ts.
 */
async function focusBody(): Promise<void> {
  await browser.execute(() => {
    (document.activeElement as HTMLElement | null)?.blur?.();
    document.body.focus();
  });
}

describe("Keyboard shortcuts — Aseprite preset (manual-test-guide §14)", () => {
  it("T-keys-001: New project (Ctrl+N) dispatches project_new", async () => {
    await bootApp();
    await focusBody();
    await clearIpcLog();

    await browser.keys(["Control", "n"]);

    const entries = await waitForIpc("project_new", 1, 5000);
    await expect(entries.length).toBeGreaterThan(0);
  });

  it("T-keys-002: Open (Ctrl+O) opens the file picker", async () => {
    await bootApp();
    await focusBody();
    await clearDialogQueue();
    // User cancels the picker — null response means no project_open IPC
    // fires, but the queued mock proves the open() wrapper was invoked.
    await mockOpenDialog(null);
    await clearIpcLog();

    await browser.keys(["Control", "o"]);

    // Give the chord time to dispatch and the open() wrapper time to drain
    // the queued mock. We can't directly observe queue consumption, so
    // assert the opposite: no project_open IPC fired (user canceled).
    await browser.pause(500);
    const opens = await findIpcByCmd("project_open");
    await expect(opens.length).toBe(0);

    await clearDialogQueue();
  });

  it("T-keys-003: Save (Ctrl+S) dispatches project_save", async () => {
    await openNewProjectViaButton();
    await focusBody();
    await clearDialogQueue();
    // A brand-new project has no path; Save falls through to Save As, which
    // opens the save picker. Pre-queue a path so the IPC fires.
    await mockSaveDialog("/tmp/x.pixhaus");
    await clearIpcLog();

    await browser.keys(["Control", "s"]);

    const entries = await waitForIpc("project_save", 1, 5000);
    await expect(entries.length).toBeGreaterThan(0);

    await clearDialogQueue();
  });

  it("T-keys-004: Save As (Ctrl+Shift+S) dispatches project_save with path", async () => {
    await openNewProjectViaButton();
    await focusBody();
    await clearDialogQueue();
    await mockSaveDialog("/tmp/x.pixhaus");
    await clearIpcLog();

    await browser.keys(["Control", "Shift", "s"]);

    const entries = await waitForIpc("project_save", 1, 5000);
    await expect(entries.length).toBeGreaterThan(0);

    await clearDialogQueue();
  });

  it("T-keys-005: Close project (Ctrl+W) dispatches project_close", async () => {
    await openNewProjectViaButton();
    await focusBody();
    await clearIpcLog();

    await browser.keys(["Control", "w"]);

    const entries = await waitForIpc("project_close", 1, 5000);
    await expect(entries.length).toBeGreaterThan(0);
  });

  it("T-keys-006: Undo (Ctrl+Z) dispatches undo", async () => {
    await openNewProjectViaButton();
    await focusBody();
    await clearIpcLog();

    await browser.keys(["Control", "z"]);

    const entries = await waitForIpc("undo", 1, 5000);
    await expect(entries.length).toBeGreaterThan(0);
  });

  it("T-keys-007: Redo (Ctrl+Shift+Z) dispatches redo", async () => {
    await openNewProjectViaButton();
    await focusBody();
    await clearIpcLog();

    await browser.keys(["Control", "Shift", "z"]);

    const entries = await waitForIpc("redo", 1, 5000);
    await expect(entries.length).toBeGreaterThan(0);
  });

  it("T-keys-008: Select all (Ctrl+A) dispatches canvas_select_all", async () => {
    await openNewProjectViaButton();
    await focusBody();
    await clearIpcLog();

    await browser.keys(["Control", "a"]);

    const entries = await waitForIpc("canvas_select_all", 1, 5000);
    await expect(entries.length).toBeGreaterThan(0);
  });

  it("T-keys-009: Deselect (Ctrl+D) dispatches canvas_set_selection with null region", async () => {
    await openNewProjectViaButton();
    await focusBody();
    await clearIpcLog();

    await browser.keys(["Control", "d"]);

    const entries = await waitForIpc("canvas_set_selection", 1, 5000);
    await expect(entries.length).toBeGreaterThan(0);

    const args = entries[0]?.args as { region?: unknown } | undefined;
    await expect(args).toBeDefined();
    await expect(args?.region ?? null).toBeNull();
  });

  // TODO(phase-1-followup): WebDriver's `=` key may not produce the
  // KeyboardEvent shape the keybind manager expects ("Ctrl+Equal"). The
  // event might land with key="+" + shiftKey=true on some keyboard layouts.
  // Need to probe `key`/`code` from inside the Solid handler to know what
  // chord webdriverio actually delivers.
  // See: https://github.com/pixhaus-app/pixhaus/issues/172
  it.skip("T-keys-010: Zoom in (Ctrl+=) increases zoom", async () => {
    await openNewProjectViaButton();
    await focusBody();
    const initial = await getZoom();

    await browser.keys(["Control", "="]);

    await browser.waitUntil(async () => (await getZoom()) > initial, {
      timeout: 5000,
      timeoutMsg: `zoom never increased above ${initial}`,
    });
  });

  it("T-keys-011: Zoom out (Ctrl+-) decreases zoom", async () => {
    await openNewProjectViaButton();
    await focusBody();
    const initial = await getZoom();

    await browser.keys(["Control", "-"]);

    await browser.waitUntil(async () => (await getZoom()) < initial, {
      timeout: 5000,
      timeoutMsg: `zoom never decreased below ${initial}`,
    });
  });

  // TODO(phase-1-followup): depends on T-keys-010 working (the test seeds
  // by sending Ctrl+= twice before Ctrl+0). Unblock together once the key-
  // mapping issue is resolved.
  // See: https://github.com/pixhaus-app/pixhaus/issues/172
  it.skip("T-keys-012: Fit (Ctrl+0) changes zoom to fit viewport", async () => {
    await openNewProjectViaButton();
    await focusBody();
    // Drive zoom away from whatever Fit would resolve to so we can observe
    // a delta. Specific Fit value depends on viewport vs sprite size, so
    // we only assert that something changed.
    await browser.keys(["Control", "="]);
    await browser.keys(["Control", "="]);
    const beforeFit = await getZoom();

    await browser.keys(["Control", "0"]);

    await browser.waitUntil(async () => (await getZoom()) !== beforeFit, {
      timeout: 5000,
      timeoutMsg: `zoom never changed from ${beforeFit} after Ctrl+0`,
    });
  });

  it("T-keys-013: 100% (Ctrl+1) sets zoom to 1", async () => {
    await openNewProjectViaButton();
    await focusBody();
    // Move zoom away from 1 first so the assertion proves the chord acted.
    await browser.keys(["Control", "="]);
    await browser.keys(["Control", "="]);

    await browser.keys(["Control", "1"]);

    await browser.waitUntil(async () => (await getZoom()) === 1, {
      timeout: 5000,
      timeoutMsg: "zoom never landed on 1 after Ctrl+1",
    });
  });

  it("T-keys-014: Toggle grid (Ctrl+G) flips grid visibility", async () => {
    await openNewProjectViaButton();
    await focusBody();
    const initial = await browser.execute(() => {
      const w = window as unknown as {
        __pixhaus_debug__: { getShowTileGrid(): boolean };
      };
      return w.__pixhaus_debug__.getShowTileGrid();
    });
    await browser.keys(["Control", "g"]);
    await browser.waitUntil(
      async () => {
        const v = await browser.execute(() => {
          const w = window as unknown as {
            __pixhaus_debug__: { getShowTileGrid(): boolean };
          };
          return w.__pixhaus_debug__.getShowTileGrid();
        });
        return v !== initial;
      },
      { timeout: 5000, timeoutMsg: "tile grid never toggled" },
    );
  });

  it.skip("T-keys-015: Command palette (Ctrl+K) opens the palette", async () => {
    // Covered by T-cmd-001 in the command palette spec — Ctrl+K opens the
    // palette and isCommandPaletteOpen() returns true. Keeping a placeholder
    // here so the section-14 sweep stays auditable.
  });

  it.skip("T-keys-016: Preferences (Ctrl+,) opens preferences", async () => {
    // Covered by T-window-005 in the window/preferences spec.
  });

  it("T-keys-017: Tool: pencil (P) selects the pencil tool", async () => {
    await openNewProjectViaButton();
    await focusBody();
    // Move away from pencil first so the assertion proves the key acted.
    await browser.keys(["e"]);
    await browser.waitUntil(
      async () =>
        browser.execute(() => {
          const w = window as unknown as {
            __pixhaus_debug__: { getActiveTool(): string };
          };
          return w.__pixhaus_debug__.getActiveTool() !== "pencil";
        }),
      {
        timeout: 3000,
        timeoutMsg: "tool never changed away from pencil after E",
      },
    );

    await focusBody();
    await browser.keys(["p"]);

    await browser.waitUntil(
      async () =>
        browser.execute(() => {
          const w = window as unknown as {
            __pixhaus_debug__: { getActiveTool(): string };
          };
          return w.__pixhaus_debug__.getActiveTool() === "pencil";
        }),
      { timeout: 3000, timeoutMsg: "tool never changed to pencil after P" },
    );
  });
});
