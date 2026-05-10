---
title: Audio-driven timing
description: Detect beats or syllables in audio and place frame timing markers at the detected times.
---

## What it does

Audio-driven timing turns a clip of audio into animation timing. In beat mode, it finds the energy onsets in the track and produces frames whose durations match the inter-beat intervals, then tags the run so you can loop or scrub it as a unit. In lip-sync mode, it does the same and adds a `Mouth` layer with one cel per frame, each marked `open` or `closed`, so the artist has a scaffold to draw into.

The verb runs entirely on the local CPU using a classical energy-envelope onset detector. No inference backend is required.

## Parameters

- **`audio_bytes`** — `array<integer>` (required). Raw audio bytes. Must be PCM WAV (RIFF/WAVE, format code 1). 8, 16, 24, and 32-bit signed PCM are supported. MP3, OGG, and FLAC are detected and rejected with instructions to convert first.
- **`format`** — `string` (default: `wav`). One of `wav`, `mp3`, `ogg`, `flac`, `unknown`. With `unknown`, the verb sniffs magic bytes.
- **`mode`** — `string` (default: `beat`). One of `beat` or `lip_sync`. Beat mode emits frames and a tag. Lip-sync mode also emits a mouth layer.
- **`fps`** — `number` (default: `12.0`, range `1.0`–`120.0`). Target animation frame rate. Detected onsets are snapped to the nearest frame boundary at this rate.
- **`start_frame`** — `integer` (default: `0`). Insertion point. The first new frame lands after this 0-based frame index. `0` prepends.
- **`sensitivity`** — `number` (default: `0.5`, range `0.0`–`1.0`). Onset detection threshold as a fraction of the maximum observed energy difference. Higher values keep more beats; lower values keep only the strongest hits.
- **`tag_name`** — `string` (optional). Name for the generated frame tag. Defaults to `Beat` in beat mode and `Lip sync` in lip-sync mode.
- **`layer_name`** — `string` (optional). Name for the mouth-shape layer in lip-sync mode. Defaults to `Mouth`. Ignored in beat mode.

## Backend requirements

This verb runs entirely on the local CPU and does not require an inference backend.

## Output

- **`AddFrames`** — inserts one frame per detected onset, with per-frame durations derived from the gaps between onsets.
- **`AddTag`** — names the inserted range so you can loop, scrub, or rename it from the timeline.
- **`AddLayer`** — only in lip-sync mode. Creates the mouth layer and seeds each frame with a 1×1 placeholder cel: white for an `open` frame, black for a `closed` frame. Each cel carries `user_data.text` of `"open"` or `"closed"` so the artist can filter and replace the placeholders with real mouth art.

## Cost and latency

- Typical: ~200 ms, free.
- Max: ~2 s, free.

## Example

You have a 4-second voiceover clip for a character introduction and you want a mouth-shape scaffold synced to the syllables. Convert the recording to PCM WAV, drop it into the verb panel with `mode: lip_sync` and `fps: 12`. The verb returns a preview that adds 18 frames to the timeline, tags them `Lip sync`, and creates a `Mouth` layer with alternating `open`/`closed` placeholder cels. Accept the preview, hide the placeholder cels, and draw your real mouth shapes into the existing slots — the timing is already correct.

For beat mode, drop a 1-bar drum loop in at `mode: beat`, `sensitivity: 0.4`, `fps: 24`. The verb finds the kick and snare hits and lays out a tagged loop you can attach to a walk cycle or attack animation. See `examples/samples/dialogue-intro.pixhaus` for a worked example.

## Related verbs

- [Motion from video](/ai-verbs/motion-from-video/) — same skeleton-then-art pattern, but the timing comes from a reference video instead of audio.
- [Inbetween](/ai-verbs/inbetween/) — fill the gaps between the placeholder mouth cels with interpolated frames.
- [Continue](/ai-verbs/continue/) — extend a tagged beat loop with predicted frames in the same rhythm.
