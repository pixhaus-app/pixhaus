---
title: Lua API reference
description: The Pixhaus Lua scripting API for plugins and automation.
---

import { Aside } from "@astrojs/starlight/components";

<Aside type="caution">
The Lua API lands with stream S38. This page is a forward-reference stub populated from the planned API surface.
</Aside>

Pixhaus exposes a Lua 5.4 scripting API via the `mlua` crate. The API surface mirrors Aseprite's `app` global where possible, so existing Aseprite scripts have a migration path.

## Global: `app`

The `app` global is the entry point to the editor state.

```lua
-- Current project
local sprite = app.activeSprite     -- Sprite or nil
local layer  = app.activeLayer      -- Layer or nil
local frame  = app.activeFrame      -- Frame or nil
local cel    = app.activeCel        -- Cel or nil

-- Colors
local fg = app.fgColor              -- Color
local bg = app.bgColor              -- Color

-- Commands
app.command.NewSprite()
app.command.SaveFile()
```

## Sprite

```lua
local sprite = app.activeSprite
sprite.width         -- number
sprite.height        -- number
sprite.colorMode     -- "rgb" or "indexed"
sprite.palette       -- Palette

-- Layers
for _, layer in ipairs(sprite.layers) do
  print(layer.name)
end

-- Frames
for _, frame in ipairs(sprite.frames) do
  print(frame.duration)  -- milliseconds
end
```

## Layer

```lua
local layer = app.activeLayer
layer.name
layer.opacity         -- 0-255
layer.blendMode       -- "normal", "multiply", etc.
layer.isVisible
layer.isLocked
layer.isTilemap       -- true if tilemap layer
```

## Color

```lua
local c = Color { r=255, g=128, b=0, a=255 }
local c = Color { index=5 }          -- indexed mode
local c = Color { h=30, s=1, v=1 }  -- HSV
```

## Palette

```lua
local pal = app.activeSprite.palette
pal:getColor(0)        -- Color at index 0
pal:setColor(0, c)     -- Set color at index 0
#pal                   -- Palette size
```

## Pixhaus extensions

Pixhaus adds these beyond the Aseprite API:

```lua
-- Register a custom verb
app.ai.registerVerb {
  name = "my-verb",
  label = "My Custom Verb",
  run = function(context)
    -- context.sprite, context.palette, context.frames
    return { previewLayer = ... }
  end
}

-- Register a command palette entry
app.commands.register {
  name = "my-command",
  label = "My Custom Command",
  execute = function() ... end
}

-- Custom panel
app.ui.panel {
  title = "My Panel",
  content = function(widget)
    widget:label("Hello from Lua")
    widget:button("Click me", function() ... end)
  end
}
```

## Aseprite compatibility

Pixhaus implements the Aseprite Lua API surface described at https://github.com/aseprite/api. Common scripts (Color Reduction, Outline, Sprite Sheet Generator) should port with under 20 lines of changes. Incompatibilities are documented in `docs/aseprite-compat.md`.
