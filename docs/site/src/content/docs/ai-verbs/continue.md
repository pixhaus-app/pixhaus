---
title: Continue
description: Predict the next 1–3 animation frames from the last 3–5 frames of the active layer.
---

## What it does

Continue predicts how an animation should keep going. Given the last 3–5 frames of the active layer as context, it asks a frame-interpolation backend to generate the next 1–3 frames in the same motion and style, then snaps them to the active palette and appends them to the timeline.

Use it when a walk cycle, idle loop, or hit reaction needs to extend past where you stopped drawing. The verb works best when the existing frames already establish a clear motion arc — three frames of a leg lifting give the model enough to predict the foot landing.

## Parameters

- **`num_frames`** — `integer` (default: `1`, range `1`–`3`). How many frames to generate. Stay at 1 or 2 for tight loops; 3 only when the existing motion is unambiguous.
- **`frame_duration_ms`** — `integer` or `null` (default: `null`, minimum `1`). Display duration of each generated frame in milliseconds. When `null`, the verb copies the active frame's duration so the new frames hold for the same beat as their context. Falls back to 100 ms (10 fps) when neither is available.

The host populates the context window from `ctx.references` — the last 3–5 frames of the active layer in display order. You don't pass this directly; the AI panel assembles it when you invoke the verb.

## Backend requirements

- **`IMAGE_GENERATION`** — needed to render the new frames as RGBA pixels.
- **`FRAME_INTERPOLATION`** — needed to condition generation on the prior-frame context window rather than a free-form prompt.

The runtime selects a compatible backend automatically (Replicate, Stability, OpenAI, Anthropic, Ollama, or ComfyUI). Pin a specific backend in `Edit > Preferences > AI backends` if you want consistent results across runs.

## Output

- **`AddFrames`** — appends `num_frames` new frames to the active sprite immediately after the active frame. Each frame holds one cel on the active layer with the generated pixels.

## Cost and latency

- Typical: ~20s, ~$0.05 per call.
- Max: ~120s, ~$0.20 per call (slow backend, three frames, large canvas).

Local backends (Ollama, ComfyUI) report latency only — no dollar cost.

## Example

You've drawn three frames of a slime enemy compressing for a jump (frames 4, 5, 6 on the `body` layer). Open `examples/samples/enemy-slime.pixhaus`, place the playhead on frame 6, run `AI > Continue` with `num_frames: 2`. The verb sends frames 4–6 as context, asks the backend for two more frames, snaps them to the slime's palette, and inserts them as frames 7 and 8.

The preview appears in the AI panel. Press Enter to commit; the timeline now has eight frames with the new pair holding for the same duration as frame 6 (you didn't override `frame_duration_ms`).

## Related verbs

- [Inbetween](/ai-verbs/inbetween/) — generate frames between two existing keys instead of past them.
- [Cleanup](/ai-verbs/cleanup/) — palette-snap and de-fringe the generated frames if the model produced soft edges.
- [Extend](/ai-verbs/extend/) — generate alternate directional views of a single frame rather than continuing motion.
