// Modal for creating a new library entity.
//
// Adapts its form fields to the selected kind. Tileset and Tilemap entities
// require dimension inputs; Custom entities require a category and canvas
// size and can optionally attach a reference sheet image.

import { type Component, Show, createEffect, createSignal, For } from "solid-js";
import { open as dialogOpen } from "../lib/dialog";
import type { EntityKind, GroupId } from "../lib/types";
import { Button } from "../lib/ui/Button";
import { Dialog } from "../lib/ui/Dialog";
import { FormField } from "../lib/ui/FormField";
import { groups, createEntity } from "./library-state";
import { closeEntityCreate, entityCreateRequest } from "./entity-create-state";

const CATEGORY_SUGGESTIONS = [
  "Character",
  "Enemy",
  "NPC",
  "Prop",
  "Vehicle",
  "UI",
  "Effect",
  "Pet",
  "Mount",
  "Weapon",
  "Item",
];

const STATE_SUGGESTIONS = ["idle", "walk", "run", "jump", "attack", "hurt", "death"];

const EntityCreateModal: Component = () => {
  const [kindTag, setKindTag] = createSignal<"Tileset" | "Tilemap" | "Custom">("Custom");
  const [name, setName] = createSignal("");
  const [category, setCategory] = createSignal("Character");
  const [groupId, setGroupId] = createSignal<GroupId | null>(null);

  const [canvasWidth, setCanvasWidth] = createSignal(32);
  const [canvasHeight, setCanvasHeight] = createSignal(32);
  const [stateList, setStateList] = createSignal("idle");

  const [tileWidth, setTileWidth] = createSignal(16);
  const [tileHeight, setTileHeight] = createSignal(16);

  const [sceneWidth, setSceneWidth] = createSignal(20);
  const [sceneHeight, setSceneHeight] = createSignal(15);

  const [refBytes, setRefBytes] = createSignal<number[] | null>(null);
  const [refMime, setRefMime] = createSignal("image/png");
  const [refFileName, setRefFileName] = createSignal<string | null>(null);

  const [error, setError] = createSignal<string | null>(null);

  // Reset form when a new request comes in.
  createEffect(() => {
    const req = entityCreateRequest();
    if (req === null) return;
    setKindTag("Custom");
    setName("");
    setCategory("Character");
    setGroupId(req.initialGroupId);
    setCanvasWidth(32);
    setCanvasHeight(32);
    setStateList("idle");
    setTileWidth(16);
    setTileHeight(16);
    setSceneWidth(20);
    setSceneHeight(15);
    setRefBytes(null);
    setRefMime("image/png");
    setRefFileName(null);
    setError(null);
  });

  function buildKind(): EntityKind {
    const k = kindTag();
    if (k === "Custom") return { kind: "Custom", value: category().trim() || "Custom" };
    return { kind: k };
  }

  function parseStates(): string[] {
    return stateList()
      .split(/[,\n]+/)
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
  }

  async function handlePickRef(): Promise<void> {
    const result = await dialogOpen({
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif"] }],
      multiple: false,
    });
    const path = typeof result === "string" ? result : null;
    if (!path) return;

    try {
      const { convertFileSrc } = await import("@tauri-apps/api/core");
      const src = convertFileSrc(path);
      const resp = await fetch(src);
      if (!resp.ok) {
        throw new Error(`failed to read selected file: ${resp.status} ${resp.statusText}`);
      }
      const arrayBuf = await resp.arrayBuffer();
      const bytes = Array.from(new Uint8Array(arrayBuf));
      const ext = path.split(".").pop()?.toLowerCase() ?? "png";
      const mime = ext === "jpg" || ext === "jpeg" ? "image/jpeg" : `image/${ext}`;
      setRefBytes(bytes);
      setRefMime(mime);
      setRefFileName(path.split(/[\\/]/).pop() ?? path);
      setError(null);
    } catch (err) {
      setError("Failed to read image file.");
      console.error("[pixhaus] entity create ref read:", err);
    }
  }

  function validate(): string | null {
    if (!name().trim()) return "Name is required.";
    const k = kindTag();
    if (k === "Custom") {
      if (!category().trim()) return "Category is required.";
      if (canvasWidth() <= 0 || canvasHeight() <= 0) return "Canvas size must be greater than 0.";
      const states = parseStates();
      if (states.length === 0) return "At least one state is required.";
    }
    if (k === "Tileset") {
      if (tileWidth() <= 0 || tileHeight() <= 0) return "Tile size must be greater than 0.";
    }
    if (k === "Tilemap") {
      if (sceneWidth() <= 0 || sceneHeight() <= 0) return "Scene size must be greater than 0.";
    }
    return null;
  }

  function handleSubmit(): void {
    const err = validate();
    if (err) {
      setError(err);
      return;
    }

    const k = kindTag();
    const base = {
      kind: buildKind(),
      name: name().trim(),
      group_id: groupId(),
    };

    if (k === "Custom") {
      const referenceBytes = refBytes();
      createEntity({
        ...base,
        canvas_width: canvasWidth(),
        canvas_height: canvasHeight(),
        initial_states: parseStates(),
        ...(referenceBytes === null
          ? {}
          : { reference_bytes: referenceBytes, reference_mime: refMime() }),
      });
    } else if (k === "Tileset") {
      createEntity({
        ...base,
        tile_width: tileWidth(),
        tile_height: tileHeight(),
      });
    } else if (k === "Tilemap") {
      createEntity({
        ...base,
        scene_width: sceneWidth(),
        scene_height: sceneHeight(),
      });
    }

    closeEntityCreate();
  }

  function handleKeyDown(e: KeyboardEvent): void {
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) handleSubmit();
  }

  return (
    <Dialog
      open={entityCreateRequest() !== null}
      title="New entity"
      onClose={closeEntityCreate}
      size="sm"
      testId="entity-create-modal"
    >
      <div onKeyDown={handleKeyDown}>
        <Dialog.Body>
          <div class="modal__field">
            <label class="modal__label">Kind</label>
            <div class="modal__kind-tabs">
              <For each={["Custom", "Tileset", "Tilemap"] as const}>
                {(k) => (
                  <button
                    class="modal__kind-tab"
                    classList={{ "modal__kind-tab--active": kindTag() === k }}
                    onClick={() => setKindTag(k)}
                    type="button"
                  >
                    {k}
                  </button>
                )}
              </For>
            </div>
          </div>

          {/* Name first so Dialog.initialFocus="first-input" lands here
              for every kind, not on the Custom-only Category combobox. */}
          <FormField label="Name" for="entity-create-name">
            <input
              id="entity-create-name"
              class="modal__input"
              value={name()}
              onInput={(e) => setName(e.currentTarget.value)}
              placeholder={kindTag() === "Custom" ? "Hero, Goblin..." : "Forest, Level-1..."}
            />
          </FormField>

          <Show when={kindTag() === "Custom"}>
            <FormField label="Category" for="entity-create-category">
              <input
                id="entity-create-category"
                class="modal__input"
                list="category-suggestions"
                value={category()}
                onInput={(e) => setCategory(e.currentTarget.value)}
                placeholder="Character, Enemy, Prop..."
              />
              <datalist id="category-suggestions">
                <For each={CATEGORY_SUGGESTIONS}>{(s) => <option value={s} />}</For>
              </datalist>
            </FormField>
          </Show>

          <Show when={groups().length > 0}>
            <div class="modal__field">
              <label class="modal__label">Group</label>
              <select
                class="modal__select"
                value={groupId() ?? ""}
                onChange={(e) => {
                  const v = e.currentTarget.value;
                  setGroupId(v === "" ? null : (parseInt(v, 10) as GroupId));
                }}
              >
                <option value="">None</option>
                <For each={groups()}>{(g) => <option value={g.id}>{g.name}</option>}</For>
              </select>
            </div>
          </Show>

          <Show when={kindTag() === "Custom"}>
            <div class="modal__field modal__field--row">
              <div class="modal__field modal__field--grow">
                <label class="modal__label">Canvas width</label>
                <input
                  class="modal__input"
                  type="number"
                  min="1"
                  max="4096"
                  value={canvasWidth()}
                  onInput={(e) => setCanvasWidth(parseInt(e.currentTarget.value, 10) || 32)}
                />
              </div>
              <div class="modal__field modal__field--grow">
                <label class="modal__label">Canvas height</label>
                <input
                  class="modal__input"
                  type="number"
                  min="1"
                  max="4096"
                  value={canvasHeight()}
                  onInput={(e) => setCanvasHeight(parseInt(e.currentTarget.value, 10) || 32)}
                />
              </div>
            </div>

            <div class="modal__field">
              <label class="modal__label">Initial states</label>
              <input
                class="modal__input"
                list="state-suggestions"
                value={stateList()}
                onInput={(e) => setStateList(e.currentTarget.value)}
                placeholder="idle, walk, run, attack"
              />
              <datalist id="state-suggestions">
                <For each={STATE_SUGGESTIONS}>{(s) => <option value={s} />}</For>
              </datalist>
              <span class="modal__hint">Comma-separated list</span>
            </div>

            <div class="modal__field">
              <label class="modal__label">Reference sheet</label>
              <div class="modal__file-row">
                <Button variant="ghost" size="sm" onClick={handlePickRef}>
                  Choose file…
                </Button>
                <Show when={refFileName() !== null}>
                  <span class="modal__file-name">{refFileName()}</span>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => {
                      setRefBytes(null);
                      setRefMime("image/png");
                      setRefFileName(null);
                    }}
                  >
                    Clear
                  </Button>
                </Show>
              </div>
            </div>
          </Show>

          <Show when={kindTag() === "Tileset"}>
            <div class="modal__field modal__field--row">
              <div class="modal__field modal__field--grow">
                <label class="modal__label">Tile width</label>
                <input
                  class="modal__input"
                  type="number"
                  min="1"
                  max="256"
                  value={tileWidth()}
                  onInput={(e) => setTileWidth(parseInt(e.currentTarget.value, 10) || 16)}
                />
              </div>
              <div class="modal__field modal__field--grow">
                <label class="modal__label">Tile height</label>
                <input
                  class="modal__input"
                  type="number"
                  min="1"
                  max="256"
                  value={tileHeight()}
                  onInput={(e) => setTileHeight(parseInt(e.currentTarget.value, 10) || 16)}
                />
              </div>
            </div>
          </Show>

          <Show when={kindTag() === "Tilemap"}>
            <div class="modal__field modal__field--row">
              <div class="modal__field modal__field--grow">
                <label class="modal__label">Scene width (tiles)</label>
                <input
                  class="modal__input"
                  type="number"
                  min="1"
                  max="1000"
                  value={sceneWidth()}
                  onInput={(e) => setSceneWidth(parseInt(e.currentTarget.value, 10) || 20)}
                />
              </div>
              <div class="modal__field modal__field--grow">
                <label class="modal__label">Scene height (tiles)</label>
                <input
                  class="modal__input"
                  type="number"
                  min="1"
                  max="1000"
                  value={sceneHeight()}
                  onInput={(e) => setSceneHeight(parseInt(e.currentTarget.value, 10) || 15)}
                />
              </div>
            </div>
          </Show>

          <Show when={error() !== null}>
            <p class="form-field__error" role="alert">
              {error()}
            </p>
          </Show>
        </Dialog.Body>

        <Dialog.Footer>
          <Button variant="ghost" onClick={closeEntityCreate}>
            Cancel
          </Button>
          <Button onClick={handleSubmit}>Create entity</Button>
        </Dialog.Footer>
      </div>
    </Dialog>
  );
};

export default EntityCreateModal;
