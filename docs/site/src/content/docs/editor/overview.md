---
title: Editor overview
description: The Pixhaus editor layout and core concepts.
---

The Pixhaus editor follows the same panel layout as Aseprite: canvas in the center, tools on the left, layers and timeline on the right, palette at the bottom. If you know Aseprite, you are oriented.

## Layout

| Panel | Default location | Shortcut |
|---|---|---|
| Canvas | Center | — |
| Tool options | Top bar | — |
| Layers | Right | `F7` |
| Timeline | Bottom | `Tab` |
| Color / Palette | Bottom-left | `F4` |
| Command palette | Overlay | `Ctrl+K` |

All panels are dockable and can be rearranged. `View > Reset layout` restores the default.

## Canvas

The canvas renders your sprite at the configured zoom level using a WebGL2 viewport. Pan with `Space+drag` or middle mouse. Zoom with the scroll wheel (anchored at cursor) or `+`/`-`.

Above 800% zoom the pixel grid appears automatically. Disable it in `View > Pixel grid`.

## Command palette

`Ctrl+K` (`Cmd+K` on macOS) opens a fuzzy-search overlay covering every command in the editor. Type any part of the command name — e.g., "inbetween" to find the AI verb, "merge" to find layer merge operations. Every menu item, tool, and AI verb is reachable here.

## Keybinds

Pixhaus ships two built-in presets:
- **Pixhaus default** — the native layout
- **Aseprite-compatible** — maps to Aseprite's defaults so existing muscle memory transfers

Load a preset at `Edit > Keybinds > Load preset`. Individual keys are remappable. See the [keyboard shortcuts reference](/reference/keybinds/).

## Projects

Pixhaus saves in the `.pixhaus` format — an open binary format (MessagePack + zstd compression). You can also open `.aseprite` files directly. See [file formats](/reference/file-formats/) for the full list.
