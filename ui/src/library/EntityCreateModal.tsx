// Modal for creating a new library entity.
//
// Adapts its form fields to the selected kind. Tileset and Tilemap entities
// require dimension inputs; Custom entities require a category and canvas
// size; Reference entities require an image file.

import { type Component, Show, createSignal, For } from "solid-js";
import { open as dialogOpen } from "../lib/dialog";
import type { EntityKind, GroupId } from "../lib/types";
import { groups, createEntity } from "./library-state";

// Common category suggestions for Custom entities. Free-form is still valid.
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

// Conventional initial state names for Custom entities.
const STATE_SUGGESTIONS = ["idle", "walk", "run", "jump", "attack", "hurt", "death"];

type Props = {
  onClose: () => void;
  initialGroupId?: GroupId | null;
};

const EntityCreateModal: Component<Props> = (props) => {
  const [kindTag, setKindTag] = createSignal<"Tileset" | "Tilemap" | "Reference" | "Custom">(
    "Custom",
  );
  const [name, setName] = createSignal("");
  const [category, setCategory] = createSignal("Character");
  const [groupId, setGroupId] = createSignal<GroupId | null>(props.initialGroupId ?? null);

  // Custom fields
  const [canvasWidth, setCanvasWidth] = createSignal(32);
  const [canvasHeight, setCanvasHeight] = createSignal(32);
  const [stateList, setStateList] = createSignal("idle");

  // Tileset fields
  const [tileWidth, setTileWidth] = createSignal(16);
  const [tileHeight, setTileHeight] = createSignal(16);

  // Tilemap fields
  const [sceneWidth, setSceneWidth] = createSignal(20);
  const [sceneHeight, setSceneHeight] = createSignal(15);

  // Reference fields — we store file bytes after loading
  const [refBytes, setRefBytes] = createSignal<number[] | null>(null);
  const [refMime, setRefMime] = createSignal("image/png");
  const [refFileName, setRefFileName] = createSignal<string | null>(null);

  const [error, setError] = createSignal<string | null>(null);

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

    // Read the file via fetch (Tauri exposes local files via the asset
    // protocol; use convertFileSrc for the local path).
    try {
      const { convertFileSrc } = await import("@tauri-apps/api/core");
      const src = convertFileSrc(path);
      const resp = await fetch(src);
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
    if (k === "Reference") {
      if (!refBytes()) return "Select an image file.";
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
      createEntity({
        ...base,
        canvas_width: canvasWidth(),
        canvas_height: canvasHeight(),
        initial_states: parseStates(),
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
    } else if (k === "Reference") {
      createEntity({
        ...base,
        reference_bytes: refBytes()!,
        reference_mime: refMime(),
      });
    }

    props.onClose();
  }

  function handleKeyDown(e: KeyboardEvent): void {
    if (e.key === "Escape") props.onClose();
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) handleSubmit();
  }

  return (
    <div class="modal-backdrop" onKeyDown={handleKeyDown}>
      <div class="modal" role="dialog" aria-label="Create entity" data-testid="entity-create-modal">
        <div class="modal__header">
          <span class="modal__title">New entity</span>
          <button class="modal__close-btn" onClick={props.onClose} title="Close">
            <svg
              width="10"
              height="10"
              viewBox="0 0 10 10"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
            >
              <path d="M1 1 L9 9 M9 1 L1 9" />
            </svg>
          </button>
        </div>

        <div class="modal__body">
          {/* Kind selector */}
          <div class="modal__field">
            <label class="modal__label">Kind</label>
            <div class="modal__kind-tabs">
              <For each={["Custom", "Tileset", "Tilemap", "Reference"] as const}>
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

          {/* Category (Custom only) */}
          <Show when={kindTag() === "Custom"}>
            <div class="modal__field">
              <label class="modal__label">Category</label>
              <input
                class="modal__input"
                list="category-suggestions"
                value={category()}
                onInput={(e) => setCategory(e.currentTarget.value)}
                placeholder="Character, Enemy, Prop..."
              />
              <datalist id="category-suggestions">
                <For each={CATEGORY_SUGGESTIONS}>{(s) => <option value={s} />}</For>
              </datalist>
            </div>
          </Show>

          {/* Name */}
          <div class="modal__field">
            <label class="modal__label">Name</label>
            <input
              class="modal__input"
              value={name()}
              onInput={(e) => setName(e.currentTarget.value)}
              placeholder={kindTag() === "Custom" ? "Hero, Goblin..." : "Forest, Level-1..."}
              autofocus
            />
          </div>

          {/* Group */}
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

          {/* Custom: canvas size + states */}
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
          </Show>

          {/* Tileset: tile size */}
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

          {/* Tilemap: scene size */}
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

          {/* Reference: file picker */}
          <Show when={kindTag() === "Reference"}>
            <div class="modal__field">
              <label class="modal__label">Source image</label>
              <div class="modal__file-row">
                <button class="modal__file-btn" type="button" onClick={handlePickRef}>
                  Choose file...
                </button>
                <Show when={refFileName() !== null}>
                  <span class="modal__file-name">{refFileName()}</span>
                </Show>
              </div>
            </div>
          </Show>

          {/* Error */}
          <Show when={error() !== null}>
            <p class="modal__error">{error()}</p>
          </Show>
        </div>

        <div class="modal__footer">
          <button class="modal__btn modal__btn--ghost" type="button" onClick={props.onClose}>
            Cancel
          </button>
          <button class="modal__btn modal__btn--primary" type="button" onClick={handleSubmit}>
            Create
          </button>
        </div>
      </div>
    </div>
  );
};

export default EntityCreateModal;
