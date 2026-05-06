# Project style learning

**ID:** `pixhaus.builtin.project_style_learning`  
**Capability required:** `STYLE_TRAINING`  
**Typical duration:** 15–30 minutes  
**Estimated cost:** $2–5 USD (Replicate GPU time)

Trains a small LoRA (Low-Rank Adaptation) on all visible raster layers in
the active project and registers the result as the project's default style
reference. Once trained, subsequent verbs (Inbetween, Continue, Variant,
Sketch finishing, and others) automatically condition their output on the
learned style.

## When to use

Run this verb once at the start of a project when you have enough art to
define the style — typically after drawing 5–20 representative frames. Re-run
it whenever the project's style evolves significantly.

## Inputs

| Field | Type | Default | Description |
|---|---|---|---|
| `training_images` | array of pixel buffers | — | **Required.** Pixel data extracted by the host from every visible raster layer. The host packages these before invoking. |
| `lora_rank` | integer 4–32 | 16 | LoRA adapter dimension. Lower = faster training, smaller model. Higher = more detail captured. |
| `steps` | integer 200–2000 | 1000 | Training step count. More steps improve style fidelity but increase time and cost. |
| `label` | string | project name | Human-readable name shown in the style picker (e.g. "Knight sprite style"). |
| `model` | string | `"ostris/flux-dev-lora-trainer"` | Replicate model used for training. Override only if you need a different LoRA trainer. |

## Output

On accept, the host:

1. Downloads the trained LoRA weights (safetensors) from the URL in the
   effect payload.
2. Saves the file to `.pixhaus/style.safetensors` inside the project
   directory.
3. Registers a `StyleReference::TrainedModel` entry in the project so
   every subsequent verb invocation receives it in `VerbContext::style_refs`.

The undo system records the registration as a single command. Undoing removes
the style reference; it does not delete the weights file.

## Effect payload

The `VerbEffect::Custom` payload is a JSON object:

```json
{
  "weights_url":      "https://replicate.delivery/.../lora_weights.safetensors",
  "training_id":      "abc123",
  "label":            "Knight sprite style",
  "model_id":         "pixhaus.style.lora.replicate",
  "training_model":   "ostris/flux-dev-lora-trainer",
  "steps":            1000,
  "lora_rank":        16,
  "image_count":      25
}
```

`model_id` is the opaque format identifier verbs use to decide how to feed
the model to their backend. Only the Replicate image-generation adapters
consume `"pixhaus.style.lora.replicate"` currently.

## Backend

Requires a Replicate API key stored in the OS keychain under the service
name `"replicate"`. Set it in **Preferences → AI backends → Replicate**.

The verb uses Replicate's training API (`POST /v1/models/{owner}/{model}/trainings`)
rather than the standard predictions endpoint, because LoRA training produces
a model artifact rather than an image. Progress events stream Replicate's
training log lines during the 15–30 minute wait.

## Cancellation

The verb honours its cancellation token between polling ticks (1-second
intervals). When cancelled, it sends a best-effort cancel request to
Replicate. Replicate may not cancel the job immediately; partial GPU charges
may still be incurred.

## Tips

- Include frames from multiple animation states (idle, walk, attack) to
  capture the project's full style range, not just one pose.
- If style fidelity is poor, increase `steps` to 1500–2000 and re-run.
- If training is slow and cost is a concern, drop `lora_rank` to 8 and
  `steps` to 500 for a quick draft pass.
- The trained model persists in the project directory and survives project
  moves — it is not re-downloaded unless the weights file is deleted.
