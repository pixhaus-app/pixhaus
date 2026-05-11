// Crash-reporting bootstrap for the JavaScript layer.
//
// `@sentry/browser` is only initialized when `VITE_SENTRY_DSN` is set at
// build time. In development (or in forks built without a DSN) the module
// is a no-op: all exported functions still exist so call sites compile
// identically across configurations.
//
// The enabled gate is separate from initialization: the Sentry client is
// initialized once and a `beforeSend` callback drops events when the user
// has not opted in.

import * as Sentry from "@sentry/browser";
import { invoke } from "../lib/ipc";

// DSN injected at build time via Vite; undefined in dev or fork builds.
const DSN = import.meta.env["VITE_SENTRY_DSN"] as string | undefined;

let jsEnabled = false;

/**
 * Initialise Sentry for the JS layer.
 *
 * Call once at app startup, passing the user's stored preference and an
 * anonymous stable UID. Has no effect when no DSN is compiled in.
 */
export function initCrashReporting(options: { enabled: boolean; uid: string }): void {
  if (!DSN) return;

  jsEnabled = options.enabled;

  const release = import.meta.env["VITE_APP_VERSION"] as string | undefined;

  Sentry.init({
    dsn: DSN,
    ...(release !== undefined ? { release } : {}),
    environment: import.meta.env.MODE,
    // No performance tracing — panics and unhandled errors only.
    tracesSampleRate: 0,
    sendDefaultPii: false,
    // Sentry 10 dropped autoSessionTracking. Drop the default session
    // integration so release-health pings still don't fire without
    // user consent. beforeSend covers error events.
    integrations: (defaultIntegrations) =>
      defaultIntegrations.filter((i) => i.name !== "BrowserSession"),
    beforeSend: (event) => (jsEnabled ? event : null),
  });

  Sentry.setUser({ id: options.uid });
}

/**
 * Update the JS-layer enabled gate.
 *
 * Also calls the Rust-side IPC command so both layers stay in sync.
 * Safe to call before `initCrashReporting`.
 */
export function setCrashReportingEnabled(enabled: boolean): void {
  jsEnabled = enabled;
  invoke("crash_reporting_set_enabled", { enabled }).catch((err: unknown) => {
    console.error("[pixhaus] crash_reporting_set_enabled failed:", err);
  });
}
