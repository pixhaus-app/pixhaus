import { For, type Component } from "solid-js";
import { open as dialogOpen } from "@tauri-apps/plugin-dialog";
import { recentProjects, setActiveProject, pushRecentProject } from "../project-state";
import {
  projectNew,
  projectOpen,
  projectImportPsd,
  type ProjectStatus,
} from "../lib/commands/project";
import { extractFilename } from "../lib/utils/path";
import { reportCommandFailure } from "../lib/utils/errors";

const OPEN_FILTERS = [
  { name: "All supported files", extensions: ["pixhaus", "psd"] },
  { name: "Pixhaus Projects", extensions: ["pixhaus"] },
  { name: "Photoshop Documents", extensions: ["psd"] },
];

async function pickOpenPath(): Promise<string | null> {
  return dialogOpen({ multiple: false as const, filters: OPEN_FILTERS });
}

function openByExtension(path: string): Promise<ProjectStatus> {
  return path.toLowerCase().endsWith(".psd") ? projectImportPsd(path) : projectOpen(path);
}

const WelcomeScreen: Component = () => {
  function handleNewProject(): void {
    projectNew("Untitled")
      .then((status) => setActiveProject(status))
      .catch((err: unknown) => reportCommandFailure("project_new", err));
  }

  function handleOpenProject(): void {
    pickOpenPath()
      .then((path) => {
        if (path === null) return;
        return openByExtension(path).then((status) => {
          setActiveProject(status);
          pushRecentProject({ name: extractFilename(path), path });
        });
      })
      .catch((err: unknown) => reportCommandFailure("project_open", err));
  }

  function handleOpenRecent(path: string): void {
    openByExtension(path)
      .then((status) => {
        setActiveProject(status);
        pushRecentProject({ name: extractFilename(path), path });
      })
      .catch((err: unknown) => reportCommandFailure("project_open", err));
  }

  return (
    <div class="welcome">
      <h1 class="welcome__title">Pixhaus</h1>
      <p class="welcome__subtitle">
        AI-native pixel art editor for sprites, animations, and tilemaps
      </p>

      <div class="welcome__actions">
        <button class="welcome__btn welcome__btn--primary" onClick={handleNewProject}>
          New Project
        </button>
        <button class="welcome__btn" onClick={handleOpenProject}>
          Open Project...
        </button>
      </div>

      {recentProjects().length > 0 && (
        <div class="welcome__recent">
          <p class="welcome__recent-title">Recent</p>
          <div class="welcome__recent-list">
            <For each={recentProjects()}>
              {(project) => (
                <button class="welcome__recent-item" onClick={() => handleOpenRecent(project.path)}>
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

export default WelcomeScreen;
