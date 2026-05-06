//! WASM plugin host.
//!
//! Loads a `.wasm` plugin via extism and wires each verb it declares into the
//! verb runtime. The protocol between the host and the WASM plugin is a thin
//! JSON-over-string layer:
//!
//! - Host calls `plugin_init()` → plugin returns a JSON array of
//!   [`pixhaus_ai::plugin::descriptor::VerbDescriptor`]s it wants to register.
//! - Host calls `verb_invoke(json)` where the JSON is
//!   `{"verb_id": "<id>", "inputs": <VerbInputs.as_value()>}` → plugin returns
//!   a `VerbOutput` JSON object or surfaces an error via the extism error
//!   mechanism.
//!
//! This protocol is stable across S37. Lua plugins (S38) use the same
//! descriptor format but enter through the mlua host instead of extism.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument};

use pixhaus_ai::plugin::context::VerbContext;
use pixhaus_ai::plugin::descriptor::VerbDescriptor;
use pixhaus_ai::plugin::error::{Result as VerbResult, VerbError};
use pixhaus_ai::plugin::inputs::VerbInputs;
use pixhaus_ai::plugin::output::VerbOutput;
use pixhaus_ai::plugin::progress::VerbProgress;
use pixhaus_ai::plugin::verb::Verb;

use crate::error::{PluginError, Result};

/// A WASM plugin that has been loaded into memory.
///
/// `WasmPlugin` holds the extism `Plugin` behind a `Mutex` so `WasmVerb`
/// instances (which are `Send + Sync`) can share a reference to it.
/// `extism::Plugin::call` takes `&mut self`, requiring exclusive access.
pub(crate) struct WasmPlugin {
    name: String,
    inner: Arc<Mutex<extism::Plugin>>,
    /// Verb descriptors this plugin declared in `plugin_init`.
    pub(crate) descriptors: Vec<VerbDescriptor>,
}

impl WasmPlugin {
    /// Loads a WASM file, calls `plugin_init`, and collects verb descriptors.
    ///
    /// # Errors
    /// Returns [`PluginError::Wasm`] when extism fails to load the module or
    /// when `plugin_init` returns unparsable JSON.
    #[instrument(skip(entry_path), fields(plugin = plugin_name))]
    pub(crate) fn load(plugin_name: &str, entry_path: &Path) -> Result<Self> {
        let wasm = extism::Wasm::file(entry_path);
        let manifest = extism::Manifest::new([wasm]);
        // `true` enables WASI for plugins that need it; the sandbox limits
        // filesystem access to what extism's default allow list exposes.
        let plugin = extism::Plugin::new(&manifest, [], true).map_err(|e| PluginError::Wasm {
            plugin: plugin_name.to_owned(),
            message: e.to_string(),
        })?;

        let inner = Arc::new(Mutex::new(plugin));

        // Call plugin_init to get the verb descriptors.
        let descriptors = {
            let mut guard = inner.lock();
            let raw =
                guard
                    .call::<&str, &str>("plugin_init", "")
                    .map_err(|e| PluginError::Wasm {
                        plugin: plugin_name.to_owned(),
                        message: format!("plugin_init failed: {e}"),
                    })?;
            parse_descriptors(plugin_name, raw)?
        };

        debug!(
            plugin = plugin_name,
            verb_count = descriptors.len(),
            "WASM plugin loaded"
        );

        Ok(Self {
            name: plugin_name.to_owned(),
            inner,
            descriptors,
        })
    }

    /// Constructs a [`WasmVerb`] for each descriptor this plugin declared.
    pub(crate) fn into_verbs(self) -> Vec<WasmVerb> {
        let inner = self.inner;
        self.descriptors
            .into_iter()
            .map(|desc| WasmVerb {
                descriptor: desc,
                plugin: inner.clone(),
                plugin_name: self.name.clone(),
            })
            .collect()
    }
}

/// A single verb backed by a WASM plugin.
///
/// Implements [`Verb`] so the verb runtime treats it identically to a
/// built-in verb. The WASM call is dispatched to a blocking thread via
/// `spawn_blocking` because extism's call path is synchronous CPU work.
pub(crate) struct WasmVerb {
    descriptor: VerbDescriptor,
    plugin: Arc<Mutex<extism::Plugin>>,
    plugin_name: String,
}

#[async_trait]
impl Verb for WasmVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    #[instrument(skip(self, _ctx, inputs, _progress, _cancel),
                 fields(verb = %self.descriptor.id, plugin = %self.plugin_name))]
    async fn invoke(
        &self,
        _ctx: VerbContext,
        inputs: VerbInputs,
        _progress: VerbProgress,
        _cancel: CancellationToken,
    ) -> VerbResult<VerbOutput> {
        // Serialise the dispatch payload. Defined before any statements to
        // satisfy the items-after-statements lint.
        #[derive(serde::Serialize)]
        struct DispatchPayload {
            verb_id: String,
            inputs: serde_json::Value,
        }

        let plugin = self.plugin.clone();
        let verb_id = self.descriptor.id.as_str().to_owned();
        let plugin_name = self.plugin_name.clone();

        let call_input = serde_json::to_string(&DispatchPayload {
            verb_id: verb_id.clone(),
            inputs: inputs.as_value().clone(),
        })
        .map_err(VerbError::Payload)?;

        // Run the WASM call on a blocking thread — never block the async
        // runtime with synchronous WASM execution.
        let raw_output = tokio::task::spawn_blocking(move || {
            let mut guard = plugin.lock();
            guard
                .call::<&str, &str>("verb_invoke", &call_input)
                .map(str::to_owned)
                .map_err(|e| format!("{e}"))
        })
        .await
        .map_err(|e| VerbError::Aborted(format!("WASM task panicked: {e}")))?
        .map_err(|msg| {
            VerbError::Backend(format!("plugin `{plugin_name}` verb_invoke error: {msg}"))
        })?;

        serde_json::from_str::<VerbOutput>(&raw_output).map_err(VerbError::Payload)
    }
}

/// Parses the JSON array of verb descriptors returned by `plugin_init`.
fn parse_descriptors(plugin_name: &str, raw: &str) -> Result<Vec<VerbDescriptor>> {
    serde_json::from_str::<Vec<VerbDescriptor>>(raw).map_err(|e| PluginError::Wasm {
        plugin: plugin_name.to_owned(),
        message: format!("plugin_init returned invalid descriptor JSON: {e}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_descriptor_list() {
        let descs = parse_descriptors("echo", "[]").unwrap();
        assert!(descs.is_empty());
    }

    #[test]
    fn parse_descriptor_list_error_includes_plugin_name() {
        let err = parse_descriptors("echo", "not json").unwrap_err();
        assert!(err.to_string().contains("echo"));
    }
}
