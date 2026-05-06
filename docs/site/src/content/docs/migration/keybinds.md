---
title: Keybind comparison
description: Side-by-side table of Aseprite defaults versus Pixhaus defaults, and where they intentionally diverge.
sidebar:
  order: 2
---

import { Aside } from "@astrojs/starlight/components";

The Aseprite keybind preset is the Pixhaus default. Most bindings are
identical. This page documents the complete mapping, the two intentional
divergences, and the shortcuts not yet wired.

<Aside type="tip">
Load a keybind preset at `Edit > Keybinds > Presets`. The Aseprite preset
is active by default. A Photoshop-compatible preset is also available.
Presets are starting points — you can remap any individual binding on top.
</Aside>

## File

| Action | Aseprite | Pixhaus |
|---|---|---|
| New | `Ctrl+N` | `Ctrl+N` |
| Open | `Ctrl+O` | `Ctrl+O` |
| Save | `Ctrl+S` | `Ctrl+S` |
| Save as | `Ctrl+Shift+S` | `Ctrl+Shift+S` |
| Close | `Ctrl+W` | `Ctrl+W` |
| Export | `Ctrl+Alt+Shift+S` | `Ctrl+Alt+Shift+S` |

## Edit

| Action | Aseprite | Pixhaus | Notes |
|---|---|---|---|
| Undo | `Ctrl+Z` | `Ctrl+Z` | |
| Redo | `Ctrl+Y` | `Ctrl+Shift+Z` | Intentional divergence — see below |
| Cut | `Ctrl+X` | `Ctrl+X` | Planned: selection stream |
| Copy | `Ctrl+C` | `Ctrl+C` | Planned: selection stream |
| Paste | `Ctrl+V` | `Ctrl+V` | Planned: selection stream |
| Paste in place | `Ctrl+Shift+V` | `Ctrl+Shift+V` | Planned: selection stream |
| Preferences | `Ctrl+,` | `Ctrl+,` | |

## Tools

| Tool | Aseprite | Pixhaus | Notes |
|---|---|---|---|
| Pencil | `B` | `B` | Planned: brush engine stream |
| Eraser | `E` | `E` | Planned: brush engine stream |
| Eyedropper | `I` | `I` | Planned: brush engine stream |
| Fill bucket | `G` | `G` | Planned: brush engine stream |
| Line | `L` | `L` | Planned: brush engine stream |
| Rectangle | `R` | `R` | Planned: brush engine stream |
| Ellipse | `O` | `O` | Planned: brush engine stream |
| Contour | `D` | `D` | Planned: brush engine stream |
| Shading | `K` | `K` | Planned: brush engine stream |
| Marquee select | `M` | `M` | Planned: selection stream |
| Freehand lasso | `Q` | `Q` | Planned: selection stream |
| Magic wand | `W` | `W` | Planned: selection stream |
| Move | `V` | `V` | Planned: selection stream |
| Hand (pan) | `H` | `Space+drag` | `H` also works as a toggle |
| Zoom | `Z` | `Z` | Planned: canvas stream |

## Canvas

| Action | Aseprite | Pixhaus | Notes |
|---|---|---|---|
| Zoom in | `Ctrl+=` or `+` | `Ctrl+=` | |
| Zoom out | `Ctrl+-` or `-` | `Ctrl+-` | |
| Fit to window | `Ctrl+0` | `Ctrl+0` | |
| Zoom 100% | `Ctrl+1` | `Ctrl+1` | |
| Zoom 200% | `Ctrl+2` | `Ctrl+2` | |
| Pan | `Space+drag` | `Space+drag` | |
| Toggle pixel grid | `Ctrl+G` | `Ctrl+G` | |
| Toggle onion skin | `Shift+F1` | `Shift+F1` | Planned: timeline stream |
| Sprite properties | `Ctrl+P` | `Ctrl+P` | Planned |
| Canvas resize | `Ctrl+Shift+M` | `Ctrl+Shift+M` | Planned |

## Color

| Action | Aseprite | Pixhaus | Notes |
|---|---|---|---|
| Swap fg/bg | `X` | `X` | Planned: palette stream |
| Reset to black/white | `D` | `D` | Planned: palette stream |

## Select

| Action | Aseprite | Pixhaus | Notes |
|---|---|---|---|
| Select all | `Ctrl+A` | `Ctrl+A` | Planned: selection stream |
| Deselect | `Ctrl+D` | `Ctrl+D` | Planned: selection stream |
| Invert selection | `Ctrl+Shift+I` | `Ctrl+Shift+I` | Planned: selection stream |

## Layers

| Action | Aseprite | Pixhaus | Notes |
|---|---|---|---|
| New layer | `Shift+Ctrl+N` | `Shift+Ctrl+N` | Working |
| Toggle visibility | `Ctrl+Shift+H` | `Ctrl+Shift+H` | Working |
| Toggle panels (F7) | `F7` | `Ctrl+Shift+L` | Intentional divergence — see below |

## Timeline and animation

| Action | Aseprite | Pixhaus | Notes |
|---|---|---|---|
| Toggle timeline | `Tab` | `Ctrl+Shift+T` | Working |
| Play / pause | `Enter` | `Enter` | Planned: timeline stream |
| Stop | `Esc` | `Esc` | Planned: timeline stream |
| Next frame | `Right` | `Right` | Planned: timeline stream |
| Previous frame | `Left` | `Left` | Planned: timeline stream |
| First frame | `Home` | `Home` | Planned: timeline stream |
| Last frame | `End` | `End` | Planned: timeline stream |
| New frame | `Alt+N` | `Alt+N` | Planned: timeline stream |
| Delete frame | `Alt+D` | `Alt+D` | Planned: timeline stream |
| Duplicate frame | `Alt+J` | `Alt+J` | Planned: timeline stream |

## Window

| Action | Aseprite | Pixhaus |
|---|---|---|
| Command palette | — | `Ctrl+K` |
| Preferences | `Ctrl+,` | `Ctrl+,` |
| Toggle layers panel | `F7` | `Ctrl+Shift+L` |
| Toggle timeline | `Tab` | `Ctrl+Shift+T` |

## Intentional divergences

Two shortcuts are permanently different from Aseprite. These are not gaps
— Pixhaus will not adopt the Aseprite binding even after the feature ships.

### Redo: Ctrl+Y → Ctrl+Shift+Z

Aseprite uses `Ctrl+Y` for redo, which is the Excel convention. Most
desktop applications (VS Code, browsers, Figma, GIMP, Krita) use
`Ctrl+Shift+Z`. Pixhaus follows the majority convention so that switching
between tools during a work session does not require mental context-switching
on a command you invoke constantly.

### Layers panel: F7 → Ctrl+Shift+L

Aseprite binds `F7` to toggle the layers panel. Pixhaus binds all panel
toggles as `Ctrl+Shift+*` — layers (`L`), timeline (`T`), and future panels
follow the same pattern. Pressing `F7` no-ops and shows a hint pointing to
the new binding.

If you prefer `F7`, remap it at `Edit > Keybinds`.
