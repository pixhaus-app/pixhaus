// Reactive state for the layer panel.
//
// Layer data lives in Rust; this module caches the last-fetched list and
// tracks UI-only state (selection, renaming, drag target, panel visibility).
// Mutations go through the command wrappers, which call Rust and then refresh
// the list via refreshLayers().

import { createStore } from "solid-js/store";
import type { BlendMode, Layer, LayerEffect, LayerId, SpriteId, TilesetId } from "../lib/types";
import {
  layerAdd,
  layerConvertToTilemap,
  layerDelete,
  layerFlattenVisible,
  layerList,
  layerMergeDown,
  layerMergeSelected,
  layerRename,
  layerReorder,
  layerSetBlendMode,
  layerSetEffects,
  layerSetLocked,
  layerSetOpacity,
  layerSetParent,
  layerSetVisibility,
  layerWrapInGroup,
} from "../lib/commands/layers";
import { canvasRecompositeFrame } from "../lib/commands/canvas";
import {
  activeFrameIndex,
  activeSpriteId,
  activeLayerId,
  setActiveLayerId,
  scheduleViewportSync,
} from "../canvas/canvas-state";
import { createBackendQuery } from "../lib/sync/query";
import { runMutation } from "../lib/sync/mutation";
import { invalidate } from "../lib/sync/invalidation";
import { pushToast } from "../lib/toast/toast-state";

/**
 * Asks the backend to recomposite the active frame and emit tile-dirty
 * events for every tile so the viewport repaints. Call after any layer
 * state change with no pixel-mutation IPC of its own (visibility,
 * opacity, blend mode). Lock toggles are intentionally not in this set —
 * `locked` has no visual effect.
 */
function refreshViewport(spriteId: SpriteId): void {
  canvasRecompositeFrame(spriteId, activeFrameIndex()).catch((err: unknown) =>
    console.error("[pixhaus] canvas_recomposite_frame:", err),
  );
}

// ── Tree flattening ─────────────────────────────────────────────────────────

export type FlatEntry = { layer: Layer; depth: number; index: number };

/**
 * Flatten the layer tree into a top-to-bottom ordered list of rows with depth
 * and flat-list index info. Rust stores layers bottom-to-top; we reverse so
 * the topmost layer appears first in the rendered list.
 *
 * Groups that are collapsed exclude their children from the result.
 */
export function flattenLayers(all: Layer[], expandedCheck: (id: LayerId) => boolean): FlatEntry[] {
  // Pre-compute id → index once so the recursive walk is O(n) instead
  // of O(n²) (the prior implementation called all.indexOf() per node).
  const indexById = new Map<LayerId, number>();
  for (let i = 0; i < all.length; i++) {
    const layer = all[i];
    if (layer !== undefined) indexById.set(layer.id, i);
  }

  const childrenOf = new Map<LayerId | null, Layer[]>();
  for (const layer of all) {
    const key = layer.parent ?? null;
    const bucket = childrenOf.get(key) ?? [];
    bucket.push(layer);
    childrenOf.set(key, bucket);
  }

  const result: FlatEntry[] = [];

  function visit(parentId: LayerId | null, depth: number): void {
    const children = childrenOf.get(parentId) ?? [];
    for (let i = children.length - 1; i >= 0; i--) {
      const layer = children[i];
      if (layer === undefined) continue;
      const index = indexById.get(layer.id) ?? 0;
      result.push({ layer, depth, index });
      if (layer.kind.kind === "group" && expandedCheck(layer.id)) {
        visit(layer.id, depth + 1);
      }
    }
  }

  visit(null, 0);
  return result;
}

// ── Layer list cache ────────────────────────────────────────────────────────

// Flat list ordered bottom-to-top (index 0 = bottom layer), matching Rust's order.
// The panel renders in reverse to show topmost layers at the top of the list.
//
// Backed by a createBackendQuery keyed on the active sprite. The query
// refetches automatically when the active sprite changes and discards
// out-of-order responses itself, so the hand-rolled refreshToken guard this
// module used to carry is gone. `layers()` keeps its old read shape.
const layersQuery = createBackendQuery<SpriteId, Layer[]>({
  key: "layers",
  source: activeSpriteId,
  fetch: (spriteId) => layerList(spriteId),
  initial: [],
  onLoaded: (list) => ensureActiveLayer(list),
  errorTitle: "Failed to load layers",
});

export const layers = layersQuery.data;

/** Refetches the layer list for the active sprite. */
export function refreshLayers(): void {
  void layersQuery.refetch();
}

/**
 * Picks a sensible default for `activeLayerId` when the previous one is
 * gone (or never set). Without an active layer, every paint / transform /
 * select-on-layer command silently fails its `if (layerId === null) return`
 * guard and the user sees "nothing happens." Runs whenever the layers query
 * commits a fresh list, so the auto-pick lines up with the rendered panel
 * state. Tolerates an undefined list (idle cache before the first fetch).
 */
function ensureActiveLayer(list: readonly Layer[] | undefined): void {
  if (list === undefined || list.length === 0) {
    setActiveLayerId(null);
    return;
  }
  const current = activeLayerId();
  if (current !== null && list.some((l) => l.id === current)) {
    return;
  }
  // Pick the topmost (last in bottom-to-top order). Mirrors what a user
  // expects after "New Project" — paints land on the top visible layer.
  const top = list[list.length - 1];
  if (top !== undefined) {
    setActiveLayerId(top.id);
  }
}

// ── Selection ───────────────────────────────────────────────────────────────

// When non-null, a tileset picker dialog is open and Convert-to-Tilemap is
// pending the user's tileset choice. Set by the layer context menu, cleared by
// the dialog on confirm/cancel.
export type TilesetPickerTarget = {
  spriteId: SpriteId;
  layerId: LayerId;
};

// UI-only layer-panel state, grouped in one store. Reads are
// layerUi.selectedLayerIds, layerUi.renamingLayerId, etc.; writes go through
// the setter functions below (which accept either a value or an updater so the
// Set-mutating call sites keep their callback form). The "primary" active
// layer (for painting) is activeLayerId() from canvas-state; selectedLayerIds
// tracks which rows are highlighted in the panel.
interface LayerUiState {
  selectedLayerIds: ReadonlySet<LayerId>;
  renamingLayerId: LayerId | null;
  /** Groups default to expanded; collapsed ones are tracked explicitly. */
  collapsedGroups: ReadonlySet<LayerId>;
  /** Flat-list index where the drop indicator should render. */
  dragOverIndex: number | null;
  tilesetPickerTarget: TilesetPickerTarget | null;
}

const [layerUi, setLayerUi] = createStore<LayerUiState>({
  selectedLayerIds: new Set(),
  renamingLayerId: null,
  collapsedGroups: new Set(),
  dragOverIndex: null,
  tilesetPickerTarget: null,
});

export { layerUi };

type SetArg<T> = T | ((prev: T) => T);

export function setSelectedLayerIds(next: SetArg<ReadonlySet<LayerId>>): void {
  setLayerUi("selectedLayerIds", typeof next === "function" ? next : () => next);
}
export const setRenamingLayerId = (v: LayerId | null): void => setLayerUi("renamingLayerId", v);
function setCollapsedGroups(next: SetArg<ReadonlySet<LayerId>>): void {
  setLayerUi("collapsedGroups", typeof next === "function" ? next : () => next);
}
export const setDragOverIndex = (v: number | null): void => setLayerUi("dragOverIndex", v);
export const setTilesetPickerTarget = (v: TilesetPickerTarget | null): void =>
  setLayerUi("tilesetPickerTarget", v);

export function selectLayer(id: LayerId, extend: boolean): void {
  setActiveLayerId(id);
  // The backend keeps its own CanvasState.active_layer; without this
  // sync the editor's painting target stays on the old layer until the
  // next viewport interaction.
  scheduleViewportSync();
  if (extend) {
    setSelectedLayerIds((prev) => new Set([...prev, id]));
  } else {
    setSelectedLayerIds(new Set([id]));
  }
}

export function toggleLayerSelection(id: LayerId): void {
  setSelectedLayerIds((prev) => {
    const next = new Set(prev);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    return next;
  });
}

// ── Inline rename state ─────────────────────────────────────────────────────

export function beginRename(id: LayerId): void {
  setRenamingLayerId(id);
}

export function commitRename(id: LayerId, name: string): void {
  const spriteId = activeSpriteId();
  if (spriteId === null) return;
  setRenamingLayerId(null);
  const trimmed = name.trim();
  if (!trimmed) return;
  void runMutation({
    run: () => layerRename(spriteId, id, trimmed),
    invalidate: ["layers"],
  });
}

export function cancelRename(): void {
  setRenamingLayerId(null);
}

// ── Expanded groups ─────────────────────────────────────────────────────────

export function isGroupExpanded(id: LayerId): boolean {
  return !layerUi.collapsedGroups.has(id);
}

export function toggleGroupExpanded(id: LayerId): void {
  setCollapsedGroups((prev) => {
    const next = new Set(prev);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    return next;
  });
}

/**
 * Idempotent expand: drops `id` from the collapsed set if present, leaves
 * it otherwise. Uses the setter's callback form so the read happens inside
 * the reactive update and doesn't trip solid/reactivity in a one-shot
 * Promise callback.
 */
export function ensureGroupExpanded(id: LayerId): void {
  setCollapsedGroups((prev) => {
    if (!prev.has(id)) return prev;
    const next = new Set(prev);
    next.delete(id);
    return next;
  });
}

// ── Tileset picker (Convert to Tilemap Layer) ───────────────────────────────

export function openTilesetPicker(spriteId: SpriteId, layerId: LayerId): void {
  setTilesetPickerTarget({ spriteId, layerId });
}

export function closeTilesetPicker(): void {
  setTilesetPickerTarget(null);
}

// ── Auto-name helper ────────────────────────────────────────────────────────

/**
 * Picks the next name in a `<prefix> N` series, where N is one more than
 * the highest matching N currently on the sprite. Deletes don't gap-fill —
 * after `Layer 1, Layer 2, Layer 3` and deleting `Layer 2`, the next name
 * is `Layer 4`. Less surprising over a long session than reusing numbers.
 *
 * `prefix` is treated as a literal string, so callers can safely pass
 * names that contain regex metacharacters. Mirrors the Rust
 * `next_auto_name` in `app/src/commands/layers.rs` — keep the two in
 * sync.
 */
export function nextAutoName(all: readonly Layer[], prefix: string): string {
  const needle = `${prefix} `;
  let max = 0;
  for (const l of all) {
    if (!l.name.startsWith(needle)) continue;
    const rest = l.name.slice(needle.length);
    if (!/^\d+$/.test(rest)) continue;
    const n = parseInt(rest, 10);
    if (Number.isFinite(n) && n > max) max = n;
  }
  return `${prefix} ${max + 1}`;
}

// ── Mutation helpers ────────────────────────────────────────────────────────

export function addLayer(spriteId: SpriteId, name: string): void {
  void runMutation({
    run: () => layerAdd({ sprite_id: spriteId, name, kind: { kind: "raster" } }),
    invalidate: ["layers"],
    onSuccess: (layer) => selectLayer(layer.id, false),
  });
}

export function deleteLayer(spriteId: SpriteId, id: LayerId): void {
  void runMutation({
    run: () => layerDelete(spriteId, id),
    invalidate: ["layers"],
    onSuccess: () => {
      // Clear the active layer if it was the one deleted.
      if (activeLayerId() === id) setActiveLayerId(null);
      setSelectedLayerIds((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    },
  });
}

/**
 * Batch delete: fans out per-id IPCs in parallel, then refreshes once.
 * `Promise.allSettled` so a single failed delete doesn't strand the UI
 * with the other (successful) deletes invisible. Each per-layer delete
 * is still its own undo step on the backend; that trade-off is
 * documented in the layer-panel-bugs PR.
 */
export function deleteLayers(spriteId: SpriteId, ids: readonly LayerId[]): void {
  if (ids.length === 0) return;
  void Promise.allSettled(ids.map((id) => layerDelete(spriteId, id))).then((results) => {
    const failed = results.filter((r) => r.status === "rejected").length;
    if (failed > 0) {
      pushToast({
        kind: "error",
        title: "Some layers could not be deleted",
        body: `${failed} of ${ids.length} deletes failed.`,
      });
    }
    invalidate("layers");
    const idSet = new Set(ids);
    const active = activeLayerId();
    if (active !== null && idSet.has(active)) setActiveLayerId(null);
    setSelectedLayerIds((prev) => {
      const next = new Set(prev);
      for (const id of ids) next.delete(id);
      return next;
    });
  });
}

export function reorderLayer(spriteId: SpriteId, id: LayerId, newIndex: number): void {
  void runMutation({
    run: () => layerReorder(spriteId, id, newIndex),
    invalidate: ["layers"],
  });
}

export function setLayerVisibility(spriteId: SpriteId, id: LayerId, visible: boolean): void {
  void runMutation({
    run: () => layerSetVisibility(spriteId, id, visible),
    invalidate: ["layers"],
    onSuccess: () => refreshViewport(spriteId),
  });
}

export function setLayerLocked(spriteId: SpriteId, id: LayerId, locked: boolean): void {
  void runMutation({
    run: () => layerSetLocked(spriteId, id, locked),
    invalidate: ["layers"],
  });
}

/**
 * Returns false if the layer or any ancestor group is locked. Mirrors the
 * Rust `check_layer_writable` helper so the UI can refuse paint without a
 * round-trip; the Rust guard is the authoritative check, this is just for
 * instant feedback and a clean toast.
 */
export function isLayerWritable(all: readonly Layer[], id: LayerId): boolean {
  const byId = new Map<LayerId, Layer>();
  for (const l of all) byId.set(l.id, l);
  let current: Layer | undefined = byId.get(id);
  if (!current) return true; // missing layer is the IPC's problem to surface
  while (current) {
    if (current.locked) return false;
    if (current.parent === null || current.parent === undefined) break;
    current = byId.get(current.parent);
  }
  return true;
}

/**
 * Convenience for canvas input: looks up the active layer in the cached
 * layer list and runs `isLayerWritable`. Returns true (writable) when no
 * active layer is set; the caller already early-returns in that case.
 */
export function isActiveLayerWritable(): boolean {
  const id = activeLayerId();
  if (id === null) return true;
  return isLayerWritable(layers(), id);
}

export function setLayerOpacity(spriteId: SpriteId, id: LayerId, opacity: number): void {
  void runMutation({
    run: () => layerSetOpacity(spriteId, id, opacity),
    invalidate: ["layers"],
    onSuccess: () => refreshViewport(spriteId),
  });
}

export function setLayerBlendMode(spriteId: SpriteId, id: LayerId, blendMode: BlendMode): void {
  void runMutation({
    run: () => layerSetBlendMode(spriteId, id, blendMode),
    invalidate: ["layers"],
    onSuccess: () => refreshViewport(spriteId),
  });
}

export function setLayerEffects(spriteId: SpriteId, id: LayerId, effects: LayerEffect[]): void {
  void runMutation({
    run: () => layerSetEffects(spriteId, id, effects),
    invalidate: ["layers"],
    onSuccess: () => refreshViewport(spriteId),
  });
}

export function reparentLayer(spriteId: SpriteId, id: LayerId, parentId: LayerId | null): void {
  void runMutation({
    run: () => layerSetParent(spriteId, id, parentId),
    invalidate: ["layers"],
  });
}

export function wrapLayersInGroup(spriteId: SpriteId, layerIds: readonly LayerId[]): void {
  if (layerIds.length === 0) return;
  void runMutation({
    run: () => layerWrapInGroup(spriteId, [...layerIds]),
    invalidate: ["layers"],
    // Auto-expand the new group so its children (the wrapped layers) are
    // immediately visible — and so the group is a valid drop target for
    // additional layers without an extra chevron click.
    onSuccess: (newGroup) => ensureGroupExpanded(newGroup.id),
  });
}

export function convertLayerToTilemap(spriteId: SpriteId, id: LayerId, tilesetId: TilesetId): void {
  void runMutation({
    run: () => layerConvertToTilemap(spriteId, id, tilesetId),
    invalidate: ["layers"],
  });
}

export function mergeLayerDown(spriteId: SpriteId, id: LayerId): void {
  void runMutation({
    run: () => layerMergeDown(spriteId, id),
    invalidate: ["layers"],
  });
}

export function mergeSelectedLayers(spriteId: SpriteId, ids: ReadonlySet<LayerId>): void {
  void runMutation({
    run: () => layerMergeSelected(spriteId, [...ids]),
    invalidate: ["layers"],
  });
}

export function flattenVisibleLayers(spriteId: SpriteId): void {
  void runMutation({
    run: () => layerFlattenVisible(spriteId),
    invalidate: ["layers"],
  });
}
