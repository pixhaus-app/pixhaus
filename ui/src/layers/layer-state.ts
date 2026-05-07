// Reactive state for the layer panel.
//
// Layer data lives in Rust; this module caches the last-fetched list and
// tracks UI-only state (selection, renaming, drag target, panel visibility).
// Mutations go through the command wrappers, which call Rust and then refresh
// the list via refreshLayers().

import { createSignal } from "solid-js";
import type { BlendMode, Layer, LayerId, SpriteId, TilesetId } from "../lib/types";
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
  layerSetLocked,
  layerSetOpacity,
  layerSetParent,
  layerSetVisibility,
  layerWrapInGroup,
} from "../lib/commands/layers";
import {
  activeSpriteId,
  activeLayerId,
  setActiveLayerId,
  scheduleViewportSync,
} from "../canvas/canvas-state";

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

// ── Panel visibility ────────────────────────────────────────────────────────

export const [isLayerPanelVisible, setLayerPanelVisible] = createSignal(true);

// ── Layer list cache ────────────────────────────────────────────────────────

// Flat list ordered bottom-to-top (index 0 = bottom layer), matching Rust's order.
// The panel renders in reverse to show topmost layers at the top of the list.
export const [layers, setLayers] = createSignal<Layer[]>([]);

// Monotonically incremented on every refreshLayers() call. The async
// layerList() resolves out-of-order if the active sprite changes
// between two refreshes; by capturing the token we started with and
// comparing against the current value, late responses get dropped
// instead of overwriting the new sprite's layer list.
let refreshToken = 0;

export function refreshLayers(): void {
  refreshToken += 1;
  const myToken = refreshToken;
  const spriteId = activeSpriteId();
  if (spriteId === null) {
    setLayers([]);
    return;
  }
  layerList(spriteId)
    .then((next) => {
      if (myToken !== refreshToken) return; // stale
      setLayers(next);
      ensureActiveLayer(next);
    })
    .catch((err: unknown) => {
      console.error("[pixhaus] layer_list:", err);
    });
}

/**
 * Picks a sensible default for `activeLayerId` when the previous one is
 * gone (or never set). Without an active layer, every paint / transform /
 * select-on-layer command silently fails its `if (layerId === null) return`
 * guard and the user sees "nothing happens." Called from `refreshLayers`
 * after the new list lands, so the auto-pick lines up with the rendered
 * panel state.
 */
function ensureActiveLayer(list: readonly Layer[]): void {
  if (list.length === 0) {
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

// Multi-select set. The "primary" active layer (for painting) is activeLayerId()
// from canvas-state. selectedLayerIds tracks which rows are highlighted in the panel.
export const [selectedLayerIds, setSelectedLayerIds] = createSignal<ReadonlySet<LayerId>>(
  new Set(),
);

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

export const [renamingLayerId, setRenamingLayerId] = createSignal<LayerId | null>(null);

export function beginRename(id: LayerId): void {
  setRenamingLayerId(id);
}

export function commitRename(id: LayerId, name: string): void {
  const spriteId = activeSpriteId();
  if (spriteId === null) return;
  setRenamingLayerId(null);
  const trimmed = name.trim();
  if (!trimmed) return;
  layerRename(spriteId, id, trimmed)
    .then(() => refreshLayers())
    .catch((err: unknown) => console.error("[pixhaus] layer_rename:", err));
}

export function cancelRename(): void {
  setRenamingLayerId(null);
}

// ── Expanded groups ─────────────────────────────────────────────────────────

// Groups default to expanded. Track collapsed ones explicitly.
const [collapsedGroups, setCollapsedGroups] = createSignal<ReadonlySet<LayerId>>(new Set());

export function isGroupExpanded(id: LayerId): boolean {
  return !collapsedGroups().has(id);
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

// ── Drag-to-reorder state ───────────────────────────────────────────────────

// Index (in the flat layers array) where the drop indicator should render.
export const [dragOverIndex, setDragOverIndex] = createSignal<number | null>(null);

// ── Tileset picker (Convert to Tilemap Layer) ───────────────────────────────

// When non-null, a tileset picker dialog is open and Convert-to-Tilemap is
// pending the user's tileset choice. Set by the layer context menu, cleared
// by the dialog on confirm/cancel.
export type TilesetPickerTarget = {
  spriteId: SpriteId;
  layerId: LayerId;
};
export const [tilesetPickerTarget, setTilesetPickerTarget] =
  createSignal<TilesetPickerTarget | null>(null);

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
  layerAdd({ sprite_id: spriteId, name, kind: { kind: "raster" } })
    .then((layer) => {
      refreshLayers();
      selectLayer(layer.id, false);
    })
    .catch((err: unknown) => console.error("[pixhaus] layer_add:", err));
}

export function deleteLayer(spriteId: SpriteId, id: LayerId): void {
  layerDelete(spriteId, id)
    .then(() => {
      refreshLayers();
      // Clear the active layer if it was the one deleted.
      if (activeLayerId() === id) setActiveLayerId(null);
      setSelectedLayerIds((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    })
    .catch((err: unknown) => console.error("[pixhaus] layer_delete:", err));
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
    for (const r of results) {
      if (r.status === "rejected") {
        console.error("[pixhaus] layer_delete batch:", r.reason);
      }
    }
    refreshLayers();
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
  layerReorder(spriteId, id, newIndex)
    .then(() => refreshLayers())
    .catch((err: unknown) => console.error("[pixhaus] layer_reorder:", err));
}

export function setLayerVisibility(spriteId: SpriteId, id: LayerId, visible: boolean): void {
  layerSetVisibility(spriteId, id, visible)
    .then(() => refreshLayers())
    .catch((err: unknown) => console.error("[pixhaus] layer_set_visibility:", err));
}

export function setLayerLocked(spriteId: SpriteId, id: LayerId, locked: boolean): void {
  layerSetLocked(spriteId, id, locked)
    .then(() => refreshLayers())
    .catch((err: unknown) => console.error("[pixhaus] layer_set_locked:", err));
}

export function setLayerOpacity(spriteId: SpriteId, id: LayerId, opacity: number): void {
  layerSetOpacity(spriteId, id, opacity)
    .then(() => refreshLayers())
    .catch((err: unknown) => console.error("[pixhaus] layer_set_opacity:", err));
}

export function setLayerBlendMode(spriteId: SpriteId, id: LayerId, blendMode: BlendMode): void {
  layerSetBlendMode(spriteId, id, blendMode)
    .then(() => refreshLayers())
    .catch((err: unknown) => console.error("[pixhaus] layer_set_blend_mode:", err));
}

export function reparentLayer(spriteId: SpriteId, id: LayerId, parentId: LayerId | null): void {
  layerSetParent(spriteId, id, parentId)
    .then(() => refreshLayers())
    .catch((err: unknown) => console.error("[pixhaus] layer_set_parent:", err));
}

export function wrapLayersInGroup(spriteId: SpriteId, layerIds: readonly LayerId[]): void {
  if (layerIds.length === 0) return;
  layerWrapInGroup(spriteId, [...layerIds])
    .then((newGroup) => {
      refreshLayers();
      // Auto-expand the new group so its children (the wrapped layers)
      // are immediately visible — and so the group is a valid drop
      // target for additional layers without an extra chevron click.
      ensureGroupExpanded(newGroup.id);
    })
    .catch((err: unknown) => console.error("[pixhaus] layer_wrap_in_group:", err));
}

export function convertLayerToTilemap(spriteId: SpriteId, id: LayerId, tilesetId: TilesetId): void {
  layerConvertToTilemap(spriteId, id, tilesetId)
    .then(() => refreshLayers())
    .catch((err: unknown) => console.error("[pixhaus] layer_convert_to_tilemap:", err));
}

export function mergeLayerDown(spriteId: SpriteId, id: LayerId): void {
  layerMergeDown(spriteId, id)
    .then(() => refreshLayers())
    .catch((err: unknown) => console.error("[pixhaus] layer_merge_down:", err));
}

export function mergeSelectedLayers(spriteId: SpriteId, ids: ReadonlySet<LayerId>): void {
  layerMergeSelected(spriteId, [...ids])
    .then(() => refreshLayers())
    .catch((err: unknown) => console.error("[pixhaus] layer_merge_selected:", err));
}

export function flattenVisibleLayers(spriteId: SpriteId): void {
  layerFlattenVisible(spriteId)
    .then(() => refreshLayers())
    .catch((err: unknown) => console.error("[pixhaus] layer_flatten_visible:", err));
}
