// Fill options as a right-rail section body.
//
// Replaces the fill half of the legacy ToolOptionsPanel. Auto-expanded
// by the rail when activeTool() === "fill" (see rail-state).

import { type Component } from "solid-js";
import { tool, setFillTolerance } from "./tool-state";

const FillSection: Component = () => (
  <div class="tool-options-group" data-testid="tool-options-fill">
    <label class="tool-option-label">
      Tolerance
      <input
        type="range"
        min="0"
        max="255"
        value={tool.fillTolerance}
        onInput={(e) => setFillTolerance(Number(e.currentTarget.value))}
        class="tool-option-range"
        data-testid="tool-option-tolerance"
      />
      <span class="tool-option-value">{tool.fillTolerance}</span>
    </label>
  </div>
);

export default FillSection;
