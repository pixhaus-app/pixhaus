---
title: Motion from video
description: Extract a timing skeleton and pose reference layers from a reference video.
---

## What it does

Motion from video turns a reference clip into a timing skeleton on the timeline. It analyses pixel motion across the supplied frames, picks the keyframes where motion crosses a sensitivity threshold, and adds them to the active animation tag with durations that match the source timestamps. Each keyframe also gets a silhouette thumbnail on a `Pose reference` layer for the artist to trace.

The verb owns the timing; you own the pixel art. AI does not draw the final frames — it gives you the rhythm and the rough shapes, and you fill in the actual sprites. The verb runs entirely on the local CPU. A `POSE_ESTIMATION`-capable backend (planned for S22) will replace the pixel-difference analysis with proper pose extraction in a future stream.

## Parameters

- **`frames`** — `array<object>` (required, minimum 2). The reference video as a sequence of RGBA8 frames. Each entry is an object with:
  - **`pixels`** — RGBA8 pixel data with `width`, `height`, `bytes_per_pixel`, `stride`, and `bytes`. All frames must share the same dimensions.
  - **`timestamp_ms`** — presentation time in milliseconds from the video start. Timestamps must be strictly increasing.
- **`tag_name`** — `string` (optional). Name for the resulting frame tag. Defaults to `Motion`.
- **`keyframe_sensitivity`** — `number` (default: `0.25`, range `0.0`–`1.0`). Threshold for keyframe selection. `0.0` keeps every frame; `0.25` keeps frames with at least 25 percent of the maximum observed motion; `1.0` keeps only the most motion-heavy frame plus the mandatory first and last frames.

## Backend requirements

This verb runs entirely on the local CPU and does not require an inference backend.

## Output

- **`AddLayer`** — creates the `Pose reference` layer. Cels live in the next effect so they are added alongside the frames they reference.
- **`AddFrames`** — inserts one frame per selected keyframe. Each frame carries a silhouette cel on the pose layer: opaque pixels are darkened to a dim blue-grey; transparent pixels stay transparent. Frame durations come from the gaps between source-video timestamps; the last keyframe gets the median of the others (or 100 ms when there is only one).
- **`AddTag`** — names the inserted range so the new keyframes loop or scrub as a unit.

The output includes a note reminding you that the pose layer is reference-only — fill in the real pixel art on a sibling layer.

## Cost and latency

- Typical: free, sub-second for clips of a few seconds at modest resolution.
- Max: free, scales with the total number of pixels analysed.

## Example

You have a 2-second clip of a person performing a kick at 30 fps — 60 frames. Decode the video to RGBA8 frames with timestamps, drop them in with the defaults (`keyframe_sensitivity: 0.25`, `tag_name: "Motion"`). The verb finds the eight or so frames where the leg actually moves — windup, mid-kick, impact, recovery — and adds them to the timeline with durations matching the original timing.

The `Pose reference` layer renders dim blue-grey silhouettes of the source frames at each keyframe. Open `examples/samples/character-knight.pixhaus`, add a sibling layer above the pose layer, and trace the silhouettes one by one to draw the actual pixel-art kick. Crank `keyframe_sensitivity` toward `1.0` if the verb selects too many keyframes; drop it toward `0.0` if you want more in-between reference poses.

## Related verbs

- [Audio-driven timing](/ai-verbs/audio-driven-timing/) — same skeleton-then-art pattern, but timing comes from audio onsets instead of video motion.
- [Inbetween](/ai-verbs/inbetween/) — fill the gaps between two traced keyframes with interpolated frames.
- [Auto-mesh deformation](/ai-verbs/auto-mesh-deformation/) — drive a rigged sprite from the same reference clip without redrawing.
