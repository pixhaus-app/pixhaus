// Color and palette panel — S18.
//
// Main container that wires together the color picker, foreground/background
// swatches, palette grid, harmony picker, ramp generator, palette I/O menu,
// and Lospec browser.
//
// State lives in palette-panel-state.ts so it can be read by other panels
// (e.g. the brush engine in S15 reads foregroundIndex).

import { createEffect, createSignal, Show, For, type Component } from "solid-js";
import type { Rgba, SpriteId } from "../lib/types";
import {
  paletteAddColor,
  paletteSetColor,
  paletteRemoveColor,
  paletteAdd,
  type PaletteAddColorArgs,
  type PaletteSetColorArgs,
} from "../lib/commands/palette";
import {
  palettes,
  activePalette,
  activePaletteId,
  setActivePaletteId,
  foregroundIndex,
  editingIndex,
  setEditingIndex,
  editingColor,
  setEditingColor,
  cancelEditing,
  refreshPalettes,
  startEditing,
  isLocked,
} from "./palette-panel-state";
import { reportCommandFailure } from "../lib/utils/errors";
import ColorPicker from "./ColorPicker";
import FgBgSwatches from "./FgBgSwatches";
import PaletteGrid from "./PaletteGrid";
import HarmonyPicker from "./HarmonyPicker";
import RampGenerator from "./RampGenerator";
import PaletteIOMenu from "./PaletteIOMenu";
import LospecBrowser from "./LospecBrowser";
import PalettePagesSection from "./PalettePagesSection";
import PaletteAnimationSection from "./PaletteAnimationSection";
import ModalInput from "../components/ModalInput";

type SubPanel = "harmony" | "ramp" | "lospec" | null;

// "edit" maps the picker to an existing palette entry: slider releases
// round-trip to palette_set_color. "append" stages a draft color that is
// only sent (as palette_add_color) when the user clicks "Add to palette" —
// this is the only path that creates the first color in a fresh palette.
type PickerMode = "edit" | "append";

type Props = {
  /** The sprite whose palettes are displayed. Null when no project is open. */
  spriteId: SpriteId | null;
  /** When true, render inside the right rail (no outer aside chrome — the
   *  rail provides it). */
  inRail?: boolean;
};

const PalettePanel: Component<Props> = (props) => {
  const [subPanel, setSubPanel] = createSignal<SubPanel>(null);
  const [pickerOpen, setPickerOpen] = createSignal(false);
  const [pickerMode, setPickerMode] = createSignal<PickerMode>("edit");
  const [newPaletteOpen, setNewPaletteOpen] = createSignal(false);

  // Reload palettes whenever the active sprite changes.
  createEffect(() => {
    const id = props.spriteId;
    if (id !== null) {
      void refreshPalettes(id);
    }
  });

  // ── Color picker callbacks ─────────────────────────────────────────────────

  const handlePickerChange = (color: Rgba) => {
    // Live update the draft color for instant visual feedback in the grid.
    setEditingColor(color);
  };

  const handlePickerCommit = async (color: Rgba) => {
    // In append mode the picker stages a draft; slider releases must not
    // round-trip to the backend (each one would create a new swatch).
    // The explicit "Add to palette" button is the only commit path.
    if (pickerMode() === "append") return;

    const idx = editingIndex();
    const pid = activePaletteId();
    const sid = props.spriteId;
    if (idx === null || pid === null || sid === null) return;
    if (isLocked(idx)) return;

    const args: PaletteSetColorArgs = {
      sprite_id: sid,
      palette_id: pid,
      index: idx,
      color,
      name: activePalette()?.colors[idx]?.name ?? null,
    };
    try {
      await paletteSetColor(args);
      await refreshPalettes(sid);
    } catch (err: unknown) {
      reportCommandFailure("palette_set_color", err);
    }
  };

  const handlePickerOpen = (index: number) => {
    setPickerMode("edit");
    setPickerOpen(true);
    const entry = activePalette()?.colors[index];
    if (entry) startEditing(index, entry.color);
  };

  const handlePickerClose = () => {
    setPickerOpen(false);
    setPickerMode("edit");
    cancelEditing();
  };

  // Open the picker with no associated swatch index. The chosen color is
  // appended via paletteAddColor only when the user clicks "Add to palette".
  const handleAddColorClick = () => {
    if (props.spriteId === null || activePaletteId() === null) return;
    const seed = activePalette()?.colors[foregroundIndex()]?.color ?? {
      r: 0,
      g: 0,
      b: 0,
      a: 255,
    };
    setEditingIndex(null);
    setEditingColor(seed);
    setPickerMode("append");
    setPickerOpen(true);
  };

  const handlePickerAdd = async () => {
    const color = editingColor();
    if (!color) return;
    // Only close and clear the staged draft if the backend accepted the
    // append; on failure (already toasted by appendColor) keep the picker
    // open so the user can retry without losing the color they tuned.
    if (!(await appendColor(color))) return;
    setPickerOpen(false);
    setPickerMode("edit");
    cancelEditing();
  };

  // ── Grid callbacks ─────────────────────────────────────────────────────────

  const handleRemoveSwatch = async (index: number) => {
    const pid = activePaletteId();
    const sid = props.spriteId;
    if (pid === null || sid === null) return;
    try {
      await paletteRemoveColor(sid, pid, index);
      await refreshPalettes(sid);
    } catch (err: unknown) {
      reportCommandFailure("palette_remove_color", err);
    }
  };

  const handleRenameSwatch = async (index: number, name: string) => {
    const pid = activePaletteId();
    const sid = props.spriteId;
    const entry = activePalette()?.colors[index];
    if (!entry || pid === null || sid === null) return;
    const args: PaletteSetColorArgs = {
      sprite_id: sid,
      palette_id: pid,
      index,
      color: entry.color,
      name: name.trim() || null,
    };
    try {
      await paletteSetColor(args);
      await refreshPalettes(sid);
    } catch (err: unknown) {
      reportCommandFailure("palette_set_color (rename)", err);
    }
  };

  // ── Add-color helper (used by harmony, ramp, picker append) ──────────────

  // Returns true if the color was appended and the palette refreshed; false
  // when there's no active palette/sprite or the IPC call failed (already
  // surfaced via toast). Callers that need to react to failure (e.g. keep
  // the picker open so the user can retry) check the boolean.
  const appendColor = async (color: Rgba): Promise<boolean> => {
    const pid = activePaletteId();
    const sid = props.spriteId;
    if (pid === null || sid === null) return false;
    const args: PaletteAddColorArgs = { sprite_id: sid, palette_id: pid, color };
    try {
      await paletteAddColor(args);
      await refreshPalettes(sid);
      return true;
    } catch (err: unknown) {
      reportCommandFailure("palette_add_color", err);
      return false;
    }
  };

  // ── Lospec import ──────────────────────────────────────────────────────────

  const handleLospecImport = async (name: string, colors: Rgba[]) => {
    const sid = props.spriteId;
    if (sid === null) return;
    // Create a new named palette and populate it.
    try {
      const newPalette = await paletteAdd(sid, name);
      setActivePaletteId(newPalette.id);
      for (const color of colors) {
        await paletteAddColor({ sprite_id: sid, palette_id: newPalette.id, color });
      }
      await refreshPalettes(sid);
      setSubPanel(null);
    } catch (err: unknown) {
      reportCommandFailure("Lospec import", err);
    }
  };

  // ── Palette I/O import ────────────────────────────────────────────────────

  const handleFileImport = async (colors: Rgba[]) => {
    const pid = activePaletteId();
    const sid = props.spriteId;
    if (pid === null || sid === null) return;
    for (const color of colors) {
      await appendColor(color);
    }
  };

  // ── Palette switcher helpers ───────────────────────────────────────────────

  const handleNewPalette = () => {
    if (props.spriteId === null) return;
    setNewPaletteOpen(true);
  };

  const submitNewPalette = async (name: string) => {
    setNewPaletteOpen(false);
    const sid = props.spriteId;
    if (sid === null) return;
    const trimmed = name.trim();
    if (trimmed === "") return;
    try {
      const p = await paletteAdd(sid, trimmed);
      await refreshPalettes(sid);
      setActivePaletteId(p.id);
    } catch (err: unknown) {
      reportCommandFailure("palette_add", err);
    }
  };

  // ── Effective color shown by the picker ───────────────────────────────────

  const pickerColor = (): Rgba => {
    const draft = editingColor();
    if (draft) return draft;
    const entry = activePalette()?.colors[foregroundIndex()];
    return entry?.color ?? { r: 0, g: 0, b: 0, a: 255 };
  };

  // ── Sub-panel toggle helper ────────────────────────────────────────────────

  const toggleSub = (panel: SubPanel) => setSubPanel((prev) => (prev === panel ? null : panel));

  const noProject = () => props.spriteId === null;

  return (
    <aside class="pp" classList={{ "pp--in-rail": !!props.inRail }} aria-label="Palette panel">
      <Show when={noProject()}>
        <div class="pp__empty">Open a project to use the palette panel.</div>
      </Show>

      <Show when={!noProject()}>
        {/* Palette selector row */}
        <div class="pp__palette-row">
          <select
            class="pp__palette-select"
            value={activePaletteId() ?? ""}
            onChange={(e) => {
              const v = e.currentTarget.value;
              setActivePaletteId(v === "" ? null : parseInt(v, 10));
            }}
            aria-label="Active palette"
          >
            <For each={palettes()}>{(p) => <option value={String(p.id)}>{p.name}</option>}</For>
          </select>
          <button
            class="pp__icon-btn"
            onClick={handleNewPalette}
            title="New palette"
            aria-label="New palette"
          >
            +
          </button>
        </div>

        {/* Foreground / background swatches */}
        <FgBgSwatches onPickerOpen={handlePickerOpen} />

        {/* Inline color picker (shown when a swatch is being edited or a new color is being staged) */}
        <Show when={pickerOpen()}>
          <div class="pp__picker-header">
            <span class="pp__picker-label">
              {pickerMode() === "append"
                ? "New color"
                : (activePalette()?.colors[editingIndex() ?? 0]?.name ??
                  `Swatch ${editingIndex() ?? 0}`)}
            </span>
            <button
              class="pp__icon-btn"
              onClick={handlePickerClose}
              title="Close picker"
              aria-label="Close color picker"
            >
              &#x2715;
            </button>
          </div>
          <ColorPicker
            color={pickerColor()}
            onChange={handlePickerChange}
            onCommit={(color) => void handlePickerCommit(color)}
          />
          <Show when={pickerMode() === "append"}>
            <button
              class="pp__action-btn pp__action-btn--primary"
              onClick={() => void handlePickerAdd()}
            >
              Add to palette
            </button>
          </Show>
        </Show>

        {/* Palette swatch grid */}
        <div class="pp__grid-header">
          <span class="pp__grid-label">
            {activePalette()?.name ?? "—"} ({activePalette()?.colors.length ?? 0})
          </span>
          <button
            class="pp__icon-btn"
            onClick={handleAddColorClick}
            title="Add color"
            aria-label="Add color"
            disabled={activePaletteId() === null}
          >
            +
          </button>
        </div>
        <PaletteGrid
          onPickerOpen={handlePickerOpen}
          onRemove={(idx) => void handleRemoveSwatch(idx)}
          onRename={(idx, name) => void handleRenameSwatch(idx, name)}
        />

        {/* Palette pages — named subset views over the swatch grid */}
        <PalettePagesSection spriteId={props.spriteId} />

        {/* Palette animation — per-entry keyframed colors over the timeline */}
        <PaletteAnimationSection spriteId={props.spriteId} />

        {/* Sub-panel action bar */}
        <div class="pp__actions">
          <button
            class="pp__action-btn"
            data-testid="palette-harmony-toggle"
            classList={{ "pp__action-btn--active": subPanel() === "harmony" }}
            onClick={() => toggleSub("harmony")}
          >
            Harmony
          </button>
          <button
            class="pp__action-btn"
            data-testid="palette-ramp-toggle"
            classList={{ "pp__action-btn--active": subPanel() === "ramp" }}
            onClick={() => toggleSub("ramp")}
          >
            Ramp
          </button>
          <button
            class="pp__action-btn"
            classList={{ "pp__action-btn--active": subPanel() === "lospec" }}
            onClick={() => toggleSub("lospec")}
          >
            Lospec
          </button>
        </div>

        {/* Sub-panels */}
        <Show when={subPanel() === "harmony"}>
          <div class="pp__sub">
            <HarmonyPicker onAddColor={(c) => void appendColor(c)} />
          </div>
        </Show>

        <Show when={subPanel() === "ramp"}>
          <div class="pp__sub">
            <RampGenerator onAddColor={appendColor} />
          </div>
        </Show>

        <Show when={subPanel() === "lospec"}>
          <div class="pp__sub">
            <LospecBrowser onImport={(name, colors) => void handleLospecImport(name, colors)} />
          </div>
        </Show>

        {/* I/O menu (always at the bottom) */}
        <div class="pp__io">
          <PaletteIOMenu onImport={(colors) => void handleFileImport(colors)} />
        </div>
      </Show>

      <ModalInput
        open={newPaletteOpen()}
        title="New palette"
        label="Palette name"
        initialValue="Palette"
        submitLabel="Create"
        validate={(v) => (v.trim() === "" ? "Name cannot be empty" : null)}
        onSubmit={(name) => void submitNewPalette(name)}
        onClose={() => setNewPaletteOpen(false)}
      />
    </aside>
  );
};

export default PalettePanel;
