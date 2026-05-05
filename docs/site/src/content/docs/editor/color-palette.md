---
title: Color and palette
description: The color picker, palette panel, indexed-color mode, and palette I/O.
---

## Color picker

The color picker supports these input modes: HSV, HSL, RGB, Hex, and OKLCH. Click the mode label in the color panel to cycle between them.

The foreground and background swatches in the corner of the color panel show the current colors. Press `X` to swap them. Press `D` to reset to black foreground, white background.

## Palette panel

The palette panel shows the project's color palette as a grid. Click any swatch to set the foreground color. Right-click for options: rename color, delete, lock (prevents accidental changes).

Drag swatches to reorder the palette.

## Indexed-color mode

In indexed mode, the palette is the source of truth. Every pixel stores a palette index (0–255), not an RGBA value. Changing a color in the palette updates every pixel that uses that index across all frames and layers simultaneously. This is the canonical workflow for palette swap art.

Switch between RGBA and indexed mode in `Sprite > Color mode`.

## Palette operations

| Operation | Where |
|---|---|
| Add color | `+` button in palette panel, or pick with eyedropper then `Palette > Add color` |
| Delete color | Right-click swatch > Delete |
| Sort by hue/luminance | `Palette > Sort` |
| Ramp generator | `Palette > Generate ramp` — pick two colors and a step count |
| Harmony picker | `Palette > Harmony` — shows split-complement, triad, tetrad, analogous suggestions |

## Palette I/O

Import and export palettes via `Palette > Load` and `Palette > Save`. Supported formats:

| Format | Extension | Notes |
|---|---|---|
| GIMP palette | `.gpl` | Most common for pixel art community sharing |
| Microsoft palette | `.pal` | RIFF format |
| JASC palette | `.pal` | Paint Shop Pro format |
| Photoshop swatches | `.aco` | Read-only |
| Lospec hex | `.hex` | One hex color per line |

## Lospec browser

`Palette > Browse Lospec` opens a searchable browser of community palettes from lospec.com. Click a palette to preview it against your sprite. Click `Import` to add it to the project.
