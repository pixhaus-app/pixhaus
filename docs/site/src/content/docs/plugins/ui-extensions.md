---
title: UI extensions
description: Register custom panels, drawing tools, and command palette entries from a plugin.
---

import { Aside } from "@astrojs/starlight/components";

Plugins can extend the editor's UI in three ways:

1. **Commands** — entries in the command palette and any menu
2. **Panels** — dockable side panels with their own widget tree
3. **Tools** — custom drawing tools in the toolbox

All three require the matching permission in `plugin.toml`.

---

## Commands

Commands are the lowest-friction extension point. They appear in the command palette (**Ctrl+K** / **Cmd+K**) and can be bound to keybindings by the user.

```toml
[permissions]
commands = true
```

```lua
app.commands.register {
  name    = "my-plugin:do-thing",
  label   = "My Plugin: Do Thing",

  -- Optional: show this command only when a condition is true.
  enabled = function()
    return app.activeSprite ~= nil
  end,

  execute = function()
    -- ... perform the action ...
  end,
}
```

### Naming

Command names follow a `<plugin-slug>:<action>` convention to avoid collisions. The slug is your plugin's folder name. Labels are shown to the user; names are used in keybind config files.

### Grouping commands in the menu

To add a command to a named menu group, pass a `menu` field:

```lua
app.commands.register {
  name    = "my-plugin:analyze",
  label   = "Analyze Pixels",
  menu    = "AI",           -- adds to the AI menu
  execute = function() ... end,
}
```

Valid menu values: `"File"`, `"Edit"`, `"Sprite"`, `"Frame"`, `"Layer"`, `"Select"`, `"View"`, `"AI"`, `"Window"`.

---

## Panels

Panels are dockable widgets that live alongside the layer panel, palette panel, etc. Declare `panels = true` in `[permissions]`.

```toml
[permissions]
panels = true
```

```lua
app.ui.panel {
  id    = "my-plugin:stats",
  title = "Sprite Stats",

  -- content() is called each time the panel redraws.
  -- `w` is the widget builder.
  content = function(w)
    local sprite = app.activeSprite
    if not sprite then
      w:label("No sprite open.")
      return
    end

    w:label(string.format("Size: %d × %d", sprite.width, sprite.height))
    w:label(string.format("Layers: %d", #sprite.layers))
    w:label(string.format("Frames: %d", #sprite.frames))

    w:separator()

    w:button("Export palette CSV", function()
      app.command["palette-export:export-csv"]()
    end)
  end,
}
```

### Widget builder API

| Call | Description |
|---|---|
| `w:label(text)` | Static text. |
| `w:button(label, fn)` | Clickable button. `fn` runs on click. |
| `w:separator()` | Horizontal rule. |
| `w:input(opts)` | Single-line text field. `opts.value`, `opts.on_change`. |
| `w:checkbox(label, opts)` | Boolean toggle. `opts.value`, `opts.on_change`. |
| `w:slider(opts)` | Integer or float slider. `opts.min`, `opts.max`, `opts.value`, `opts.on_change`. |
| `w:dropdown(opts)` | Select from a list. `opts.items` (array of strings), `opts.value`, `opts.on_change`. |
| `w:color_swatch(opts)` | Color picker swatch. `opts.color`, `opts.on_change`. |
| `w:image(opts)` | Render pixel data inline. `opts.pixels` (Image object), `opts.scale`. |
| `w:row(fn)` | Horizontal layout group. `fn` receives the same `w`. |
| `w:column(fn)` | Vertical layout group. |

<Aside>
Panel `content()` may be called multiple times per second. Keep it side-effect-free; build the widget tree from current state instead of storing mutable state in closures.
</Aside>

### Forcing a panel redraw

Panels redraw automatically when editor state changes. To trigger an explicit redraw (e.g., after an async operation):

```lua
app.ui.invalidatePanel("my-plugin:stats")
```

---

## Custom drawing tools

Custom tools integrate with the brush engine — they receive pointer events on the canvas and can read or write pixel data.

```toml
[permissions]
tools = true
```

```lua
app.tools.register {
  id    = "my-plugin:dot-grid",
  label = "Dot Grid",
  icon  = "dot-grid.png",   -- 16x16 PNG in the plugin folder

  -- Called once when the user activates the tool.
  activate = function(ctx)
    ctx.setCursor("crosshair")
  end,

  -- Called for every pointer-down event while the tool is active.
  stroke_begin = function(ctx, x, y)
    -- Snap to the nearest grid point (every 8 pixels).
    local gx = math.floor(x / 8) * 8
    local gy = math.floor(y / 8) * 8
    ctx.putPixel(gx, gy, app.fgColor)
  end,

  -- Called for pointer-move events (held down).
  stroke_move = function(ctx, x, y)
    local gx = math.floor(x / 8) * 8
    local gy = math.floor(y / 8) * 8
    ctx.putPixel(gx, gy, app.fgColor)
  end,

  -- Called when the pointer is released.  The accumulated stroke is
  -- committed as a single undo entry.
  stroke_end = function(ctx)
    ctx.commit()
  end,
}
```

### Tool context (`ctx`)

| Field / method | Description |
|---|---|
| `ctx.sprite` | Active sprite. |
| `ctx.layer` | Active layer. |
| `ctx.frame` | Active frame. |
| `ctx.putPixel(x, y, color)` | Write a pixel to the stroke buffer. |
| `ctx.getPixel(x, y)` | Read a pixel from the active layer. |
| `ctx.setCursor(name)` | Set the cursor shape: `"crosshair"`, `"default"`, `"none"`. |
| `ctx.commit()` | Finalize the stroke as one undo entry. |
| `ctx.cancel()` | Discard all `putPixel` calls since `stroke_begin`. |

<Aside type="caution">
Do not call `app.transaction` inside tool callbacks. The tool context manages the undo grouping. Calling `ctx.commit()` outside of `stroke_end` is valid and produces multiple undo entries per drag.
</Aside>

---

## Keybind registration

Register a default keybind that users can override in preferences:

```lua
app.commands.register {
  name     = "my-plugin:do-thing",
  label    = "My Plugin: Do Thing",
  shortcut = "ctrl+shift+g",    -- default shortcut (optional)
  execute  = function() ... end,
}
```

Users can rebind any command in **Edit > Preferences > Keyboard**. Your default is only used if the user has not customized that slot.

---

## See also

- [Developing plugins](/plugins/developing/) — command, panel, and verb basics
- [AI verb authoring](/plugins/ai-verb-authoring/) — build verbs that call inference backends
- [Lua API reference](/scripting/lua-api/) — full `app.*` reference
