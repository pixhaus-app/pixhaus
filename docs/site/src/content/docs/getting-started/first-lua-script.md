---
title: Write your first Lua script
description: Automate a palette sort using the Pixhaus Lua scripting API.
---

import { Steps, Aside } from "@astrojs/starlight/components";

Pixhaus exposes a Lua 5.4 scripting API that mirrors Aseprite's `app` global. This tutorial writes a script that sorts the active sprite's palette by luminance — a simple but practical operation that demonstrates reading and writing palette data.

**Starter file:** `examples/tutorials/lua-palette-start.pixhaus` — a 32×32 sprite with a 16-color palette in arbitrary order.

<Steps>
1. **Open the starter file.** `File > Open` and navigate to `examples/tutorials/lua-palette-start.pixhaus`.

2. **Open the script console.** `File > Scripts > Console`. A Lua REPL appears at the bottom of the editor. You can run one-liner expressions here before writing a full script file.

3. **Inspect the palette.** In the console, type:

   ```lua
   local pal = app.activeSprite.palette
   print(#pal)
   ```

   Press `Enter`. The console prints `16` — the palette has 16 entries.

4. **Create a script file.** `File > Scripts > New script`. A file dialog opens; save the file as `sort-palette-by-luminance.lua` in `~/.pixhaus/scripts/` (or anywhere on disk). The script editor opens.

5. **Write the sort script.** Paste the following:

   ```lua
   -- Sort the active sprite's palette by luminance (dark to light).
   -- Transparent index 0 stays at index 0.

   local sprite = app.activeSprite
   if not sprite then
     app.alert("No active sprite.")
     return
   end

   local pal = sprite.palette
   local n = #pal

   -- Collect all entries except index 0 (transparent).
   local entries = {}
   for i = 1, n - 1 do
     local c = pal:getColor(i)
     -- BT.601 luminance
     local lum = 0.299 * c.red + 0.587 * c.green + 0.114 * c.blue
     entries[#entries + 1] = { color = c, lum = lum }
   end

   -- Sort by luminance ascending.
   table.sort(entries, function(a, b) return a.lum < b.lum end)

   -- Write sorted colors back, starting at index 1.
   app.transaction(function()
     for i, entry in ipairs(entries) do
       pal:setColor(i, entry.color)
     end
   end)

   app.refresh()
   print("Palette sorted by luminance.")
   ```

6. **Run the script.** `File > Scripts > Run script` and select your file, or click **Run** in the script editor toolbar. The palette panel reorders. The canvas updates immediately because the pixel data still references palette indices — only the palette entry values changed.

7. **Check the result.** Click palette entries in order from index 1 to 15. They should progress from dark to light. Index 0 (transparent) is unchanged.

8. **Add to the command palette.** To run this script from the command palette without navigating to `File > Scripts`, add it to the scripts directory and register it:

   ```lua
   -- At the top of your script file, add:
   if app.commands then
     app.commands.register {
       name  = "sort-palette-luminance",
       label = "Sort palette by luminance",
       execute = function()
         -- paste the body of the sort script here, or require() it
       end,
     }
   end
   ```

   After restarting Pixhaus (or running the registration block once), `Ctrl+K` and typing `sort palette` shows your command.
</Steps>

<Aside>
`app.transaction()` wraps all palette writes into a single undo entry. Without it, each `setColor()` call would be its own undo step — sorting a 16-color palette would take 15 undos to reverse.
</Aside>

## What to try next

**Script that adds a color ramp.** Use `Color { r=..., g=..., b=... }` to construct colors and `pal:setColor()` to append a gradient between two chosen colors.

**Script that outlines a layer.** Iterate over every cel in the active layer, read pixel data, detect edge pixels (non-transparent neighbors), and paint them with a chosen outline color. See `app.activeSprite:newCel()` and cel pixel access in the [Lua API reference](/scripting/lua-api/).

**Script that exports every animation tag as a separate GIF.** Loop over `sprite.tags`, activate each tag's frame range, and call `app.command.ExportSpriteSheet()` with a filename derived from the tag name.

## Packaging as a plugin

Once your script is working, package it as a Pixhaus plugin so it can be shared:

1. Create a folder `~/.pixhaus/plugins/my-palette-tools/`.
2. Add a `plugin.toml` manifest:

   ```toml
   name        = "my-palette-tools"
   version     = "0.1.0"
   author      = "You"
   description = "Palette automation scripts"
   entry       = "init.lua"
   permissions = ["commands"]
   ```

3. Move your script to `init.lua` inside the folder.
4. Restart Pixhaus. The plugin loads automatically.

See the [plugin developer guide](/plugins/developing/) for the full manifest format and permission model.

## Next steps

- Read the full [Lua API reference](/scripting/lua-api/)
- Read the [plugin developer guide](/plugins/developing/)
- See [scripting examples](/scripting/examples/) for more complete scripts
