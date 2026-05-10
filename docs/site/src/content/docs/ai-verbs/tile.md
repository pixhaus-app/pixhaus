---
title: Tile
description: Generate a 47-tile blob autotile set from 1-3 example transitions.
---

## What it does

Generates a complete 47-tile Wang edge-blob autotile set from one to three example tiles that establish the visual style. Provide the example tiles you would draw by hand — an isolated tile, a straight edge, a corner — and the verb fills in every remaining transition variant in the same style. Output is a ready-to-paint tileset with the empty tile reserved at index 0 and the 47 blob configurations at indices 1–47.

## Parameters

- **`examples`** — `array of PixelData` (required, 1–3 items). RGBA8 example tiles that condition the style. More examples improve fidelity; one example is the minimum the style-analysis pass needs.
- **`tile_width`** — `integer` (optional, default `16`, range 1–256). Width of each tile in pixels.
- **`tile_height`** — `integer` (optional, default `16`, range 1–256). Height of each tile in pixels.
- **`tileset_name`** — `string | null` (optional, default `"Autotile"`). Display name for the resulting tileset.

## Backend requirements

- **`VISION_LANGUAGE`** — needed to analyse the example tiles and produce a style description for the generation prompt.
- **`IMAGE_GENERATION`** — needed to render the 8×6 tile atlas.

A single backend that advertises both capabilities (Anthropic Claude with vision plus a paired image-gen adapter, or a multi-modal Replicate model) satisfies the requirement. The runtime refuses to dispatch the verb until a matching backend is registered.

## Output

- **`AddTileset`** — adds one new tileset to the active sprite. The tileset record carries the tile size, a tile count of 48 (47 blob configurations plus the reserved empty tile at index 0), `base_index = 1`, and an inline pixel buffer holding the 8×6 atlas.

The output also includes a thumbnail extracted from tile index 1 — the isolated tile — so the host UI has something representative to show before the artist clicks through.

## Cost and latency

- Typical: ~30s, ~$0.05 per call
- Max: ~120s, ~$0.20 per call

The two-pass design (style analysis, then atlas generation) is what makes the verb expensive relative to single-image verbs. Style analysis is advisory — if it fails or the backend returns an unexpected shape, the verb logs a warning and falls through to a generic prompt rather than failing the run.

## Example

A forest project in `examples/samples/level-forest.pixhaus` ships with a hand-drawn grass tile and a grass-to-dirt edge tile. Select both as examples, invoke `AI > Tile`, leave tile size at 16×16, and name the result `"Grass autotile"`. The vision pass produces a description like *"flat green grass, #4a7a3c palette, crisp 1-pixel edges, dirt transitions in warm browns"*. The generation pass produces the 128×96 atlas. The tileset is added to the active sprite and is immediately usable with the autotile-aware brush — paint a region, the resolver picks the right tile per cell.

If the backend returns the atlas at the wrong dimensions the verb resizes with nearest-neighbour to keep tile boundaries sharp; no smoothing is ever applied to pixel-art output.

## Related verbs

- [Tileset from description](/ai-verbs/tileset-from-description/) — generate a tileset from a text prompt when no example tiles exist yet.
- [Variant](/ai-verbs/variant/) — re-skin an existing tile or sprite with a different palette or accent.
- [Cleanup](/ai-verbs/cleanup/) — snap the generated atlas to the project palette if the backend drifts.
