---
title: Extend
description: Generate multi-direction sprite views from a single source frame.
---

## What it does

Extend takes a single sprite frame — typically the front-facing pose you've already drawn — and synthesises views of the same character from other directions. Pick a 4-direction set (south, west, north, east), an 8-direction set (cardinals plus diagonals), or a custom list, and the verb produces one new layer per direction with the synthesised pixels.

The source direction is never re-generated. If you set `source_direction: south` and ask for the 4-direction set, you get three new layers (west, north, east), not four.

## Parameters

- **`source`** — `object` (required). RGBA8 pixel data of the source frame. Must be well-formed and 4 bytes per pixel; indexed and grayscale inputs are rejected at the boundary.
- **`source_direction`** — `string` (default: `"south"`). Which direction the source sprite is currently facing. One of `south`, `south_west`, `west`, `north_west`, `north`, `north_east`, `east`, `south_east`.
- **`direction_set`** — `object` (default: `{kind: "four_direction"}`). Which directions to generate. One of:
  - `{kind: "four_direction"}` — south, west, north, east.
  - `{kind: "eight_direction"}` — all eight compass directions.
  - `{kind: "custom", directions: [...]}` — explicit list. Duplicates are de-duplicated; the source direction is silently excluded.
- **`style_intensity`** — `number` (default: `0.8`, range `0.0`–`1.0`). Higher values keep the output closer to the source's palette and pixel style; lower values give the model more creative latitude. Stick near `0.8` for sprite work — `0.5` and below will start to drift in shape and detail.
- **`layer_name`** — `string` or `null` (default: `null`). Base name for the generated layers. Defaults to `"Extend"`. Each new layer is named `"{base} – {Direction}"`, e.g. `"Walk – West"`.

## Backend requirements

- **`IMAGE_GENERATION`** — needed to render the synthesised pixels.
- **`VIEW_SYNTHESIS`** — needed to reason about the sprite's geometry across viewpoints. Backends that advertise the capability but don't implement the `ViewSynthesisBackend` sub-trait are rejected at invoke time.

In practice this means a Replicate adapter wrapping a TripoSR-class model or a Stability backend with style-conditioned generation. Configure one in `Edit > Preferences > AI backends`.

## Output

- **`AddLayer`** — one effect per generated direction. Each effect adds a new layer to the active sprite with a single cel at the active frame containing the synthesised pixels. Failed directions are skipped with a warning in `notes` rather than aborting the whole run.

## Cost and latency

- Typical: ~60s, ~$0.20 per call (4 directions at ~15 s and ~5 ¢ each via Replicate).
- Max: ~480s, ~$0.80 per call (8 directions on a high-resolution model).

## Example

You've drawn a front-facing knight at 32×32 in `examples/samples/character-knight.pixhaus`. Open it, select the cel, run `AI > Extend` with `direction_set: four_direction` and the default `style_intensity: 0.8`. The verb sends the south-facing source to the backend three times — once asking for west, once for north, once for east — and adds three new layers named `Extend – West`, `Extend – North`, and `Extend – East` above the source.

If one direction fails (rate limit, timeout, model rejection), the others still commit and the failure shows up as a warning note. Switch to `eight_direction` for diagonal walk cycles, but expect the cost and latency to roughly double.

## Related verbs

- [Continue](/ai-verbs/continue/) — extend an animation forward in time rather than into other directions.
- [Variant](/ai-verbs/variant/) — generate palette swaps or equipment overlays of an existing direction instead of new viewpoints.
- [Cleanup](/ai-verbs/cleanup/) — snap each generated direction to the palette after the fact if `style_intensity` left some drift.
