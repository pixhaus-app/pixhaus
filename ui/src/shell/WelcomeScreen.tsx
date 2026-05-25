import { For, createSignal, onMount, type Component } from "solid-js";
import { open as dialogOpen } from "../lib/dialog";
import { projectState, setActiveProject, pushRecentProject } from "../project-state";
import {
  createNewProject,
  listSamples,
  openProjectByExtension,
  type SampleEntry,
} from "../lib/commands/project";
import { extractFilename } from "../lib/utils/path";
import { reportCommandFailure } from "../lib/utils/errors";
import { openCanvasSizeDialog } from "./canvas-size-dialog-state";

const OPEN_FILTERS = [
  {
    name: "All supported files",
    extensions: ["pixhaus", "psd", "aseprite", "ase"],
  },
  { name: "Pixhaus Projects", extensions: ["pixhaus"] },
  { name: "Aseprite", extensions: ["aseprite", "ase"] },
  { name: "Photoshop Documents", extensions: ["psd"] },
];

async function pickOpenPath(): Promise<string | null> {
  return dialogOpen({ multiple: false as const, filters: OPEN_FILTERS });
}

const WelcomeScreen: Component = () => {
  const [samples, setSamples] = createSignal<readonly SampleEntry[]>([]);

  onMount(() => {
    listSamples()
      .then((entries) => setSamples(entries))
      .catch((err: unknown) => {
        // List failures shouldn't break the welcome screen — log and
        // leave the section empty.
        console.error("[pixhaus] list_samples failed:", err);
      });
  });

  function handleNewProject(): void {
    openCanvasSizeDialog({
      mode: "project",
      onConfirm: ({ name, width, height }) => {
        createNewProject(name ?? "Untitled", { width, height })
          .then((status) => setActiveProject(status))
          .catch((err: unknown) => reportCommandFailure("project_new", err));
      },
    });
  }

  function handleOpenProject(): void {
    pickOpenPath()
      .then((path) => {
        if (path === null) return;
        return openProjectByExtension(path)
          .then(({ status, operation: _operation }) => {
            setActiveProject(status);
            pushRecentProject({ name: extractFilename(path), path });
          })
          .catch((err: unknown) => {
            // Re-derive the operation name so the toast names the
            // command the user actually triggered (psd / aseprite /
            // pixhaus), not a generic open.
            const op = inferOpenOperation(path);
            reportCommandFailure(op, err);
          });
      })
      .catch((err: unknown) => reportCommandFailure("file_dialog", err));
  }

  function handleOpenRecent(path: string): void {
    openProjectByExtension(path)
      .then(({ status }) => {
        setActiveProject(status);
        pushRecentProject({ name: extractFilename(path), path });
      })
      .catch((err: unknown) => {
        const op = inferOpenOperation(path);
        reportCommandFailure(op, err);
      });
  }

  function handleOpenSample(sample: SampleEntry): void {
    openProjectByExtension(sample.path)
      .then(({ status }) => {
        setActiveProject(status);
        pushRecentProject({ name: sample.name, path: sample.path });
      })
      .catch((err: unknown) => reportCommandFailure("project_open", err));
  }

  return (
    <div class="welcome" data-testid="welcome">
      <h1 class="welcome__title">Pixhaus</h1>
      <p class="welcome__subtitle">
        AI-native pixel art editor for sprites, animations, and tilemaps
      </p>

      <div class="welcome__actions">
        <button
          class="welcome__btn welcome__btn--primary"
          data-testid="welcome-new-project"
          onClick={handleNewProject}
        >
          New Project
        </button>
        <button class="welcome__btn" data-testid="welcome-open-project" onClick={handleOpenProject}>
          Open Project...
        </button>
      </div>

      {samples().length > 0 && (
        <div class="welcome__samples" data-testid="welcome-samples">
          <p class="welcome__samples-title">Samples</p>
          <div class="welcome__samples-list">
            <For each={samples()}>
              {(sample) => (
                <button
                  class="welcome__samples-item"
                  data-testid={`welcome-sample-${sample.name}`}
                  onClick={() => handleOpenSample(sample)}
                >
                  <span class="welcome__samples-item__name">{sample.name}</span>
                </button>
              )}
            </For>
          </div>
        </div>
      )}

      {projectState.recentProjects.length > 0 && (
        <div class="welcome__recent" data-testid="welcome-recent">
          <p class="welcome__recent-title">Recent</p>
          <div class="welcome__recent-list">
            <For each={projectState.recentProjects}>
              {(project, i) => (
                <button
                  class="welcome__recent-item"
                  data-testid={`welcome-recent-${i()}`}
                  onClick={() => handleOpenRecent(project.path)}
                >
                  <span class="welcome__recent-item__name">{project.name}</span>
                  <span class="welcome__recent-item__path">{project.path}</span>
                </button>
              )}
            </For>
          </div>
        </div>
      )}
    </div>
  );
};

/** Names the IPC operation a path will dispatch to, for error reporting. */
function inferOpenOperation(path: string): string {
  const lower = path.toLowerCase();
  if (lower.endsWith(".psd")) return "project_import_psd";
  if (lower.endsWith(".aseprite") || lower.endsWith(".ase")) {
    return "project_import_aseprite";
  }
  return "project_open";
}

export default WelcomeScreen;
