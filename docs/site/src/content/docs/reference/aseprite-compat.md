---
title: Aseprite compatibility
description: What round-trips cleanly between Pixhaus and Aseprite, and what doesn't.
---

Pixhaus reads and writes `.aseprite` files. This page documents the round-trip fidelity — what survives perfectly, what has caveats, and what is out of scope.

## What round-trips cleanly

| Feature | Notes |
|---|---|
| RGBA and indexed color modes | Exact |
| Layer hierarchy and groups | Exact |
| Blend modes (all 18 modes) | Exact — blend math matches Aseprite byte-for-byte |
| Opacity per layer and cel | Exact |
| Layer visibility and lock | Exact |
| Frame durations | Exact |
| Frame tags (name, range, loop direction) | Exact |
| Palette (up to 256 colors with names) | Exact |
| Cels (linked and unlinked) | Exact |
| Slices (named regions with pivot) | Exact |
| Tilemap layers (Aseprite 1.3+) | Read: exact. Write: Pixhaus tilemap chunks → Aseprite 1.3 tilemap format |
| User data (string + color per layer/cel) | Exact |

## What has caveats

| Feature | Status |
|---|---|
| Layer color labels | Read: mapped to Pixhaus layer color. Write: preserved as Aseprite chunk |
| Custom blend modes (beyond Aseprite's set) | Pixhaus layers with non-Aseprite blend modes save as Normal in `.aseprite` with a warning |
| 16-bit per channel sprites | Read: downsampled to 8-bit with warning. Write: not supported |
| 32-bit float sprites | Same as 16-bit |
| Grayscale mode | Read: converted to RGBA. Write: RGBA saved as RGBA; Aseprite will show it as RGBA |
| Color profile chunks | Preserved as opaque bytes; not interpreted |

## What is not supported

| Feature | Notes |
|---|---|
| External files (linked sprites) | Treated as embedded cels |
| Deprecated Aseprite palette chunks | Upgraded to the modern palette chunk on save |
| Undocumented proprietary chunks | Skipped on read; not emitted on write |

## Keybind compatibility

Pixhaus ships an Aseprite-compatible keybind preset (load via `Preferences > Keybinds > Aseprite`). The mapping covers the global, canvas, tool, and timeline operations Pixhaus has wired so far. The full table of working-today shortcuts lives in [Keyboard shortcuts](/reference/keybinds/) under the "Global" and "Canvas" sections.

### Known incompatibilities

Aseprite shortcuts Pixhaus does not yet bind by default — these will arrive with the streams that wire the corresponding feature:

| Aseprite shortcut | Action | Status |
|---|---|---|
| `B`, `E`, `M`, `Z`, `H`, `I`, `K`, `G` | Pencil / eraser / marquee / zoom / hand / eyedropper / shading / paint bucket | Planned: brush engine stream |
| `Tab` | Toggle timeline panel | Planned: animation timeline stream |
| `F1`–`F4` | Toggle layer / palette / preview / mini-editor panels | Planned: layer panel + palette panel + preview window streams |
| `Ctrl+Y` (redo) | Pixhaus uses `Ctrl+Shift+Z` instead | Intentional divergence — `Ctrl+Y` is also bound to redo as a fallback once the redo command lands |
| `Alt+N` | New frame | Planned: timeline stream |
| `Shift+F1` | Toggle onion skin | Planned: animation timeline stream |
| `Ctrl+Shift+M` | Sprite resize | Planned: canvas-edit stream |

Anything not on this list and not in the [Keyboard shortcuts](/reference/keybinds/) page is unbound today; remap any combo at `Preferences > Keybinds`.

## Scripting compatibility

The Lua scripting API mirrors Aseprite's `app` global. Common Aseprite community scripts should port with minimal changes. See [Lua API reference](/scripting/lua-api/) for known divergences.

## Reporting incompatibilities

Open a [GitHub issue](https://github.com/pixhaus-app/pixhaus/issues) with:
- The `.aseprite` file (or a minimal reproduction)
- The Aseprite version it was saved with
- What you expected vs what happened
