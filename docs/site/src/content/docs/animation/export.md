---
title: Export formats
description: Sprite sheets, animated GIF, WebP, and TMX export.
---

## Sprite sheet (PNG + JSON)

`File > Export > Sprite sheet (PNG)`

Packs animation frames into a sprite sheet PNG with Aseprite-compatible JSON metadata. This is the primary Unity handoff format.

**Layout options:**
- **Grid** — uniform cells, one frame per cell
- **Packed** — rectangle bin-packing for tightest output (minimizes sheet size)
- **By row** — one frame per row

**JSON schema** — Aseprite-compatible. Includes `frames` (rectangles + durations), `meta` (sheet size, scale, frame tags, slices). Unity's Pixhaus importer reads this JSON to generate `Sprite` assets and `AnimationClip` assets.

## Animated GIF

`File > Export > Animated GIF`

Options: palette mode (existing palette, quantize to 256 colors, per-frame quantize), dithering (off, Floyd-Steinberg, Bayer 8×8), loop count, frame timing.

## Animated WebP

`File > Export > Animated WebP`

Lossless or lossy. Smaller than GIF for the same quality.

## TMX tilemap

`File > Export > TMX tilemap`

Exports tilemap layers as Tiled-compatible `.tmx` (XML) plus a tileset PNG. Supports layer hierarchy, per-tile flip/rotate flags, and tile animation. Unity's SuperTiled2Unity importer reads this format.

## Native format (.pixhaus)

`File > Save` or `File > Save as`

The `.pixhaus` format stores everything — all layers, frames, tilemaps, palettes, frame tags, slices — in an open binary format (MessagePack + zstd). Designed for version control friendliness.

See [file formats](/reference/file-formats/) for the full format spec.

## Aseprite (.aseprite)

`File > Export > Aseprite`

Exports a `.aseprite` file compatible with Aseprite. Useful for artists on mixed teams where some members prefer Aseprite. See [Aseprite compatibility](/reference/aseprite-compat/) for what round-trips cleanly.
