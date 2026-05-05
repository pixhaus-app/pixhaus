---
title: Aseprite keybind preset
description: Full mapping table for the built-in Aseprite-compatible keybind preset, and where it diverges from Aseprite.
sidebar:
  order: 3
---

import { Aside } from "@astrojs/starlight/components";

The Aseprite preset maps Pixhaus commands to the shortcuts Aseprite users already know. It is the default preset when you first launch Pixhaus. Load it at any time via `Edit > Keybinds > Presets > Aseprite`.

<Aside type="caution" title="Not every shortcut is wired yet">
Entries marked "planned" exist in the preset table so the binding is reserved, but the underlying command has no handler today. Invoking them logs a warning and no-ops. Each one lands with the stream that wires the corresponding feature.
</Aside>

## Full mapping

### File

| Shortcut | Command |
|---|---|
| `Ctrl+N` | New project |
| `Ctrl+O` | Open project |
| `Ctrl+S` | Save |
| `Ctrl+Shift+S` | Save as |
| `Ctrl+W` | Close |

### Edit

| Shortcut | Command | Status |
|---|---|---|
| `Ctrl+Z` | Undo | Working |
| `Ctrl+Shift+Z` | Redo | Working |
| `Ctrl+X` | Cut | Planned: selection stream |
| `Ctrl+C` | Copy | Planned: selection stream |
| `Ctrl+V` | Paste | Planned: selection stream |
| `Ctrl+A` | Select all | Planned: selection stream |
| `Ctrl+D` | Deselect | Planned: selection stream |

### Select

| Shortcut | Command | Status |
|---|---|---|
| `Ctrl+Shift+I` | Invert selection | Planned: selection stream |

### View / canvas

| Shortcut | Command | Status |
|---|---|---|
| `Ctrl+=` | Zoom in | Working |
| `Ctrl+-` | Zoom out | Working |
| `Ctrl+0` | Fit to window | Working |
| `Ctrl+1` | Zoom 100% | Working |
| `Ctrl+G` | Toggle pixel grid | Working |

### Window

| Shortcut | Command | Status |
|---|---|---|
| `Ctrl+K` | Command palette | Working |
| `Ctrl+,` | Preferences | Working |
| `Ctrl+Shift+L` | Toggle layers panel | Working |
| `Ctrl+Shift+T` | Toggle timeline panel | Working |

## Where this preset diverges from Aseprite

These are intentional differences, not gaps. Pixhaus will not adopt the Aseprite binding even after the feature ships:

| Aseprite shortcut | Aseprite action | Pixhaus binding |
|---|---|---|
| `Ctrl+Y` | Redo | `Ctrl+Shift+Z` (matches most other desktop apps) |
| `F7` | Toggle layers panel | `Ctrl+Shift+L` (panel-toggle family is `Ctrl+Shift+*`) |

## Aseprite shortcuts not yet bound

These Aseprite shortcuts have no mapping in this preset today. They will be added as the relevant streams land:

| Aseprite shortcut | Action | Arrives with |
|---|---|---|
| `B` / `E` / `M` / `Z` / `H` / `I` / `K` / `G` | Tool keys (pencil / eraser / marquee / zoom / hand / eyedropper / shading / paint bucket) | Brush engine stream |
| `Tab` | Toggle timeline panel | Animation timeline stream |
| `F1`–`F4` | Toggle layer / palette / preview / mini-editor panels | Layer panel + palette panel streams |
| `X` | Swap foreground / background colors | Palette panel stream |
| `D` | Reset to black / white | Palette panel stream |
| `Alt+N` | New frame | Animation timeline stream |
| `Shift+F1` | Toggle onion skin | Animation timeline stream |
| `Ctrl+Shift+M` | Sprite resize | Canvas-edit stream |
| `Shift+Ctrl+N` | New layer | Layer panel stream |
| `Ctrl+Shift+H` | Toggle layer visibility | Layer panel stream |

Any shortcut not in the tables above is unbound. You can assign it yourself at `Edit > Keybinds`.
