---
title: Keyboard shortcuts
description: Default keyboard shortcuts and how to customize them.
---

import { Aside } from "@astrojs/starlight/components";

<Aside type="caution" title="What ships today">
The shortcuts below mark which streams have wired the corresponding command. Entries flagged "planned" appear in the keybinds preset table but do not have a working handler yet — invoking them no-ops and logs a warning. The brush, layer-management, animation-playback, and selection streams will land them.
</Aside>

## Global (working today)

| Action | Windows/Linux | macOS |
|---|---|---|
| Command palette | `Ctrl+K` | `Cmd+K` |
| New project | `Ctrl+N` | `Cmd+N` |
| Open project | `Ctrl+O` | `Cmd+O` |
| Save | `Ctrl+S` | `Cmd+S` |
| Save as | `Ctrl+Shift+S` | `Cmd+Shift+S` |
| Undo | `Ctrl+Z` | `Cmd+Z` |
| Redo | `Ctrl+Shift+Z` | `Cmd+Shift+Z` |
| Preferences | `Ctrl+,` | `Cmd+,` |

## Window panels (working today)

| Action | Shortcut |
|---|---|
| Toggle layers panel | `Ctrl+Shift+L` |
| Toggle timeline panel | `Ctrl+Shift+T` |

## Tools (planned — wired by the brush stream)

| Tool | Shortcut |
|---|---|
| Pencil | `P` |
| Eraser | `E` |
| Line | `L` |
| Rectangle | `R` |
| Ellipse | `O` |
| Polygon | `Y` |
| Fill bucket | `G` |
| Eyedropper | `I` |
| Magic wand | `W` |
| Freehand lasso | `Q` |
| Move | `V` |

## Canvas

| Action | Shortcut |
|---|---|
| Zoom in | `+` or `Ctrl+=` |
| Zoom out | `-` or `Ctrl+-` |
| Fit to window | `Ctrl+0` |
| Zoom 100% | `Ctrl+1` |
| Zoom 200% | `Ctrl+2` |
| Pan | `Space+drag` or middle mouse drag |
| Toggle pixel grid | `Ctrl+G` |

## Color (planned — wired by the palette panel)

| Action | Shortcut |
|---|---|
| Swap fg/bg colors | `X` |
| Reset to black/white | `D` |

## Layers (planned — wired by the layer panel)

| Action | Shortcut |
|---|---|
| New raster layer | `Shift+Ctrl+N` |
| Toggle layer visibility | `Ctrl+Shift+H` (while layer selected) |

## Timeline (planned — wired by the animation timeline stream)

The shipped command palette has a single timeline entry (`window:toggle-timeline`); per-frame navigation, play/pause, and tag operations all land with the timeline panel stream.

## Selection (planned — wired by the selection UI stream)

| Action | Shortcut |
|---|---|
| Deselect | `Ctrl+D` |
| Select all | `Ctrl+A` |
| Invert selection | `Ctrl+Shift+I` |

## Customizing shortcuts

Open `Edit > Keybinds` to remap any shortcut. Two built-in presets are available:
- **Aseprite** — [Aseprite-compatible defaults](/reference/preset-aseprite/) (current default)
- **Photoshop** — Photoshop-compatible defaults

Load a preset, then modify individual bindings as needed. Presets do not overwrite customized bindings automatically.
