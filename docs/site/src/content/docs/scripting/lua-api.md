---
title: Lua API reference
description: Complete reference for the Pixhaus Lua scripting API — the app global and all its members.
---

import { Aside } from "@astrojs/starlight/components";

Pixhaus embeds Lua 5.4 via the `mlua` crate. The API mirrors Aseprite's `app` global where possible so existing Aseprite scripts have a migration path; Pixhaus-specific extensions are marked **[Pixhaus]**.

---

## `app` — global entry point

### Editor state

```lua
app.activeSprite    -- Sprite or nil
app.activeLayer     -- Layer or nil
app.activeFrame     -- Frame or nil
app.activeCel       -- Cel or nil
app.fgColor         -- Color (foreground)
app.bgColor         -- Color (background)
app.tool            -- string — active tool name
app.brush           -- Brush
app.editor          -- Editor **[Pixhaus]**
```

### Dialogs and notifications

```lua
app.alert(message)
-- Show a modal message box.  Returns nil.

app.alert { title = "Warning", message = "...", buttons = {"OK", "Cancel"} }
-- Show a modal with custom buttons.  Returns the index of the clicked button.

app.prompt(question)
-- Show a text-input dialog.  Returns the string the user typed, or nil if cancelled.
```

### Transactions

Wrap mutations in a transaction to group them as a single undo entry:

```lua
app.transaction(function()
  sprite:newLayer()
  sprite:newLayer()
end)
-- Both layer creations appear as one undo step.
```

Nested transactions are flattened into the outermost one.

### File system **[Pixhaus]**

```lua
app.fs.userDocsPath           -- string: ~/Documents
app.fs.userHomePath           -- string: ~/
app.fs.appDataPath            -- string: ~/.pixhaus
app.fs.joinPath(a, b, ...)    -- string: path joined with the OS separator
app.fs.exists(path)           -- boolean
app.fs.isFile(path)           -- boolean
app.fs.isDirectory(path)      -- boolean
app.fs.listFiles(dir)         -- array of filename strings (not full paths)
```

---

## `app.command.*`

Call a built-in editor command:

```lua
app.command.NewSprite()
app.command.OpenFile()
app.command.SaveFile()
app.command.SaveFileAs()
app.command.Undo()
app.command.Redo()
app.command.Cut()
app.command.Copy()
app.command.Paste()
app.command.Crop()
app.command.FlattenLayers()
app.command.NewLayer()
app.command.RemoveLayer()
app.command.DuplicateLayer()
app.command.MergeDownLayer()
app.command.NewFrame()
app.command.RemoveFrame()
app.command.DuplicateFrame()
app.command.PlayAnimation()
app.command.StopAnimation()
app.command.ZoomIn()
app.command.ZoomOut()
app.command.FitScreen()
```

Commands also accept an optional table of parameters (where the command supports them):

```lua
app.command.Resize { width = 64, height = 64, method = "nearest" }
```

---

## `app.commands` **[Pixhaus]**

Plugin command registration. See [UI extensions](/plugins/ui-extensions/).

```lua
app.commands.register {
  name     = "my-plugin:name",   -- string: unique identifier
  label    = "My Plugin: Name",  -- string: shown in palette
  shortcut = "ctrl+shift+g",     -- string: default keybind (optional)
  enabled  = function() ... end, -- function: return bool (optional)
  execute  = function() ... end, -- function: called on activation
}

app.commands.unregister("my-plugin:name")

-- Call a registered command by name:
app.commands["my-plugin:name"]()
```

---

## `app.ui` **[Pixhaus]**

Plugin panel registration. See [UI extensions](/plugins/ui-extensions/).

```lua
app.ui.panel {
  id      = "my-plugin:panel-id",  -- string: unique identifier
  title   = "Panel Title",
  content = function(w) ... end,
}

app.ui.invalidatePanel("my-plugin:panel-id")
-- Force the panel to redraw.
```

---

## `app.ai` **[Pixhaus]**

AI verb registration and invocation. See [AI verb authoring](/plugins/ai-verb-authoring/).

```lua
app.ai.registerVerb {
  id           = "com.example.verb-id",
  label        = "Verb Label",
  description  = "...",
  cost         = { credits = 0 },
  streaming    = false,
  cancellable  = false,
  input_schema = { ... },   -- JSON Schema table (optional)
  run          = function(ctx, progress, cancel, inputs) ... end,
}

app.ai.findVerb("com.example.verb-id")
-- Returns the registered verb table, or nil.

app.ai.makeTestContext { spriteWidth = 32, spriteHeight = 32 }
-- Build a VerbContext for testing without an open sprite.
```

---

## `app.tools` **[Pixhaus]**

Custom drawing tool registration. See [UI extensions](/plugins/ui-extensions/).

```lua
app.tools.register {
  id           = "my-plugin:tool-id",
  label        = "Tool Label",
  icon         = "icon.png",
  activate     = function(ctx) ... end,
  stroke_begin = function(ctx, x, y) ... end,
  stroke_move  = function(ctx, x, y) ... end,
  stroke_end   = function(ctx) ... end,
}
```

---

## Sprite

```lua
local sprite = app.activeSprite  -- Sprite or nil

sprite.width          -- number (read-only; use Resize command to change)
sprite.height         -- number (read-only)
sprite.colorMode      -- "rgb" or "indexed" (read-only)
sprite.filename       -- string or nil

sprite.layers         -- array of Layer
sprite.frames         -- array of Frame
sprite.tags           -- array of Tag
sprite.palette        -- Palette

-- Create / remove layers
sprite:newLayer()                 -- returns Layer
sprite:newGroup()                 -- returns Layer (group)
sprite:newTilemapLayer()          -- returns Layer (tilemap) [Pixhaus]
sprite:deleteLayer(layer)

-- Create / remove frames
sprite:newEmptyFrame(after_frame)
sprite:deleteFrame(frame)

-- Flatten
sprite:flatten()
```

---

## Layer

```lua
layer.name            -- string (writable)
layer.opacity         -- 0–255 (writable)
layer.blendMode       -- string (writable): "normal", "multiply", "screen", "overlay", ...
layer.isVisible       -- boolean (writable)
layer.isLocked        -- boolean (writable)
layer.isGroup         -- boolean (read-only)
layer.isTilemap       -- boolean (read-only) [Pixhaus]
layer.parent          -- Layer (group) or Sprite

-- Access cels
layer:cel(frame)      -- Cel or nil
layer:cels()          -- array of all Cel on this layer
```

---

## Frame

```lua
frame.frameNumber     -- 1-based index (read-only)
frame.duration        -- number in milliseconds (writable)
```

---

## Cel

```lua
cel.layer             -- Layer
cel.frame             -- Frame
cel.image             -- Image
cel.position          -- Point {x, y}
cel.opacity           -- 0–255 (writable)

cel.image             -- Image (read-only snapshot; clone before modifying)
```

---

## Image

```lua
local img = cel.image

img.width             -- number
img.height            -- number
img.colorMode         -- "rgb" or "indexed"

img:clone()           -- returns a mutable copy
img:getPixel(x, y)    -- Color (at pixel coordinates)
img:putPixel(x, y, c) -- set pixel (only on mutable copies)
img:pixels()          -- iterator of pixel handles (see below)
img:clear(color)      -- fill image with color
img:drawImage(src, x, y)  -- blit src onto this image at (x, y)
```

### Pixel iterator

```lua
for px in img:pixels() do
  local c = px()          -- get Color at this pixel
  px(new_color)           -- set Color at this pixel
  -- px.x, px.y           -- position
end
```

---

## Color

```lua
-- Constructors
Color { r=255, g=128, b=0, a=255 }   -- RGBA
Color { index=5 }                    -- indexed
Color { h=30, s=1.0, v=1.0 }        -- HSV (a defaults to 255)
Color { h=30, s=0.5, l=0.5 }        -- HSL

-- Fields (all writable)
c.red, c.green, c.blue, c.alpha     -- 0–255
c.hsvHue, c.hsvSaturation, c.hsvValue
c.hslHue, c.hslSaturation, c.hslLightness
c.index                             -- palette index (indexed mode)

-- Named colors
c.name                              -- string or nil [Pixhaus]
```

---

## Palette

```lua
local pal = sprite.palette

#pal                          -- palette size
pal:getColor(index)           -- Color at 0-based index
pal:setColor(index, color)    -- set color (inside a transaction)
pal:resize(size)              -- resize (inside a transaction)
pal:sort(comparator)          -- sort; comparator fn(a, b) returns bool [Pixhaus]
pal:load(filename)            -- load from .gpl / .pal / .aco / .hex file [Pixhaus]
pal:save(filename)            -- save to file [Pixhaus]
```

---

## Point and Size

```lua
Point { x=10, y=20 }
p.x, p.y

Size { width=32, height=32 }
s.width, s.height

Rectangle { x=0, y=0, width=32, height=32 }
r.x, r.y, r.width, r.height
r:contains(point)   -- boolean
r:intersects(rect)  -- boolean
```

---

## Tag (frame tag)

```lua
tag.name          -- string (writable)
tag.fromFrame     -- Frame
tag.toFrame       -- Frame
tag.aniDir        -- "forward", "reverse", "pingpong", "once"
tag.color         -- Color (label color in the timeline)
```

---

## Brush

```lua
app.brush
brush.type        -- "pixel", "circle", "square", "custom"
brush.size        -- number (writable)
brush.angle       -- number in degrees (writable)
brush.pattern     -- "none", "checker", "stripes" [Pixhaus]
brush.image       -- Image or nil (for custom brushes) [Pixhaus]
```

---

## Selection **[Pixhaus]**

```lua
local sel = sprite.selection  -- Selection or nil

sel:isEmpty()                -- boolean
sel:contains(x, y)           -- boolean
sel.bounds                   -- Rectangle
sel:clear()
sel:selectAll()
sel:invert()
sel:expand(pixels)
sel:contract(pixels)
sel:addRect(rect)
sel:subtractRect(rect)
```

---

## Aseprite compatibility notes

The Pixhaus Lua API implements the Aseprite scripting API documented at https://github.com/aseprite/api. Known differences:

| Feature | Aseprite | Pixhaus |
|---|---|---|
| `app.sprite` | alias for `app.activeSprite` | same |
| `app.sprites` | array of all open sprites | same |
| `app.open(filename)` | open file, return sprite | same |
| `Sprite:saveAs(filename)` | save to path | same |
| `app.command.*` | 100+ built-in commands | matched; see the IPC command catalog for the full list |
| `Image:resize` | resize in-place | **not yet implemented**; use `app.command.Resize` |
| `Dialog` API | rich dialog widgets | **partial**; `app.ui.panel` covers panels; modal dialogs are planned |
| `app.range` | timeline selection | **not yet implemented** |
| Tilemap APIs | partial in 1.3 | full `Layer.isTilemap`, `TilemapLayer.*` — see below |

Scripts that use the `Dialog` API for tool options will need to be ported to `app.ui.panel`.

---

## Error handling

Errors inside plugin callbacks are caught by the host and displayed in the plugin error log (**Edit > Plugins > Show errors**). They do not crash the editor. Use `error(message)` to raise an intentional error:

```lua
execute = function()
  local sprite = app.activeSprite
  if not sprite then
    error("No sprite is open.")  -- shown in the plugin error log
  end
  -- ...
end
```

To log a message without raising an error:

```lua
app.log("Loaded " .. count .. " colors.")   -- info
app.log.warn("Palette is empty.")           -- [Pixhaus]
app.log.error("File not found: " .. path)  -- [Pixhaus]
```
