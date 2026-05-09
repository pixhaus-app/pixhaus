// Canvas viewport e2e — covers manual-test-guide section 4
// (T-canvas-001..009).
//
// Tests exercise pan, zoom, and overlay toggles through the same input
// surfaces a user touches: spacebar+drag, middle-mouse drag, wheel,
// modifier-held wheel, Ctrl-chord shortcuts, and the command palette.
//
// Where the browser action API can't reproduce a native gesture cleanly
// (wheel events with modifiers, Ctrl+= chord — see keys.e2e.ts T-keys-010),
// the test is skipped with a TODO citing the underlying gap.

import { $, browser, expect } from "@wdio/globals";
import { bootApp } from "../helpers/app.js";
import { byTestId, testid } from "../helpers/selectors.js";
import { getActiveProject, getScroll, getZoom } from "../helpers/state.js";
import { clearIpcLog } from "../helpers/ipc.js";

/**
 * Boots the app and clicks the welcome "New project" button, waiting
 * until an active project is registered. Mirrors openNewProjectViaButton
 * from keys.e2e.ts; canvas tests need an editor surface mounted before
 * they can drive the viewport.
 */
async function openProject(): Promise<void> {
  await bootApp();
  const newProject = await $(byTestId(testid.welcome.newProject));
  await newProject.waitForDisplayed({ timeout: 5000 });
  await newProject.click();
  await browser.waitUntil(async () => (await getActiveProject()) !== null, {
    timeout: 10000,
    timeoutMsg: "active project never set",
  });
}

/**
 * Drops focus to the document body so the global keybind manager (which
 * ignores chords originating from <input>/<textarea>/contenteditable)
 * sees the next browser.keys() call. Mirrors the pattern in keys.e2e.ts.
 */
async function focusBody(): Promise<void> {
  await browser.execute(() => {
    (document.activeElement as HTMLElement | null)?.blur?.();
    document.body.focus();
  });
}

/**
 * Returns the centre of the canvas element in WebDriver page coordinates.
 * Used as the anchor for wheel events and pointer drags so the gesture
 * lands inside the viewport regardless of layout.
 */
async function getCanvasCenter(): Promise<{ x: number; y: number }> {
  return browser.execute(() => {
    const canvas = document.querySelector(
      'canvas[data-testid="canvas-viewport"]',
    );
    const rect = canvas?.getBoundingClientRect();
    if (!rect) return { x: 0, y: 0 };
    return {
      x: Math.round(rect.left + rect.width / 2),
      y: Math.round(rect.top + rect.height / 2),
    };
  });
}

/** Reads the debug-surface pixel-grid flag without a typed accessor. */
async function getShowPixelGrid(): Promise<boolean> {
  return browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: { getShowPixelGrid(): boolean };
    };
    return w.__pixhaus_debug__.getShowPixelGrid();
  });
}

/** Reads the debug-surface tile-grid flag without a typed accessor. */
async function getShowTileGrid(): Promise<boolean> {
  return browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: { getShowTileGrid(): boolean };
    };
    return w.__pixhaus_debug__.getShowTileGrid();
  });
}

/** Reads the onion-skin enabled flag from the debug surface. */
async function getOnionSkinEnabled(): Promise<boolean> {
  return browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: {
        getOnionSkin(): { enabled: boolean };
      };
    };
    return w.__pixhaus_debug__.getOnionSkin().enabled;
  });
}

/**
 * Sends Ctrl+K and waits until the palette open/closed state matches
 * `expectOpen`. Lifted from cmd.e2e.ts so this spec can dispatch palette
 * commands without sharing an import-level helper.
 */
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
    async () => {
      const open = await browser.execute(() => {
        const w = window as unknown as {
          __pixhaus_debug__: { isCommandPaletteOpen(): boolean };
        };
        return w.__pixhaus_debug__.isCommandPaletteOpen();
      });
      return open === expectOpen;
    },
    {
      timeout: 5000,
      timeoutMsg: `palette never reached open=${String(expectOpen)}`,
    },
  );
}

/** Opens the palette, types `query`, and presses Enter to dispatch. */
async function dispatchViaPalette(query: string): Promise<void> {
  await toggleCommandPalette(true);
  const input = await $(byTestId(testid.commandPalette.input));
  await input.waitForDisplayed({ timeout: 5000 });
  await input.setValue(query);
  await browser.keys(["Enter"]);
  await browser.waitUntil(
    async () => {
      const open = await browser.execute(() => {
        const w = window as unknown as {
          __pixhaus_debug__: { isCommandPaletteOpen(): boolean };
        };
        return w.__pixhaus_debug__.isCommandPaletteOpen();
      });
      return !open;
    },
    {
      timeout: 5000,
      timeoutMsg: "palette did not close after Enter",
    },
  );
}

describe("Canvas viewport (manual-test-guide §4)", () => {
  // TODO(phase-2-followup): WebDriver action chain with keyDown(Space) +
  // pointer drag doesn't trigger the panSpace handler under tauri-driver.
  // Pan via middle-mouse (T-canvas-002) covers the same code path.
  it.skip("T-canvas-001: Spacebar + drag pans the canvas", async () => {
    await openProject();
    await focusBody();
    const initial = await getScroll();
    const center = await getCanvasCenter();
    await clearIpcLog();

    // Spacebar+drag: hold Space, press the left button, move, release the
    // button, release Space. The IPC for canvas_set_viewport is debounced
    // — we only assert the state changed, per the manual guide note.
    await browser.action("key").down(" ").perform();

    await browser
      .action("pointer", { parameters: { pointerType: "mouse" } })
      .move({ x: center.x, y: center.y })
      .down({ button: 0 })
      .move({ x: center.x + 30, y: center.y + 20, duration: 32 })
      .move({ x: center.x + 60, y: center.y + 40, duration: 32 })
      .move({ x: center.x + 90, y: center.y + 60, duration: 32 })
      .up({ button: 0 })
      .perform();

    await browser.action("key").up(" ").perform();

    await browser.waitUntil(
      async () => {
        const now = await getScroll();
        return now.x !== initial.x || now.y !== initial.y;
      },
      {
        timeout: 5000,
        timeoutMsg: `scroll never moved off ${JSON.stringify(initial)}`,
      },
    );
  });

  it("T-canvas-002: Middle-mouse drag pans", async () => {
    await openProject();
    await focusBody();
    const initial = await getScroll();
    const center = await getCanvasCenter();

    // Same idea as T-canvas-001 but with the middle button (button: 1)
    // and no spacebar modifier. The viewport handler accepts either path.
    await browser
      .action("pointer", { parameters: { pointerType: "mouse" } })
      .move({ x: center.x, y: center.y })
      .down({ button: 1 })
      .move({ x: center.x + 30, y: center.y + 20, duration: 32 })
      .move({ x: center.x + 60, y: center.y + 40, duration: 32 })
      .move({ x: center.x + 90, y: center.y + 60, duration: 32 })
      .up({ button: 1 })
      .perform();

    await browser.waitUntil(
      async () => {
        const now = await getScroll();
        return now.x !== initial.x || now.y !== initial.y;
      },
      {
        timeout: 5000,
        timeoutMsg: `scroll never moved off ${JSON.stringify(initial)}`,
      },
    );
  });

  // TODO(phase-2-followup): synthetic WheelEvent dispatched via execute
  // doesn't propagate through the canvas wheel handler under tauri-driver.
  it.skip("T-canvas-003: Wheel scroll zooms (smooth)", async () => {
    await openProject();
    await focusBody();
    const initial = await getZoom();
    const center = await getCanvasCenter();

    // WDIO 9's browser.action('wheel') is unreliable across drivers
    // (tauri-driver/WebDriverIO mapping doesn't always honour deltaY).
    // Dispatching a synthetic WheelEvent on the canvas exercises the
    // same handler the renderer attaches.
    await browser.execute(
      (cx: number, cy: number) => {
        const canvas = document.querySelector(
          'canvas[data-testid="canvas-viewport"]',
        ) as HTMLCanvasElement | null;
        if (!canvas) return;
        const evt = new WheelEvent("wheel", {
          deltaY: -100,
          clientX: cx,
          clientY: cy,
          bubbles: true,
          cancelable: true,
        });
        canvas.dispatchEvent(evt);
      },
      center.x,
      center.y,
    );

    await browser.waitUntil(async () => (await getZoom()) > initial, {
      timeout: 5000,
      timeoutMsg: `zoom never increased above ${initial}`,
    });
  });

  // TODO(phase-2-followup): same root cause as T-canvas-003.
  it.skip("T-canvas-004: Ctrl+wheel snaps zoom to power-of-2 levels", async () => {
    await openProject();
    await focusBody();
    const center = await getCanvasCenter();

    // Snap-zoom resolves to a power-of-2 step, so dispatch one wheel
    // tick with ctrlKey=true and assert the new zoom is in {2, 4, 8, ...}.
    // The starting zoom isn't pinned to 1 (Fit may have run during boot),
    // so assert "is a power of 2 and increased" rather than "==2".
    const initial = await getZoom();

    await browser.execute(
      (cx: number, cy: number) => {
        const canvas = document.querySelector(
          'canvas[data-testid="canvas-viewport"]',
        ) as HTMLCanvasElement | null;
        if (!canvas) return;
        const evt = new WheelEvent("wheel", {
          deltaY: -100,
          ctrlKey: true,
          clientX: cx,
          clientY: cy,
          bubbles: true,
          cancelable: true,
        });
        canvas.dispatchEvent(evt);
      },
      center.x,
      center.y,
    );

    await browser.waitUntil(
      async () => {
        const z = await getZoom();
        if (z <= initial) return false;
        // Power of 2: log2(z) is an integer (within float tolerance).
        const log2 = Math.log2(z);
        return Math.abs(log2 - Math.round(log2)) < 1e-6;
      },
      {
        timeout: 5000,
        timeoutMsg: "zoom never snapped to a power-of-2 step above initial",
      },
    );
  });

  // TODO(phase-1-followup): Ctrl+= delivery is broken in webdriverio under
  // tauri-driver — see keys.e2e.ts T-keys-010 (skipped) and T-keys-012
  // (skipped, depends on T-keys-010). Ctrl+- and Ctrl+1 are already covered
  // by keys.e2e.ts T-keys-011 and T-keys-013, so re-running them here would
  // duplicate work. Unblock together once the key-mapping issue is resolved.
  it.skip("T-canvas-005: Keyboard zoom shortcuts", async () => {
    // Ctrl+= / Ctrl+0 / Ctrl+1 / Ctrl+- — already exercised by keys.e2e.ts
    // for the chords that work; the rest are blocked on T-keys-010.
  });

  // TODO(phase-1-followup): horizontal pan via Shift+wheel can't be
  // observed reliably yet. The canvas wheel handler treats shiftKey as
  // a horizontal-pan modifier and updates scrollX through the same path
  // as drag, but the synthetic WheelEvent in WebKit/Servo (tauri-driver)
  // doesn't always reach the listener with the modifier flag intact.
  // Revisit when the wheel-event path stabilises across drivers.
  it.skip("T-canvas-006: Shift+wheel pans horizontally", async () => {
    // Synthesise WheelEvent with shiftKey:true and assert scroll.x changed
    // while scroll.y stayed put. Skipped until the synthetic-event delivery
    // path is reliable.
  });

  it("T-canvas-007: Toggle pixel grid", async () => {
    await openProject();
    const before = await getShowPixelGrid();

    await dispatchViaPalette("toggle pixel grid");

    await browser.waitUntil(async () => (await getShowPixelGrid()) !== before, {
      timeout: 5000,
      timeoutMsg: `showPixelGrid never flipped from ${before}`,
    });
  });

  it("T-canvas-008: Toggle tile grid", async () => {
    await openProject();
    await focusBody();
    const before = await getShowTileGrid();

    // Ctrl+G is bound to view:toggle-grid (see ui/src/keybinds/defaults.ts);
    // the same command is reachable via the palette as "Toggle Grid".
    await browser.keys(["Control", "g"]);

    await browser.waitUntil(async () => (await getShowTileGrid()) !== before, {
      timeout: 5000,
      timeoutMsg: `showTileGrid never flipped from ${before}`,
    });
  });

  it("T-canvas-009: Onion skin toggle", async () => {
    await openProject();
    await focusBody();
    const initial = await getOnionSkinEnabled();
    await dispatchViaPalette("toggle onion skin");
    await browser.waitUntil(
      async () => (await getOnionSkinEnabled()) !== initial,
      {
        timeout: 5000,
        timeoutMsg: "onion skin never toggled",
      },
    );
  });
});
