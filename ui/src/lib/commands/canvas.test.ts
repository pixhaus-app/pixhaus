// Contract tests for canvas IPC wrappers.
//
// Covers the magic-wand wrapper's gap_close pass-through (S57). The
// wrapper must forward the optional gap_close payload to the Rust side
// untouched so the snake_case keys match the GapCloseRequest struct.

import { describe, expect, it, vi, beforeEach } from "vitest";

const invokeMock = vi.fn();

// Same indirection as plugins.test.ts: mock the module-level invoke
// wrapper, not @tauri-apps/api/core, because the wrapper depends on a
// window-side log buffer that isn't present in the node-flavoured
// Vitest environment.
vi.mock("../ipc", () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { canvasSelectMagicWand, type MagicWandArgs } from "./canvas";

const baseArgs: MagicWandArgs = {
  sprite_id: 1 as MagicWandArgs["sprite_id"],
  anchor_layer: 2 as MagicWandArgs["anchor_layer"],
  seed_x: 4,
  seed_y: 5,
  tolerance: 32,
  connectivity: "four",
};

describe("canvasSelectMagicWand", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue({ region: null, anchor_layer: null });
  });

  it("invokes canvas_select_magic_wand without gap_close when undefined", async () => {
    await canvasSelectMagicWand(baseArgs);

    expect(invokeMock).toHaveBeenCalledTimes(1);
    const call = invokeMock.mock.calls[0];
    if (!call) throw new Error("invoke was not called");
    const [cmd, payload] = call;
    expect(cmd).toBe("canvas_select_magic_wand");
    expect(payload).toMatchObject({
      sprite_id: 1,
      anchor_layer: 2,
      tolerance: 32,
      connectivity: "four",
    });
    // gap_close key must be absent (not just undefined) so Rust's
    // Option<GapCloseRequest> deserialises to None cleanly.
    expect((payload as Record<string, unknown>).gap_close).toBeUndefined();
  });

  it("forwards gap_close payload through unchanged when set", async () => {
    await canvasSelectMagicWand({
      ...baseArgs,
      gap_close: { closing_distance: 12 },
    });

    const call = invokeMock.mock.calls[0];
    if (!call) throw new Error("invoke was not called");
    const [, payload] = call;
    expect((payload as { gap_close?: unknown }).gap_close).toEqual({
      closing_distance: 12,
    });
  });

  it("passes gap_close: null through so the backend reads it as None", async () => {
    await canvasSelectMagicWand({ ...baseArgs, gap_close: null });

    const call = invokeMock.mock.calls[0];
    if (!call) throw new Error("invoke was not called");
    const [, payload] = call;
    expect((payload as { gap_close?: unknown }).gap_close).toBeNull();
  });

  it("forwards every GapCloseRequest field when fully populated", async () => {
    await canvasSelectMagicWand({
      ...baseArgs,
      gap_close: {
        closing_distance: 20,
        closing_angle_rad: 1.2,
        ink_threshold: 100,
      },
    });

    const call = invokeMock.mock.calls[0];
    if (!call) throw new Error("invoke was not called");
    const [, payload] = call;
    expect((payload as { gap_close?: unknown }).gap_close).toEqual({
      closing_distance: 20,
      closing_angle_rad: 1.2,
      ink_threshold: 100,
    });
  });
});
