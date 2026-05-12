// Sheet view panel state.
//
// Tracks which Reference entity is open in the sheet panel, which panel
// region the user last clicked (for scoped inpainting in B10.2), and
// whether the panel overlay of labelled rectangles is visible.

import { createSignal } from "solid-js";
import type { EntityId } from "../lib/types/EntityId";
import type { Rect } from "../lib/types/Rect";

/** The Reference entity currently shown in the sheet panel. `null` when closed. */
export const [activeSheetEntityId, setActiveSheetEntityId] = createSignal<EntityId | null>(null);

/**
 * The panel rectangle the user clicked most recently, used to scope the
 * iterate-reference-sheet verb to a single panel. `null` means no panel
 * is selected — the Refine button uses whole-sheet mode.
 */
export const [selectedPanelRegion, setSelectedPanelRegion] = createSignal<Rect | null>(null);

/** Whether the composition overlay (labelled panel rectangles) is drawn. */
export const [showPanelOverlay, setShowPanelOverlay] = createSignal(true);

/** Whether the sheet panel is mounted in the editor layout. */
export const [isSheetPanelVisible, setSheetPanelVisible] = createSignal(false);

/** Opens the sheet panel for the given Reference entity and makes the panel visible. */
export function openSheetPanel(entityId: EntityId): void {
  setActiveSheetEntityId(entityId);
  setSelectedPanelRegion(null);
  setSheetPanelVisible(true);
}

/** Closes the sheet panel and clears transient selection state. */
export function closeSheetPanel(): void {
  setSheetPanelVisible(false);
  setActiveSheetEntityId(null);
  setSelectedPanelRegion(null);
}
