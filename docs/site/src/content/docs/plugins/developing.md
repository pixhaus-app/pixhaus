---
title: Developing plugins
description: Build plugins for Pixhaus using Lua or WASM.
---

import { Aside } from "@astrojs/starlight/components";

<Aside type="caution">
The plugin system lands with stream S37. This page is a forward-reference stub.
</Aside>

Pixhaus plugins are folders with a `plugin.toml` manifest and either Lua scripts or pre-compiled WASM. Plugins can register:

- Custom AI verbs
- Custom drawing tools
- Custom UI panels
- Custom file format readers/writers
- Custom command palette commands

## Plugin location

Plugins live in `~/.pixhaus/plugins/<plugin-name>/`. On first launch, Pixhaus scans this directory and loads all valid plugins.

## Quickstart (Lua)

```
my-plugin/
  plugin.toml
  main.lua
```

**plugin.toml:**
```toml
[plugin]
name = "My Plugin"
version = "0.1.0"
author = "Your Name"
description = "Does something useful."
entry = "main.lua"

[permissions]
commands = true
panels = false
verbs = false
```

**main.lua:**
```lua
app.commands.register {
  name = "my-plugin:hello",
  label = "Say Hello",
  execute = function()
    app.alert("Hello from My Plugin!")
  end
}
```

## Quickstart (WASM)

WASM plugins are compiled with `extism`. The plugin exposes functions that the host calls using the Extism protocol.

See `examples/plugins/wasm-example/` in the repository for a full Rust-WASM plugin example.

## Hot-reload

While developing a plugin, any change to a file inside the plugin folder triggers an automatic reload without restarting the editor. A reload notification appears in the status bar.

## Sandbox

WASM plugins cannot access the filesystem outside the current project folder. Lua plugins run in a restricted environment — no `os.execute`, no arbitrary network access.

## Next steps

- [Plugin manifest reference](/plugins/manifest/)
- [Publishing a plugin](/plugins/publishing/)
- [Lua API reference](/scripting/lua-api/)
