// Tilemap side panel.
//
// Shown when a sprite is active.  Contains three tabs:
//   Tileset  — tile grid browser and flip/rotation controls (TilesetPanel)
//   Rules    — autotile rule editor (AutotileRuleEditor)
//   Tilesets — CRUD management: list, create, rename tilesets
//
// The panel also hosts the tile-tool toggle (pencil / erase) and the
// autotile-mode checkbox (shown only when a tileset is active).

import {
  For,
  Show,
  createMemo,
  createResource,
  createSignal,
  createEffect,
  type Component,
} from "solid-js";
import type { TileProperties, Tileset, TilesetId, SpriteId } from "../lib/types";
import { activeSpriteId } from "../canvas/canvas-state";
import {
  activeTilemapCtx,
  setActiveTilemapCtx,
  tilemapTool,
  setTilemapTool,
  autotileMode,
  setAutotileMode,
  resetTileSelection,
} from "./tilemap-state";
import { tilesetList, tilesetAdd, tilesetRename, tilesetSetTileMetadata } from "../lib/commands";
import { reportCommandFailure } from "../lib/utils/errors";
import TilesetPanel from "./TilesetPanel";
import AutotileRuleEditor from "./AutotileRuleEditor";

// ── Tab type ──────────────────────────────────────────────────────────────

type Tab = "tileset" | "rules" | "tilesets";

// ── Tileset management tab ────────────────────────────────────────────────

interface TilesetsTabProps {
  spriteId: SpriteId;
  activeId: TilesetId | null;
  onSwitch: (ts: Tileset) => void;
  onCreated: (ts: Tileset) => void;
  onTilesetRenamed: (ts: Tileset) => void;
}

const TilesetsTab: Component<TilesetsTabProps> = (props) => {
  const [tilesets, { refetch }] = createResource(
    () => props.spriteId,
    (sid) => tilesetList(sid),
  );

  // Add-tileset form state.
  const [adding, setAdding] = createSignal(false);
  const [addName, setAddName] = createSignal("");
  const [addW, setAddW] = createSignal(16);
  const [addH, setAddH] = createSignal(16);

  async function submitAdd(e: SubmitEvent) {
    e.preventDefault();
    const name = addName().trim();
    if (!name) return;
    const created = await tilesetAdd({
      sprite_id: props.spriteId,
      name,
      tile_width: addW(),
      tile_height: addH(),
    });
    await refetch();
    props.onCreated(created);
    setAdding(false);
    setAddName("");
    setAddW(16);
    setAddH(16);
  }

  // Inline rename state.
  const [renamingId, setRenamingId] = createSignal<TilesetId | null>(null);
  const [renameValue, setRenameValue] = createSignal("");

  function startRename(ts: Tileset) {
    setRenamingId(ts.id);
    setRenameValue(ts.name);
  }

  async function commitRename(ts: Tileset) {
    const name = renameValue().trim();
    if (name && name !== ts.name) {
      await tilesetRename(props.spriteId, ts.id, name);
      await refetch();
      // If the renamed tileset is the currently-active context, the
      // header still shows the old name — propagate the new Tileset
      // object up to refresh activeTilemapCtx.
      if (props.activeId === ts.id) {
        const updated = (tilesets() ?? []).find((t) => t.id === ts.id);
        if (updated) props.onTilesetRenamed(updated);
      }
    }
    setRenamingId(null);
  }

  return (
    <div class="tm-manage">
      <Show when={!tilesets.loading} fallback={<p class="tm-manage__loading">Loading…</p>}>
        <div class="tm-manage__list">
          <For
            each={tilesets() ?? []}
            fallback={<p class="tm-manage__empty">No tilesets. Create one below.</p>}
          >
            {(ts) => (
              <div
                class="tm-manage__row"
                classList={{ "tm-manage__row--active": ts.id === props.activeId }}
              >
                <Show
                  when={renamingId() === ts.id}
                  fallback={
                    <button
                      class="tm-manage__ts-name"
                      title="Switch to this tileset"
                      onClick={() => props.onSwitch(ts)}
                    >
                      {ts.name}
                    </button>
                  }
                >
                  <input
                    class="tm-manage__rename-input"
                    value={renameValue()}
                    onInput={(e) => setRenameValue(e.currentTarget.value)}
                    onBlur={() => commitRename(ts)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") commitRename(ts);
                      if (e.key === "Escape") setRenamingId(null);
                    }}
                  />
                </Show>
                <span class="tm-manage__ts-size">
                  {ts.tile_size.width}&#xd7;{ts.tile_size.height}px
                </span>
                <button
                  class="tm-manage__rename-btn"
                  title="Rename"
                  onClick={() => startRename(ts)}
                >
                  Rename
                </button>
              </div>
            )}
          </For>
        </div>
      </Show>

      <Show
        when={adding()}
        fallback={
          <button class="tm-manage__add-btn" onClick={() => setAdding(true)}>
            + New tileset
          </button>
        }
      >
        <form class="tm-manage__add-form" onSubmit={submitAdd}>
          <input
            class="tm-manage__add-name"
            placeholder="Name"
            value={addName()}
            onInput={(e) => setAddName(e.currentTarget.value)}
            autofocus
          />
          <div class="tm-manage__add-size">
            <label>
              W
              <input
                class="tm-manage__add-dim"
                type="number"
                min={1}
                value={addW()}
                onInput={(e) => setAddW(Math.max(1, parseInt(e.currentTarget.value, 10) || 1))}
              />
            </label>
            <label>
              H
              <input
                class="tm-manage__add-dim"
                type="number"
                min={1}
                value={addH()}
                onInput={(e) => setAddH(Math.max(1, parseInt(e.currentTarget.value, 10) || 1))}
              />
            </label>
          </div>
          <div class="tm-manage__add-actions">
            <button type="submit">Create</button>
            <button type="button" onClick={() => setAdding(false)}>
              Cancel
            </button>
          </div>
        </form>
      </Show>
    </div>
  );
};

// ── Root component ─────────────────────────────────────────────────────────

const TilemapPanel: Component = () => {
  const ctx = activeTilemapCtx;
  const tileset = createMemo(() => ctx()?.tileset ?? null);
  const spriteId = activeSpriteId;

  // Default to the tab that's actually visible: when no tileset is
  // active the Tileset and Rules tabs are gated behind a Show, leaving
  // a blank body if `activeTab` defaults there. Pick `tilesets` so the
  // empty-state user lands on the create-new form.
  const [activeTab, setActiveTab] = createSignal<Tab>(ctx() === null ? "tilesets" : "tileset");

  // Reset selection when the context switches to a different tileset.
  createEffect(() => {
    const c = ctx();
    if (c) resetTileSelection();
  });

  // Persists a tile-property change via IPC, then refreshes the active
  // tilemap context with the updated tileset so the panel reflects the
  // toggle across re-renders.
  async function onTilePropertiesChange(tileIndex: number, propsToSave: TileProperties) {
    const sid = spriteId();
    const c = ctx();
    if (sid === null || c === null) return;
    try {
      const updated = await tilesetSetTileMetadata({
        sprite_id: sid,
        tileset_id: c.tilesetId,
        tile_index: tileIndex,
        metadata: propsToSave,
      });
      setActiveTilemapCtx({ ...c, tileset: updated });
    } catch (err) {
      reportCommandFailure("tile metadata update", err);
    }
  }

  function switchTileset(ts: Tileset) {
    const c = ctx();
    if (!c) return;
    setActiveTilemapCtx({ ...c, tilesetId: ts.id, tileset: ts });
    setActiveTab("tileset");
  }

  function onTilesetCreated(ts: Tileset) {
    // When a tilemap layer is already active, treat the just-created
    // tileset as the new working set: rebind ctx and jump to the
    // Tileset tab. With no active tilemap layer there's no layerId to
    // bind to, so leave the user on the Tilesets tab — the new row
    // shows up highlighted via the activeId prop.
    const c = ctx();
    if (c) {
      switchTileset(ts);
    }
  }

  function onTilesetRenamed(updated: Tileset) {
    const c = ctx();
    if (!c) return;
    setActiveTilemapCtx({ ...c, tileset: updated });
  }

  return (
    <Show when={spriteId() !== null}>
      <div class="tilemap-panel">
        {/* Header — only when a tileset is active */}
        <Show when={tileset() !== null}>
          <div class="tilemap-panel__header">
            <span class="tilemap-panel__title">{tileset()!.name}</span>
            <span class="tilemap-panel__sub">tileset</span>
          </div>

          {/* Tool row */}
          <div class="tilemap-panel__tool-row">
            <button
              class="tilemap-panel__tool-btn"
              classList={{ "tilemap-panel__tool-btn--active": tilemapTool() === "pencil" }}
              title="Pencil — place tile (P)"
              onClick={() => setTilemapTool("pencil")}
            >
              Pencil
            </button>
            <button
              class="tilemap-panel__tool-btn"
              classList={{ "tilemap-panel__tool-btn--active": tilemapTool() === "erase" }}
              title="Eraser — clear cell (E)"
              onClick={() => setTilemapTool("erase")}
            >
              Erase
            </button>
            <label class="tilemap-panel__autotile-toggle">
              <input
                type="checkbox"
                checked={autotileMode()}
                onChange={(e) => setAutotileMode(e.currentTarget.checked)}
              />
              Autotile
            </label>
          </div>
        </Show>

        {/* Tabs */}
        <div class="tilemap-panel__tabs" role="tablist">
          <Show when={tileset() !== null}>
            <button
              class="tilemap-panel__tab"
              classList={{ "tilemap-panel__tab--active": activeTab() === "tileset" }}
              role="tab"
              aria-selected={activeTab() === "tileset"}
              onClick={() => setActiveTab("tileset")}
            >
              Tileset
            </button>
            <button
              class="tilemap-panel__tab"
              classList={{ "tilemap-panel__tab--active": activeTab() === "rules" }}
              role="tab"
              aria-selected={activeTab() === "rules"}
              onClick={() => setActiveTab("rules")}
            >
              Rules
            </button>
          </Show>
          <button
            class="tilemap-panel__tab"
            classList={{ "tilemap-panel__tab--active": activeTab() === "tilesets" }}
            role="tab"
            aria-selected={activeTab() === "tilesets"}
            onClick={() => setActiveTab("tilesets")}
          >
            Tilesets
          </button>
        </div>

        {/* Tab body */}
        <div class="tilemap-panel__body">
          <Show when={activeTab() === "tileset" && tileset() !== null}>
            <TilesetPanel tileset={tileset()!} onTilePropertiesChange={onTilePropertiesChange} />
          </Show>
          <Show when={activeTab() === "rules" && tileset() !== null}>
            <AutotileRuleEditor />
          </Show>
          <Show when={activeTab() === "tilesets"}>
            <TilesetsTab
              spriteId={spriteId()!}
              activeId={ctx()?.tilesetId ?? null}
              onSwitch={switchTileset}
              onCreated={onTilesetCreated}
              onTilesetRenamed={onTilesetRenamed}
            />
          </Show>
        </div>
      </div>
    </Show>
  );
};

export default TilemapPanel;
