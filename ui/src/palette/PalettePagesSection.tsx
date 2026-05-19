// Palette pages list and CRUD controls.
//
// Pages are named subset views over the active palette's swatches. This
// minimal surface lists each page, supports adding a new (empty) page,
// and removing an existing one. Inline rename and "assign current
// selection" are deferred — they need a swatch-selection state model
// the panel doesn't yet have.

import { For, Show, createSignal, type Component } from "solid-js";
import { activePalette, activePaletteId, refreshPalettes } from "./palette-panel-state";
import { palettePageAdd, palettePageRemove } from "../lib/commands/palette";
import { reportCommandFailure } from "../lib/utils/errors";
import type { SpriteId } from "../lib/types";

type Props = {
  /** The sprite owning the palette. Null when no project is open. */
  spriteId: SpriteId | null;
};

const PalettePagesSection: Component<Props> = (props) => {
  const [adding, setAdding] = createSignal(false);
  const [newName, setNewName] = createSignal("");

  const beginAdd = (): void => {
    setNewName("");
    setAdding(true);
  };

  const cancelAdd = (): void => {
    setAdding(false);
    setNewName("");
  };

  const submitAdd = async (): Promise<void> => {
    const sid = props.spriteId;
    const pid = activePaletteId();
    const name = newName().trim();
    if (sid === null || pid === null || name === "") {
      cancelAdd();
      return;
    }
    try {
      await palettePageAdd(sid, pid, name);
      await refreshPalettes(sid);
      cancelAdd();
    } catch (err: unknown) {
      reportCommandFailure("palette_page_add", err);
    }
  };

  const handleRemove = async (pageId: number): Promise<void> => {
    const sid = props.spriteId;
    const pid = activePaletteId();
    if (sid === null || pid === null) return;
    try {
      await palettePageRemove(sid, pid, pageId);
      await refreshPalettes(sid);
    } catch (err: unknown) {
      reportCommandFailure("palette_page_remove", err);
    }
  };

  return (
    <div class="pp__pages" data-testid="palette-pages-section">
      <div class="pp__pages-header">
        <span class="pp__pages-label">Pages ({activePalette()?.pages?.length ?? 0})</span>
        <button
          type="button"
          class="pp__icon-btn"
          onClick={beginAdd}
          disabled={activePaletteId() === null || adding()}
          title="New page"
          aria-label="New page"
          data-testid="palette-page-add"
        >
          +
        </button>
      </div>

      <Show when={adding()}>
        <div class="pp__pages-add-row">
          <input
            type="text"
            class="pp__pages-add-input"
            placeholder="Page name"
            value={newName()}
            onInput={(e) => setNewName(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void submitAdd();
              if (e.key === "Escape") cancelAdd();
            }}
            data-testid="palette-page-name-input"
            ref={(el) => {
              // Focus on mount so the user can type immediately after
              // clicking "+". `autofocus` triggers eslint's a11y rule;
              // a ref callback is the canonical Solid workaround.
              queueMicrotask(() => el?.focus());
            }}
          />
          <button
            type="button"
            class="pp__action-btn"
            onClick={() => void submitAdd()}
            data-testid="palette-page-create"
          >
            Create
          </button>
          <button
            type="button"
            class="pp__action-btn"
            onClick={cancelAdd}
            data-testid="palette-page-cancel"
          >
            Cancel
          </button>
        </div>
      </Show>

      <ul class="pp__pages-list" data-testid="palette-pages-list">
        <For each={activePalette()?.pages ?? []}>
          {(page) => (
            <li class="pp__pages-row" data-testid={`palette-page-${page.id}`}>
              <span class="pp__pages-row-name">{page.name}</span>
              <span class="pp__pages-row-count">{page.entry_indices.length}</span>
              <button
                type="button"
                class="pp__icon-btn"
                onClick={() => void handleRemove(page.id)}
                title={`Remove page "${page.name}"`}
                aria-label={`Remove page ${page.name}`}
                data-testid={`palette-page-remove-${page.id}`}
              >
                &#x2715;
              </button>
            </li>
          )}
        </For>
      </ul>
    </div>
  );
};

export default PalettePagesSection;
