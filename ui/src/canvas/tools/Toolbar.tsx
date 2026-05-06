// Tool selector toolbar.
//
// Renders a vertical strip of tool buttons on the left edge of the canvas.
// Clicking a button sets `activeTool`; the active button gets an
// `aria-pressed` attribute so CSS can highlight it.

import { For, type Component } from "solid-js";
import { activeTool, setActiveTool, type ToolType } from "./tool-state";

const TOOLS: { id: ToolType; label: string; shortcut: string }[] = [
  { id: "pencil", label: "Pencil", shortcut: "P" },
  { id: "eraser", label: "Eraser", shortcut: "E" },
  { id: "fill", label: "Fill", shortcut: "F" },
  { id: "line", label: "Line", shortcut: "L" },
  { id: "rect", label: "Rect", shortcut: "R" },
  { id: "ellipse", label: "Ellipse", shortcut: "O" },
];

const Toolbar: Component = () => {
  return (
    <div class="toolbar" role="toolbar" aria-label="Drawing tools">
      <For each={TOOLS}>
        {(tool) => (
          <button
            class="toolbar-btn"
            aria-pressed={activeTool() === tool.id}
            title={`${tool.label} (${tool.shortcut})`}
            onClick={() => setActiveTool(tool.id)}
          >
            {tool.label[0]}
          </button>
        )}
      </For>
    </div>
  );
};

export default Toolbar;
