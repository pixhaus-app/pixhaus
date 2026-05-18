// Debug surface registered on window.__pixhaus_debug__.
//
// Exists so e2e specs (tests/e2e) can assert against UI state and drive
// test-only mutations without a roundtrip to the Rust backend. The
// surface mixes two kinds of API:
//   - Read-only state accessors (getActiveProject, getZoom, panel.*,
//     ipc.log/findByCmd/last/clear, etc.) — these return snapshots of
//     Solid signals or recorded IPC entries.
//   - Test-only mutators (dialog.enqueue*, command.dispatch,
//     layer.refresh / setActiveByIndex) — these intentionally change
//     UI state to bypass UI surfaces that aren't yet WebDriver-
//     addressable (native dialogs, palette edge cases, layer-state
//     races after createNewProject).
//
// Always installed (no DEV gate) so the same harness works against
// `pnpm tauri build --debug` binaries. The surface only exposes data
// already visible to anyone with DevTools open and only triggers code
// paths that the menu/palette already expose; there is no security
// boundary to cross here.

import type { IpcLogEntry } from "../ipc";
import { activeProject, recentProjects } from "../../project-state";
import {
  activeFrameIndex,
  activeLayerId,
  activeSpriteId,
  isSelectMode,
  onionSkin,
  onionSkinNext,
  onionSkinOpacity,
  onionSkinPrev,
  scrollX,
  scrollY,
  selectionLayerId,
  selectionRect,
  showPixelGrid,
  showTileGrid,
  zoom,
} from "../../canvas/canvas-state";
import { layers, refreshLayers, selectedLayerIds } from "../../layers/layer-state";
import { setActiveLayerId } from "../../canvas/canvas-state";
import {
  frameTags,
  frames,
  isLooping,
  isPlaying,
  selectedFrames,
} from "../../timeline/timeline-state";
import {
  closeSection,
  isLibraryCollapsed,
  isSectionOpen,
  isTimelineCollapsed,
  openSection,
} from "../../shell/rail-state";
import { clearSheetEntity } from "../../sheet/sheet-state";
import { isCommandPaletteOpen } from "../../palette-state";
import { isPreferencesOpen } from "../../preferences/preferences-state";
import {
  crashReportingDialogShown,
  crashReportingEnabled,
} from "../../preferences/preferences-store";
import {
  clearDialogQueue,
  enqueueConfirm,
  enqueueMessage,
  enqueueOpen,
  enqueueSave,
} from "../dialog";
import type { MessageDialogResult } from "@tauri-apps/plugin-dialog";
import { dispatchCommand } from "../../command-palette/command-registry";
import {
  activeTool,
  pixelPerfect,
  toolShape,
  toolSize,
  type BrushShape,
  type ToolType,
} from "../../canvas/tools/tool-state";

interface LayerDebug {
  /** Force a layer-list IPC + refresh of the layers signal. Use sparingly
   * — production code calls refreshLayers via the layer panel's effect.
   * Spec workaround for the layer-panel-mount race after createNewProject. */
  refresh(): void;
  /** Returns the currently cached layer list (after the most recent
   * layer_list IPC). Each entry carries id, name, kind, blend mode. */
  list(): unknown[];
  /** Sets activeLayerId directly to the layer at flat-index `i` (top-to-
   * bottom order matches the panel). Use when the auto-pick path fails
   * to populate activeLayerId after createNewProject. */
  setActiveByIndex(i: number): boolean;
}

interface ToolDebug {
  /** Active drawing tool. */
  active(): ToolType;
  /** Brush diameter in canvas pixels. */
  size(): number;
  /** Brush shape: pixel | circle | square. */
  shape(): BrushShape;
  /** Whether the pixel-perfect post-process is on. */
  pixelPerfect(): boolean;
}

interface CanvasDebug {
  /** Reads RGBA at a viewport pixel from the canvas element via getImageData.
   * Returns null if the canvas isn't mounted. Spec coords are
   * viewport-pixel (CSS), not sprite-pixel — call helpers/canvas.ts to
   * convert sprite coords. */
  getPixelAt(x: number, y: number): { r: number; g: number; b: number; a: number } | null;
  /** Width / height of the canvas backing buffer in CSS pixels. */
  size(): { width: number; height: number } | null;
}

interface CommandDebug {
  /** Dispatches a registered command by id. Equivalent to opening the
   * palette and clicking the item, but bypasses the UI — useful when
   * the palette filtering or item click is fragile under tauri-driver. */
  dispatch(id: string): void;
}

interface DialogDebug {
  /** Pre-queue a response to the next `open()` dialog call. */
  enqueueOpen(value: string | string[] | null): void;
  /** Pre-queue a response to the next `save()` dialog call. */
  enqueueSave(value: string | null): void;
  /** Pre-queue a response to the next `confirm()` dialog call. */
  enqueueConfirm(value: boolean): void;
  /** Pre-queue a response to the next `message()` dialog call. */
  enqueueMessage(value: MessageDialogResult): void;
  /** Drop every queued response. */
  clear(): void;
}

interface IpcDebug {
  /** Returns a shallow copy of the IPC log array. The entries inside are
   * the same references the tap pushes — mutating an entry's `args` /
   * `result` will mutate the underlying log. Treat the returned entries
   * as read-only or call `structuredClone(entry)` before mutating. */
  log(): IpcLogEntry[];
  /** Empties the IPC log in-place so subsequent assertions see only
   * commands fired after this point. */
  clear(): void;
  /** Returns log entries with a matching cmd, in original order. */
  findByCmd(cmd: string): IpcLogEntry[];
  /** Returns the last entry, or undefined if the log is empty. */
  last(): IpcLogEntry | undefined;
}

interface PanelDebug {
  layers(): boolean;
  timeline(): boolean;
  palette(): boolean;
  tilemap(): boolean;
  library(): boolean;
  sheet(): boolean;
  setSheetVisible(visible: boolean): void;
}

export interface PixhausDebug {
  // ── Project ────────────────────────────────────────────────────────────────
  getActiveProject(): unknown;
  getRecentProjects(): unknown;
  getActiveSpriteId(): number | null;
  getActiveLayerId(): number | null;
  getActiveFrameIndex(): number;
  // ── Active drawing tool (flat accessor) ───────────────────────────────────
  getActiveTool(): ToolType;
  // ── Viewport ──────────────────────────────────────────────────────────────
  getZoom(): number;
  getScroll(): { x: number; y: number };
  getShowPixelGrid(): boolean;
  getShowTileGrid(): boolean;
  // ── Selection ─────────────────────────────────────────────────────────────
  getSelectionRect(): unknown;
  getSelectionLayerId(): number | null;
  getIsSelectMode(): boolean;
  // ── Onion skin ────────────────────────────────────────────────────────────
  getOnionSkin(): { enabled: boolean; prev: number; next: number; opacity: number };
  // ── Layers / Frames ───────────────────────────────────────────────────────
  getLayerCount(): number;
  getSelectedLayerIds(): number[];
  getFrameCount(): number;
  getFrameTags(): unknown;
  getSelectedFrames(): number[];
  getIsPlaying(): boolean;
  getIsLooping(): boolean;
  // ── Modals & overlays ─────────────────────────────────────────────────────
  isCommandPaletteOpen(): boolean;
  isPreferencesOpen(): boolean;
  // ── Panels ────────────────────────────────────────────────────────────────
  panel: PanelDebug;
  // ── Crash reporting ───────────────────────────────────────────────────────
  getCrashReportingEnabled(): boolean;
  getCrashReportingDialogShown(): boolean;
  // ── IPC log ───────────────────────────────────────────────────────────────
  ipc: IpcDebug;
  // ── Native dialog mock queue ──────────────────────────────────────────────
  dialog: DialogDebug;
  // ── Direct command dispatch ──────────────────────────────────────────────
  command: CommandDebug;
  // ── Drawing tool state ────────────────────────────────────────────────────
  tool: ToolDebug;
  // ── Canvas pixel sampling ─────────────────────────────────────────────────
  canvas: CanvasDebug;
  // ── Layer panel debug actions ─────────────────────────────────────────────
  layer: LayerDebug;
}

interface TaggedWindow extends Window {
  __pixhaus_debug__?: PixhausDebug;
  __pixhaus_ipc_log__?: IpcLogEntry[];
}

/**
 * Registers __pixhaus_debug__ on window. The IPC log is populated by the
 * wrapped invoke in `../ipc.ts`, which command wrappers import directly —
 * no runtime monkey-patch needed here. Idempotent. Call once from main.tsx
 * before render().
 */
export function installDebugSurface(): void {
  const w = window as TaggedWindow;
  if (w.__pixhaus_debug__ !== undefined) return;

  const surface: PixhausDebug = {
    getActiveProject: () => activeProject(),
    getRecentProjects: () => recentProjects(),
    getActiveSpriteId: () => activeSpriteId(),
    getActiveLayerId: () => activeLayerId(),
    getActiveFrameIndex: () => activeFrameIndex(),
    getActiveTool: () => activeTool(),

    getZoom: () => zoom(),
    getScroll: () => ({ x: scrollX(), y: scrollY() }),
    getShowPixelGrid: () => showPixelGrid(),
    getShowTileGrid: () => showTileGrid(),

    getSelectionRect: () => selectionRect(),
    getSelectionLayerId: () => selectionLayerId(),
    getIsSelectMode: () => isSelectMode(),

    getOnionSkin: () => ({
      enabled: onionSkin(),
      prev: onionSkinPrev(),
      next: onionSkinNext(),
      opacity: onionSkinOpacity(),
    }),

    getLayerCount: () => layers().length,
    getSelectedLayerIds: () => [...selectedLayerIds()],
    getFrameCount: () => frames().length,
    getFrameTags: () => frameTags(),
    getSelectedFrames: () => [...selectedFrames()],
    getIsPlaying: () => isPlaying(),
    getIsLooping: () => isLooping(),

    isCommandPaletteOpen: () => isCommandPaletteOpen(),
    isPreferencesOpen: () => isPreferencesOpen(),

    panel: {
      layers: () => isSectionOpen("layers"),
      timeline: () => !isTimelineCollapsed(),
      palette: () => isSectionOpen("color"),
      tilemap: () => isSectionOpen("tilemap"),
      library: () => !isLibraryCollapsed(),
      sheet: () => isSectionOpen("reference"),
      setSheetVisible: (visible: boolean) => {
        if (visible) {
          openSection("reference");
        } else {
          closeSection("reference");
          clearSheetEntity();
        }
      },
    },

    getCrashReportingEnabled: () => crashReportingEnabled(),
    getCrashReportingDialogShown: () => crashReportingDialogShown(),

    ipc: {
      log: () => (w.__pixhaus_ipc_log__ ?? []).slice(),
      // Clear in-place (length = 0) so the array reference the IPC tap
      // captured stays the same one we're reading from. Reassigning would
      // detach the tap's writes from log()/findByCmd()/last() reads.
      clear: () => {
        const log = w.__pixhaus_ipc_log__;
        if (log !== undefined) log.length = 0;
      },
      findByCmd: (cmd: string) => (w.__pixhaus_ipc_log__ ?? []).filter((e) => e.cmd === cmd),
      last: () => {
        const log = w.__pixhaus_ipc_log__ ?? [];
        return log[log.length - 1];
      },
    },

    dialog: {
      enqueueOpen,
      enqueueSave,
      enqueueConfirm,
      enqueueMessage,
      clear: clearDialogQueue,
    },

    command: {
      dispatch: (id: string) => dispatchCommand(id),
    },

    tool: {
      active: () => activeTool(),
      size: () => toolSize(),
      shape: () => toolShape(),
      pixelPerfect: () => pixelPerfect(),
    },

    layer: {
      refresh: () => refreshLayers(),
      list: () => layers().slice(),
      setActiveByIndex: (i: number) => {
        const ls = layers();
        // Layers come back bottom-to-top from Rust; the panel reverses
        // for display. For tests, "index 0 = topmost" is the natural
        // mapping, so reverse here.
        const reversed = [...ls].reverse();
        const target = reversed[i];
        if (target === undefined) return false;
        setActiveLayerId(target.id);
        return true;
      },
    },

    canvas: {
      getPixelAt: (x, y) => {
        const el = document.querySelector<HTMLCanvasElement>(
          'canvas[data-testid="canvas-viewport"]',
        );
        if (el === null) return null;
        const ctx = el.getContext("2d");
        if (ctx === null) return null;
        try {
          const data = ctx.getImageData(x, y, 1, 1).data;
          return { r: data[0] ?? 0, g: data[1] ?? 0, b: data[2] ?? 0, a: data[3] ?? 0 };
        } catch {
          // getImageData throws on cross-origin or WebGL canvases. The
          // Pixhaus canvas uses WebGL2; spec needs a different sampling
          // strategy (readPixels via the renderer). Return null so the
          // caller can branch.
          return null;
        }
      },
      size: () => {
        const el = document.querySelector<HTMLCanvasElement>(
          'canvas[data-testid="canvas-viewport"]',
        );
        if (el === null) return null;
        return { width: el.width, height: el.height };
      },
    },
  };

  w.__pixhaus_debug__ = surface;
}

export type { IpcLogEntry } from "../ipc";
