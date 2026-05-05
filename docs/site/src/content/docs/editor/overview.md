---
title: Editor overview
description: The Pixhaus editor layout and core concepts.
---

The Pixhaus editor follows the same panel layout as Aseprite: canvas in the center, tools on the left, layers and timeline on the right, palette at the bottom. If you know Aseprite, you are oriented.

## Layout

| Panel | Default location |
|---|---|
| Canvas | Center |
| Tool options | Top bar |
| Layers | Right |
| Timeline | Right (below layers) |
| Color / Palette | Bottom-left |
| Command palette (`Ctrl+K`) | Overlay |

Panel-toggle shortcuts (Layers / Timeline / Palette) are part of the planned brush + window stream and are not wired today; use the View menu in the meantime.

## Canvas

The canvas renders your sprite at the configured zoom level using a WebGL2 viewport. Pan with `Space+drag` or middle mouse. Zoom with the scroll wheel (anchored at cursor) or `+`/`-`.

Above 800% zoom the pixel grid appears automatically. Disable it in `View > Pixel grid`.

## Command palette

`Ctrl+K` (`Cmd+K` on macOS) opens a fuzzy-search overlay covering every command in the editor. Type any part of the command name — e.g., "inbetween" to find the AI verb, "merge" to find layer merge operations. Every menu item, tool, and AI verb is reachable here.

## Keybinds

Pixhaus ships two built-in presets:
- **Aseprite** — Aseprite-compatible defaults (currently the shipped default)
- **Photoshop** — Photoshop-compatible defaults

Load a preset at `Preferences > Keybinds`. Individual keys are remappable. See the [keyboard shortcuts reference](/reference/keybinds/).

## Projects

Pixhaus saves in the `.pixhaus` format — an open binary format (MessagePack + zstd compression). You can also open `.aseprite` files directly. See [file formats](/reference/file-formats/) for the full list.
