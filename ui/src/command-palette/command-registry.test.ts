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

// List commands that the module-level backend queries (layers, timeline,
// library) fire in the background whenever the active sprite/project is set.
// They must resolve to an array so the query fetchers don't throw on the
// default `undefined` mock return.
const LIST_COMMANDS = new Set([
  "layer_list",
  "frame_list",
  "frame_tag_list",
  "cel_list",
  "palette_list",
  "library_list_entities",
  "library_list_groups",
  "library_list_tags",
]);

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
  closeSection,
  isLibraryCollapsed,
  isSectionOpen,
  isTimelineCollapsed,
  openSection,
  resetSections,
  setLibraryCollapsed,
  setTimelineCollapsed,
} from "../shell/rail-state";
import { clearSheetEntity, setActiveSheetEntityId } from "../sheet/sheet-state";
import { setActiveProject } from "../project-state";
import { activeVerb, clearVerbCache, setActiveVerb } from "../lib/ai/verb-invoke-state";

const FAKE_PROJECT = {
  metadata: { name: "Test", version: "0.0.0" },
  path: null,
  dirty: false,
  sprite_count: 1,
} as unknown as Parameters<typeof setActiveProject>[0];

beforeEach(async () => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((cmd: string) =>
    Promise.resolve(LIST_COMMANDS.has(cmd) ? [] : undefined),
  );
  setActiveSpriteId(1);
  setActiveFrameIndex(0);
  setZoom(1);
  setShowTileGrid(false);
  setShowPixelGrid(true);
  resetSections();
  setLibraryCollapsed(false);
  setTimelineCollapsed(false);
  clearSheetEntity();
  setActiveProject(FAKE_PROJECT);
  // Setting the active sprite makes the module-level layers query (see
  // layer-state) fetch layer_list in a microtask. Let that settle and clear
  // it so each test body observes only the IPC its own dispatch triggers.
  await new Promise((resolve) => setTimeout(resolve, 0));
  invokeMock.mockClear();
});

afterEach(() => {
  setActiveSpriteId(null);
  clearSheetEntity();
  closeSection("reference");
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
  it("window:toggle-palette flips the color section", () => {
    dispatchCommand("window:toggle-palette");
    expect(isSectionOpen("color")).toBe(false);
    dispatchCommand("window:toggle-palette");
    expect(isSectionOpen("color")).toBe(true);
  });

  it("window:toggle-tilemap flips the tilemap section", () => {
    openSection("tilemap");
    dispatchCommand("window:toggle-tilemap");
    expect(isSectionOpen("tilemap")).toBe(false);
  });

  it("window:toggle-library flips the library dock collapse state", () => {
    setLibraryCollapsed(false);
    dispatchCommand("window:toggle-library");
    expect(isLibraryCollapsed()).toBe(true);
    dispatchCommand("window:toggle-library");
    expect(isLibraryCollapsed()).toBe(false);
  });

  it("window:reset-layout restores default section state and uncollapses docks", () => {
    closeSection("color");
    closeSection("layers");
    setLibraryCollapsed(true);
    setTimelineCollapsed(true);
    setActiveSheetEntityId(99 as never);
    openSection("reference");

    dispatchCommand("window:reset-layout");

    expect(isSectionOpen("color")).toBe(true);
    expect(isSectionOpen("layers")).toBe(true);
    expect(isSectionOpen("reference")).toBe(false);
    expect(isLibraryCollapsed()).toBe(false);
    expect(isTimelineCollapsed()).toBe(false);
  });
});

describe("dispatchCommand — sprite", () => {
  it("sprite:new opens the canvas-size dialog instead of dispatching sprite_add directly", async () => {
    invokeMock.mockResolvedValue({ id: 2, canvas: { width: 64, height: 64 } });
    const { canvasSizeRequest, closeCanvasSizeDialog } =
      await import("../shell/canvas-size-dialog-state");
    closeCanvasSizeDialog();

    dispatchCommand("sprite:new");

    // The handler must defer to the dialog rather than fire sprite_add.
    expect(invokeMock.mock.calls.find(([cmd]) => cmd === "sprite_add")).toBeUndefined();
    const req = canvasSizeRequest();
    expect(req).not.toBeNull();
    expect(req?.mode).toBe("sprite");

    // Invoking the dialog's onConfirm callback should dispatch sprite_add
    // with the chosen dimensions.
    req?.onConfirm({ width: 64, height: 48 });
    const call = invokeMock.mock.calls.find(([cmd]) => cmd === "sprite_add");
    expect(call).toBeDefined();
    expect(call?.[1]).toEqual({
      args: {
        name: "Untitled",
        canvas_width: 64,
        canvas_height: 48,
        color_mode: "rgba",
      },
    });

    closeCanvasSizeDialog();
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

describe("dispatchCommand — ai", () => {
  beforeEach(() => {
    // The verb-list cache is module-scoped and survives across tests
    // unless explicitly cleared. Reset so each test sees a fresh fetch.
    void clearVerbCache();
    setActiveVerb(null);
  });

  it("ai:critique fetches verb metadata via verb_list and opens the modal", async () => {
    invokeMock.mockResolvedValue([
      {
        id: "pixhaus.builtin.critique",
        display_name: "Critique",
        description: "VLM analysis",
        cancellable: true,
        required_capabilities: 0,
        input_schema: { type: "object", properties: {} },
      },
    ]);
    dispatchCommand("ai:critique");
    // openVerbModal awaits verb_list before setting activeVerb; flush.
    await new Promise((r) => setTimeout(r, 0));
    expect(invokeMock).toHaveBeenCalledWith("verb_list", undefined);
    expect(activeVerb()?.id).toBe("pixhaus.builtin.critique");
  });

  it("ai:inbetween triggers a verb_list fetch and opens the modal for inbetween", async () => {
    invokeMock.mockResolvedValue([
      {
        id: "pixhaus.builtin.inbetween",
        display_name: "Inbetween",
        description: "Frame interpolation",
        cancellable: true,
        required_capabilities: 0,
        input_schema: { type: "object", properties: {} },
      },
    ]);
    dispatchCommand("ai:inbetween");
    await new Promise((r) => setTimeout(r, 0));
    expect(invokeMock).toHaveBeenCalledWith("verb_list", undefined);
    expect(activeVerb()?.id).toBe("pixhaus.builtin.inbetween");
  });
});

describe("dispatchCommand — layer", () => {
  it("layer:flatten-visible invokes layer_flatten_visible with the active sprite id", async () => {
    dispatchCommand("layer:flatten-visible");
    // The handler kicks off a then/catch chain; flush the microtask.
    await new Promise((r) => setTimeout(r, 0));
    expect(invokeMock).toHaveBeenCalledWith("layer_flatten_visible", { sprite_id: 1 });
  });
});
