// Project lifecycle commands: new, open, save, close, sprite CRUD.

import { invoke } from "@tauri-apps/api/core";
import type { ColorMode, ProjectMetadata, Sprite, SpriteId } from "../types";

// ── response types ────────────────────────────────────────────────────────────

export type ProjectStatus = {
  metadata: ProjectMetadata;
  path: string | null;
  dirty: boolean;
  sprite_count: number;
};

// ── argument types ────────────────────────────────────────────────────────────

export type SpriteAddArgs = {
  name: string;
  canvas_width: number;
  canvas_height: number;
  color_mode: ColorMode;
};

// ── commands ──────────────────────────────────────────────────────────────────

/** Creates a new empty project, replacing any currently open document. */
export function projectNew(name: string): Promise<ProjectStatus> {
  return invoke<ProjectStatus>("project_new", { name });
}

/**
 * Opens a project from disk.
 * Requires B3 (.pixhaus format) — returns an error until B3 lands.
 */
export function projectOpen(path: string): Promise<ProjectStatus> {
  return invoke<ProjectStatus>("project_open", { path });
}

/**
 * Imports a PSD file and makes it the active project.
 * The imported project has no filesystem path (not saved as .pixhaus yet),
 * so dirty is true on return. Non-fatal conversion warnings are logged on
 * the Rust side; callers do not receive them through this command.
 */
export function projectImportPsd(path: string): Promise<ProjectStatus> {
  return invoke<ProjectStatus>("project_import_psd", { path });
}

/**
 * Saves the active project to disk.
 * Requires B3 (.pixhaus format) — returns an error until B3 lands.
 */
export function projectSave(path?: string): Promise<void> {
  return invoke<void>("project_save", { path: path ?? null });
}

/** Closes the active project, discarding all in-memory state. */
export function projectClose(): Promise<void> {
  return invoke<void>("project_close");
}

/** Returns the active project's status, or `null` if no project is open. */
export function projectGet(): Promise<ProjectStatus | null> {
  return invoke<ProjectStatus | null>("project_get");
}

/** Adds a new empty sprite to the active project. */
export function spriteAdd(args: SpriteAddArgs): Promise<Sprite> {
  return invoke<Sprite>("sprite_add", { args });
}

/** Removes a sprite from the active project by ID. */
export function spriteDelete(sprite_id: SpriteId): Promise<void> {
  return invoke<void>("sprite_delete", { sprite_id });
}

/** Returns all sprites in the active project. */
export function spriteList(): Promise<Sprite[]> {
  return invoke<Sprite[]>("sprite_list");
}
