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

const Shell: Component = () => {
  onMount(() => {
    // Initialise JS-layer crash reporting using the stored preference.
    initCrashReporting({ enabled: crashReportingEnabled(), uid: crashReportingUid });
    // Sync the Rust-side ENABLED gate so the panic hook honours the same
    // persisted preference. Without this, a user who opted in on a
    // previous session restarts to a state where the JS Sentry client
    // is initialised but Rust drops every panic in `before_send` until
    // they interact with the dialog again. setSentryEnabled returns void
    // and handles its own IPC errors internally — fire-and-forget.
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

  function handleCrashReportingAccept(): void {
    setCrashReportingEnabled(true);
    setSentryEnabled(true);
    markCrashReportingDialogShown();
  }

  function handleCrashReportingDecline(): void {
    setCrashReportingEnabled(false);
    setSentryEnabled(false);
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
              <div class="editor-layout__canvas">
                <Canvas />
                <Show when={activeSpriteId() !== null}>
                  <TilemapPanel />
                </Show>
              </div>
              <Show when={isLayerPanelVisible()}>
                <LayerPanel />
              </Show>
            </div>
          </Show>
        </div>

        <PalettePanel spriteId={activeSpriteId()} />
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
          onAccept={handleCrashReportingAccept}
          onDecline={handleCrashReportingDecline}
        />
      </Show>
    </div>
  );
};

export default Shell;
