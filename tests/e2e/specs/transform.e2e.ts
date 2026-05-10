// Transform e2e — covers manual-test-guide section 6 (T-transform-001..004).
//
// Each test boots the app, opens a fresh project via the welcome screen
// "New project" button, waits for activeLayerId to populate (the
// transform dispatchers return early when there is no active layer —
// transform-input.ts line 278), clears the IPC log, and dispatches the
// transform via the command palette.
//
// The assertion shape is the same for every test: canvas_transform fires
// with `{ args: { ops: [{ kind: "<op>" }] } }`. The wrapped invoke
// double-wraps args, so drill twice into entry.args.args.ops[0].kind.

import { $, browser, expect } from "@wdio/globals";
import { bootApp } from "../helpers/app.js";
import { byTestId, testid } from "../helpers/selectors.js";
import {
  getActiveLayerId,
  getActiveProject,
  isCommandPaletteOpen,
} from "../helpers/state.js";
import { clearIpcLog, waitForIpc } from "../helpers/ipc.js";

/**
 * Boots the app and clicks the welcome "New project" button, waiting until
 * an active project is registered. Mirrors keys.e2e.ts so each spec is
 * self-contained.
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
 * ignores chords originating from <input>/<textarea>/contenteditable)
 * sees the next browser.keys() call.
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
 * top-ranked match. The palette closes itself on dispatch — wait for that
 * so the next palette open sees a clean state.
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
 * Boots, opens a project, waits for activeLayerId to populate, and
 * clears the IPC log. Returns true if the active layer was ready in time
 * and false if the test should bail (mirroring cmd.e2e.ts T-cmd-003f's
 * skip rationale — activeLayerId can stay null right after
 * createNewProject, in which case the dispatcher returns early without
 * firing canvas_transform).
 */
async function setupForTransform(): Promise<boolean> {
  await openNewProjectViaButton();
  try {
    await browser.waitUntil(async () => (await getActiveLayerId()) !== null, {
      timeout: 5000,
      timeoutMsg: "activeLayerId never populated after createNewProject",
    });
  } catch {
    return false;
  }
  await clearIpcLog();
  return true;
}

/**
 * Reads the kind of the first op on the most recent canvas_transform IPC.
 * canvasTransform passes `{ args }` to invoke, so the wrapped log entry
 * stores `entry.args = { args: { ..., ops: [{kind: ...}] } }`. Drill twice.
 */
function firstOpKind(entry: { args: unknown } | undefined): string | undefined {
  const outer = entry?.args as
    | { args?: { ops?: { kind?: string }[] } }
    | undefined;
  return outer?.args?.ops?.[0]?.kind;
}

describe("Transform (manual-test-guide §6)", () => {
  it("T-transform-001: Flip horizontal dispatches canvas_transform with flip_horizontal", async () => {
    const ready = await setupForTransform();
    if (!ready) return;

    await dispatchViaPalette("flip horizontal");

    const entries = await waitForIpc("canvas_transform", 1, 10000);
    await expect(entries.length).toBeGreaterThan(0);
    await expect(firstOpKind(entries[0])).toBe("flip_horizontal");
  });

  it("T-transform-002: Flip vertical dispatches canvas_transform with flip_vertical", async () => {
    const ready = await setupForTransform();
    if (!ready) return;

    await dispatchViaPalette("flip vertical");

    const entries = await waitForIpc("canvas_transform", 1, 10000);
    await expect(entries.length).toBeGreaterThan(0);
    await expect(firstOpKind(entries[0])).toBe("flip_vertical");
  });

  it("T-transform-003: Rotate 90 CW dispatches canvas_transform with rotate90_cw", async () => {
    const ready = await setupForTransform();
    if (!ready) return;

    // Registry label is "Rotate 90° Clockwise"; "rotate clockwise" matches
    // via the keywords array (["rotate","clockwise","cw","90"]) and is
    // safer than typing the degree glyph through WebDriver.
    await dispatchViaPalette("rotate clockwise");

    const entries = await waitForIpc("canvas_transform", 1, 10000);
    await expect(entries.length).toBeGreaterThan(0);
    await expect(firstOpKind(entries[0])).toBe("rotate90_cw");
  });

  it("T-transform-004: Rotate 90 CCW dispatches canvas_transform with rotate90_ccw", async () => {
    const ready = await setupForTransform();
    if (!ready) return;

    // Registry keywords include "counter" and "ccw" — "rotate counter"
    // disambiguates from the CW command without needing the degree glyph.
    await dispatchViaPalette("rotate counter");

    const entries = await waitForIpc("canvas_transform", 1, 10000);
    await expect(entries.length).toBeGreaterThan(0);
    await expect(firstOpKind(entries[0])).toBe("rotate90_ccw");
  });
});
