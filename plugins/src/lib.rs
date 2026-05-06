//! Pixhaus plugin loader and public API surface.
//!
//! Plugins are directories in `~/.pixhaus/plugins/<name>/` containing a
//! `plugin.toml` manifest and either a `.wasm` or `.lua` entry point:
//!
//! - **WASM plugins** — loaded via extism. The host calls two exports:
//!   `plugin_init()` (returns verb descriptors as JSON) and
//!   `verb_invoke(json)` (dispatches a verb call).
//! - **Lua plugins** — runtime wired by S38. This crate records Lua plugins
//!   in the registry but defers execution until mlua bindings land.
//!
//! # Quick start for plugin authors
//!
//! 1. Create `~/.pixhaus/plugins/my-plugin/plugin.toml`:
//!
//!    ```toml
//!    [plugin]
//!    name = "my-plugin"
//!    version = "0.1.0"
//!    entry_point = "my_plugin.wasm"
//!
//!    [plugin.permissions]
//!    register_verbs = true
//!    ```
//!
//! 2. Compile your Rust plugin with the extism PDK and place the `.wasm`
//!    output next to `plugin.toml`. See `examples/plugins/echo/` for a
//!    complete worked example.
//!
//! 3. Restart Pixhaus (or call the `plugin_reload` IPC command once S38 is
//!    wired to the hot-reload watcher).

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::missing_panics_doc,
        clippy::disallowed_methods
    )
)]

pub mod error;
pub mod manifest;
pub mod registry;
pub mod watcher;

mod wasm;

pub use error::{PluginError, Result};
pub use manifest::{Manifest, Permissions, PluginMeta, PluginRuntime};
pub use registry::{LoadedPlugin, PluginInfo, PluginRegistry};
pub use watcher::{PluginWatcher, start as start_watcher};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_surface_is_stable() {
        // Smoke-test that the public re-exports compile.
        let _: fn() -> &'static str = manifest::json_schema;
    }
}
