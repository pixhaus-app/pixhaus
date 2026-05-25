import { createStore } from "solid-js/store";
import type { ProjectStatus } from "./lib/commands/project";
import { loadStorageJSON } from "./lib/utils/storage";

export type RecentProject = { name: string; path: string };

const MAX_RECENT = 10;
const RECENT_KEY = "pixhaus:recent-projects";

function isRecentProjectArray(v: unknown): v is RecentProject[] {
  return (
    Array.isArray(v) &&
    v.every(
      (e) =>
        e !== null &&
        typeof e === "object" &&
        typeof (e as RecentProject).name === "string" &&
        typeof (e as RecentProject).path === "string",
    )
  );
}

function loadRecent(): RecentProject[] {
  return loadStorageJSON<RecentProject[]>(RECENT_KEY, [], isRecentProjectArray);
}

// One store for project session state. Reads are projectState.activeProject
// and projectState.recentProjects; writes go through the functions below.
interface ProjectState {
  activeProject: ProjectStatus | null;
  recentProjects: RecentProject[];
}

export const [projectState, setProjectState] = createStore<ProjectState>({
  activeProject: null,
  recentProjects: loadRecent(),
});

export function setActiveProject(status: ProjectStatus | null): void {
  setProjectState("activeProject", status);
}

export function pushRecentProject(entry: RecentProject): void {
  const prev = projectState.recentProjects;
  const filtered = prev.filter((p) => p.path !== entry.path);
  const next = [entry, ...filtered].slice(0, MAX_RECENT);
  localStorage.setItem(RECENT_KEY, JSON.stringify(next));
  setProjectState("recentProjects", next);
}
