// State module for the anchor-reference picker dialog.
//
// Mounted at Shell level via AnchorPickerDialog. LibraryPanel calls
// openAnchorPicker(...) from its row context-menu; the onConfirm callback
// closes the loop back to the library-side state (refresh, toast).

import { createSignal } from "solid-js";
import type { EntityId } from "../lib/types";

export interface AnchorReference {
  id: EntityId;
  name: string;
}

export interface AnchorPickerRequest {
  entityId: EntityId;
  references: AnchorReference[];
  onConfirm: (refId: EntityId) => void;
}

const [request, setRequest] = createSignal<AnchorPickerRequest | null>(null);

export const anchorPickerRequest = request;

export function openAnchorPicker(req: AnchorPickerRequest): void {
  setRequest(req);
}

export function closeAnchorPicker(): void {
  setRequest(null);
}
