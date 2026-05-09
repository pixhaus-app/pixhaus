// Layer panel e2e — covers manual-test-guide section 7
// (T-layers-001..009).
//
// Most layer mutations live behind the layer-panel context menu, drag
// gestures, or multi-select clicks — none of which are addressable by
// testid yet. The skipped tests below document why each one waits on a
// follow-up (testids, palette commands, or harness drag support).
//
// The wired surface for this spec is the layer-add button + the command
// palette. T-layers-001 drives the button; the rest depend on palette
// commands that either don't exist yet (merge-selected, convert-to-group)
// or are stubbed (merge-down, flatten-visible) in command-registry.ts.

import { $, browser, expect } from "@wdio/globals";
import { bootApp } from "../helpers/app.js";
import { byTestId, testid } from "../helpers/selectors.js";
import {
  getActiveLayerId,
  getActiveProject,
  getActiveSpriteId,
  getLayerCount,
  isCommandPaletteOpen,
} from "../helpers/state.js";
import { clearIpcLog, waitForIpc } from "../helpers/ipc.js";

/**
 * Boots the app and clicks the welcome "New project" button, waiting until
 * an active project is registered. Mirrors the helper used by keys.e2e.ts
 * and tools.e2e.ts.
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
 * ignores chords from inputs / contenteditable) sees the next chord.
 * Mirrors the pattern in tools.e2e.ts and keys.e2e.ts.
 */
async function focusBody(): Promise<void> {
  await browser.execute(() => {
    (document.activeElement as HTMLElement | null)?.blur?.();
    document.body.focus();
  });
}

/**
 * Waits for `getActiveLayerId()` to return non-null. createNewProject seeds
 * a default raster layer, but the auto-pick path doesn't always fire under
 * tauri-driver — force the active layer via the debug surface as a belt-
 * and-braces fallback. Mirrors the helper in tools.e2e.ts.
 */
async function waitForActiveLayer(timeoutMs = 15000): Promise<void> {
  await browser.waitUntil(async () => (await getActiveSpriteId()) !== null, {
    timeout: timeoutMs,
    timeoutMsg: "active sprite never registered after project open",
  });
  await browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__?: { layer?: { refresh(): void } };
    };
    w.__pixhaus_debug__?.layer?.refresh();
  });
  await browser.waitUntil(
    async () => {
      const count = await browser.execute(() => {
        const w = window as unknown as {
          __pixhaus_debug__?: { layer?: { list(): unknown[] } };
        };
        return w.__pixhaus_debug__?.layer?.list().length ?? 0;
      });
      return count > 0;
    },
    {
      timeout: timeoutMs,
      timeoutMsg: "layer list never populated after refresh",
    },
  );
  await browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__?: { layer?: { setActiveByIndex(i: number): boolean } };
    };
    w.__pixhaus_debug__?.layer?.setActiveByIndex(0);
  });
  await browser.waitUntil(async () => (await getActiveLayerId()) !== null, {
    timeout: 5000,
    timeoutMsg: "active layer never registered after setActiveByIndex",
  });
}

/**
 * Opens the palette via Ctrl+K, types `query`, and presses Enter to
 * dispatch the top-ranked match. Mirrors the helper in cmd.e2e.ts. The
 * palette closes itself on dispatch; we wait for that to settle so the
 * next call sees a clean state.
 */
async function dispatchViaPalette(query: string): Promise<void> {
  await focusBody();
  await browser.keys(["Control", "k"]);
  await browser.waitUntil(async () => (await isCommandPaletteOpen()) === true, {
    timeout: 5000,
    timeoutMsg: "palette never opened on Ctrl+K",
  });

  const input = await $(byTestId(testid.commandPalette.input));
  await input.waitForDisplayed({ timeout: 5000 });
  await input.setValue(query);
  await browser.keys(["Enter"]);

  await browser.waitUntil(async () => !(await isCommandPaletteOpen()), {
    timeout: 5000,
    timeoutMsg: "palette did not close after Enter",
  });
}

describe("Layer panel (manual-test-guide §7)", () => {
  it("T-layers-001: Add layer.", async () => {
    await openNewProjectViaButton();
    await waitForActiveLayer();

    // createNewProject seeds one default raster layer; the panel header
    // "+" button appends a sibling, taking the count to 2.
    await expect(await getLayerCount()).toBe(1);

    await clearIpcLog();
    const addBtn = await $(byTestId("layer-add"));
    await addBtn.waitForClickable({ timeout: 5000 });
    await addBtn.click();

    const adds = await waitForIpc("layer_add", 1, 5000);
    await expect(adds.length).toBeGreaterThan(0);

    await browser.waitUntil(async () => (await getLayerCount()) === 2, {
      timeout: 5000,
      timeoutMsg: "layer count never reached 2 after layer-add click",
    });
  });

  // TODO(testid): the per-row context menu (Rename, Delete, Merge Down,
  // Convert to Group, Convert to Tilemap) doesn't expose any data-testids
  // yet, and there's no palette command that triggers the inline rename
  // flow. Add testids to LayerRow's context menu items before unskipping.
  it.skip("T-layers-002: Rename via context menu.", async () => {
    // Blocked on context-menu testids.
  });

  // TODO(testid): the palette has a "Delete Layer" command (layer:delete)
  // and the IPC fires correctly, but the manual guide also asserts on a
  // confirmation dialog that has no testid yet. Promote this test once the
  // confirm() wrapper exposes accept/cancel testids — until then, the
  // dialog auto-resolves under tauri-driver in a way that's racy with the
  // IPC tap. Right-click context-menu delete is not addressable either.
  it.skip("T-layers-003: Delete with confirmation.", async () => {
    // Blocked on confirmation dialog testid.
  });

  // TODO(harness): drag-to-reorder requires synthesising HTML5 drag events
  // (dragstart/dragover/drop) which webdriverio's pointer action API does
  // not generate. Layer rows depend on the native drag protocol, not the
  // pointer-events drag flow exercised by canvas drag tests. Add a
  // reorderByIndex(i, j) helper to __pixhaus_debug__.layer or a custom
  // drag-event dispatcher before unskipping.
  it.skip("T-layers-004: Drag to reorder.", async () => {
    // Blocked on drag harness support.
  });

  it("T-layers-005: Merge Down composites pixels and drops the active layer.", async () => {
    await openNewProjectViaButton();
    await waitForActiveLayer();
    // Add a second layer so we have something to merge.
    const addBtn = await $(byTestId("layer-add"));
    await addBtn.click();
    await waitForIpc("layer_add", 1, 5000);
    await clearIpcLog();
    await dispatchViaPalette("merge down");
    await waitForIpc("layer_merge_down", 1, 5000);
  });

  it("T-layers-006: Merge Selected.", async () => {
    await openNewProjectViaButton();
    await waitForActiveLayer();
    // Add 2 more layers so we have 3 total.
    const addBtn = await $(byTestId("layer-add"));
    await addBtn.click();
    await waitForIpc("layer_add", 1, 5000);
    await addBtn.click();
    await waitForIpc("layer_add", 2, 5000);
    await clearIpcLog();
    await dispatchViaPalette("merge selected");
    await waitForIpc("layer_merge_selected", 1, 5000);
  });

  it("T-layers-007: Flatten Visible.", async () => {
    await openNewProjectViaButton();
    await waitForActiveLayer();
    const addBtn = await $(byTestId("layer-add"));
    await addBtn.click();
    await waitForIpc("layer_add", 1, 5000);
    await clearIpcLog();
    await dispatchViaPalette("flatten visible");
    await waitForIpc("layer_flatten_visible", 1, 5000);
  });

  it("T-layers-008: Convert to Group.", async () => {
    await openNewProjectViaButton();
    await waitForActiveLayer();
    await clearIpcLog();
    await dispatchViaPalette("convert to group");
    await waitForIpc("layer_wrap_in_group", 1, 5000);
  });

  // TODO(deps): convert-to-tilemap needs an existing tileset on the sprite
  // (T-tilemap-001 covers tileset creation) and routes through a tileset
  // picker dialog. Both prerequisites are absent from the e2e harness.
  it.skip("T-layers-009: Convert to Tilemap Layer.", async () => {
    // Blocked on tileset fixture + picker dialog testids.
  });
});
