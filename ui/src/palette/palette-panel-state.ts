// Reactive state for the color and palette panel (S18).
//
// Module-level Solid signals own all panel state so any component can read
// or write without prop-drilling. The pattern mirrors canvas-state.ts.

import { createStore } from "solid-js/store";
import type { Palette, PaletteId, Rgba, SpriteId } from "../lib/types";
import { paletteList } from "../lib/commands/palette";
import { createBackendQuery } from "../lib/sync/query";
import { activeTarget } from "../canvas/canvas-state";

// ── Palettes ──────────────────────────────────────────────────────────────────

// UI-only palette-panel state in one store. Reads are paletteUi.activePaletteId,
// paletteUi.foregroundIndex, etc.; writes go through the setters below (the
// Set-mutating one accepts a value or updater). Declared before the palettes
// query because the query's onLoaded defaults activePaletteId.
interface PaletteUiState {
  activePaletteId: PaletteId | null;
  /** Index into the active palette used for the foreground color. */
  foregroundIndex: number;
  /** Index into the active palette used for the background color. */
  backgroundIndex: number;
  /** Index of the swatch being edited, or null when no edit is in progress. */
  editingIndex: number | null;
  /** Draft color for the swatch at editingIndex before it is committed. */
  editingColor: Rgba | null;
  /** Per-swatch lock (UI guard only, not persisted to Rust). */
  lockedIndices: ReadonlySet<number>;
}

export const [paletteUi, setPaletteUi] = createStore<PaletteUiState>({
  activePaletteId: null,
  foregroundIndex: 0,
  backgroundIndex: 1,
  editingIndex: null,
  editingColor: null,
  lockedIndices: new Set(),
});

type SetArg<T> = T | ((prev: T) => T);

export const setActivePaletteId = (v: PaletteId | null): void => setPaletteUi("activePaletteId", v);
export const setForegroundIndex = (v: number): void => setPaletteUi("foregroundIndex", v);
export const setBackgroundIndex = (v: number): void => setPaletteUi("backgroundIndex", v);
export const setEditingIndex = (v: number | null): void => setPaletteUi("editingIndex", v);
export const setEditingColor = (v: Rgba | null): void => setPaletteUi("editingColor", v);
export const setLockedIndices = (next: SetArg<ReadonlySet<number>>): void =>
  setPaletteUi("lockedIndices", typeof next === "function" ? next : () => next);

// Palettes for the active sprite, backed by a createBackendQuery. The cache
// reloads on sprite change and on refreshPalettes() (which palette mutations
// in the panel call after editing).
const palettesQuery = createBackendQuery<SpriteId, Palette[]>({
  key: "palettes",
  source: () => activeTarget.spriteId,
  fetch: (spriteId) => paletteList(spriteId),
  initial: [],
  onLoaded: (list) => ensureActivePalette(list),
  errorTitle: "Failed to load palettes",
});

export const palettes = palettesQuery.data;

/** The currently active palette, defaulting to the first in the list. */
export function activePalette(): Palette | null {
  const list = palettes();
  if (list.length === 0) return null;
  const id = paletteUi.activePaletteId;
  if (id === null) return list[0] ?? null;
  return list.find((p) => p.id === id) ?? list[0] ?? null;
}

// Keep activePaletteId pointing at a live palette: default to the first when
// nothing is selected or the selection vanished (e.g. after a sprite change).
function ensureActivePalette(list: Palette[] | undefined): void {
  const result = list ?? [];
  const id = paletteUi.activePaletteId;
  const first = result[0];
  if (first && (id === null || !result.some((p) => p.id === id))) {
    setActivePaletteId(first.id);
  }
}

/**
 * Refetches palettes for the active sprite. The `spriteId` argument is kept
 * for call-site compatibility but ignored — the query already keys on the
 * active sprite. Resolves when the fetch settles.
 */
export async function refreshPalettes(_spriteId?: SpriteId): Promise<void> {
  await palettesQuery.refetch();
}

// ── Foreground / background indices ───────────────────────────────────────────

/** Swap foreground and background indices (Photoshop X key). */
export function swapFgBg(): void {
  const fg = paletteUi.foregroundIndex;
  setForegroundIndex(paletteUi.backgroundIndex);
  setBackgroundIndex(fg);
}

/** Reset foreground to index 0 and background to index 1 (Photoshop D key). */
export function resetFgBg(): void {
  setForegroundIndex(0);
  setBackgroundIndex(1);
}

// ── Per-swatch edit session ───────────────────────────────────────────────────
//
// These track the in-progress color edit while the picker is being dragged.
// The committed value lives in the palettes query (updated after IPC).

/** Begin editing a specific swatch. */
export function startEditing(index: number, color: Rgba): void {
  setEditingIndex(index);
  setEditingColor(color);
}

/** Discard the current edit without committing. */
export function cancelEditing(): void {
  setEditingIndex(null);
  setEditingColor(null);
}

// ── Per-swatch lock (UI guard only, not persisted to Rust) ────────────────────

export function toggleLock(index: number): void {
  setLockedIndices((prev) => {
    const next = new Set(prev);
    if (next.has(index)) next.delete(index);
    else next.add(index);
    return next;
  });
}

export function isLocked(index: number): boolean {
  return paletteUi.lockedIndices.has(index);
}
