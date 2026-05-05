---
title: Tools and brushes
description: Drawing tools, tool options, and custom brushes.
---

## Drawing tools

| Tool | Shortcut | Description |
|---|---|---|
| Pencil | `P` | 1-pixel strokes by default; configurable size. Pixel-perfect mode removes doubled corner pixels. |
| Eraser | `E` | Erases to transparent (RGBA mode) or to background color (indexed mode). |
| Line | `L` | Click-drag to draw a straight line. Shift-constrains to 45-degree angles. |
| Rectangle | `R` | Click-drag for a rectangle outline. Shift for square. Alt for filled. |
| Ellipse | `O` | Click-drag for an ellipse. Same modifiers as rectangle. |
| Polygon | `Y` | Click to place vertices; double-click or Enter to close. |
| Fill bucket | `G` | Flood-fill with contiguous (default) or global mode. Configurable tolerance. |
| Dither brush | `D` | Alternates between foreground and background colors in a dither pattern (50/50 checker by default). |
| Pattern stamp | — | Load any PNG as a brush stamp. Accessible via `Brush > Load custom brush`. |
| Eyedropper | `I` | Click to pick the pixel color under the cursor as the foreground color. |
| Gradient | — | Linear or radial gradient between foreground and background. |

## Tool options

The tool options bar appears below the menu bar when a drawing tool is active. Options vary per tool — for pencil: size (1–64), pixel-perfect mode toggle, symmetry mode.

## Symmetry

The pencil and other drawing tools support symmetry:
- **Horizontal** — mirror strokes left/right across the canvas center
- **Vertical** — mirror strokes top/bottom
- **Both** — 4-way mirror

Enable in the tool options bar while a drawing tool is selected.

## Custom brushes

Load any image as a brush stamp via `Brush > Load custom brush`. The loaded image is used as a stamp that follows the cursor. Options: rotation, mirroring, color tint (applies the foreground color to the stamp's hue).

## Pixel-perfect mode

When active, the pencil automatically removes "doubled" pixels at corners of diagonal strokes — the same behavior as Aseprite's "Pixel-perfect strokes" option. Recommended on at 1-pixel brush size.
