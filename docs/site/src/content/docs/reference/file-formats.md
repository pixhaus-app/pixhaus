---
title: File formats
description: File formats that Pixhaus reads and writes.
---

## Native format: .pixhaus

The Pixhaus project file. Stores all project data — layers, frames, tilemaps, palettes, frame tags, slices, and embedded assets.

| Property | Value |
|---|---|
| Extension | `.pixhaus` |
| Container | Binary (magic bytes `PIXH` + version u16) |
| Encoding | MessagePack |
| Compression | zstd (level 3 by default) |
| Schema | Versioned; forward-compatible reader skips unknown optional chunks |

Read the full format specification in `docs/file-format.md` in the repository.

## Aseprite: .aseprite / .ase

Pixhaus reads and writes `.aseprite` files at near-full fidelity. Round-trip compatibility means an `.aseprite` file opened in Pixhaus and saved back should be indistinguishable from the original in Aseprite.

See [Aseprite compatibility](/reference/aseprite-compat/) for a detailed feature matrix.

## Photoshop: .psd

Import only. Pixhaus reads Photoshop `.psd` files with layer hierarchy, blend modes, opacity, and visibility. Layer effects, smart objects, adjustment layers, and text layers are not supported and are either skipped or rasterized with a warning.

## Sprite sheet export: .png + .json

Pixhaus exports sprite sheets as a PNG plus Aseprite-compatible JSON metadata. This is the primary handoff format for Unity.

JSON schema:
```json
{
  "frames": {
    "sprite 0": {
      "frame": { "x": 0, "y": 0, "w": 32, "h": 32 },
      "duration": 100
    }
  },
  "meta": {
    "size": { "w": 256, "h": 256 },
    "frameTags": [...],
    "slices": [...]
  }
}
```

## Animated GIF: .gif

Export only. Supports palette quantization (use existing palette, quantize to 256 colors, per-frame quantize), dithering (off, Floyd-Steinberg, Bayer 8×8), and loop count.

## Animated WebP: .webp

Export only. Lossless or lossy.

## Tiled tilemap: .tmx

Export only. Tiled-compatible XML tilemap with embedded or external tileset. Compatible with SuperTiled2Unity for Unity import.

## Palette formats

| Format | Extension | Read | Write |
|---|---|---|---|
| GIMP palette | `.gpl` | Yes | Yes |
| Microsoft palette | `.pal` (RIFF) | Yes | Yes |
| JASC palette | `.pal` | Yes | Yes |
| Photoshop swatches | `.aco` | Yes | No |
| Lospec hex | `.hex` | Yes | Yes |
