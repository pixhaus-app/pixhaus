import { createSignal } from "solid-js";
import type { ProjectStatus } from "./lib/commands/project";

export type RecentProject = { name: string; path: string };

const MAX_RECENT = 10;
const RECENT_KEY = "pixhaus:recent-projects";

function loadRecent(): RecentProject[] {
  try {
    const raw = localStorage.getItem(RECENT_KEY);
    if (raw !== null) {
      return JSON.parse(raw) as RecentProject[];
    }
  } catch {
    // malformed — start fresh
  }
  return [];
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
