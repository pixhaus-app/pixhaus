// Right-rail accordion state.
//
// Replaces the per-panel visibility signals (isLayerPanelVisible,
// isPalettePanelVisible, isTilemapPanelVisible, isLibraryPanelVisible,
// isTimelinePanelVisible, isSheetPanelVisible) with a single source of
// truth for the new contextual rail layout.
//
// Each section has two pieces of state: `explicitOpen` (the user's last
// manual choice, if any) and `userTouched`. `isSectionOpen(id)` returns
// `explicitOpen` if the user has touched the section; otherwise it
// returns the auto-rule for that section (derived from activeTool and
// activeTilemapCtx). The moment the user clicks a section header,
// `userTouched` flips to true and the section's state is owned by the
// user for the rest of the project session.
//
// Library and Timeline live outside the right rail but share the same
// pattern via dedicated collapsed booleans.

import { createMemo } from "solid-js";
import { createStore } from "solid-js/store";
import { activeTool, BRUSH_TOOLS } from "../canvas/tools/tool-state";
import { selectTool } from "../canvas/select/select-state";
import { activeTilemapCtx } from "../tilemap/tilemap-state";

export type SectionId =
  | "brush"
  | "fill"
  | "select"
  | "color"
  | "layers"
  | "tilemap"
  | "dithering"
  | "fx"
  | "reference";

export const SECTION_ORDER: readonly SectionId[] = [
  "brush",
  "fill",
  "select",
  "dithering",
  "color",
  "layers",
  "tilemap",
  "fx",
  "reference",
];

export const SECTION_TITLE: Record<SectionId, string> = {
  brush: "Brush",
  fill: "Fill",
  select: "Select",
  dithering: "Dithering",
  color: "Color",
  layers: "Layers",
  tilemap: "Tilemap",
  fx: "FX",
  reference: "Reference",
};

/** One-line explanation surfaced via the header help icon for each section. */
export const SECTION_HELP: Record<SectionId, string> = {
  brush: "Size, shape and pixel-perfect options for the active drawing tool.",
  fill: "Tolerance and contiguity for the flood-fill bucket.",
  select:
    "Options for the active selection tool. Pick the magic wand from the toolbar to tune tolerance and gap-closing.",
  dithering: "Apply ordered dithering patterns while you paint.",
  color: "Pick colors, manage the palette, and set color animation keyframes.",
  layers: "Stack, reorder, blend and toggle layers. The active layer receives your edits.",
  tilemap: "Paint with tiles from the active tilemap context.",
  fx: "Non-destructive effects applied to the active layer.",
  reference: "Preview the exported sprite sheet and reference images.",
};

type SectionState = { explicitOpen: boolean; userTouched: boolean };

type RailState = {
  sections: Record<SectionId, SectionState>;
  libraryCollapsed: boolean;
  timelineCollapsed: boolean;
};

function isBrushTool(tool: string): boolean {
  return (BRUSH_TOOLS as readonly string[]).includes(tool);
}

function autoOpen(id: SectionId): boolean {
  switch (id) {
    case "brush":
      return isBrushTool(activeTool());
    case "fill":
      return activeTool() === "fill";
    case "select":
      // Auto-open whenever the user has the wand tool active; the
      // section body shows nothing useful for other select tools.
      return selectTool() === "wand";
    case "tilemap":
      return activeTilemapCtx() !== null;
    case "color":
    case "layers":
      return true;
    case "dithering":
    case "fx":
    case "reference":
      return false;
  }
}

function blankSections(): Record<SectionId, SectionState> {
  return {
    brush: { explicitOpen: false, userTouched: false },
    fill: { explicitOpen: false, userTouched: false },
    select: { explicitOpen: false, userTouched: false },
    dithering: { explicitOpen: false, userTouched: false },
    color: { explicitOpen: false, userTouched: false },
    layers: { explicitOpen: false, userTouched: false },
    tilemap: { explicitOpen: false, userTouched: false },
    fx: { explicitOpen: false, userTouched: false },
    reference: { explicitOpen: false, userTouched: false },
  };
}

const [railState, setRailState] = createStore<RailState>({
  sections: blankSections(),
  libraryCollapsed: false,
  timelineCollapsed: false,
});

// ── Public API ───────────────────────────────────────────────────────────────

/** Whether a section is currently rendered open. */
export function isSectionOpen(id: SectionId): boolean {
  const s = railState.sections[id];
  return s.userTouched ? s.explicitOpen : autoOpen(id);
}

/** Toggle a section open/closed. Sets userTouched so auto-rules no longer fire. */
export function toggleSection(id: SectionId): void {
  const current = isSectionOpen(id);
  setRailState("sections", id, { explicitOpen: !current, userTouched: true });
}

/** Open a section explicitly (idempotent). Sets userTouched. */
export function openSection(id: SectionId): void {
  setRailState("sections", id, { explicitOpen: true, userTouched: true });
}

/** Close a section explicitly (idempotent). Sets userTouched. */
export function closeSection(id: SectionId): void {
  setRailState("sections", id, { explicitOpen: false, userTouched: true });
}

/** Reset every section to default + auto-rule state. */
export function resetSections(): void {
  setRailState("sections", blankSections());
}

/** Visibility of the left library dock (true = collapsed to icon strip). */
export function isLibraryCollapsed(): boolean {
  return railState.libraryCollapsed;
}

export function setLibraryCollapsed(collapsed: boolean): void {
  setRailState("libraryCollapsed", collapsed);
}

export function toggleLibraryCollapsed(): void {
  setRailState("libraryCollapsed", (c) => !c);
}

/** Visibility of the bottom timeline (true = collapsed to thumbnail strip). */
export function isTimelineCollapsed(): boolean {
  return railState.timelineCollapsed;
}

export function setTimelineCollapsed(collapsed: boolean): void {
  setRailState("timelineCollapsed", collapsed);
}

export function toggleTimelineCollapsed(): void {
  setRailState("timelineCollapsed", (c) => !c);
}

/** Memo for the open sections in display order — useful for the rail renderer. */
export const openSectionIds = createMemo<readonly SectionId[]>(() =>
  SECTION_ORDER.filter((id) => isSectionOpen(id)),
);
