---
title: Cleanup
description: Snap a cel to the project palette, remove anti-aliasing, and detect pivot drift.
---

## What it does

Cleanup post-processes the active cel to make it conform to the project's palette and pixel-art conventions. It snaps every opaque pixel to the nearest palette color, optionally removes the semi-transparent fringe that image generators and PSD imports leave around shapes, and flags slices whose pivot keys have drifted across animation frames.

The verb runs entirely on the local CPU. Use it as a final pass after Inbetween, Continue, or Extend, or after pulling a layer in from an external source.

## Parameters

- **`pixels`** — `object` (required). The active cel as RGBA8 pixel data. Must include `width`, `height`, `bytes_per_pixel` (always `4`), `stride`, and a `bytes` array. The host fills this in from the active layer's cel at the active frame.
- **`alpha_threshold`** — `integer` (default: `0`, range `0`–`254`). Pixels with alpha at or below this value are zeroed to fully transparent. `0` only clears pixels with `a == 0`; `128` would also strip anything half-transparent or below. `255` is rejected because it would erase the whole buffer.
- **`fix_antialiasing`** — `boolean` (default: `true`). When on, semi-transparent pixels touching opaque content are snapped to the nearest palette color at full opacity, and isolated semi-transparent pixels become fully transparent.
- **`fix_pivot_drift`** — `boolean` (default: `true`). When on, the verb inspects every slice's per-frame pivot keys and reports any slice whose pivot deviates from its base key by more than two pixels on either axis. No structural edits are committed — drift is surfaced as a note for you to address in the slice panel.

## Backend requirements

This verb runs entirely on the local CPU and does not require an inference backend.

## Output

- **`ReplaceCels`** — replaces the active cel with the cleaned pixel buffer. The existing layer and frame stay where they are; only the pixels change. Pivot drift findings appear in `notes` on the verb output, not as edits.

## Cost and latency

- Typical: sub-second on a sprite up to a few hundred pixels per side. No API spend.
- Max: scales linearly with pixel count. Free either way.

## Example

You finish a Continue run that adds three new walk-cycle frames. The generator picked colors that are close to your palette but not exact, and left a halo of half-transparent pixels around the silhouette. Run Cleanup with the defaults: every pixel snaps to the nearest palette entry, the halo collapses into the silhouette where it touches solid color, and the rest of the halo zeros out.

If you also have a `body` slice with a pivot at `(8, 8)` on frame 0 that crept to `(8, 16)` by frame 3, Cleanup leaves the slice alone but appends a note: `Pivot drift detected in 1 slice: body. Review pivot keys in the slice panel.` Open `examples/samples/character-knight.pixhaus` and run Cleanup against the imported `armor` layer to see the palette snap on a realistic input.

## Related verbs

- [Inbetween](/ai-verbs/inbetween/) — pair with Cleanup to remove the soft fringe interpolation models tend to produce.
- [Continue](/ai-verbs/continue/) — same — snap generated frames to your palette before they enter the timeline proper.
- [Variant](/ai-verbs/variant/) — useful after a palette-swap variant if the swap was AI-refined rather than index-based.
