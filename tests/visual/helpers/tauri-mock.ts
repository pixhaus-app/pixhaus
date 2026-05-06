import type { Page } from "@playwright/test";

// ── Seed data ─────────────────────────────────────────────────────────────────
//
// Shared between the mock script (injected into the page) and test files that
// want to reference the same IDs when setting up additional overrides.

/** Sprite ID used in the default mock sprite. */
export const MOCK_SPRITE_ID = 1;

/** Layer ID used in the default mock layer. */
export const MOCK_LAYER_ID = 1;

// ── Base mock script ──────────────────────────────────────────────────────────
//
// Injected via page.addInitScript before every visual test. Sets up
// window.__TAURI_INTERNALS__ so @tauri-apps/api doesn't throw when it
// initialises inside a plain Chromium page (no Tauri webview host).
//
// The mock exposes window.__PIXHAUS_MOCK__.setResponse(cmd, value) so
// individual tests can override specific IPC commands before page.goto().
//
// How Tauri v2 IPC works:
//   invoke(cmd, args) → window.__TAURI_INTERNALS__.invoke(cmd, args)
//   listen(event, handler) → transformCallback(handler) + invoke('plugin:event|listen')
//   The backend fires events by calling window[callbackId](payload).
//
// Default responses mirror what the Rust backend returns for a freshly-
// created project containing one 32×32 RGBA sprite with one frame. The
// sprite_list response carries an empty `layers` array — the canvas only
// needs the sprite outline to draw the checkerboard background, and
// layer_list is what the layer panel reads. Tests can override any
// command by calling mockInvokeResponse() after injectTauriMock() and
// before page.goto().

export const TAURI_MOCK_SCRIPT = `
(() => {
  let nextId = 1;
  const overrides = Object.create(null);

  // Default responses for commands issued during normal canvas startup.
  // Keyed by command name; values are functions receiving the args object.
  const DEFAULTS = {
    sprite_list: () => [{
      id: 1,
      name: 'Untitled',
      canvas: { width: 32, height: 32 },
      color_mode: 'rgba',
      layers: [],
      frames: [{ duration_ms: 100 }],
      cels: [],
      palettes: [],
      tilesets: [],
      frame_tags: [],
      animations: [],
      slices: [],
    }],
    canvas_composite: () => ({
      sprite_width: 32,
      sprite_height: 32,
      tile_size: 64,
      tiles_x: 1,
      tiles_y: 1,
    }),
    // Echo the viewport state back — the command is a fire-and-forget sync.
    canvas_set_viewport: (args) => args && args.canvas ? args.canvas : null,
    layer_list: () => [{
      id: 1,
      name: 'Layer 1',
      kind: { kind: 'raster' },
      blend_mode: 'normal',
      opacity: 255,
      visible: true,
      locked: false,
    }],
    frame_list: () => [{ duration_ms: 100 }],
    canvas_set_selection: () => ({ region: null, anchor_layer: null }),
    project_get: () => null,
    project_save: () => null,
    project_close: () => null,
  };

  window.__PIXHAUS_MOCK__ = {
    setResponse(cmd, val) { overrides[cmd] = val; },
  };

  window.__TAURI_INTERNALS__ = {
    invoke: async (cmd, args) => {
      // Per-test override takes priority over defaults.
      if (Object.prototype.hasOwnProperty.call(overrides, cmd)) {
        const val = overrides[cmd];
        return typeof val === 'function' ? val(args) : val;
      }
      // Event plugin — return a numeric event ID so listen() resolves cleanly.
      if (cmd === 'plugin:event|listen')   return nextId++;
      if (cmd === 'plugin:event|unlisten') return null;
      if (cmd === 'plugin:event|emit')     return null;
      // Built-in defaults for standard canvas commands.
      if (Object.prototype.hasOwnProperty.call(DEFAULTS, cmd)) {
        const fn = DEFAULTS[cmd];
        return fn(args);
      }
      // Unrecognised: log and return null rather than throwing so the app
      // degrades gracefully and the test can observe the warning.
      console.warn('[tauri-mock] unhandled invoke:', cmd, args);
      return null;
    },

    // Registers a callback so the (mocked) backend can fire events.
    // The backend calls window[id](payload); tests can do the same.
    transformCallback: (cb, once) => {
      const id = nextId++;
      window[String(id)] = (payload) => {
        cb(payload);
        if (once) delete window[String(id)];
      };
      return id;
    },

    metadata: { debug: false, version: '0.0.0-visual-test' },
  };

  // Plugin namespace — plugins check for window.__TAURI__.<plugin>.
  window.__TAURI__ = {};

  // Suppress the first-launch crash-reporting consent dialog so it
  // doesn't intercept pointer events in tests. The Shell renders the
  // dialog whenever localStorage 'pixhaus:crash-reporting-dialog-shown'
  // is unset, and Playwright runs each test in a fresh context with
  // empty storage. Pre-seeding the flag matches what a returning user
  // sees and keeps the visual tests focused on the actual editor UI.
  try {
    localStorage.setItem('pixhaus:crash-reporting-dialog-shown', '1');
  } catch (_err) {
    // localStorage may be unavailable in some test environments;
    // dialog suppression is best-effort here.
  }
})();
`;

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Injects the Tauri mock into every script context on the page. Must be
 *  called before page.goto() so the mock is in place when app scripts run. */
export async function injectTauriMock(page: Page): Promise<void> {
  await page.addInitScript(TAURI_MOCK_SCRIPT);
}

/** Configures a named IPC command to return a fixed value when invoked.
 *
 *  The response is structured-cloned into the page context, so it must be
 *  serialisable — primitives, plain objects, arrays, typed arrays. Functions
 *  cannot cross the boundary; the inner overrides table supports a factory
 *  callback, but it can only be set from inside the page (not via this
 *  helper from the test runner).
 *
 *  Must be called after injectTauriMock() and before page.goto(). */
export async function mockInvokeResponse(
  page: Page,
  cmd: string,
  response: unknown,
): Promise<void> {
  // addInitScript with a serialisable argument: the function runs in the
  // page context and receives a deep-cloned copy of the arg object.
  await page.addInitScript(
    ({ c, r }: { c: string; r: unknown }) => {
      (
        window as unknown as {
          __PIXHAUS_MOCK__: {
            setResponse: (cmd: string, val: unknown) => void;
          };
        }
      ).__PIXHAUS_MOCK__.setResponse(c, r);
    },
    { c: cmd, r: response },
  );
}
