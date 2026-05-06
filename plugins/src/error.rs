//! Error type for the plugin system.

use thiserror::Error;

/// Closed set of failures that can occur during plugin loading, scanning, and
/// invocation routing.
#[derive(Debug, Error)]
pub enum PluginError {
    /// A `plugin.toml` manifest could not be parsed.
    #[error("manifest parse error in {path}: {source}")]
    ManifestParse {
        /// Filesystem path to the offending manifest.
        path: String,
        /// Underlying toml error.
        source: toml::de::Error,
    },

    /// The plugin directory or entry-point file could not be read.
    #[error("I/O error reading plugin {name}: {source}")]
    Io {
        /// Plugin name from the manifest, or the directory path when the
        /// manifest itself could not be read.
        name: String,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// The entry point declared in the manifest does not exist on disk.
    #[error("plugin `{name}` declares entry `{entry}` which does not exist")]
    EntryPointMissing {
        /// Plugin name.
        name: String,
        /// Declared entry point path.
        entry: String,
    },

    /// The entry point extension maps to no supported runtime.
    #[error("unsupported plugin runtime for entry point `{entry}` — expected .wasm or .lua")]
    UnsupportedRuntime {
        /// The entry point path (including the unrecognised extension).
        entry: String,
    },

    /// The WASM runtime returned an error while loading or calling the plugin.
    #[error("WASM error in plugin `{plugin}`: {message}")]
    Wasm {
        /// Plugin name.
        plugin: String,
        /// Error message from the extism/wasmtime layer.
        message: String,
    },

    /// The plugin attempted to register a capability it did not declare in its
    /// manifest's `[plugin.permissions]` table.
    #[error("plugin `{name}` tried to register `{capability}` but permission was denied")]
    PermissionDenied {
        /// Plugin name.
        name: String,
        /// Capability the plugin attempted to register (e.g. `"register_verbs"`).
        capability: String,
    },

    /// A plugin-defined verb could not be registered with the verb runtime.
    #[error("verb registration failed in plugin `{plugin}`: {source}")]
    VerbRegistration {
        /// Plugin name.
        plugin: String,
        /// Error from the verb runtime.
        #[source]
        source: pixhaus_ai::plugin::error::VerbError,
    },

    /// The caller requested an operation on a plugin that is not currently
    /// loaded.
    #[error("plugin `{name}` is not loaded")]
    NotLoaded {
        /// Requested plugin name.
        name: String,
    },

    /// The file-system watcher could not be initialised.
    #[error("plugin directory watch error: {0}")]
    Watch(String),
}

/// Result alias used throughout the plugin system.
pub type Result<T> = std::result::Result<T, PluginError>;
