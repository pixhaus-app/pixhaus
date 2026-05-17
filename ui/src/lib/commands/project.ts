// Project lifecycle commands: new, open, save, close, sprite CRUD.

import { invoke } from "../ipc";
import { save as dialogSave } from "../dialog";
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
  reference_bytes?: number[];
  reference_mime?: string;
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
 * Imports an `.aseprite` / `.ase` file and makes it the active project.
 * Mirrors `projectImportPsd`: the imported project has no filesystem
 * path (the user must Save As to write a `.pixhaus` file). Non-fatal
 * conversion warnings are logged Rust-side.
 */
export function projectImportAseprite(path: string): Promise<ProjectStatus> {
  return invoke<ProjectStatus>("project_import_aseprite", { path });
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

// ── Bundled samples ──────────────────────────────────────────────────────────

/** Metadata for one sample project surfaced on the welcome screen. */
export type SampleEntry = {
  /** File stem, e.g. "character-knight". */
  name: string;
  /** Absolute path on disk; pass directly to `projectOpen`. */
  path: string;
};

/**
 * Lists sample `.pixhaus` projects shipped with the editor.
 *
 * Returns an empty array when the bundled samples directory cannot be
 * found (a fresh checkout without `examples/samples/` should still
 * launch). Errors from the filesystem walk are swallowed Rust-side.
 */
export function listSamples(): Promise<SampleEntry[]> {
  return invoke<SampleEntry[]>("list_samples");
}

// ── High-level helpers ───────────────────────────────────────────────────────

/** File extensions recognised by `openProjectByExtension`. */
const PSD_EXT = ".psd";
const ASEPRITE_EXTS = [".aseprite", ".ase"] as const;

/**
 * Routes a path to the correct open / import IPC based on its
 * extension. `.pixhaus` opens normally; `.psd` and `.aseprite`/`.ase`
 * import (the imported project has no filesystem path until Save As).
 *
 * Returns the IPC operation name alongside the status so callers can
 * report a failure under the command they actually triggered, not a
 * generic "open" label.
 */
export async function openProjectByExtension(
  path: string,
): Promise<{ status: ProjectStatus; operation: string }> {
  const lower = path.toLowerCase();
  if (lower.endsWith(PSD_EXT)) {
    return { status: await projectImportPsd(path), operation: "project_import_psd" };
  }
  if (ASEPRITE_EXTS.some((ext) => lower.endsWith(ext))) {
    return {
      status: await projectImportAseprite(path),
      operation: "project_import_aseprite",
    };
  }
  return { status: await projectOpen(path), operation: "project_open" };
}

/**
 * Creates a new project and seeds it with a default 32x32 RGBA sprite
 * AND one raster layer so the canvas is non-empty on first launch and
 * the user can draw immediately. Without the layer seed, the input
 * handler returns early because activeLayerId stays null — pencil
 * clicks silently no-op and the manual test guide's "New Project
 * creates a 32×32 sprite with one layer" expectation fails.
 */
export async function createNewProject(
  name: string = "Untitled",
  size: { width: number; height: number } = { width: 32, height: 32 },
): Promise<ProjectStatus> {
  const status = await projectNew(name);
  const sprite = await spriteAdd({
    name: "Sprite",
    canvas_width: size.width,
    canvas_height: size.height,
    color_mode: "rgba",
  });
  // Seed the first raster layer. layer_add is part of the public API
  // and is what the layer panel's "+" button calls; doing it here means
  // the user lands on a usable canvas.
  const { layerAdd } = await import("./layers");
  await layerAdd({
    sprite_id: sprite.id,
    name: "Layer 1",
    kind: { kind: "raster" },
  });
  // Re-fetch status so sprite_count reflects the seeded sprite.
  const updated = await projectGet();
  return updated ?? status;
}

const PIXHAUS_DIALOG_FILTER = [{ name: "Pixhaus Projects", extensions: ["pixhaus"] }];

/**
 * Saves the active project, opening the Save As dialog when the
 * project has no on-disk path yet. The Rust `project_save` command
 * rejects with a `Validation` error on the first save of a new
 * project; this helper catches that one specific failure and falls
 * through to a `dialogSave` call, then retries `projectSave` with the
 * chosen path.
 *
 * Returns `true` if the save completed (including via the Save As
 * branch), `false` if the user dismissed the dialog. Throws for any
 * other failure so callers' `reportCommandFailure` paths still fire.
 */
export async function saveOrSaveAs(): Promise<boolean> {
  try {
    await projectSave();
    return true;
  } catch (err) {
    if (!isNoPathYetError(err)) {
      throw err;
    }
  }
  // First-save fallback: open the Save As dialog and retry with a path.
  const result = await dialogSave({ filters: PIXHAUS_DIALOG_FILTER });
  const target = typeof result === "string" ? result : null;
  if (target === null) return false;
  await projectSave(target);
  return true;
}

/**
 * Detects the "save requires a path on first call" error returned by
 * the Rust `project_save` command for a fresh project. Matches on the
 * error `kind` first (resilient to message localisation) and falls
 * back to a substring match on the message for older builds.
 */
function isNoPathYetError(err: unknown): boolean {
  if (err === null || typeof err !== "object") return false;
  const e = err as { kind?: unknown; message?: unknown };
  if (e.kind !== "validation") return false;
  if (typeof e.message !== "string") return false;
  return e.message.toLowerCase().includes("save requires a path");
}
