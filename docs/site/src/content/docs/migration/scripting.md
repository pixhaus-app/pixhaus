---
title: Porting Aseprite scripts
description: Walk through three common Aseprite community scripts and the changes needed to run them in Pixhaus.
sidebar:
  order: 3
---

import { Aside } from "@astrojs/starlight/components";

<Aside type="caution">
The Lua scripting runtime lands with stream S38. This page documents the
planned API surface and the expected diffs for porting Aseprite scripts.
If something described here does not work yet, the S38 stream is where to
look.
</Aside>

Pixhaus implements the Aseprite Lua API at `app.*` and mirrors the global
structure so that existing scripts have a migration path. Most community
scripts need only a handful of changes — typically one renamed command and
minor API divergences on the UI side.

The [Lua API reference](/scripting/lua-api/) documents every supported
symbol. This page focuses on the porting deltas for three scripts that
represent common patterns.

## What needs no changes

Before looking at what differs, here is what works identically:

- `app.activeSprite`, `app.activeLayer`, `app.activeFrame`, `app.activeCel`
- `app.fgColor`, `app.bgColor`
- `Color { r, g, b, a }`, `Color { h, s, v }`, `Color { index = n }`
- `sprite.layers`, `sprite.frames`, `sprite.palette`
- `layer.name`, `layer.opacity`, `layer.blendMode`, `layer.isVisible`
- `frame.duration`
- `cel.image`, `cel.position`, `cel.opacity`
- `image:getPixel(x, y)`, `image:drawPixel(x, y, color)`
- `image:clear(color)`, `image.width`, `image.height`
- `Dialog { ... }:show()`, `dialog.data.*`
- `app.refresh()`

Most data-manipulation scripts use only these and need zero changes.

## Color Reduction

**What it does:** Prompts the user for a target color count, then quantizes
the active sprite down to that palette.

### Aseprite version

```lua
local sprite = app.activeSprite
if not sprite then return end

local dlg = Dialog("Color Reduction")
dlg:number{ id="colors", label="Colors:", text="16", min=2, max=256 }
dlg:button{ id="ok",     text="OK" }
dlg:button{ id="cancel", text="Cancel" }
dlg:show()
if not dlg.data.ok then return end

local n = math.max(2, math.min(256, dlg.data.colors))

app.command.ChangePixelFormat{
  format        = "indexed",
  dithering     = "ordered",
  ditherType    = "bayer8x8",
  factorHisto   = 100,
  factor        = 50,
}
```

### Pixhaus version

```lua
local sprite = app.activeSprite
if not sprite then return end

local dlg = Dialog("Color Reduction")
dlg:number{ id="colors", label="Colors:", text="16", min=2, max=256 }
dlg:button{ id="ok",     text="OK" }
dlg:button{ id="cancel", text="Cancel" }
dlg:show()
if not dlg.data.ok then return end

local n = math.max(2, math.min(256, dlg.data.colors))

-- Changed: ChangePixelFormat -> ChangeColorMode; dithering arg renamed
app.command.ChangeColorMode{
  mode      = "indexed",
  dithering = "bayer8x8",
  colors    = n,
}
```

**Changes:**
- `app.command.ChangePixelFormat` → `app.command.ChangeColorMode`
- `format = "indexed"` → `mode = "indexed"`
- `dithering = "ordered", ditherType = "bayer8x8"` → `dithering = "bayer8x8"`
- `factorHisto` and `factor` dropped (Pixhaus uses a single quantization
  pass rather than Aseprite's two-phase approach); the `colors` argument
  is now explicit

Everything else is identical.

---

## Outline

**What it does:** Adds a 1-pixel solid-color outline around every non-transparent
pixel in the active cel.

### Aseprite version

```lua
local sprite = app.activeSprite
if not sprite then return end
local cel = app.activeCel
if not cel then return end

local dlg = Dialog("Outline")
dlg:color{ id="color", label="Color:", color=app.fgColor }
dlg:button{ id="ok", text="OK" }
dlg:button{ id="cancel", text="Cancel" }
dlg:show()
if not dlg.data.ok then return end

local outlineColor = dlg.data.color
local src = cel.image
local w, h = src.width, src.height

-- Create a new image one pixel larger on each side
local dst = Image(w + 2, h + 2, src.colorMode)
dst:clear(Color{ a=0 })

-- Copy source shifted by (1, 1)
for y = 0, h - 1 do
  for x = 0, w - 1 do
    local c = src:getPixel(x, y)
    if app.pixelColor.rgbaA(c) > 0 then
      -- Paint outline in the 4 cardinal neighbours
      for _, d in ipairs({{0,-1},{0,1},{-1,0},{1,0}}) do
        local nx, ny = x + 1 + d[1], y + 1 + d[2]
        if dst:getPixel(nx, ny) == 0 then
          dst:drawPixel(nx, ny, outlineColor)
        end
      end
    end
  end
end

-- Copy original on top
for y = 0, h - 1 do
  for x = 0, w - 1 do
    local c = src:getPixel(x, y)
    if app.pixelColor.rgbaA(c) > 0 then
      dst:drawPixel(x + 1, y + 1, c)
    end
  end
end

app.transaction("Outline", function()
  cel.image:resize(w + 2, h + 2)
  cel.image:clear(Color{ a=0 })
  cel.image:drawImage(dst, 0, 0)
  cel.position = { x = cel.position.x - 1, y = cel.position.y - 1 }
end)

app.refresh()
```

### Pixhaus version

```lua
local sprite = app.activeSprite
if not sprite then return end
local cel = app.activeCel
if not cel then return end

local dlg = Dialog("Outline")
dlg:color{ id="color", label="Color:", color=app.fgColor }
dlg:button{ id="ok", text="OK" }
dlg:button{ id="cancel", text="Cancel" }
dlg:show()
if not dlg.data.ok then return end

local outlineColor = dlg.data.color
local src = cel.image
local w, h = src.width, src.height

local dst = Image(w + 2, h + 2, src.colorMode)
dst:clear(Color{ a=0 })

for y = 0, h - 1 do
  for x = 0, w - 1 do
    local c = src:getPixel(x, y)
    if app.pixelColor.rgbaA(c) > 0 then
      for _, d in ipairs({{0,-1},{0,1},{-1,0},{1,0}}) do
        local nx, ny = x + 1 + d[1], y + 1 + d[2]
        if dst:getPixel(nx, ny) == 0 then
          dst:drawPixel(nx, ny, outlineColor)
        end
      end
    end
  end
end

for y = 0, h - 1 do
  for x = 0, w - 1 do
    local c = src:getPixel(x, y)
    if app.pixelColor.rgbaA(c) > 0 then
      dst:drawPixel(x + 1, y + 1, c)
    end
  end
end

-- Changed: app.transaction takes a label as the first argument in both editors,
-- but Pixhaus exposes the label via a named key
app.transaction{ label="Outline", function()
  cel.image:resize(w + 2, h + 2)
  cel.image:clear(Color{ a=0 })
  cel.image:drawImage(dst, 0, 0)
  cel.position = { x = cel.position.x - 1, y = cel.position.y - 1 }
end }

app.refresh()
```

**Changes:**
- `app.transaction("Outline", fn)` → `app.transaction{ label="Outline", fn }`

  Pixhaus uses a table argument instead of positional arguments. If your
  Aseprite script passes the label as a bare string, change it to a named
  key. If it omits the label (`app.transaction(fn)`), both editors accept
  that form unchanged.

The pixel manipulation, cel image access, and position manipulation are
identical.

---

## Sprite Sheet Generator

**What it does:** Exports all frames of the active sprite as a horizontal
strip PNG, naming each file by the active tag.

### Aseprite version

```lua
local sprite = app.activeSprite
if not sprite then return end

local dlg = Dialog("Export Sprite Sheet")
dlg:file{
  id       = "filename",
  label    = "Output file:",
  save     = true,
  filetypes= { "png" },
}
dlg:button{ id="ok",     text="Export" }
dlg:button{ id="cancel", text="Cancel" }
dlg:show()
if not dlg.data.ok then return end

app.command.ExportSpriteSheet{
  ui         = false,
  type       = SpriteSheetType.HORIZONTAL,
  textureFilename = dlg.data.filename,
  dataFilename    = dlg.data.filename:gsub("%.png$", ".json"),
  dataFormat      = SpriteSheetDataFormat.JSON_ARRAY,
  tag             = "",          -- all tags
  trim            = false,
  padding         = 0,
}
```

### Pixhaus version

```lua
local sprite = app.activeSprite
if not sprite then return end

local dlg = Dialog("Export Sprite Sheet")
dlg:file{
  id        = "filename",
  label     = "Output file:",
  save      = true,
  filetypes = { "png" },
}
dlg:button{ id="ok",     text="Export" }
dlg:button{ id="cancel", text="Cancel" }
dlg:show()
if not dlg.data.ok then return end

-- Changed: ExportSpriteSheet -> ExportPng; argument shape differs
app.command.ExportPng{
  path        = dlg.data.filename,
  layout      = "horizontal",          -- "horizontal" | "vertical" | "packed"
  dataFormat  = "json-array",          -- matches Aseprite JSON output
  tag         = nil,                   -- nil = all tags
  trim        = false,
  padding     = 0,
}
```

**Changes:**
- `app.command.ExportSpriteSheet` → `app.command.ExportPng`
- `type = SpriteSheetType.HORIZONTAL` → `layout = "horizontal"` (string
  instead of enum constant)
- `textureFilename` → `path` (Pixhaus derives the JSON path automatically
  as `<path>.json`; `dataFilename` is not needed)
- `dataFormat = SpriteSheetDataFormat.JSON_ARRAY` → `dataFormat =
  "json-array"` (string instead of enum constant)
- `ui = false` dropped (Pixhaus commands never show a separate export dialog
  when invoked from Lua)

The JSON output format is identical to Aseprite's JSON array format, so the
Unity importer and any downstream tooling that reads Aseprite JSON works
without changes.

---

## General porting checklist

When porting any Aseprite script:

1. **Check enum constants.** Aseprite exposes constants like
   `SpriteSheetType.HORIZONTAL` and `BlendMode.MULTIPLY`. Pixhaus uses
   lowercase strings (`"horizontal"`, `"multiply"`). Grep your script for
   all-caps identifiers and replace them.

2. **Check `app.command.*` names.** The name may have changed slightly
   (e.g., `ChangePixelFormat` → `ChangeColorMode`, `ExportSpriteSheet` →
   `ExportPng`). Open the Lua API reference for the current mapping.

3. **Check `app.transaction` call shape.** If you pass a label, use the
   table form `{ label="…", fn }`.

4. **Check UI widget names.** Dialog widget types (`number`, `color`, `file`,
   `button`, `check`, `radio`, `slider`, `entry`) are the same. Widget
   option keys may differ; check any that fail against the Lua API reference.

5. **Remove `app.command.Refresh` calls.** Pixhaus refreshes automatically
   after each command and transaction. Explicit refresh calls are no-ops and
   can be removed, but they do not break anything.

If a command you rely on is missing from the API reference, open an issue
with the script name and command. The Lua surface grows as streams land;
common missing entries get prioritized.
