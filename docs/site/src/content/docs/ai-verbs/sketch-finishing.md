---
title: Sketch finishing
description: Refines rough sketches into finished pixel art in the project style.
---

## What it does

Refines rough sketches, silhouettes, or stick-figure gesture poses into finished pixel art sprites in the project's learned style. The artist owns the pose and the timing; the verb owns the rendering. Input is one or more sketch frames; output is a new "AI finish" layer with one refined cel per sketch frame, frame-aligned to the originals so animation timing is preserved exactly.

## Parameters

- **`sketches`** — `array of SketchFrame` (required, at least one item). Each entry has:
  - **`frame_index`** — `integer` (required, ≥ 0). Frame in the sprite's timeline this sketch belongs to. The refined cel is placed on the same frame index in the output layer.
  - **`pixels`** — `PixelData` (required). The sketch pixel buffer. RGBA8 only; the verb refuses non-RGBA8 inputs at validation.
- **`style_prompt`** — `string | null` (optional). Free-form text appended after the base prompt — e.g. *"dark fantasy, muted palette, hard light from upper-left"*. Useful when the project style references alone do not pin the look you want.
- **`layer_name`** — `string | null` (optional, default `"AI finish"`). Display name for the new refined layer.

The verb also reads the active `VerbContext`'s style references — style sheets, trained LoRAs, and named palettes are all woven into the prompt automatically. You don't need to restate them in `style_prompt`.

## Backend requirements

- **`IMAGE_EDIT`** — needed to refine each sketch frame against the project style. The verb concretely supports `StabilityBackend`, `OpenAiBackend`, and `ReplicateBackend`; it downcasts the injected backend at invoke time and returns a clear `Backend` error if none of those three is registered.

## Output

- **`AddLayer`** — adds one new raster layer to the active sprite holding the refined frames. Each input sketch produces one cel in the output layer at the matching frame index.

The first refined frame is also surfaced as the preview thumbnail so the artist can judge style adherence before clicking through every cel.

## Cost and latency

- Typical: ~10s, ~$0.035 per call
- Max: ~30s, ~$0.065 per call

Latency scales linearly with the number of sketch frames — each frame is a separate image-edit call. A six-frame walk cycle takes roughly six times the typical single-frame latency. The cancel button drops the in-flight request and stops the loop between frames.

## Example

Open `examples/samples/character-knight.pixhaus` and add a new layer above the base. Sketch a four-frame attack animation as rough silhouettes — block out the wind-up, the strike, the recovery, and the return-to-rest. Don't worry about palette or detail; gesture and timing are what matter. Select all four sketch frames and invoke `AI > Sketch finishing`. Optionally enter a `style_prompt` like *"polearm, two-handed grip"* to nudge the rendering.

The verb runs four image-edit calls in sequence, each conditioned on the project's style references and the base prompt about preserving pose and proportion. After ~40 seconds you get a new layer named *"AI finish"* with four refined cels, one per sketch frame, ready to accept or reject as a single undo entry.

If the project has been through [style learning](/ai-verbs/style-learning/), the trained LoRA is automatically attached as a style reference; results stay on-model without any extra configuration.

## Related verbs

- [Style learning](/ai-verbs/style-learning/) — train a per-project style LoRA so sketch-finishing output stays on-model.
- [Cleanup](/ai-verbs/cleanup/) — snap the refined output to the project palette and remove sub-pixel anti-aliasing if the backend drifts.
- [Inbetween](/ai-verbs/inbetween/) — once the keyframes are finished, fill the gaps automatically.
