---
title: Animated tiles
description: Tiles that animate independently of the sprite timeline.
---

Animated tiles cycle through a sequence of frames with configurable per-frame timing, independent of the main animation timeline. This is how you get water ripples, lava, conveyor belts, and other environmental animations without coupling them to character animations.

## Creating an animated tile

1. In the tileset panel, right-click a tile and choose `Edit animation`.
2. Add frames to the tile animation — each frame points to another tile in the tileset (or you can draw new frames inline).
3. Set the duration for each frame in milliseconds.
4. Click `OK`. The tile shows an animation indicator icon.

## Playback

Animated tiles play in the canvas viewport at real-time speed. They loop by default. Playback is independent per-tile — each cell playing an animated tile runs its own animation clock.

## Tile animation vs sprite animation

Sprite animation is frame-based: the whole composition (all layers) advances one frame at a time. Tile animation is time-based: each tile runs on a wall-clock timer and loops independently.

The two systems coexist in the same project. A character walking (sprite animation) over a water tile (tile animation) renders both animations simultaneously.

## Exporting

Animated tile frame data exports in both TMX format (Tiled `<animation>` element per tile) and in the `.pixhaus` native format. The Unity Pixhaus importer converts animated tiles into Unity `Tile` assets with animation frames.
