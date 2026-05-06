# Inbetween

Generates intermediate frames between two key frames using a frame-interpolation backend.

## What it does

You pick two frames in your animation (A and B) and ask the verb how many frames to add between them. The verb calls a frame-interpolation model (RIFE-class or video diffusion) with both frames and gets back N new frames that smoothly bridge the two poses.

If your project has an active palette, each output pixel is snapped to the nearest palette color by squared Euclidean distance in RGB space. Transparent pixels (alpha == 0) are not snapped.

The result is a set of new frames inserted into the timeline immediately after frame A, with one cel per frame on the active layer.

## When to use it

- Rough walk cycles where you have key poses but need the inbetweens
- Any animation where the motion between two frames is too abrupt
- Quickly filling out an animation to see if the timing works before committing to hand-drawn inbetweens

## Inputs

| Field | Type | Default | Description |
|---|---|---|---|
| `frame_a` | PixelData (RGBA8) | required | First key frame |
| `frame_b` | PixelData (RGBA8) | required | Second key frame |
| `frame_a_index` | integer | required | Timeline position of frame A |
| `frame_b_index` | integer | required | Timeline position of frame B (must be > `frame_a_index`) |
| `num_outputs` | integer, 1–16 | 1 | Number of intermediate frames to generate |

Both frames must have identical dimensions and be RGBA8 (4 bytes per pixel).

## Output

A single `AddFrames` effect:

- `num_outputs` new frames inserted after `frame_a_index`
- One cel per frame on the active layer
- Pixels snapped to the active palette (if present)

The active layer receives all generated cels. If you want the interpolated frames on a separate layer, duplicate the layer first and run the verb on the copy.

## Backend requirements

Requires a backend with `FRAME_INTERPOLATION` capability. The verb runtime selects the highest-priority configured backend that satisfies this requirement.

Likely backends: Replicate (RIFE, EMA-VFI, or similar), ComfyUI with a video interpolation workflow. See the preferences panel to configure backends.

If no `FRAME_INTERPOLATION` backend is configured, the verb fails before calling `invoke`.

## Cost estimate

| | Typical | Maximum |
|---|---|---|
| Latency | 30 s | 5 min |
| Cost (USD) | $0.05/frame | $0.20/frame |

Estimates depend on the backend and model. Replicate's per-second pricing means short clips are cheap and long ones add up quickly.

## Notes

- The palette snap is a post-process: whatever the model returns gets snapped. It preserves the feel of your palette but does not guarantee perfect anti-aliasing removal. Use the Cleanup verb afterwards if the output has fringing.
- If the two input frames have very different poses, the interpolation model may produce a blurry or unrealistic middle frame. Reducing `num_outputs` (1 or 2) gives the model a harder but more achievable task.
- The verb does not know about the layer's blend mode or opacity. Generated cels inherit the layer's settings at render time.
