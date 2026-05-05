import { createSignal } from "solid-js";
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

const [activeProject, setActiveProjectInternal] = createSignal<ProjectStatus | null>(null);
const [recentProjects, setRecentProjectsInternal] = createSignal<RecentProject[]>(loadRecent());

export { activeProject, recentProjects };

export function setActiveProject(status: ProjectStatus | null): void {
  setActiveProjectInternal(status);
}

export function pushRecentProject(entry: RecentProject): void {
  setRecentProjectsInternal((prev) => {
    const filtered = prev.filter((p) => p.path !== entry.path);
    const next = [entry, ...filtered].slice(0, MAX_RECENT);
    localStorage.setItem(RECENT_KEY, JSON.stringify(next));
    return next;
  });
}
