// Smoke tests for the command-palette dispatcher.
//
// The interesting behaviour is: dispatching a command id routes to a
// real IPC call, mutates the right Solid signal, and doesn't fall
// through to the unknown-command branch. Mock @tauri-apps/api/core to
// observe IPC calls without spinning up a Tauri process.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// preferences-store and project-state both touch localStorage and
// document at module load. Vitest's node environment provides
// neither, so polyfill minimal shims BEFORE any module imports
// resolve. vi.hoisted runs before the import section so the polyfills
// are in place when the registry transitively pulls in preferences-store.
//
// jsdom would be cleaner but isn't currently a dev dep, and the surface
// the registry actually touches is small enough to fake by hand.
vi.hoisted(() => {
  const g = globalThis as unknown as {
    localStorage?: Storage;
    document?: { documentElement: { dataset: Record<string, string> }; querySelector: () => null };
    window?: { open: (...args: unknown[]) => null };
  };
  const store = new Map<string, string>();
  g.localStorage = {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => {
      store.set(k, v);
    },
    removeItem: (k: string) => {
      store.delete(k);
    },
    clear: () => store.clear(),
    key: (i: number) => Array.from(store.keys())[i] ?? null,
    get length() {
      return store.size;
    },
  };
  g.document = {
    documentElement: { dataset: {} },
    querySelector: () => null,
  };
  g.window = { open: () => null };
});

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

// The dialog plugin pulls in the IPC layer; stub the surface used by
// the registry so the tests don't depend on a Tauri host.
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
  message: vi.fn().mockResolvedValue("Ok"),
  confirm: vi.fn().mockResolvedValue(true),
  ask: vi.fn().mockResolvedValue(true),
}));

import { dispatchCommand, getAllCommands } from "./command-registry";
import {
  zoom,
  setZoom,
  showTileGrid,
  setShowTileGrid,
  showPixelGrid,
  setShowPixelGrid,
  setActiveSpriteId,
  setActiveFrameIndex,
} from "../canvas/canvas-state";
import {
  isPalettePanelVisible,
  setPalettePanelVisible,
  isTilemapPanelVisible,
  setTilemapPanelVisible,
} from "../shell/panel-state";
import { setActiveProject } from "../project-state";

const FAKE_PROJECT = {
  metadata: { name: "Test", version: "0.0.0" },
  path: null,
  dirty: false,
  sprite_count: 1,
} as unknown as Parameters<typeof setActiveProject>[0];

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(undefined);
  setActiveSpriteId(1);
  setActiveFrameIndex(0);
  setZoom(1);
  setShowTileGrid(false);
  setShowPixelGrid(true);
  setPalettePanelVisible(true);
  setTilemapPanelVisible(true);
  setActiveProject(FAKE_PROJECT);
});

afterEach(() => {
  setActiveSpriteId(null);
  setActiveProject(null);
});

describe("getAllCommands", () => {
  it("includes every implemented entry but excludes the deferred clipboard verbs", () => {
    const ids = new Set(getAllCommands().map((c) => c.id));
    expect(ids.has("edit:undo")).toBe(true);
    expect(ids.has("edit:redo")).toBe(true);
    expect(ids.has("edit:select-all")).toBe(true);
    expect(ids.has("edit:deselect")).toBe(true);
    expect(ids.has("view:zoom-in")).toBe(true);
    expect(ids.has("help:about")).toBe(true);
    // Deferred — clipboard pipeline not wired yet.
    expect(ids.has("edit:cut")).toBe(false);
    expect(ids.has("edit:copy")).toBe(false);
    expect(ids.has("edit:paste")).toBe(false);
  });
});

describe("dispatchCommand — edit", () => {
  it("edit:undo invokes the `undo` IPC", () => {
    dispatchCommand("edit:undo");
    expect(invokeMock).toHaveBeenCalledWith("undo", undefined);
  });

  it("edit:redo invokes the `redo` IPC", () => {
    dispatchCommand("edit:redo");
    expect(invokeMock).toHaveBeenCalledWith("redo", undefined);
  });

  it("edit:select-all invokes canvas_select_all with the active sprite id", () => {
    dispatchCommand("edit:select-all");
    expect(invokeMock).toHaveBeenCalledWith("canvas_select_all", { sprite_id: 1 });
  });

  it("edit:deselect invokes canvas_set_selection with null region", () => {
    dispatchCommand("edit:deselect");
    expect(invokeMock).toHaveBeenCalledWith("canvas_set_selection", {
      region: null,
      anchor_layer: null,
    });
  });
});

describe("dispatchCommand — frame", () => {
  it("frame:new invokes frame_add for the active sprite", () => {
    dispatchCommand("frame:new");
    expect(invokeMock).toHaveBeenCalledWith("frame_add", { sprite_id: 1, duration_ms: 100 });
  });

  it("frame:delete invokes frame_delete with the active frame index", () => {
    setActiveFrameIndex(3);
    dispatchCommand("frame:delete");
    expect(invokeMock).toHaveBeenCalledWith("frame_delete", { sprite_id: 1, frame_index: 3 });
  });

  it("frame:duplicate invokes frame_duplicate with the active frame index", () => {
    setActiveFrameIndex(2);
    dispatchCommand("frame:duplicate");
    expect(invokeMock).toHaveBeenCalledWith("frame_duplicate", { sprite_id: 1, frame_index: 2 });
  });

  it("frame:new is a no-op when no sprite is active", () => {
    setActiveSpriteId(null);
    dispatchCommand("frame:new");
    expect(invokeMock).not.toHaveBeenCalled();
  });
});

describe("dispatchCommand — view", () => {
  it("view:zoom-in steps to the next snap level", () => {
    setZoom(1);
    dispatchCommand("view:zoom-in");
    expect(zoom()).toBe(2);
  });

  it("view:zoom-out steps to the previous snap level", () => {
    setZoom(1);
    dispatchCommand("view:zoom-out");
    expect(zoom()).toBe(0.5);
  });

  it("view:zoom-100 sets the zoom to 1", () => {
    setZoom(4);
    dispatchCommand("view:zoom-100");
    expect(zoom()).toBe(1);
  });

  it("view:toggle-grid flips showTileGrid", () => {
    setShowTileGrid(false);
    dispatchCommand("view:toggle-grid");
    expect(showTileGrid()).toBe(true);
    dispatchCommand("view:toggle-grid");
    expect(showTileGrid()).toBe(false);
  });

  it("view:toggle-pixel-grid flips showPixelGrid", () => {
    setShowPixelGrid(true);
    dispatchCommand("view:toggle-pixel-grid");
    expect(showPixelGrid()).toBe(false);
  });
});

describe("dispatchCommand — window", () => {
  it("window:toggle-palette flips the palette-panel signal", () => {
    setPalettePanelVisible(true);
    dispatchCommand("window:toggle-palette");
    expect(isPalettePanelVisible()).toBe(false);
    dispatchCommand("window:toggle-palette");
    expect(isPalettePanelVisible()).toBe(true);
  });

  it("window:toggle-tilemap flips the tilemap-panel signal", () => {
    setTilemapPanelVisible(true);
    dispatchCommand("window:toggle-tilemap");
    expect(isTilemapPanelVisible()).toBe(false);
  });
});

describe("dispatchCommand — sprite", () => {
  it("sprite:new invokes sprite_add with default 32x32 RGBA canvas", () => {
    invokeMock.mockResolvedValue({ id: 2, canvas: { width: 32, height: 32 } });
    dispatchCommand("sprite:new");
    // sprite_add is the first call. The follow-up viewport sync may
    // not run synchronously in jsdom-less node; assert on the trigger.
    const call = invokeMock.mock.calls.find(([cmd]) => cmd === "sprite_add");
    expect(call).toBeDefined();
    expect(call?.[1]).toEqual({
      args: {
        name: "Untitled",
        canvas_width: 32,
        canvas_height: 32,
        color_mode: "rgba",
      },
    });
  });
});

describe("dispatchCommand — help", () => {
  it("help:about invokes the app_about IPC", async () => {
    invokeMock.mockResolvedValue({ name: "Pixhaus", version: "0.1.0" });
    dispatchCommand("help:about");
    // The handler chains onto a microtask; flush it.
    await new Promise((r) => setTimeout(r, 0));
    expect(invokeMock).toHaveBeenCalledWith("app_about", undefined);
  });
});

describe("dispatchCommand — unknown", () => {
  it("unknown ids are a no-op (does not invoke any IPC)", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    dispatchCommand("does-not-exist");
    expect(invokeMock).not.toHaveBeenCalled();
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });
});
