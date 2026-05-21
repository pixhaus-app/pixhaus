// Controlled form editor for a Structure record.
//
// Handles the StructureOutput tagged-enum shape:
//   "single" | { paneled: { canvas: Dimensions, panels: StructurePanel[] } }
//
// Defaults new structures to Paneled. The Single/Paneled toggle switches the
// output variant. The paneled branch shows canvas dimensions, a panel list
// editor, layout_negatives, and a live SVG preview.

import { type Component, For, Show, createMemo } from "solid-js";
import type {
  Dimensions,
  PanelSlot,
  Structure,
  StructureOutput,
  StructurePanel,
} from "../../lib/types";
import { previewBoxes } from "./preview";

// Preview area dimensions (px).
const PREVIEW_W = 240;
const PREVIEW_H = 180;

// All PanelSlot values in display order.
const PANEL_SLOTS: PanelSlot[] = [
  "view",
  "expression",
  "callout",
  "outfit",
  "palette_swatch",
  "generic",
];

type Props = {
  value: Structure;
  onChange: (s: Structure) => void;
  readOnly?: boolean;
};

// ── helpers ───────────────────────────────────────────────────────────────────

function isPaneled(
  output: StructureOutput,
): output is { paneled: { canvas: Dimensions; panels: StructurePanel[] } } {
  return typeof output === "object" && "paneled" in output;
}

function defaultPaneledOutput(): { paneled: { canvas: Dimensions; panels: StructurePanel[] } } {
  return {
    paneled: {
      canvas: { width: 512, height: 512 },
      panels: [],
    },
  };
}

function newPanel(): StructurePanel {
  return {
    label: "panel",
    rect: { x: 0, y: 0, w: 64, h: 64 },
    prose_fragment: "",
    slot: "generic",
  };
}

// ── component ─────────────────────────────────────────────────────────────────

const StructureEditor: Component<Props> = (props) => {
  function update(patch: Partial<Structure>): void {
    props.onChange({ ...props.value, ...patch });
  }

  // Updates layout_negatives with proper exactOptionalPropertyTypes handling.
  function updateLayoutNegatives(value: string): void {
    const next: Structure = { ...props.value };
    if (value === "") {
      delete next.layout_negatives;
    } else {
      next.layout_negatives = value;
    }
    props.onChange(next);
  }

  const paneled = createMemo(() => isPaneled(props.value.output));

  // Switches the output variant; preserves data when possible.
  function toggleOutputKind(toKind: "single" | "paneled"): void {
    if (toKind === "single") {
      update({ output: "single" });
    } else {
      // Preserve existing paneled data if we already have it; otherwise create default.
      const current = props.value.output;
      update({ output: isPaneled(current) ? current : defaultPaneledOutput() });
    }
  }

  function getPaneled(): { canvas: Dimensions; panels: StructurePanel[] } {
    const out = props.value.output;
    if (isPaneled(out)) return out.paneled;
    return defaultPaneledOutput().paneled;
  }

  function updateCanvas(patch: Partial<Dimensions>): void {
    const p = getPaneled();
    update({ output: { paneled: { ...p, canvas: { ...p.canvas, ...patch } } } });
  }

  function updatePanel(index: number, patch: Partial<StructurePanel>): void {
    const p = getPaneled();
    const panels = p.panels.map((panel, i) => (i === index ? { ...panel, ...patch } : panel));
    update({ output: { paneled: { ...p, panels } } });
  }

  function addPanel(): void {
    const p = getPaneled();
    update({ output: { paneled: { ...p, panels: [...p.panels, newPanel()] } } });
  }

  function removePanel(index: number): void {
    const p = getPaneled();
    update({
      output: { paneled: { ...p, panels: p.panels.filter((_, i) => i !== index) } },
    });
  }

  const boxes = createMemo(() => {
    if (!paneled()) return [];
    const p = getPaneled();
    return previewBoxes(
      p.canvas.width,
      p.canvas.height,
      p.panels.map((panel) => ({ label: panel.label, rect: panel.rect })),
      PREVIEW_W,
      PREVIEW_H,
    );
  });

  return (
    <div class="lib-structure-editor">
      <div class="lib-field">
        <label class="lib-field__label" for="structure-name">
          Name
        </label>
        <input
          id="structure-name"
          class="lib-field__input"
          type="text"
          disabled={props.readOnly}
          value={props.value.name}
          onInput={(e) => update({ name: e.currentTarget.value })}
        />
      </div>

      <div class="lib-field lib-field--row">
        <span class="lib-field__label">Output kind</span>
        <label class="lib-radio">
          <input
            type="radio"
            name="output-kind"
            value="single"
            disabled={props.readOnly}
            checked={!paneled()}
            onChange={() => toggleOutputKind("single")}
          />{" "}
          Single
        </label>
        <label class="lib-radio">
          <input
            type="radio"
            name="output-kind"
            value="paneled"
            disabled={props.readOnly}
            checked={paneled()}
            onChange={() => toggleOutputKind("paneled")}
          />{" "}
          Paneled
        </label>
      </div>

      <Show when={paneled()}>
        {/* Canvas dimensions */}
        <div class="lib-field lib-field--row">
          <label class="lib-field__label">Canvas</label>
          <input
            class="lib-field__input lib-field__input--num"
            type="number"
            min={1}
            disabled={props.readOnly}
            value={getPaneled().canvas.width}
            onInput={(e) => updateCanvas({ width: Number(e.currentTarget.value) || 1 })}
          />
          <span class="lib-field__sep">×</span>
          <input
            class="lib-field__input lib-field__input--num"
            type="number"
            min={1}
            disabled={props.readOnly}
            value={getPaneled().canvas.height}
            onInput={(e) => updateCanvas({ height: Number(e.currentTarget.value) || 1 })}
          />
        </div>

        {/* Panel list */}
        <div class="lib-panels">
          <div class="lib-panels__heading">Panels</div>
          <For each={getPaneled().panels}>
            {(panel, i) => (
              <div class="lib-panel-row">
                <input
                  class="lib-field__input lib-field__input--label"
                  type="text"
                  placeholder="label"
                  disabled={props.readOnly}
                  value={panel.label}
                  onInput={(e) => updatePanel(i(), { label: e.currentTarget.value })}
                />
                <select
                  class="lib-field__select lib-field__select--slot"
                  disabled={props.readOnly}
                  value={panel.slot}
                  onChange={(e) => updatePanel(i(), { slot: e.currentTarget.value as PanelSlot })}
                >
                  <For each={PANEL_SLOTS}>{(s) => <option value={s}>{s}</option>}</For>
                </select>
                <span class="lib-panel-row__coord-label">x</span>
                <input
                  class="lib-field__input lib-field__input--num"
                  type="number"
                  disabled={props.readOnly}
                  value={panel.rect.x}
                  onInput={(e) =>
                    updatePanel(i(), { rect: { ...panel.rect, x: Number(e.currentTarget.value) } })
                  }
                />
                <span class="lib-panel-row__coord-label">y</span>
                <input
                  class="lib-field__input lib-field__input--num"
                  type="number"
                  disabled={props.readOnly}
                  value={panel.rect.y}
                  onInput={(e) =>
                    updatePanel(i(), { rect: { ...panel.rect, y: Number(e.currentTarget.value) } })
                  }
                />
                <span class="lib-panel-row__coord-label">w</span>
                <input
                  class="lib-field__input lib-field__input--num"
                  type="number"
                  min={1}
                  disabled={props.readOnly}
                  value={panel.rect.w}
                  onInput={(e) =>
                    updatePanel(i(), { rect: { ...panel.rect, w: Number(e.currentTarget.value) } })
                  }
                />
                <span class="lib-panel-row__coord-label">h</span>
                <input
                  class="lib-field__input lib-field__input--num"
                  type="number"
                  min={1}
                  disabled={props.readOnly}
                  value={panel.rect.h}
                  onInput={(e) =>
                    updatePanel(i(), { rect: { ...panel.rect, h: Number(e.currentTarget.value) } })
                  }
                />
                <Show when={!props.readOnly}>
                  <button
                    class="lib-btn lib-btn--danger lib-btn--sm"
                    type="button"
                    onClick={() => removePanel(i())}
                  >
                    Remove
                  </button>
                </Show>
                <textarea
                  class="lib-field__textarea lib-panel-row__prose"
                  placeholder="Prose fragment"
                  disabled={props.readOnly}
                  rows={2}
                  value={panel.prose_fragment}
                  onInput={(e) => updatePanel(i(), { prose_fragment: e.currentTarget.value })}
                />
              </div>
            )}
          </For>
          <Show when={!props.readOnly}>
            <button class="lib-btn lib-btn--secondary" type="button" onClick={addPanel}>
              Add panel
            </button>
          </Show>
        </div>

        {/* Live SVG preview */}
        <div class="lib-structure-preview">
          <div class="lib-structure-preview__heading">Preview</div>
          <svg
            class="lib-structure-preview__svg"
            width={PREVIEW_W}
            height={PREVIEW_H}
            viewBox={`0 0 ${PREVIEW_W} ${PREVIEW_H}`}
          >
            {/* Canvas border */}
            <rect
              x={0}
              y={0}
              width={(() => {
                const p = getPaneled();
                const scale = Math.min(PREVIEW_W / p.canvas.width, PREVIEW_H / p.canvas.height);
                return p.canvas.width * scale;
              })()}
              height={(() => {
                const p = getPaneled();
                const scale = Math.min(PREVIEW_W / p.canvas.width, PREVIEW_H / p.canvas.height);
                return p.canvas.height * scale;
              })()}
              fill="none"
              stroke="#444"
              stroke-width={1}
            />
            <For each={boxes()}>
              {(box) => (
                <>
                  <rect
                    x={box.x}
                    y={box.y}
                    width={box.w}
                    height={box.h}
                    fill="rgba(100,150,255,0.15)"
                    stroke="#6696ff"
                    stroke-width={1}
                  />
                  <text
                    x={box.x + box.w / 2}
                    y={box.y + box.h / 2}
                    text-anchor="middle"
                    dominant-baseline="middle"
                    font-size="10"
                    fill="#ccc"
                  >
                    {box.label}
                  </text>
                </>
              )}
            </For>
          </svg>
        </div>
      </Show>

      <div class="lib-field">
        <label class="lib-field__label" for="structure-layout-negatives">
          Layout negatives
        </label>
        <textarea
          id="structure-layout-negatives"
          class="lib-field__textarea"
          disabled={props.readOnly}
          rows={3}
          value={props.value.layout_negatives ?? ""}
          onInput={(e) => updateLayoutNegatives(e.currentTarget.value)}
        />
      </div>
    </div>
  );
};

export default StructureEditor;
