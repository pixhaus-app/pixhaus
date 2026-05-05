---
title: Selection
description: Selection tools, modifiers, and boolean operations.
---

## Selection tools

| Tool | Shortcut | Description |
|---|---|---|
| Rectangular marquee | `M` | Click-drag for a rectangular selection. |
| Elliptical marquee | `M` twice | Click-drag for an ellipse selection. |
| Freehand lasso | `Q` | Click-drag to draw a freehand selection boundary. |
| Magic wand | `W` | Click a pixel to select all connected pixels within tolerance. |
| Color range | — | `Select > Color range` — select all pixels matching a color ± tolerance across the whole layer. |

## Modifiers

Hold modifier keys while using any selection tool to combine selections:

| Modifier | Effect |
|---|---|
| `Shift` | Add to selection (union) |
| `Alt` | Subtract from selection |
| `Shift+Alt` | Intersect with selection |

## Boolean operations

`Select` menu provides:
- **Invert** (`Ctrl+Shift+I`) — selects everything not currently selected
- **Expand** — grow the selection by N pixels
- **Contract** — shrink the selection by N pixels
- **Feather** — soft-edge the selection by a radius (less common for pixel art)

## Magic wand tolerance

The magic wand's tolerance (0–255) controls how different adjacent pixel colors must be before the flood fill stops. A tolerance of 0 selects only pixels that exactly match the clicked color. Connectivity mode (4-connected or 8-connected) controls whether diagonals count as neighbors.

## Deselecting

Press `Ctrl+D` or `Select > Deselect` to remove the active selection.

## Selection and transforms

With an active selection, the transform handles appear automatically. Drag the canvas inside the selection to move the selected pixels. Drag corner handles to scale; drag the rotation handle to rotate (uses RotSprite algorithm for pixel art quality). See [Transforms](/editor/transforms/) for details.
