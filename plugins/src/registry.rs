//! Plugin registry: owns loaded plugins, hot-reload watcher, and the verb
//! runtime handle used when registering plugin-provided verbs.
//!
//! # Layout on disk
//!
//! ```text
//! ~/.pixhaus/plugins/
//!   echo/
//!     plugin.toml       ← manifest
//!     echo.wasm         ← entry point (runtime = WASM)
//!   my-lua-plugin/
//!     plugin.toml
//!     main.lua          ← entry point (runtime = Lua, wired by S38)
//! ```
//!
//! Each subdirectory is one plugin. The directory name is ignored; the plugin
//! name comes from the manifest's `plugin.name` field. Directories that lack a
//! `plugin.toml` are skipped with a warning.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument, warn};

use pixhaus_ai::plugin::runtime::VerbRuntime;

use crate::error::{PluginError, Result};
use crate::manifest::{self, Manifest, PluginRuntime};
use pixhaus_ai::plugin::verb::Verb as _;

use crate::wasm::WasmPlugin;

/// A plugin that has been successfully loaded into the registry.
#[derive(Debug)]
pub struct LoadedPlugin {
    /// Parsed manifest.
    pub manifest: Manifest,
    /// Absolute path to the plugin directory.
    pub dir: PathBuf,
    /// IDs of verbs registered by this plugin, used on unload.
    pub registered_verb_ids: Vec<pixhaus_ai::plugin::descriptor::VerbId>,
}

/// Snapshot of a loaded plugin suitable for serialisation over IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Plugin name from the manifest.
    pub name: String,
    /// Plugin version from the manifest.
    pub version: String,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional author string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Absolute path to the plugin directory.
    pub dir: String,
    /// Number of verbs this plugin registered.
    pub verb_count: usize,
}

impl From<&LoadedPlugin> for PluginInfo {
    fn from(p: &LoadedPlugin) -> Self {
        Self {
            name: p.manifest.plugin.name.clone(),
            version: p.manifest.plugin.version.clone(),
            description: p.manifest.plugin.description.clone(),
            author: p.manifest.plugin.author.clone(),
            dir: p.dir.to_string_lossy().into_owned(),
            verb_count: p.registered_verb_ids.len(),
        }
    }
}

/// Registry of loaded plugins.
///
/// Owns the verb runtime handle used when registering plugin-provided verbs.
/// Thread-safe: the inner map is guarded by a `parking_lot::RwLock` so
/// read-only operations (list, lookup) do not block each other.
pub struct PluginRegistry {
    verbs: Arc<VerbRuntime>,
    base_dir: PathBuf,
    plugins: RwLock<HashMap<String, LoadedPlugin>>,
}

impl PluginRegistry {
    /// Constructs an empty registry.
    ///
    /// `verbs` is the shared verb runtime; verbs registered by plugins are
    /// inserted here and removed on unload.
    ///
    /// The base directory defaults to `~/.pixhaus/plugins`. If the user's home
    /// directory cannot be resolved, the registry is inert (no plugins loaded,
    /// no errors).
    #[must_use]
    pub fn new(verbs: Arc<VerbRuntime>) -> Self {
        let base_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("pixhaus")
            .join("plugins");

        Self {
            verbs,
            base_dir,
            plugins: RwLock::new(HashMap::new()),
        }
    }

    /// Constructs a registry with an explicit base directory. Used in tests.
    #[must_use]
    pub fn with_base_dir(verbs: Arc<VerbRuntime>, base_dir: PathBuf) -> Self {
        Self {
            verbs,
            base_dir,
            plugins: RwLock::new(HashMap::new()),
        }
    }

    /// The directory the registry scans for plugin subdirectories.
    #[must_use]
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Scans `base_dir` for plugin subdirectories and loads each one that has a
    /// valid `plugin.toml`. Subdirectories that fail to load are logged as
    /// warnings and skipped; the scan continues.
    ///
    /// Returns the number of plugins successfully loaded.
    ///
    /// # Errors
    /// Only returns an error when the base directory exists but cannot be read
    /// (permission denied, I/O error). A missing base directory is silently
    /// treated as "zero plugins" — the directory is created on first install.
    pub async fn scan(&self) -> Result<usize> {
        if !self.base_dir.exists() {
            debug!(
                dir = %self.base_dir.display(),
                "plugin directory does not exist — skipping scan"
            );
            return Ok(0);
        }

        let entries = std::fs::read_dir(&self.base_dir).map_err(|source| PluginError::Io {
            name: self.base_dir.to_string_lossy().into_owned(),
            source,
        })?;

        let mut loaded = 0usize;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            match self.load_from_dir(&path).await {
                Ok(()) => loaded += 1,
                Err(e) => warn!(dir = %path.display(), error = %e, "failed to load plugin"),
            }
        }

        Ok(loaded)
    }

    /// Loads a single plugin from a directory.
    ///
    /// On success, the plugin's verbs are registered in the verb runtime and
    /// the plugin is inserted into the registry under its manifest name.
    ///
    /// # Errors
    /// Returns errors for missing manifest, unparsable manifest, missing entry
    /// point, unsupported runtime, and WASM load failures.
    #[instrument(skip(self), fields(dir = %plugin_dir.display()))]
    pub async fn load_from_dir(&self, plugin_dir: &Path) -> Result<()> {
        let manifest_path = plugin_dir.join("plugin.toml");
        let toml_str =
            std::fs::read_to_string(&manifest_path).map_err(|source| PluginError::Io {
                name: plugin_dir.to_string_lossy().into_owned(),
                source,
            })?;

        let manifest = manifest::parse(&toml_str, &manifest_path.to_string_lossy())?;

        let entry_path = plugin_dir.join(&manifest.plugin.entry_point);
        if !entry_path.exists() {
            return Err(PluginError::EntryPointMissing {
                name: manifest.plugin.name.clone(),
                entry: manifest.plugin.entry_point.clone(),
            });
        }

        let runtime = manifest
            .plugin
            .runtime()
            .ok_or_else(|| PluginError::UnsupportedRuntime {
                entry: manifest.plugin.entry_point.clone(),
            })?;

        let plugin_name = manifest.plugin.name.clone();
        let registered_verb_ids = match runtime {
            PluginRuntime::Wasm => {
                if manifest.plugin.permissions.register_verbs {
                    self.load_wasm_verbs(&plugin_name, &entry_path, &manifest)?
                } else {
                    // A WASM plugin that doesn't register verbs has nothing to do yet.
                    // Future permission surfaces (tools, panels) expand this.
                    debug!(plugin = %plugin_name, "WASM plugin has no verb permission — skipping");
                    vec![]
                }
            }
            PluginRuntime::Lua => {
                // Lua binding wired by S38. Log and continue.
                warn!(
                    plugin = %plugin_name,
                    "Lua plugin support is not yet available (S38); plugin loaded without bindings"
                );
                vec![]
            }
        };

        let loaded = LoadedPlugin {
            dir: plugin_dir.to_owned(),
            registered_verb_ids,
            manifest,
        };
        self.plugins.write().insert(plugin_name, loaded);
        Ok(())
    }

    /// Unloads a plugin by name: unregisters its verbs and removes it from
    /// the registry.
    ///
    /// # Errors
    /// Returns [`PluginError::NotLoaded`] if no plugin with that name is
    /// currently loaded.
    pub fn unload(&self, name: &str) -> Result<()> {
        let loaded = self
            .plugins
            .write()
            .remove(name)
            .ok_or_else(|| PluginError::NotLoaded {
                name: name.to_owned(),
            })?;

        for verb_id in &loaded.registered_verb_ids {
            // Ignore "not found" errors — the verb may have been
            // double-unregistered or the runtime may have restarted.
            if let Err(e) = self.verbs.unregister(verb_id) {
                debug!(id = %verb_id, error = %e, "verb already gone on plugin unload");
            }
        }

        debug!(plugin = name, "plugin unloaded");
        Ok(())
    }

    /// Reloads a plugin by unloading the current instance and loading it fresh
    /// from the same directory. Used by the hot-reload watcher.
    ///
    /// # Errors
    /// Returns errors from either the unload or the reload step. On reload
    /// failure the plugin stays unloaded.
    pub async fn reload(&self, name: &str) -> Result<()> {
        let dir = {
            let plugins = self.plugins.read();
            plugins
                .get(name)
                .map(|p| p.dir.clone())
                .ok_or_else(|| PluginError::NotLoaded {
                    name: name.to_owned(),
                })?
        };
        self.unload(name)?;
        self.load_from_dir(&dir).await
    }

    /// Returns a snapshot of all currently loaded plugins.
    #[must_use]
    pub fn list(&self) -> Vec<PluginInfo> {
        let plugins = self.plugins.read();
        let mut infos: Vec<PluginInfo> = plugins.values().map(PluginInfo::from).collect();
        infos.sort_by(|a, b| a.name.cmp(&b.name));
        infos
    }

    /// Returns `true` if a plugin with this name is loaded.
    #[must_use]
    pub fn is_loaded(&self, name: &str) -> bool {
        self.plugins.read().contains_key(name)
    }

    /// Returns the number of loaded plugins.
    #[must_use]
    pub fn len(&self) -> usize {
        self.plugins.read().len()
    }

    /// Returns `true` when no plugins are loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.plugins.read().is_empty()
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    fn load_wasm_verbs(
        &self,
        plugin_name: &str,
        entry_path: &Path,
        manifest: &Manifest,
    ) -> Result<Vec<pixhaus_ai::plugin::descriptor::VerbId>> {
        let wasm = WasmPlugin::load(plugin_name, entry_path)?;
        let verbs = wasm.into_verbs();
        let mut ids = Vec::with_capacity(verbs.len());

        for verb in verbs {
            let id = verb.descriptor().id.clone();
            // Permission check: plugins must declare register_verbs = true.
            if !manifest.plugin.permissions.register_verbs {
                return Err(PluginError::PermissionDenied {
                    name: plugin_name.to_owned(),
                    capability: "register_verbs".into(),
                });
            }
            self.verbs
                .register(verb)
                .map_err(|source| PluginError::VerbRegistration {
                    plugin: plugin_name.to_owned(),
                    source,
                })?;
            ids.push(id);
        }

        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt() -> Arc<VerbRuntime> {
        Arc::new(VerbRuntime::new())
    }

    #[test]
    fn new_registry_is_empty() {
        let reg = PluginRegistry::new(rt());
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        assert!(reg.list().is_empty());
    }

    #[test]
    fn is_loaded_returns_false_for_missing_plugin() {
        let reg = PluginRegistry::new(rt());
        assert!(!reg.is_loaded("nonexistent"));
    }

    #[tokio::test]
    async fn scan_missing_dir_returns_zero() {
        let reg = PluginRegistry::with_base_dir(
            rt(),
            PathBuf::from("/this/path/does/not/exist/pixhaus/plugins"),
        );
        let count = reg.scan().await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn scan_empty_dir_returns_zero() {
        let dir = tempfile::tempdir().unwrap();
        let reg = PluginRegistry::with_base_dir(rt(), dir.path().to_owned());
        let count = reg.scan().await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn load_from_dir_missing_manifest_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let reg = PluginRegistry::with_base_dir(rt(), dir.path().to_owned());
        let result = reg.load_from_dir(dir.path()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn unload_unknown_plugin_returns_not_loaded() {
        let reg = PluginRegistry::new(rt());
        let err = reg.unload("ghost").unwrap_err();
        assert!(matches!(err, PluginError::NotLoaded { .. }));
    }
}
