---
title: Tilemaps overview
description: How tilemap layers work in Pixhaus.
---

Tilemaps are a first-class layer type in Pixhaus, not a separate tool or export target. A tilemap layer lives in the same project file alongside your sprite layers, shares the same undo stack, and renders in the same canvas.

## Concepts

**Tileset** — a collection of tiles, each of a fixed pixel size (e.g., 16×16). The tileset stores both the tile images and per-tile metadata (collision shape, animation frames, autotile membership).

**Tilemap layer** — a 2D grid of tile references. Each cell holds a tile ID and optional flags: rotate 90°/180°/270°, flip H, flip V.

**Autotile** — a set of rules that automatically selects the correct tile variant based on what neighboring cells contain. Pixhaus supports Wang corner-blob (16 tiles), Wang edge-blob (47 tiles), and custom rule-based sets.

## Workflow

1. Add a tilemap layer (`Layer > New layer > Tilemap`).
2. Draw or import a tileset — paint tiles in the tileset panel, or import from a PNG.
3. Configure autotile rules if needed (optional).
4. Paint the tilemap — click or drag on the canvas to place tiles. With autotile active, transitions update automatically.
5. Export as TMX for Tiled/Unity or embed in the `.pixhaus` project file.

## Why in the same tool

The standard workflow for indie games is:
1. Draw sprites in Aseprite
2. Assemble tilesets in Aseprite
3. Paint the map in Tiled
4. Import into Unity

Every tool handoff adds friction and creates sync problems. In Pixhaus, steps 1–3 are one tool, one file, one undo stack.

## Animated tiles

Tilemap cells can reference animated tiles — tiles with multiple frames and timing, independent of the sprite timeline. Water ripples, lava bubbles, and conveyor belts are the canonical examples. See [Animated tiles](/tilemaps/animated-tiles/).
