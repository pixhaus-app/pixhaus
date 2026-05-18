// Tests for the right-rail accordion state model.

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { createRoot } from "solid-js";
import { setActiveTool } from "../canvas/tools/tool-state";
import { setActiveTilemapCtx } from "../tilemap/tilemap-state";
import {
  closeSection,
  isLibraryCollapsed,
  isSectionOpen,
  isTimelineCollapsed,
  openSection,
  resetSections,
  setLibraryCollapsed,
  setTimelineCollapsed,
  toggleLibraryCollapsed,
  toggleSection,
  toggleTimelineCollapsed,
} from "./rail-state";

beforeEach(() => {
  setActiveTool("pencil");
  setActiveTilemapCtx(null);
  resetSections();
  setLibraryCollapsed(false);
  setTimelineCollapsed(false);
});

afterEach(() => {
  setActiveTool("pencil");
  setActiveTilemapCtx(null);
});

describe("rail-state defaults", () => {
  it("opens Color and Layers by default", () => {
    expect(isSectionOpen("color")).toBe(true);
    expect(isSectionOpen("layers")).toBe(true);
  });

  it("opens Brush when a brush tool is active", () => {
    setActiveTool("pencil");
    expect(isSectionOpen("brush")).toBe(true);
    expect(isSectionOpen("fill")).toBe(false);
  });

  it("opens Fill when fill tool is active", () => {
    setActiveTool("fill");
    expect(isSectionOpen("fill")).toBe(true);
    expect(isSectionOpen("brush")).toBe(false);
  });

  it("keeps Tilemap, Dithering, FX, Reference closed by default", () => {
    expect(isSectionOpen("tilemap")).toBe(false);
    expect(isSectionOpen("dithering")).toBe(false);
    expect(isSectionOpen("fx")).toBe(false);
    expect(isSectionOpen("reference")).toBe(false);
  });
});

describe("auto-expand rules", () => {
  it("toggles Brush vs Fill as active tool changes", () =>
    createRoot((dispose) => {
      resetSections();
      setActiveTool("pencil");
      expect(isSectionOpen("brush")).toBe(true);
      expect(isSectionOpen("fill")).toBe(false);

      setActiveTool("fill");
      expect(isSectionOpen("brush")).toBe(false);
      expect(isSectionOpen("fill")).toBe(true);

      setActiveTool("eraser");
      expect(isSectionOpen("brush")).toBe(true);
      expect(isSectionOpen("fill")).toBe(false);
      dispose();
    }));

  it("auto-expands Tilemap when active layer becomes a tilemap", () =>
    createRoot((dispose) => {
      resetSections();
      expect(isSectionOpen("tilemap")).toBe(false);

      setActiveTilemapCtx({
        layerId: 1 as never,
        tilesetId: 1 as never,
        tileset: { id: 1, name: "t", tile_w: 16, tile_h: 16, columns: 1, tiles: [] } as never,
      });
      expect(isSectionOpen("tilemap")).toBe(true);

      setActiveTilemapCtx(null);
      expect(isSectionOpen("tilemap")).toBe(false);
      dispose();
    }));

  it("respects user-touched state — auto-rules stop firing for that section", () =>
    createRoot((dispose) => {
      resetSections();
      setActiveTool("pencil");
      expect(isSectionOpen("brush")).toBe(true);

      // User explicitly closes brush.
      closeSection("brush");
      expect(isSectionOpen("brush")).toBe(false);

      // Switching to a non-brush tool would normally close brush, but it's
      // already closed and user-touched. Switching back to a brush tool
      // should NOT reopen brush because userTouched is true.
      setActiveTool("fill");
      setActiveTool("pencil");
      expect(isSectionOpen("brush")).toBe(false);
      dispose();
    }));
});

describe("toggle/open/close", () => {
  it("toggleSection flips open and marks user-touched", () => {
    expect(isSectionOpen("dithering")).toBe(false);
    toggleSection("dithering");
    expect(isSectionOpen("dithering")).toBe(true);
    toggleSection("dithering");
    expect(isSectionOpen("dithering")).toBe(false);
  });

  it("openSection and closeSection are idempotent", () => {
    closeSection("color");
    closeSection("color");
    expect(isSectionOpen("color")).toBe(false);

    openSection("color");
    openSection("color");
    expect(isSectionOpen("color")).toBe(true);
  });

  it("resetSections re-applies default + auto-rule state", () => {
    closeSection("color");
    closeSection("layers");
    openSection("dithering");

    resetSections();

    expect(isSectionOpen("color")).toBe(true);
    expect(isSectionOpen("layers")).toBe(true);
    expect(isSectionOpen("dithering")).toBe(false);
  });
});

describe("library and timeline collapse", () => {
  it("library collapse round-trips", () => {
    expect(isLibraryCollapsed()).toBe(false);
    toggleLibraryCollapsed();
    expect(isLibraryCollapsed()).toBe(true);
    toggleLibraryCollapsed();
    expect(isLibraryCollapsed()).toBe(false);
  });

  it("timeline collapse round-trips", () => {
    expect(isTimelineCollapsed()).toBe(false);
    toggleTimelineCollapsed();
    expect(isTimelineCollapsed()).toBe(true);
    toggleTimelineCollapsed();
    expect(isTimelineCollapsed()).toBe(false);
  });
});
