---
title: Tileset editor
description: Drawing and managing tiles in a tileset.
---

The tileset panel is the companion to the tilemap canvas. It shows all tiles in the active tileset as a scrollable grid.

## Panel layout

The tileset panel opens automatically when a tilemap layer is active. It shows:
- A scrollable grid of tile thumbnails
- The active tile (highlighted, used when painting)
- A toolbar for tileset management

## Drawing tiles

Click any tile to select it for painting. Double-click a tile to open the tile canvas — a separate view focused on that tile's pixel art at a comfortable zoom level. Edits to the tile canvas update all cells in the tilemap that use that tile immediately.

## Tile properties

Right-click any tile for its properties dialog:
- **Name** — optional label for the tile
- **Collision shape** — none, full, custom (for physics layers in Unity)
- **Autotile membership** — which autotile groups this tile belongs to
- **Animation** — configure tile animation (see [Animated tiles](/tilemaps/animated-tiles/))

## Importing a tileset

`Tileset > Import from PNG` lets you import an existing tileset image. Pixhaus slices it into individual tiles based on the tile dimensions you specify (e.g., 16×16 with 0px spacing). Each slice becomes a tile in the new tileset.

## Multiple tilesets

A project can have multiple tilesets. Each tilemap layer references one tileset. Switch the active tileset in the tileset panel dropdown.
