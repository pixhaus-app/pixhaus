---
title: WASM plugins
description: Compile a Rust plugin to WebAssembly and run it inside Pixhaus.
---

import { Aside, Steps, FileTree, Code } from "@astrojs/starlight/components";

WASM plugins are compiled binaries that run in an isolated sandbox via [Extism](https://extism.org/). They are the right choice when you need:

- Performance-critical pixel operations
- Third-party Rust crates (image codecs, math libraries, etc.)
- Distributing a plugin without exposing source code
- Maximum isolation from the editor process

WASM plugins can register the same capabilities as Lua plugins — commands, verbs, formats, tools, and panels — by exporting named functions the host calls.

---

## Prerequisites

Install the Extism CLI and the Rust WASM target:

```sh
cargo install extism-cli
rustup target add wasm32-wasip1
```

---

## Step-by-step: invert-colors verb

This tutorial builds a **Invert Colors** verb in Rust, compiles it to WASM, and installs it in Pixhaus.

### 1. Create the project

```sh
cargo new --lib invert-colors-verb
cd invert-colors-verb
```

### 2. Configure `Cargo.toml`

```toml
[package]
name    = "invert-colors-verb"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
extism-pdk    = "1"
serde         = { version = "1", features = ["derive"] }
serde_json    = "1"
```

### 3. Write `src/lib.rs`

```rust
use extism_pdk::*;
use serde::{Deserialize, Serialize};

// --- Input / output types ------------------------------------------------

#[derive(Deserialize)]
struct InvertInput {
    /// RGBA pixel bytes, width * height * 4 values.
    pixels: Vec<u8>,
    width:  u32,
    height: u32,
}

#[derive(Serialize)]
struct InvertOutput {
    pixels: Vec<u8>,
    width:  u32,
    height: u32,
}

// --- Verb registration ---------------------------------------------------

/// Called once at load time.  Return a JSON descriptor.
#[plugin_fn]
pub fn pixhaus_describe() -> FnResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "type": "verb",
        "id":   "com.example.invert-colors",
        "label":       "Invert Colors",
        "description": "Invert the RGB channels of the active layer.",
        "cost": { "credits": 0 }
    })))
}

/// Called when the user runs the verb.
#[plugin_fn]
pub fn pixhaus_verb_run(Json(input): Json<InvertInput>) -> FnResult<Json<InvertOutput>> {
    let mut pixels = input.pixels;

    // Invert every pixel's RGB channels; leave alpha unchanged.
    for chunk in pixels.chunks_mut(4) {
        chunk[0] = 255 - chunk[0]; // R
        chunk[1] = 255 - chunk[1]; // G
        chunk[2] = 255 - chunk[2]; // B
                                   // chunk[3] = alpha — unchanged
    }

    Ok(Json(InvertOutput {
        pixels,
        width:  input.width,
        height: input.height,
    }))
}
```

### 4. Build the WASM binary

```sh
cargo build --release --target wasm32-wasip1
```

The output is at `target/wasm32-wasip1/release/invert_colors_verb.wasm`.

### 5. Create the plugin folder

<FileTree>

- invert-colors-verb/
  - plugin.toml
  - plugin.wasm  ← rename / copy the compiled binary here

</FileTree>

**plugin.toml:**

```toml
[plugin]
name        = "Invert Colors Verb"
version     = "0.1.0"
author      = "Your Name"
description = "Invert the RGB channels of the active layer."
entry       = "plugin.wasm"

[permissions]
verbs = true
```

Copy the compiled binary:

```sh
cp target/wasm32-wasip1/release/invert_colors_verb.wasm \
   ~/.pixhaus/plugins/invert-colors-verb/plugin.wasm
```

Pixhaus hot-reloads the plugin when the `.wasm` file changes. During development, add a `build.sh` that compiles and copies in one step:

```sh
#!/usr/bin/env sh
set -e
cargo build --release --target wasm32-wasip1
cp target/wasm32-wasip1/release/invert_colors_verb.wasm \
   ~/.pixhaus/plugins/invert-colors-verb/plugin.wasm
echo "Installed."
```

The full source ships in `examples/plugins/custom-verb-wasm/`.

---

## Host functions available to WASM plugins

The Extism host exposes these functions. Import them via the Extism PDK's `host_fn!` macro.

| Function | Signature | Description |
|---|---|---|
| `pixhaus_log` | `(level: i32, msg: *const u8, len: i32)` | Write a log message to the editor's console. Level 0 = debug, 1 = info, 2 = warn, 3 = error. |
| `pixhaus_alert` | `(msg: *const u8, len: i32)` | Show a modal alert. |
| `pixhaus_active_sprite_id` | `() -> i64` | Returns the current sprite ID, or -1 if none. |
| `pixhaus_active_layer_id` | `() -> i64` | Returns the current layer ID, or -1 if none. |
| `pixhaus_active_frame` | `() -> i32` | Returns the current frame index (0-based), or -1 if none. |

Additional host functions are added as the WASM plugin API matures. Use `pixhaus_log` for debug output rather than `eprintln!`; stderr is not captured inside the sandbox.

---

## Registering multiple capabilities

A single WASM module can export descriptors for more than one capability. Return an array from `pixhaus_describe`:

```rust
#[plugin_fn]
pub fn pixhaus_describe() -> FnResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!([
        {
            "type":  "verb",
            "id":    "com.example.invert-colors",
            "label": "Invert Colors"
        },
        {
            "type":    "command",
            "name":    "com.example.flip-horizontal",
            "label":   "Flip: Horizontal (fast)"
        }
    ])))
}
```

For each entry the runtime calls the matching dispatch function:

| Type | Entry point |
|---|---|
| `verb` | `pixhaus_verb_run` |
| `command` | `pixhaus_command_<name_slug>` where the slug replaces `:` and `.` with `_` |
| `format` | `pixhaus_format_read` and `pixhaus_format_write` |
| `tool` | `pixhaus_tool_stroke` |
| `panel` | `pixhaus_panel_render` |

---

## Sandboxing details

WASM plugins run in the Extism sandbox with these constraints:

- **No filesystem access.** The sandbox has no WASI filesystem capabilities. Pixel data passes through function arguments; the plugin cannot open files.
- **No network.** The WASI networking proposal is not enabled.
- **Memory cap.** The runtime enforces a default 256 MB heap limit per plugin.
- **CPU time.** A slow plugin does not stall the editor — verb invocations run on a background thread.

If your verb needs to write output files (e.g., a custom format writer), the host calls your export function with the serialized data; the host handles the actual file write after the sandbox exits.

---

## See also

- [Developing plugins](/plugins/developing/) — Lua plugins and general introduction
- [AI verb authoring](/plugins/ai-verb-authoring/) — calling inference backends from a verb
- [Lua API reference](/scripting/lua-api/) — `app.*` Lua API
