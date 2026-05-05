import type { Page } from "@playwright/test";

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

export const TAURI_MOCK_SCRIPT = `
(() => {
  let nextId = 1;
  const overrides = Object.create(null);

  window.__PIXHAUS_MOCK__ = {
    setResponse(cmd, val) { overrides[cmd] = val; },
  };

  window.__TAURI_INTERNALS__ = {
    invoke: async (cmd, args) => {
      if (Object.prototype.hasOwnProperty.call(overrides, cmd)) {
        const val = overrides[cmd];
        return typeof val === 'function' ? val(args) : val;
      }
      // Event plugin — return a numeric event ID so listen() resolves cleanly.
      if (cmd === 'plugin:event|listen')   return nextId++;
      if (cmd === 'plugin:event|unlisten') return null;
      if (cmd === 'plugin:event|emit')     return null;
      // All other commands: log and return null rather than throwing.
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
