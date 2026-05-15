// State module for the entity create dialog.
//
// Mounted at Shell level via EntityCreateModal. Library callers invoke
// openEntityCreate(...) to surface the dialog; the dialog closes itself
// after success or cancel.

import { createSignal } from "solid-js";
import type { GroupId } from "../lib/types";

export interface EntityCreateRequest {
  initialGroupId: GroupId | null;
}

const [request, setRequest] = createSignal<EntityCreateRequest | null>(null);

export const entityCreateRequest = request;

export function openEntityCreate(req: EntityCreateRequest = { initialGroupId: null }): void {
  setRequest(req);
}

export function closeEntityCreate(): void {
  setRequest(null);
}
