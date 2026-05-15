// Tileset picker dialog for "Convert to Tilemap Layer".
//
// Lists existing tilesets on the active sprite. If none exist, the user can
// create one inline without leaving the dialog. On confirm, calls
// convertLayerToTilemap with the chosen tileset id.

import { type Component, For, Show, createEffect, createSignal } from "solid-js";
import type { Tileset, TilesetId } from "../lib/types";
import { Button } from "../lib/ui/Button";
import { Dialog } from "../lib/ui/Dialog";
import { tilesetAdd, tilesetList } from "../lib/commands/tilesets";
import { closeTilesetPicker, convertLayerToTilemap, tilesetPickerTarget } from "./layer-state";

const DEFAULT_TILE_SIZE = 16;

const TilesetPickerDialog: Component = () => {
  const target = () => tilesetPickerTarget();

  const [tilesets, setTilesets] = createSignal<Tileset[]>([]);
  const [selectedId, setSelectedId] = createSignal<TilesetId | null>(null);
  const [creating, setCreating] = createSignal(false);
  const [newName, setNewName] = createSignal("");
  const [newTileWidth, setNewTileWidth] = createSignal(DEFAULT_TILE_SIZE);
  const [newTileHeight, setNewTileHeight] = createSignal(DEFAULT_TILE_SIZE);
  const [submitting, setSubmitting] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  function isCurrentRequest(spriteId: number): boolean {
    const cur = tilesetPickerTarget();
    return cur !== null && cur.spriteId === spriteId;
  }

  createEffect(() => {
    const t = target();
    if (t === null) return;
    setError(null);
    const requestSpriteId = t.spriteId;
    tilesetList(requestSpriteId)
      .then((list) => {
        if (!isCurrentRequest(requestSpriteId)) return;
        setTilesets(list);
        if (list.length === 0) {
          setCreating(true);
          setNewName("Tileset 1");
        } else {
          setCreating(false);
          setSelectedId(list[0]?.id ?? null);
        }
      })
      .catch((err: unknown) => {
        if (!isCurrentRequest(requestSpriteId)) return;
        console.error("[pixhaus] tileset_list:", err);
        setError("Failed to load tilesets");
      });
  });

  function cancel(): void {
    setCreating(false);
    setNewName("");
    setSelectedId(null);
    setError(null);
    closeTilesetPicker();
  }

  function confirm(): void {
    const t = target();
    const id = selectedId();
    if (t === null || id === null) return;
    convertLayerToTilemap(t.spriteId, t.layerId, id);
    cancel();
  }

  async function submitCreate(e: SubmitEvent): Promise<void> {
    e.preventDefault();
    const t = target();
    if (t === null) return;
    const name = newName().trim();
    if (!name) {
      setError("Name is required");
      return;
    }
    const w = newTileWidth();
    const h = newTileHeight();
    if (!Number.isFinite(w) || w <= 0 || !Number.isFinite(h) || h <= 0) {
      setError("Tile size must be positive");
      return;
    }
    setSubmitting(true);
    setError(null);
    const requestSpriteId = t.spriteId;
    try {
      const created = await tilesetAdd({
        sprite_id: requestSpriteId,
        name,
        tile_width: w,
        tile_height: h,
      });
      const list = await tilesetList(requestSpriteId);
      if (!isCurrentRequest(requestSpriteId)) return;
      setTilesets(list);
      setSelectedId(created.id);
      setCreating(false);
    } catch (err: unknown) {
      if (!isCurrentRequest(requestSpriteId)) return;
      console.error("[pixhaus] tileset_add:", err);
      setError("Failed to create tileset");
    } finally {
      if (isCurrentRequest(requestSpriteId)) setSubmitting(false);
    }
  }

  return (
    <Dialog open={target() !== null} title="Convert to tilemap layer" onClose={cancel} size="md">
      <Dialog.Body>
        <Show
          when={creating()}
          fallback={
            <Show
              when={tilesets().length > 0}
              fallback={<p class="prefs__sublabel">No tilesets yet. Create one below.</p>}
            >
              <p class="prefs__section-title">Pick a tileset</p>
              <div class="ts-picker__list" role="radiogroup">
                <For each={tilesets()}>
                  {(ts) => (
                    <label
                      class="ts-picker__row"
                      classList={{ "ts-picker__row--selected": selectedId() === ts.id }}
                    >
                      <input
                        type="radio"
                        name="ts-picker"
                        checked={selectedId() === ts.id}
                        onChange={() => setSelectedId(ts.id)}
                      />
                      <span class="ts-picker__name">{ts.name}</span>
                      <span class="ts-picker__meta">
                        {ts.tile_size.width}×{ts.tile_size.height}, {ts.tile_count} tiles
                      </span>
                    </label>
                  )}
                </For>
              </div>
            </Show>
          }
        >
          <p class="prefs__section-title">Create a new tileset</p>
          <form onSubmit={submitCreate}>
            <div class="prefs__row">
              <div>
                <div class="prefs__label">Name</div>
              </div>
              <input
                class="ts-picker__input"
                value={newName()}
                onInput={(e) => setNewName(e.currentTarget.value)}
                placeholder="Tileset 1"
              />
            </div>
            <div class="prefs__row">
              <div>
                <div class="prefs__label">Tile size</div>
                <div class="prefs__sublabel">Width × height in pixels</div>
              </div>
              <div class="ts-picker__size">
                <input
                  class="ts-picker__input ts-picker__input--num"
                  type="number"
                  min="1"
                  value={newTileWidth()}
                  onInput={(e) => setNewTileWidth(parseInt(e.currentTarget.value, 10) || 0)}
                />
                <span aria-hidden="true">×</span>
                <input
                  class="ts-picker__input ts-picker__input--num"
                  type="number"
                  min="1"
                  value={newTileHeight()}
                  onInput={(e) => setNewTileHeight(parseInt(e.currentTarget.value, 10) || 0)}
                />
              </div>
            </div>
            <Show when={error() !== null}>
              <p class="form-field__error" role="alert">
                {error()}
              </p>
            </Show>
            <div class="ts-picker__create-actions">
              <Show when={tilesets().length > 0}>
                <Button
                  variant="ghost"
                  onClick={() => {
                    setCreating(false);
                    setError(null);
                  }}
                >
                  Back to list
                </Button>
              </Show>
              <Button type="submit" loading={submitting()}>
                {submitting() ? "Creating" : "Create"}
              </Button>
            </div>
          </form>
        </Show>
      </Dialog.Body>

      <Show when={!creating()}>
        <Dialog.Footer>
          <Button
            variant="ghost"
            onClick={() => {
              setError(null);
              setNewName(`Tileset ${tilesets().length + 1}`);
              setCreating(true);
            }}
            class="ts-picker__new-btn"
          >
            New tileset
          </Button>
          <Button variant="ghost" onClick={cancel}>
            Cancel
          </Button>
          <Button
            onClick={confirm}
            disabled={selectedId() === null}
            title={selectedId() === null ? "Pick a tileset first" : undefined}
          >
            Convert to tileset
          </Button>
        </Dialog.Footer>
      </Show>
    </Dialog>
  );
};

export default TilesetPickerDialog;
