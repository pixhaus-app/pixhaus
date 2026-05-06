//! File-system watcher for hot-reload support.
//!
//! Wraps [`notify`] and forwards modification events to the registry so
//! plugins are reloaded in-place when their entry point or manifest changes.
//! The watcher runs a background tokio task; dropping the returned
//! [`PluginWatcher`] stops the task and frees the OS-level watcher.

use std::path::Path;
use std::sync::Arc;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher, recommended_watcher};
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::error::{PluginError, Result};
use crate::registry::PluginRegistry;

/// Handle to a running hot-reload watcher.
///
/// Dropping this value cancels the background task and releases the
/// OS-level file watch. The handle is intentionally not `Clone` so
/// ownership stays with a single component (typically `AppState`).
pub struct PluginWatcher {
    /// Shutdown signal for the background task.
    _shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl std::fmt::Debug for PluginWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginWatcher").finish_non_exhaustive()
    }
}

/// Starts watching `registry.base_dir()` for file changes.
///
/// Spawns a tokio background task that listens for `notify` events and
/// calls [`PluginRegistry::reload`] on the affected plugin directory.
///
/// # Errors
/// Returns [`PluginError::Watch`] when the OS watcher cannot be
/// initialised (unsupported platform, permission denied, etc.).
pub fn start(registry: Arc<PluginRegistry>) -> Result<PluginWatcher> {
    let (event_tx, mut event_rx) = mpsc::channel::<notify::Result<Event>>(64);
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    // The notify callback runs on a dedicated thread managed by the OS
    // watcher. We forward events into a tokio mpsc channel so the async
    // side can process them without blocking.
    let mut watcher: RecommendedWatcher = recommended_watcher(move |res: notify::Result<Event>| {
        // `blocking_send` is safe here because the callback is never
        // called from an async context. If the channel is full we
        // drop the event — the registry will self-heal on the next
        // reload request.
        let _ = event_tx.blocking_send(res);
    })
    .map_err(|e| PluginError::Watch(e.to_string()))?;

    let base_dir = registry.base_dir().to_owned();
    watcher
        .watch(&base_dir, RecursiveMode::Recursive)
        .map_err(|e| PluginError::Watch(e.to_string()))?;

    debug!(dir = %base_dir.display(), "plugin hot-reload watcher started");

    tokio::spawn(async move {
        // Keep `watcher` alive for the duration of the task — dropping it
        // would stop the OS watch immediately.
        let _watcher = watcher;

        loop {
            tokio::select! {
                biased;
                _ = &mut shutdown_rx => {
                    debug!("plugin watcher shutdown received");
                    break;
                }
                Some(res) = event_rx.recv() => {
                    handle_event(res, &registry, &base_dir).await;
                }
            }
        }
    });

    Ok(PluginWatcher {
        _shutdown_tx: shutdown_tx,
    })
}

/// Routes a single watcher event to the appropriate registry action.
async fn handle_event(res: notify::Result<Event>, registry: &PluginRegistry, base_dir: &Path) {
    let event = match res {
        Ok(e) => e,
        Err(e) => {
            warn!(error = %e, "plugin watcher error");
            return;
        }
    };

    // Only care about data modifications and file creations, not attribute
    // changes or access events.
    let interesting = matches!(
        event.kind,
        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
    );
    if !interesting {
        return;
    }

    for path in event.paths {
        if let Some(plugin_name) = plugin_name_for_path(&path, base_dir) {
            debug!(plugin = %plugin_name, file = %path.display(), "hot-reload triggered");
            if let Err(e) = registry.reload(&plugin_name).await {
                warn!(plugin = %plugin_name, error = %e, "hot-reload failed");
            }
        }
    }
}

/// Extracts the plugin name from a path inside the plugin directory.
///
/// Strips `base_dir` from `path` and returns the first remaining component,
/// which is the plugin directory name. For example, given base
/// `~/.pixhaus/plugins` and path `~/.pixhaus/plugins/echo/echo.wasm`, this
/// returns `"echo"`.
fn plugin_name_for_path(path: &Path, base_dir: &Path) -> Option<String> {
    let rel = path.strip_prefix(base_dir).ok()?;
    rel.components()
        .next()
        .and_then(|c| c.as_os_str().to_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn extracts_plugin_name_from_nested_path() {
        let base = PathBuf::from("/home/user/.local/share/pixhaus/plugins");
        let path = base.join("echo").join("echo.wasm");
        assert_eq!(plugin_name_for_path(&path, &base), Some("echo".to_owned()));
    }

    #[test]
    fn extracts_plugin_name_from_manifest() {
        let base = PathBuf::from("/home/user/.local/share/pixhaus/plugins");
        let path = base.join("my-plugin").join("plugin.toml");
        assert_eq!(
            plugin_name_for_path(&path, &base),
            Some("my-plugin".to_owned())
        );
    }

    #[test]
    fn returns_none_for_path_outside_base() {
        let base = PathBuf::from("/home/user/.local/share/pixhaus/plugins");
        let path = PathBuf::from("/tmp/other/file.wasm");
        assert_eq!(plugin_name_for_path(&path, &base), None);
    }
}
