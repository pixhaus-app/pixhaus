import { Show, onMount, onCleanup, type Component } from "solid-js";
import { listen } from "@tauri-apps/api/event";
import { isCommandPaletteOpen } from "../palette-state";
import { isPreferencesOpen } from "../preferences/preferences-state";
import { activeProject } from "../project-state";
import { activeSpriteId } from "../canvas/canvas-state";
import { dispatchCommand } from "../command-palette/command-registry";
import { setupKeybindManager } from "../keybinds/keybind-manager";
import CommandPalette from "../command-palette/CommandPalette";
import PreferencesModal from "../preferences/PreferencesModal";
import ToastHost from "../lib/toast/ToastHost";
import UpdateAvailableModal from "./UpdateAvailableModal";
import CanvasSizeDialog from "./CanvasSizeDialog";
import VerbInvokeHost from "../lib/ai/VerbInvokeHost";
import StatusBar from "./StatusBar";
import WelcomeScreen from "./WelcomeScreen";
import Canvas from "../canvas/Canvas";
import LayerPanel from "../layers/LayerPanel";
import { isLayerPanelVisible } from "../layers/layer-state";
import PalettePanel from "../palette/PalettePanel";
import TilemapPanel from "../tilemap/TilemapPanel";
import FirstLaunchDialog from "../crash-reporting/FirstLaunchDialog";
import {
  crashReportingDialogShown,
  crashReportingEnabled,
  crashReportingUid,
  hydrateCrashReportingFromBackend,
  setCrashReportingEnabled,
  markCrashReportingDialogShown,
} from "../preferences/preferences-store";
import {
  initCrashReporting,
  setCrashReportingEnabled as setSentryEnabled,
} from "../crash-reporting/crash-reporting";
import TimelinePanel from "../timeline/TimelinePanel";
import { isTimelinePanelVisible } from "../timeline/timeline-state";
import { isPalettePanelVisible, isTilemapPanelVisible } from "./panel-state";
import LibraryPanel from "../library/LibraryPanel";
import { isLibraryPanelVisible } from "../library/library-state";
import { setupPaletteColorSync } from "../canvas/tools/palette-color-sync";
import { installTilemapCtxSync } from "../tilemap/tilemap-ctx-sync";
import SheetView from "../sheet/SheetView";
import { isSheetPanelVisible } from "../sheet/sheet-state";

const Shell: Component = () => {
  // Bridge palette FG/BG indices to the tool-state RGBA signals so brush
  // strokes use the selected swatch color instead of the initial black.
  setupPaletteColorSync();
  // Bridge active layer -> activeTilemapCtx so the tilemap panel and the
  // tile-paint code path light up when a tilemap layer is foregrounded.
  installTilemapCtxSync();

  onMount(() => {
    // Resolve the authoritative crash-reporting value from the backend
    // before initialising Sentry. The backend persists the choice to disk
    // (`app/src/crash_reporting.rs`), so this read returns the user's real
    // preference across restarts rather than the in-memory Rust default.
    // localStorage is just a cache — it might disagree with the backend
    // if it was cleared, copied between machines, etc.
    //
    // Sentry isn't capturing during the IPC window (~10ms). The menu
    // listener and keybinds below register synchronously so user input is
    // never lost.
    hydrateCrashReportingFromBackend()
      .then((resolved) => {
        initCrashReporting({ enabled: resolved, uid: crashReportingUid });
        setSentryEnabled(resolved);
      })
      .catch((err: unknown) => {
        // Hydrator already falls back to localStorage and logs the
        // failure, so this catch is unreachable in practice. Belt-and-
        // braces: init Sentry from the localStorage-seeded signal so the
        // panic hook is wired up regardless of IPC health.
        console.error("[pixhaus] crash-reporting boot failed:", err);
        initCrashReporting({ enabled: crashReportingEnabled(), uid: crashReportingUid });
        setSentryEnabled(crashReportingEnabled());
      });

    // Forward native menu events to the command dispatcher
    const menuListenerPromise = listen<string>("shell:menu", (event) => {
      dispatchCommand(event.payload);
    });

    // Register keyboard shortcuts
    const removeKeybinds = setupKeybindManager();

    onCleanup(() => {
      menuListenerPromise
        .then((unlisten) => unlisten())
        .catch((err: unknown) => console.error("[pixhaus] failed to unlisten shell:menu:", err));
      removeKeybinds();
    });
  });

  function answerCrashReportingDialog(enabled: boolean): void {
    setCrashReportingEnabled(enabled);
    setSentryEnabled(enabled);
    markCrashReportingDialogShown();
  }

  return (
    <div class="shell" data-testid="shell">
      <div class="shell-body">
        <div class="shell-main">
          <Show when={activeProject() === null}>
            <WelcomeScreen />
          </Show>
          <Show when={activeProject() !== null}>
            <div class="editor-layout" data-testid="editor-layout">
              <Show when={isLibraryPanelVisible()}>
                <LibraryPanel />
              </Show>
              <div class="editor-layout__canvas-area">
                <div class="editor-layout__canvas">
                  <Canvas />
                </div>
                <Show when={isTimelinePanelVisible()}>
                  <TimelinePanel />
                </Show>
              </div>
              <Show when={activeSpriteId() !== null && isTilemapPanelVisible()}>
                <TilemapPanel />
              </Show>
              <Show when={isLayerPanelVisible()}>
                <LayerPanel />
              </Show>
              <Show when={isSheetPanelVisible()}>
                <SheetView />
              </Show>
            </div>
          </Show>
        </div>

        <Show when={isPalettePanelVisible()}>
          <PalettePanel spriteId={activeSpriteId()} />
        </Show>
      </div>

      <StatusBar />

      <Show when={isCommandPaletteOpen()}>
        <CommandPalette />
      </Show>

      <Show when={isPreferencesOpen()}>
        <PreferencesModal />
      </Show>

      <UpdateAvailableModal />

      <CanvasSizeDialog />

      <VerbInvokeHost />
      <ToastHost />

      <Show when={!crashReportingDialogShown()}>
        <FirstLaunchDialog
          onAccept={() => answerCrashReportingDialog(true)}
          onDecline={() => answerCrashReportingDialog(false)}
        />
      </Show>
    </div>
  );
};

export default Shell;
