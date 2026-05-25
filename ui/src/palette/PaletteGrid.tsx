// Palette swatch grid.
//
// Features:
//   - Click to set as foreground index (shift-click for background)
//   - Drag-to-reorder (persisted via palette_reorder_colors)
//   - Right-click context menu: rename, delete, lock/unlock
//   - Per-swatch lock indicator (prevents accidental edits)
//   - Double-click to open color picker for that swatch
//
// The grid is a flat CSS grid; each swatch is a button for keyboard navigation.

import { createSignal, For, Show, type Component } from "solid-js";
import type { PaletteEntry, Rgba } from "../lib/types";
import { paletteReorderColors } from "../lib/commands/palette";
import { reportCommandFailure } from "../lib/utils/errors";
import { activeTarget } from "../canvas/canvas-state";
import {
  paletteUi,
  setForegroundIndex,
  setBackgroundIndex,
  toggleLock,
  activePalette,
  refreshPalettes,
  startEditing,
} from "./palette-panel-state";
import { rgbaToCss, contrastColor } from "./color-utils";
import ModalInput from "../components/ModalInput";

type Props = {
  /** Called when the user double-clicks a swatch to open the color picker. */
  onPickerOpen: (index: number) => void;
  /** Called when the user requests to remove a swatch. */
  onRemove: (index: number) => void;
  /** Called when the user requests to rename a swatch. */
  onRename: (index: number, name: string) => void;
};

type ContextMenu = {
  x: number;
  y: number;
  index: number;
};

const PaletteGrid: Component<Props> = (props) => {
  const [contextMenu, setContextMenu] = createSignal<ContextMenu | null>(null);
  const [dragFrom, setDragFrom] = createSignal<number | null>(null);
  const [dragOver, setDragOver] = createSignal<number | null>(null);
  // Rename dialog: { index, current } when open, null when closed.
  const [renameTarget, setRenameTarget] = createSignal<{
    index: number;
    current: string;
  } | null>(null);

  const closeMenu = () => setContextMenu(null);

  const handleContextMenu = (e: MouseEvent, index: number) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, index });
  };

  const handleClick = (e: MouseEvent, index: number) => {
    closeMenu();
    if (e.shiftKey) {
      setBackgroundIndex(index);
    } else {
      setForegroundIndex(index);
    }
  };

  const handleDblClick = (index: number) => {
    const p = activePalette();
    const entry = p?.colors[index];
    if (entry) {
      startEditing(index, entry.color);
      props.onPickerOpen(index);
    }
  };

  // ── Drag-to-reorder ──────────────────────────────────────────────────────
  const handleDragStart = (e: DragEvent, index: number) => {
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/plain", String(index));
    }
    setDragFrom(index);
  };

  const handleDragOver = (e: DragEvent, index: number) => {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    setDragOver(index);
  };

  const handleDrop = (e: DragEvent, toIndex: number) => {
    e.preventDefault();
    const fromIndex = dragFrom();
    setDragFrom(null);
    setDragOver(null);
    if (fromIndex === null || fromIndex === toIndex) return;
    const spriteId = activeTarget.spriteId;
    const paletteId = paletteUi.activePaletteId;
    if (spriteId === null || paletteId === null) return;
    paletteReorderColors(spriteId, paletteId, fromIndex, toIndex)
      .then(() => refreshPalettes(spriteId))
      .catch((err: unknown) => reportCommandFailure("palette_reorder_colors", err));
  };

  const handleDragEnd = () => {
    setDragFrom(null);
    setDragOver(null);
  };

  const handleRenamePrompt = (index: number) => {
    closeMenu();
    const p = activePalette();
    const entry = p?.colors[index];
    setRenameTarget({ index, current: entry?.name ?? "" });
  };

  const handleDelete = (index: number) => {
    closeMenu();
    if (paletteUi.lockedIndices.has(index)) return;
    props.onRemove(index);
  };

  const handleToggleLock = (index: number) => {
    closeMenu();
    toggleLock(index);
  };

  const palette = () => activePalette();

  return (
    <>
      <div class="pgrid" role="listbox" aria-label="Palette swatches" aria-multiselectable="false">
        <For each={palette()?.colors ?? []}>
          {(entry: PaletteEntry, i) => {
            const isFg = () => paletteUi.foregroundIndex === i();
            const isBg = () => paletteUi.backgroundIndex === i();
            const locked = () => paletteUi.lockedIndices.has(i());
            const isDragSrc = () => dragFrom() === i();
            const isDragTarget = () => dragOver() === i();

            return (
              <button
                class="pgrid__swatch"
                data-testid={`palette-swatch-${i()}`}
                classList={{
                  "pgrid__swatch--fg": isFg(),
                  "pgrid__swatch--bg": isBg(),
                  "pgrid__swatch--locked": locked(),
                  "pgrid__swatch--drag-src": isDragSrc(),
                  "pgrid__swatch--drag-target": isDragTarget(),
                }}
                style={{ background: rgbaToCss(entry.color) }}
                role="option"
                aria-selected={isFg()}
                aria-label={entry.name ?? `Swatch ${i()}`}
                title={buildTitle(i(), entry.color, entry.name)}
                draggable={!locked()}
                onClick={(e) => handleClick(e, i())}
                onDblClick={() => handleDblClick(i())}
                onContextMenu={(e) => handleContextMenu(e, i())}
                onDragStart={(e) => handleDragStart(e, i())}
                onDragOver={(e) => handleDragOver(e, i())}
                onDrop={(e) => handleDrop(e, i())}
                onDragEnd={handleDragEnd}
              >
                <Show when={isFg()}>
                  <span
                    class="pgrid__fg-marker"
                    style={{ color: contrastColor(entry.color) }}
                    aria-hidden="true"
                  >
                    F
                  </span>
                </Show>
                <Show when={isBg() && !isFg()}>
                  <span
                    class="pgrid__bg-marker"
                    style={{ color: contrastColor(entry.color) }}
                    aria-hidden="true"
                  >
                    B
                  </span>
                </Show>
                <Show when={locked()}>
                  <span class="pgrid__lock-icon" aria-hidden="true">
                    &#x1F512;
                  </span>
                </Show>
              </button>
            );
          }}
        </For>
      </div>

      {/* Context menu */}
      <Show when={contextMenu() !== null}>
        <div
          class="pgrid__menu-backdrop"
          onClick={closeMenu}
          onContextMenu={(e) => {
            e.preventDefault();
            closeMenu();
          }}
        />
        <div
          class="pgrid__menu"
          style={{ left: `${contextMenu()!.x}px`, top: `${contextMenu()!.y}px` }}
          role="menu"
        >
          <button
            class="pgrid__menu-item"
            role="menuitem"
            onClick={() => {
              const idx = contextMenu()!.index;
              closeMenu();
              handleDblClick(idx);
            }}
          >
            Edit color
          </button>
          <button
            class="pgrid__menu-item"
            role="menuitem"
            onClick={() => handleRenamePrompt(contextMenu()!.index)}
          >
            Rename
          </button>
          <button
            class="pgrid__menu-item"
            role="menuitem"
            onClick={() => handleToggleLock(contextMenu()!.index)}
          >
            {paletteUi.lockedIndices.has(contextMenu()!.index) ? "Unlock" : "Lock"}
          </button>
          <div class="pgrid__menu-sep" role="separator" />
          <button
            class="pgrid__menu-item pgrid__menu-item--danger"
            data-testid="palette-swatch-delete"
            role="menuitem"
            disabled={paletteUi.lockedIndices.has(contextMenu()!.index)}
            onClick={() => handleDelete(contextMenu()!.index)}
          >
            Remove swatch
          </button>
        </div>
      </Show>

      <ModalInput
        open={renameTarget() !== null}
        title="Rename swatch"
        label="Swatch name"
        initialValue={renameTarget()?.current ?? ""}
        onSubmit={(value) => {
          const target = renameTarget();
          if (target !== null) props.onRename(target.index, value);
          setRenameTarget(null);
        }}
        onClose={() => setRenameTarget(null)}
      />
    </>
  );
};

function buildTitle(index: number, color: Rgba, name?: string | null): string {
  const hex =
    "#" + [color.r, color.g, color.b].map((x) => x.toString(16).padStart(2, "0")).join("");
  const label = name ? `${name} (${hex})` : hex;
  return `[${index}] ${label}\nClick: foreground  Shift+click: background  Dbl-click: edit`;
}

export default PaletteGrid;
