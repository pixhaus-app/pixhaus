// Plugin management commands.
//
// Mirrors the Rust `PluginInfo` shape from `plugins/src/registry.rs`.
// `description` and `author` are `Option<String>` with `skip_serializing_if`,
// so they may be absent on the wire.

import { invoke } from "../ipc";

// ── response types ────────────────────────────────────────────────────────────

export interface PluginInfo {
  /** Plugin name from the manifest. */
  name: string;
  /** Plugin version from the manifest. */
  version: string;
  /** Optional description from the manifest. */
  description?: string;
  /** Optional author from the manifest. */
  author?: string;
  /** Absolute path to the plugin directory. */
  dir: string;
  /** Number of verbs this plugin registered. */
  verb_count: number;
}

// ── commands ──────────────────────────────────────────────────────────────────

/** Returns all currently loaded plugins. */
export function pluginList(): Promise<PluginInfo[]> {
  return invoke<PluginInfo[]>("plugin_list");
}

/**
 * Re-scans the plugin directory and loads any newly added plugins.
 * Returns the count of plugins now loaded. Does NOT reload plugins
 * that were already loaded — use {@link pluginReload} for that.
 */
export function pluginScan(): Promise<number> {
  return invoke<number>("plugin_scan");
}

/** Unloads and reloads the named plugin from disk. */
export function pluginReload(name: string): Promise<void> {
  return invoke<void>("plugin_reload", { name });
}

/** Unloads the named plugin. Its verbs are deregistered immediately. */
export function pluginUnload(name: string): Promise<void> {
  return invoke<void>("plugin_unload", { name });
}
