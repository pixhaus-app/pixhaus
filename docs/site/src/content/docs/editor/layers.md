---
title: Layers
description: Layer types, blend modes, groups, and the layer panel.
---

## Layer types

| Type | Description |
|---|---|
| Raster | Standard pixel layer. Stores RGBA pixel data. |
| Group | Contains child layers. Blend mode and opacity apply to the group composite. |
| Tilemap | Stores a 2D grid of tile indices referencing a tileset. Supports autotile rules and animated tiles. |

Add a layer with the `+` button in the layer panel or via `Layer > New layer`.

## Layer panel

The layer panel shows the layer stack from top (front) to bottom (back). Each row shows:
- **Thumbnail** — 32x32 live preview, updated within 100ms of a paint operation
- **Name** — double-click to rename
- **Blend mode** — dropdown selector (Normal, Multiply, Screen, Overlay, and more)
- **Opacity** — slider 0–255
- **Visibility toggle** — eye icon
- **Lock toggle** — lock icon (prevents edits)

Drag rows to reorder. Drag into a group to nest. Multi-select with `Shift+click` or `Ctrl+click`.

## Blend modes

Pixhaus supports the full Aseprite blend mode set:

Normal, Multiply, Screen, Overlay, Darken, Lighten, Color Dodge, Color Burn, Hard Light, Soft Light, Difference, Exclusion, Hue, Saturation, Color, Luminosity, Add, Subtract, Divide.

Blend math matches Aseprite byte-for-byte for `.aseprite` round-trips.

## Context menu operations

Right-click any layer row for:
- Rename
- Duplicate
- Delete
- Merge down
- Merge selected (when multiple layers are selected)
- Flatten visible
- Convert to group
- Convert to tilemap layer

All operations go through the undo stack and appear in the history panel.
