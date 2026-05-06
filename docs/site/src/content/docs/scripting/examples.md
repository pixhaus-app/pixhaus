---
title: Example scripts
description: Sample Lua scripts for common Pixhaus automation tasks.
---

These examples are runnable scripts and plugin entry points. Each is also available in `examples/plugins/` in the repository.

---

## Palette to CSV

Export the active palette as a CSV file with hex values and names.

```lua
-- examples/plugins/palette-export/main.lua
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

---

## Add outline layer

Add a 1-pixel outline around every cel on the active layer.

```lua
local sprite = app.activeSprite
local layer  = app.activeLayer
if not sprite or not layer then
  app.alert("No active layer.")
  return
end

local outlineLayer     = sprite:newLayer()
outlineLayer.name      = layer.name .. " outline"
outlineLayer.blendMode = "normal"

app.transaction(function()
  for _, frame in ipairs(sprite.frames) do
    local cel = layer:cel(frame)
    if cel then
      local src = cel.image
      local dst = src:clone()
      dst:clear(Color { r=0, g=0, b=0, a=0 })

      -- Expand 1 pixel in 4-connected neighbors.
      local fg = app.fgColor
      for px in src:pixels() do
        if px().alpha > 0 then
          local x, y = px.x, px.y
          for _, d in ipairs({ {0,-1},{0,1},{-1,0},{1,0} }) do
            local nx, ny = x + d[1], y + d[2]
            if nx >= 0 and ny >= 0 and nx < src.width and ny < src.height then
              if src:getPixel(nx, ny).alpha == 0 then
                dst:putPixel(nx, ny, fg)
              end
            end
          end
        end
      end

      outlineLayer:newCel(frame, dst, cel.position)
    end
  end
end)
```

---

## Recolor indexed sprite

Replace one palette index with another across all cels.

```lua
local from = 5  -- source index
local to   = 7  -- destination index

local sprite = app.activeSprite
if not sprite then
  app.alert("No sprite is open.")
  return
end
if sprite.colorMode ~= "indexed" then
  app.alert("This script requires an indexed-mode sprite.")
  return
end

local count = 0

app.transaction(function()
  for _, layer in ipairs(sprite.layers) do
    for _, frame in ipairs(sprite.frames) do
      local cel = layer:cel(frame)
      if cel then
        local img = cel.image:clone()
        for px in img:pixels() do
          if px().index == from then
            px(Color { index = to })
            count = count + 1
          end
        end
        -- Replace the cel's image with the modified copy.
        cel.image = img
      end
    end
  end
end)

app.alert(string.format("Replaced %d pixels (index %d → %d).", count, from, to))
```

---

## Posterize to reduced palette

Quantize the active sprite to N palette colors using a simple popularity algorithm.

```lua
local N = 8  -- target color count

local sprite = app.activeSprite
if not sprite then return end

-- Count per-color usage across all frames of the active layer.
local counts = {}
local layer  = app.activeLayer
for _, frame in ipairs(sprite.frames) do
  local cel = layer:cel(frame)
  if cel then
    for px in cel.image:pixels() do
      if px().alpha > 0 then
        local c   = px()
        local key = string.format("%d,%d,%d", c.red, c.green, c.blue)
        counts[key] = (counts[key] or 0) + 1
      end
    end
  end
end

-- Sort by frequency descending, take top N.
local sorted = {}
for k, v in pairs(counts) do table.insert(sorted, { key = k, n = v }) end
table.sort(sorted, function(a, b) return a.n > b.n end)

-- Build a new palette.
app.transaction(function()
  local pal = sprite.palette
  pal:resize(N)
  for i = 1, math.min(N, #sorted) do
    local r, g, b = sorted[i].key:match("(%d+),(%d+),(%d+)")
    pal:setColor(i - 1, Color { r = tonumber(r), g = tonumber(g), b = tonumber(b), a = 255 })
  end
end)

app.alert("Palette reduced to " .. N .. " colors.")
```

---

## Sprite-sheet metadata dump

Print metadata for every frame tag to the Pixhaus console.

```lua
local sprite = app.activeSprite
if not sprite then return end

app.log("=== " .. (sprite.filename or "untitled") .. " ===")
app.log(string.format("Size: %d × %d", sprite.width, sprite.height))
app.log(string.format("Frames: %d", #sprite.frames))
app.log(string.format("Tags: %d", #sprite.tags))
app.log("")

for _, tag in ipairs(sprite.tags) do
  app.log(string.format(
    "  [%s] frames %d–%d, dir=%s",
    tag.name,
    tag.fromFrame.frameNumber,
    tag.toFrame.frameNumber,
    tag.aniDir
  ))
end

app.log("")
for i, frame in ipairs(sprite.frames) do
  app.log(string.format("  frame %d: %dms", i, frame.duration))
end
```

---

## Aseprite compatibility probe

A small script that reports which Aseprite API calls work in the current Pixhaus version. Useful when porting an Aseprite script.

```lua
local results = {}

local function check(name, fn)
  local ok, err = pcall(fn)
  table.insert(results, string.format("  [%s] %s%s", ok and "ok" or "FAIL", name, ok and "" or (": " .. tostring(err))))
end

check("app.activeSprite",   function() return type(app.activeSprite) end)
check("app.fgColor.red",    function() return app.fgColor.red end)
check("Color{r,g,b}",       function() return Color{r=255,g=0,b=0} end)
check("app.fs.joinPath",    function() return app.fs.joinPath("/a", "b") end)
check("app.transaction",    function() app.transaction(function() end) end)
check("app.command.Undo",   function() return type(app.command.Undo) end)

app.log("Aseprite API compatibility probe:")
for _, line in ipairs(results) do app.log(line) end
```

---

## More examples

The full plugin source files are under `examples/plugins/` in the repository:

- `hello-command/` — minimal command registration
- `palette-export/` — full palette CSV exporter with file system permissions
- `grayscale-verb/` — classical AI verb (no backend, pure pixel math)
- `custom-verb-wasm/` — Rust-compiled WASM verb (invert colors)
