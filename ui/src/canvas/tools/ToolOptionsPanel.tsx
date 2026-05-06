// Per-tool options panel rendered below the toolbar.
//
// Shows size, shape, and toggle controls that apply to the active tool.
// Only brush tools (pencil, eraser, line) show the shape and pixel-perfect
// controls; the fill tool shows a tolerance slider instead.

import { For, type Component, Match, Switch } from "solid-js";
import {
  activeTool,
  fillTolerance,
  pixelPerfect,
  setFillTolerance,
  setPixelPerfect,
  setToolShape,
  setToolSize,
  toolShape,
  toolSize,
  type BrushShape,
} from "./tool-state";

const SHAPES: BrushShape[] = ["pixel", "circle", "square"];

const BrushOptions: Component = () => (
  <div class="tool-options-group">
    <label class="tool-option-label">
      Size
      <input
        type="range"
        min="1"
        max="64"
        value={toolSize()}
        onInput={(e) => setToolSize(Number(e.currentTarget.value))}
        class="tool-option-range"
      />
      <span class="tool-option-value">{toolSize()}</span>
    </label>
    <fieldset class="tool-option-fieldset">
      <legend>Shape</legend>
      <For each={SHAPES}>
        {(s) => (
          <label class="tool-option-radio">
            <input
              type="radio"
              name="brush-shape"
              value={s}
              checked={toolShape() === s}
              onChange={() => setToolShape(s)}
            />
            {s}
          </label>
        )}
      </For>
    </fieldset>
    <label class="tool-option-checkbox">
      <input
        type="checkbox"
        checked={pixelPerfect()}
        onChange={(e) => setPixelPerfect(e.currentTarget.checked)}
      />
      Pixel-perfect
    </label>
  </div>
);

const FillOptions: Component = () => (
  <div class="tool-options-group">
    <label class="tool-option-label">
      Tolerance
      <input
        type="range"
        min="0"
        max="255"
        value={fillTolerance()}
        onInput={(e) => setFillTolerance(Number(e.currentTarget.value))}
        class="tool-option-range"
      />
      <span class="tool-option-value">{fillTolerance()}</span>
    </label>
  </div>
);

const ToolOptionsPanel: Component = () => {
  return (
    <div class="tool-options-panel">
      <Switch>
        <Match
          when={activeTool() === "pencil" || activeTool() === "eraser" || activeTool() === "line"}
        >
          <BrushOptions />
        </Match>
        <Match when={activeTool() === "fill"}>
          <FillOptions />
        </Match>
        <Match when={activeTool() === "rect" || activeTool() === "ellipse"}>
          <BrushOptions />
        </Match>
      </Switch>
    </div>
  );
};

export default ToolOptionsPanel;
