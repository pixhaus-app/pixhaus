---
title: Your first sprite
description: Draw a simple sprite from scratch in Pixhaus.
---

import { Steps, Aside } from "@astrojs/starlight/components";

This guide walks through creating a 32x32 sprite from a blank canvas.

<Steps>
1. **Create a new project.** `File > New` or `Ctrl+N`. Set width and height to `32`, color mode to `RGBA`, and add a palette (the default 16-color palette works fine for a first sprite).

2. **Name your first layer.** The layer panel on the right shows one layer named `Layer 1`. Double-click it and rename it to `Base`.

3. **Pick a color.** Click any color in the palette panel. The foreground swatch in the top-left of the color panel updates.

4. **Draw.** Select the pencil tool (`P`) and click or drag on the canvas. At 100% zoom pixels are small — use `+` to zoom in to 8x or 16x.

5. **Use the pixel grid.** Above 800% zoom, a pixel grid appears automatically. This helps you place individual pixels precisely.

6. **Save.** `File > Save` or `Ctrl+S`. Pixhaus saves as `.pixhaus` by default. You can also export to `.aseprite` or `.png` via `File > Export`.
</Steps>

## Useful shortcuts

| Action | Shortcut |
|---|---|
| Pencil | `P` |
| Eraser | `E` |
| Eyedropper | `I` |
| Fill bucket | `G` |
| Zoom in/out | `+` / `-` |
| Fit to window | `Ctrl+0` |
| Undo | `Ctrl+Z` |
| Redo | `Ctrl+Y` |
| Swap fg/bg colors | `X` |

<Aside>
All shortcuts are configurable. Open `Edit > Keybinds` to change them or load a preset (Aseprite-compatible and Photoshop-compatible presets are built in).
</Aside>

## Next steps

- [Build your first animation](/getting-started/first-animation/) from this sprite
- Learn more about [tools and brushes](/editor/tools/)
