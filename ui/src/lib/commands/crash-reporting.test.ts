// Contract test for crashReportingGetEnabled.
//
// Asserts the command name and argument shape so a future signature
// drift on the Rust side (Tauri routes args by their snake_case keys)
// trips here instead of 404'ing at runtime.

import { describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

// Mock the module-level wrapper rather than @tauri-apps/api/core: the
// wrapper logs to `window.__pixhaus_ipc_log__`, which doesn't exist in
// the node-flavoured test environment configured in `vitest.config.ts`.
vi.mock("../ipc", () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { crashReportingGetEnabled } from "./crash-reporting";

describe("crashReportingGetEnabled", () => {
  it("invokes crash_reporting_get_enabled with no arguments and returns the boolean", async () => {
    invokeMock.mockResolvedValueOnce(true);

    const result = await crashReportingGetEnabled();

    expect(invokeMock).toHaveBeenCalledWith("crash_reporting_get_enabled", undefined);
    expect(result).toBe(true);
  });
});
