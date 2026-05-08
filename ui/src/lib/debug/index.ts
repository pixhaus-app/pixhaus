// Read-only debug surface registered on window.__pixhaus_debug__.
//
// Exists so e2e specs (tests/e2e) can assert against UI state without a
// roundtrip to the Rust backend. Every accessor returns the current value
// of a Solid signal — the spec calls a function via WebDriver and gets a
// snapshot back. None of these mutate state; tests drive the UI through
// real input (keyboard, mouse, IPC), not through this object.
//
// Always installed (no DEV gate) so the same harness works against
// `pnpm tauri build --debug` binaries. The surface only exposes data
// already visible to anyone with DevTools open; there is no security
// boundary to cross here.

import { installIpcTap, type IpcLogEntry } from "./ipc-tap";
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
import { isLayerPanelVisible, layers, selectedLayerIds } from "../../layers/layer-state";
import {
  frameTags,
  frames,
  isLooping,
  isPlaying,
  isTimelinePanelVisible,
  selectedFrames,
} from "../../timeline/timeline-state";
import { isPalettePanelVisible, isTilemapPanelVisible } from "../../shell/panel-state";
import { isCommandPaletteOpen } from "../../palette-state";
import { isPreferencesOpen } from "../../preferences/preferences-state";
import {
  crashReportingDialogShown,
  crashReportingEnabled,
} from "../../preferences/preferences-store";

interface IpcDebug {
  /** Returns a snapshot copy of the IPC log. Mutating it does not affect future calls. */
  log(): IpcLogEntry[];
  /** Empties the IPC log so subsequent assertions see only commands fired after this point. */
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
}

export interface PixhausDebug {
  // ── Project ────────────────────────────────────────────────────────────────
  getActiveProject(): unknown;
  getRecentProjects(): unknown;
  getActiveSpriteId(): number | null;
  getActiveLayerId(): number | null;
  getActiveFrameIndex(): number;
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
}

interface TaggedWindow extends Window {
  __pixhaus_debug__?: PixhausDebug;
  __pixhaus_ipc_log__?: IpcLogEntry[];
}

/**
 * Wires up the IPC tap and registers __pixhaus_debug__ on window.
 * Idempotent. Call once from main.tsx before render().
 */
export function installDebugSurface(): void {
  installIpcTap();

  const w = window as TaggedWindow;
  if (w.__pixhaus_debug__ !== undefined) return;

  const surface: PixhausDebug = {
    getActiveProject: () => activeProject(),
    getRecentProjects: () => recentProjects(),
    getActiveSpriteId: () => activeSpriteId(),
    getActiveLayerId: () => activeLayerId(),
    getActiveFrameIndex: () => activeFrameIndex(),

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
      layers: () => isLayerPanelVisible(),
      timeline: () => isTimelinePanelVisible(),
      palette: () => isPalettePanelVisible(),
      tilemap: () => isTilemapPanelVisible(),
    },

    getCrashReportingEnabled: () => crashReportingEnabled(),
    getCrashReportingDialogShown: () => crashReportingDialogShown(),

    ipc: {
      log: () => (w.__pixhaus_ipc_log__ ?? []).slice(),
      clear: () => {
        w.__pixhaus_ipc_log__ = [];
      },
      findByCmd: (cmd: string) => (w.__pixhaus_ipc_log__ ?? []).filter((e) => e.cmd === cmd),
      last: () => {
        const log = w.__pixhaus_ipc_log__ ?? [];
        return log[log.length - 1];
      },
    },
  };

  w.__pixhaus_debug__ = surface;
}

export type { IpcLogEntry } from "./ipc-tap";
