//! Plugin management IPC commands.
//!
//! These commands let the UI inspect, load, and unload plugins. Hot-reload is
//! handled transparently by the watcher background task; `plugin_reload` is
//! provided for cases where the user wants to force a reload without waiting
//! for a file event.

use tauri::State;

use pixhaus_plugins::PluginInfo;

use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

/// Lists all currently loaded plugins and their metadata.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn plugin_list(state: State<'_, AppState>) -> CommandResult<Vec<PluginInfo>> {
    Ok(state.plugins.list())
}

/// Triggers a re-scan of the plugin directory and loads any new plugins.
///
/// This is the manual equivalent of restarting the editor. Plugins already
/// loaded are NOT reloaded; use `plugin_reload` for that.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn plugin_scan(state: State<'_, AppState>) -> CommandResult<usize> {
    state
        .plugins
        .scan()
        .await
        .map_err(|e| AppCommandError::Validation {
            detail: e.to_string(),
        })
}

/// Unloads the named plugin and reloads it from disk.
///
/// Returns an error if the plugin is not currently loaded or if the reload
/// fails (e.g. manifest parse error, WASM load failure). On failure the plugin
/// stays unloaded — the caller should surface the error to the user.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn plugin_reload(name: String, state: State<'_, AppState>) -> CommandResult<()> {
    state
        .plugins
        .reload(&name)
        .await
        .map_err(|e| AppCommandError::Validation {
            detail: e.to_string(),
        })
}

/// Unloads the named plugin. Its registered verbs, tools, and commands are
/// removed immediately.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn plugin_unload(name: String, state: State<'_, AppState>) -> CommandResult<()> {
    state
        .plugins
        .unload(&name)
        .map_err(|e| AppCommandError::Validation {
            detail: e.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Smoke-test that PluginInfo is serialisable. The IPC bridge requires all
    // return types to implement serde::Serialize.
    #[test]
    fn plugin_info_serialises() {
        let info = PluginInfo {
            name: "echo".into(),
            version: "0.1.0".into(),
            description: Some("Echo verb for testing".into()),
            author: None,
            dir: "/home/user/.pixhaus/plugins/echo".into(),
            verb_count: 1,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("echo"));
    }
}
