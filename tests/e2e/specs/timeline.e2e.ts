// Timeline panel e2e — covers manual-test-guide section 9
// (T-timeline-001..010).
//
// Each test boots the app, opens a fresh project via the welcome screen
// "New project" button, waits for the active sprite + layer to register,
// then drives the timeline through the command palette and asserts on
// the IPC tap or the debug surface (frame count, isPlaying).
//
// Tests gated on UI not yet addressable from WebDriver — duration input,
// tag bar drag interactions, context menus, the loop checkbox, and the
// onion skin sliders — are skipped with a TODO note. They unblock once
// the corresponding testids land or palette commands ship.

import { $, browser, expect } from "@wdio/globals";
import { bootApp } from "../helpers/app.js";
import { byTestId, testid } from "../helpers/selectors.js";
import {
  getActiveLayerId,
  getActiveProject,
  getActiveSpriteId,
  getFrameCount,
  isCommandPaletteOpen,
} from "../helpers/state.js";
import { clearIpcLog, waitForIpc } from "../helpers/ipc.js";

/**
 * Boots the app and clicks the welcome "New project" button, waiting
 * until an active project is registered. Mirrors keys.e2e.ts so this
 * spec is self-contained.
 */
async function openNewProjectViaButton(): Promise<void> {
  await bootApp();
  const newProject = await $(byTestId(testid.welcome.newProject));
  await newProject.waitForDisplayed({ timeout: 5000 });
  await newProject.click();
  const createBtn = await $(byTestId(testid.canvasSizeDialog.create));
  await createBtn.waitForDisplayed({ timeout: 5000 });
  await createBtn.click();
  await browser.waitUntil(async () => (await getActiveProject()) !== null, {
    timeout: 10000,
    timeoutMsg: "active project never registered after New Project click",
  });
}

/**
 * Drops focus to the document body so the global keybind manager (which
 * ignores chords from <input>/<textarea>/contenteditable) sees the next
 * browser.keys() call.
 */
async function focusBody(): Promise<void> {
  await browser.execute(() => {
    const el = document.activeElement as HTMLElement | null;
    if (el && el.tagName !== "INPUT" && el.tagName !== "TEXTAREA") {
      el.blur?.();
      document.body.focus();
    }
  });
}

/**
 * Sends Ctrl+K and waits until the palette open/closed state matches
 * `expectOpen`. Inlined here (not shared with cmd.e2e.ts) so this spec
 * is self-contained.
 */
async function toggleCommandPalette(expectOpen: boolean): Promise<void> {
  await focusBody();
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
 * Opens the palette, types `query`, and presses Enter to dispatch the
 * top-ranked match. Waits for the palette to close after dispatch so a
 * subsequent open() sees a clean state.
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
 * Waits for the active sprite + layer to register after createNewProject.
 * Frame commands return early when activeSpriteId is null
 * (command-registry.ts handlers guard on this), so any frame_add /
 * frame_delete / frame_duplicate dispatched before the seed sprite
 * registers is silently dropped.
 *
 * Mirrors the workaround in tools.e2e.ts: the production auto-pick path
 * doesn't reliably populate activeLayerId under tauri-driver, so we
 * force-set it via __pixhaus_debug__.layer.setActiveByIndex(0) as a
 * belt-and-braces fallback.
 */
async function waitForActiveSpriteAndLayer(timeoutMs = 15000): Promise<void> {
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
 * Reads the timeline-state isPlaying signal off the debug surface. The
 * shared state helper module doesn't expose this yet; inlining keeps
 * the spec self-contained.
 */
async function getIsPlaying(): Promise<boolean> {
  return browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: { getIsPlaying(): boolean };
    };
    return w.__pixhaus_debug__.getIsPlaying();
  });
}

async function getIsLooping(): Promise<boolean> {
  return browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: { getIsLooping(): boolean };
    };
    return w.__pixhaus_debug__.getIsLooping();
  });
}

describe("Timeline panel (manual-test-guide §9)", () => {
  it("T-timeline-001: Add frame", async () => {
    await openNewProjectViaButton();
    await waitForActiveSpriteAndLayer();
    const before = await getFrameCount();
    await clearIpcLog();

    // "Add Frame" is the label on the frame:new command (command-registry.ts).
    await dispatchViaPalette("add frame");

    const entries = await waitForIpc("frame_add", 1, 10000);
    await expect(entries.length).toBeGreaterThan(0);
    await browser.waitUntil(async () => (await getFrameCount()) > before, {
      timeout: 5000,
      timeoutMsg: `frame count never increased above ${before}`,
    });
  });

  it("T-timeline-002: Delete frame", async () => {
    await openNewProjectViaButton();
    await waitForActiveSpriteAndLayer();

    // Pre: at least 2 frames. The seed project's frame count varies
    // (0 if no default seeded; 1 if so). Read live and add until >= 2.
    let count = await getFrameCount();
    while (count < 2) {
      await dispatchViaPalette("add frame");
      await waitForIpc("frame_add", count + 1, 10000);
      await browser.waitUntil(async () => (await getFrameCount()) > count, {
        timeout: 5000,
        timeoutMsg: `frame count never increased above ${count}`,
      });
      count = await getFrameCount();
    }
    const before = await getFrameCount();
    await clearIpcLog();

    await dispatchViaPalette("delete frame");

    const entries = await waitForIpc("frame_delete", 1, 10000);
    await expect(entries.length).toBeGreaterThan(0);
    await browser.waitUntil(async () => (await getFrameCount()) < before, {
      timeout: 5000,
      timeoutMsg: `frame count never decreased below ${before}`,
    });
  });

  // TODO(phase-3-followup): same frame-cache race as T-timeline-002.
  it("T-timeline-003: Duplicate frame", async () => {
    await openNewProjectViaButton();
    await waitForActiveSpriteAndLayer();
    // Need at least 1 frame to duplicate. Seed if necessary.
    let before = await getFrameCount();
    if (before === 0) {
      await dispatchViaPalette("add frame");
      await waitForIpc("frame_add", 1, 10000);
      await browser.waitUntil(async () => (await getFrameCount()) > 0, {
        timeout: 5000,
        timeoutMsg: "frame count never reached 1 before duplicate",
      });
      before = await getFrameCount();
    }
    await clearIpcLog();

    await dispatchViaPalette("duplicate frame");

    const entries = await waitForIpc("frame_duplicate", 1, 10000);
    await expect(entries.length).toBeGreaterThan(0);
    await browser.waitUntil(async () => (await getFrameCount()) > before, {
      timeout: 5000,
      timeoutMsg: `frame count never increased above ${before}`,
    });
  });

  it("T-timeline-004: Set frame duration", async () => {
    await openNewProjectViaButton();
    await waitForActiveSpriteAndLayer();

    // Pre: at least one frame. Seed one if the project starts empty.
    let count = await getFrameCount();
    if (count === 0) {
      await dispatchViaPalette("add frame");
      await waitForIpc("frame_add", 1, 10000);
      await browser.waitUntil(async () => (await getFrameCount()) > 0, {
        timeout: 5000,
        timeoutMsg: "frame count never reached 1 before duration edit",
      });
    }
    await clearIpcLog();

    // Double-click the duration display for frame 0 to enter edit mode.
    const durDisplay = await $(byTestId(testid.timeline.frameDuration(0)));
    await durDisplay.waitForDisplayed({ timeout: 5000 });
    await durDisplay.doubleClick();

    // Type a new duration and commit with Enter.
    const durInput = await $(".timeline-panel__dur-input");
    await durInput.waitForDisplayed({ timeout: 3000 });
    await durInput.setValue("200");
    await browser.keys(["Enter"]);

    const entries = await waitForIpc("frame_set_duration", 1, 5000);
    await expect(entries.length).toBeGreaterThan(0);
  });

  it.skip("T-timeline-005: Create a frame tag", async () => {
    // testid="tl-tag-bar" is now wired on the tag bar container.
    // Tag creation requires a pointer drag across the tag bar — the bar
    // uses onPointerDown/onPointerMove/onPointerUp (not HTML5 drag events),
    // so WebDriver's action chain should work in principle. However, the
    // drag must land on exact frame-column positions derived from
    // FRAME_WIDTH (36px), and tauri-driver's synthetic pointer events
    // haven't been validated for this path under CI yet.
    // See: https://github.com/pixhaus-app/pixhaus/issues/171
  });

  it.skip("T-timeline-006: Rename / delete tag", async () => {
    // testid="tl-tag-${name}" on tag elements and testid="tl-ctx-rename" /
    // testid="tl-ctx-delete" on the context menu items are now wired.
    // Blocked on T-timeline-005 (no tags to right-click without creation).
    // See: https://github.com/pixhaus-app/pixhaus/issues/171
  });

  it("T-timeline-007: Play", async () => {
    await openNewProjectViaButton();
    await waitForActiveSpriteAndLayer();

    // Pre: ≥2 frames so playback has something to cycle through.
    let count = await getFrameCount();
    while (count < 2) {
      await dispatchViaPalette("add frame");
      await waitForIpc("frame_add", count + 1, 10000);
      await browser.waitUntil(async () => (await getFrameCount()) > count, {
        timeout: 5000,
        timeoutMsg: `frame count never increased above ${count}`,
      });
      count = await getFrameCount();
    }

    // No palette command for play exists today (command-registry.ts has
    // no timeline:play entry). Drive playback by clicking the play button
    // in the timeline header. The button toggles between Play/Pause and
    // its label reads "Play" when stopped.
    const playBtn = await $('button[title^="Play"]');
    await playBtn.waitForDisplayed({ timeout: 5000 });
    await playBtn.click();

    await browser.waitUntil(async () => (await getIsPlaying()) === true, {
      timeout: 5000,
      timeoutMsg: "isPlaying never flipped to true after Play",
    });
  });

  it("T-timeline-008: Pause", async () => {
    await openNewProjectViaButton();
    await waitForActiveSpriteAndLayer();

    // Pre: ≥2 frames + playback running.
    let count = await getFrameCount();
    while (count < 2) {
      await dispatchViaPalette("add frame");
      await waitForIpc("frame_add", count + 1, 10000);
      await browser.waitUntil(async () => (await getFrameCount()) > count, {
        timeout: 5000,
        timeoutMsg: `frame count never increased above ${count}`,
      });
      count = await getFrameCount();
    }

    const playBtn = await $('button[title^="Play"]');
    await playBtn.waitForDisplayed({ timeout: 5000 });
    await playBtn.click();
    await browser.waitUntil(async () => (await getIsPlaying()) === true, {
      timeout: 5000,
      timeoutMsg: "isPlaying never flipped to true before Pause",
    });

    // After play, the same button's title flips to "Pause (Space)".
    const pauseBtn = await $('button[title^="Pause"]');
    await pauseBtn.waitForDisplayed({ timeout: 5000 });
    await pauseBtn.click();

    await browser.waitUntil(async () => (await getIsPlaying()) === false, {
      timeout: 5000,
      timeoutMsg: "isPlaying never flipped to false after Pause",
    });
  });

  it("T-timeline-009: Loop", async () => {
    await openNewProjectViaButton();
    await waitForActiveSpriteAndLayer();
    const initial = await getIsLooping();
    await dispatchViaPalette("toggle playback loop");
    await browser.waitUntil(async () => (await getIsLooping()) !== initial, {
      timeout: 5000,
      timeoutMsg: "loop flag never toggled",
    });
  });

  it("T-timeline-010: Onion skin", async () => {
    await openNewProjectViaButton();
    await waitForActiveSpriteAndLayer();

    const initial = await browser.execute(() => {
      const w = window as unknown as {
        __pixhaus_debug__: {
          getOnionSkin(): { enabled: boolean; prev: number; next: number };
        };
      };
      return w.__pixhaus_debug__.getOnionSkin();
    });

    // Enable onion skin if it's off so the sliders are rendered.
    if (!initial.enabled) {
      await dispatchViaPalette("toggle onion skin");
      await browser.waitUntil(
        async () =>
          browser.execute(() => {
            const w = window as unknown as {
              __pixhaus_debug__: { getOnionSkin(): { enabled: boolean } };
            };
            return w.__pixhaus_debug__.getOnionSkin().enabled;
          }),
        { timeout: 3000, timeoutMsg: "onion skin never enabled" },
      );
    }

    const prevInput = await $(byTestId(testid.timeline.onionPrev));
    await prevInput.waitForDisplayed({ timeout: 3000 });
    // Set prev frames to 2 and confirm the signal updated.
    await prevInput.setValue("2");
    // Blur to trigger onInput handler.
    await browser.execute(() => {
      (document.activeElement as HTMLElement | null)?.blur?.();
    });

    await browser.waitUntil(
      async () =>
        browser.execute(() => {
          const w = window as unknown as {
            __pixhaus_debug__: { getOnionSkin(): { prev: number } };
          };
          return w.__pixhaus_debug__.getOnionSkin().prev === 2;
        }),
      { timeout: 3000, timeoutMsg: "onion prev never updated" },
    );

    const afterPrev = await browser.execute(() => {
      const w = window as unknown as {
        __pixhaus_debug__: { getOnionSkin(): { prev: number } };
      };
      return w.__pixhaus_debug__.getOnionSkin().prev;
    });
    await expect(afterPrev).toBe(2);
  });
});
