# Plugin system

Pixhaus plugins extend the editor with custom AI verbs, canvas tools, UI
panels, file format readers and writers, and command palette entries. Plugins
are directories dropped into `~/.pixhaus/plugins/`; no package manager or
editor restart is required for supported hot-reload scenarios.

## Quick start

Create a directory in the plugin folder, add a manifest, and drop in your
compiled WASM file or Lua script:

```
~/.pixhaus/plugins/
  my-plugin/
    plugin.toml    ← manifest (required)
    my_plugin.wasm ← WASM entry point, or my_plugin.lua for Lua
```

The editor scans this directory on startup and whenever a file-system change
is detected in one of the plugin directories.

## plugin.toml

Every plugin directory must contain exactly one `plugin.toml`. The format:

```toml
[plugin]
name        = "my-plugin"          # kebab-case, ASCII only
version     = "0.1.0"             # semantic version
author      = "Jane Developer"    # optional
description = "Does something useful"  # optional, one sentence
entry_point = "my_plugin.wasm"    # relative to plugin directory

[plugin.permissions]
register_verbs           = true   # may add verbs to the verb runtime
register_tools           = false  # may add canvas tools
register_panels          = false  # may add UI panels
register_commands        = false  # may add command-palette entries
register_format_readers  = false  # may add file format readers
register_format_writers  = false  # may add file format writers
```

All `[plugin.permissions]` keys default to `false`. The loader enforces
permissions at load time — a plugin that tries to register a capability it
did not declare is unloaded immediately and an error is logged.

A JSON Schema for `plugin.toml` is embedded in `pixhaus-plugins` and
accessible at runtime via `pixhaus_plugins::manifest::json_schema()`.

## Runtimes

### WASM (extism)

Plugins with a `.wasm` entry point are hosted by [extism], which wraps
[wasmtime] and provides the cross-language PDK. WASM plugins run in a
sandboxed environment: filesystem access is limited to what extism's default
allow list exposes, and the network is not accessible unless the plugin
declares the corresponding WASI socket permission (not currently exposed).

#### Protocol

The WASM module must export two functions:

**`plugin_init() → string`**

Called once when the plugin is loaded. Must return a JSON array of
`VerbDescriptor` objects (one per verb the plugin wants to register):

```json
[
  {
    "id": "com.example.my-verb",
    "display_name": "My Verb",
    "description": "One-sentence description.",
    "version": "0.1.0",
    "required_capabilities": 0,
    "input_schema": { "type": "object" },
    "output_kinds": [{ "kind": "add_layer" }],
    "cost_estimate": {
      "typical_latency": { "secs": 0, "nanos": 0 },
      "max_latency": { "secs": 0, "nanos": 0 },
      "typical_usd_cents": 0.0,
      "max_usd_cents": 0.0
    },
    "streaming": false,
    "cancellable": false
  }
]
```

**`verb_invoke(input: string) → string`**

Called each time a user triggers a verb. Input is:

```json
{ "verb_id": "com.example.my-verb", "inputs": { /* verb-specific */ } }
```

Output must be a `VerbOutput` JSON object:

```json
{
  "summary": "Did the thing.",
  "effects": [],
  "thumbnail": null,
  "actual_cost": { "elapsed": { "secs": 0, "nanos": 1000000 }, "usd_cents": 0.0 },
  "notes": []
}
```

Return an extism error (via the PDK's `FnResult::Err`) to surface a failure
back to the verb runtime, which will propagate it as `VerbError::Backend`.

#### Writing a WASM plugin in Rust

Add the extism PDK to your plugin crate and compile to `wasm32-wasip1`:

```toml
# Cargo.toml
[lib]
crate-type = ["cdylib"]

[dependencies]
extism-pdk = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

```rust
// src/lib.rs
use extism_pdk::*;
use serde_json::json;

#[plugin_fn]
pub fn plugin_init(_: ()) -> FnResult<Json<serde_json::Value>> {
    Ok(Json(json!([{
        "id": "com.example.my-verb",
        // ... descriptor fields ...
    }])))
}

#[plugin_fn]
pub fn verb_invoke(Json(req): Json<serde_json::Value>) -> FnResult<Json<serde_json::Value>> {
    let output = json!({
        "summary": "Done.",
        "effects": [],
        "thumbnail": null,
        "actual_cost": { "elapsed": { "secs": 0, "nanos": 0 }, "usd_cents": 0.0 },
        "notes": []
    });
    Ok(Json(output))
}
```

Build and install:

```sh
cargo build --release --target wasm32-wasip1

DEST=~/.pixhaus/plugins/my-plugin
mkdir -p "$DEST"
cp target/wasm32-wasip1/release/my_plugin.wasm "$DEST/my_plugin.wasm"
cp plugin.toml "$DEST/plugin.toml"
```

See `examples/plugins/echo/` for the complete reference plugin.

### Lua (S38)

Plugins with a `.lua` entry point use the mlua runtime added in S38. The
manifest format is identical; the runtime binding is different. Until S38
lands, Lua plugins are accepted by the loader but run without bindings — the
plugin is recorded in the registry and a warning is logged.

## Hot-reload

The loader starts a file-system watcher on `~/.pixhaus/plugins/` at startup.
When a plugin file changes (manifest or entry point), the affected plugin is
unloaded and reloaded automatically. Verbs registered by the old instance are
removed before the new instance registers its own.

Hot-reload can also be triggered manually via the IPC command `plugin_reload`
or through the editor's plugin browser once it is implemented.

## IPC commands

| Command | Description |
|---|---|
| `plugin_list` | Returns `PluginInfo[]` for all loaded plugins. |
| `plugin_scan` | Re-scans `~/.pixhaus/plugins/` and loads new plugins. |
| `plugin_reload` | Unloads and reloads a specific plugin by name. |
| `plugin_unload` | Unloads a plugin by name; its verbs are removed immediately. |

## Permissions model

The `[plugin.permissions]` table is the trust boundary. Pixhaus is a desktop
app with no code-signing requirement for plugins, so permissions are advisory
rather than cryptographic — they prevent accidental misuse rather than
malicious actors. Future work may add signature verification for plugins
distributed via a registry.

Current permissions and what they gate:

| Permission | What it enables |
|---|---|
| `register_verbs` | Registering entries in the verb runtime via `plugin_init`. |
| `register_tools` | (Reserved; not yet implemented.) |
| `register_panels` | (Reserved; not yet implemented.) |
| `register_commands` | (Reserved; not yet implemented.) |
| `register_format_readers` | (Reserved; not yet implemented.) |
| `register_format_writers` | (Reserved; not yet implemented.) |

## Plugin directory location

| Platform | Default path |
|---|---|
| Windows | `%LOCALAPPDATA%\pixhaus\plugins\` |
| macOS | `~/Library/Application Support/pixhaus/plugins/` |
| Linux | `~/.local/share/pixhaus/plugins/` |

The path is resolved via `dirs::data_local_dir()`. If the directory does not
exist on startup, the loader skips the scan silently — no error, zero plugins.
Create the directory and drop in a plugin to get started.

[extism]: https://extism.org
[wasmtime]: https://wasmtime.dev
