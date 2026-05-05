// Single row in the layer panel.
//
// Renders: thumbnail placeholder, name (with inline rename), visibility toggle,
// lock toggle, blend mode dropdown, opacity slider. The row is draggable for
// reorder. Group rows have an expand/collapse chevron.

import { type Component, For, Show, createEffect, createSignal, onCleanup } from "solid-js";
import type { BlendMode, Layer, LayerId, SpriteId } from "../lib/types";
import { activeLayerId } from "../canvas/canvas-state";
import {
  beginRename,
  cancelRename,
  commitRename,
  dragOverIndex,
  isGroupExpanded,
  renamingLayerId,
  selectedLayerIds,
  selectLayer,
  setDragOverIndex,
  setLayerBlendMode,
  setLayerLocked,
  setLayerOpacity,
  setLayerVisibility,
  toggleGroupExpanded,
  toggleLayerSelection,
  reorderLayer,
} from "./layer-state";

// Human-readable labels for blend modes shown in the dropdown.
const BLEND_MODE_LABELS: Record<BlendMode, string> = {
  normal: "Normal",
  darken: "Darken",
  multiply: "Multiply",
  color_burn: "Color Burn",
  lighten: "Lighten",
  screen: "Screen",
  color_dodge: "Color Dodge",
  addition: "Addition",
  overlay: "Overlay",
  soft_light: "Soft Light",
  hard_light: "Hard Light",
  difference: "Difference",
  exclusion: "Exclusion",
  subtract: "Subtract",
  divide: "Divide",
  hue: "Hue",
  saturation: "Saturation",
  color: "Color",
  luminosity: "Luminosity",
};

const BLEND_MODES = Object.keys(BLEND_MODE_LABELS) as BlendMode[];

type Props = {
  layer: Layer;
  layerIndex: number;
  depth: number;
  spriteId: SpriteId;
  onContextMenu: (e: MouseEvent, layerId: LayerId) => void;
};

const LayerRow: Component<Props> = (props) => {
  const isActive = () => activeLayerId() === props.layer.id;
  const isSelected = () => selectedLayerIds().has(props.layer.id);
  const isRenaming = () => renamingLayerId() === props.layer.id;
  const isGroup = () => props.layer.kind.kind === "group";
  const expanded = () => isGroup() && isGroupExpanded(props.layer.id);
  const isDragTarget = () => dragOverIndex() === props.layerIndex;

  function handleClick(e: MouseEvent): void {
    if (e.ctrlKey || e.metaKey) {
      toggleLayerSelection(props.layer.id);
    } else {
      selectLayer(props.layer.id, e.shiftKey);
    }
  }

  function handleDoubleClick(): void {
    beginRename(props.layer.id);
  }

  function handleChevronClick(e: MouseEvent): void {
    e.stopPropagation();
    toggleGroupExpanded(props.layer.id);
  }

  function handleVisibilityClick(e: MouseEvent): void {
    e.stopPropagation();
    setLayerVisibility(props.spriteId, props.layer.id, !props.layer.visible);
  }

  function handleLockClick(e: MouseEvent): void {
    e.stopPropagation();
    setLayerLocked(props.spriteId, props.layer.id, !props.layer.locked);
  }

  function handleOpacityChange(e: Event): void {
    const value = parseInt((e.target as HTMLInputElement).value, 10);
    setLayerOpacity(props.spriteId, props.layer.id, value);
  }

  function handleBlendModeChange(e: Event): void {
    const value = (e.target as HTMLSelectElement).value as BlendMode;
    setLayerBlendMode(props.spriteId, props.layer.id, value);
  }

  // Drag-to-reorder handlers.
  //
  // The drag payload carries TWO bits of information:
  //   id    — the dragged layer's stable LayerId (so the drop target
  //           can pass it to reorderLayer; using only the source's
  //           index would mean the drop handler reorders the wrong
  //           layer if any panel state has changed since dragstart).
  //   index — the source's flat-list index, used to compute the
  //           target insert position (moving down vs. up needs the
  //           ±1 adjustment so the dragged row lands at the visual
  //           drop slot).
  function handleDragStart(e: DragEvent): void {
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData(
        "text/plain",
        JSON.stringify({ id: props.layer.id, index: props.layerIndex }),
      );
    }
  }

  function handleDragOver(e: DragEvent): void {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    setDragOverIndex(props.layerIndex);
  }

  function handleDragLeave(): void {
    setDragOverIndex(null);
  }

  function handleDrop(e: DragEvent): void {
    e.preventDefault();
    setDragOverIndex(null);
    const raw = e.dataTransfer?.getData("text/plain") ?? "";
    let parsed: { id: number; index: number } | null = null;
    try {
      const v = JSON.parse(raw) as unknown;
      if (
        v !== null &&
        typeof v === "object" &&
        typeof (v as { id: unknown }).id === "number" &&
        typeof (v as { index: unknown }).index === "number"
      ) {
        parsed = v as { id: number; index: number };
      }
    } catch {
      // Bad payload: ignore the drop rather than mutating state.
    }
    if (parsed === null || parsed.index === props.layerIndex) return;
    // When dropping onto row 0 from below, the `props.layerIndex - 1`
    // branch yields -1; the backend rejects negative indices so clamp
    // to 0. The same `Math.max(0, …)` covers any future drag math that
    // tries to slide a row off the top.
    const newIndex = Math.max(
      0,
      parsed.index > props.layerIndex ? props.layerIndex : props.layerIndex - 1,
    );
    reorderLayer(props.spriteId, parsed.id, newIndex);
  }

  return (
    <div
      class="layer-row"
      classList={{
        "layer-row--active": isActive(),
        "layer-row--selected": isSelected(),
        "layer-row--hidden": !props.layer.visible,
        "layer-row--drag-target": isDragTarget(),
      }}
      style={{ "padding-left": `${8 + props.depth * 16}px` }}
      onClick={handleClick}
      onDblClick={handleDoubleClick}
      onContextMenu={(e) => {
        e.preventDefault();
        props.onContextMenu(e, props.layer.id);
      }}
      draggable
      onDragStart={handleDragStart}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
    >
      {/* Chevron for groups */}
      <Show when={isGroup()}>
        <button
          class="layer-row__chevron"
          onClick={handleChevronClick}
          title={expanded() ? "Collapse group" : "Expand group"}
        >
          <svg width="8" height="8" viewBox="0 0 8 8" fill="currentColor">
            <Show
              when={expanded()}
              fallback={
                <path
                  d="M2 1 L6 4 L2 7"
                  stroke="currentColor"
                  stroke-width="1.5"
                  fill="none"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              }
            >
              <path
                d="M1 2 L4 6 L7 2"
                stroke="currentColor"
                stroke-width="1.5"
                fill="none"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </Show>
          </svg>
        </button>
      </Show>
      <Show when={!isGroup()}>
        {/* Spacer aligns non-group rows with group children */}
        <div class="layer-row__chevron-spacer" />
      </Show>

      {/* Thumbnail — placeholder until S01 pixel buffers land */}
      <div
        class="layer-row__thumb"
        title={`Layer kind: ${props.layer.kind.kind}`}
        aria-label={`${props.layer.name} thumbnail`}
      >
        <KindIcon kind={props.layer.kind.kind} />
      </div>

      {/* Name — or rename input */}
      <Show when={isRenaming()} fallback={<span class="layer-row__name">{props.layer.name}</span>}>
        <RenameInput layerId={props.layer.id} initialName={props.layer.name} />
      </Show>

      {/* Controls on the right */}
      <div class="layer-row__controls">
        <button
          class="layer-row__icon-btn"
          classList={{ "layer-row__icon-btn--inactive": !props.layer.visible }}
          onClick={handleVisibilityClick}
          title={props.layer.visible ? "Hide layer" : "Show layer"}
        >
          <EyeIcon visible={props.layer.visible} />
        </button>
        <button
          class="layer-row__icon-btn"
          classList={{ "layer-row__icon-btn--inactive": !props.layer.locked }}
          onClick={handleLockClick}
          title={props.layer.locked ? "Unlock layer" : "Lock layer"}
        >
          <LockIcon locked={props.layer.locked} />
        </button>
      </div>

      {/* Blend mode + opacity — only shown when active to save space */}
      <Show when={isActive()}>
        <div class="layer-row__props" onClick={(e) => e.stopPropagation()}>
          <select
            class="layer-row__blend-select"
            value={props.layer.blend_mode}
            onChange={handleBlendModeChange}
            title="Blend mode"
          >
            <For each={BLEND_MODES}>
              {(mode) => <option value={mode}>{BLEND_MODE_LABELS[mode]}</option>}
            </For>
          </select>
          <input
            class="layer-row__opacity"
            type="range"
            min="0"
            max="255"
            value={props.layer.opacity}
            onInput={handleOpacityChange}
            title={`Opacity: ${Math.round((props.layer.opacity / 255) * 100)}%`}
          />
        </div>
      </Show>
    </div>
  );
};

// ── Inline rename input ──────────────────────────────────────────────────────

type RenameInputProps = { layerId: LayerId; initialName: string };

const RenameInput: Component<RenameInputProps> = (props) => {
  let inputRef!: HTMLInputElement;
  const [value, setValue] = createSignal(props.initialName);

  createEffect(() => {
    // Auto-focus and select-all when the input mounts.
    inputRef?.focus();
    inputRef?.select();
  });

  function handleKeyDown(e: KeyboardEvent): void {
    if (e.key === "Enter") {
      e.preventDefault();
      commitRename(props.layerId, value());
    } else if (e.key === "Escape") {
      e.preventDefault();
      cancelRename();
    }
  }

  onCleanup(() => {
    // If the input unmounts mid-edit (panel collapsed, project closed,
    // row scrolled out of the virtual window) the onBlur handler won't
    // fire — DOM removal doesn't dispatch blur. Commit the typed value
    // here so users don't lose what they typed. Empty input falls back
    // to a cancel so we don't IPC an empty rename. The renamingLayerId
    // guard avoids re-committing if some other path (Enter, Escape,
    // blur) already finalized this rename.
    if (renamingLayerId() === props.layerId) {
      const v = value().trim();
      if (v.length > 0) {
        commitRename(props.layerId, v);
      } else {
        cancelRename();
      }
    }
  });

  return (
    <input
      ref={inputRef}
      class="layer-row__rename-input"
      value={value()}
      onInput={(e) => setValue(e.currentTarget.value)}
      onKeyDown={handleKeyDown}
      onBlur={() => commitRename(props.layerId, value())}
      onClick={(e) => e.stopPropagation()}
    />
  );
};

// ── Small SVG icons ──────────────────────────────────────────────────────────

type EyeIconProps = { visible: boolean };
const EyeIcon: Component<EyeIconProps> = (props) => (
  <svg
    width="12"
    height="12"
    viewBox="0 0 12 12"
    fill="none"
    stroke="currentColor"
    stroke-width="1.2"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <Show
      when={props.visible}
      fallback={
        <>
          <path d="M1 1 L11 11" />
          <path d="M5 2.2A5.8 5.8 0 0 1 11 6c-.4.8-1 1.5-1.7 2" />
          <path d="M2.5 3.6A5.8 5.8 0 0 0 1 6c1 2 3 3.5 5 3.5a5 5 0 0 0 1.8-.3" />
        </>
      }
    >
      <>
        <ellipse cx="6" cy="6" rx="5" ry="3.5" />
        <circle cx="6" cy="6" r="1.8" fill="currentColor" stroke="none" />
      </>
    </Show>
  </svg>
);

type LockIconProps = { locked: boolean };
const LockIcon: Component<LockIconProps> = (props) => (
  <svg
    width="10"
    height="12"
    viewBox="0 0 10 12"
    fill="none"
    stroke="currentColor"
    stroke-width="1.2"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <rect x="1" y="5.5" width="8" height="6" rx="1" />
    <Show when={props.locked} fallback={<path d="M3 5.5 V3.5 A2 2 0 0 1 9 3.5" />}>
      <path d="M3 5.5 V3.5 A2 2 0 0 1 7 3.5 V5.5" />
    </Show>
  </svg>
);

type KindIconProps = { kind: string };
const KindIcon: Component<KindIconProps> = (props) => {
  // Color-coded icons by layer kind — no pixel data until S01 lands.
  const colors: Record<string, string> = {
    raster: "var(--text-disabled)",
    group: "var(--accent)",
    tilemap: "var(--success)",
    reference: "var(--info)",
  };
  const color = () => colors[props.kind] ?? "var(--text-disabled)";

  return (
    <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
      <Show when={props.kind === "group"}>
        <rect
          x="2"
          y="6"
          width="16"
          height="10"
          rx="1"
          stroke={color()}
          stroke-width="1.2"
          fill="none"
        />
        <path d="M2 6V4h5l2 2H2z" stroke={color()} stroke-width="1.2" fill="none" />
      </Show>
      <Show when={props.kind === "tilemap"}>
        <rect x="2" y="2" width="7" height="7" stroke={color()} stroke-width="1.2" />
        <rect x="11" y="2" width="7" height="7" stroke={color()} stroke-width="1.2" />
        <rect x="2" y="11" width="7" height="7" stroke={color()} stroke-width="1.2" />
        <rect x="11" y="11" width="7" height="7" stroke={color()} stroke-width="1.2" />
      </Show>
      <Show when={props.kind === "reference"}>
        <rect x="3" y="5" width="14" height="11" rx="1" stroke={color()} stroke-width="1.2" />
        <path
          d="M3 12 L7 8 L11 12 L14 9 L17 12"
          stroke={color()}
          stroke-width="1.2"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
        <circle cx="8" cy="8" r="1.5" stroke={color()} stroke-width="1.2" />
      </Show>
      <Show when={props.kind === "raster"}>
        {/* Checkerboard pattern for raster layer */}
        <rect x="3" y="3" width="14" height="14" rx="1" fill="var(--bg-active)" />
        <rect x="3" y="3" width="7" height="7" fill="var(--text-disabled)" opacity="0.3" />
        <rect x="10" y="10" width="7" height="7" fill="var(--text-disabled)" opacity="0.3" />
      </Show>
    </svg>
  );
};

export default LayerRow;
