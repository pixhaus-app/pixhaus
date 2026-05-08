// Bridge: active layer -> activeTilemapCtx.
//
// The tilemap panel and tile-paint code path are gated behind
// `activeTilemapCtx`. Without this bridge it stays null, leaving the
// panel content (header, Tileset/Rules tabs, tool row) hidden and the
// pencil tool no-op'd on tilemap layers — so converting a layer or
// clicking on an existing tilemap layer "does nothing" visibly.

import { createEffect, untrack } from "solid-js";
import { activeLayerId, activeSpriteId } from "../canvas/canvas-state";
import { layers } from "../layers/layer-state";
import { tilesetList } from "../lib/commands/tilesets";
import { reportCommandFailure } from "../lib/utils/errors";
import { activeTilemapCtx, setActiveTilemapCtx } from "./tilemap-state";

// One-way reactive bridge. Must be called from a reactive root (e.g.
// inside a component setup); the effect lives for the owning scope's
// lifetime and re-runs whenever the active sprite, active layer, or
// the cached layer list changes.
export function installTilemapCtxSync(): void {
  // Stale-response guard for tilesetList — same pattern as
  // refreshLayers in layer-state.ts. Without it, a slow first fetch
  // can land after the user clicks away to a non-tilemap layer.
  let token = 0;

  createEffect(() => {
    const sid = activeSpriteId();
    const lid = activeLayerId();
    const all = layers();

    token += 1;
    const myToken = token;

    if (sid === null || lid === null) {
      setActiveTilemapCtx(null);
      return;
    }

    const layer = all.find((l) => l.id === lid);
    if (!layer || layer.kind.kind !== "tilemap") {
      setActiveTilemapCtx(null);
      return;
    }

    const tilesetId = layer.kind.tileset;
    const cur = untrack(() => activeTilemapCtx());

    if (cur !== null && cur.tilesetId === tilesetId) {
      // Same tileset; just refresh the bound layerId in case the
      // user switched between two tilemap layers backed by it.
      if (cur.layerId !== lid) {
        setActiveTilemapCtx({ ...cur, layerId: lid });
      }
      return;
    }

    tilesetList(sid)
      .then((list) => {
        if (myToken !== token) return;
        const ts = list.find((t) => t.id === tilesetId);
        if (!ts) {
          setActiveTilemapCtx(null);
          return;
        }
        setActiveTilemapCtx({ layerId: lid, tilesetId, tileset: ts });
      })
      .catch((err: unknown) => {
        if (myToken !== token) return;
        reportCommandFailure("load tileset", err);
        setActiveTilemapCtx(null);
      });
  });
}
