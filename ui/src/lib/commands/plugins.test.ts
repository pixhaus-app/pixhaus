// Contract tests for the plugin management IPC wrappers.
//
// Asserts the command name and argument shape for each wrapper. A
// signature drift on the Rust side (where Tauri routes args by their
// snake_case keys) would otherwise 404 silently at runtime.

import { describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

// Mock the module-level wrapper rather than @tauri-apps/api/core: the
// wrapper logs to `window.__pixhaus_ipc_log__`, which doesn't exist in
// the node-flavoured test environment configured in `vitest.config.ts`.
vi.mock("../ipc", () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { pluginList, pluginReload, pluginScan, pluginUnload, type PluginInfo } from "./plugins";

describe("pluginList", () => {
  it("invokes plugin_list with no arguments and returns the array", async () => {
    const fake: PluginInfo[] = [
      {
        name: "echo",
        version: "0.1.0",
        description: "Echo verb for testing",
        dir: "/home/user/.pixhaus/plugins/echo",
        verb_count: 1,
      },
    ];
    invokeMock.mockResolvedValueOnce(fake);

    const result = await pluginList();

    expect(invokeMock).toHaveBeenCalledWith("plugin_list", undefined);
    expect(result).toEqual(fake);
  });
});

describe("pluginScan", () => {
  it("invokes plugin_scan with no arguments and returns the count", async () => {
    invokeMock.mockResolvedValueOnce(3);

    const result = await pluginScan();

    expect(invokeMock).toHaveBeenCalledWith("plugin_scan", undefined);
    expect(result).toBe(3);
  });
});

describe("pluginReload", () => {
  it("sends the plugin name to plugin_reload", async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    await pluginReload("echo");

    expect(invokeMock).toHaveBeenCalledWith("plugin_reload", { name: "echo" });
  });
});

describe("pluginUnload", () => {
  it("sends the plugin name to plugin_unload", async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    await pluginUnload("echo");

    expect(invokeMock).toHaveBeenCalledWith("plugin_unload", { name: "echo" });
  });
});
