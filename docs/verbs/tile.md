# Tile verb

Generate a complete 47-tile blob autotile set from 1–3 example tiles.

## What it does

The Tile verb takes example tiles that show representative transitions in the
desired visual style and produces a full 48-tile atlas ready for blob autotile
painting. Tile 0 is the empty/transparent tile by convention; tiles 1–47 cover
every valid blob autotile configuration.

The verb runs two backend passes:

1. **Style analysis** — a VLM call that describes the visual style of the
   example tiles. This pass is non-fatal: if it fails, the verb falls back to a
   generic description and continues.

2. **Atlas generation** — an image-generation call that produces an 8×6 grid
   (one cell per tile, starting from the top-left). The resulting image is
   resized to exact dimensions if the backend returns a different size.

## Inputs

| Field | Type | Required | Default | Notes |
|---|---|---|---|---|
| `examples` | `PixelData[]` | yes | — | 1–3 RGBA8 tiles showing the desired style. Each `PixelData` is `{ width, height, bytes_per_pixel: 4, stride, bytes }` where `bytes` is base64-encoded over IPC. |
| `tile_width` | `u32` | no | `16` | Width of one tile in pixels (1–256) |
| `tile_height` | `u32` | no | `16` | Height of one tile in pixels (1–256) |
| `tileset_name` | `string` | no | `"Autotile"` | Display name for the new tileset |

Examples must be RGBA8 format. The pixel buffer size must match
`tile_width × tile_height × 4` bytes (stride-padded buffers are unpacked
before encoding).

## Output

A single `AddTileset` effect containing:

- A `Tileset` record configured for blob autotile layout (`tile_count: 48`,
  `base_index: 1`, inline pixel buffer source).
- The atlas pixel buffer (`tile_width * 8` × `tile_height * 6` pixels, RGBA8).

The tileset ID and buffer ID are placeholder zeros; the host rewrites them when
committing the effect.

## Backend requirements

The verb requires a backend that satisfies both `IMAGE_GENERATION` and
`VISION_LANGUAGE` capabilities. If no such backend is registered, the verb
fails immediately with a `Backend` error.

## Cancellation

The verb checks the cancellation token after each backend pass. If cancelled,
it returns `VerbError::Cancelled` without emitting a partial effect.

## Progress events

| Progress | Message |
|---|---|
| —     | `Started` with backend ID |
| 5 %   | "analyzing example tile style" |
| 30 %  | "generating autotile atlas" |
| 90 %  | "building tileset" |
| 100 % | "autotile set ready" |

## Atlas layout

```
col  0    1    2    3    4    5    6    7
row 0: [  0] [  1] [  2] [  3] [  4] [  5] [  6] [  7]
row 1: [  8] [  9] [ 10] [ 11] [ 12] [ 13] [ 14] [ 15]
row 2: [ 16] [ 17] [ 18] [ 19] [ 20] [ 21] [ 22] [ 23]
row 3: [ 24] [ 25] [ 26] [ 27] [ 28] [ 29] [ 30] [ 31]
row 4: [ 32] [ 33] [ 34] [ 35] [ 36] [ 37] [ 38] [ 39]
row 5: [ 40] [ 41] [ 42] [ 43] [ 44] [ 45] [ 46] [ 47]
```

Cell 0 (top-left) is the empty tile. Cells 1–47 are filled in
`BLOB47_MASKS` index order.

## Example usage (IPC)

```json
{
  "verb": "pixhaus.builtin.tile",
  "inputs": {
    "examples": [
      {
        "width": 16,
        "height": 16,
        "bytes_per_pixel": 4,
        "stride": 64,
        "bytes": "<base64-encoded pixels>"
      }
    ],
    "tile_width": 16,
    "tile_height": 16,
    "tileset_name": "dungeon_walls"
  }
}
```
