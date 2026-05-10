---
title: Tileset from description
description: Generate a full autotile-compatible tileset from a text description using an image-generation backend.
---

## What it does

Generates a complete autotile-compatible tileset from a single text description — no source tiles required. Pick a layout (47-tile blob, 16-tile corner, or 4-tile minimal), describe the look in plain language, and the verb produces a sprite sheet where each cell maps 1-to-1 to a tile slot in the chosen layout. The result is ready to paint with the autotile-aware brush.

## Parameters

- **`description`** — `string` (required, non-empty). Text description of the tileset's visual style and subject. This is the dominant input to the generation prompt.
- **`tile_size`** — `integer` (optional, default `16`, range 4–128). Width and height of each tile in pixels. Tiles are square.
- **`autotile_kind`** — `enum` (optional, default `"blob47"`). One of:
  - `"blob47"` — 8×6 sheet, 48 cells. Index 0 is the transparent empty tile; indices 1–47 cover every orthogonal-neighbour combination. The right pick when you need a fully-featured autotile.
  - `"corner16"` — 4×4 sheet, 16 cells. Lighter than blob47, blends on corners. Every cell is paintable.
  - `"minimal4"` — 4×1 sheet, 4 cells. Isolated, one-sided, two-sided, fully-connected. The cheapest layout to generate.
- **`name`** — `string | null` (optional). Display name for the resulting tileset. When omitted, the verb derives a slug from the first 32 characters of the description, cut at a word boundary with an ellipsis.
- **`style_reference`** — `array of integer | null` (optional). Raw PNG bytes of a style-conditioning image passed to the backend alongside the prompt.

## Backend requirements

- **`IMAGE_GENERATION`** — needed to render the tile sheet. Stability, OpenAI image-gen, Replicate, and ComfyUI adapters all satisfy this. The verb returns a clear `Backend` error if no matching backend is configured.

## Output

- **`AddTileset`** — adds one new tileset to the active sprite. The tileset's `tile_count` and `base_index` track the chosen layout (Blob47 reserves index 0 for the empty tile and starts paintable indices at 1; Corner16 and Minimal4 use every cell, base index 0). The pixel buffer is inlined into the project file.

The tile sheet itself is also returned as the thumbnail, so the host UI can show the full sheet in the preview panel before commit.

## Cost and latency

- Typical: ~15s, ~$0.02 per call
- Max: ~120s, ~$0.10 per call

A Minimal4 generation finishes faster than a Blob47 because the output canvas is one-twelfth the area; latency scales with sheet pixels, not with tile count.

## Example

Open `examples/samples/level-forest.pixhaus`, invoke `AI > Tileset from description`, and enter `"stone dungeon floor with moss, muted earth palette, sharp 1-pixel edges"`. Leave tile size at 16, leave the layout at `blob47`. The verb sends one image-generation request sized 128×96 and returns a 48-cell sheet — empty tile at top-left, the 47 blob transitions filling the remaining cells. Accept the preview; the new tileset is registered against the active sprite and the autotile brush picks it up immediately.

For a quick prototype, drop to `minimal4` instead. Same prompt, four tiles, ~5s round-trip. Useful when you are sketching a level layout and want plausible tiles in place before committing to art direction.

If the backend returns a sheet at the wrong dimensions the verb fails the run rather than resizing — sheet geometry is contractual for the autotile resolver.

## Related verbs

- [Tile](/ai-verbs/tile/) — generate a tileset from 1–3 example tiles when you already have a style established.
- [Variant](/ai-verbs/variant/) — generate alternate palettes of a finished tile or sprite.
- [Cleanup](/ai-verbs/cleanup/) — snap a generated sheet to the project palette and strip sub-pixel anti-aliasing.
