// Tool selector toolbar.
//
// A vertical strip on the left edge of the canvas, split into two groups:
// drawing tools (pencil, eraser, fill, line, rect, ellipse) that set
// `activeTool`, and selection tools (marquees, lasso, magic wand, color
// range) that set `selectTool` and flip the canvas into selection mode.
//
// Each button shows a lucide icon and a rich tooltip with its name,
// keyboard shortcut, and a one-line description. The shortcut text comes
// from the active keybind preset so it stays accurate when the user
// remaps keys.

import { For, type Component } from "solid-js";
import {
  Pencil,
  Eraser,
  PaintBucket,
  Slash,
  Square,
  Circle,
  SquareDashed,
  CircleDashed,
  Lasso,
  Wand2,
  Pipette,
  type LucideProps,
} from "lucide-solid";
import { tool as toolState, setActiveTool, type ToolType } from "./tool-state";
import { select, setSelectTool, type SelectTool } from "../select/select-state";
import { viewport, setIsSelectMode } from "../canvas-state";
import { comboForCommand } from "../../keybinds/keybind-manager";
import Tooltip from "../../lib/ui/Tooltip";

type LucideIcon = (props: LucideProps) => ReturnType<Component>;

type DrawTool = {
  id: ToolType;
  label: string;
  description: string;
  icon: LucideIcon;
  command: string;
};

type SelectToolMeta = {
  id: SelectTool;
  label: string;
  description: string;
  icon: LucideIcon;
  command: string;
};

const DRAW_TOOLS: DrawTool[] = [
  {
    id: "pencil",
    label: "Pencil",
    description: "Draw single pixels freehand.",
    icon: Pencil,
    command: "tool:pencil",
  },
  {
    id: "eraser",
    label: "Eraser",
    description: "Erase pixels back to transparency.",
    icon: Eraser,
    command: "tool:eraser",
  },
  {
    id: "fill",
    label: "Fill",
    description: "Flood-fill the connected region under the cursor.",
    icon: PaintBucket,
    command: "tool:fill",
  },
  {
    id: "line",
    label: "Line",
    description: "Drag to draw a straight line.",
    icon: Slash,
    command: "tool:line",
  },
  {
    id: "rect",
    label: "Rectangle",
    description: "Drag to draw a rectangle.",
    icon: Square,
    command: "tool:rect",
  },
  {
    id: "ellipse",
    label: "Ellipse",
    description: "Drag to draw an ellipse.",
    icon: Circle,
    command: "tool:ellipse",
  },
];

const SELECT_TOOLS: SelectToolMeta[] = [
  {
    id: "rect",
    label: "Rectangle Marquee",
    description: "Select a rectangular region.",
    icon: SquareDashed,
    command: "select:tool-rect",
  },
  {
    id: "ellipse",
    label: "Ellipse Marquee",
    description: "Select an elliptical region.",
    icon: CircleDashed,
    command: "select:tool-ellipse",
  },
  {
    id: "lasso",
    label: "Lasso",
    description: "Click points to draw a freehand selection.",
    icon: Lasso,
    command: "select:tool-lasso",
  },
  {
    id: "wand",
    label: "Magic Wand",
    description: "Select connected pixels by color similarity.",
    icon: Wand2,
    command: "select:tool-wand",
  },
  {
    id: "color-range",
    label: "Color Range",
    description: "Select every pixel matching a sampled color.",
    icon: Pipette,
    command: "select:tool-color-range",
  },
];

const Toolbar: Component = () => {
  const drawActive = (id: ToolType) => !viewport.isSelectMode && toolState.activeTool === id;
  const selectActive = (id: SelectTool) => viewport.isSelectMode && select.selectTool === id;

  return (
    <div class="toolbar" role="toolbar" aria-label="Tools">
      <For each={DRAW_TOOLS}>
        {(tool) => (
          <Tooltip
            label={tool.label}
            shortcut={comboForCommand(tool.command)}
            description={tool.description}
          >
            <button
              type="button"
              class="toolbar-btn"
              data-testid={`tool-${tool.id}`}
              aria-label={tool.label}
              aria-pressed={drawActive(tool.id)}
              onClick={() => {
                setActiveTool(tool.id);
                setIsSelectMode(false);
              }}
            >
              <tool.icon size={18} aria-hidden={true} />
            </button>
          </Tooltip>
        )}
      </For>

      <div class="toolbar-divider" role="separator" aria-orientation="horizontal" />

      <For each={SELECT_TOOLS}>
        {(tool) => (
          <Tooltip
            label={tool.label}
            shortcut={comboForCommand(tool.command)}
            description={tool.description}
          >
            <button
              type="button"
              class="toolbar-btn"
              data-testid={`select-tool-${tool.id}`}
              aria-label={tool.label}
              aria-pressed={selectActive(tool.id)}
              onClick={() => {
                setSelectTool(tool.id);
                setIsSelectMode(true);
              }}
            >
              <tool.icon size={18} aria-hidden={true} />
            </button>
          </Tooltip>
        )}
      </For>
    </div>
  );
};

export default Toolbar;
