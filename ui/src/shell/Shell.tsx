import { Show, onMount, onCleanup, type Component } from "solid-js";
import { listen } from "@tauri-apps/api/event";
import { isCommandPaletteOpen } from "../palette-state";
import { isPreferencesOpen } from "../preferences/preferences-state";
import { activeProject } from "../project-state";
import { dispatchCommand } from "../command-palette/command-registry";
import { setupKeybindManager } from "../keybinds/keybind-manager";
import CommandPalette from "../command-palette/CommandPalette";
import PreferencesModal from "../preferences/PreferencesModal";
import StatusBar from "./StatusBar";
import WelcomeScreen from "./WelcomeScreen";
import Canvas from "../canvas/Canvas";

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
      <div class="shell-main">
        <Show when={activeProject() === null}>
          <WelcomeScreen />
        </Show>
        <Show when={activeProject() !== null}>
          <Canvas />
        </Show>
      </div>

      <StatusBar />

      <Show when={isCommandPaletteOpen()}>
        <CommandPalette />
      </Show>

      <Show when={isPreferencesOpen()}>
        <PreferencesModal />
      </Show>
    </div>
  );
};

export default Shell;
