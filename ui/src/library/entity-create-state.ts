// State module for the entity create dialog.
//
// Mounted at Shell level via EntityCreateModal. Library callers invoke
// openEntityCreate(...) to surface the dialog; the dialog closes itself
// after success or cancel.

import { createStore } from "solid-js/store";
import type { GroupId } from "../lib/types";

export interface EntityCreateRequest {
  initialGroupId: GroupId | null;
}

// A store for consistency with the rest of the UI state layer; read as
// entityCreate.request (non-null while the dialog is open).
export const [entityCreate, setEntityCreate] = createStore<{ request: EntityCreateRequest | null }>(
  { request: null },
);

export function openEntityCreate(req: EntityCreateRequest = { initialGroupId: null }): void {
  setEntityCreate("request", req);
}

export function closeEntityCreate(): void {
  setEntityCreate("request", null);
}
