// Crash-reporting IPC wrappers.
//
// The setter (`crash_reporting_set_enabled`) is invoked inline from
// `ui/src/crash-reporting/crash-reporting.ts`; only the getter lives here
// because it's the source of truth for the Preferences > Privacy toggle's
// initial state on app start.

import { invoke } from "../ipc";

/** Returns the backend's current crash-reporting gate state. */
export function crashReportingGetEnabled(): Promise<boolean> {
  return invoke<boolean>("crash_reporting_get_enabled");
}
