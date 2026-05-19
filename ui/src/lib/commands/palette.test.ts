// Contract tests for palette IPC wrappers.
//
// Covers the S54 page CRUD commands and asserts payload shapes match the
// snake_case argument names the Rust side expects.

import { describe, expect, it, vi, beforeEach } from "vitest";

const invokeMock = vi.fn();

vi.mock("../ipc", () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import {
  palettePageAdd,
  palettePageRemove,
  palettePageRename,
  palettePageSetEntries,
} from "./palette";

describe("palette page commands", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("palettePageAdd forwards name in the IPC payload", async () => {
    invokeMock.mockResolvedValueOnce(7);
    const newId = await palettePageAdd(1 as never, 2 as never, "warm");
    expect(newId).toBe(7);
    expect(invokeMock).toHaveBeenCalledWith("palette_page_add", {
      sprite_id: 1,
      palette_id: 2,
      name: "warm",
    });
  });

  it("palettePageRemove forwards the page id", async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    await palettePageRemove(1 as never, 2 as never, 3 as never);
    expect(invokeMock).toHaveBeenCalledWith("palette_page_remove", {
      sprite_id: 1,
      palette_id: 2,
      page_id: 3,
    });
  });

  it("palettePageRename uses snake_case new_name", async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    await palettePageRename(1 as never, 2 as never, 3 as never, "cool");
    expect(invokeMock).toHaveBeenCalledWith("palette_page_rename", {
      sprite_id: 1,
      palette_id: 2,
      page_id: 3,
      new_name: "cool",
    });
  });

  it("palettePageSetEntries forwards the index array", async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    await palettePageSetEntries(1 as never, 2 as never, 3 as never, [0, 4, 7]);
    expect(invokeMock).toHaveBeenCalledWith("palette_page_set_entries", {
      sprite_id: 1,
      palette_id: 2,
      page_id: 3,
      entry_indices: [0, 4, 7],
    });
  });
});
