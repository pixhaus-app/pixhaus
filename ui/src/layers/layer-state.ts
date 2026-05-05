// Reactive state for the layer panel.
//
// Layer data lives in Rust; this module caches the last-fetched list and
// tracks UI-only state (selection, renaming, drag target, panel visibility).
// Mutations go through the command wrappers, which call Rust and then refresh
// the list via refreshLayers().

import { createSignal } from "solid-js";
import type { BlendMode, Layer, LayerId, SpriteId } from "../lib/types";
import {
  layerAdd,
  layerDelete,
  layerList,
  layerRename,
  layerReorder,
  layerSetBlendMode,
  layerSetLocked,
  layerSetOpacity,
  layerSetVisibility,
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
    })
    .catch((err: unknown) => {
      console.error("[pixhaus] layer_list:", err);
    });
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

// ── Drag-to-reorder state ───────────────────────────────────────────────────

// Index (in the flat layers array) where the drop indicator should render.
export const [dragOverIndex, setDragOverIndex] = createSignal<number | null>(null);

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
