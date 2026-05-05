import { open as dialogOpen, save as dialogSave } from "@tauri-apps/plugin-dialog";
import { isCommandPaletteOpen, openCommandPalette, closeCommandPalette } from "../palette-state";
import { openPreferences } from "../preferences/preferences-state";
import { keybindPreset, customKeybinds } from "../preferences/preferences-store";
import { ASEPRITE_DEFAULTS, PHOTOSHOP_DEFAULTS, defaultCombo } from "../keybinds/defaults";
import { projectNew, projectOpen, projectSave, projectClose } from "../lib/commands/project";
import { setActiveProject, pushRecentProject } from "../project-state";
import { extractFilename } from "../lib/utils/path";
import { reportCommandFailure } from "../lib/utils/errors";
import { activeSpriteId, activeLayerId } from "../canvas/canvas-state";
import {
  addLayer,
  beginRename,
  deleteLayer,
  isLayerPanelVisible,
  setLayerPanelVisible,
} from "../layers/layer-state";
import {
  tilemapTool,
  setTilemapTool,
  autotileMode,
  setAutotileMode,
} from "../tilemap/tilemap-state";

export type Command = {
  readonly id: string;
  readonly label: string;
  readonly category: string;
  readonly keywords?: readonly string[];
};

type CommandEntry = Command & { readonly handler: () => void };

const PIXHAUS_FILTER = [{ name: "Pixhaus Projects", extensions: ["pixhaus"] }];

// Opens a native file-open dialog. Returns the selected path or null.
async function pickOpenPath(): Promise<string | null> {
  return dialogOpen({ multiple: false as const, filters: PIXHAUS_FILTER });
}

// Opens a native file-save dialog. Returns the chosen path or null.
async function pickSavePath(): Promise<string | null> {
  const result = await dialogSave({ filters: PIXHAUS_FILTER });
  return typeof result === "string" ? result : null;
}

function stub(id: string): () => void {
  return () => console.warn(`[pixhaus] command "${id}" not yet implemented`);
}

const COMMANDS: ReadonlyMap<string, CommandEntry> = new Map<string, CommandEntry>([
  // ── File ─────────────────────────────────────────────────────────────────
  [
    "file:new",
    {
      id: "file:new",
      label: "New Project",
      category: "File",
      handler: () => {
        projectNew("Untitled")
          .then((status) => {
            setActiveProject(status);
          })
          .catch((err: unknown) => reportCommandFailure("project_new", err));
      },
    },
  ],
  [
    "file:open",
    {
      id: "file:open",
      label: "Open...",
      category: "File",
      handler: () => {
        pickOpenPath()
          .then((path) => {
            if (path === null) return;
            return projectOpen(path).then((status) => {
              setActiveProject(status);
              pushRecentProject({ name: extractFilename(path), path });
            });
          })
          .catch((err: unknown) => reportCommandFailure("project_open", err));
      },
    },
  ],
  [
    "file:save",
    {
      id: "file:save",
      label: "Save",
      category: "File",
      handler: () => {
        projectSave().catch((err: unknown) => reportCommandFailure("project_save", err));
      },
    },
  ],
  [
    "file:save-as",
    {
      id: "file:save-as",
      label: "Save As...",
      category: "File",
      handler: () => {
        pickSavePath()
          .then((path) => {
            if (path === null) return;
            return projectSave(path);
          })
          .catch((err: unknown) => reportCommandFailure("project_save_as", err));
      },
    },
  ],
  [
    "file:close",
    {
      id: "file:close",
      label: "Close Project",
      category: "File",
      handler: () => {
        projectClose()
          .then(() => setActiveProject(null))
          .catch((err: unknown) => reportCommandFailure("project_close", err));
      },
    },
  ],

  // ── Edit ──────────────────────────────────────────────────────────────────
  ["edit:undo", { id: "edit:undo", label: "Undo", category: "Edit", handler: stub("edit:undo") }],
  ["edit:redo", { id: "edit:redo", label: "Redo", category: "Edit", handler: stub("edit:redo") }],
  ["edit:cut", { id: "edit:cut", label: "Cut", category: "Edit", handler: stub("edit:cut") }],
  ["edit:copy", { id: "edit:copy", label: "Copy", category: "Edit", handler: stub("edit:copy") }],
  [
    "edit:paste",
    { id: "edit:paste", label: "Paste", category: "Edit", handler: stub("edit:paste") },
  ],
  [
    "edit:select-all",
    {
      id: "edit:select-all",
      label: "Select All",
      category: "Edit",
      handler: stub("edit:select-all"),
    },
  ],
  [
    "edit:deselect",
    { id: "edit:deselect", label: "Deselect", category: "Edit", handler: stub("edit:deselect") },
  ],

  // ── Sprite ────────────────────────────────────────────────────────────────
  [
    "sprite:new",
    { id: "sprite:new", label: "New Sprite", category: "Sprite", handler: stub("sprite:new") },
  ],
  [
    "sprite:delete",
    {
      id: "sprite:delete",
      label: "Delete Sprite",
      category: "Sprite",
      handler: stub("sprite:delete"),
    },
  ],

  // ── Frame ─────────────────────────────────────────────────────────────────
  [
    "frame:new",
    { id: "frame:new", label: "Add Frame", category: "Frame", handler: stub("frame:new") },
  ],
  [
    "frame:delete",
    { id: "frame:delete", label: "Delete Frame", category: "Frame", handler: stub("frame:delete") },
  ],
  [
    "frame:duplicate",
    {
      id: "frame:duplicate",
      label: "Duplicate Frame",
      category: "Frame",
      handler: stub("frame:duplicate"),
    },
  ],

  // ── Layer ─────────────────────────────────────────────────────────────────
  [
    "layer:new",
    {
      id: "layer:new",
      label: "New Layer",
      category: "Layer",
      handler: () => {
        const id = activeSpriteId();
        if (id !== null) addLayer(id, "Layer");
      },
    },
  ],
  [
    "layer:delete",
    {
      id: "layer:delete",
      label: "Delete Layer",
      category: "Layer",
      handler: () => {
        const spriteId = activeSpriteId();
        const layerId = activeLayerId();
        if (spriteId !== null && layerId !== null) deleteLayer(spriteId, layerId);
      },
    },
  ],
  [
    "layer:rename",
    {
      id: "layer:rename",
      label: "Rename Layer",
      category: "Layer",
      handler: () => {
        const layerId = activeLayerId();
        if (layerId !== null) beginRename(layerId);
      },
    },
  ],
  [
    "layer:merge-down",
    {
      id: "layer:merge-down",
      label: "Merge Down",
      category: "Layer",
      handler: stub("layer:merge-down"),
    },
  ],
  [
    "layer:flatten",
    {
      id: "layer:flatten",
      label: "Flatten All Layers",
      category: "Layer",
      handler: stub("layer:flatten"),
    },
  ],

  // ── Tilemap ───────────────────────────────────────────────────────────────
  [
    "tilemap:tool-pencil",
    {
      id: "tilemap:tool-pencil",
      label: "Tile pencil",
      category: "Tilemap",
      keywords: ["paint", "place", "draw"],
      handler: () => setTilemapTool("pencil"),
    },
  ],
  [
    "tilemap:tool-erase",
    {
      id: "tilemap:tool-erase",
      label: "Tile eraser",
      category: "Tilemap",
      keywords: ["erase", "clear", "delete"],
      handler: () => setTilemapTool("erase"),
    },
  ],
  [
    "tilemap:toggle-autotile",
    {
      id: "tilemap:toggle-autotile",
      label: "Toggle autotile mode",
      category: "Tilemap",
      keywords: ["autotile", "auto", "wang", "rules"],
      handler: () => setAutotileMode(!autotileMode()),
    },
  ],
  [
    "tilemap:toggle-tool",
    {
      id: "tilemap:toggle-tool",
      label: "Toggle tile tool (pencil/erase)",
      category: "Tilemap",
      handler: () => setTilemapTool(tilemapTool() === "pencil" ? "erase" : "pencil"),
    },
  ],

  // ── Select ────────────────────────────────────────────────────────────────
  [
    "select:all",
    { id: "select:all", label: "Select All", category: "Select", handler: stub("select:all") },
  ],
  [
    "select:deselect",
    {
      id: "select:deselect",
      label: "Deselect",
      category: "Select",
      handler: stub("select:deselect"),
    },
  ],
  [
    "select:invert",
    {
      id: "select:invert",
      label: "Invert Selection",
      category: "Select",
      handler: stub("select:invert"),
    },
  ],

  // ── View ──────────────────────────────────────────────────────────────────
  [
    "view:zoom-in",
    { id: "view:zoom-in", label: "Zoom In", category: "View", handler: stub("view:zoom-in") },
  ],
  [
    "view:zoom-out",
    { id: "view:zoom-out", label: "Zoom Out", category: "View", handler: stub("view:zoom-out") },
  ],
  [
    "view:zoom-fit",
    {
      id: "view:zoom-fit",
      label: "Fit to Window",
      category: "View",
      handler: stub("view:zoom-fit"),
    },
  ],
  [
    "view:zoom-100",
    { id: "view:zoom-100", label: "100%", category: "View", handler: stub("view:zoom-100") },
  ],
  [
    "view:toggle-grid",
    {
      id: "view:toggle-grid",
      label: "Toggle Grid",
      category: "View",
      handler: stub("view:toggle-grid"),
    },
  ],
  [
    "view:toggle-pixel-grid",
    {
      id: "view:toggle-pixel-grid",
      label: "Toggle Pixel Grid",
      category: "View",
      handler: stub("view:toggle-pixel-grid"),
    },
  ],

  // ── AI ────────────────────────────────────────────────────────────────────
  [
    "ai:inbetween",
    {
      id: "ai:inbetween",
      label: "Inbetween",
      category: "AI",
      keywords: ["tween", "interpolate"],
      handler: stub("ai:inbetween"),
    },
  ],
  [
    "ai:continue",
    { id: "ai:continue", label: "Continue", category: "AI", handler: stub("ai:continue") },
  ],
  [
    "ai:variant",
    { id: "ai:variant", label: "Variant", category: "AI", handler: stub("ai:variant") },
  ],
  [
    "ai:cleanup",
    { id: "ai:cleanup", label: "Cleanup", category: "AI", handler: stub("ai:cleanup") },
  ],
  [
    "ai:critique",
    { id: "ai:critique", label: "Critique", category: "AI", handler: stub("ai:critique") },
  ],
  [
    "ai:settings",
    {
      id: "ai:settings",
      label: "AI Backend Settings",
      category: "AI",
      handler: stub("ai:settings"),
    },
  ],

  // ── Window ────────────────────────────────────────────────────────────────
  [
    "window:command-palette",
    {
      id: "window:command-palette",
      label: "Command Palette",
      category: "Window",
      keywords: ["search", "commands"],
      handler: () => {
        if (isCommandPaletteOpen()) closeCommandPalette();
        else openCommandPalette();
      },
    },
  ],
  [
    "window:preferences",
    {
      id: "window:preferences",
      label: "Preferences",
      category: "Window",
      keywords: ["settings", "theme", "keybinds"],
      handler: () => openPreferences(),
    },
  ],
  [
    "window:toggle-layers",
    {
      id: "window:toggle-layers",
      label: "Toggle Layer Panel",
      category: "Window",
      handler: () => setLayerPanelVisible(!isLayerPanelVisible()),
    },
  ],
  [
    "window:toggle-timeline",
    {
      id: "window:toggle-timeline",
      label: "Toggle Timeline",
      category: "Window",
      handler: stub("window:toggle-timeline"),
    },
  ],
  [
    "window:toggle-palette",
    {
      id: "window:toggle-palette",
      label: "Toggle Color Palette",
      category: "Window",
      handler: stub("window:toggle-palette"),
    },
  ],
  [
    "window:toggle-tilemap",
    {
      id: "window:toggle-tilemap",
      label: "Toggle Tilemap Panel",
      category: "Window",
      keywords: ["tiles", "autotile", "tileset"],
      handler: stub("window:toggle-tilemap"),
    },
  ],

  // ── Help ──────────────────────────────────────────────────────────────────
  [
    "help:docs",
    { id: "help:docs", label: "Documentation", category: "Help", handler: stub("help:docs") },
  ],
  [
    "help:about",
    { id: "help:about", label: "About Pixhaus", category: "Help", handler: stub("help:about") },
  ],
]);

// Returns all commands with their current keybind resolved from preferences.
export function getAllCommands(): ReadonlyArray<Command & { keybind?: string }> {
  const preset = keybindPreset();
  const table = preset === "photoshop" ? PHOTOSHOP_DEFAULTS : ASEPRITE_DEFAULTS;
  const custom = customKeybinds();

  return Array.from(COMMANDS.values()).map((cmd) => {
    const customCombo = custom[cmd.id];
    const presetCombo = customCombo !== undefined ? undefined : defaultCombo(table, cmd.id);
    const keybind = customCombo ?? presetCombo;
    return keybind !== undefined ? { ...cmd, keybind } : { ...cmd };
  });
}

// Calls the handler for the given command id. No-op for unknown ids.
export function dispatchCommand(id: string): void {
  const entry = COMMANDS.get(id);
  if (entry) {
    entry.handler();
  } else {
    console.warn(`[pixhaus] unknown command: "${id}"`);
  }
}
