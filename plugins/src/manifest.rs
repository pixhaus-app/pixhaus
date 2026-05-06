//! Plugin manifest — `plugin.toml` format.
//!
//! Every plugin directory contains exactly one `plugin.toml`. The loader
//! reads it, validates entry-point existence, and uses it to wire the plugin
//! into the appropriate runtime host (WASM via extism, Lua via mlua in S38).

use serde::{Deserialize, Serialize};

/// Top-level structure that maps to the full contents of `plugin.toml`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    /// `[plugin]` table.
    pub plugin: PluginMeta,
}

/// Contents of the `[plugin]` table.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginMeta {
    /// Machine-readable plugin name. Used as the registry key and as the
    /// namespace prefix for verb IDs registered by this plugin.
    /// Convention: `kebab-case`, ASCII only.
    pub name: String,

    /// Semantic version of this plugin.
    pub version: String,

    /// Optional author or organisation string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    /// One-sentence description for the plugin browser.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Path to the plugin entry point, relative to the plugin directory.
    /// Extension determines the runtime:
    /// - `.wasm` → extism WASM host
    /// - `.lua` → mlua Lua runtime (S38)
    pub entry_point: String,

    /// Declared capabilities this plugin needs from the host.
    #[serde(default)]
    pub permissions: Permissions,
}

/// `[plugin.permissions]` table.
///
/// All fields default to `false`. The loader enforces these at load time —
/// a plugin that tries to register a verb when `register_verbs = false`
/// receives a `PermissionDenied` error and the plugin is unloaded.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct Permissions {
    /// May register custom AI verbs via the verb runtime.
    #[serde(default)]
    pub register_verbs: bool,

    /// May register custom canvas tools (brush, fill, selection algorithms).
    #[serde(default)]
    pub register_tools: bool,

    /// May register custom UI panels.
    #[serde(default)]
    pub register_panels: bool,

    /// May register entries in the command palette.
    #[serde(default)]
    pub register_commands: bool,

    /// May register custom file format readers.
    #[serde(default)]
    pub register_format_readers: bool,

    /// May register custom file format writers.
    #[serde(default)]
    pub register_format_writers: bool,
}

/// Runtime that handles a plugin's entry point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginRuntime {
    /// extism WASM host.
    Wasm,
    /// mlua Lua runtime (S38).
    Lua,
}

impl PluginMeta {
    /// Infers the runtime from the entry-point file extension. Returns `None`
    /// for unrecognised extensions; the loader surfaces this as
    /// [`super::error::PluginError::UnsupportedRuntime`].
    #[must_use]
    pub fn runtime(&self) -> Option<PluginRuntime> {
        match std::path::Path::new(&self.entry_point)
            .extension()
            .and_then(|e| e.to_str())
        {
            Some("wasm") => Some(PluginRuntime::Wasm),
            Some("lua") => Some(PluginRuntime::Lua),
            _ => None,
        }
    }
}

/// Parses a `plugin.toml` manifest from a TOML string.
///
/// # Errors
/// Returns [`super::error::PluginError::ManifestParse`] when the string is
/// not valid TOML or does not conform to the manifest schema.
pub fn parse(toml_str: &str, path: &str) -> super::error::Result<Manifest> {
    toml::from_str(toml_str).map_err(|source| super::error::PluginError::ManifestParse {
        path: path.to_owned(),
        source,
    })
}

/// Returns the JSON Schema that describes the `plugin.toml` format.
///
/// Tooling (language-server plugins, IDE extensions) can use this to
/// provide autocompletion and inline validation for manifest files.
#[must_use]
pub fn json_schema() -> &'static str {
    include_str!("../schema/plugin-manifest.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
[plugin]
name = "echo"
version = "0.1.0"
entry_point = "echo.wasm"
"#;

    const FULL: &str = r#"
[plugin]
name = "my-plugin"
version = "1.2.3"
author = "Pixhaus Team"
description = "A sample plugin"
entry_point = "main.wasm"

[plugin.permissions]
register_verbs = true
register_commands = true
"#;

    #[test]
    fn parse_minimal_manifest() {
        let m = parse(MINIMAL, "plugin.toml").unwrap();
        assert_eq!(m.plugin.name, "echo");
        assert_eq!(m.plugin.version, "0.1.0");
        assert_eq!(m.plugin.entry_point, "echo.wasm");
        assert!(!m.plugin.permissions.register_verbs);
    }

    #[test]
    fn parse_full_manifest() {
        let m = parse(FULL, "plugin.toml").unwrap();
        assert_eq!(m.plugin.name, "my-plugin");
        assert_eq!(m.plugin.author.as_deref(), Some("Pixhaus Team"));
        assert!(m.plugin.permissions.register_verbs);
        assert!(m.plugin.permissions.register_commands);
        assert!(!m.plugin.permissions.register_tools);
    }

    #[test]
    fn runtime_detection() {
        let mut meta = parse(MINIMAL, "plugin.toml").unwrap().plugin;
        assert_eq!(meta.runtime(), Some(PluginRuntime::Wasm));

        meta.entry_point = "script.lua".into();
        assert_eq!(meta.runtime(), Some(PluginRuntime::Lua));

        meta.entry_point = "unknown.js".into();
        assert_eq!(meta.runtime(), None);
    }

    #[test]
    fn invalid_toml_returns_error() {
        let result = parse("not valid toml {{{", "bad.toml");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("bad.toml"));
    }

    #[test]
    fn schema_is_valid_json() {
        let schema = json_schema();
        let _: serde_json::Value =
            serde_json::from_str(schema).expect("plugin-manifest.json must be valid JSON");
    }
}
