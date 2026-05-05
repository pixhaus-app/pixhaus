---
title: Example scripts
description: Sample Lua scripts for common Pixhaus automation tasks.
---

import { Aside } from "@astrojs/starlight/components";

<Aside type="caution">
The Lua scripting system lands with stream S38. These examples are forward-reference stubs.
</Aside>

## Palette to CSV

Export the active palette as a CSV file with hex values and names.

```lua
local sprite = app.activeSprite
if not sprite then return end

local pal = sprite.palette
local lines = {"index,hex,name"}
for i = 0, #pal - 1 do
  local c = pal:getColor(i)
  local hex = string.format("#%02x%02x%02x", c.red, c.green, c.blue)
  local name = c.name or ""
  table.insert(lines, string.format("%d,%s,%s", i, hex, name))
end

local path = app.fs.joinPath(app.fs.userDocsPath, "palette.csv")
local file = io.open(path, "w")
file:write(table.concat(lines, "\n"))
file:close()
app.alert("Exported to " .. path)
```

## Add outline layer

Add a 1-pixel outline around every cel on the active layer.

```lua
local sprite = app.activeSprite
local layer = app.activeLayer
if not sprite or not layer then return end

local outlineLayer = sprite:newLayer()
outlineLayer.name = layer.name .. " outline"
outlineLayer.blendMode = "normal"

app.transaction(function()
  for _, frame in ipairs(sprite.frames) do
    local cel = layer:cel(frame)
    if cel then
      -- Outline logic here (expand selection by 1, fill with fg color)
      local img = cel.image:clone()
      -- ... expand and fill ...
      outlineLayer:newCel(frame, img, cel.position)
    end
  end
end)
```

## Recolor indexed sprite

Replace one palette index with another across all cels.

```lua
local from = 5  -- source index
local to   = 7  -- target index

local sprite = app.activeSprite
if not sprite then return end

app.transaction(function()
  for _, layer in ipairs(sprite.layers) do
    for _, frame in ipairs(sprite.frames) do
      local cel = layer:cel(frame)
      if cel then
        local img = cel.image
        for pixel in img:pixels() do
          if pixel() == from then
            pixel(to)
          end
        end
      end
    end
  end
end)
```

## More examples

Sample plugins are in `examples/plugins/` in the repository, including:
- `color-reduction.lua` — posterize the active layer to a reduced color count
- `sprite-sheet-info.lua` — print frame dimensions and tag names to the console
- `aseprite-compat-test.lua` — verify which Aseprite API calls work in Pixhaus
