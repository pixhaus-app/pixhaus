// Brush options as a right-rail section body.
//
// Replaces the brush half of the legacy ToolOptionsPanel. Auto-expanded
// by the rail when activeTool() is a brush tool (see rail-state).

import { For, type Component } from "solid-js";
import { tool, setPixelPerfect, setToolShape, setToolSize, type BrushShape } from "./tool-state";

const SHAPES: BrushShape[] = ["pixel", "circle", "square"];

const BrushSection: Component = () => (
  <div class="tool-options-group" data-testid="tool-options-brush">
    <label class="tool-option-label">
      Size
      <input
        type="range"
        min="1"
        max="64"
        value={tool.size}
        onInput={(e) => setToolSize(Number(e.currentTarget.value))}
        class="tool-option-range"
        data-testid="tool-option-size"
      />
      <span class="tool-option-value">{tool.size}</span>
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
              checked={tool.shape === s}
              onChange={() => setToolShape(s)}
              data-testid={`tool-option-shape-${s}`}
            />
            {s}
          </label>
        )}
      </For>
    </fieldset>
    <label class="tool-option-checkbox">
      <input
        type="checkbox"
        checked={tool.pixelPerfect}
        onChange={(e) => setPixelPerfect(e.currentTarget.checked)}
        data-testid="tool-option-pixel-perfect"
      />
      Pixel-perfect
    </label>
  </div>
);

export default BrushSection;
