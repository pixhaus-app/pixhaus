---
title: Plugin manifest
description: The plugin.toml manifest format.
---

Every Pixhaus plugin has a `plugin.toml` in its root folder. This file declares the plugin's identity, entry point, and permissions.

## Full schema

```toml
[plugin]
name        = "My Plugin"         # display name (required)
version     = "0.1.0"             # semver (required)
author      = "Your Name"         # display author (required)
description = "Short description" # shown in the plugin manager (required)
entry       = "main.lua"          # Lua entry point OR...
# entry     = "plugin.wasm"       # ...WASM entry point (mutually exclusive)

# Optional metadata
homepage    = "https://example.com/my-plugin"
license     = "MIT"
icon        = "icon.png"          # 32x32 PNG shown in plugin manager

[permissions]
# Grant only what the plugin needs. Missing keys default to false.
commands    = true    # register command palette entries
panels      = false   # register custom UI panels
verbs       = false   # register custom AI verbs
formats     = false   # register file format readers/writers
tools       = false   # register custom drawing tools

# Optional: file system access beyond the project folder
# Requires explicit user approval on first run.
# fs = ["~/Pictures"]  # list of allowed paths
```

## Version format

`version` follows [Semantic Versioning](https://semver.org/): `MAJOR.MINOR.PATCH`.

## Entry point

Specify either a `.lua` file (interpreted by the embedded Lua 5.4 runtime) or a `.wasm` file (executed by the Extism runtime in a sandbox). You cannot mix both in a single plugin.

## Permissions model

Pixhaus uses a capability-based permission model. A plugin only has access to the host APIs it declares in `[permissions]`. Requesting a permission does not prompt the user at install time — the prompt appears when the plugin first attempts to use the capability.

The `fs` key is the exception: it always prompts, because file system access outside the project is high-risk. The user can approve or deny the exact path list.
