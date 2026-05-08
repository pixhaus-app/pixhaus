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
import { setupPaletteColorSync } from "../canvas/tools/palette-color-sync";
import { installTilemapCtxSync } from "../tilemap/tilemap-ctx-sync";

const Shell: Component = () => {
  // Bridge palette FG/BG indices to the tool-state RGBA signals so brush
  // strokes use the selected swatch color instead of the initial black.
  setupPaletteColorSync();
  // Bridge active layer -> activeTilemapCtx so the tilemap panel and the
  // tile-paint code path light up when a tilemap layer is foregrounded.
  installTilemapCtxSync();

  onMount(() => {
    // Initialise JS-layer crash reporting and sync the Rust-side gate so
    // both honour the persisted preference; without the second call the
    // Rust panic hook drops events until the user re-interacts with the
    // dialog. Fire-and-forget — setSentryEnabled handles IPC errors.
    initCrashReporting({ enabled: crashReportingEnabled(), uid: crashReportingUid });
    setSentryEnabled(crashReportingEnabled());

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
    <div class="shell">
      <div class="shell-body">
        <div class="shell-main">
          <Show when={activeProject() === null}>
            <WelcomeScreen />
          </Show>
          <Show when={activeProject() !== null}>
            <div class="editor-layout">
              <div class="editor-layout__canvas-area">
                <div class="editor-layout__canvas">
                  <Canvas />
                  <Show when={activeSpriteId() !== null && isTilemapPanelVisible()}>
                    <TilemapPanel />
                  </Show>
                </div>
                <Show when={isTimelinePanelVisible()}>
                  <TimelinePanel />
                </Show>
              </div>
              <Show when={isLayerPanelVisible()}>
                <LayerPanel />
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
