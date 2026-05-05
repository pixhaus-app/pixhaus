---
title: Autotile rules
description: Wang blob autotile and custom rule-based tile selection.
---

Autotile automatically picks the correct tile variant for a cell based on what its neighbors contain. Paint a ground tile next to a wall tile and the edge transition appears without manually selecting each transition variant.

## Autotile types

### Wang edge-blob (47 tiles)

The standard for game tilemaps. 47 tile variants cover every possible neighbor combination for a simple binary (inside/outside) rule. This is what Tiled calls the "blob" tileset.

Configure by right-clicking a tileset in the tileset panel and choosing `Configure autotile > Wang edge-blob (47 tiles)`. You then map your drawn tiles to each of the 47 positions using the visual position editor.

### Wang corner-blob (16 tiles)

A simplified variant using only corner topology — 16 tiles instead of 47. Faster to draw but produces less detailed transitions. Good for small tilesets.

### Custom rule-based

`Configure autotile > Custom rules`. Define per-tile matching rules using a visual neighbor grid — each cell in a 3×3 neighbor grid can be set to "must match", "must not match", or "don't care". The rule engine evaluates from top to bottom and picks the first matching rule.

This is equivalent to Tiled's "rule tiles" feature.

## Painting with autotile

When a tilemap layer with autotile configured is active, painting a tile at any position automatically updates the tile and its neighbors to show correct transitions. The result is the same as if you had hand-placed every variant.

To temporarily disable autotile while painting (to place a specific variant manually), hold `Alt` while painting.

## Autotile generation with AI

The **Tile** AI verb can generate a full 47-tile blob autotile set from just 1–3 example transition drawings. See [Tile](/ai-verbs/tile/).
