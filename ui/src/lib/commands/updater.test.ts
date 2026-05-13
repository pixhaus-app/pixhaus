// Contract tests for the updater IPC wrappers.
//
// Asserts the IPC command names and argument shapes so a future signature
// drift on the Rust side trips here instead of 404'ing at runtime, and
// pins the `null` return path that distinguishes "no update" from
// "update available" in updater_check.

import { describe, expect, it, vi } from "vitest";
import type { UpdateInfo } from "../types";

const invokeMock = vi.fn();

// Mock the module-level wrapper rather than @tauri-apps/api/core: the
// wrapper logs to `window.__pixhaus_ipc_log__`, which doesn't exist in
// the node-flavoured test environment configured in `vitest.config.ts`.
vi.mock("../ipc", () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { updaterCheck, updaterInstall } from "./updater";

describe("updaterCheck", () => {
  it("invokes updater_check with no arguments and returns the UpdateInfo", async () => {
    const info: UpdateInfo = {
      version: "0.2.0",
      body: "Fixes a thing.",
      date: "2026-05-01T00:00:00Z",
    };
    invokeMock.mockResolvedValueOnce(info);

    const result = await updaterCheck();

    expect(invokeMock).toHaveBeenCalledWith("updater_check", undefined);
    expect(result).toEqual(info);
  });

  it("returns null when the running build is already the latest", async () => {
    invokeMock.mockResolvedValueOnce(null);

    const result = await updaterCheck();

    expect(invokeMock).toHaveBeenCalledWith("updater_check", undefined);
    expect(result).toBeNull();
  });
});

describe("updaterInstall", () => {
  it("invokes updater_install with no arguments and resolves to undefined", async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    const result = await updaterInstall();

    expect(invokeMock).toHaveBeenCalledWith("updater_install", undefined);
    expect(result).toBeUndefined();
  });
});
