import { Show, onMount, onCleanup, type Component } from "solid-js";
import { subscribeTauriEvent } from "../lib/sync/subscribe-event";
import { commandPalette } from "../palette-state";
import { preferencesModal } from "../preferences/preferences-state";
import { compositionLibrary } from "../composition-library/composition-library-state";
import { projectState } from "../project-state";
import { dispatchCommand } from "../command-palette/command-registry";
import { setupKeybindManager } from "../keybinds/keybind-manager";
import CommandPalette from "../command-palette/CommandPalette";
import PreferencesModal from "../preferences/PreferencesModal";
import CompositionLibraryModal from "../composition-library/CompositionLibraryModal";
import ToastHost from "../lib/toast/ToastHost";
import UpdateAvailableModal from "./UpdateAvailableModal";
import CanvasSizeDialog from "./CanvasSizeDialog";
import VerbInvokeHost from "../lib/ai/VerbInvokeHost";
import StatusBar from "./StatusBar";
import WelcomeScreen from "./WelcomeScreen";
import Canvas from "../canvas/Canvas";
import FirstLaunchDialog from "../crash-reporting/FirstLaunchDialog";
import {
  prefs,
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
import LibraryPanel from "../library/LibraryPanel";
import EntityCreateModal from "../library/EntityCreateModal";
import { setupPaletteColorSync } from "../canvas/tools/palette-color-sync";
import { installTilemapCtxSync } from "../tilemap/tilemap-state";
import ReferenceSheetEditor from "../sheet/ReferenceSheetEditor";
import { sheetState } from "../sheet/sheet-state";
import AnimationStudio from "../animation/AnimationStudio";
import { studio } from "../animation/animation-studio-state";
import RightRail from "./RightRail";
import {
  isLibraryCollapsed,
  isTimelineCollapsed,
  toggleLibraryCollapsed,
  toggleTimelineCollapsed,
} from "./rail-state";

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
        initCrashReporting({ enabled: prefs.crashReportingEnabled, uid: crashReportingUid });
        setSentryEnabled(prefs.crashReportingEnabled);
      });

    // Forward native menu events to the command dispatcher
    subscribeTauriEvent<string>("shell:menu", (payload) => dispatchCommand(payload));

    // Register keyboard shortcuts
    const removeKeybinds = setupKeybindManager();

    onCleanup(removeKeybinds);
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
          <Show when={projectState.activeProject === null}>
            <WelcomeScreen />
          </Show>
          <Show when={projectState.activeProject !== null}>
            <div class="editor-layout" data-testid="editor-layout">
              <Show
                when={!isLibraryCollapsed()}
                fallback={
                  <button
                    class="library-dock-collapsed"
                    onClick={toggleLibraryCollapsed}
                    title="Show library"
                    aria-label="Show library panel"
                    data-testid="library-expand"
                  >
                    ›
                  </button>
                }
              >
                <LibraryPanel />
              </Show>
              <div class="editor-layout__canvas-area">
                <Show when={!studio.isAnimationStudioOpen} fallback={<AnimationStudio />}>
                  <Show when={!sheetState.isSheetEditorOpen} fallback={<ReferenceSheetEditor />}>
                    <div class="editor-layout__canvas">
                      <Canvas />
                    </div>
                    <Show
                      when={!isTimelineCollapsed()}
                      fallback={
                        <button
                          class="timeline-dock-collapsed"
                          onClick={toggleTimelineCollapsed}
                          title="Show timeline"
                          aria-label="Show timeline panel"
                          data-testid="timeline-expand"
                        >
                          ▴
                        </button>
                      }
                    >
                      <TimelinePanel />
                    </Show>
                  </Show>
                </Show>
              </div>
              <RightRail />
            </div>
          </Show>
        </div>
      </div>

      <StatusBar />

      <Show when={commandPalette.open}>
        <CommandPalette />
      </Show>

      <Show when={preferencesModal.open}>
        <PreferencesModal />
      </Show>

      <Show when={compositionLibrary.open}>
        <CompositionLibraryModal />
      </Show>

      <UpdateAvailableModal />

      <CanvasSizeDialog />

      <EntityCreateModal />

      <VerbInvokeHost />
      <ToastHost />

      <Show when={!prefs.crashReportingDialogShown}>
        <FirstLaunchDialog
          onAccept={() => answerCrashReportingDialog(true)}
          onDecline={() => answerCrashReportingDialog(false)}
        />
      </Show>
    </div>
  );
};

export default Shell;
