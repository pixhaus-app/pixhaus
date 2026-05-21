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
} from "../lib/types";
import { Button } from "../lib/ui/Button";
import { previewBoxes } from "./preview";

// Preview fits the canvas (at its real aspect ratio) within this box.
const PREVIEW_MAX_W = 300;
const PREVIEW_MAX_H = 460;

// A panel label is only drawn inside its preview box when the box is big
// enough to hold a few legible characters; otherwise the box still carries a
// hover <title>. Avoids the overlapping-label pile-up on narrow panels.
const MIN_LABEL_W = 30;
const MIN_LABEL_H = 14;

// Truncates a label (with an ellipsis) to roughly fit `boxW` at `fontSize`.
function fitLabel(label: string, boxW: number, fontSize: number): string {
  const maxChars = Math.floor(boxW / (fontSize * 0.6));
  if (label.length <= maxChars) return label;
  if (maxChars <= 1) return "";
  return `${label.slice(0, maxChars - 1)}…`;
}

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

  // SVG dimensions: the canvas scaled to fit the max box at its real aspect
  // ratio, so the preview rect fills the SVG with no empty margin.
  const previewDims = createMemo(() => {
    const c = getPaneled().canvas;
    const scale = Math.min(PREVIEW_MAX_W / c.width, PREVIEW_MAX_H / c.height);
    return { width: c.width * scale, height: c.height * scale };
  });

  const boxes = createMemo(() => {
    if (!paneled()) return [];
    const p = getPaneled();
    return previewBoxes(
      p.canvas.width,
      p.canvas.height,
      p.panels.map((panel) => ({ label: panel.label, rect: panel.rect })),
      PREVIEW_MAX_W,
      PREVIEW_MAX_H,
    );
  });

  return (
    <div class={`lib-structure-editor${paneled() ? " lib-structure-editor--paneled" : ""}`}>
      <div class="lib-structure-editor__fields">
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
            <For
              each={getPaneled().panels}
              fallback={<div class="lib-panels__empty">No panels yet.</div>}
            >
              {(panel, i) => (
                <div class="lib-panel-row">
                  <div class="lib-panel-row__head">
                    <input
                      class="lib-field__input lib-panel-row__label"
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
                      onChange={(e) =>
                        updatePanel(i(), { slot: e.currentTarget.value as PanelSlot })
                      }
                    >
                      <For each={PANEL_SLOTS}>{(s) => <option value={s}>{s}</option>}</For>
                    </select>
                    <Show when={!props.readOnly}>
                      <Button variant="danger" size="sm" onClick={() => removePanel(i())}>
                        Remove
                      </Button>
                    </Show>
                  </div>
                  <div class="lib-coords">
                    <label class="lib-coord">
                      <span class="lib-coord__label">X</span>
                      <input
                        class="lib-field__input lib-coord__input"
                        type="number"
                        disabled={props.readOnly}
                        value={panel.rect.x}
                        onInput={(e) =>
                          updatePanel(i(), {
                            rect: { ...panel.rect, x: Number(e.currentTarget.value) },
                          })
                        }
                      />
                    </label>
                    <label class="lib-coord">
                      <span class="lib-coord__label">Y</span>
                      <input
                        class="lib-field__input lib-coord__input"
                        type="number"
                        disabled={props.readOnly}
                        value={panel.rect.y}
                        onInput={(e) =>
                          updatePanel(i(), {
                            rect: { ...panel.rect, y: Number(e.currentTarget.value) },
                          })
                        }
                      />
                    </label>
                    <label class="lib-coord">
                      <span class="lib-coord__label">W</span>
                      <input
                        class="lib-field__input lib-coord__input"
                        type="number"
                        min={1}
                        disabled={props.readOnly}
                        value={panel.rect.w}
                        onInput={(e) =>
                          updatePanel(i(), {
                            rect: { ...panel.rect, w: Number(e.currentTarget.value) },
                          })
                        }
                      />
                    </label>
                    <label class="lib-coord">
                      <span class="lib-coord__label">H</span>
                      <input
                        class="lib-field__input lib-coord__input"
                        type="number"
                        min={1}
                        disabled={props.readOnly}
                        value={panel.rect.h}
                        onInput={(e) =>
                          updatePanel(i(), {
                            rect: { ...panel.rect, h: Number(e.currentTarget.value) },
                          })
                        }
                      />
                    </label>
                  </div>
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
              <Button variant="ghost" size="sm" onClick={addPanel}>
                Add panel
              </Button>
            </Show>
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

      {/* Live SVG preview — pinned beside the fields on wide screens. */}
      <Show when={paneled()}>
        <div class="lib-structure-preview">
          <div class="lib-structure-preview__heading">Preview</div>
          <svg
            class="lib-structure-preview__svg"
            width={previewDims().width}
            height={previewDims().height}
            viewBox={`0 0 ${previewDims().width} ${previewDims().height}`}
          >
            <rect
              x={0}
              y={0}
              width={previewDims().width}
              height={previewDims().height}
              fill="var(--bg-app)"
              stroke="var(--border-strong)"
              stroke-width={1}
            />
            <For each={boxes()}>
              {(box) => {
                const fontSize = Math.max(6, Math.min(10, box.h * 0.4));
                const showLabel = box.w >= MIN_LABEL_W && box.h >= MIN_LABEL_H;
                return (
                  <g>
                    <title>{box.label}</title>
                    <rect
                      x={box.x}
                      y={box.y}
                      width={box.w}
                      height={box.h}
                      fill="var(--accent-subtle)"
                      stroke="var(--accent-border)"
                      stroke-width={1}
                    />
                    <Show when={showLabel}>
                      <text
                        x={box.x + box.w / 2}
                        y={box.y + box.h / 2}
                        text-anchor="middle"
                        dominant-baseline="middle"
                        font-size={String(fontSize)}
                        fill="var(--text-primary)"
                      >
                        {fitLabel(box.label, box.w, fontSize)}
                      </text>
                    </Show>
                  </g>
                );
              }}
            </For>
          </svg>
        </div>
      </Show>
    </div>
  );
};

export default StructureEditor;
