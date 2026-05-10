---
title: Learn Project Style
description: Train a LoRA on all project layers and register it as the project style reference.
---

## What it does

Style learning trains a small LoRA adapter on every visible raster layer in the project, then registers the trained model as the default style reference. Once it's run, every other verb that supports style conditioning — Inbetween, Continue, Variant, Sketch finishing, and the tile generators — picks it up automatically. You stop having to attach reference layers by hand for every call.

Training takes 15–30 minutes on Replicate's hosted GPUs. The resulting weights file lives in the project folder, so reusing the model on subsequent verb runs is free; you only pay the training cost once per style refresh.

## Parameters

- **`training_images`** — `PixelData[]` (required, minimum 1). Pixel buffers the host extracted from visible raster layers. The verb does not read pixels from the project itself — the host composites and supplies them. Each entry must be well-formed (consistent width, height, stride, and bytes_per_pixel).
- **`lora_rank`** — `integer | null` (default: `16`, range 4–32, optional). Adapter matrix dimensionality. Lower values train faster and produce smaller files; higher values capture more detail at higher training cost.
- **`steps`** — `integer | null` (default: `1000`, range 200–2000, optional). Training step count. More steps improve fidelity but extend wall-clock time and dollar cost roughly linearly.
- **`label`** — `string | null` (default: project name, optional). Human-readable name shown in the style picker.
- **`model`** — `string | null` (default: `"ostris/flux-dev-lora-trainer"`, optional). The Replicate model to train against. Override only if you've validated an alternative trainer for your art style.

## Backend requirements

- **`STYLE_TRAINING`** — needed to drive a hosted LoRA training job. Currently satisfied by the Replicate backend; other providers can implement the same capability.

## Output

Style learning emits a single `Custom` effect with name `pixhaus.builtin.project_style_learning.model`. The payload carries the weights download URL, the Replicate training job ID, the chosen label, the effective steps and rank, and the training image count. On commit, the host downloads the safetensors file, drops it into the project's `styles/` directory, and registers it as a `StyleReference::TrainedModel` so dependent verbs can find it.

The verb also returns two notes reminding you to commit the preview to actually register the model.

## Cost and latency

- Typical: ~900s (15 minutes), ~$2.00 per call
- Max: ~1800s (30 minutes), ~$5.00 per call

Cost scales with `steps` and image count. Defaults (1000 steps, ~25 images) land near the typical figure. Bumping rank to 32 with 2000 steps approaches the max.

## Example

You've been building `examples/samples/character-knight.pixhaus` for an hour and have roughly forty distinct frames across walk, idle, and attack animations. The style is locked in — chunky outlines, a tight 16-color earthen palette, soft dithered shading.

Run Learn Project Style with the defaults. The host extracts every visible raster layer as a training image and submits the job. Twenty minutes later the preview dialog returns a single trained-model effect labeled "character-knight". Accept, and the next time you run Inbetween or Continue on this project the new frames come back already matching the established line weight, palette, and shading style — no manual reference attachment needed.

## Related verbs

- [Inbetween](/ai-verbs/inbetween/) — consumes the trained model automatically when generating tween frames
- [Variant](/ai-verbs/variant/) — uses the style reference for palette swaps and equipment overlays
- [Sketch finishing](/ai-verbs/sketch-finishing/) — finishes rough sketches in the trained style
