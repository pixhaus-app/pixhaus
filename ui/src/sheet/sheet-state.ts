// Sheet view panel state.
//
// Tracks which sprite entity is open in the sheet panel and which panel
// region the user last clicked (for scoped inpainting in B10.2). Panel
// open/closed state itself is owned by the right-rail accordion now
// (see ui/src/shell/rail-state.ts).

import { createSignal } from "solid-js";
import type { EntityId } from "../lib/types/EntityId";
import type { Rect } from "../lib/types/Rect";

/** The sprite entity currently shown in the sheet panel. `null` when no entity is selected. */
export const [activeSheetEntityId, setActiveSheetEntityId] = createSignal<EntityId | null>(null);

/**
 * The panel rectangle the user clicked most recently, used to scope the
 * iterate-reference-sheet verb to a single panel. `null` means no panel
 * is selected — the Refine button uses whole-sheet mode.
 */
export const [selectedPanelRegion, setSelectedPanelRegion] = createSignal<Rect | null>(null);

/** Whether the composition overlay (labelled panel rectangles) is drawn. */
export const [showPanelOverlay, setShowPanelOverlay] = createSignal(true);

/** The sprite entity currently open in the dedicated AI sheet editor. */
export const [activeSheetEditorEntityId, setActiveSheetEditorEntityId] =
  createSignal<EntityId | null>(null);

/** Whether the dedicated AI sheet editor replaces the normal canvas area. */
export const [isSheetEditorOpen, setSheetEditorOpen] = createSignal(false);

/** Sets the active sheet entity. Use with `openSection("reference")` from rail-state. */
export function showSheetForEntity(entityId: EntityId): void {
  setActiveSheetEntityId(entityId);
  setSelectedPanelRegion(null);
}

/** Opens the dedicated AI reference-sheet editor for the given sprite entity. */
export function openSheetEditor(entityId: EntityId): void {
  setActiveSheetEditorEntityId(entityId);
  setSelectedPanelRegion(null);
  setSheetEditorOpen(true);
}

/** Closes the dedicated AI reference-sheet editor. */
export function closeSheetEditor(): void {
  setSheetEditorOpen(false);
  setActiveSheetEditorEntityId(null);
  setSelectedPanelRegion(null);
}

/** Clears the active sheet entity. */
export function clearSheetEntity(): void {
  setActiveSheetEntityId(null);
  setSelectedPanelRegion(null);
}
