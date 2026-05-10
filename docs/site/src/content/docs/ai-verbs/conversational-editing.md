---
title: Conversational Edit
description: Turn a natural language instruction into a sequence of editor commands.
---

## What it does

Conversational editing takes a free-form instruction — "add a scar layer over the eye, slow the walk to 8 fps, hide the outline layer" — and asks a tool-using vision-language model to plan the equivalent sequence of editor commands. The plan appears in a preview dialog before anything runs, and on accept the whole sequence commits as a single coalesced undo entry.

The model can plan structural edits (add layers, add frames, retime, rename, opacity, visibility, animation tags) directly. Pixel-level edits that need actual painting — "draw a scar at frame 0" — are surfaced as notes so you can paint them by hand or hand them off to a generation verb.

## Parameters

- **`instruction`** — `string` (required). The natural language editing instruction. Must not be empty or whitespace-only. There is no hard length cap, but short and specific instructions plan more reliably than paragraphs.

## Backend requirements

- **`VISION_LANGUAGE`** — needed so the model can see the current sprite when planning edits.
- **`TOOL_USE`** — needed because the verb works by exposing eight editor tools to the model and converting tool calls into effects.

Anthropic Claude (3.5 Sonnet and newer) and OpenAI GPT-5 both satisfy these. Tool-calling Ollama models work for offline use.

## Output

Conversational editing returns a mix of effects depending on what the model planned:

- `AddLayer` — empty raster layers added with a chosen blend mode and opacity.
- `AddFrames` — empty frames inserted at a position with a chosen duration.
- `AddTag` — named animation tag covering a frame range, with a loop direction.
- `Custom` (`pixhaus.builtin.conversational.rename_layer`) — rename an existing layer.
- `Custom` (`pixhaus.builtin.conversational.set_frame_durations`) — retime frames to a target FPS, optionally scoped to a tag.
- `Custom` (`pixhaus.builtin.conversational.set_layer_opacity`) — change a layer's opacity 0–255.
- `Custom` (`pixhaus.builtin.conversational.set_layer_visibility`) — show or hide a layer.

Pixel-edit instructions land in `notes`, not `effects`. The preview dialog shows them as a checklist of follow-up work.

## Cost and latency

- Typical: ~4s, ~$0.005 per call
- Max: ~30s, ~$0.05 per call

A simple two-operation plan returns in a few seconds. Long plans on large sprites (many layers, many tags) approach the max.

## Example

Open `examples/samples/enemy-slime.pixhaus` and run Conversational Edit with:

> Add a "Glow" layer above the body in screen blend mode at 60 percent opacity, then create a "bounce" tag covering frames 0–7 in ping-pong, and slow it to 6 fps.

The preview dialog shows three operations queued — `AddLayer Glow`, `AddTag bounce 0-7 ping_pong`, `set_frame_durations target_fps=6` — plus zero pixel-edit notes. Accept and the slime project gains the layer, the tag, and the new timing in one undo step. Reject and nothing changes.

## Related verbs

- [Critique](/ai-verbs/critique/) — find the issues; describe the fix here
- [Cleanup](/ai-verbs/cleanup/) — for palette/pivot fixes you'd otherwise plan in plain language
- [Variant](/ai-verbs/variant/) — when the request is "make N versions of this", reach for Variant instead
