// Undo and redo commands.

import { invoke } from "../ipc";

/** Undo the most recently applied command. */
export function undo(): Promise<void> {
  return invoke<void>("undo");
}

/** Redo the most-recently-undone command. */
export function redo(): Promise<void> {
  return invoke<void>("redo");
}
