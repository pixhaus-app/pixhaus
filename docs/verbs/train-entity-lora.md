# Train consistency LoRA

**ID:** `pixhaus.builtin.train_entity_lora`
**Capability required:** `STYLE_TRAINING`
**Typical duration:** 15-30 minutes
**Estimated cost:** $2-5 USD (Replicate GPU time)
**B10 sub-task:** [B10.5](../planning/work/b10-reference-sheets.md#b105--per-entity-lora-training-optional-defer-able)

Trains a per-entity LoRA against a Reference entity's canonical sheet so
subsequent AI verbs invoked against that entity (or any Custom entity
anchored on it) condition their output on those weights. Companion to
[Project style learning](./project_style_learning.md): the project verb
trains a broad style across all visible raster layers; this one trains
tight identity from a single sheet.

Without this verb, the [anchor mechanic](../reference-sheets.md) already
ships the canonical sheet image as an IP-Adapter reference and applies
the extracted palette as a constraint — enough to keep most generations
visually consistent. Per-entity LoRAs take consistency from "good" to
"indistinguishable across hundreds of generations." Worth running once
identity-critical assets stabilize; defer until then.

## When to use

After approving a Reference sheet you intend to keep. The training is a
one-shot per entity: re-run only when the canonical sheet changes
substantially (a re-skin, a re-design). For sheets that are still
iterating, leave the per-entity LoRA untrained — running it on a
half-finished sheet wastes the GPU budget.

## Inputs

| Field | Type | Default | Description |
|---|---|---|---|
| `entity_id` | integer | — | **Required.** Reference entity to train against. The IPC command (`library_train_entity_lora`) reads this entity's canonical sheet image and packages it for the verb. |
| `training_images` | array of pixel buffers | — | **Required.** Decoded sheet image(s). The host typically passes just the canonical sheet; pass archive variants too when the entity has approved alternates. |
| `lora_rank` | integer 4-32 | 16 | LoRA adapter dimension. Lower trains faster; higher captures more detail. |
| `steps` | integer 200-2000 | 1000 | Training step count. |
| `label` | string | entity name (or `"entity-{id}"`) | Trigger word for the LoRA. |
| `model` | string | `"ostris/flux-dev-lora-trainer"` | Replicate training model. |

The host-facing IPC command takes a thinner [`TrainEntityLoraOptions`]
shape: only the four overrides above. The host derives
`training_images` from the canonical sheet automatically.

## Output

Two effects:

1. `VerbEffect::Custom { name: "pixhaus.builtin.train_entity_lora.model", payload }` —
   carries an [`EntityLoraResult`](../../ai/src/verbs/train_entity_lora/mod.rs)
   with the weights URL, training ID, label, and effective parameters.
2. `VerbEffect::UpdateEntityAi { entity_id, lora_path: None }` —
   a marker. The host applies the real `lora_path` after binding the URL
   (or a downloaded local path) to `Entity.ai.lora_path` and invalidates
   the anchor cache so subsequent verb invocations see the new weights.

The path currently lives as the Replicate weights URL. Downloading the
safetensors to the project directory is a shared follow-up with the
project-wide style training flow.

## Effect payload

```json
{
  "entity_id":      11,
  "weights_url":    "https://replicate.delivery/.../lora_weights.safetensors",
  "training_id":    "tr-xyz",
  "label":          "Hero",
  "training_model": "ostris/flux-dev-lora-trainer",
  "steps":          1000,
  "lora_rank":      16,
  "image_count":    1
}
```

## Anchor preference

When [`AnchorPayload`](../../ai/src/plugin/anchor.rs) builds for an
entity, the resolver prefers `Entity.ai.lora_path` over
`ProjectAi.project_lora_path`. Generations conditioned on the entity's
own sheet are biased toward the entity's identity rather than the
broader project style; the project-wide LoRA remains the fallback when
the entity hasn't been trained.

## Backend

Requires a Replicate API key stored in the OS keychain under the service
name `"replicate"`. Set it in **Preferences → AI backends → Replicate**.

## Cancellation

The verb itself respects cooperative cancellation: it checks
`cancel.is_cancelled()` at every progress checkpoint and forwards
`verb_cancel` to Replicate when it lands. The Replicate polling runs at
1-second intervals; partial GPU charges may still be incurred for
in-flight steps.

**Cancellation during the in-flight run is not currently exposed
through the user-facing IPC**, however. `library_train_entity_lora`
returns the `invocation_id` only after the promise resolves, so a
mid-run cancel button has nothing to cancel against. Wiring an
event-side channel that hands the active `invocation_id` to the UI as
soon as the verb starts (and downloading the safetensors to the project
directory so `lora_path` ends up as a real path) is tracked as a
follow-up issue against PR #179.

## Tips

- The sheet should already be approved — train against a sheet you plan
  to keep. Training the wrong sheet wastes 15-30 minutes.
- Don't train every entity. The IP-Adapter path (sheet image as
  reference) covers most consistency needs; per-entity LoRAs are for
  the protagonists, recurring NPCs, and signature props that appear in
  hundreds of generations.
- Re-running training overwrites `Entity.ai.lora_path`. The previous
  weights URL is dropped on the floor — Replicate's CDN keeps the file
  reachable for the audit horizon but Pixhaus stops referencing it.
