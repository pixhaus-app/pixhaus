// Sheet view panel state.
//
// Tracks which sprite entity is open in the sheet panel and which panel
// region the user last clicked (for scoped inpainting in B10.2). Panel
// open/closed state itself is owned by the right-rail accordion now
// (see ui/src/shell/rail-state.ts).

import { createStore } from "solid-js/store";
import type { EntityId } from "../lib/types/EntityId";
import type { Rect } from "../lib/types/Rect";

// One store for the sheet panel / editor. Reads are sheet.activeSheetEntityId,
// sheet.isSheetEditorOpen, etc.; writes go through the named functions below.
interface SheetState {
  /** Sprite entity shown in the sheet panel; null when none selected. */
  activeSheetEntityId: EntityId | null;
  /** Panel rect the user clicked most recently, used to scope the
   * iterate-reference-sheet verb. null = whole-sheet mode. */
  selectedPanelRegion: Rect | null;
  /** Whether the composition overlay (labelled panel rectangles) is drawn. */
  showPanelOverlay: boolean;
  /** Sprite entity open in the dedicated AI sheet editor. */
  activeSheetEditorEntityId: EntityId | null;
  /** Whether the dedicated AI sheet editor replaces the normal canvas area. */
  isSheetEditorOpen: boolean;
}

export const [sheetState, setSheetState] = createStore<SheetState>({
  activeSheetEntityId: null,
  selectedPanelRegion: null,
  showPanelOverlay: true,
  activeSheetEditorEntityId: null,
  isSheetEditorOpen: false,
});

export const setActiveSheetEntityId = (v: EntityId | null): void =>
  setSheetState("activeSheetEntityId", v);
export const setSelectedPanelRegion = (v: Rect | null): void =>
  setSheetState("selectedPanelRegion", v);
export const setShowPanelOverlay = (v: boolean): void => setSheetState("showPanelOverlay", v);

/** Sets the active sheet entity. Use with `openSection("reference")` from rail-state. */
export function showSheetForEntity(entityId: EntityId): void {
  setSheetState({ activeSheetEntityId: entityId, selectedPanelRegion: null });
}

/** Opens the dedicated AI reference-sheet editor for the given sprite entity. */
export function openSheetEditor(entityId: EntityId): void {
  setSheetState({
    activeSheetEditorEntityId: entityId,
    selectedPanelRegion: null,
    isSheetEditorOpen: true,
  });
}

/** Closes the dedicated AI reference-sheet editor. */
export function closeSheetEditor(): void {
  setSheetState({
    isSheetEditorOpen: false,
    activeSheetEditorEntityId: null,
    selectedPanelRegion: null,
  });
}

/** Clears the active sheet entity. */
export function clearSheetEntity(): void {
  setSheetState({ activeSheetEntityId: null, selectedPanelRegion: null });
}
