// Smoke spec — proves the e2e harness boots end-to-end.
//
// If this passes:
//   - pnpm tauri:build:debug produces a binary tauri-driver can launch
//   - the Solid bundle renders FirstLaunchDialog + WelcomeScreen
//   - data-testid selectors hit the right elements
//   - window.__pixhaus_debug__ is available
//   - the IPC tap captures crash-reporting roundtrips
//
// Phase 1+ specs build on the same harness; if smoke fails, they all do.

import { $, expect } from "@wdio/globals";
import { bootApp } from "../helpers/app.js";
import { byTestId, testid } from "../helpers/selectors.js";
import {
  getActiveProject,
  getCrashReportingDialogShown,
  getCrashReportingEnabled,
} from "../helpers/state.js";
import { findIpcByCmd } from "../helpers/ipc.js";

describe("Smoke (Phase 0 harness)", () => {
  it("boots, exposes the debug surface, and renders the welcome screen after dialog dismiss", async () => {
    await bootApp();

    // Welcome screen up, no project active.
    const welcome = await $(byTestId(testid.welcome.root));
    await expect(welcome).toBeDisplayed();
    await expect(await getActiveProject()).toBeNull();

    // First-launch dialog dismissed (declined): localStorage flag flipped.
    await expect(await getCrashReportingDialogShown()).toBe(true);
    await expect(await getCrashReportingEnabled()).toBe(false);
  });

  it("captured at least one crash_reporting_set_enabled IPC during boot", async () => {
    await bootApp();

    // The decline path calls setCrashReportingEnabled(false) which fires
    // the IPC. If the tap is wired correctly, the log has it.
    const entries = await findIpcByCmd("crash_reporting_set_enabled");
    await expect(entries.length).toBeGreaterThan(0);
  });
});
