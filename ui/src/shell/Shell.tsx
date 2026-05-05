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

// Import to trigger initial theme application as a side effect
import "../preferences/preferences-store";

const Shell: Component = () => {
  onMount(() => {
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
    </div>
  );
};

export default Shell;
