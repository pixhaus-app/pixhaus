//! Echo verb — reference WASM plugin for Pixhaus.
//!
//! This plugin demonstrates the minimal WASM plugin protocol:
//!
//! 1. `plugin_init` → returns a JSON array of VerbDescriptors the plugin
//!    wants to register. The host calls this once on load.
//!
//! 2. `verb_invoke` → receives `{"verb_id": "...", "inputs": {...}}` and
//!    returns a JSON VerbOutput. The host calls this each time the user
//!    triggers the verb.
//!
//! # Building
//!
//! ```sh
//! cargo build --release --target wasm32-wasip1
//! ```
//!
//! # Installing
//!
//! ```sh
//! DEST=~/.pixhaus/plugins/echo
//! mkdir -p $DEST
//! cp target/wasm32-wasip1/release/pixhaus_plugin_echo.wasm $DEST/echo.wasm
//! cp examples/plugins/echo/plugin.toml $DEST/plugin.toml
//! ```

use extism_pdk::*;
use serde::{Deserialize, Serialize};
use serde_json::json;

// ── Types that mirror the host-side verb protocol ──────────────────────────

/// Verb invocation request sent by the host.
#[derive(Debug, Deserialize)]
struct InvokeRequest {
    verb_id: String,
    inputs: serde_json::Value,
}

/// VerbOutput subset: the host deserialises into the full `VerbOutput` type.
#[derive(Debug, Serialize)]
struct VerbOutput {
    summary: String,
    effects: Vec<serde_json::Value>,
    thumbnail: Option<serde_json::Value>,
    actual_cost: serde_json::Value,
    notes: Vec<String>,
}

// ── Exports ────────────────────────────────────────────────────────────────

/// Called once by the host when the plugin is loaded.
///
/// Returns a JSON array of VerbDescriptor objects — one per verb this plugin
/// wants to register.
#[plugin_fn]
pub fn plugin_init(_input: ()) -> FnResult<Json<serde_json::Value>> {
    let descriptors = json!([
        {
            "id": "com.example.echo",
            "display_name": "Echo",
            "description": "Returns its inputs as a note — useful for testing the plugin system.",
            "version": "0.1.0",
            "required_capabilities": 0,
            "input_schema": {
                "type": "object",
                "description": "Any JSON object; it is echoed back as a note."
            },
            "output_kinds": [{ "kind": "critique" }],
            "cost_estimate": {
                "typical_latency": { "secs": 0, "nanos": 0 },
                "max_latency": { "secs": 0, "nanos": 0 },
                "typical_usd_cents": 0.0,
                "max_usd_cents": 0.0
            },
            "streaming": false,
            "cancellable": false
        }
    ]);
    Ok(Json(descriptors))
}

/// Called by the host each time the user invokes one of this plugin's verbs.
///
/// Receives `{"verb_id": "...", "inputs": {...}}` and returns a VerbOutput
/// JSON object.
#[plugin_fn]
pub fn verb_invoke(Json(req): Json<InvokeRequest>) -> FnResult<Json<VerbOutput>> {
    let note = format!(
        "Echo from plugin `{}`: inputs = {}",
        req.verb_id,
        req.inputs,
    );

    let output = VerbOutput {
        summary: format!("Echo: {}", req.verb_id),
        effects: vec![],
        thumbnail: None,
        actual_cost: json!({
            "elapsed": { "secs": 0, "nanos": 0 },
            "usd_cents": 0.0
        }),
        notes: vec![note],
    };

    Ok(Json(output))
}
