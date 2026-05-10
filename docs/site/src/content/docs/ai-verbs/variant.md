---
title: Variant
description: Generate sprite variants — palette swaps, equipment overlays, expression sets.
---

## What it does

Generates one or more derived sprites from a base layer: a re-coloured version of the same character, the same pose with different equipment, or the same face with a different expression. The verb has two modes — a deterministic palette swap that needs no backend, and a free-form text-edit mode that calls an image-edit model. The source layer is never modified; results land on new layers above it.

## Parameters

- **`pixels`** — `PixelData` (required). Source pixel buffer for the active layer or selection. RGBA8 only (`bytes_per_pixel == 4`); the verb refuses indexed or RGB-only buffers.
- **`mode`** — `oneOf` (required). Either a palette swap or a text edit:
  - **Palette swap** — `{ "kind": "palette_swap", "substitutions": [...] }`. Each substitution has a `from` `Rgba` and a `to` `Rgba`. Pixels whose RGBA channels exactly match a `from` value are replaced; unmatched pixels are copied through. Alpha is part of the match, so a fully transparent pixel and a coloured pixel with the same RGB are treated independently. First match wins when substitutions overlap.
  - **Text edit** — `{ "kind": "text_edit", "description": "...", "count": 1 }`. The `description` (non-empty string) is sent to an image-edit backend along with the source PNG. `count` (integer, default 1) requests that many independent variants in a single call.
- **`layer_name`** — `string | null` (optional, default `"Variant"`). Display-name prefix for the output layer(s). When more than one variant is produced the verb suffixes a 1-based index — `"Variant 1"`, `"Variant 2"`, and so on.

## Backend requirements

The descriptor declares no required capabilities, so the verb installs cleanly without any inference backend configured. Behaviour at invoke time:

- Palette-swap mode runs entirely on the local CPU.
- Text-edit mode looks up an `IMAGE_EDIT`-capable backend in the registry at invoke time and returns a clear `Backend` error if none is configured. Stability, OpenAI, and Replicate adapters all satisfy the capability.

## Output

- **`AddLayer`** — adds one new raster layer per generated variant to the active sprite, each with a single cel on the active frame holding the variant pixels.

## Cost and latency

- Typical: ~5s, ~$0.005 per call
- Max: ~60s, ~$0.05 per call

Palette-swap mode is effectively free and finishes in milliseconds; the cost estimate covers the text-edit path, which dominates wall time.

## Example

A character study in `examples/samples/character-knight.pixhaus` ships with one base sprite. To generate three colour variants for team identification, select the body layer, invoke `AI > Variant`, switch to text-edit mode, and describe the change: *"same pose and equipment, blue tabard instead of red, identical palette count"*. Set `count` to `3`. The verb sends one image-edit request to the configured backend and returns three new layers labelled *"Variant 1"*, *"Variant 2"*, and *"Variant 3"* above the base.

For the deterministic case, palette-swap mode is the right tool when you already know the exact colours involved — for instance, swapping the red `(220, 40, 40, 255)` accent on the knight's shield for a green `(60, 180, 60, 255)` accent across every frame in one pass. No backend, no token cost, exact pixel result.

The cancel button stops both modes between progress checkpoints; in text-edit mode the in-flight backend call is dropped via the cancellation token.

## Related verbs

- [Tile](/ai-verbs/tile/) — generate a 47-tile autotile set from example transitions.
- [Tileset from description](/ai-verbs/tileset-from-description/) — generate a tileset from a text prompt instead of source pixels.
- [Sketch finishing](/ai-verbs/sketch-finishing/) — refine rough sketches into finished sprites.
