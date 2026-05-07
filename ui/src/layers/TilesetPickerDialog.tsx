// Tileset picker dialog for "Convert to Tilemap Layer".
//
// Lists existing tilesets on the active sprite. If none exist, the user can
// create one inline without leaving the dialog. On confirm, calls
// convertLayerToTilemap with the chosen tileset id.

import { type Component, For, Show, createEffect, createSignal, onCleanup } from "solid-js";
import type { Tileset, TilesetId } from "../lib/types";
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

  // Load tilesets whenever the dialog opens. If the sprite has none, drop
  // straight into the create form so the user doesn't see an empty list
  // and have to find a button.
  createEffect(() => {
    const t = target();
    if (t === null) return;
    setError(null);
    tilesetList(t.spriteId)
      .then((list) => {
        setTilesets(list);
        if (list.length === 0) {
          setCreating(true);
          setNewName("Tileset 1");
        } else {
          setCreating(false);
          // Pre-select the first tileset so Convert is enabled by default.
          setSelectedId(list[0]?.id ?? null);
        }
      })
      .catch((err: unknown) => {
        console.error("[pixhaus] tileset_list:", err);
        setError("Failed to load tilesets");
      });
  });

  // Escape closes the dialog. Mounted via createEffect so the listener is
  // installed only while the dialog is open.
  createEffect(() => {
    if (target() === null) return;
    function onKeyDown(e: KeyboardEvent): void {
      if (e.key === "Escape") {
        e.preventDefault();
        cancel();
      }
    }
    document.addEventListener("keydown", onKeyDown);
    onCleanup(() => document.removeEventListener("keydown", onKeyDown));
  });

  function cancel(): void {
    setCreating(false);
    setNewName("");
    setSelectedId(null);
    setError(null);
    closeTilesetPicker();
  }

  function onBackdropClick(e: MouseEvent): void {
    if (e.target === e.currentTarget) cancel();
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
    try {
      const created = await tilesetAdd({
        sprite_id: t.spriteId,
        name,
        tile_width: w,
        tile_height: h,
      });
      const list = await tilesetList(t.spriteId);
      setTilesets(list);
      setSelectedId(created.id);
      setCreating(false);
    } catch (err: unknown) {
      console.error("[pixhaus] tileset_add:", err);
      setError("Failed to create tileset");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Show when={target() !== null}>
      <div class="prefs-backdrop" onClick={onBackdropClick}>
        <div class="prefs ts-picker" role="dialog" aria-label="Choose Tileset" aria-modal="true">
          <div class="prefs__header">
            <h2 class="prefs__title">Convert to Tilemap Layer</h2>
            <button class="prefs__close" onClick={cancel} aria-label="Close">
              ✕
            </button>
          </div>

          <div class="prefs__body">
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
                    autofocus
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
                  <p class="ts-picker__error">{error()}</p>
                </Show>
                <div class="ts-picker__create-actions">
                  <Show when={tilesets().length > 0}>
                    <button
                      type="button"
                      class="prefs__btn prefs__btn--ghost"
                      onClick={() => {
                        setCreating(false);
                        setError(null);
                      }}
                    >
                      Back to list
                    </button>
                  </Show>
                  <button type="submit" class="prefs__btn" disabled={submitting()}>
                    {submitting() ? "Creating…" : "Create"}
                  </button>
                </div>
              </form>
            </Show>
          </div>

          <Show when={!creating()}>
            <div class="prefs__footer ts-picker__footer">
              <button
                class="prefs__btn prefs__btn--ghost"
                onClick={() => {
                  setError(null);
                  setNewName(`Tileset ${tilesets().length + 1}`);
                  setCreating(true);
                }}
              >
                Create new tileset…
              </button>
              <div class="ts-picker__footer-spacer" />
              <button class="prefs__btn prefs__btn--ghost" onClick={cancel}>
                Cancel
              </button>
              <button
                class="prefs__btn"
                onClick={confirm}
                disabled={selectedId() === null}
                title={selectedId() === null ? "Pick a tileset first" : undefined}
              >
                Convert
              </button>
            </div>
          </Show>
        </div>
      </div>
    </Show>
  );
};

export default TilesetPickerDialog;
