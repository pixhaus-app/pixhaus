---
title: Developing plugins
description: Build Pixhaus plugins with Lua or WASM — manifest, APIs, hot-reload, and a step-by-step walkthrough.
---

import { Aside, Steps, FileTree } from "@astrojs/starlight/components";

Pixhaus plugins are folders on disk. Drop one into `~/.pixhaus/plugins/` and the editor loads it on startup. Plugins can register:

- Custom command palette commands
- Custom UI panels
- Custom AI verbs
- Custom file format readers and writers
- Custom drawing tools

See [Plugin manifest](/plugins/manifest/) for the full `plugin.toml` schema. This page is a step-by-step walkthrough from zero to a working plugin.

---

## Part 1 — hello command

The smallest useful plugin registers one command palette entry.

<Steps>

1. Create the plugin folder:

   ```
   ~/.pixhaus/plugins/hello-command/
   ```

2. Add `plugin.toml`:

   ```toml
   [plugin]
   name        = "Hello Command"
   version     = "0.1.0"
   author      = "Your Name"
   description = "Adds a greeting to the command palette."
   entry       = "main.lua"

   [permissions]
   commands = true
   ```

3. Add `main.lua`:

   ```lua
   app.commands.register {
     name    = "hello-command:greet",
     label   = "Hello: Greet",
     execute = function()
       app.alert("Hello from Hello Command!")
     end,
   }
   ```

4. Open Pixhaus. Press **Ctrl+K** (or **Cmd+K** on macOS) and type "Hello" — the command appears. Select it to run.

</Steps>

The plugin source for this example ships in `examples/plugins/hello-command/`.

---

## Part 2 — palette CSV exporter

A more realistic plugin: export the active palette as a CSV file. This one uses the file system and reads editor state.

<FileTree>

- hello-palette-export/
  - plugin.toml
  - main.lua

</FileTree>

**plugin.toml:**

```toml
[plugin]
name        = "Palette Export"
version     = "0.1.0"
author      = "Your Name"
description = "Exports the active palette as a CSV file."
entry       = "main.lua"

[permissions]
commands = true
fs       = ["~/Documents"]
```

<Aside>
The `fs` key lists paths the plugin may write to. Pixhaus shows a one-time approval prompt when the user first runs a command that touches the file system. The approved paths are stored in `~/.pixhaus/plugin-grants.toml`.
</Aside>

**main.lua:**

```lua
app.commands.register {
  name    = "palette-export:export-csv",
  label   = "Palette: Export as CSV",
  execute = function()
    local sprite = app.activeSprite
    if not sprite then
      app.alert("No sprite is open.")
      return
    end

    local pal   = sprite.palette
    local lines = { "index,hex,name" }

    for i = 0, #pal - 1 do
      local c   = pal:getColor(i)
      local hex = string.format("#%02x%02x%02x", c.red, c.green, c.blue)
      local name = c.name or ""
      table.insert(lines, string.format("%d,%s,%s", i, hex, name))
    end

    local path = app.fs.joinPath(app.fs.userDocsPath, "palette.csv")
    local file = io.open(path, "w")
    if not file then
      app.alert("Could not open " .. path .. " for writing.")
      return
    end
    file:write(table.concat(lines, "\n"))
    file:close()
    app.alert("Exported " .. #pal .. " colors to " .. path)
  end,
}
```

Source: `examples/plugins/palette-export/`.

---

## Part 3 — custom AI verb

AI verbs appear in the **AI** menu and in the command palette under their label. A Lua verb follows the same preview-then-commit lifecycle as a built-in verb: produce a result, the user accepts or cancels.

This example implements a **Grayscale** verb that desaturates the active layer.

<FileTree>

- grayscale-verb/
  - plugin.toml
  - main.lua

</FileTree>

**plugin.toml:**

```toml
[plugin]
name        = "Grayscale Verb"
version     = "0.1.0"
author      = "Your Name"
description = "Desaturate the active layer in one click."
entry       = "main.lua"

[permissions]
verbs = true
```

**main.lua:**

```lua
app.ai.registerVerb {
  -- Unique stable ID for this verb.  Use reverse-domain notation.
  id      = "com.example.grayscale",
  label   = "Grayscale",
  description = "Desaturate the active layer.",

  -- Cost estimate shown in the AI menu before the user runs the verb.
  -- Classical ops that don't call any backend are free.
  cost = { credits = 0 },

  -- run() receives a VerbContext table and returns a VerbResult.
  run = function(ctx)
    local layer = ctx.activeLayer
    if not layer then
      return { error = "No active layer." }
    end

    -- Collect new pixel data for every frame.
    local effects = {}
    for _, frame in ipairs(ctx.sprite.frames) do
      local cel = layer:cel(frame)
      if cel then
        local img = cel.image:clone()
        for px in img:pixels() do
          local c = px()
          -- Rec. 601 luminance
          local lum = math.floor(c.red * 0.299 + c.green * 0.587 + c.blue * 0.114)
          px(Color { r = lum, g = lum, b = lum, a = c.alpha })
        end
        table.insert(effects, {
          kind   = "replace_cel",
          layer  = layer,
          frame  = frame,
          pixels = img,
        })
      end
    end

    return {
      summary = "Desaturate layer \"" .. layer.name .. "\"",
      effects = effects,
    }
  end,
}
```

When the user picks **Grayscale** from the AI menu:

1. The runtime calls `run(ctx)` and collects the returned effects.
2. A preview panel shows the before/after diff.
3. The user clicks **Accept** — the effects are committed as a single undo entry.
4. The user clicks **Cancel** — nothing changes.

Source: `examples/plugins/grayscale-verb/`.

For verbs that call an inference backend (image generation, vision-language models), see [AI verb authoring](/plugins/ai-verb-authoring/).

---

## Hot-reload

While `~/.pixhaus/plugins/<plugin-name>/` is open and Pixhaus is running, any file change inside that folder triggers an automatic reload. The status bar briefly shows "Reloaded *plugin name*". You do not need to restart the editor during development.

To force a reload without editing a file: **Edit > Plugins > Reload All**.

---

## Sandbox

Plugins run in a restricted environment. The restrictions are different for Lua vs. WASM.

### Lua sandbox

Standard Lua 5.4 with these restrictions:

| Blocked | Why |
|---|---|
| `os.execute`, `io.popen` | Arbitrary shell access |
| `require` for native `.so`/`.dll` modules | Binary injection |
| `debug.*` | Runtime introspection bypass |
| Network sockets | Unbounded external access |

File I/O (`io.open`) is allowed for paths listed in the plugin's `[permissions] fs` array and approved by the user. All other paths return `nil` from `io.open`.

### WASM sandbox

WASM plugins run in an Extism host sandbox:

- No filesystem access at all by default.
- The host exposes specific functions via the Extism PDK; only those are callable.
- Memory is isolated; the plugin cannot inspect editor state beyond what the host passes it.

See [WASM plugins](/plugins/wasm-plugins/) for the full walkthrough.

---

## Next steps

- [Plugin manifest reference](/plugins/manifest/) — full `plugin.toml` schema
- [WASM plugins](/plugins/wasm-plugins/) — compile Rust to WASM and ship it as a plugin
- [UI extensions](/plugins/ui-extensions/) — register panels and custom tools
- [AI verb authoring](/plugins/ai-verb-authoring/) — call inference backends from a verb
- [Publishing a plugin](/plugins/publishing/) — package, distribute, and version your plugin
- [Lua API reference](/scripting/lua-api/) — every `app.*` call documented
