---
title: Inbetween
description: Generate intermediate frames between two key frames.
---

## What it does

Inbetween takes two pixel-art frames you've already drawn and generates one or more intermediate frames between them. The verb sends both keys to a frame-interpolation backend (RIFE-class or video diffusion), receives the requested number of in-between frames back, and — when the project has an active palette — snaps every output pixel to the nearest palette color.

Use it to fill in the breakdowns and tweens between hand-drawn keys: anticipation to extreme, contact to recoil, idle pose A to idle pose B.

## Parameters

- **`frame_a`** — `object` (required). RGBA8 pixel data of the first key frame.
- **`frame_b`** — `object` (required). RGBA8 pixel data of the second key frame. Must match `frame_a`'s width and height.
- **`frame_a_index`** — `integer` (required, minimum `0`). Timeline index of frame A. Generated cels are inserted immediately after this position.
- **`frame_b_index`** — `integer` (required, minimum `1`). Timeline index of frame B. Must be strictly greater than `frame_a_index`. Used for validation only; the verb does not move frame B.
- **`num_outputs`** — `integer` (default: `1`, range `1`–`16`). Number of intermediate frames to generate. The hard cap of 16 guards against runaway calls that would burn backend quota.

## Backend requirements

- **`FRAME_INTERPOLATION`** — needed to interpolate motion between the two source frames rather than generating from scratch.

The runtime selects a backend that advertises the capability (Replicate, Stability, ComfyUI with a RIFE workflow). The verb dispatches through `BackendProxy`; backends without the proxy attached are rejected at invoke time with a clear error.

## Output

- **`AddFrames`** — inserts `num_outputs` new frames into the active sprite immediately after `frame_a_index`. Each frame holds one cel on the active layer with the generated, palette-snapped pixels. When the project has no active palette, the verb commits the raw backend output and appends a note to that effect.

## Cost and latency

- Typical: ~30s, ~$0.05 per call.
- Max: ~300s, ~$0.20 per call (large canvas, 16 outputs, slow backend).

## Example

You've drawn frame 0 of a sword swing (sword raised) and frame 5 (sword fully down). Open the sprite, select the active layer, run `AI > Inbetween` with `frame_a_index: 0`, `frame_b_index: 5`, `num_outputs: 4`. The verb sends both frames to the backend, receives four PNGs back, snaps each to the active palette, and inserts them as frames 1, 2, 3, 4 — the original frame 5 is renumbered as frame 5 still, but it now follows the four new frames.

`examples/samples/character-knight.pixhaus` has a two-frame attack pose useful for trying this against. The default `num_outputs: 1` is the right starting point — generate one inbetween, accept or reject, then iterate.

## Related verbs

- [Continue](/ai-verbs/continue/) — generate frames after the last existing frame instead of between two keys.
- [Cleanup](/ai-verbs/cleanup/) — useful when the project has no palette and the raw backend output needs a manual snap pass.
- [Extend](/ai-verbs/extend/) — generate alternate directional views of a single frame rather than time-based interpolation.
