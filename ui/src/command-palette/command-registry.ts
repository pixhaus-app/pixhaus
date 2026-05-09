import { open as dialogOpen, save as dialogSave, confirm, message } from "../lib/dialog";
import { isCommandPaletteOpen, openCommandPalette, closeCommandPalette } from "../palette-state";
import { openPreferences } from "../preferences/preferences-state";
import { keybindPreset, customKeybinds } from "../preferences/preferences-store";
import { ASEPRITE_DEFAULTS, PHOTOSHOP_DEFAULTS, defaultCombo } from "../keybinds/defaults";
import {
  createNewProject,
  openProjectByExtension,
  projectClose,
  projectSave,
  saveOrSaveAs,
  spriteAdd,
  spriteDelete,
  spriteList,
} from "../lib/commands/project";
import {
  exportAnimatedGif,
  exportAnimatedWebp,
  exportPngSpriteSheet,
  exportTmx,
} from "../lib/commands/exports";
import { activeProject, setActiveProject, pushRecentProject } from "../project-state";
import { extractFilename } from "../lib/utils/path";
import { reportCommandFailure } from "../lib/utils/errors";
import {
  activeSpriteId,
  setActiveSpriteId,
  activeFrameIndex,
  activeLayerId,
  setIsSelectMode,
  setSelectionRect,
  setTransformBounds,
  zoom,
  setZoom,
  showTileGrid,
  setShowTileGrid,
  showPixelGrid,
  setShowPixelGrid,
  onionSkin,
  setOnionSkin,
  resetViewport,
} from "../canvas/canvas-state";
import { snapZoom } from "../canvas/viewport";
import { setSelectTool } from "../canvas/select/select-state";
import {
  dispatchFlipX,
  dispatchFlipY,
  dispatchRotateCw,
  dispatchRotateCcw,
} from "../canvas/transform/transform-input";
import {
  canvasSelectAll,
  canvasInvertSelection,
  canvasSetSelection,
  canvasComposite,
} from "../lib/commands/canvas";
import { undo, redo } from "../lib/commands/undo";
import { frameAdd, frameDelete, frameDuplicate } from "../lib/commands/frames";
import { appAbout } from "../lib/commands/app_info";
import { pushToast } from "../lib/toast/toast-state";
import {
  addLayer,
  beginRename,
  deleteLayer,
  flattenVisibleLayers,
  isLayerPanelVisible,
  layers,
  mergeLayerDown,
  mergeSelectedLayers,
  nextAutoName,
  selectedLayerIds,
  setLayerPanelVisible,
  wrapLayersInGroup,
} from "../layers/layer-state";
import { refreshTimeline } from "../timeline/timeline-state";
import { isTimelinePanelVisible, setTimelinePanelVisible } from "../timeline/timeline-state";
import {
  isPalettePanelVisible,
  setPalettePanelVisible,
  isTilemapPanelVisible,
  setTilemapPanelVisible,
} from "../shell/panel-state";
import {
  tilemapTool,
  setTilemapTool,
  autotileMode,
  setAutotileMode,
} from "../tilemap/tilemap-state";
import { foregroundColor, setActiveTool, type ToolType } from "../canvas/tools/tool-state";
import {
  paletteAdd as paletteAddCmd,
  paletteAddColor as paletteAddColorCmd,
  paletteDelete as paletteDeleteCmd,
} from "../lib/commands/palette";
import { activePalette, palettes, refreshPalettes } from "../palette/palette-panel-state";

export type Command = {
  readonly id: string;
  readonly label: string;
  readonly category: string;
  readonly keywords?: readonly string[];
};

type CommandEntry = Command & { readonly handler: () => void };

const PIXHAUS_FILTER = [{ name: "Pixhaus Projects", extensions: ["pixhaus"] }];
const OPEN_FILTERS = [
  {
    name: "All supported files",
    extensions: ["pixhaus", "psd", "aseprite", "ase"],
  },
  { name: "Pixhaus Projects", extensions: ["pixhaus"] },
  { name: "Aseprite", extensions: ["aseprite", "ase"] },
  { name: "Photoshop Documents", extensions: ["psd"] },
];

// Default canvas size for new sprites. Matches the welcome screen's
// project_new flow, which leaves the initial sprite at this size.
const DEFAULT_SPRITE_SIZE = 32;

// Documentation URL opened by the help:docs command.
const DOCS_URL = "https://pixhaus.app/docs";

// Opens a native file-open dialog filtered to every readable project
// format. Returns the selected path or null.
async function pickOpenPath(): Promise<string | null> {
  return dialogOpen({ multiple: false as const, filters: OPEN_FILTERS });
}

// Opens a native file-save dialog. Returns the chosen path or null.
async function pickSavePath(): Promise<string | null> {
  const result = await dialogSave({ filters: PIXHAUS_FILTER });
  return typeof result === "string" ? result : null;
}

// Reads the live viewport size from the canvas container so zoom-fit
// can pick the correct snap level. Returns null when the canvas isn't
// mounted (welcome screen, no project open).
function getCanvasViewportRect(): { width: number; height: number } | null {
  const el = document.querySelector(".canvas-container");
  if (!(el instanceof HTMLElement)) return null;
  const rect = el.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) return null;
  return { width: rect.width, height: rect.height };
}

/**
 * Opens a save dialog filtered to a single export extension and returns
 * the chosen path. Centralised so every export entry uses the same
 * dialog shape.
 */
async function pickExportPath(extension: string, label: string): Promise<string | null> {
  const result = await dialogSave({
    filters: [{ name: label, extensions: [extension] }],
  });
  return typeof result === "string" ? result : null;
}

/** Names the IPC operation a path triggers, for error reporting. */
function inferOpenOperation(path: string): string {
  const lower = path.toLowerCase();
  if (lower.endsWith(".psd")) return "project_import_psd";
  if (lower.endsWith(".aseprite") || lower.endsWith(".ase")) {
    return "project_import_aseprite";
  }
  return "project_open";
}

/**
 * Returns the active sprite id for an export, or null if no project /
 * sprite is open. Export commands need an active sprite; the menu and
 * palette items are dispatched without context, so this resolves it on
 * the fly. The first sprite is a pragmatic default — multi-sprite
 * projects can wire a sprite picker later.
 */
function activeSpriteIdForExport(): number | null {
  const project = activeProject();
  if (project === null) return null;
  // The project-state store doesn't expose the sprite list; fall back
  // to the canvas's active id which is the editor's current sprite.
  // `activeSpriteId` is imported from "../canvas/canvas-state" already.
  const id = activeSpriteId();
  return id;
}

type ExportSpec = {
  id: string;
  label: string;
  keywords: string[];
  extension: string;
  formatLabel: string;
  operation: string;
  invoke: (spriteId: number, outputPath: string) => Promise<unknown>;
};

function buildExportCommand(spec: ExportSpec): [string, CommandEntry] {
  return [
    spec.id,
    {
      id: spec.id,
      label: spec.label,
      category: "File",
      keywords: spec.keywords,
      handler: () => {
        const spriteId = activeSpriteIdForExport();
        if (spriteId === null) return;
        pickExportPath(spec.extension, spec.formatLabel)
          .then((path) => {
            if (path === null) return;
            return spec.invoke(spriteId, path);
          })
          .catch((err: unknown) => reportCommandFailure(spec.operation, err));
      },
    },
  ];
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
        createNewProject("Untitled")
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
            return openProjectByExtension(path)
              .then(({ status }) => {
                setActiveProject(status);
                pushRecentProject({ name: extractFilename(path), path });
              })
              .catch((err: unknown) => reportCommandFailure(inferOpenOperation(path), err));
          })
          .catch((err: unknown) => reportCommandFailure("file_dialog", err));
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
        // saveOrSaveAs handles the first-save fallback automatically:
        // if the project has no path yet, it opens the Save As dialog
        // instead of bubbling the Validation error to a toast.
        saveOrSaveAs().catch((err: unknown) => reportCommandFailure("project_save", err));
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
  buildExportCommand({
    id: "file:export-png-sheet",
    label: "Export PNG Sprite Sheet...",
    keywords: ["png", "sheet", "atlas", "sprite"],
    extension: "png",
    formatLabel: "PNG image",
    operation: "export_png_sprite_sheet",
    invoke: (spriteId, output_path) => exportPngSpriteSheet({ sprite_id: spriteId, output_path }),
  }),
  buildExportCommand({
    id: "file:export-gif",
    label: "Export Animated GIF...",
    keywords: ["gif", "animation", "export"],
    extension: "gif",
    formatLabel: "Animated GIF",
    operation: "export_animated_gif",
    invoke: (spriteId, output_path) => exportAnimatedGif({ sprite_id: spriteId, output_path }),
  }),
  buildExportCommand({
    id: "file:export-webp",
    label: "Export Animated WebP...",
    keywords: ["webp", "animation", "export"],
    extension: "webp",
    formatLabel: "Animated WebP",
    operation: "export_animated_webp",
    invoke: (spriteId, output_path) => exportAnimatedWebp({ sprite_id: spriteId, output_path }),
  }),
  buildExportCommand({
    id: "file:export-tmx",
    label: "Export Tilemap (Tiled .tmx)...",
    keywords: ["tmx", "tiled", "tilemap", "export"],
    extension: "tmx",
    formatLabel: "Tiled tilemap",
    operation: "export_tmx",
    invoke: (spriteId, output_path) => exportTmx({ sprite_id: spriteId, output_path }),
  }),
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
  //
  // edit:cut, edit:copy, edit:paste are intentionally absent. They require
  // a clipboard pipeline (Tauri clipboard plugin + selection-bounds → pixel
  // bytes round-trip) that hasn't been wired yet, and the existing IPC
  // surface can't shape the operation cleanly in isolation. Keybinds in
  // defaults.ts still reference these IDs; dispatchCommand will log an
  // unknown-command warning until the clipboard work lands.
  [
    "edit:undo",
    {
      id: "edit:undo",
      label: "Undo",
      category: "Edit",
      handler: () => {
        undo().catch((err: unknown) => {
          const e = err as { kind?: string };
          // NothingToUndo is an expected "stack empty" state, not an error
          // worth a toast — the menu/keybind fires whether or not there's
          // anything queued. Drop it on the floor.
          if (e?.kind === "NothingToUndo") return;
          reportCommandFailure("undo", err);
        });
      },
    },
  ],
  [
    "edit:redo",
    {
      id: "edit:redo",
      label: "Redo",
      category: "Edit",
      handler: () => {
        redo().catch((err: unknown) => {
          const e = err as { kind?: string };
          if (e?.kind === "NothingToRedo") return;
          reportCommandFailure("redo", err);
        });
      },
    },
  ],
  [
    "edit:select-all",
    {
      id: "edit:select-all",
      label: "Select All",
      category: "Edit",
      handler: () => {
        const spriteId = activeSpriteId();
        if (spriteId === null) return;
        canvasSelectAll(spriteId)
          .then((state) => {
            if (state.region?.kind === "rect") {
              const b = state.region.bounds;
              setSelectionRect({
                x: b.origin.x,
                y: b.origin.y,
                width: b.size.width,
                height: b.size.height,
              });
            }
            setIsSelectMode(true);
          })
          .catch((err: unknown) => reportCommandFailure("canvas_select_all", err));
      },
    },
  ],
  [
    "edit:deselect",
    {
      id: "edit:deselect",
      label: "Deselect",
      category: "Edit",
      handler: () => {
        canvasSetSelection(null, null)
          .then(() => {
            setSelectionRect(null);
            setIsSelectMode(false);
            setTransformBounds(null);
          })
          .catch((err: unknown) => reportCommandFailure("canvas_set_selection", err));
      },
    },
  ],

  // ── Sprite ────────────────────────────────────────────────────────────────
  [
    "sprite:new",
    {
      id: "sprite:new",
      label: "New Sprite",
      category: "Sprite",
      handler: () => {
        if (activeProject() === null) {
          pushToast({ kind: "info", title: "Open or create a project first." });
          return;
        }
        spriteAdd({
          name: "Untitled",
          canvas_width: DEFAULT_SPRITE_SIZE,
          canvas_height: DEFAULT_SPRITE_SIZE,
          color_mode: "rgba",
        })
          .then((sprite) => {
            // Sit the new sprite under the cursor: clear any previous
            // selection, point the canvas at the new id, and reset the
            // viewport so it appears centred.
            setSelectionRect(null);
            setTransformBounds(null);
            const vp = getCanvasViewportRect();
            if (vp !== null) {
              resetViewport(
                sprite.canvas.width,
                sprite.canvas.height,
                vp.width,
                vp.height,
                sprite.id,
              );
            } else {
              setActiveSpriteId(sprite.id);
            }
          })
          .catch((err: unknown) => reportCommandFailure("sprite_add", err));
      },
    },
  ],
  [
    "sprite:delete",
    {
      id: "sprite:delete",
      label: "Delete Sprite",
      category: "Sprite",
      handler: () => {
        const id = activeSpriteId();
        if (id === null) {
          pushToast({ kind: "info", title: "No active sprite to delete." });
          return;
        }
        confirm("Delete the active sprite? This can be undone.", {
          title: "Delete Sprite",
          kind: "warning",
        })
          .then((ok) => {
            if (!ok) return;
            return spriteDelete(id).then(() => {
              // Pick a remaining sprite (if any) so the canvas doesn't
              // sit on a dead id.
              return spriteList().then((sprites) => {
                const next = sprites[0];
                if (next === undefined) {
                  setActiveSpriteId(null);
                  return;
                }
                const vp = getCanvasViewportRect();
                if (vp !== null) {
                  resetViewport(
                    next.canvas.width,
                    next.canvas.height,
                    vp.width,
                    vp.height,
                    next.id,
                  );
                } else {
                  setActiveSpriteId(next.id);
                }
              });
            });
          })
          .catch((err: unknown) => reportCommandFailure("sprite_delete", err));
      },
    },
  ],

  // ── Frame ─────────────────────────────────────────────────────────────────
  [
    "frame:new",
    {
      id: "frame:new",
      label: "Add Frame",
      category: "Frame",
      handler: () => {
        const spriteId = activeSpriteId();
        if (spriteId === null) return;
        // Re-use the duration of the currently-active frame so quick
        // additions keep the timeline cadence consistent. frame_add
        // always appends to the end; if the user wants insertion
        // semantics they can use the timeline panel.
        frameAdd(spriteId, 100)
          .then(() => refreshTimeline())
          .catch((err: unknown) => reportCommandFailure("frame_add", err));
      },
    },
  ],
  [
    "frame:delete",
    {
      id: "frame:delete",
      label: "Delete Frame",
      category: "Frame",
      handler: () => {
        const spriteId = activeSpriteId();
        if (spriteId === null) return;
        const idx = activeFrameIndex();
        frameDelete(spriteId, idx)
          .then(() => refreshTimeline())
          .catch((err: unknown) => reportCommandFailure("frame_delete", err));
      },
    },
  ],
  [
    "frame:duplicate",
    {
      id: "frame:duplicate",
      label: "Duplicate Frame",
      category: "Frame",
      handler: () => {
        const spriteId = activeSpriteId();
        if (spriteId === null) return;
        const idx = activeFrameIndex();
        frameDuplicate(spriteId, idx)
          .then(() => refreshTimeline())
          .catch((err: unknown) => reportCommandFailure("frame_duplicate", err));
      },
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
        if (id !== null) addLayer(id, nextAutoName(layers(), "Layer"));
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
      handler: () => {
        const spriteId = activeSpriteId();
        const layerId = activeLayerId();
        if (spriteId === null || layerId === null) return;
        mergeLayerDown(spriteId, layerId);
      },
    },
  ],
  [
    "layer:merge-selected",
    {
      id: "layer:merge-selected",
      label: "Merge Selected Layers",
      category: "Layer",
      keywords: ["merge", "selected"],
      handler: () => {
        const spriteId = activeSpriteId();
        if (spriteId === null) return;
        mergeSelectedLayers(spriteId, selectedLayerIds());
      },
    },
  ],
  [
    "layer:flatten-visible",
    {
      id: "layer:flatten-visible",
      label: "Flatten Visible Layers",
      category: "Layer",
      keywords: ["flatten", "visible"],
      handler: () => {
        const spriteId = activeSpriteId();
        if (spriteId === null) return;
        flattenVisibleLayers(spriteId);
      },
    },
  ],
  [
    "layer:wrap-in-group",
    {
      id: "layer:wrap-in-group",
      label: "Convert to Group",
      category: "Layer",
      keywords: ["group", "wrap", "convert"],
      handler: () => {
        const spriteId = activeSpriteId();
        const layerId = activeLayerId();
        if (spriteId === null || layerId === null) return;
        const ids = selectedLayerIds().size > 0 ? [...selectedLayerIds()] : [layerId];
        wrapLayersInGroup(spriteId, ids);
      },
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

  // ── Palette ───────────────────────────────────────────────────────────────
  [
    "palette:new",
    {
      id: "palette:new",
      label: "New Palette",
      category: "Palette",
      keywords: ["palette", "new", "create"],
      handler: () => {
        const spriteId = activeSpriteId();
        if (spriteId === null) return;
        const existing = palettes();
        const name = `Palette ${existing.length + 1}`;
        paletteAddCmd(spriteId, name)
          .then(() => refreshPalettes(spriteId))
          .catch((err: unknown) => reportCommandFailure("palette_add", err));
      },
    },
  ],
  [
    "palette:delete",
    {
      id: "palette:delete",
      label: "Delete Active Palette",
      category: "Palette",
      keywords: ["palette", "delete", "remove"],
      handler: () => {
        const spriteId = activeSpriteId();
        const pal = activePalette();
        if (spriteId === null || pal === null) return;
        paletteDeleteCmd(spriteId, pal.id)
          .then(() => refreshPalettes(spriteId))
          .catch((err: unknown) => reportCommandFailure("palette_delete", err));
      },
    },
  ],
  [
    "palette:add-color",
    {
      id: "palette:add-color",
      label: "Add Foreground to Palette",
      category: "Palette",
      keywords: ["palette", "add", "color", "swatch"],
      handler: () => {
        const spriteId = activeSpriteId();
        const pal = activePalette();
        if (spriteId === null || pal === null) return;
        paletteAddColorCmd({
          sprite_id: spriteId,
          palette_id: pal.id,
          color: foregroundColor(),
        })
          .then(() => refreshPalettes(spriteId))
          .catch((err: unknown) => reportCommandFailure("palette_add_color", err));
      },
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
    "select:tool-rect",
    {
      id: "select:tool-rect",
      label: "Rectangle Marquee",
      category: "Select",
      keywords: ["select", "rect", "marquee"],
      handler: () => {
        setSelectTool("rect");
        setIsSelectMode(true);
      },
    },
  ],
  [
    "select:tool-ellipse",
    {
      id: "select:tool-ellipse",
      label: "Ellipse Marquee",
      category: "Select",
      keywords: ["select", "ellipse", "oval", "circle", "marquee"],
      handler: () => {
        setSelectTool("ellipse");
        setIsSelectMode(true);
      },
    },
  ],
  [
    "select:tool-lasso",
    {
      id: "select:tool-lasso",
      label: "Lasso",
      category: "Select",
      keywords: ["select", "lasso", "freehand", "polygon"],
      handler: () => {
        setSelectTool("lasso");
        setIsSelectMode(true);
      },
    },
  ],
  [
    "select:tool-wand",
    {
      id: "select:tool-wand",
      label: "Magic Wand",
      category: "Select",
      keywords: ["select", "wand", "magic", "flood", "fill"],
      handler: () => {
        setSelectTool("wand");
        setIsSelectMode(true);
      },
    },
  ],
  [
    "select:tool-color-range",
    {
      id: "select:tool-color-range",
      label: "Color Range",
      category: "Select",
      keywords: ["select", "color", "range", "hue"],
      handler: () => {
        setSelectTool("color-range");
        setIsSelectMode(true);
      },
    },
  ],
  [
    "select:invert",
    {
      id: "select:invert",
      label: "Invert Selection",
      category: "Select",
      keywords: ["invert", "inverse"],
      handler: () => {
        const spriteId = activeSpriteId();
        const layerId = activeLayerId();
        if (spriteId === null) return;
        canvasInvertSelection(spriteId, layerId)
          .then((state) => {
            if (state.region?.kind === "rect") {
              const b = state.region.bounds;
              setSelectionRect({
                x: b.origin.x,
                y: b.origin.y,
                width: b.size.width,
                height: b.size.height,
              });
            } else {
              setSelectionRect(null);
            }
          })
          .catch((err: unknown) => {
            const e = err as { kind?: string; stream?: string };
            if (e?.kind === "unimplemented") {
              pushToast({
                title: `Invert selection requires ${e.stream ?? "S01"} — not yet available.`,
                kind: "info",
              });
            } else {
              reportCommandFailure("canvas_invert_selection", err);
            }
          });
      },
    },
  ],

  // ── Transform ─────────────────────────────────────────────────────────────
  [
    "transform:flip-x",
    {
      id: "transform:flip-x",
      label: "Flip Horizontal",
      category: "Transform",
      keywords: ["flip", "mirror", "horizontal"],
      handler: () => dispatchFlipX(),
    },
  ],
  [
    "transform:flip-y",
    {
      id: "transform:flip-y",
      label: "Flip Vertical",
      category: "Transform",
      keywords: ["flip", "mirror", "vertical"],
      handler: () => dispatchFlipY(),
    },
  ],
  [
    "transform:rotate-cw",
    {
      id: "transform:rotate-cw",
      label: "Rotate 90° Clockwise",
      category: "Transform",
      keywords: ["rotate", "clockwise", "cw", "90"],
      handler: () => dispatchRotateCw(),
    },
  ],
  [
    "transform:rotate-ccw",
    {
      id: "transform:rotate-ccw",
      label: "Rotate 90° Counter-clockwise",
      category: "Transform",
      keywords: ["rotate", "counter", "ccw", "90"],
      handler: () => dispatchRotateCcw(),
    },
  ],

  // ── Tool selection ────────────────────────────────────────────────────────
  // Single-letter shortcuts (P/E/G/L/U/O) live in keybinds/defaults.ts.
  // They route through the registry so a "Tools" category shows up in the
  // command palette and the tool keys can be remapped via preferences.
  // Typed as `[ToolType, string][]` so any drift between this list and the
  // `ToolType` union shows up as a typecheck error rather than a silent
  // unknown-id at runtime.
  ...(
    [
      ["pencil", "Pencil"],
      ["eraser", "Eraser"],
      ["fill", "Fill"],
      ["line", "Line"],
      ["rect", "Rectangle"],
      ["ellipse", "Ellipse"],
    ] as const satisfies ReadonlyArray<readonly [ToolType, string]>
  ).map<[string, CommandEntry]>(([id, label]) => [
    `tool:${id}`,
    {
      id: `tool:${id}`,
      label: `Tool: ${label}`,
      category: "Tools",
      keywords: ["tool", id],
      handler: () => setActiveTool(id),
    },
  ]),

  // ── View ──────────────────────────────────────────────────────────────────
  [
    "view:zoom-in",
    {
      id: "view:zoom-in",
      label: "Zoom In",
      category: "View",
      handler: () => setZoom(snapZoom(zoom(), 1)),
    },
  ],
  [
    "view:zoom-out",
    {
      id: "view:zoom-out",
      label: "Zoom Out",
      category: "View",
      handler: () => setZoom(snapZoom(zoom(), -1)),
    },
  ],
  [
    "view:zoom-fit",
    {
      id: "view:zoom-fit",
      label: "Fit to Window",
      category: "View",
      handler: () => {
        const spriteId = activeSpriteId();
        if (spriteId === null) return;
        const vp = getCanvasViewportRect();
        if (vp === null) return;
        // Fetch the sprite's canvas dimensions so resetViewport picks the
        // right snap level. canvas_composite is cheap (returns metadata
        // only) and avoids duplicating spriteList here.
        canvasComposite(spriteId)
          .then((info) => {
            resetViewport(info.sprite_width, info.sprite_height, vp.width, vp.height, spriteId);
          })
          .catch((err: unknown) => reportCommandFailure("canvas_composite", err));
      },
    },
  ],
  [
    "view:zoom-100",
    {
      id: "view:zoom-100",
      label: "100%",
      category: "View",
      handler: () => setZoom(1),
    },
  ],
  [
    "view:toggle-grid",
    {
      id: "view:toggle-grid",
      label: "Toggle Grid",
      category: "View",
      handler: () => setShowTileGrid(!showTileGrid()),
    },
  ],
  [
    "view:toggle-pixel-grid",
    {
      id: "view:toggle-pixel-grid",
      label: "Toggle Pixel Grid",
      category: "View",
      handler: () => setShowPixelGrid(!showPixelGrid()),
    },
  ],
  [
    "view:toggle-onion-skin",
    {
      id: "view:toggle-onion-skin",
      label: "Toggle Onion Skin",
      category: "View",
      keywords: ["onion", "skin"],
      handler: () => setOnionSkin(!onionSkin()),
    },
  ],

  // ── AI ────────────────────────────────────────────────────────────────────
  // The verb runtime + backend-config flow is a separate effort. Until then
  // these entries forward to the unified stub so the menu remains discoverable.
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
      handler: () => setTimelinePanelVisible(!isTimelinePanelVisible()),
    },
  ],
  [
    "window:toggle-palette",
    {
      id: "window:toggle-palette",
      label: "Toggle Color Palette",
      category: "Window",
      handler: () => setPalettePanelVisible(!isPalettePanelVisible()),
    },
  ],
  [
    "window:toggle-tilemap",
    {
      id: "window:toggle-tilemap",
      label: "Toggle Tilemap Panel",
      category: "Window",
      keywords: ["tiles", "autotile", "tileset"],
      handler: () => setTilemapPanelVisible(!isTilemapPanelVisible()),
    },
  ],

  // ── Help ──────────────────────────────────────────────────────────────────
  [
    "help:docs",
    {
      id: "help:docs",
      label: "Documentation",
      category: "Help",
      handler: () => {
        // No shell-open / opener plugin is registered yet, so route through
        // the browser. The Tauri webview honours window.open with
        // tauri.conf.json's default permissions; if a future config locks
        // this down, swap in tauri-plugin-opener.
        window.open(DOCS_URL, "_blank");
      },
    },
  ],
  [
    "help:about",
    {
      id: "help:about",
      label: "About Pixhaus",
      category: "Help",
      handler: () => {
        appAbout()
          .then((info) => {
            return message(`${info.name} ${info.version}`, {
              title: "About Pixhaus",
              kind: "info",
            });
          })
          .catch((err: unknown) => reportCommandFailure("app_about", err));
      },
    },
  ],
]);

function stub(id: string): () => void {
  return () => console.warn(`[pixhaus] command "${id}" not yet implemented`);
}

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
